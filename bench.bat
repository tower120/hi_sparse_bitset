@echo off
setlocal
set RUSTFLAGS=-C target-feature=+popcnt,+bmi1
cargo bench --bench %1 --all-features
endlocal