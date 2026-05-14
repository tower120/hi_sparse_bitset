@echo off
setlocal
set RUSTFLAGS=-C target-feature=+popcnt,+bmi1,+bmi2
cargo bench --bench %1 --all-features
endlocal
