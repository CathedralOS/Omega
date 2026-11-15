# Canonical float arithmetic

This witness covers ordinary `f32` and `f64` addition, subtraction,
multiplication and division for **LOWERED-FORM-CHECKER** in
[TASKS.md](../../../../../TASKS.md). The
[numeric specification](../../../../../wiki/spec/language/numeric_values.md)
and [target-realization decision](../../../../../wiki/drafts/designs/target_realized_float_arithmetic.md)
define the operations and their rounding and control requirements.

[main.omg](main.omg) passes actual operands into helpers and traps if any
operation returns the wrong result. Positive and negative finite inputs expose
wrong operation selection or reversed subtraction/division. The halfway cases
at `2^24` and `2^53` expose incorrect rounding. Quiet `0/0` must produce NaN,
and division by negative zero must produce negative infinity. The Build binds
all four hosted targets without choosing a float provider.

The [negative twin](../../../fail/float/canonical_arithmetic/main.omg) changes
only the first addition's right operand from `f32` to `f64`. The checker must
reject that format mismatch. The callers and runtime checks are identical.

Each arithmetic call retains the complete result-meaning equation, including
its ordered operands, format and sealed semantic contract. Calls substitute
their actual arguments and result simultaneously. The checker binds those
sources to their actual owner and producer; saved field observations retain
their Entry/Current identity. Meaning equality remains separate from IEEE
comparison and raw representation identity.

Conversions, other float operations, checked software realizations and other
transport places remain outside this witness's acceptance.
