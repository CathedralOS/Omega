# Fixed-size 32-element array indexing bound preservation

Fact: a static index bound idx < 32u64 carried through the place proves safe fixed-size array indexing arr[idx] on [u64; 32].

Owning Task: `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS`

## Requirements and Evidence

Measured on windows_x86_64 with an `opt-level 1` producer.

- **Establishment**: Fixed-size 32-element array `[u64; 32]` construction and static entry precondition `requires idx < 32` (or record constraint `where idx < 32` / named constant `LIMIT = 32`) establish proven in-bounds access before indexing `arr[idx]`. `omega run --both` reports native exit 0 and interpreter exit 0 agreeing for all nine places.
- **Identity and Lifetime**: The element values are conserved across all value transfers and transitions. Accessing element `arr[25]` yields `25u64`, verified by transition guard `elem == 25` transitioning to `done()` with exit code 0.
- **Composition**: Machine calls, pattern match joins, and state transitions preserve exact preconditions and actual values without narrowing or corruption. Each `invalid/main.omg` differs from its positive twin in only the bound carrying the fact and its explanatory comment.
- **Soundness Boundaries**: Weakened preconditions (`idx <= 32` or widened limits) are rejected at compile time with `cannot prove index ... is within length 32`, proving that out-of-bounds indexing is strictly prevented across all nine places.
- **Terminal Psi and carrier lowering / native realization**: `omega run --both` executes each place on both engines with agreeing exits (0), and `sample_native_gate` builds and runs all places.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | The array and bounded index are copied into locals before indexing. |
| field | `field/` | A record field carries the bounded index alongside the fixed-size array. |
| parameter | `parameter/` | The reading machine receives the bounded index and array directly as parameters. |
| call result | `call_result/` | The index arrives from a helper call with an ensures result < 32 contract. |
| match join | `match_join/` | Both branches of a match expression produce in-bounds indices. |
| const name | `const_name/` | A named constant bounds the index contract for the fixed-size array. |
| other state | `other_state/` | A transition to a successor state forwards the bounded index and array. |
| other machine | `other_machine/` | A forwarding machine transports the bounded index and array to the reader. |
| target body | `target_body/` | Each of four target bodies reads the array element using the bounded index. |

Each place is an application that binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` in its `build.omg`, declares `Expected exit: 0`, carries a measured `expected_native_exit.txt`, and passes `omega run --both`.

Each place has an `invalid/` twin that weakens the index bound (to `idx <= 32` or `LIMIT = 33`), so the array index access is no longer proved in bounds. Every twin rejects at compile time with `cannot prove index`.
