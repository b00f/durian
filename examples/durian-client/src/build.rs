extern crate capnpc;

fn main() {
    ::capnpc::CompilerCommand::new().file("durian.capnp").run().unwrap();
}