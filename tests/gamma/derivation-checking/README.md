# Supplied-theory derivation checking

Run `sh tests/gamma/derivation-checking/run.sh` from the repository root.
The gate uses the selected Gamma evaluator on macOS arm64 or Windows x64 Git
Bash. An unavailable Python installation explicitly skips; other hosts report
unsupported. Host availability is not evidence of cross-platform execution.

The tiny [main.gamma](main.gamma) calls the complete source-owned
[`check_derivation`](../../../bootstrap/proofs/checker/CHECKING.md)
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
closure prefixes the bound checker member bytes with this gate's
`main.gamma` entry, bound at 1,155 bytes, SHA-256
`8601e23955e3054eba95a2b5e7e2dd2a92d4ae47c8cb9bf49d9ce77c295a16a2`; the pin
lives in `tools/bootstrap/proofs/sources_env.sh` and
`require_derivation_checking_entry_identity` runs before the pack. The
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

There are 82 vectors and 160 observations: 78 small vectors run twice with a
60-second host timeout, and four large vectors run once with 600 seconds.

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
one row yields `4P+8`. The 163,838-row fixture therefore completes at exactly
655,360 units. With 163,839 rows, the first final root comparison requests
655,361 at byte 2,621,604 and must refuse.

A 262,143-row table consumes 262,144 units during setup and the remaining
393,216 in 131,072 Ref rows. The next row reservation refuses at byte 2,097,268
with the full limit/requested values. The 130 MiB request extent also admits a
fresh proof-index reservation exhaustion directly: a 655,360-row table of
minimum-size Reflexivity rows requests 655,361 units during setup and refuses
on the table itself (`fresh_proof_index_reservation_exhaustion`). Substitution's
bulk controls separately cover exact and adjacent reservation refusal in an
already consumed session.
The 32,768-row backward Symmetry chain costs `6P+3 = 196,611`; it checks logical
proof depth without expanding the chain or recursively traversing premises.

The connected all-rules fixture costs 39: six setup units, row costs
8/5/3/7/6, and four final-comparison units. The FORMAT example costs 14.
These are authored expectations from the documented transition charges, not
values learned by executing the implementation under test.
