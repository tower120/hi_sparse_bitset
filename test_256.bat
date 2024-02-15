@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_256 --cfg hisparsebitset_test_bitset
cargo test
endlocal