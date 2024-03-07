@echo off
setlocal
set RUSTDOCFLAGS=--cfg docsrs
cargo +nightly doc --lib --all-features --no-deps %1
endlocal