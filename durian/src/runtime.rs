use error::{Error};
use log_entry::LogEntry;
use panic_payload;
use primitive_types::{H256, U256};
use schedule::Schedule;
use state::State;
use address::Address;
use types::{ActionParams, ActionType};
use wasmi::{MemoryRef, RuntimeArgs, RuntimeValue};

pub struct Runtime<'a> {
	schedule: &'a Schedule,
	gas_counter: u64,
	gas_limit: u64,
	params: &'a ActionParams,
	memory: MemoryRef,
	result: Vec<u8>,
	state: &'a mut State<'a>,
	logs: Vec<LogEntry>,
}

impl<'a> Runtime<'a> {
	/// New runtime for wasm contract with specified params
	pub fn new(
		params: &'a ActionParams,
		schedule: &'a Schedule,
		state: &'a mut State<'a>,
		memory: MemoryRef,
		gas_limit: u64,
	) -> Self {
		Runtime {
			schedule: schedule,
			gas_counter: 0,
			gas_limit: gas_limit,
			memory: memory,
			params: params,
			state: state,
			logs: Vec::new(),
			result: Vec::new(),
		}
	}

	/// Loads 256-bit hash from the specified sandboxed memory pointer
	fn h256_at(&self, ptr: u32) -> Result<H256, Error> {
		let mut buf = [0u8; 32];
		self.memory.get_into(ptr, &mut buf[..])?;

		Ok(H256::from_slice(&buf[..]))
	}

	/// Loads 160-bit hash (Ethereum address) from the specified sandboxed memory pointer
	fn address_at(&self, ptr: u32) -> Result<Address, Error> {
		let mut buf = [0u8; 20];
		self.memory.get_into(ptr, &mut buf[..])?;

		Ok(Address::from_slice(&buf[..]))
	}

	/// Loads 256-bit integer represented with bigendian from the specified sandboxed memory pointer
	fn u256_at(&self, ptr: u32) -> Result<U256, Error> {
		let mut buf = [0u8; 32];
		self.memory.get_into(ptr, &mut buf[..])?;

		Ok(U256::from_big_endian(&buf[..]))
	}

	/// Charge specified amount of gas
	///
	/// Returns false if gas limit exceeded and true if not.
	/// Intuition about the return value sense is to aswer the question 'are we allowed to continue?'
	fn charge_gas(&mut self, amount: u64) -> bool {
		let prev = self.gas_counter;
		match prev.checked_add(amount) {
			// gas charge overflow protection
			None => false,
			Some(val) if val > self.gas_limit => false,
			Some(_) => {
				self.gas_counter = prev + amount;
				true
			}
		}
	}

	/// Charge gas according to closure
	pub fn charge<F>(&mut self, f: F) -> Result<(), Error>
	where
		F: FnOnce(&Schedule) -> u64,
	{
		let amount = f(self.schedule());
		if !self.charge_gas(amount as u64) {
			Err(Error::GasLimit)
		} else {
			Ok(())
		}
	}

	/// Adjusted charge of gas which scales actual charge according to the wasm opcode counting coefficient
	pub fn adjusted_charge<F>(&mut self, f: F) -> Result<(), Error>
	where
		F: FnOnce(&Schedule) -> u64,
	{
		self.charge(|schedule| {
			f(schedule) * schedule.wasm().opcodes_div as u64 / schedule.wasm().opcodes_mul as u64
		})
	}

	/// Charge gas provided by the closure
	///
	/// Closure also can return overflowing flag as None in gas cost.
	pub fn overflow_charge<F>(&mut self, f: F) -> Result<(), Error>
	where
		F: FnOnce(&Schedule) -> Option<u64>,
	{
		let amount = match f(self.schedule) {
			Some(amount) => amount,
			None => {
				return Err(Error::GasLimit.into());
			}
		};

		if !self.charge_gas(amount as u64) {
			Err(Error::GasLimit.into())
		} else {
			Ok(())
		}
	}

	/// Same as overflow_charge, but with amount adjusted by wasm opcodes coeff
	pub fn adjusted_overflow_charge<F>(&mut self, f: F) -> Result<(), Error>
	where
		F: FnOnce(&Schedule) -> Option<u64>,
	{
		self.overflow_charge(|schedule| {
			f(schedule)
				.and_then(|x| x.checked_mul(schedule.wasm().opcodes_div as u64))
				.map(|x| x / schedule.wasm().opcodes_mul as u64)
		})
	}

	/// Read from the storage to wasm memory
	pub fn storage_read(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let key = self.h256_at(args.nth_checked(0)?)?;
		let val_ptr: u32 = args.nth_checked(1)?;

		let val = self.state.storage_at(&self.params.address, &key)?;

		self.adjusted_charge(|schedule| schedule.sload_gas as u64)?;

		self.memory.set(val_ptr as u32, val.as_bytes())?;

		Ok(())
	}

	/// Write to storage from wasm memory
	pub fn storage_write(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let key = self.h256_at(args.nth_checked(0)?)?;
		let val_ptr: u32 = args.nth_checked(1)?;

		let val = self.h256_at(val_ptr)?;
		let former_val = self.state.storage_at(&self.params.address, &key)?;

		if former_val == H256::zero() && val != H256::zero() {
			self.adjusted_charge(|schedule| schedule.sstore_set_gas as u64)?;
		} else {
			self.adjusted_charge(|schedule| schedule.sstore_reset_gas as u64)?;
		}

		self.state.set_storage(&self.params.address, &key, &val);

		if former_val != H256::zero() && val == H256::zero() {
			let sstore_clears_schedule = self.schedule().sstore_refund_gas;
			self.add_sstore_refund(sstore_clears_schedule);
		}

		Ok(())
	}

	/// Return currently used schedule
	pub fn schedule(&self) -> &Schedule {
		self.schedule
	}

	/// Sets a return value for the call
	///
	/// Syscall takes 2 arguments:
	/// * pointer in sandboxed memory where result is
	/// * the length of the result
	pub fn ret(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let ptr: u32 = args.nth_checked(0)?;
		let len: u32 = args.nth_checked(1)?;

		trace!(target: "wasm", "Contract ret: {} bytes @ {}", len, ptr);

		self.result = self.memory.get(ptr, len as usize)?;

		Err(Error::Return)
	}

	/// Destroy the runtime, returning currently recorded result of the execution
	pub fn into_result(&self) -> Vec<u8> {
		self.result.clone()
	}

	/// Query current gas left for execution
	pub fn gas_left(&self) -> Result<u64, Error> {
		if self.gas_counter > self.gas_limit {
			return Err(Error::InvalidGasState);
		}
		Ok(self.gas_limit - self.gas_counter)
	}

	/// General gas charging extern.
	fn gas(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let amount: u32 = args.nth_checked(0)?;
		if self.charge_gas(amount as u64) {
			Ok(())
		} else {
			Err(Error::GasLimit.into())
		}
	}

	/// Query the length of the input bytes
	fn input_legnth(&mut self) -> RuntimeValue {
		RuntimeValue::I32(self.params.args.len() as i32)
	}

	/// Write input bytes to the memory location using the passed pointer
	fn fetch_input(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let ptr: u32 = args.nth_checked(0)?;

		let args_len = self.params.args.len() as u64;
		self.charge(|s| args_len * s.wasm().memcpy as u64)?;

		self.memory.set(ptr, &self.params.args[..])?;
		Ok(())
	}

	/// User panic
	///
	/// Contract can invoke this when he encounters unrecoverable error.
	fn panic(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let payload_ptr: u32 = args.nth_checked(0)?;
		let payload_len: u32 = args.nth_checked(1)?;

		let raw_payload = self.memory.get(payload_ptr, payload_len as usize)?;
		let payload = panic_payload::decode(&raw_payload);
		let msg = format!(
			"{msg}, {file}:{line}:{col}",
			msg = payload
				.msg
				.as_ref()
				.map(String::as_ref)
				.unwrap_or("<msg was stripped>"),
			file = payload
				.file
				.as_ref()
				.map(String::as_ref)
				.unwrap_or("<unknown>"),
			line = payload.line.unwrap_or(0),
			col = payload.col.unwrap_or(0)
		);
		trace!(target: "wasm", "Contract custom panic message: {}", msg);

		Err(Error::Panic { msg })
	}

	fn do_call(
		&mut self,
		use_val: bool,
		call_type: ActionType,
		args: RuntimeArgs,
	) -> Result<RuntimeValue, Error> {
		trace!(target: "wasm", "runtime: CALL({:?})", call_type);

		let gas: u64 = args.nth_checked(0)?;
		trace!(target: "wasm", "           gas: {:?}", gas);

		let address = self.address_at(args.nth_checked(1)?)?;
		trace!(target: "wasm", "       address: {:?}", address);

		let vofs = if use_val { 1 } else { 0 };
		let val = if use_val {
			Some(self.u256_at(args.nth_checked(2)?)?)
		} else {
			None
		};
		trace!(target: "wasm", "           val: {:?}", val);

		let input_ptr: u32 = args.nth_checked(2 + vofs)?;
		trace!(target: "wasm", "     input_ptr: {:?}", input_ptr);

		let input_len: u32 = args.nth_checked(3 + vofs)?;
		trace!(target: "wasm", "     input_len: {:?}", input_len);

		let result_ptr: u32 = args.nth_checked(4 + vofs)?;
		trace!(target: "wasm", "    result_ptr: {:?}", result_ptr);

		let result_alloc_len: u32 = args.nth_checked(5 + vofs)?;
		trace!(target: "wasm", "    result_len: {:?}", result_alloc_len);

		if let Some(val) = val {
			let address_balance = self.state.balance(&self.params.address)?;

			if address_balance < val {
				trace!(target: "wasm", "runtime: call failed due to balance check");
				return Ok((-1i32).into());
			}
		}

		self.adjusted_charge(|schedule| schedule.call_gas as u64)?;

		let mut result = Vec::with_capacity(result_alloc_len as usize);
		result.resize(result_alloc_len as usize, 0);

		// todo: optimize to use memory views once it's in
		let _payload = self.memory.get(input_ptr, input_len as usize)?;

		let adjusted_gas = match gas
			.checked_mul(self.schedule.wasm().opcodes_div as u64)
			.map(|x| x / self.schedule.wasm().opcodes_mul as u64)
		{
			Some(x) => x,
			None => {
				trace!("CALL overflowed gas, call aborted with error returned");
				return Ok(RuntimeValue::I32(-1));
			}
		};

		self.charge(|_| adjusted_gas)?;

		// TODO: fix it later
		/*
		let call_result = self
			.ext
			.call(
				&gas.into(),
				match call_type {
					ActionType::DelegateCall => &self.context.sender,
					_ => &self.context.address,
				},
				match call_type {
					ActionType::Call | ActionType::StaticCall => &address,
					_ => &self.context.address,
				},
				val,
				&payload,
				&address,
				call_type,
				false,
			)
			.ok()
			.expect("Trap is false; trap error will not happen; qed");

		match call_result {
			MessageCallResult::Success(gas_left, data) => {
				let len = cmp::min(result.len(), data.len());
				(&mut result[..len]).copy_from_slice(&data[..len]);

				// cannot overflow, before making call gas_counter was incremented with gas, and gas_left < gas
				self.gas_counter = self.gas_counter
					- gas_left.low_u64() * self.schedule.wasm().opcodes_div as u64
						/ self.schedule.wasm().opcodes_mul as u64;

				self.memory.set(result_ptr, &result)?;
				Ok(0i32.into())
			}
			MessageCallResult::Reverted(gas_left, data) => {
				let len = cmp::min(result.len(), data.len());
				(&mut result[..len]).copy_from_slice(&data[..len]);

				// cannot overflow, before making call gas_counter was incremented with gas, and gas_left < gas
				self.gas_counter = self.gas_counter
					- gas_left.low_u64() * self.schedule.wasm().opcodes_div as u64
						/ self.schedule.wasm().opcodes_mul as u64;

				self.memory.set(result_ptr, &result)?;
				Ok((-1i32).into())
			}
			MessageCallResult::Failed => Ok((-1i32).into()),
		}
		*/
		Err(Error::Panic {
			msg: "not completed".to_string(),
		})
	}

	/// Message call
	fn ccall(&mut self, args: RuntimeArgs) -> Result<RuntimeValue, Error> {
		self.do_call(true, ActionType::Call, args)
	}

	// TODO: find the use cases?
	/*
	/// Delegate call
	fn dcall(&mut self, args: RuntimeArgs) -> Result<RuntimeValue> {
		self.do_call(false, ActionType::DelegateCall, args)
	}

	/// Static call
	fn scall(&mut self, args: RuntimeArgs) -> Result<RuntimeValue> {
		self.do_call(false, ActionType::StaticCall, args)
	}
	*/

	fn return_address_ptr(&mut self, ptr: u32, val: Address) -> Result<(), Error> {
		self.charge(|schedule| schedule.wasm().static_address as u64)?;
		self.memory.set(ptr, val.as_bytes())?;
		Ok(())
	}

	fn return_u256_ptr(&mut self, ptr: u32, val: U256) -> Result<(), Error> {
		let mut ret = H256::zero();
		val.to_big_endian(ret.as_bytes_mut());
		self.charge(|schedule| schedule.wasm().static_u256 as u64)?;
		self.memory.set(ptr, ret.as_bytes())?;
		Ok(())
	}

	/// Returns value (in Wei) passed to contract
	pub fn value(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let val = self.params.value;
		self.return_u256_ptr(args.nth_checked(0)?, val)
	}


	#[allow(dead_code)]
		fn do_create(
		&mut self,
		_endowment: U256,
		_code_ptr: u32,
		_code_len: u32,
		_result_ptr: u32,
	) -> Result<RuntimeValue, Error> {
		// TODO: Not completed
		/*
		let code = self.memory.get(code_ptr, code_len as usize)?;

		self.adjusted_charge(|schedule| schedule.create_gas as u64)?;
		self.adjusted_charge(|schedule| schedule.create_data_gas as u64 * code.len() as u64)?;

		let gas_left: U256 = U256::from(self.gas_left()?)
			* U256::from(self.schedule.wasm().opcodes_mul)
			/ U256::from(self.schedule.wasm().opcodes_div);


		match self
			.create(
				&gas_left,
				&endowment,
				&code,
				&self.context.code_version,
			)
			.ok()
			.expect("Trap is false; trap error will not happen; qed")
		{
			ContractCreateResult::Created(address, gas_left) => {
				self.memory.set(result_ptr, address.as_bytes())?;
				self.gas_counter = self.gas_limit -
					// this cannot overflow, since initial gas is in [0..u64::max) range,
					// and gas_left cannot be bigger
					gas_left.low_u64() * self.schedule.wasm().opcodes_div as u64
						/ self.schedule.wasm().opcodes_mul as u64;
				trace!(target: "wasm", "runtime: create contract success (@{:?})", address);
				Ok(0i32.into())
			}
			ContractCreateResult::Failed => {
				trace!(target: "wasm", "runtime: create contract fail");
				Ok((-1i32).into())
			}
			ContractCreateResult::Reverted(gas_left, _) => {
				trace!(target: "wasm", "runtime: create contract reverted");
				self.gas_counter = self.gas_limit -
					// this cannot overflow, since initial gas is in [0..u64::max) range,
					// and gas_left cannot be bigger
					gas_left.low_u64() * self.schedule.wasm().opcodes_div as u64
						/ self.schedule.wasm().opcodes_mul as u64;

				Ok((-1i32).into())
			}
		}
		*/
		Err(Error::Panic {
			msg: "Not completed".to_string(),
		})
	}

	/// Creates a new contract
	///
	/// Arguments:
	/// * endowment - how much value (in Wei) transfer to the newly created contract
	/// * code_ptr - pointer to the code data
	/// * code_len - lenght of the code data
	/// * result_ptr - pointer to write an address of the newly created contract
	pub fn create(&mut self, _args: RuntimeArgs) -> Result<RuntimeValue, Error> {
		//
		// method signature:
		//   fn create(endowment: *const u8, code_ptr: *const u8, code_len: u32, result_ptr: *mut u8) -> i32;
		//
		// TODO: Not completed
		/*
		trace!(target: "wasm", "runtime: CREATE");
		let endowment = self.u256_at(args.nth_checked(0)?)?;
		trace!(target: "wasm", "       val: {:?}", endowment);
		let code_ptr: u32 = args.nth_checked(1)?;
		trace!(target: "wasm", "  code_ptr: {:?}", code_ptr);
		let code_len: u32 = args.nth_checked(2)?;
		trace!(target: "wasm", "  code_len: {:?}", code_len);
		let result_ptr: u32 = args.nth_checked(3)?;
		trace!(target: "wasm", "result_ptr: {:?}", result_ptr);

		self.do_create(
			endowment,
			code_ptr,
			code_len,
			result_ptr,
			CreateContractAddress::FromSenderAndCodeHash,
		)
		*/

		Err(Error::Panic {
			msg: "Not completed".to_string(),
		})
	}

	fn debug(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		trace!(target: "wasm", "Contract debug message: {}", {
			let msg_ptr: u32 = args.nth_checked(0)?;
			let msg_len: u32 = args.nth_checked(1)?;

			String::from_utf8(self.memory.get(msg_ptr, msg_len as usize)?)
				.map_err(|_| Error::BadUtf8)?
		});

		Ok(())
	}

	/// Pass suicide to state runtime
	pub fn suicide(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let _refund_address = self.address_at(args.nth_checked(0)?)?;

		// TODO: Not completed
		/*
		if self
			.exists(&refund_address)
			.map_err(|_| Error::SuicideAbort)?
		{
			trace!(target: "wasm", "Suicide: refund to existing address {}", refund_address);
			self.adjusted_charge(|schedule| schedule.suicide_gas as u64)?;
		} else {
			trace!(target: "wasm", "Suicide: refund to new address {}", refund_address);
			self.adjusted_charge(|schedule| schedule.suicide_to_new_account_cost as u64)?;
		}

		self.ext
			.suicide(&refund_address)
			.map_err(|_| Error::SuicideAbort)?;

		// We send trap to interpreter so it should abort further execution
		Err(Error::Suicide.into())
		*/
		Err(Error::Panic {
			msg: "Not completed".to_string(),
		})
	}

	///	Signature: `fn block_hash(number: i64, dest: *mut u8)`
	pub fn block_hash(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		self.adjusted_charge(|schedule| schedule.blockhash_gas as u64)?;
		let hash = self.state.block_hash(args.nth_checked::<u64>(0)?)?;
		self.memory.set(args.nth_checked(1)?, hash.as_bytes())?;

		Ok(())
	}

	///	Signature: `fn blocknumber() -> i64`
	pub fn block_number(&mut self) -> Result<RuntimeValue, Error> {
		Ok(RuntimeValue::from(self.state.block_number()))
	}

	///	Signature: `fn block_author(dest: *mut u8)`
	pub fn block_author(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let author = self.state.block_author()?;
		self.return_address_ptr(args.nth_checked(0)?, author)
	}

	///	Signature: `fn difficulty(dest: *mut u8)`
	pub fn difficulty(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let difficulty = self.state.difficulty()?;
		self.return_u256_ptr(args.nth_checked(0)?, difficulty)
	}

	///	Signature: `fn gaslimit(dest: *mut u8)`
	pub fn gaslimit(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let gas_limit = self.state.gas_limit()?;
		self.return_u256_ptr(args.nth_checked(0)?, gas_limit)
	}

	///	Signature: `timestamp() -> i64`
	pub fn timestamp(&mut self) -> Result<RuntimeValue, Error> {
		let timestamp = self.state.timestamp();
		Ok(RuntimeValue::from(timestamp))
	}

	///	Signature: `fn gasleft() -> i64`
	pub fn gasleft(&mut self) -> Result<RuntimeValue, Error> {
		Ok(RuntimeValue::from(
			self.gas_left()? * self.schedule.wasm().opcodes_mul as u64
				/ self.schedule.wasm().opcodes_div as u64,
		))
	}

	///	Signature: `fn address(dest: *mut u8)`
	pub fn address(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let address = self.params.address;
		self.return_address_ptr(args.nth_checked(0)?, address)
	}

	///	Signature: `sender(dest: *mut u8)`
	pub fn sender(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let sender = self.params.sender;
		self.return_address_ptr(args.nth_checked(0)?, sender)
	}

	///	Signature: `origin(dest: *mut u8)`
	pub fn origin(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let origin = self.params.origin;
		self.return_address_ptr(args.nth_checked(0)?, origin)
	}

	///	Signature: `fn elog(topic_ptr: *const u8, topic_count: u32, data_ptr: *const u8, data_len: u32)`
	pub fn elog(&mut self, args: RuntimeArgs) -> Result<(), Error> {
		let topic_ptr: u32 = args.nth_checked(0)?;
		let topic_count: u32 = args.nth_checked(1)?;
		let data_ptr: u32 = args.nth_checked(2)?;
		let data_len: u32 = args.nth_checked(3)?;

		if topic_count > 4 {
			return Err(Error::Log.into());
		}

		self.adjusted_overflow_charge(|schedule| {
			let topics_gas =
				schedule.log_gas as u64 + schedule.log_topic_gas as u64 * topic_count as u64;
			(schedule.log_data_gas as u64)
				.checked_mul(schedule.log_data_gas as u64)
				.and_then(|data_gas| data_gas.checked_add(topics_gas))
		})?;

		let mut topics: Vec<H256> = Vec::with_capacity(topic_count as usize);
		topics.resize(topic_count as usize, H256::zero());
		for i in 0..topic_count {
			let offset = i
				.checked_mul(32)
				.ok_or(Error::MemoryAccessViolation)?
				.checked_add(topic_ptr)
				.ok_or(Error::MemoryAccessViolation)?;

			*topics.get_mut(i as usize)
				.expect("topics is resized to `topic_count`, i is in 0..topic count iterator, get_mut uses i as an indexer, get_mut cannot fail; qed")
				= H256::from_slice(&self.memory.get(offset, 32)?[..]);
		}

		let data = self.memory.get(data_ptr, data_len as usize)?;

		self.logs.push(LogEntry {
			address: self.params.address.clone(),
			topics: topics,
			data: data.to_vec()
		});

		Ok(())
	}

	fn add_sstore_refund(&mut self, _value: usize) {
		// TODO: Better calculate the gas after flushing the state
		//self.substate.sstore_clears_refund += value as i128;
	}

	pub fn init_code(&mut self, address: &Address, code: Vec<u8>) {
		self.state.init_code(address, code);
	}

	pub fn update_state(&mut self) -> Result<(), Error> {
		self.state.update_state()
	}
}

mod ext_impl {

	use env::ids::*;
	use wasmi::{Externals, RuntimeArgs, RuntimeValue, Trap};

	macro_rules! void {
		{ $e: expr } => { { $e?; Ok(None) } }
	}

	macro_rules! some {
		{ $e: expr } => { { Ok(Some($e?)) } }
	}

	macro_rules! cast {
		{ $e: expr } => { { Ok(Some($e)) } }
	}

	impl<'a> Externals for super::Runtime<'a> {
		fn invoke_index(
			&mut self,
			index: usize,
			args: RuntimeArgs,
		) -> Result<Option<RuntimeValue>, Trap> {
			match index {
				STORAGE_WRITE_FUNC => void!(self.storage_write(args)),
				STORAGE_READ_FUNC => void!(self.storage_read(args)),
				RET_FUNC => void!(self.ret(args)),
				GAS_FUNC => void!(self.gas(args)),
				INPUT_LENGTH_FUNC => cast!(self.input_legnth()),
				FETCH_INPUT_FUNC => void!(self.fetch_input(args)),
				PANIC_FUNC => void!(self.panic(args)),
				DEBUG_FUNC => void!(self.debug(args)),
				CCALL_FUNC => some!(self.ccall(args)),
				//DCALL_FUNC => some!(self.dcall(args)),
				//SCALL_FUNC => some!(self.scall(args)),
				VALUE_FUNC => void!(self.value(args)),
				CREATE_FUNC => some!(self.create(args)),
				SUICIDE_FUNC => void!(self.suicide(args)),
				BLOCK_HASH_FUNC => void!(self.block_hash(args)),
				BLOCK_NUMBER_FUNC => some!(self.block_number()),
				BLOCK_AUTHOR_FUNC => void!(self.block_author(args)),
				DIFFICULTY_FUNC => void!(self.difficulty(args)),
				GASLIMIT_FUNC => void!(self.gaslimit(args)),
				TIMESTAMP_FUNC => some!(self.timestamp()),
				ADDRESS_FUNC => void!(self.address(args)),
				SENDER_FUNC => void!(self.sender(args)),
				ORIGIN_FUNC => void!(self.origin(args)),
				ELOG_FUNC => void!(self.elog(args)),
				//CREATE2_FUNC => some!(self.create2(args)),
				GASLEFT_FUNC => some!(self.gasleft()),
				_ => panic!("env module doesn't provide function at index {}", index),
			}
		}
	}
}
