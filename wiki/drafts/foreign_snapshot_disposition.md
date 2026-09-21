# Foreign snapshot disposition — design draft

Status: draft. Audience: owners of EPOCH-RESOURCE-SNAPSHOTS,
NEW-FRAB-FOREIGN-STORAGE-SPEC-ALIGNMENT, and the provider-attachment
legs that join snapshots to installed foreign occurrences.

This note fixes the vocabulary and disposition rules for *foreign
snapshots*: private, semantically qualified copies of foreign-held state
taken under an explicit contract, as distinct from retained custody
loans and from identity-preserving backing transfers. It binds the
spec's outbound-custody row — "a permitted semantic snapshot uses
private stable backing" ([foreign storage](../../spec/build/foreign_storage.md))
— to the epoch/aggregate capacity machinery already landed in
`omega-rust/omega/backend/runtime/external-roots/src/program_local`.

## What a foreign snapshot is

A foreign snapshot is a `Content<A>`-qualified copy taken at a recorded
revision of an admitted foreign extent, stored on private stable
backing owned by the Omega side. Per the foreign-storage contract:

- It requires an explicit semantic contract permitting independent
  copying. Identity preservation and unchecked write-back are both
  out of scope — a snapshot is not a view and not a proxy.
- It does not change the separately compiled public result type; the
  snapshot's type identity is the qualified content, not a foreign
  extent descriptor.
- Every retained native pointer slot still needs exact stable-root,
  range, access, lifetime, and revision/lease provenance; a snapshot
  admits only by an established route that fixes all of them.

## Disposition taxonomy

Each snapshot occurrence carries exactly one live disposition at a
time; transitions are total and observed:

| Disposition | Rule |
| --- | --- |
| **Retained** | Snapshot bytes count as persistent demand for the live occurrence. Live-occurrence capacity is finite: success transfers the exact capacity into the registration; rejection returns it unchanged. |
| **Consumed into transfer** | A snapshot may be consumed to produce an ordinary result (e.g., a `PendingWrite` payload) when the consumed-input-to-result mapping is unambiguous; ambiguous mappings need an ordinary postcondition establishing exact correspondence, else reject. |
| **Partial release** | Splits an exact separated subextent from the remainder still in flight; the residual keeps the same epoch lease. |
| **Released** | Unregister returns the same occurrence capacity; the occurrence record discharges its revision lease and private backing together. |
| **Invalidated** | Foreign writes over a writable extent invalidate semantic facts over exactly that extent; the snapshot row is marked stale against the live reconstruction rather than silently continuing. |

## Epoch and aggregate capacity

The landed machinery supplies the reconstruction discipline the
disposition rules need:

- `ProgramLocalExtentRegistry::materialize_aggregate` /
  `materialize_aggregate_over_receiver` reconstruct aggregate capacity
  over installed backing and reject a presented capacity that is stale
  or substituted for the live reconstructed membership ("presented
  program-local aggregate capacity is stale or substituted…").
- `retire_aggregate` atomically completes the complete live membership
  of one reconstructed cohort; it replays each member's live epoch
  lease, so a snapshot row joined to a dead epoch cannot retire
  silently.
- A snapshot disposition therefore names its lifecycle cohort at
  creation and re-validates that cohort at every join: materialize,
  partial release, retire. Stale or substituted capacity is a
  rejection, not a warning.

## Invalidation model

- Concurrent foreign writes invalidate facts over exactly the writable
  extent they cover; untouched-complement facts survive (mirrors the
  write-only outcome-footprint rule).
- A provider-created handle or pending operation pins the exact era
  whose state gives it meaning; snapshot rows derived from that state
  carry the same era pin, and reclamation waits for the era's
  discharge path (callable-during-teardown, quiescence, unload).
- Cancellation releases nothing until terminal acknowledgement; a
  snapshot joined to an in-flight operation stays Retained until the
  boundary completion receipt commits.

## Rejections (must hold)

- Snapshot presented against foreign provenance with unknown
  stable-root/range/revision — reject unless an admitted provider
  route establishes it.
- Recursive or dynamically sized foreign pointer graphs without a
  covering arena/extent root — reject (the compiler does not traverse
  runtime pointers to discover custody).
- Write-back of snapshot bytes into the foreign extent through an
  ordinary store path — reject; exclusive foreign mutation moves
  storage into the protocol and returns it with explicit
  qualifications, it is not a snapshot round-trip.
- Duplicate fresh-supply or double-release of one occurrence —
  reject via the epoch-lease replay.

## Open legs (hand-off targets)

1. **EPOCH-RESOURCE-SNAPSHOTS** — the program-local aggregate cohort
   that names a snapshot epoch across the external-roots ledger;
   disposition transitions above ride its reconstruction rules.
2. **NEW-FRAB-FOREIGN-STORAGE-SPEC-ALIGNMENT** — the spec-side
   alignment of the outbound-custody table rows this note cites.
3. Provider-planning/native-settlement join to the installed
   occurrence (COMPONENT-SUBSTRATE frontier): the disposition record
   is only real once issuance attaches it to an installed foreign
   root with keepalive and reclamation authority.

## Non-goals

- No general permanent-custodian spelling; process-lifetime authority
  still moves only into an already-established static root.
- No identity-preserving snapshot semantics; that is a retained-custody
  loan, a different row.
- No foreign-only extent algebra; qualified content uses the
  owner-unique `Content<A>` projection.

## Implementation route (companion design record, verified b90ac7155d47)

The taxonomy above fixes what a snapshot *is*; this section fixes how an
authored boundary *names* it, end to end through the pipeline. Surveyed
state at `b90ac7155d47` (linux x86-64):

- **Checker.** `typed-trees-to-checked-trees/src/checks/content/
  retained_custody.rs` derives custody from the authored contract: one
  consumed owned source records moved retention;
  `lifetime_bound_borrow_custody` emits `RetainedBorrowCustodyFact` for
  exactly one whole direct shared reference bound to an explicit callable
  lifetime carried by a linear result. Every other shape rejects
  ("requires a consumed owned input" / "ambiguous compatible consumed
  inputs"). No snapshot derivation exists.
- **Terminal.** `terminal_module/boundary/declarations.rs`:
  `BoundaryContentGuarantee::{Conservation, RetainedBorrow}` — no Snapshot
  row. `RetainedBorrow` rows stay non-executable catalog entries selected
  only through `BoundaryCall`.
- **Verifier.** `terminal-verifier` replays `RetainedBorrow` rows in
  `validation/content.rs` and admits calls in `unit_operation/
  boundary_calls.rs` (`validate_retained_borrow_call`: whole-place
  `SharedBorrow` argument at the row's source position, exact retained
  occurrence as result, loan claims re-homed on it). No snapshot
  admission exists.
- **Native.** `external-roots/src/program_local/program_local_extents/
  retained_foreign_arguments.rs` already implements the rung:
  `retain_foreign_argument_snapshot(source, request, backing)` enforces
  private stable backing concretely — distinct `program_local_origin` (no
  aliasing the argument), exact length match, `request.rights` ⊆ backing
  authority, provenance/era pinned from the backing extent, release
  returning the backing `Extent`. The selection entry
  `retain_foreign_argument_under_custody` binds only `RetainedBorrow`
  rows: "moved and snapshot dispositions have no authored Terminal row
  yet and cannot be selected here" — the disposition is the authored
  row's choice, not the caller's method.

### Authored spelling

The permission is boundary-scoped — *this callable* may copy *this
argument*; the domain does not globally license copying. Follow the
existing `requires <place> in <domain>` membership clause with an
attached permission rather than a new top-level contract kind:

```text
boundary trait Snapshotter {
    machine hold(buffer: &Buffer in Buffer::Owned)
    requires buffer in Buffer::Owned permits snapshot within SnapshotDemand;
}
```

where `SnapshotDemand` is a `domain` quantifying over a countable unit —
the per-occurrence capacity algebra — reusing
`ContentProjectionExpression` (the carrier
`program_local_root_introductions` schemas already use to publish
per-occurrence capacity). A `permits snapshot` clause without the
`within` demand bound rejects: persistent demand per live occurrence has
no meaning unbounded. `permits write-back` and `permits identity` are
never spellings; both reject at parse/check.

Alternatives recorded and declined: a domain-declaration flag (`domain
Buffer::Owned permits snapshot`) licenses copying for every caller and
loses the per-boundary demand bound; an `ensures` correspondence asserts
a result equality a snapshot deliberately does not produce.

### Checked form

`RetainedSnapshotCustodyFact` beside `RetainedBorrowCustodyFact`
(checked_trees `facts/content.rs`):

- `source`: `ContentStructuralPlace` — whole direct non-`self`
  parameter, `Entry` version, empty segments (the bounded borrow-rung
  shape; nested/indirect sources stay directed rejections).
- `source_access`: `Shared` only — the copy reads the argument; a
  mutable or write-only source is a directed rejection.
- `source_projection` / `demand`: the exact content projection copied
  and the per-occurrence `ContentProjectionExpression` bound.
- `callable` identity and the permission's contract ordinal for replay.

Derivation in `check_callable`: when the clause is present, the borrowed
source satisfying it exits the "requires a consumed owned input"
rejection path — the snapshot consumes no owned custody. Its absence is
unchanged: borrowed-only sources still reject.

### Terminal row

`BoundaryContentGuarantee::RetainedSnapshot(RetainedSnapshotCustody)` —
a third non-executable catalog row carrying `source` place, access,
`demand`, and `source_projection`. The verifier replays the row against
the authored signature exactly as `validate_retained_borrow_custody`
does, plus the snapshot-specific laws:

- the `BoundaryCall` presents the source argument `SharedBorrow` at the
  row's position;
- the operation's result correspondence names the source place in no
  equation — identity preservation and write-back are both rejected by
  the absence of any result binding to the source, not by a rule looking
  for them;
- the retention receipt binds the *established private backing*
  occurrence and consumes the published per-occurrence demand against
  the caller's live-occurrence capacity — success transfers the exact
  capacity into the retention, rejection returns it unchanged.

### Native join

`retain_foreign_argument_under_custody` grows the `RetainedSnapshot`
arm: allocate the private `Extent` from an established program-local
root published through the requirement's
`program_local_root_introductions` schema (the existing
per-occurrence-capacity route), then call the already-implemented
`retain_foreign_argument_snapshot` — which keeps enforcing distinct
origin, exact length, rights ⊆ authority, and era/provenance pinning. No
registry change is required for this leg.

### Checker-level rejection matrix

1. Snapshot attempted with no `permits snapshot` clause — permissionless
   retention.
2. Mutable, write-only, nested, or `self` source — directed rejections
   beside the borrow rung's.
3. `permits snapshot` without the `within` demand bound — unbounded
   persistent demand.
4. Retention receipt naming aliased backing (shared
   `program_local_origin` with the argument) — refused at realization.
5. Any result correspondence binding the source place — write-back
   refused by construction; an authored clause asserting one is a check
   error.
6. Live-occurrence capacity exhausted — rejection returns the caller's
   capacity unchanged.

### Fence map (advisory at b90ac7155d47 — re-check `tools/claims.py
status` at implementation time)

- Checker + facts: `typed-trees-to-checked-trees` checks/content, under
  the wave's rotating issuance/custody claims.
- Terminal row + verifier: `terminal-psi` declarations and
  `terminal-verifier` content/boundary-calls.
- Lowering: `checked-trees-to-lowered-psi/src/retention/` sibling to the
  borrow row.
- Native: `external-roots/src/program_local` under
  EPOCH-RESOURCE-SNAPSHOTS (~11:32Z); the provider-planning/
  native-settlement join stays with PROVIDER-ATTACHMENT-MACHINE-PLAN
  (~09:49Z).
