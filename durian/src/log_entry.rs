use primitive_types::H256;
use address::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub address: Address,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
}
