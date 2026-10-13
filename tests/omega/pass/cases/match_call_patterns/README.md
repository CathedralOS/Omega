# match_call_patterns

Call-result patterns in `match`: `identity(first)` in arm position compares the subject against that call's result — ordered first-match wins, `_` catches the rest. Float subjects included: `-0.0f32` matches the `0.0f32` pattern through `identity`.

## Learnings

- Arm patterns accept arbitrary call expressions, evaluated as equality patterns against the subject.
- `use omega::language::core::float_operations` admits `f32`/`f64` literals and float machines (28 source units).
- `match` is an expression — binds directly into `let`.

Compile-checked only: run a check target, not native.
