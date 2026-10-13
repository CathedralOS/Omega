# case_arm_pattern_names

Arm-position record binding grammar on case payloads: `dx as step` renames the binding (uses of `step` read the subject's payload), `dy as _` waives a field (spelled, unbound), `..` opts out of exhaustiveness for remaining fields, `if` guards see renamed bindings.

## Learnings

- `match` admits only value/wildcard patterns — record/case destructure patterns belong to `transition` dispatch with `state` targets.
- State targets carry renamed bindings: `Msg::Move { dx as step, .. } if step == 30 -> hit(step)`; `hit(step: i32 in Step)` re-declares the domain on the param.
- Interval result types `-> i32 [-100..=140]` on machine+states let a caller's `moved + stopped` stay provable.

Compile-checked only: run a check target, not native.
