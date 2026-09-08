# Component publication implementation

[Component publication](../../../../../wiki/spec/build/component_publication.md)
owns the required deployment contract. This crate joins independently admitted
native installation, provider/progress closure, and lifecycle custody; it is not
an executable implementation of arbitrary source `Independent` selections.

## Runtime custody and journal

[lib.rs](src/lib.rs) retains the real installed runnable and root/progress
custody through era retirement. The
[era ledger](../../../representations/effects/src/component_era_entry_ledger.rs)
binds exact entry contract/plan, profile-sealed executable manifest, and strong
installed-artifact occurrence identity. Entry remains on its chosen era across
routing changes; quiescence/retirement check active entries, retained holds,
and complete dispositions. Era publication and program-local epoch leases retain
complete candidates, not only compact coordinates. Concrete entry acquisition,
durable ledger encoding, and OS policy remain separate work.

[deployment_journal.rs](src/deployment_journal.rs) validates canonical phase
records and exact predecessor transitions. Decoding produces report/replay data,
not live authority. Activated custody retains the opaque installed-code context;
finalization rejoins it and canonical installation bytes with the ledger's
runnable occurrence. Serialized compact installed/artifact values remain reports;
the collision-resistant occurrence digest is independently retained per era.

[deployment_journal_storage.rs](src/deployment_journal_storage.rs) publishes a
new phase record through same-directory staging, file synchronization, atomic
no-clobber hard linking, staging-name removal, and directory synchronization.
It never replaces an existing destination. A post-publication cleanup/sync error
reports possibly visible publication, not rollback. The non-clonable receipt
replays exact path and canonical bytes.

Restart-to-runtime joining takes the caller's explicit rollback/roll-forward
choice and durable receipt, rejoins the selected era with the current live ledger
and runnable, and returns a non-clonable continuation. Rejection returns receipt,
choice, and ledger. Joining does not publish, retire, redirect, or choose policy.
Cathedral chooses journal location, retention, update cohort, and recovery policy.

[callback_registration.rs](src/callback_registration.rs) separates process-lifetime
gateway admission from direct registration with unregistration/quiescence.
[Quarantine](../executable-installation/src/replacement_quarantine.rs) retains
exact installed context and capacity loss instead of treating execute removal as
successful reclamation.

## Source composition boundary

[Provider planning](../../../build/provider-planning/src/plans.rs) retains mode
through selected-plan provenance but rejects Independent before publishing
checked/package-review facts until the component closure and routed installation
exist. The Fused source carrier is toolchain-owned affine `Service<R> in Bound`
with one public closed nongeneric, lifetime-free requirement. Exact typed
carrier/base/domain/requirement and full plan digest rejoin erasure authorization;
lookalikes, extra qualifications, or provenance substitution cannot erase it.

The bounded direct-parameter lane permits one owned service carrier in an
attached or free one-state Unit machine, with an authored-order primitive input
partition feeding direct Unit boundary calls. One free whole-root forwarding
edge into a direct-calling helper has a separate exact receipt. Multiple
carriers, repeated/scalar-bearing forwarding, borrowing, projections, wider
control, installed-provider scalar ABI, and runtime publication remain outside
these entrances. Multiple distinct `Service` applications additionally need a
carrier-aware Terminal domain-application identity: one domain ID bound to one
carrier cannot safely describe both `Service<A>` and `Service<B>`.

Selected attached ProgramEntry receiver fields use separate source-free
establishment rows joined to source signature/slot, receiver/attachment,
field/carrier/domain, schema, and Fused plan. Native entry settlement replays
the canonical sorted roster and exact custody; targetless/free/non-root owners
produce no such row. Zero runtime bytes cannot replace establishment evidence.

## Candidate and deployment handoff

The [candidate owner](../../artifacts/component-candidate/src/lib.rs) retains and
rederives emitter-derived internal call-graph stack demand for its canonical
object entry, comparing full native target, Terminal/entry identity, byte bound,
alignment, and contributors. It excludes external-entry adapter headroom and
proves no provision or stack lease. An object-validated full-body Linux
`exit_group(i32)` leaf may have exact zero body stack; missing physical evidence
does not justify that inference for arbitrary functions.

[Component deployment](../../../build/component-deployment/README.md) sequences
the independently supplied authority above compilation and documents remaining
production integration. Symbolic component imports/exports, lifetime cohorts,
general disposition/migration interfaces, and cross-component specialization
remain implementation work.

## Artifact-local backend assumptions

Native candidates and replayed object artifacts already exist; that does not
complete source `Independent` composition. The current
[target plan](../../../representations/target-operations/src/target_operations.rs)
names one semantic entry and its functions. The
[object publisher](../../images/image-emission/src/lib.rs) builds artifact-local
symbols, data, and relocations and resolves internal calls against that artifact's
function roster. These local machine/symbol handles are not cross-compilation
identities. General independent crossings need the specification's exported
requirement identities and exact calling, state, representation, and observation
plans, not addresses or handle coordinates borrowed from another compilation.

Component stack demand describes the candidate's internal body closure; it does
not supply crossing frames, runtime stack backing, or external-entry headroom.
Entry selection and selected provider facts remain bound to each admitted
artifact. Reusing those facts for a different artifact requires a new exact join.
Keep new whole-artifact address, frame, or provider assumptions explicit beside
their owner so they can be reconciled when widening component composition.

Direct image construction remains the backend strategy; component loading extends
the artifact/image machinery rather than requiring an external linker. Concrete
mapping, patching, deduplication, lifetime cohorts, and loader policy belong to the
consumer runtime/OS. A package-shaped initial closure restriction is only an
implementation fence: widening it must preserve accepted meanings and identities,
as required by the specification. See
[representation ownership](../../../representations/README.md) for the shared
place, annotation, and emitted-layout invariants.
