#[macro_use]
extern crate capnp_rpc;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

extern crate simple_logger;

pub mod durian_capnp {
    include!(concat!(env!("OUT_DIR"), "/durian_capnp.rs"));
}

mod provider_impl;

use blockchain::blockchain::Blockchain;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use durian::address::Address;
use durian::transaction::{Action, Transaction};
use durian_capnp::executor;
use futures::task::LocalSpawn;
use futures::AsyncReadExt;
use futures::FutureExt;
use log::Level;
use primitive_types::{H256, U256};
use provider_impl::ProviderImpl;
use std::fs::File;
use std::io::Read;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
    pub static ref BC: Arc<Mutex<Blockchain>> = Arc::new(Mutex::new(Blockchain::new()));
}

fn main() {
    simple_logger::init_with_level(Level::Debug).unwrap();

    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 2 {
        println!("usage: {} HOST:PORT", args[0]);
        return;
    }

    let addr = args[1]
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("could not parse address");

    let mut exec = futures::executor::LocalPool::new();
    let spawner = exec.spawner();

    let _result: Result<(), Box<dyn std::error::Error>> = exec.run_until(async move {
        let stream = async_std::net::TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;
        let (reader, writer) = stream.split();
        let rpc_network = Box::new(twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        ));
        let mut rpc_system = RpcSystem::new(rpc_network, None);
        let executor: executor::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

        spawner
            .spawn_local_obj(Box::pin(rpc_system.map(|_| ())).into())
            .unwrap();

        // -------------------------------------
        let provider_impl = ProviderImpl::new(&BC);
        let provider: durian_capnp::provider::Client = capnp_rpc::new_client(provider_impl);

        BC.lock()?.commit();

        // Deploy token contract
        let file_path = "./run/cli/compiled-contracts/token.wasm";
        let mut file = match File::open(file_path) {
            Ok(file) => file,
            Err(err) => panic!(err.to_string()),
        };
        let mut code = Vec::new();
        if let Err(err) = file.read_to_end(&mut code) {
            panic!(err.to_string());
        }

        info!("=== deploy token contract");
        let params1: Vec<u8> = vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];

        let tx1 = Transaction::make_create(
            BC.lock()?.address_from_alias("alice"),
            U256::zero(),
            U256::from(1000000),
            U256::zero(),
            code,
            params1,
            H256::zero(),
        );

        let mut request = executor.execute_request();
        {
            let mut builder = request.get().init_transaction();
            build_tx(&mut builder, &tx1);
            request.get().set_provider(provider.clone());
        }
        let ret1: durian::execute::ResultData = request.send().promise.await?.get()?.into();
        //info!("ret1: {:?}", ret1);

        BC.lock()?.commit();

        BC.lock()?.inc_nonce("alice");
        BC.lock()?.commit();
        let contract = ret1.contract;
        BC.lock()?.add_transactions(tx1, ret1);

        // transfer to bob: 0xa9059cbb
        info!("=== transfer to bob");
        let mut params2 = vec![0xa9, 0x05, 0x9c, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        params2.append(&mut BC.lock()?.address_from_alias("bob").as_bytes_mut().to_vec());
        params2.append(&mut vec![
            0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ]);

        let tx2 = Transaction::make_call(
            BC.lock()?.address_from_alias("alice"),
            contract,
            U256::zero(),
            U256::from(1000000),
            U256::zero(),
            params2,
        );

        let mut request = executor.execute_request();
        {
            let mut builder = request.get().init_transaction();
            build_tx(&mut builder, &tx2);
            request.get().set_provider(provider.clone());
        }
        let ret2: durian::execute::ResultData = request.send().promise.await?.get()?.into();
        info!("ret2: {:?}", ret2);

        BC.lock()?.inc_nonce("alice");
        BC.lock()?.commit();
        BC.lock()?.add_transactions(tx2, ret2);

        // total_supply: 0x18160ddd
        info!("=== total_supply");
        let params3 = vec![0x18, 0x16, 0x0d, 0xdd];
        let tx3 = Transaction::make_call(
            BC.lock()?.address_from_alias("alice"),
            contract,
            U256::zero(),
            U256::from(1000000),
            U256::zero(),
            params3,
        );
        let mut request = executor.execute_request();
        {
            let mut builder = request.get().init_transaction();
            build_tx(&mut builder, &tx3);
            request.get().set_provider(provider.clone());
        }
        let ret3: durian::execute::ResultData = request.send().promise.await?.get()?.into();
        info!("ret3: {:?}", ret3);

        BC.lock()?.inc_nonce("alice");
        BC.lock()?.commit();
        BC.lock()?.add_transactions(tx3, ret3);

        // balance_of: 0x70a08231
        info!("=== balance_of bob");
        let mut params4 = vec![0x70, 0xa0, 0x82, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        params4.append(&mut BC.lock()?.address_from_alias("bob").as_bytes_mut().to_vec());

        let tx4 = Transaction::make_call(
            BC.lock()?.address_from_alias("bob"),
            contract,
            U256::zero(),
            U256::from(1000000),
            U256::zero(),
            params4,
        );
        let mut request = executor.execute_request();
        {
            let mut builder = request.get().init_transaction();
            build_tx(&mut builder, &tx4);
            request.get().set_provider(provider.clone());
        }
        let ret4: durian::execute::ResultData = request.send().promise.await?.get()?.into();
        info!("ret4: {:?}", ret4);

        BC.lock()?.inc_nonce("bob");
        BC.lock()?.commit();
        BC.lock()?.add_transactions(tx4, ret4);

        Ok(())
    });
}

impl<'a> From<durian_capnp::executor::execute_results::Reader<'a>> for durian::execute::ResultData {
    fn from(reader: durian_capnp::executor::execute_results::Reader<'a>) -> Self {
        let gas_left =
            U256::from_little_endian(reader.get_result_data().unwrap().get_gas_left().unwrap());
        let data = reader.get_result_data().unwrap().get_data().unwrap();
        let contract =
            Address::from_slice(reader.get_result_data().unwrap().get_contract().unwrap());
        let logs = vec![];

        durian::execute::ResultData {
            gas_left: gas_left,
            data: data.to_vec(),
            contract: contract,
            logs: logs,
        }
    }
}

fn build_tx(builder: &mut durian_capnp::transaction::Builder, tx: &Transaction) {
    let mut tmp = Vec::new();
    tmp.resize(32, 0);

    builder.set_sender(tx.sender.as_bytes());

    tx.value.to_little_endian(&mut tmp);
    builder.set_value(&tmp);

    tx.gas.to_little_endian(&mut tmp);
    builder.set_gas(&tmp);

    tx.gas_price.to_little_endian(&mut tmp);
    builder.set_gas_price(&tmp);

    builder.set_args(&tx.args);

    let action_builder = builder.reborrow().init_action();
    match &tx.action {
        Action::Create(code, salt) => {
            let mut create_builder = action_builder.init_create();
            create_builder.set_code(&code);
            create_builder.set_salt(salt.as_bytes());
        }
        Action::Call(address) => {
            let mut call_builder = action_builder.init_call();
            call_builder.set_address(address.as_bytes());
        }
    }
}
