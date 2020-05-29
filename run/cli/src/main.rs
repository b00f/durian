extern crate blockchain;
extern crate durian;
extern crate primitive_types;
extern crate simple_logger;

#[macro_use]
extern crate log;

use blockchain::blockchain::Blockchain;
use durian::execute;
use durian::transaction::Transaction;
use primitive_types::{H256, U256};
use log::Level;
use std::fs::File;
use std::io::Read;

fn main() {
    simple_logger::init_with_level(Level::Debug).unwrap();

    let mut bc = Blockchain::new();

    let file_path = "./run/cli/compiled-contracts/token.wasm";
    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(err) => panic!(err.to_string()),
    };
    let mut code = Vec::new();
    if let Err(err) = file.read_to_end(&mut code) {
        panic!(err.to_string());
    }

    bc.commit();

    // deploy token contract
    let params1 = vec![
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF,
    ];
    let tx1 = Transaction::make_create(
        bc.address_from_alias("alice"),
        U256::zero(),
        U256::from(1000000),
        U256::zero(),
        code,
        params1,
        H256::zero(),
    );

    let ret1 = execute::execute(&mut bc, &tx1).unwrap();

    //info!("ret1: {:?}", ret1);
    bc.inc_nonce("alice");
    bc.commit();
    let contract = ret1.contract;
    bc.add_transactions(tx1, ret1);

    // transfer to bob: 0xa9059cbb
    let mut params2 = vec![0xa9, 0x05, 0x9c, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    params2.append(&mut bc.address_from_alias("bob").as_bytes_mut().to_vec());
    params2.append(&mut vec![
        0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF,
    ]);

    let tx2 = Transaction::make_call(
        bc.address_from_alias("alice"),
        contract,
        U256::zero(),
        U256::from(1000000),
        U256::zero(),
        params2,
    );

    let ret2 = execute::execute(&mut bc, &tx2).unwrap();
    info!("ret2: {:?}", ret2);
    bc.inc_nonce("alice");
    bc.commit();
    bc.add_transactions(tx2, ret2);


    // total_supply: 0x18160ddd
    let params3 = vec![0x18, 0x16, 0x0d, 0xdd];
    let tx3 = Transaction::make_call(
        bc.address_from_alias("alice"),
        contract,
        U256::zero(),
        U256::from(1000000),
        U256::zero(),
        params3,
    );
    let ret3 = execute::execute(&mut bc, &tx3).unwrap();
    info!("ret3: {:?}", ret3);
    bc.inc_nonce("alice");
    bc.commit();
    bc.add_transactions(tx3, ret3);


    // balance_of: 0x70a08231
    let mut params4 = vec![0x70, 0xa0, 0x82, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    params4.append(&mut bc.address_from_alias("bob").as_bytes_mut().to_vec());

    let tx4 = Transaction::make_call(
        bc.address_from_alias("bob"),
        contract,
        U256::zero(),
        U256::from(1000000),
        U256::zero(),
        params4,
    );
    let ret4 = execute::execute(&mut bc, &tx4).unwrap();
    info!("ret4: {:?}", ret4);
    bc.inc_nonce("bob");
    bc.commit();
    bc.add_transactions(tx4, ret4);

}
