#!/usr/bin/env sh
# beta_symbolic's DATA-DEPENDENT loop summarization, pinned to the concrete interpreter. beta_symbolic derives
# an all-inputs closed form for a linear counter loop (`total += c` while `i < n`) without unrolling — a
# SYMBOLIC trip count. This gate checks that closed form against beta_interp.py run at every point of an input
# grid, and that loops outside the recognized linear class are conservatively refused. Source-side half of
# instruction-level refinement for data-dependent loops (the bytecode/alpha half is a later slice).
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "symbolic loops: skipped (python3 absent)"; exit 0; }
python3 symbolic_loop_check.py
