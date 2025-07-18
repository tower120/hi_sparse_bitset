@echo off
setlocal
set RUSTFLAGS=--cfg hisparsebitset_test_256
cargo test --all-features
endlocal