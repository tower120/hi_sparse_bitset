@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_64
cargo test
endlocal