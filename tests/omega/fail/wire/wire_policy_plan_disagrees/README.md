# Authored wire plans must match their schemas

The policy intentionally frames both scalar fields as `LengthPrefixed` instead
of `Varint`. Its evaluation must succeed far enough to report both authored and
expected plans; an unrelated termination, ownership, or precondition refusal
does not satisfy this control.

At historical library revision `c1303f89d4`, `SchemaField.size` carried
`u64 [0..=4096]`, `SchemaField.align` carried `u64 [1..=16]`, and
`Schema.field_count` carried `u64 [0..=32]`. Their canonical replacements are
the containing data predicates. `evaluate.fuel`, `fill.fuel`, and
`put_wrong.fuel` each carried `u64 [1..=96]`; each now has that exact arrival
premise. Unsigned carriers retain the zero floors. No nominal interval domain
replaces a proposition.

`FieldPlan` explicitly declares copyability because returning the plan array
reads borrowed scratch storage rather than consuming it. The deliberately wrong
framing, field identities, conversion call, mutable cursor, and fuel algorithm
remain unchanged.

The conversion call in the assignment previously prevented ranked-prefix
preservation despite its complete readonly frame. The general operand-frame
repair permits that call without granting its result any new premise.

```sh
python3 tools/corpus_gate.py --filter wire/wire_policy_plan_disagrees,wire/runtime_wire_policy_authored_plan_exit
python3 tools/corpus_gate.py --native --filter wire/runtime_wire_policy_authored_plan_exit
```
