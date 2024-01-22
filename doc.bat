@echo off
setlocal
set RUSTDOCFLAGS=--cfg docsrs
cargo +nightly doc --lib --all-features %1
endlocal