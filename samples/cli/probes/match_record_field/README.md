# match_record_field

Match nested inside a record literal field, and match composed inside an operand chain — `(match ...) | (8u64 | 16u64)` evaluates once as one operand.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
