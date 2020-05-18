#[macro_use]
extern crate capnp_rpc;

pub mod durian_capnp {
    include!(concat!(env!("OUT_DIR"), "/durian_capnp.rs"));
}

mod executor_impl;
mod provider_adaptor;

use durian_capnp::executor;
use executor_impl::ExecutorImpl;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::rc::Rc;

use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};

use capnp::capability::Promise;

use futures::task::LocalSpawn;
use futures::{AsyncReadExt, FutureExt, StreamExt};

pub fn main() {
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
    let spawner1 = exec.spawner();

    let result: Result<(), Box<dyn std::error::Error>> = exec.run_until(async move {
        let executor_impl = ExecutorImpl::new();
        let listener = async_std::net::TcpListener::bind(&addr).await?;
        let executor: executor::Client = capnp_rpc::new_client(executor_impl);

        loop {
            let (stream, _) = listener.accept().await?;
            stream.set_nodelay(true)?;
            let (reader, writer) = stream.split();
            let network = twoparty::VatNetwork::new(
                reader,
                writer,
                rpc_twoparty_capnp::Side::Server,
                Default::default(),
            );

            let rpc_system = RpcSystem::new(Box::new(network), Some(executor.clone().client));
            spawner1.spawn_local_obj(Box::pin(rpc_system.map(|_| ())).into())?;
        }
    });

    result.expect("main");
}
