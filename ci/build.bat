:: Print version information
rustc -Vv || exit /b 1
cargo -V || exit /b 1

cargo build --release || exit /b 1
