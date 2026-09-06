# Epsilon checking conformance

Run `sh tests/epsilon/checking/run.sh` from the repository root. This gate
checks complete Epsilon source without executing `Main` or an unused machine.
The selected Gamma evaluator runs the canonical Delta compiler to compile the
exact manifested Epsilon source closure plus `checking_driver.delta` under
`ConformanceBytesV1`.

The driver calls only `epsilon_check_through_body`. Its private diagnostic
format is one `00` byte for acceptance; a rejection reason byte from the closed
language table followed by the exact four-byte little-endian source offset for
rejection; or `ff` followed by that offset for an internal contradiction. This
is not the final Epsilon request or observation envelope.

`fixtures.tsv` pins every fixture's exact bytes, SHA-256 digest, and complete
diagnostic hex. Coordinates are authored fixture expectations, not calculated
from semantic source parsing in the runner. The runner checks identities,
frames/invokes the selected stages, and compares exact observations. It neither
extracts checker functions nor translates Epsilon source.

The original 16 fixtures cover raw, grouped, and nested non-callable continuation targets,
grouped genuine machine/state calls, independent unknown arguments, unresolved
callees, genuine-call arity, and after-never parent suppression while child
name checking remains active. Invalid-control anchors retain the outer
continuation start; independent call and argument judgments retain their own
language-defined anchors. All probe machines are unused by the empty Main,
so execution reachability cannot substitute for complete checking.

Two checker defects motivate the controls. Grouping a non-callable continuation
previously bypassed control-target rejection; grouped and nested forms must
retain the same judgment at the outer continuation start. Separately, a
transition after a resolved `never` call still derived subject-admission and
sum-coverage parent judgments. D53 suppresses those parent relations and
requires `InvalidTerminal` at the transition's closing brace, while independent
subject, argument, and pattern name errors remain active. Paired reachable
record-subject and incomplete-sum controls retain their ordinary `TypeMismatch`
and `NonexhaustiveSum` judgments.

Another 22 controls cover complete operand premises for binary, index, and
slice relations. An unresolved operand or bound cannot be replaced by the
sibling's resultless category to derive an enclosing `TypeMismatch`. Doing so
previously either displaced the correct child diagnostic or collided with it
at the same coordinate and produced `InternalFailure`. Ordinary parent joins
also cannot consume a nonreturning operand: its independent `InvalidTerminal`
retains the exact call-head anchor. Paired controls retain resultless mismatches
when all parent premises exist and preserve D52's independent machine and
constructor argument rules. The previous checker fails 13 of these 22 exact
observations; grouping and opposite operand orders prevent an implementation
from merely suppressing coordinate ties.

Ten pattern controls retain terminal-local identity ownership across machine
entries and states, including wildcard-only boundaries. Nonadjacent scalar
aliases and repeated sum cases still reject; owner compatibility, payload arity,
earlier child errors, and after-`never` suppression keep their exact precedence.
The checker searches only the current terminal's contiguous prepended pattern
facts for duplicates. It retains the complete program ledger for later consumers.

Four state-width controls each contain 260 authored states in one unused
machine. They require acceptance of distinct empty states, `DuplicateName`
at the final repeated state declaration (offset 5,703), and `UnknownType` at a final
state parameter or local annotation (offsets 5,724 and 5,732). These sources
previously ended in outer Gamma status 250 with no checker observation because
state census and formed-type folds retained one return context per state.
The controls require complete checking and exact diagnostics under the existing
selected evaluator; 260 is a regression width, not a new language limit.

Two value-edge controls declare a record with 260 named by-value fields. The
acyclic record must be accepted; replacing only the last field's type with its
own record type must report `RecursiveValueType` at that type token (offset
4,943). Two declaration-width controls contain 64 empty data declarations,
64 unused machines, and the three entry-support declarations. The valid
131-declaration source must be accepted; an `i32`-returning `Main::main` must
report `InvalidEntry` at its machine declaration (offset 2,643). These pairs
previously ended in outer Gamma status 250 instead of publishing a checker
observation. They exercise edge/declaration folds under the unchanged selected
profile; their widths are regression cases, not Epsilon limits or a claim that
all source sizes fit the evaluator.

Four small entry-support controls preserve the declaration scans' seen-state
and candidate ordering. A second valid `Console`, `Main`, or `Main::main`
declaration reports `InvalidEntry` at that declaration's first byte (offsets
177, 209, and 242). With an authored entry candidate, the census defers these
reserved duplicates to entry formation. A missing `Console` alongside an
`i32`-returning entry reports the authored entry defect at offset 32 rather
than the missing-support candidate at source extent 83. The pre-repair checker
also publishes these exact diagnostics; the fixtures pin their preservation.

`receipt.tsv` records the measured 700,181-byte checker receipt with SHA-256
`b8e0c5d2f7eb9bd851fdd13313da56ba0dcf765b36dc322a7995bb14239d5830`.
Every gate run reconstructs this exact receipt before comparing the 60 complete
judgments. These controls establish the listed checking relations, not full
Epsilon conformance, runtime execution, or closure of the Omega bootstrap edge.
