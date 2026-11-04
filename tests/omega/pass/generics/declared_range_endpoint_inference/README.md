# Declared endpoint contracts

This program keeps the original endpoint helpers' input and result bounds while
selecting generic arguments explicitly. Calls still check the chosen bound;
choosing `N` does not prove `value <= N`. The existing runtime calculation and
exit-70 oracle are unchanged.

This is a bounded source restoration under
**CANONICALIZE-SCALAR-RANGE-CONTRACTS**. At historical revision
`7a9a7b8287e622137adeef7681b3143138d5724f`, these five positions used scalar
suffixes. Migration `818f643f12d35eabc9f3428b98ea4ce3bb7e2cc8` removed their
clauses as well as their suffix spelling.

| Original position | Original interval | Restored clause |
| --- | --- | --- |
| `upper_bound.value`, line 4 | `u64[0..=N]` | `requires value <= N` |
| `exclusive_upper_bound.value`, line 5 | `u64[0..N]` | `requires value < N` |
| `bounded_endpoint.value`, line 54 | `u64[1..=later_limit()]` | `requires value >= 1 && value <= later_limit()` |
| `bounded_endpoint.result`, line 54 | `u64[0..=later_limit()]` | `ensures result <= later_limit()` |
| `bounded_wide.value`, line 56 | `u64[9223372036854775808..=18446744073709551615]` | `requires value >= 9223372036854775808u64` |

The `u64` carrier supplies the zero floor and maximum ceiling where those are
redundant. The last lower bound is 2^63, so it must retain its unsigned meaning.
`later_limit()` remains the ordinary helper returning 256; calls used in its
clients' contracts still owe formation and invocation admission. No named domain
is introduced solely for a bound. The already migrated `bounded_value` result
clause stays unchanged.

The runtime entry checks seven existing results, including the forwarded bound,
signed and narrow values, full-width unsigned value and relayed generic calls.
It exits 70 on success and 71 on a mismatch. It does not execute every helper,
so runtime success alone is not evidence for every restored clause. Source
checking also owes each helper's complete contract.

The private `restored_endpoint_inputs` helper checks the valid boundary inputs
1, 2^63 and 255 at source checking. Its three exact copies change only one
actual input and must reject:

- [`bounded_endpoint(0)`](../../../fail/generics/declared_endpoint_named_input_zero/main.omg)
- [`bounded_wide(2^63 - 1)`](../../../fail/generics/declared_endpoint_unsigned_lower_bound/main.omg)
- [`exclusive_upper_bound<256>(256)`](../../../fail/generics/declared_endpoint_exclusive_input_equal/main.omg)

The related existing rejection controls retain ordinary bounded-call admission,
explicit generic call requirements and result proof:

- [`authored_numeric_name_requires`](../../../fail/calls/authored_numeric_name_requires/main.omg)
- [`inferred_equation_deferred_callee_requires_rejected`](../../../fail/generics/inferred_equation_deferred_callee_requires_rejected/main.omg)
- [`integer_piecewise_wrong_result`](../../../fail/proofs/integer_piecewise_wrong_result/main.omg)

This restoration does not audit the other `RangeValue` positions in this file,
retire range-shell inference in the compiler, or close the historical
1,598-position audit. The parent capability's remaining acceptance stays open.
