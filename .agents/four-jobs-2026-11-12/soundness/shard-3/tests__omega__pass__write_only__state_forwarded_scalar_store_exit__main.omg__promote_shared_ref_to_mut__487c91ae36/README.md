# Write-only scalar forwarding

Run `cargo run -p omega -- run --both tests/omega/pass/write_only/state_forwarded_scalar_store_exit/main.omg` from the repository root.

The caller lends its integer through a state edge and an ordinary helper call.
Exit **70** means the original caller field contains 42 and its neighbor still
contains 19; exit 71 reports incorrect writeback. The loan grants no reads.

The corpus execution gate is `python3 tools/corpus_gate.py --both --filter write_only/state_forwarded_scalar_store_exit`.
It independently executes native code and the checked interpreter. The public
command also requires ordinary package review; do not bypass that admission.
