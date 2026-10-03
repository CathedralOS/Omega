# Scalar callers retain local storage across calls and states

`compute` lends its mutable local to `increment`, observes the updated value,
and forwards that scalar through another state before returning. Native exit 0
requires exactly one increment from 41 to 42; a missing or repeated effect traps.

From the repository root:

```sh
cargo run -p omega -- update --project tests/omega/pass/calls/scalar_unit_call_state_forward_exit --target macos_arm64
cargo run -p omega -- run --both tests/omega/pass/calls/scalar_unit_call_state_forward_exit/main.omg
python3 tools/corpus_gate.py --native --filter scalar_unit_call_state_forward_exit
```

Use `mbx` instead of Cargo when available. `--both` on the corpus gate also
executes the checked interpreter; a missing combined golden does not establish
a clean comparison, so inspect the two execution oracles separately.

On macOS ARM64, base `38c9e7eb39b` interprets with exit 0 but native production
rejects `composed state establishes no primitive referent to borrow`. The repair
rejoins the declaration and exact borrow occurrence in the caller's state,
independent of its body family. Both engines then return 0, including through
the public command after ordinary `omega update` package preparation. This
package-free fixture has no capability review decisions to approve. Use the
matching host target for preparation on another supported platform.

The shared closure assembler also serves pure scalar roots. Scalar contracts,
qualification identities, source mappings and loan checks remain independently
replayed; no source-shape fallback selects a different assembler.
