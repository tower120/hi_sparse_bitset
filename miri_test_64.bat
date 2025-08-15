@echo off
setlocal
set MIRIFLAGS=-Zmiri-disable-isolation 
set RUSTFLAGS=--cfg hisparsebitset_test_64
cargo +nightly miri nextest run -j6 --all-features
endlocal