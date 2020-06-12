# Durian

Durian is a stateless VM based on [parity-wasm](https://github.com/paritytech/parity-wasm) for blockchains.


## Dosumentation
You can check [here](https://docs.google.com/document/d/11BOKAwnepo-LNQtJu7UphQ75VNIY83HjJIY2GtZTtEI/edit?usp=sharing) to see how durian works.

## Building

Durian requires **latest stable Rust version** to build. You can install Rust through [rustup](https://www.rustup.rs/).

In order to use Durian as a webservice you also need to install [Cap'n Proto](https://capnproto.org/install.html).

To build the durian from the source code, you can follow these commands:

```
$ git clone https://github.com/b00f/durian
$ cd durian

# build in release mode
$ cargo build --release
```


## Examples

Some examples are provided inside the run folder.

## License

This package is licensed under the MIT License.