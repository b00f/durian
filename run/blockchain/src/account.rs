use primitive_types::{H256, U256};
use std::collections::HashMap;
use durian::address::Address;

#[derive(Debug, Clone)]
pub struct Account {
    pub address: Address,
    pub nonce: U256,
    pub balance: U256,
    pub code: Vec<u8>,
    pub storage: HashMap<H256, H256>,
}

impl Account {
    pub fn new(addr: Address, bal: U256, nonce: U256, code: Vec<u8>) -> Account {
        Account {
            address: addr,
            balance: bal,
            nonce: nonce,
            code: code,
            storage: HashMap::new(),
        }
    }
}
