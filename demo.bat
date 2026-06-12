@echo off
cd /d %~dp0

echo Starting Traxes Demo...

cargo run --release --bin Traxes-demo -- server --demo-mode

timeout /t 2 >nul

cargo run --release --bin Traxes-demo -- evaluate payloads/provision_db.json --demo-mode

echo Done.
pause
