# cargo-grumpy
Cargo utility to create and modify projects according to the Grumpy Way.

This wraps standard cargo calls to create new Rust projects, and adds a standard executable harness and the required dependencies to the Cargo.toml file. This is all very custom to how I do things, but you may find it useful!

## Usage
Build the project and copy the resulting binary to your ~/.cargo/bin folder (or somewhere on your path). Call using standard cargo syntax:

```commandline
cargo grumpy --help
```