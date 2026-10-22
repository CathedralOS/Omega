# Crash entry NaN equality

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: an entry `f32` that is NaN makes `value == value` false, so `maybe_ordered`'s Trap guard drops without a caller crash ceiling. The guard reads recorded IEEE equality. NaN is unordered, so the comparison is false and the route drops.

The twin uses `1.0`, where `value == value` stays true, and still rejects the uncovered Trap route.

A const initializer cannot divide by the sealed float operator, so `const_name` names the quiet-NaN bits `2143289344` and reads them as `f32`. That spelling is the same unordered value. The twin names the bits of `1.0`.

| Place | Positive | Twin |
| --- | --- | --- |
| `local` | `local/main.omg` | `local/one/main.omg` |
| `field` | `field/main.omg` | `field/one/main.omg` |
| `parameter` | `parameter/main.omg` | `parameter/one/main.omg` |
| `call_result` | `call_result/main.omg` | `call_result/one/main.omg` |
| `match_join` | `match_join/main.omg` | `match_join/one/main.omg` |
| `const_name` | `const_name/main.omg` | `const_name/one/main.omg` |
| `other_state` | `other_state/main.omg` | `other_state/one/main.omg` |
| `other_machine` | `other_machine/main.omg` | `other_machine/one/main.omg` |
| `target_body` | `target_body/main.omg` | `target_body/one/main.omg` |

Pinned under `tests/omega/pass/facts/crash_entry_nan_equality_<place>_exit` and `tests/omega/fail/facts/crash_entry_nan_equality_<place>_one_rejected`.

The local place also pins its writable carrier, because the equality route reads the shared entry-provenance local carrier rule: `pass/facts/crash_entry_nan_equality_local_writable_exit` records the unordered entry in a writable local that is never written, and `fail/facts/crash_entry_nan_equality_local_written_rejected` overwrites it with `1.0`, which makes `value == value` true, so the route stays uncovered (`at statement 2 call 0`).
