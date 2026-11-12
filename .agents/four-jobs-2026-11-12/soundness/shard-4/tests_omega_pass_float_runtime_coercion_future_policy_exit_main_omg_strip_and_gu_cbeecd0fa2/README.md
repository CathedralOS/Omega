Run `mbx run -p omega -- run tests/omega/pass/float/runtime_coercion_future_policy_exit/main.omg`.

Float carrier coercions preserve the source value under every arithmetic policy.
The selected policy governs subsequent integer arithmetic, not the conversion.
The program checks wrapping and saturation at the byte boundary, an in-range
Trapping addition, and an exactly representable policy-qualified format change.
Source evidence comes from a live equality guard, a stored literal and landed
literals, not from the coercion's requested target.
