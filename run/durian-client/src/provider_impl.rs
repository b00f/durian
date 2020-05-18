use crate::durian_capnp::provider;
use capnp::capability::Promise;
use blockchain::blockchain::Blockchain;
use capnp::primitive_list;
use capnp::Error;
use std::sync::Arc;
use std::sync::Mutex;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};

pub struct ProviderImpl<'a>{
    bc: &'a  Arc<Mutex<Blockchain>>,
}

impl<'a> ProviderImpl<'a> {
    pub fn new(bc: &'a Arc<Mutex<Blockchain>>) -> Self {
        ProviderImpl {
            bc: bc
        }
    }
}

impl<'a> provider::Server for ProviderImpl<'a> {
    fn get_storage(
        &mut self,
        params: provider::GetStorageParams,
        mut results: provider::GetStorageResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        println!("get_storage hit");

        self.bc.lock().unwrap().commit();
        let val = vec![1 as u8,2,3];
        results.get().set_storage(&val[..]);
        Promise::ok(())
    }
}
