use env;
use error::{Error, Result};
use log_entry::LogEntry;
use parser;
use primitive_types::U256;
use provider::Provider;
use runtime::Runtime;
use schedule::Schedule;
use state::State;
use transaction::{Action, Transaction};
use address::Address;
use types::{ActionParams, ActionType};
use utils;
use wasm_cost::WasmCosts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultData {
	pub gas_left: U256,
	pub data: Vec<u8>,
	pub contract: Address,
	pub logs: Vec<LogEntry>,
}

pub fn execute(transaction: &Transaction, provider: &mut dyn Provider) -> Result<ResultData> {
	let params = match &transaction.action {
		Action::Create(code, salt) => {
			let new_address = utils::contract_address(&transaction.sender, &code, &salt);

			ActionParams {
				code_address: new_address.clone(),
				address: new_address.clone(),
				sender: transaction.sender.clone(),
				origin: transaction.sender.clone(),
				gas: transaction.gas,
				gas_price: transaction.gas_price,
				value: transaction.value,
				action_type: ActionType::Create,
				code: code.clone(),
				args: transaction.args.clone(),
				code_hash: None,
				code_version: U256::zero(),
			}
		}
		Action::Call(address) => {
			let acc = provider.account(&address)?;
			let code = acc.code.clone();
			ActionParams {
				code_address: address.clone(),
				address: address.clone(),
				sender: transaction.sender.clone(),
				origin: transaction.sender.clone(),
				gas: transaction.gas,
				gas_price: transaction.gas_price,
				value: transaction.value,
				action_type: ActionType::Call,
				code: code,
				args: transaction.args.clone(),
				code_hash: None,
				code_version: U256::zero(),
			}
		}
	};

	let mut schedule = Schedule::default();
	let wasm = WasmCosts::default();
	schedule.wasm = Some(wasm);

	let module = parser::payload(&params, schedule.wasm())?;
	let loaded_module = wasmi::Module::from_parity_wasm_module(module)?;
	let instantiation_resolver = env::ImportResolver::with_limit(16, schedule.wasm());
	let module_instance = wasmi::ModuleInstance::new(
		&loaded_module,
		&wasmi::ImportsBuilder::new().with_resolver("env", &instantiation_resolver),
	)?;

	let adjusted_gas = params.gas * U256::from(schedule.wasm().opcodes_div)
		/ U256::from(schedule.wasm().opcodes_mul);

	if adjusted_gas > ::std::u64::MAX.into() {
		return Err(Error::Wasm {
			msg: "Wasm interpreter cannot run contracts with gas (wasm adjusted) >= 2^64"
				.to_owned(),
		});
	}

	let initial_memory = instantiation_resolver.memory_size()?;
	trace!(target: "wasm", "Contract requested {:?} pages of initial memory", initial_memory);

	let mut state = State::new(provider);
	let mut runtime = Runtime::new(
		&params,
		&schedule,
		&mut state,
		instantiation_resolver.memory_ref(),
		// cannot overflow, checked above
		adjusted_gas.low_u64(),
	);

	// cannot overflow if static_region < 2^16,
	// initial_memory ∈ [0..2^32)
	// total_charge <- static_region * 2^32 * 2^16
	// total_charge ∈ [0..2^64) if static_region ∈ [0..2^16)
	// qed
	assert!(runtime.schedule().wasm().initial_mem < 1 << 16);
	runtime.charge(|s| initial_memory as u64 * s.wasm().initial_mem as u64)?;

	let instance = module_instance.run_start(&mut runtime)?;
	let invoke_result = instance.invoke_export("call", &[], &mut runtime);

	if let Err(wasmi::Error::Trap(ref trap)) = invoke_result {
		if let wasmi::TrapKind::Host(ref boxed) = *trap.kind() {
			let ref runtime_err = boxed
				.downcast_ref::<Error>()
				.expect("Host errors other than runtime::Error never produced; qed");

			let mut have_error = false;
			match **runtime_err {
				Error::Suicide => {
					debug!("Contract suicided.");
				}
				Error::Return => {
					debug!("Contract returned.");
				}
				_ => {
					have_error = true;
				}
			}
			if let (true, Err(e)) = (have_error, invoke_result) {
				trace!(target: "wasm", "Error executing contract: {:?}", e);
				return Err(Error::from(e));
			}
		}
	}

	let gas_left = runtime
		.gas_left()
		.expect("Cannot fail since it was not updated since last charge");
	let result = runtime.into_result();
	let gas_left_adj = U256::from(gas_left) * U256::from(schedule.wasm().opcodes_mul)
		/ U256::from(schedule.wasm().opcodes_div);

	if result.is_empty() {
		trace!(target: "wasm", "Contract execution result is empty.");
		Ok(ResultData {
			gas_left: gas_left_adj,
			data: vec![],
			contract: params.address,
			// TODO::::: logs????
			logs: vec![], // ext.logs().to_vec(),
		})
	} else {
		if let Action::Create(_, _) = &transaction.action {
			runtime.init_code(&params.address, result.to_vec());
		}

		runtime.update_state()?;

		Ok(ResultData {
			gas_left: gas_left_adj,
			data: result.to_vec(),
			contract: params.address,
			// TODO::::: logs????
			logs: vec![], // ext.logs().to_vec(),
		})
	}
}
