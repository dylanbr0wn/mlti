#!/bin/bash

# print info
rustc -Vv
cargo -V

# build windows
cargo build --release --target=x86_64-pc-windows-gnu
mkdir -p builds/mlti-win64

cp target/x86_64-pc-windows-gnu/release/mlti.exe builds/mlti-win64
tar -C builds -czvf mlti-win64.tar.gz mlti-win64

# Build mac
cargo build --release --target=aarch64-apple-darwin
mkdir -p builds/mlti-macos

cp target/aarch64-apple-darwin/release/mlti.exe builds/mlti-macos
tar -C builds -czvf mlti-macos.tar.gz mlti-macos


# Build linux
cargo build --release --target=aarch64-unknown-linux-gnu
mkdir -p builds/mlti-linux

cp target/aarch64-unknown-linux-gnu/release/mlti.exe builds/mlti-linux
tar -C builds -czvf mlti-linux.tar.gz mlti-linux
