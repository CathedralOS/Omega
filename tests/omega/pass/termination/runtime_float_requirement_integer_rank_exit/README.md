# Float requirements with integer termination ranks

Run `omega update --project . --target macos_arm64 --dangerously-accept-all`,
then `omega run --both main.omg`. Both executions must exit 0.

The program carries an exact IEEE precondition beside a bounded integer rank
through direct recursion, mutual calls, f32 requirements in separate clauses,
and named states borrowing a record field. It also enters through a saved
Boolean field observation and a completed call result's public float guarantee.
These observations do not supply integer bounds: each arrival independently
proves its integer range and decrease.

The companion failures retain nondecreasing ranks, a disjunctive premise that
cannot establish the integer range, NaN, changed actuals, overwritten fields,
a saved guard after a write, prior argument captures, and a false integer
conjunct beside a valid float result guarantee.
