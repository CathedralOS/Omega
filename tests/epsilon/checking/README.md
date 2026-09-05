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

`receipt.tsv` records the measured 694,710-byte checker receipt with SHA-256
`95a0a947259bff19c792b9fb1dbb25afc042a12aef33f2232a680786752172b0`.
Every gate run reconstructs this exact receipt before comparing the 48 complete
judgments. These controls establish the listed checking relations, not full
Epsilon conformance, runtime execution, or closure of the Omega bootstrap edge.
