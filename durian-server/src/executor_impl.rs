use crate::durian_capnp;
use crate::durian_capnp::executor;
use crate::provider_adaptor::ProviderAdaptor;
use capnp::capability::Promise;
use capnp::Error;
use durian::address::Address;
use primitive_types::{H256, U256};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;

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
//unsafe impl Sync for durian_capnp::provider::Client {}

impl executor::Server for ExecutorImpl {
    fn execute(
        &mut self,
        params: executor::ExecuteParams,
        mut results: executor::ExecuteResults,
    ) -> Promise<(), Error> {
        let provider_client = pry!(pry!(params.get()).get_provider());
        let transaction = pry!(pry!(pry!(params.get()).get_transaction()).into());
        let (tx, mut rx) = oneshot::channel();

        tokio::task::spawn(async move {
            debug!("provider: {:?}", std::thread::current().id());
            let mut adaptor = ProviderAdaptor::new(provider_client);

            let result = durian::execute::execute(&mut adaptor, &transaction).unwrap();

            tx.send(result).unwrap();
        });
        debug!("executor: {:?}", std::thread::current().id());

        Promise::from_future(async move {
            loop {
                let msg = rx.try_recv();
                match msg {
                    Err(TryRecvError::Empty) => {}
                    Err(e) => {
                        return Err(Error::failed(format!("{}", e)));
                    }
                    Ok(result_data) => {
                        tokio::time::delay_for(std::time::Duration::from_millis(10 as u64)).await;

                        let mut tmp = Vec::new();
                        tmp.resize(32, 0);

                        let mut builder = results.get().get_result_data().unwrap();

                        result_data.gas_left.to_little_endian(&mut tmp);
                        builder.set_gas_left(&tmp);
                        builder.set_data(&result_data.data);
                        builder.set_contract(&result_data.contract.as_bytes());

                        // TODO: Implement it later
                        //builder.set_logs();

                        break;
                    }
                };

                //print!(".");
                tokio::task::yield_now().await;
                //tokio::time::delay_for(std::time::Duration::from_millis(10 as u64)).await;
            }

            Ok(())
        })
    }
}
