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

## Boolean helper requirements

Every place also has `negated_requirement/main.omg` and its
`negated_requirement/invalid/main.omg` twin. They copy this row's fact sentence,
rank sequence, signed carrier, interval, updates and canaries. The ordinary
`rank` helper requires `!(value > 7)` instead of `value <= 7`; the producing
measure still calls that helper and consumes its public result guarantee.
The twin changes only the selected increment from one to zero.

The optimized producer built from fixed base
`a91a8de86514f5b0ba71509b3af4d656916d6062` rejects all nine positives at rank-range
formation and source-complete cycle admission. Its SHA-256 is
`270923b0c4497ca7b098e55eaafe232601a8d1422258a44f3e256691206d4807`.
The arithmetic-negation control already executes with native/Terminal agreement;
negated and disjunctive helper requirements expose the narrower call-formation
reader. Raw baseline sources, commands, streams and producer bytes are retained
under `build/advance-negated/` in the owning worktree.

The repair uses the same selected Boolean paths for requirements and declared
matches. Each evaluated operand must form before its comparison contributes a
path premise. Every feasible requirement result must be true before the call
imports its result, and selected-path premises stay local. This establishes
exact-actual formation and composition without changing captured storage
identity, arithmetic safety, authority or termination obligations.

Three productions exercise the same fact in every transport place. The first
uses negation; the second uses
`value >= 4 && value <= 7 && (value == 4 || 8 / (value - 4) <= 8)`;
the third explicitly qualifies the division operands as Wrapping and erases
that policy from its result. At four the selected left alternative skips the
zero divisor; at five through seven the right alternative forms independently.
Every linked directory contains `main.omg` and its increment-zero
`invalid/main.omg` twin:

| Place | Negation | Guarded division | Wrapping division |
| --- | --- | --- | --- |
| local | [pair](local/negated_requirement/) | [pair](local/negated_requirement/guarded_divisor/) | [pair](local/negated_requirement/guarded_divisor/wrapping/) |
| field | [pair](field/negated_requirement/) | [pair](field/negated_requirement/guarded_divisor/) | [pair](field/negated_requirement/guarded_divisor/wrapping/) |
| parameter | [pair](parameter/negated_requirement/) | [pair](parameter/negated_requirement/guarded_divisor/) | [pair](parameter/negated_requirement/guarded_divisor/wrapping/) |
| call_result | [pair](call_result/negated_requirement/) | [pair](call_result/negated_requirement/guarded_divisor/) | [pair](call_result/negated_requirement/guarded_divisor/wrapping/) |
| match_join | [pair](match_join/negated_requirement/) | [pair](match_join/negated_requirement/guarded_divisor/) | [pair](match_join/negated_requirement/guarded_divisor/wrapping/) |
| const_name | [pair](const_name/negated_requirement/) | [pair](const_name/negated_requirement/guarded_divisor/) | [pair](const_name/negated_requirement/guarded_divisor/wrapping/) |
| other_state | [pair](other_state/negated_requirement/) | [pair](other_state/negated_requirement/guarded_divisor/) | [pair](other_state/negated_requirement/guarded_divisor/wrapping/) |
| other_machine | [pair](other_machine/negated_requirement/) | [pair](other_machine/negated_requirement/guarded_divisor/) | [pair](other_machine/negated_requirement/guarded_divisor/wrapping/) |
| target_body | [pair](target_body/negated_requirement/) | [pair](target_body/negated_requirement/guarded_divisor/) | [pair](target_body/negated_requirement/guarded_divisor/wrapping/) |

Contract checking now publishes its existing arithmetic decisions, including
occurrences under short-circuit paths, and predicate lowering consumes the
existing `formation_decided` field. A failed formation supplies no decision;
the proposed condition never licenses its own operands. Generated terms without
such a decision retain their explicit safety obligations.

The qualified division also needs the live lower endpoint before excluding
four. Both refuted equality and positive disequality consume the current integer
interval. Interior exclusions cannot choose a side, and IEEE values do not admit
integer adjacency. Explicit policy selection and erasure independently form
the operand and target carrier and preserve the payload. Already-typed call
arguments and measure results must retain their carrier and policy; a fitting
number cannot authorize an implicit erasure or width change.

The [focused controls](parameter/negated_requirement/controls/) cover nested
and disjunctive requirements, selected arithmetic, shift, remainder, policy
division, missing public premises, unsafe operands, interior exclusions and
IEEE adjacency. Explicit argument/result policy erasures run successfully;
the corresponding implicit erasures reject.

The eight requirement layers use the existing row's runtime falsifiers:

- Establishment: each helper requirement and partial operand forms before its
  public result can supply a rank bound.
- Identity and lifetime: saved input coordinates survive selected updates;
  requirement-local premises do not escape into later applications.
- Composition: all three productions preserve the same signed descent and
  exact-once counter operation across the nine places.
- Soundness boundaries: all 27 increment-zero twins reject for missing strict
  descent; nine additional negative controls reject their individual broken
  premises, safety or type obligations.
- Checked-to-Terminal Psi (`Psi 05–07`): occurrence-bound arithmetic decisions
  reach normal predicate lowering; the Terminal interpreter executes the
  selected and fallback paths and signed rank edges.
- Provider, layout and ABI (`Omega 00–02`): four-target Build bindings join the
  ordinary private calls, mutable array projections and explicit counter
  references. The Linux execution checks both selectors and neighbor canaries.
- Instruction and frame realization (`Omega 03–06`): caller-visible counter
  3→6→6, saved signed values and untouched neighbors expose lost updates,
  incorrect calls and clobbered homes across the complete cycle.
- Encoding and emission (`Omega 07–09`): direct Linux x86_64 native execution
  agrees with Terminal at exit 0 for all 36 positives, including the nine
  composition controls. All 36 intended-invalid samples match authored refusals.

The optimized executable for these observations was built from compiler source
`ec5a7071628362f00512e7701a79b53c71ad5327`; its SHA-256 is
`a1a8cdda6c2d383b5031e352533deb94a3953ec0f619e0ef32b63e40856fcac9`.
The immutable producer and raw results are retained in
`build/advance-negated/formation-samples.{json,log}` in the owning worktree.
The focused source corpus records 36 checked and 36 intended rejections; its
Linux combined corpus records 36 agreements at exit 0. Existing declared-call
and signed-rank controls retain seven refusals and two executable successes.
All 72 sources are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_signed_rank_negated_requirement_*`.
The broader parent acceptance, other-host execution and independent proof-kernel
qualification remain open.
