@echo off
setlocal
set RUSTDOCFLAGS=--cfg docsrs
cargo +nightly doc --lib --features impl --no-deps %1
endlocal