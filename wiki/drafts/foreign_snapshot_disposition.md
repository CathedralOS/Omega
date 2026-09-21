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
