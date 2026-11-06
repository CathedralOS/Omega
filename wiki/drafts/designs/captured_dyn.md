# Captured dyn: superseded

The 2026-11-06 captured-`dyn` design is superseded by the owner's later decision
to retire all `dyn` forms in favor of runtime machine values and `some` data
binders. Its capture map, Fused-only global roster and derived-sum lowering are
not implementation requirements.

Use [machines as values and some packages](machine_values_some.md) for the
replacement scope and acceptance. Static traits and static machine binders remain.
Effects, borrowing, contract evidence and replacement custody still require
full implementation; this supersession does not mark that work complete.
