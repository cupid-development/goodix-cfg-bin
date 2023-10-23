# goodix-cfg-bin

Dumper for values inside a goodix gtx8 cfg bin file.

## Compilation

Install a nightly rust compiler via [rustup](https://rustup.rs/). Then run

`cargo build --release --target=x86_64-unknown-linux-gnu` (or `aarch64-apple-darwin`, etc.)

## Running

`./target/x86_64-unknown-linux-gnu/release/goodix-cfg-bin /path/to/goodix_cfg_group.bin`

It will dump the contents of the file as JSON. This should be much easier to use than messing with the kernel module.

## Internals

This is pretty much a straight-forward port of the kernel module code. It uses unsafe code and should probably be simplified in the future. Some simplifications were made based on assumptions such as `aarch64` being little-endian.
