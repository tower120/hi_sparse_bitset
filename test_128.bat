@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_128
cargo test
endlocal