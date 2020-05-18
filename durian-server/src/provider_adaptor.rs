use crate::durian_capnp;
use capnp::capability::Promise;
use durian::address::Address;
use durian::provider::{Provider, StateAccount};
use futures::channel::oneshot;
use futures::task::LocalSpawn;
use primitive_types::{H256, U256};
use std::sync::mpsc;
use tokio::runtime::Runtime;

pub struct ProviderAdaptor {
    client:  durian_capnp::provider::Client,
}

impl ProviderAdaptor {
    pub fn new(client:  durian_capnp::provider::Client) -> Self {
        ProviderAdaptor { client }
    }
}

impl Provider for ProviderAdaptor {
    fn exist(&self, address: &Address) -> bool {
        self.account(address).is_ok()
    }

    fn account(&self, address: &Address) -> Result<StateAccount, durian::error::Error> {
        Err(durian::error::Error::NotSupported)
    }

    fn create_contract(
        &mut self,
        address: &Address,
        code: &Vec<u8>,
    ) -> Result<(), durian::error::Error> {
        Ok(())
    }

    fn update_account(
        &mut self,
        address: &Address,
        bal: &U256,
        nonce: &U256,
    ) -> Result<(), durian::error::Error> {
        Ok(())
    }

    fn storage_at(&self, address: &Address, key: &H256) -> Result<H256, durian::error::Error> {
        println!("{:?}:e1", std::thread::current().id());
        let mut exec = futures::executor::LocalPool::new();

        let handle = async move {
            let storage_req = self.client.get_storage_request();

            println!("{:?}:e3", std::thread::current().id());
            let p = storage_req.send().promise;
            println!("{:?}:e4", std::thread::current().id());

            let storage_results = p.await.unwrap();
            let storage = storage_results.get().unwrap().get_storage().unwrap();
            println!("storage = {:?}", storage);
        };
        let mut rt = Runtime::new().unwrap();
        let local = tokio::task::LocalSet::new();
        local.block_on(&mut rt, handle);


        println!("e:3");

        Err(durian::error::Error::NotSupported)
    }

    fn set_storage(
        &mut self,
        address: &Address,
        key: &H256,
        value: &H256,
    ) -> Result<(), durian::error::Error> {
        Ok(())
    }

    fn block_hash(&self, num: u64) -> Result<H256, durian::error::Error> {
        Ok(H256::zero())
    }

    fn timestamp(&self) -> u64 {
        0
    }

    fn block_number(&self) -> u64 {
        0
    }

    fn block_author(&self) -> Result<Address, durian::error::Error> {
        Err(durian::error::Error::NotSupported)
    }

    fn difficulty(&self) -> Result<U256, durian::error::Error> {
        Err(durian::error::Error::NotSupported)
    }

    fn gas_limit(&self) -> Result<U256, durian::error::Error> {
        Ok(U256::zero())
    }
}
