@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_64 --cfg hisparsebitset_test_bitsetranges
cargo test
endlocal