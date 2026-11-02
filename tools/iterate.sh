#!/bin/sh
# Rebuild, run the diagnostic shots, print the report.
set -e
cd "$(dirname "$0")/.."
cargo build -q --release -p coffee-engine -p coffee-emu
./target/release/coffee-emu run tools/shot_probe.scn --trace out/probe.csv >/dev/null || true
python3 tools/shot_report.py out/probe.csv
