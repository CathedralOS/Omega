# Explicit derivation checking

[Format](FORMAT.md) | [Comparison](COMPARISON.md) | [Substitution](SUBSTITUTION.md) | [Calculus](README.md)

`check_derivation()` is the generic ordinary-Gamma entrance. It checks the whole
supplied proof table and its final conclusion against the owner root. It does
not authenticate an artifact's theory or reconstruct its proposition. The full
Beta definition package, owner-fixed encoding proposition, complete certificate,
and admitted artifact closure remain separate acceptance requirements.

## One request, one ordered proof scan

Call `check_derivation_ground()` and forward any failure unchanged. Only Grounded
permits proof processing. The retained proof-table offset comes from physical
layout, not a supplied pointer. Let `P` be its row count. Reject zero with code
12 `derivation` at the count field before initializing comparison or indexing.

Initialize one comparison session for the request. Reserve `P+1` work units at
the proof-count coordinate before building a balanced physical row-offset index.
Physical indexing does not admit future premises. Visit every row in order,
including unused rows, retaining only the latest session returned by each call.
Before checking a row, reserve one unit at its rule field (`record+4`). Check
left and then right global term references (`record+8`, `record+12`), then require
their inferred sorts to agree. Invalid references reject with code 12 at their
own fields; sort mismatch rejects at the right field. Sorts come from checked
symbol signatures, not proof-supplied annotations or term expansion.

Earlier admission failures precede all proof errors. The per-row reservation
precedes that row's semantic checks. A rejected or incomplete operation ends
the request; no Boolean or resumable session may become Checked.

## Rule order and failure coordinates

Rule tags and record shapes have already passed layout. A premise identity must
be in `1..current_row-1` before any lookup/projection. The physical index cannot
authorize self, forward, zero, missing, or cyclic premises. An invalid premise
rejects with code 12 at that premise-reference field.

Structural requirements call the ground comparator with both diagnostic
coordinates set to the field owning that requirement. Forward owned failures
unchanged; map Compared false to rejection code 12 at that field. Thread the
returned session only when the requirement succeeds. Do not cache proof-derived
equality as structural equality.

1. **Reflexivity:** compare the two claimed sides structurally. Difference belongs
   to the right field.
2. **Symmetry:** validate the premise at `record+16`; compare claimed left with
   premise right, then claimed right with premise left. Differences belong to
   the claimed left and right fields, respectively.
3. **Transitivity:** validate the first premise (`record+16`), then the second
   (`record+20`), before structural comparisons. Compare claimed left with first
   left, first right with second left, then claimed right with second right.
   Differences belong respectively to the claimed left, second-premise, and
   claimed right fields.
4. **Congruence:** require matching application tags, then symbols; either
   mismatch belongs to the right field. Require the stated premise count
   (`record+16`) to equal the application arity; mismatch belongs to the count.
   For each ordered premise field at `record+20+4*position`, reserve one unit
   there before validating its backward reference. Compare left argument with
   premise left, then right argument with premise right; both requirements belong
   to that premise field. An earlier argument error precedes later premise
   validation. Zero-arity congruence is valid with zero premises.
5. **Unfolding:** call `compare_unfolded_terms` using the claimed references,
   local clause ordinal at `record+16`, and their actual field coordinates.
   Preserve its code 10 clause/head failures and resource failures. Map Compared
   false to code 12 at the claimed right field. No case selection or body
   evaluation is inferred from earlier equalities.

After all rows pass, compare owner left with the last row's left, then owner
right with its right. Each requirement belongs to that last row's corresponding
field. Orientation matters. Structurally identical duplicate or witness rows
may conclude the owner root, but cannot redefine it. There is no selectable
conclusion index and no acceptance of a valid prefix followed by an invalid row.

## Result and soundness boundary

Success is tag 7 `Checked`, payload `(pair P work)`. Accessors
`derivation_proof_count(payload)` and `derivation_steps(payload)` expose those
scalars. Rejected/Incomplete retain the existing four-field failure payload.
No session is returned from Checked. A diagnostic entry publishes tag 7 plus
the two u64 little-endian values (17 bytes), or a failure tag plus its four u64
fields (33 bytes). Process zero alone is not success: the complete owned
observation must be consumed. No artifact-admitting production `main` is supplied.

Induction over checked rows establishes soundness under the formed theory:
reflexivity uses structural identity; symmetry and transitivity use only checked
earlier equations with structurally matched endpoints; congruence uses the same
typed symbol and a checked equation for every ordered argument; unfolding uses
exactly a formed defining equation under checked substitution. Formation has
already established a conservative total theory. Final structural comparisons
transfer the last checked equation to the owner root without changing it.

This proves a ground equality under the supplied theory, not that the supplied
theory is the intended Beta encoder or that its root names the selected raw
source and tape. Artifact ownership must establish those facts independently.
All five rules remain subject to actual use and mutation controls in the full
encoding certificate; these generic tests do not complete P1.

## Complete generic execution provision

Outer admission limits the exact checker request to 136,314,880 bytes (130
MiB), the least whole-MiB extent above the measured complete certificate
request of 135,451,492. It leaves room in Gamma's 137,363,456-byte (131 MiB)
evaluator frame for the pinned checker source and four-byte source length. The
canonical closure and diagnostic compositions are measured by their
manifests/pins; the whole artifact owner must retain its exact entry
composition and verify this framing requirement.
The [checking diagnostic](../../../tests/gamma/derivation-checking/source.tsv)
is 1,391 lines and 63,504 bytes; an exact-limit checker input yields
`4 + 63,504 + 136,314,880 = 136,382,488` framed bytes.

Formation retains its independent 65,536-sort and 8,388,608-work-estimate
preflights. The physical extent bounds payload words below 34,078,720 (< 2^26),
global ground terms and proof rows below 8,519,680 (< 2^24), and
— since the estimate's `4W` term still bounds theory words `W <= 2^21` — a
clause's templates below 2^20. Arity and
all child/premise counts are physically bounded before loops; there is no extra
arbitrary arity or logical-depth cutoff. Ground/template/proof references are
indexed, not repeatedly found in linked tables. Index building uses at most
24 native levels; memo insertion at most 47, the ground memo's `[0,N*N)` key
space, while a clause's template memo stays within 44 levels. Row and premise
scans and explicit
comparison/substitution continuations are tail calls. A variable's ground
comparison adds one bounded suspension, not recursion through template depth;
the finite helper nesting remains below Gamma's 256 call-context rows.

The selected diagnostic composition has 208 functions, at most 15 nested body
lists, arity 10, and 11 simultaneously active bindings per function. These are
source measurements, not input-dependent provisions. A source call-path audit
allows 57 call-context rows and 58 active function frames, including `main`.
It counts pending outer calls during argument evaluation as well as suspended
non-tail calls. The longest conservative path allows three proof/premise
suspensions, two contexts entering ground comparison, two entering memo insertion,
47 recursive memo levels, two deepest preparation contexts, and the one `main`
suspension. Other positive-depth cycles are bounded index building (24), sort
marking (16), and fixed function/clause/template layout nesting (four).
All proof scans and logical term traversals have tail-only cycles.

Thus active environments need at most `58*11 = 638` binding rows, below 65,536.
For temporary values, allow 16 entries per expression level (ten arguments plus
helper saves), 16 levels per frame (15 body lists plus return handling), and
32 fixed entries: `58*16*16+32 = 14,880`, below 524,288. The evaluator resets
argument temporaries and environments on tail-frame reuse. These conservative
source/evaluator bounds are not claimed runtime peak measurements. A different
checker entry or changed implementation must recheck its function, syntax,
binding, context, and temporary-value bounds before artifact acceptance.

All work after Grounded shares the same 67,108,864-unit counter (2^26):
proof-index setup,
row checks, congruence premises, clause/index setup, and every comparison and
substitution transition. No rule restarts or rolls back the session. Live
pending frames and completed memo insertions are bounded by consumed units;
discarded local memos and replaced frames still count toward allocation.
These provisions are adjustable engineering choices, not calculus restrictions.

Owner decision `beta-encoding-certificate-admission` selects more native
backing over a checker composition rule for the complete Beta-encoding
certificate. The five rules of [FORMAT.md](FORMAT.md#certificate-section-gce1)
stay as they are and admission grows the provisions instead, so no certificate
is admitted under a premise the checker did not derive in its own table.
**GAMMA-DERIVATION-CHECKER** rederived the bounds of this ledger at the
measured extent and selects the three coupled provisions: the 136,314,880-byte
request extent and the 137,363,456-byte evaluator frame that holds it with the
63,508-byte source framing; the 67,108,864-unit work counter, ~28% above the
44.2-52.5M projection (the measured 13.9-16.5 work/row over 3,182,484 rows);
and the 3,387,293,850-pair arena this ledger implies. The deeper memo key
spaces move the amortized constant to 50 pairs per unit. The ~129 MiB frame,
~45-52M work, and ~2.17-2.50G-pair figures were measurement targets; these are
the selected provisions. They no longer fit a static image, so the Alpha
realization supplies the extents — the `M` startup extent, and in the same
leg a buffered-output provision covering the 134,800,268-byte certificate
against 16,777,212 today. The provisions take effect there; the recorded
bounds already hold at the selected extent.

The complete cumulative pair bound is
`31,850,522 + 67,108,864*50 + 128 = 3,387,293,850` — the selected arena
provision. The first term covers formation and Grounded:
`2W+3N+32S+26` under `W <= 2^21`, `N < 8,519,680`, `S <= 65,536`. `P+1` units pay
for `3P-1` index pairs, reservation carriers, and bounded proof context setup.
Each row/premise unit pays for its own reservation and any constant coordination
carriers; structural comparison and substitution charge their own operations.
The [amortized traversal argument](COMPARISON.md#amortized-allocation-argument)
establishes 50 pairs per unit across charged operations, not per individual
transition: a memo-path insertion allocates at most 94 pairs over its one
charged unit, and the at-least-42-credit entry per pending level keeps
completed calls nonnegative. The once-per-request 128-pair allowance covers the
at-most-44-pair
unfinished-prefix deficit, session setup, final Checked or failure publication,
and constant boundary carriers, not repeated row costs. The implementation must
enumerate its allocations against this ledger.

Actual proof setup uses `3P+4` pairs: four reservation carriers, `3P-1` index
pairs, and one context pair. Row and premise coordination each add at most
eight pairs to their own unit: four reservation carriers and four for a direct
failure or congruence completion. All checks and projections otherwise use
scalars or existing carriers. Initial session setup uses seven fixed pairs;
final Checked uses two, or a terminal requirement failure uses four. Existing
comparison/substitution allocations remain charged to their own transitions.

Only the final diagnostic writes output: 17 or 33 bytes, well within the selected
Gamma output provision. An outer evaluator refusal, trap, host timeout, or short
observation is never a proof result. Full-certificate size, storage, depth, and
time measurements and an artifact-specific accepted profile remain required.
