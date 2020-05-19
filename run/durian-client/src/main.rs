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
use durian::Bytes;
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

        let params1: Vec<u8> = vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];

        let mut request = executor.execute_request();
        {
            let mut builder = request.get().init_transaction();
            build_create_tx(
                &mut builder,
                BC.lock()?.address_from_alias("alice"),
                U256::zero(),
                U256::from(1000000),
                U256::zero(),
                code,
                params1,
                H256::zero(),
            );
            request.get().set_provider(provider);
        }
        request.send().promise.await?;

        debug!("contract deployed.");
        BC.lock()?.commit();

        Ok(())
    });
}

fn build_create_tx(
    builder: &mut durian_capnp::transaction::Builder,
    sender: Address,
    value: U256,
    gas: U256,
    gas_price: U256,
    code: Bytes,
    args: Bytes,
    salt: H256,
) {
    let mut tmp = Vec::new();
    tmp.resize(32, 0);

    builder.set_sender(sender.as_bytes());

    value.to_little_endian(&mut tmp);
    builder.set_value(&tmp);

    gas.to_little_endian(&mut tmp);
    builder.set_gas(&tmp);

    gas_price.to_little_endian(&mut tmp);
    builder.set_gas_price(&tmp);

    builder.set_args(&args);

    let action_builder = builder.reborrow().init_action();
    let mut action_create_builder = action_builder.init_create();
    action_create_builder.set_code(&code);
    action_create_builder.set_salt(salt.as_bytes());
}
