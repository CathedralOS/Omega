# Projected write-only state receiver

Run `cargo run -p omega -- run --both tests/omega/pass/borrow/write_only_projected_state_receiver_exit/main.omg`
after ordinary package review.

A mutable record lends its right child as write-only to a successor state.
That state forwards the captured child to another state and an ordinary named
receiver call. Exit 0 confirms the signed write reached original caller storage
without changing either padding field or the sibling. Trap reports a mismatch.

Use `python3 tools/corpus_gate.py --both --filter write_only_projected_state_receiver_exit`
to check both execution engines without replacing the authored source.
