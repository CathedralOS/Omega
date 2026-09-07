# Supplied-theory derivation checking

Run `sh tests/gamma/derivation-checking/run.sh` from the repository root.
The gate uses the selected Gamma evaluator on macOS arm64 or Windows x64 Git
Bash. An unavailable Python installation explicitly skips; other hosts report
unsupported. Host availability is not evidence of cross-platform execution.

The tiny [main.gamma](main.gamma) calls the complete source-owned
[`check_derivation`](../../../bootstrap/gamma/derivation_checker/CHECKING.md)
entrance. Success is `07` followed by proof count and cumulative work as two
u64 little-endian words (17 bytes). Rejected/Incomplete observations contain
their tag and four u64 failure fields (33 bytes). Every observation requires
process zero, exact complete stdout, and empty stderr. A process failure,
timeout, or partial observation is not a proof verdict.

This is a generic proof verdict under the supplied formed theory, with the
last checked equation constrained to the supplied owner root. It is **not
artifact admission**: the full Beta subject, intended theory, encoding
proposition, and artifact custody remain separate requirements.

## Source and fixture custody

[run.sh](run.sh) resolves bootstrap roles and materializes the complete
canonical implementation closure with the explicit diagnostic entry. The
composition's line count, byte count, and SHA-256 must match
[source.tsv](source.tsv). No production functions are extracted or replaced.
[gate.py](gate.py) only frames those exact bytes, invokes the evaluator, and
compares literal expected observations.

[fixtures.py](fixtures.py) coordinates concept-owned groups. The shared
[proof_wire.py](proof_wire.py) uses the retained layout gate's literal field
encoder; field coordinates are sums of authored section/record prefix sizes.
Host code does not decode theories, check proofs, compare terms, substitute
templates, or derive proofs from program semantics. It only encodes explicitly
authored test rows, which ordinary Gamma source admits and checks.

## Retained controls

There are 83 vectors and 161 observations: 78 small vectors run twice with a
60-second host timeout, and five large vectors run once with 600 seconds.

- [positive.py](positive.py): all five rules, ordered constructor congruence,
  repeated premises, duplicate/witness structural aliases, and a connected
  five-row derivation in which Unfold, Symmetry, Reflexivity, and Transitivity
  feed the final Congruence.
- [references.py](references.py): empty proof tables; invalid left/right term
  identities and sorts; zero, self, future, cyclic, and unused invalid premise
  rows; complete-table checking rather than acceptance of a valid prefix.
- [relations.py](relations.py): exact endpoint order for Reflexivity, Symmetry,
  and Transitivity; premise validation before relations; Congruence head,
  count, ordered premise, and child-relation failures.
- [unfolding.py](unfolding.py): clause/head failures retain code 10, incorrect
  substitution yields code 12, constructor-child case bindings work, earlier
  proof equations neither evaluate a case subject nor pollute structural memo.
- [roots.py](roots.py): the last row must establish the owner root in its
  original orientation; neither earlier conclusions nor invalid suffixes may
  replace it.
- [forwarding.py](forwarding.py): malformed outer/layout bytes, formation and
  ground errors, and existing sort/work refusals precede proof processing.
- [resources.py](resources.py): actual large proof tables exercise cumulative
  reservation and indexed backward premises, without injected session state.

## Independent work expectations

For `P` Reflexivity rows comparing the same valid term, setup costs `P+1`,
each row costs `1+2`, and final root comparisons cost four: `4P+5` total.
A nullary constant Unfold costs three more than a Reflexivity row, so replacing
one row yields `4P+8`. The 65,534-row fixture therefore completes at exactly
262,144 units. With 65,535 rows, the first final root comparison requests
262,145 at byte 1,048,740 and must refuse.

A 262,143-row table exhausts the allowance during setup and refuses the first
row reservation at byte 116. A 262,144-row table refuses setup itself at the
proof-count field, byte 108. Both report the full limit/requested values.
The 32,768-row backward Symmetry chain costs `6P+3 = 196,611`; it checks logical
proof depth without expanding the chain or recursively traversing premises.

The connected all-rules fixture costs 39: six setup units, row costs
8/5/3/7/6, and four final-comparison units. The FORMAT example costs 14.
These are authored expectations from the documented transition charges, not
values learned by executing the implementation under test.
