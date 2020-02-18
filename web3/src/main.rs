
use parity_rpc::{
	Origin, Metadata, NetworkSettings, informant, PubSubSession, FutureResult, FutureResponse, FutureOutput
};


pub fn main() {

    let dependencies = rpc::Dependencies {
		apis: deps_for_rpc_apis.clone(),
		executor: runtime.executor(),
		stats: rpc_stats.clone(),
	};
    let http_server = rpc::new_http("HTTP JSON-RPC", "jsonrpc", cmd.http_conf.clone(), &dependencies);

}
