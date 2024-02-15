@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_128 --cfg hisparsebitset_test_bitset
cargo test
endlocal