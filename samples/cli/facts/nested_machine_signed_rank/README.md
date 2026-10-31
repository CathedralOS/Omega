# Signed private-cycle ranks

Owner: [NESTED-MACHINE-UNIFICATION](../../../../TASKS.md).

Fact: a private helper preserves a signed invocation rank, and every complete
cycle strictly descends through zero while advancing the explicit counter once.
Every place uses the same input sequence 4→7 (or already-complete 7), produced
`i8` sequence 1,0,-1,-2, authored interval -128..=127, counter 3→6 and canary 641.
Both selected array elements and their untouched neighbors are observable.

The declaration accepts `u64` and returns `i8`; its ordinary helper publishes
`(result as i64) == 5i64 - (value as i64)` under `value <= 7`. Calls to that helper
also observe each signed result through an explicit widening conversion. The
carrier's finite order extends below zero; an optional authored interval
constrains produced ranks independently of their storage type. See
[finite integer ranks](../../../../wiki/spec/terminal-psi/control_flow.md) and
[termination measures](../../../../wiki/spec/language/termination.md).

- [local](local/main.omg), [no-progress twin](local/invalid/main.omg).
- [field](field/main.omg), [no-progress twin](field/invalid/main.omg).
- [parameter](parameter/main.omg), [no-progress twin](parameter/invalid/main.omg).
- [call_result](call_result/main.omg), [no-progress twin](call_result/invalid/main.omg).
- [match_join](match_join/main.omg), [no-progress twin](match_join/invalid/main.omg).
- [const_name](const_name/main.omg), [no-progress twin](const_name/invalid/main.omg).
- [other_state](other_state/main.omg), [no-progress twin](other_state/invalid/main.omg).
- [other_machine](other_machine/main.omg), [no-progress twin](other_machine/invalid/main.omg).
- [target_body](target_body/main.omg), [no-progress twin](target_body/invalid/main.omg).

The [original frozen probe](parameter/probe/main.omg) retains the first failing
program. Fixed base `0ff9f81b3933bb16aaa6eddd05fc7a40107252b0` refuses it and all
nine positive places with ranking/source-complete-witness diagnostics.
The completed producer runs all nine places, the frozen probe and four signed
variations in Terminal Psi and native macOS ARM64; all agree at exit 0. The
focused corpus independently records 14 such runnable positives, 13 intended
refusals and five existing nonincrease controls still rejecting. Three existing
neighboring programs also pass both engines.
Every no-progress twin changes only the pending update from +1 to +0.
Corpus pins are `nested_machine_signed_rank_<place>_{exit,rejected}` under
`tests/omega/{pass,fail}/facts/`.

The focused controls challenge the semantic premises:

- [Lower endpoint](controls/lower_endpoint/main.omg) descends entirely below zero
  and reaches exactly -128; [its preserving cycle](controls/preserving_below_zero/main.omg)
  must still reject rather than prove progress from a false nonnegative premise.
- [No authored range](controls/no_authored_range/main.omg) uses the carrier's
  finite order directly, without a synthetic annotation.
- [Signed inputs](controls/signed_input/main.omg) cross zero in the source
  coordinate; [signed forwarding](controls/signed_forward/main.omg) retains a
  declaration whose body is the identity rather than discarding its signed order.
- [Excluded negative arrival](controls/excluded_negative_arrival/main.omg) breaks
  the authored interval; [unsafe narrowing](controls/unsafe_below_minimum/main.omg)
  breaks the helper's own cast obligation; [implicit conversion](controls/implicit_conversion/main.omg)
  mismatches an already-typed helper result and the declared carrier. Each rejects
  on its own obligation.

Establishment and soundness: exact input observations, helper Requires and every
partial cast/subtraction form before the result supplies its carrier bounds.
No desired interval establishes its own operands. Builtin natural-distance and
zero-clamp rules keep their existing interpretation.

Identity and lifetime: the completed rank retains its exact immutable input
terms and both proven result endpoints. Later observations restore those bounds
on that saved result, independently form the destination, and compare produced
values in their signed order. Writes do not turn the earlier input into a fresh
load or grant facts to the next application.

Composition: source and destination retain their own formation and membership;
private coordinates are observed in their actual input carriers. Nonincrease
alone cannot justify a complete cycle: every cycle must contain strict descent.
The constant counter and array canaries falsify lost updates, repeated work,
wrong selector geometry and incomplete returns.

Terminal and Omega: normal measure applications produce typed result values;
the existing private rank parameter and `IntegerType` sign/width carry that
value through the cycle and codec. Terminal and native use ordinary signed
comparisons, conversion, ABI and storage paths. This repair adds no alternate
emitter, wire schema or implicit conversion.

The parent remains open for other declared bodies, nonscalar and dependent
orders, the remaining invocation-contract obligations and product-source
migration. Other-host execution and independent proof-kernel qualification
remain outside this bounded repair. Exact producers, frozen baseline failures
and raw execution observations are retained in `build/signed-rank/`.
