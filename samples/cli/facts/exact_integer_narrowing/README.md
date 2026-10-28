# Exact integer narrowing

Every private arrival supplies an integer from 0 through 255, so exact narrowing to u8 preserves its value.

Owning task: **DOMAIN-DEFAULT-PREDICATE-UNIFICATION**, including its product
native-harness prerequisite. Console byte reads and lexer hex assembly require
the shared occurrence proof for exact narrowing.

The nine programs retain the same exact `i32` to `u8` cast in a private helper.
The helper has no duplicate guard or bound. A dominating guard establishes
`0 <= value <= 255` on every actual call, or the const-name place supplies its
two exact declared values. Positive calls observe 7 and 231 with distinct tags;
input-bearing places also exercise 0 and 255 where useful. Every guarded place
also calls its wrapper with 256 and checks the guard-false fallback; the bound
change alone therefore creates an actual bad private arrival in its twin.
The const-only place changes its second constant from 231 to 256. The main
comparisons are runtime oracles, never premises for forming the cast.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Build-selected implementation | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Each twin changes only the upper guard to permit 256, or changes the second
constant from 231 to 256. Every positive has all four hosted entry bindings and
exit 0; the target-body place uses four ordinary `Computer` implementations with
four explicitly selected entry wrappers. The source/build pairs are pinned as
`exact_integer_narrowing_through_<place>_{exit,rejected}` in the facts corpus.

The [computed-source pair](controls/computed_source/main.omg) retains a borrowed
`Parts.hi < 16` guard across a private helper's unrelated marker write, then
establishes `Parts.lo < 16` before a second private helper evaluates
`((parts.hi * 16) + parts.lo) as u8`. The fields use `u32 in Trapping`; there is
no copied scalar requirement or duplicated helper contract. Its
[twin](controls/computed_source/invalid/main.omg) changes only the first guard
to `hi < 17`. This is another production of the same completed-value obligation,
not a width row. The oracle checks 7, 231, 0 and 255, the marker write, and both nonzero
neighbor canaries. An input with `hi == 16, lo == 0` takes the positive guard-false fallback and
becomes an actual out-of-range cast arrival in the twin.

Requirement evidence:

- **Establishment:** ordinary guards, exact constants and producer requirements
  establish every private actual. An unproved destination goal cannot supply
  either the lower or upper bound. The cast preserves the value; no truncation,
  saturation or alternate arithmetic policy is substituted.
- **Identity and lifetime:** local copies, exact field reads, const identities,
  completed calls and match joins retain their selected value. Match arms select
  input or literal231, with distinct observed outcomes. The computed control
  retains the guarded borrowed hi field across an unrelated marker write, then
  guards lo and reads both through an explicit shared reborrow; its neighbors
  and the field-place neighbors are checked in main.
- **Composition:** calls substitute the actuals simultaneously through the
  complete closed private arrival roster, including nested routes, an ordinary
  relay and Build-selected implementations. A single permitted out-of-range
  arrival prevents admission; no same-spelled or after-call predicate is used.
- **Soundness boundaries:** the helper receives no ownership or access authority
  from its bounds. Existing escaping-selection, changed-subject, self-formation
  and cyclic-arrival controls in `nested_machine_invocation/private_arithmetic`
  retain their separate refusal obligations; this row adds no recursion rule.
- **Psi 05–07:** cast inputs/results and selected tags execute through ordinary
  canonical Terminal Psi. The proof is erased; no runtime subject, guard
  insertion, duplicated input evaluation or result placeholder is introduced.
- **Omega 00–02:** ordinary scalar result/argument placement realizes the same
  `i32` and `u8` meanings, with existing Note/Frame/Scratch layouts. This row
  claims no new provider authority, aggregate ABI classification or foreign lane.
- **Omega 03–06:** native instruction selection must preserve each admitted
  integer value through calls, branches, registers and activation homes. The
  tag and canary checks make incorrect branch/field lowering observable.
- **Omega 07–09:** each positive needs actual native/Terminal agreement and
  canonical reload under the owning qualification. Four authored bindings do
  not establish execution on a foreign host.

## Baseline and remaining scope

The frozen optimized producer is
`2150f33d90259b83580d96d6eacf4cc9c3b540df`, with explicit matching
`OMEGA_LIBRARY_ROOT`. All nine positives and the computed-source positive
wrongly refuse the private exact cast as not provably representable. Their ten
invalid controls refuse at the same exact-cast gate. Raw streams, source hashes
and producer bindings are retained in the owning evidence; baseline refusal is
not candidate acceptance.

Actual optimized integrated producer
`8110ebd0070bc4a8d2724f1abd4a5bd30a48a315`
on `macos_arm64` accepts the twelve positive programs and rejects all thirteen
controls for their intended obligations. All twelve execute with native and
Terminal exit 0; thirteen fresh copied-project runs, including the companion
Wrapping remainder control, also retain and reload their canonical Terminal
artifacts. The integration rechecks all 789 affected retained source outcomes
without semantic movement. Corpus rows retain their original 4588 recording
identity; the new qualification records identify 8110 explicitly. Four authored target
bindings establish no execution claim for the other three hosts.

This completes the narrowing row and its computed-value controls. It closes
neither Product subject migration nor the full owning task. Other integer
conversions, floating conversion policies and higher-order or cyclic private
inference retain their own acceptance; there is no count/width matrix.

Selected mathematical descriptions remain query-owned recipes until child
formation, operand definedness and result landing independently succeed. An
authenticated normal Trapping value retains its opaque identity when optional
mathematical unfolding cannot be proved. Wrapping and Saturating keep their
selected node results and count rules; raw legacy range normalization retains
its separate formation checks. The owning qualification distinguishes the
fresh 8110 integration observations from the retained 4588 recordings and
4c99 comparisons. The reviewed one-line lint change and actual main conflict
resolution retain their separate source identities.

## Selected-operation formation controls

The [selected-operation positive](controls/selected_operations/main.omg) narrows
completed Trapping add, subtract, multiply, divide, remainder and both shift
results from the same bounded private input. Seven disjoint byte lanes in one
`u64` make each result observable without adding an aggregate ABI. Inputs 1, 7
and 15 check all lanes; 0 and 16 check the guard-false route and untouched marker.
Every call also checks the nonzero neighbor canaries.

Its [twin](controls/selected_operations/invalid/main.omg) changes only the upper
guard from 15 to 16, admitting an actual 256 result for multiplication and left
shift. Selected Trapping policy remains recorded: mathematical consequences are
available only after independent, child-first carrier/operation landing checks;
the receiving `u8` goal never establishes those children.

The [unsafe Exact-child reject](controls/unsafe_exact_intermediate/main.omg)
explicitly erases the source policy before an overflowing Exact addition. A later
multiplication by zero cannot hide that child's formation obligation. This is
not a claim that Trapping overflow must reject statically; its runtime crash
semantics remain unchanged. Exact `64ce` baseline refusals and fresh candidate
acceptance belong to the retained qualification records.

## Guarded normal Trapping results

The [guarded-result compatibility positive](controls/guarded_trapping_result/main.omg)
forms `computed = value + 1` under Trapping policy without an upper requirement,
then guards its completed normal result below 256 before calling private
`narrow`. Its actual inputs 0, 7 and 254 produce 1, 8 and 255; 255 and 300 take
fallback zero. The program does not execute MAX, whose addition may trap.
Retained `64ce` Source and native/Terminal execution already accept this program,
and its canonical artifact reloads successfully.

A prospective mathematical denotation may be unavailable when no independent
NoTrap proof exists. That must preserve the already observed opaque normal
result, its carrier and the dominating guard. It cannot turn existing accepted
source into a mandatory no-overflow proof or erase Trapping semantics.
The [twin](controls/guarded_trapping_result/invalid/main.omg) changes only the
guard to below 257, allowing the actual completed value 256 into the narrowing;
`64ce` rejects its exact cast. New candidate checks must preserve both outcomes.
