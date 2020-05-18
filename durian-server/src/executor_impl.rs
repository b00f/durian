use crate::durian_capnp;
use crate::durian_capnp::executor;
use crate::provider_adaptor::ProviderAdaptor;
use capnp::capability::Promise;
use capnp::Error;
use durian::address::Address;
use primitive_types::{H256, U256};

impl<'a> From<durian_capnp::transaction::Reader<'a>>
    for Result<durian::transaction::Transaction, Error>
{
    fn from(reader: durian_capnp::transaction::Reader<'a>) -> Self {
        let sender = Address::from_slice(reader.get_sender()?);
        let value = U256::from_little_endian(reader.get_value()?);
        let gas = U256::from_little_endian(reader.get_gas()?);
        let gas_price = U256::from_little_endian(reader.get_gas_price()?);
        let args = reader.get_args()?.to_vec();
        let action = match reader.get_action().which()? {
            durian_capnp::transaction::action::Create(create) => {
                let code = create.get_code()?.to_vec();
                let salt = H256::from_slice(create.get_salt()?);
                durian::transaction::Action::Create(code, salt)
            }
            durian_capnp::transaction::action::Call(call) => {
                let address = Address::from_slice(call.get_address()?);
                durian::transaction::Action::Call(address)
            }
        };

        Ok(durian::transaction::Transaction {
            sender: sender,
            value: value,
            gas: gas,
            gas_price: gas_price,
            action: action,
            args: args,
        })
    }
}

pub struct ExecutorImpl {}

impl ExecutorImpl {
    pub fn new() -> Self {
        ExecutorImpl {}
    }
}

unsafe impl Send for durian_capnp::provider::Client {}
unsafe impl Sync for durian_capnp::provider::Client {}

impl executor::Server for ExecutorImpl {
    fn execute(
        &mut self,
        params: executor::ExecuteParams,
        mut results: executor::ExecuteResults,
    ) -> Promise<(), Error> {
        let provider_client = pry!(pry!(params.get()).get_provider());
        let transaction = pry!(pry!(pry!(params.get()).get_transaction()).into());
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            println!("{:?}:1", std::thread::current().id());
            let mut adaptor = ProviderAdaptor::new(provider_client);

            durian::execute::execute(&mut adaptor, &transaction).unwrap();

            tx.send("hooray, it executed ").unwrap();
        });
        println!("{:?}:2", std::thread::current().id());

        Promise::from_future(async move {
            loop {
                let msg = rx.try_recv();
                match msg {
                    Err(_e) => {}
                    Ok(msg) => {
                        println!("tx data: {:?}", msg);

                        break;
                    }
                };
                tokio::task::yield_now().await;
                // tokio::time::delay_for(Duration::from_millis(20 as u64)).await;
            }

            Ok(())
        })

    }
}
