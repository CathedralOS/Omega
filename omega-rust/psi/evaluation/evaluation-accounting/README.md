# Evaluation accounting

This crate holds source-neutral resource accounts, movable storage leases and
tagged measurement records for compile-time evaluation. It has no interpreter, source admission, Build execution
or filesystem code.

`BuildEvaluationSponsor::new` records the current Terminal Psi measurement;
`new_with_measurement` fixes an explicit supported one. One account is shared
across a package closure and its retries; another storage schedule cannot
spend its ceilings. Equal work markers never mean equal work or storage units.

Before the engine runs, `EvaluationInvocationAccount::reserve_site` checks
work and live storage in one all-or-nothing step, shared total first. A refusal changes neither counter; dropping the returned lease frees
only live storage. The engine moves leases between frames and maps and shares
an owning backing's lease through `Arc`; copy-on-write reserves the
overlapping replacement before allocating. `EvaluationStorageLease::absorb`
merges reservations within one invocation without charging twice.
`truncate_cells` frees an owner's removed rows without changing its payload
reservation. Result preparation reserves copied live storage before
allocating; publication charges the cumulative result before exposing it.
That charge is not refunded on teardown or retry.

Terminal Psi measurement uses schema 8, operation/edge work schedule 1 and
logical storage schedule 1. Usage schema 7 is rejected. Terminal storage counts logical runtime rows, frame headers,
array and case payload elements, and each kept byte backing in full, once per
owner. It is not heap capacity, allocator overhead or process memory.

Usage records are telemetry: they admit nothing and are not transferable. In-process `EvaluationInvocationUsage` keeps the exact account for
issuer checks. Cache records keep the explicit measurement, work and storage
identities and reject unknown versions; an unknown measurement tag never
decodes as this account's Terminal Psi measurement.

The shared Terminal work meter can hold an invocation account and pay it
before each operation or selected edge. Its local resumable allowance is
separate from the shared invocation and total ceilings: refilling or cloning
the meter cannot enlarge them. A refused shared charge changes neither count
and stays a host error. A Build integration test checks that an exact
budget completes and one unit less is refused, across two activations with ordered binding effects.

Terminal byte backings get leases before entry allocation. Aliases share the
whole backing's reservation, copy-on-write reserves the overlapping
replacement, and boundary buffers hand the same owner back at writeback. Build
runs test exact and one-byte-short live payload budgets. Scalar and
structural runtime tables reserve each new row and overlapping copy; frame
moves and joins transfer leases; retiring rows frees their cells. The same
Build test checks exact and one-cell-short budgets across work-budget pauses.
Root activations and saved call headers each reserve one cell; saved headers
are freed on return, the root at instance close. Empty calls built from source test peak depth, not total calls.
Packed scalar-array elements hold reservations through construction, guest
copies, frame transfers and inspection of kept results. Inspection aliases
share one reservation; a mutation pays for a detached replacement first.
Frame-local descriptor tables use the same row reservation; staged arguments
and joins transfer leases instead of allocating unpaid maps. A failed return
keeps the outgoing backing and continuation, even partly installed. Global descriptor, placed and case rows, provider
records, and codec and field-transfer payloads carry their own leases.

Publishing a Terminal result charges one envelope cell, even for Unit, then
its kept payload rows and array and case elements. An immutable byte backing
counts its full length once per backing in a publication, including windows
and descriptor or reference edges. Provider allocation records and resident
records they keep count once per owning handle; provider-supplied bytes,
metadata and borrowed referents are not result payload. A returned
descriptor's identity does not permit counting storage that merely coincides
with its referent. Scalar-case inspection shares its paid payload, like
packed-array inspection. Root descriptors keep their backing until instance
close. A failed publication keeps everything outgoing; cached completion and
inspection do not charge again. Separate invocations and component
completions share cumulative ceilings, and each publication pays. These
partial counts are not complete Build budgeting; do not publish them as production Build accounting receipts. The ordinary
package-aware CLI stays on the checked Build evaluator until accounting and
host adapters are complete.

`BuildEvaluationSponsor::charge_pure_usage` pays, in one step, the checked
recorded cost of an already admitted pure result. It checks the exact engine
and schedule, refuses usage that observed the filesystem, spends cumulative
work, log and result quotas, and admits the recorded peak beside existing
reservations as a reservation freed at once. It returns current-account telemetry
without keeping the producing account. The caller must still
prove the result's identity, current admission and lack of effects; a usage
record proves none of these.
