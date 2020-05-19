use crate::durian_capnp::provider;
use blockchain::blockchain::Blockchain;
use capnp::capability::Promise;
use durian::address::Address;
use durian::provider::Provider;
use primitive_types::H256;
use std::sync::Arc;
use std::sync::Mutex;

pub struct ProviderImpl<'a> {
    bc: &'a Arc<Mutex<Blockchain>>,
}

impl<'a> ProviderImpl<'a> {
    pub fn new(bc: &'a Arc<Mutex<Blockchain>>) -> Self {
        ProviderImpl { bc: bc }
    }
}

impl<'a> provider::Server for ProviderImpl<'a> {
    fn exist(
        &mut self,
        params: provider::ExistParams,
        mut results: provider::ExistResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        debug!("server called `exist` method.");

        let address = Address::from_slice(pry!(pry!(params.get()).get_address()));
        let exist = self.bc.lock().unwrap().exist(&address);
        results.get().set_exist(exist);
        Promise::ok(())
    }

    fn account(
        &mut self,
        params: provider::AccountParams,
        mut results: provider::AccountResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        debug!("server called `account` method.");

        let address = Address::from_slice(pry!(pry!(params.get()).get_address()));

        match self.bc.lock().unwrap().account(&address) {
            Ok(account) => {
                let mut account_result = results.get().init_account();
                let mut tmp = Vec::new();
                tmp.resize(32, 0);
                account.nonce.to_little_endian(&mut tmp);
                account_result.set_nonce(&tmp);

                account.balance.to_little_endian(&mut tmp);
                account_result.set_balance(&tmp);

                account_result.set_code(&account.code);
                return Promise::ok(());
            }
            Err(_e) => {
                // TODO: How to return actual error
                return Promise::err(::capnp::Error::failed("account failed".to_string()));
            }
        }
    }

    fn create_contract(
        &mut self,
        params: provider::CreateContractParams,
        _: provider::CreateContractResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        debug!("server called `create_contract` method.");

        let address = Address::from_slice(pry!(pry!(params.get()).get_address()));
        let code = pry!(pry!(params.get()).get_code()).to_vec();


        match self.bc.lock().unwrap().create_contract(&address, &code) {
            Ok(()) => {
                return Promise::ok(());
            }
            Err(_e) => {
                // TODO: How to return actual error
                return Promise::err(::capnp::Error::failed("account failed".to_string()));
            }
        }
    }

    fn storage_at(
        &mut self,
        params: provider::StorageAtParams,
        mut results: provider::StorageAtResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        debug!("server called `storage_at` method");

        let address = Address::from_slice(pry!(pry!(params.get()).get_address()));
        let key = H256::from_slice(pry!(pry!(params.get()).get_key()));

        match self.bc.lock().unwrap().storage_at(&address, &key) {
            Ok(storage) => {
                results.get().set_storage(storage.as_bytes());
                return Promise::ok(());
            }
            Err(_e) => {
                // TODO: How to return actual error
                return Promise::err(::capnp::Error::failed("storageAt failed".to_string()));
            }
        }
    }

    fn set_storage(
        &mut self,
        params: provider::SetStorageParams,
        _: provider::SetStorageResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        debug!("server called `set_storage` method");

        let address = Address::from_slice(pry!(pry!(params.get()).get_address()));
        let key = H256::from_slice(pry!(pry!(params.get()).get_key()));
        let value = H256::from_slice(pry!(pry!(params.get()).get_value()));

        match self.bc.lock().unwrap().set_storage(&address, &key, &value) {
            Ok(()) => {
                return Promise::ok(());
            }
            Err(_e) => {
                // TODO: How to return actual error
                return Promise::err(::capnp::Error::failed("storageAt failed".to_string()));
            }
        }
    }
}
