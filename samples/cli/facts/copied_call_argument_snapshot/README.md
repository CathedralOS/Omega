# Copied call operands keep their evaluation epoch

A copied first argument equal to 7 stays 7 when the second argument overwrites its former home with 9.

Parent: **STATE-LOCAL-VALUE-FRONTIER**; prerequisite evidence for **CHECKED-PACKAGE-REUSE**. The first argument completes as 7, then `overwrite` stores 9 through the original home. The caller must establish `receive`'s explicit `requires first == 7` from that completed copy. `receive` transports its declared premise through `seven`'s entry contract, establishes its result as 7, and permits the caller's exact subtraction `observed - 7`. A runtime guard does not establish that fact.

The nine places are [`local`](local/main.omg), [`field`](field/main.omg), [`parameter`](parameter/main.omg), [`call_result`](call_result/main.omg), [`match_join`](match_join/main.omg), [`const_name`](const_name/main.omg), [`other_state`](other_state/main.omg), [`other_machine`](other_machine/main.omg), [`target_body`](target_body/main.omg). Every place has an `invalid/main.omg` twin changing only the initial first operand from 7 to 9. It must refuse the same 7 obligation. Matching pins live under `tests/omega/{pass,fail}/facts/copied_call_argument_snapshot_through_*`.

The original baseline `55898d082fc8` reread the first operand's home after later operands ran. The producer already retained exact `FlowOperandFact` completion constraints, and call admission must consume that capture; live views instead consult their exact invocation-time referents. The by-value entry-contract correction in `9ac3908938b6` also requires each `receive` body to declare its premise: a caller's literal cannot justify an unrestricted callee body. Positive and false-input twins share that same contract; only the initial first operand differs. A conservative unknown-input fallback must never re-establish a fact from a predecessor subset.

## Copied descriptor transport

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The same nine places have `<place>/descriptor/main.omg` witnesses and
`descriptor/invalid/main.omg` twins. The positives carry
`requires selected.target.value == 7` through the selected place into `seven`
and then the caller's exact subtraction. Twins change only the first backing
value from 7 to 9. Their pins are
`tests/omega/{pass,fail}/facts/copied_call_argument_snapshot_descriptor_through_*`.

A copied descriptor releases its former slot after evaluation while retaining
its selected backing until the enclosing invocation. Staged exclusions use the
actual captured descriptors and their immutable storage cuts, including a
backing installed by an earlier nested call. Mutating or consuming that backing
must reject even after another call replaces the original slot. Borrowed
arguments keep their source-home exclusion. Capture-time case evidence excludes
inactive payloads; current tags cannot erase an older active copy's loan.

Nested-reference contracts retain explicit referent crossings. An indirect read
never becomes an inline canonical field or a new proof assumption. Checked
source contracts remain the authority for source admission; this change does
not restore a second proof replay in ordinary compilation.

Related ownership controls under `tests/omega/{pass,fail}/borrows` pin retained
intermediate backing, consumed backing, borrowed source homes, independent input
permissions, and active/empty case snapshots. Explicit image provenance differs
from opaque static lifetime debt. An opaque-input call-result control also runs
through native and Terminal execution; it retains the exact supplier without
inventing an address. The image-backed descriptor control now also executes in both engines;
its former sequence-cleanup refusal is retained in the baseline evidence.

Host acceptance is `omega run --both <place>/descriptor/main.omg` with native
and Terminal exit 0, plus each twin rejecting `receive`'s 7 obligation.
Windows native execution is owner-deferred to a Windows host. Linux native
execution is unverified; declaring four hosted entries does not execute them.

## Copied byte-view descriptor transport

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The nine `<place>/byte_view/main.omg` witnesses carry the pointer-and-length
descriptor in a shared byte-view field. The unchanged consumer `seven` and exact
subtraction require its captured extent of 7; `byte_view/invalid/main.omg` changes
only the first extent to 9. This is a distinct descriptor representation, not
another count or integer width. Matching positive and reject pins live under
`tests/omega/{pass,fail}/facts/copied_byte_view_argument_snapshot_through_*`.

Fresh optimized baseline `a92f16431042e8a54bc3f6d90a87f8602fa3aa18` accepted the
positive source obligations, but eight places refused normal-exit cleanup and
`const_name` refused its inline literal range. All nine twins rejected `receive`'s
7 obligation. The smaller image-backed descriptor control also refused cleanup.
A non-consuming observation must preserve custody: a move candidate roster does
not establish a transfer. The repair uses the selected source multiplicity before
retiring a claim or transferring storage, while retaining completion cuts.

Range formation retains its complete source argument, logical root, selected
path and requested access. A computed collection is evaluated once before its
endpoints; its new descriptor formation keeps the actual child backing and
access obligations. Record-field byte types and literal descriptors enter the
canonical catalog through their owning producers. An implicit nominal region
retains its exact supplying declaration rather than repeatedly creating an
unresolved result type. Native call-result custody counts owned descriptor slots;
reference-result provenance is checked separately. State transfers retain local
backing from complete shared-leaf lifetime profiles even after a forming call
access ends; shared type spelling alone cannot establish external backing.

Related `pass/borrows/runtime_*_window_*` controls exercise computed shared
record projections with actual byte reads, aliased record prefixes, collection
and endpoint evaluation order, and mutable-parent restoration after named and
returned windows. `runtime_returned_literal_window_exit` checks a helper-created
image range; `runtime_mixed_byte_view_reference_result_exit` checks an aggregate
with both an owned byte descriptor and a returned record reference.
`runtime_transitive_mixed_result_backing_exit` retains the intermediate record
and its own borrowed backing across the same state transfers. The mutable
shared-field twin rejects an attempted access escalation, and the mixed-result
backing-write control rejects mutation while its returned reference remains live. Nominal mismatch
controls must reject through ordinary call checking, without inventing a region
binding merely to settle inference.

Host acceptance is all nine byte-view positives with native and Terminal exit 0,
and all nine twins rejecting the same `receive` obligation. Windows native
execution is owner-deferred; Linux execution is unverified. Baseline and current
producer-bound observations remain distinct in the qualification record.

## Requirements and evidence

- Establishment: initial 7 and the completed operand capture establish the fact; checked call/arrival contracts carry it, never the later 9 or an unproved goal.
- Identity and lifetime: the copied first operand is detached; `overwrite` changes its original home, not that copy. Descriptor copies retain their exact backing permissions independently of that home; byte-field observations retain custody of their containing record.
- Composition: all nine places consume the same snapshot through `seven`, checked result equality and exact subtraction. Twins change the source to 9, not a caller guard.
- Soundness: ordinary exclusive access authorizes the overwrite; operand facts grant no loan or disposal authority. Unknown/missing captures participate conservatively.
- Checked-to-Terminal Psi and native realization: every positive binds all four hosted targets and declares exit 0. Acceptance requires observed native exit 0 and interpreter exit 0 through `omega run --both`; flags alone establish no other-host execution.
- Provider/layout/ABI: the descriptor rows pass shared-reference and byte-view records through ordinary field layout and call/return placements, including its call-result place. It does not establish every aggregate ABI or target lane.
- Instruction/frame/encoding/image realization: host execution exercises ordinary indirect loads, argument staging, frame homes, calls and direct Mach-O emission. No claim is made for unexecuted targets, concurrency, or unrelated instruction paths.
- Remaining scope: these witnesses establish scalar, shared-reference and shared byte-view operand snapshots. A separate multi-view-field stored-call destination/backing relation remains incomplete; this row does not establish that broader effect summary. The rest of both parent outcomes, including general computed/cyclic composition, remains open.

Actual baseline diagnostics, producer/source/oracle identities, engine observations and reject reasons belong in the owning qualification/claim records. No unexecuted target is claimed.
