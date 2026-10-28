# Core float representation

The ordinary float bit wrappers preserve a negative signaling NaN's exact sign and payload through transport, permitting subtraction of its presented bits.

Parent: `CORE-INTEGER-FLOAT-APIS` in [TASKS.md](../../../../TASKS.md).
The [numeric API contract](../../../../wiki/spec/language/numeric_values.md#core-integer-and-float-apis)
requires ordinary checked library declarations and exact presented bits, independently
of IEEE numerical equality. The consumed value is `0xFF80_0123u32`; each reject
twin changes its producer to `0xFF80_0124u32` without changing the consuming obligation.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| const name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| other state | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| other machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| target body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

The [original bit-wrapper rung](bit_wrapper_rung/main.omg) isolates the ordinary
call contract before generic library admission. All positives declare exit 0
and bind the four hosted entries. The root and places also require the
signaling NaN classification and exact explicit quieting result. Field canaries
and both match alternatives expose overlapping native homes or lost control.

Establishment comes from checked shared scalar views and the ordinary wrapper's
postcondition. Representation terms retain the exact source value and carrier;
IEEE equality does not imply representation identity. Calls, aggregates, joins,
constants and nested machines must carry that fact into exact subtraction without
a caller equality guard. Arithmetic-produced NaNs still lack raw-bit authority.

Terminal Psi must retain the scalar view reads and ordinary calls; native
realization must preserve the bits through scalar/aggregate homes, calls and
returns. Four target-keyed Build entries select the target-body implementations.
These witnesses require direct native emission and Terminal interpreter agreement;
compile-only runs do not establish runtime acceptance. The row exercises owned
scalar transport with temporary shared views, not mutable loan restoration or
foreign calling conventions.

The nine places and their reject twins are pinned under
[`tests/omega/pass/facts`](../../../../tests/omega/pass/facts) and
[`tests/omega/fail/facts`](../../../../tests/omega/fail/facts), using the
`core_float_representation_through_<place>_{exit,rejected}` names. All nine
positives executed with exit 0 in both engines on macOS arm64 in the focused
R38 corpus capture (producer `107c834ec646b5855dcd0df0ff81fd8beaa3f75e`);
all nine twins rejected for their own representation obligation. The constant
place additionally passed ordinary package admission and both engines with
producer `bd08f84ec2b8` in R39. Execution records and raw observations retain
the complete producer identities in the owning claim.

The [format vectors](vectors/main.omg) cover both permanent formats without
duplicating this row. They exercise finite endpoints, subnormal boundaries,
epsilon, metadata, signed zeros, infinities, NaN payloads and explicit quieting.
The [API controls](controls) cover unsupported and qualified carriers, invalid
payloads, same-spelling user declarations and representation claims that IEEE
equality does not establish. The
[computed-NaN canonicalization program](controls/canonical_computed_nan/main.omg)
and [unfixed bit observation](controls/computed_nan_bits/main.omg) remain the
open acceptance pair: the former must run in both engines, and the latter must
reject for lacking a fixed representation, rather than an unrelated diagnostic.

The row establishes scalar layout and calling-plan transport, selected operations,
register/stack homes and native image emission only through its actual programs.
It does not establish foreign ABI, mutable loan restoration, arbitrary aggregate
casts, or every backend instruction. The four entry declarations do not establish
cross-host runtime qualification. The parent API task remains open until the
computed-NaN pair and final focused acceptance complete.
