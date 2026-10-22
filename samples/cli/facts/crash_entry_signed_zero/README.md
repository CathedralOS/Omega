# Crash entry signed zero

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

Fact: an entry `f32` equal to `-0.0` discharges `maybe_nonzero`'s `value != 0.0` Trap crash guard without a caller crash ceiling. The guard reads recorded IEEE equality. `-0.0` compares equal to `0.0`, so the inequality is false and the route drops. A bit-identical compare is the wrong record. NaN stays unordered and is not this sentence.

The twin proves the entry equal to `1.0`. `1.0 != 0.0` stays true, so the uncovered Trap route remains.

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
