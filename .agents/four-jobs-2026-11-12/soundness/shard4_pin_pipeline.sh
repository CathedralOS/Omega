#!/usr/bin/env bash
set -euo pipefail
cd /agent/repos/omega
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
export OMEGA_CORPUS_ROOT="$PWD"
export SHARD4_CHECK_TIMEOUT=300
export SHARD4_WORKERS=6
PIN=0835b3aaf803ed97f905be42d94727ff58459263
export SHARD4_PIN="$PIN"
LOG=/opt/cursor/artifacts/shard4_pin_pipeline.log
exec > >(tee -a "$LOG") 2>&1

echo "=== PIN build ($PIN) ==="
python3 .agents/four-jobs-2026-11-12/soundness/shard4_pin_measure.py build
export SHARD4_OMEGA=/tmp/omega-pin-shard4-target/debug/omega
export SHARD4_ROUTE=/tmp/omega-pin-shard4-target/debug/examples/route_one_comparison

echo "=== baselines (all shard-4 originals) ==="
python3 .agents/four-jobs-2026-11-12/soundness/shard4_pin_measure.py baselines

echo "=== mutate newly accepted originals (if any) ==="
python3 .agents/four-jobs-2026-11-12/soundness/shard4_gap_fill.py || true

echo "=== route backfill (--every-machine) ==="
python3 .agents/four-jobs-2026-11-12/soundness/shard4_pin_measure.py routes

echo "=== reclassify + bucket fix + finalize ==="
python3 .agents/four-jobs-2026-11-12/soundness/shard4_reclassify.py
python3 .agents/four-jobs-2026-11-12/soundness/shard4_bucket_fix.py
python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py --md-only

echo "=== DONE shard4 pin pipeline ==="
