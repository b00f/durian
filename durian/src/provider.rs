use error::Error;
use primitive_types::{H256, U256};
use address::Address;

pub struct StateAccount {
    pub nonce: U256,
    pub balance: U256,
    pub code: Vec<u8>,
}

pub trait Provider {
    fn exist(&self, address: &Address) -> bool;
    fn account(&self, address: &Address) -> Result<StateAccount, Error>;
    fn update_account(&mut self, address: &Address, bal: &U256, nonce: &U256) -> Result<(), Error>;
    fn create_contract(&mut self, address: &Address, code: &Vec<u8>) -> Result<(), Error>;
    fn storage_at(&self, address: &Address, key: &H256) -> Result<H256, Error>;
    fn set_storage(&mut self, address: &Address, key: &H256, value: &H256) -> Result<(), Error>;
    fn timestamp(&self) -> u64;
    fn block_number(&self) -> u64;
    fn block_hash(&self, block_no: u64) -> Result<H256, Error>;
    fn block_author(&self) -> Result<Address, Error>;
    fn difficulty(&self) -> Result<U256, Error>;
    fn gas_limit(&self) -> Result<U256, Error>;
}
