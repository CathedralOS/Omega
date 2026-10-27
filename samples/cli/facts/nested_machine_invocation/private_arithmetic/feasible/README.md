# Feasible private arithmetic arrivals

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../TASKS.md).
This is another production of the [private arithmetic fact](../README.md):

> Every feasible private arrival carries offset < limit, so the helper can form offset + 1.

An initial bound admits the first increment. The false edge of an ASCII-style
compound guard splits into alternatives. A later byte guard rules one alternative
out numerically: the same captured byte cannot be both `<= 0` and `>= 194`.
The remaining paths carry the next offset's strict bound into the final helper.
No caller result guard establishes the helper's arithmetic formation.

| Place | Program | Control |
| --- | --- | --- |
| local | [positive](local/main.omg) | [reject twin](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [reject twin](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [reject twin](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [reject twin](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [reject twin](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [reject twin](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [reject twin](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [reject twin](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [reject twin](target_body/invalid/main.omg) |

Every twin changes only `next < limit` to `next <= limit`. Its still-feasible
maximum-value arrival cannot form the final increment. Additional controls retain
an [unformed comparison](controls/self_formation/main.omg), a
[replaced subject](controls/changed_subject/main.omg), and an
[unbounded feasible alternative](controls/unknown_path/main.omg). None may be
removed by interpreting failed proof search as a contradiction.

All nine positives and nine twins refuse on optimized producer `9f2d790a91c`,
whose compiler inputs match the starting source `e522fcf4816`. Producer
`494b66e5807` executes every positive with native/Terminal exit-zero agreement
on macOS ARM64; each twin and all three controls still reject for their own
arithmetic obligation. All four hosted entries are authored. Native execution
on other hosts remains unverified. The fixture prefix under
`tests/omega/{pass,fail}/facts/` is `nested_machine_private_arithmetic_feasible_`.

The runtime oracle exercises two accepted multibyte ranges, both match-join
arms, ASCII, zero, out-of-range bytes, an exhausted inner bound and the initial
bound's false edge. Replacing an accepted path by a fallback or changing its
increment makes the oracle trap. The field place also observes the neighboring
field values through its guarded transport.

Applicable requirements:

- Establishment: prior guards establish each bound; only independently formed
  live integer premises can refute an incoming path. The requested increment
  and its result are absent from the infeasibility query.
- Identity and lifetime: shared captured-value identities distinguish the byte
  before and after replacement. Unsupported projection stays unknown.
- Composition: all nine places preserve the exact offset, bound and operation
  across ordinary and terminal private invocations. Closed arrival coverage is
  unchanged, including every feasible or unresolved predecessor.
- Soundness boundaries: the established integer engine supplies contradiction;
  neither an unavailable premise nor failed operation admission proves one.
  The formation, mutation and weakened-bound controls remain refusals.
- Psi 05–07: actual Terminal execution retains the selected guard paths,
  operand schedule and helper result through the existing lowering. This row
  introduces no proof erasure or cleanup disposition.
- Omega 00–02: existing scalar/record layouts, explicit call arguments and
  hosted ProgramEntry bindings carry this fact. No new ABI boundary is claimed.
- Omega 03–06: native execution checks increments, branches and continuations.
  This acyclic row does not establish recursive WCSU or new storage authority.
- Omega 07–09/emission: all nine direct macOS ARM64 images execute and agree
  with Terminal Psi, including the falsifiable result checks.

The standard-library replay removes its three UTF decoding arithmetic refusals;
four qualified ancestor-call diagnostics remain before the product parser gate.
The parent migration remains open for those calls, the Omega-written declaration
model, remaining custody/restoration and stack acceptance, and downstream source
migration. Run records remain in the lane's qualification evidence with exact
producer, source hashes and engine observations.
