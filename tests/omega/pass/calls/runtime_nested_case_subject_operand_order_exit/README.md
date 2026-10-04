# Nested case-subject operand order

The subject call evaluates a scalar operand, an affine structural operand, and
another scalar operand before its body. Each appends its position to a shared
counter. Both selected cases must return their payloads through terminal calls,
with the counter recording `12341234`: one complete evaluation for each dispatch.
The affine input and nested result transfer once along the ordinary call route.

After package preparation, run `omega run --both main.omg`; both engines must
exit 0.
