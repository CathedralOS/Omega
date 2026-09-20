# Beta theory component profile

[Theory and ownership](README.md) | [Executable gate](../../../tests/gamma/beta-encoding-theory/README.md)

This profile covers the current source-owned theory emitter and finite
diagnostic equations. It is not a profile for the certificate producer or
the accepted whole-source artifact.

## Emitter

The marked test entry plus the exact thirty-two-member source closure is 3,434
lines, 131,270 bytes, SHA-256
`9e7a7f019c048cf0f00900a26f7ad561af5a4e65a136688ed03222a992a60f3c`.
Its only admitted input is empty; framing occupies 131,274 bytes. Nonempty input
returns status 1 without publishing output. The fixed emitted section is
116,992 bytes, SHA-256
`b2ab717f574b39e7ef6986b2ec3a43036a2ca4f5e7512c1dcbe3435b564ebb4b`.
The test-owned pins bind these exact bytes; they are diagnostic custody checks,
not arithmetic operations, checker rules, or artifact authority.

The composition has 126 Gamma functions, maximum arity ten, twelve nested
expression-body lists, and at most 440 simultaneously active bindings in the
generated automaton-clause emitter.
All constructor, clause, function, variable, cell, and administrative-word loops
are tail calls with fixed finite iteration counts. The emitter allocates no
pairs; its marked entry allocates one outcome pair.

A source call-path audit allows seventeen contexts and eighteen frames,
including pending calls while arguments are evaluated. Ignoring tail-call
savings, the longest acyclic function route has fifteen functions, through the
theory/vocabulary/encoder-vocabulary/scalar-constructor/word writers. Pending
classification while a clause call is being prepared is also included. Tail-only
self loops do not accumulate contexts. Thus `18 * 450 = 8100` binding rows
suffice. Allow sixteen argument/helper slots per expression level and thirteen
levels per frame: at most `18 * 13 * 16 + 32 = 3776` temporary entries, below
the evaluator's 524,288.
These are conservative source bounds, not measured runtime peaks. A changed
closure or entry requires a fresh audit.

## Checked finite requests

The generic checker diagnostic is 63,504 bytes under
the [existing checking profile](../checker/CHECKING.md).
The emitted theory has `S=18, C=361, A=40, F=108, W=29247`; its formation work
estimate is 163,613. Every supplied clause and proof row is checked, including
ones not used by the final root.

| Positive batch | Proof rows | Required cumulative checking work |
| --- | ---: | ---: |
| All lexical equations | 1,024 | 137,781 |
| All nibble joins | 768 | 10,757 |
| All high/low nibble splits | 512 | 68,869 |
| All composed split/join round trips | 1,792 | 84,741 |
| Thirteen fixed-width Word cases | 13 | 928 |
| Counter byte helpers, each 128-case half-table | 128 | 9,029 or 25,413 |
| Nineteen separate checked successor cases | 21..84 | 290..3,341 |
| All ordered nibble pairs | 768 | 10,757 |
| Twelve separate byte comparisons | 19 | 204..1,284 |
| Thirty separate word comparisons | 177 | 2,004..10,644 |
| Encoder lexical predicates | 47 | 913 |
| Encoder choosers | 19 | 194 |
| Encoder state/fragment accessors | 84 | 1,696 |
| Encoder list operations | 48 | 643 |
| Source admission | 129 | 2,551 |
| Hexadecimal parsing | 226 | 5,888 |
| Token-automaton single steps | 835 | 15,240 |
| Token-automaton fold/end | 201 | 3,892 |
| Token classification | 4,360 | 60,538 |
| Output emission | 403 | 7,297 |
| Operand/mnemonic dispatch | 534 | 10,978 |
| Scan flush | 1,088 | 20,511 |
| Scan finalization | 765 | 13,725 |
| Tiny end-to-end encode, valid | 7,657 | 123,466 |
| Tiny end-to-end encode, rejected | 6,145 | 100,183 |

The encoder batches are produced by
[stepper.py](../../../tests/gamma/beta-encoding-theory/stepper.py), the generic
formed-theory stepper that replays ground applications through their stated
clauses and emits the ordinary certificate rows. Their equations are declared
in [encoder.py](../../../tests/gamma/beta-encoding-theory/encoder.py); every
batch is checked by the unchanged generic checker. The largest encoder request
is 418,248 bytes (the valid end-to-end encode).

The [gate's work derivations](../../../tests/gamma/beta-encoding-theory/README.md)
count clause selection, indexing, every visit/resumption, ground comparisons,
explicit rules, and final-root checks independently of observed output. A Word
unfolding includes seventeen template visits, seventeen resumptions, and sixteen
ground-comparison transitions; none of its eight fields is an unchecked byte copy.

The largest request is 188,980 bytes and the largest ground table has 1,552
rows, both in the round-trip batch. Including checker source and framing gives
252,488 bytes. Across these finite families, the generic cumulative pair bound
is at most `63752 + 137781 * 48 + 128 = 6677368`, below the selected arena's
3,422,453,760 pairs. Each vector is a separate evaluator invocation; these figures
do not claim that unrelated certificates can reset accounting mid-request.
Malformed Word arities and semantic corruptions must publish exact owned
rejections. A timeout, evaluator failure, or short observation is not a verdict.

The counter cases explicitly compose carry selection and increment through
ordinary proof rules. The maximum-Word case proves Overflow, not a resource
refusal or a wrapped successful value. These finite requests fit the current
checking profile; they do not establish the cost of a full-source certificate.

Ordering cases compare all 256 nibble pairs, literal byte priority examples,
and complete words including every highest-differing byte position and exact/
adjacent Beta capacity words. The word recipe explicitly normalizes all eight
byte comparisons before composing the ordering chain; even a decisive high byte
does not hide unchecked fixture rows. The ordering requests use at most
144,996 bytes, 531 ground terms, 768 proof rows, and 10,757 checking work.

On macOS arm64, the full gate passed two identical emissions, three nonempty-input
producer refusals, and all 181 exact checker diagnostics. Emission took
2.453..3.035 seconds; the exhaustive nibble-ordering batch took 14.221 seconds.
The twelve byte-comparison cases took 9.520..9.701 seconds and the thirty
word-comparison cases took 10.329..10.739 seconds. These are scoped
observations, not semantic limits or portable
performance guarantees.

## Full-subject certificate measurement

The same stepper independently reconstructed the owner-fixed proposition
`encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)` for the complete selected
subject — the 47,756-byte evaluator Beta source as a midpoint-split Source
tree (22,339 interned owner terms; the tree shares identical subtrees) and
the 8,575-byte persisted tape — and produced the complete untrusted
derivation in 24.1 seconds at ~2.8GB peak RSS on macOS arm64:

| Quantity | Measured |
| --- | ---: |
| Owner proposition bytes / terms | 534,208 / 22,339 |
| Certificate bytes | 134,800,268 |
| Witness terms | 2,130,039 |
| Proof rows (unfold / trans / congr / refl) | 3,182,484 (1,018,573 / 1,157,485 / 936,597 / 69,829) |
| Maximum proof depth | 204 |
| Total request bytes | 135,451,492 — 16.1 times the 8,388,608 provision |
| Projected checker work | ~45-52M — ~70-80 times the 655,360 provision and the ~675,017 pair ceiling |

The derivation value is exactly `Success(T)` for the real tape. Every figure
above was reproduced exactly from the checked-in stepper at `6a1751fe08` on
macOS arm64 (24.2 seconds), including owner/witness counts, the per-rule row
totals — which contain no symmetry rows — and the 204 maximum premise depth
(a premise-free row counts depth one). The same figures were reproduced a
second time on Linux x86-64 at `87d8b22713ff` under Python 3.10: the
sha256-pinned host-side theory reconstruction, the midpoint-split owner
proposition, and the full emission completed in 23.8 seconds at ~2.3 GiB peak
RSS with byte-identical section sizes and per-rule totals.
The gated mode `tests/gamma/beta-encoding-theory/run.sh --full-subject`
reproduces the production on any host with python3 — no evaluator seed is
materialized — pinning the theory reconstruction, both subject identities,
and every emitted figure so the run fails loudly on drift. Under its
documented floor-half midpoint partition the owner table measures 22,253
terms / 532,144 bytes and the complete request 135,485,120 bytes
(3,182,974 rows: 1,018,733 unfold / 1,157,751 transitivity / 936,662
congruence / 69,828 reflexivity / 0 symmetry, maximum premise depth 204,
~30 seconds and ~2.3 GiB on Linux x86-64), a ~0.03% table-size difference
from the first production above, whose ad-hoc partition detail was not
retained; the same equation derives under both, and both fit the selected
provisions. The mode also emits the retained-role census the audit needs:
every proof rule appears except symmetry, 107 of 108 theory functions unfold
(function 62 never does), 359 of 361 constructors appear in terms (S_EMPTY
and A_EXHAUSTED do not — this subject has no empty spans and its admission
never exhausts), and 1,911 of the 2,813 declared clauses appear as unfolding
premises.
The same gap blocks production through the selected chain: the certificate
exceeded the evaluator's former 16,777,212-byte buffered-output provision;
the AlphaBootstrapV5 provision is 135,266,304 bytes.
Owner decision `beta-encoding-certificate-admission` settles admission on the
native-backing route, recorded at the
[checking ledger](../checker/CHECKING.md#complete-generic-execution-provision);
the buffered-output provision is one of the extents that grows with it.
The request
cannot be admitted by the selected checker; the projected work uses the
measured 13.9-16.5 work/row across the checked encoder batches, not an
executed run. That ledger now selects the coupled provisions — the
136,314,880-byte request extent, the 137,363,456-byte evaluator frame, the
67,108,864-unit work counter, and the 3,387,293,850-pair arena — pending the
Alpha extent-supply leg that realizes them.
This is one straightforward producer shape, not a lower bound;
the [cost review](../../../wiki/drafts/bootstrap_cost_review.md) records the
named reduction levers and confirms the remaining shortfall is structural.

Windows runtime validation is unavailable in this session. The gate documents
the same Git Bash/Python entrypoint for Windows x64 and macOS arm64.
Full-source certificate acceptance remains open on the resource routes named
in the cost review.
