# Ranked borrowed-field update

The complete program starts `main.engine.cursor.remaining` at five, lends the
cursor to a ranked callee, and observes zero through the enclosing record after
return. Native and Terminal execution must both exit 70. An unchanged value
would exit 71.

```sh
omega run --both tests/omega/pass/termination/ranked_callee_projected_receiver_compile/main.omg
python3 tools/corpus_gate.py --both --filter termination/ranked_callee_projected_receiver_compile,termination/rank_endpoint_borrows_recovery
python3 tools/corpus_gate.py --filter termination/nested_field_without_predicate_cannot_bound_result,termination/rank_range_mutable_endpoint_overflow_after_write,termination/rank_range_subject_intervening_write
```

The public command requires ordinary local package/entry review; no broad
acceptance policy is shipped with the fixture.

## Historical scalar declarations

The six suffix positions at `8f93c866a3` retain their original `u64` carriers
and `[0..=8]` relationship:

| Declaration | Canonical obligation |
| --- | --- |
| `Cursor.remaining` | data `where remaining <= 8` |
| `Cursor::step.remaining` | machine `requires remaining <= 8` |
| `Cursor::step` result | machine `ensures result <= 8` |
| `Cursor::step.done` result | terminal zero owes the owning `Cursor::step` guarantee |
| `Engine::run` result | machine `ensures result <= 8` |
| `Engine::run.start` local | plain carrier; reading the declared cursor field supplies its bound |

The zero lower bounds follow from the unsigned carrier. Named states admit
arrival `requires`, not a second exit contract; their terminal returns owe the
owning machine's postconditions. Result propositions do not grant a nominal
domain or turn the captured start into a constant.

The bounds checker uses the shared declaration path for the nested return,
not a one-member recognizer. A mechanical probe adds a containing record,
passes the callee's result into another requirement, and reads the field through
a shared helper with `+ 0`; it retains exit 70 in both engines. The original
arrangement remains the maintained reproduction. The absent-predicate control
has the same field names but cannot promise a bounded result.
