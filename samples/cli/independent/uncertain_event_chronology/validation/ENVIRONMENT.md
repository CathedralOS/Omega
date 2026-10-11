Executor: Linux x86_64, UID 1000, 5 available CPUs, 18007 MiB RAM, no swap.
Compiler source base: 7192503a18f205b359c279dd79575835c2b3a7bb.
Pinned Rust: nightly-2026-09-04.
Build: source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/Omega/.codex/worktrees/new068/target CARGO_BUILD_JOBS=4 mbx build -p omega
Build exit 0, dev unoptimized, 1m33s. One Rust build at a time.
Binary: /workspace/Omega/.codex/worktrees/new068/target/debug/omega
SHA256: 212e130cd0fd0adf93d737556b5165f904f40c56ea210ea0885324b888052c39
Target: linux_x86_64; matching OMEGA_LIBRARY_ROOT explicitly selected.
Default sandbox root UID: 65534; compiler --check rejects cache ancestry custody.
The per-command require_escalated real-ownership check was approved.
Custody guards and filesystem ownership were not edited.

Optimized compiler: same pinned Rust and source base, separate target-opt.
CARGO_TARGET_DIR=/workspace/Omega/.codex/worktrees/new068/target-opt CARGO_BUILD_JOBS=3 CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega
Build exit 0, dev opt-level=1, 7m21s.
Binary: target-opt/debug/omega
SHA256: f7a2fa493d90f6f0ead59a1dee63d279b8b1fb5dd90012fbfb27e5b22a7cd09f
Unoptimized preparation runs produced no verdict and were terminated by the
executor after 20m18s and 14m23s respectively; logs preserved, not treated as
compiler rejection or acceptance. Final source has explicit helper preconditions.
