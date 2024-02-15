@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_64 --cfg hisparsebitset_test_bitset
cargo test
endlocal