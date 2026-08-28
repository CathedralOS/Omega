# Chapter 22: Historical Data And Component Replacement

Historical formats are ordinary immutable data plus checked conversion
machines. Live component replacement is a package protocol over boundary
runtime services. The language supplies stable identities, checked machines,
ownership, and artifact evidence; format and deployment packages select policy.

## Historical Formats Are Ordinary Data

Runtime state and durable format identity are different things. A program may
have several independent histories for one runtime type: a disk lineage, a
network protocol, a cache format, and a live-state snapshot. None is the type's
intrinsic "version."

Each published era is an immutable ordinary data declaration:

```omega
data CounterDiskV1 {
    #1 counter: i32;
}

data CounterDiskV2 {
    #1 counter: i64;
    #2 timestamp_seconds: u64;
}

data CounterDiskKnown {
    case #1 V1(value: CounterDiskV1);
    case #2 V2(value: CounterDiskV2);
}

data Counter {
    counter: AtomicI64;
    last_update: DateTime;
}
```

The durable shapes contain durable carriers. Runtime-only types such as
atomics, locks, handles, and capabilities are converted deliberately rather
than persisted by accident. Publishing a new era means declaring a new shape,
adding its envelope case, and writing its conversion. Published historical
declarations remain immutable.

Stable field/case identities, tombstones, layout policies, codecs, and
edge-specific compatibility checks are Chapter 21 layout/serialization
machinery. They operate over ordinary declarations.

## Decode Is The Open-World Seam

The versions a binary knows form a closed set; disks, peers, and older or newer
binaries form an open world. A format package must therefore choose an unknown
era policy at its decode boundary: reject, preserve opaque bytes, negotiate, or
another explicitly contracted policy. The language cannot infer that policy.

After validation, an abstract provenance domain can distinguish a decoded
envelope from arbitrary test data:

```omega
pub domain CounterDiskKnown::Decoded;
```

The format package's checked validator establishes the fact. Tests may
construct historical shapes directly; trusted consumers may require
`CounterDiskKnown::Decoded`. The security boundary is validated provenance,
not an inability to construct ordinary data.

Exhaustive matching makes adding a known era loud. A package that needs the
stronger rule "every known era has an explicit route to the runtime shape" may
express it as selected conformance to ordinary proof-machine requirements over
the envelope cases. A wildcard arm remains ordinary matching; the selected
conformance carries the stronger completeness proof.

## Migration Is Ordinary Checked Code

Format packages expose ordinary decode, upgrade, downgrade, and encode
machines. Upgrades are not required to be reversible; downgrade is an
independent, usually fallible operation.

```omega
machine counter_from_v1(old: CounterDiskV1, out: &mut Counter) {
    out.counter = AtomicI64::new(old.counter as i64);
    out.last_update = DateTime::epoch();
}
```

Mechanical codec traversal may be generated. Durable meaning is authored:
generators do not decide which runtime fields persist, how atomics snapshot,
or what semantic migration means.

The standard package exposes the nominal requirement
`FormatMigration<Lineage, Old, New>`. A format package declares an ordinary
marker type for each independent lineage and binds every conversion explicitly:

```omega
data CounterDisk {
}

machine counter_v1_to_v2(
    old: CounterDiskV1,
    out: &mut CounterDiskV2
) satisfies FormatMigration<
    CounterDisk,
    CounterDiskV1,
    CounterDiskV2
>::migrate {
    out.counter = old.counter as i64;
    out.timestamp_seconds = 0;
}
```

The `Lineage` parameter prevents two histories that reuse the same carrier
types from sharing a migration edge accidentally. The nominal exact-requirement
edge selects a checked machine; it adds no first-class version identity to either
data declaration. Reverse or fallible conversions are separate package
requirements rather than properties inferred from an upgrade.

## Live Replacement Is A Separate Protocol

Replacing a running component concerns executions, borrows, authorities,
requirement bindings, and runtime state. It is not a wire-era operation. Omega keeps
only the irreducible substrate first-class:

- normalized typed artifact and machine-contract identities;
- deterministic validation, refinement admission, and pinned requirement
  bindings;
- liveness pins for frames, borrows, callbacks, registrations, and authorities;
- admitted boundary operations for loading and atomic installation; and
- ownership, linear obligations, reach, trust receipts, and checked machines.

The replaceable unit is a selected provider realization plus the closed code,
state, resource, and version graph it owns. It is not intrinsically a package.
Calls across that closure name requirements; concrete calls remain legal
inside it. Whether a requirement is statically fused or preserved as a
replaceable edge is an owner-controlled `build.omg` selection. The exact closed
requirement application is the stable slot identity; providers, artifacts,
eras, authored strings, and ordinals are not.

Independent replacement is a checked graph property, not a promise made by the
provider. The composer closes over concrete implementation and runtime-bearing
conformance selections; every consumer fused to one of those identities joins
the same replacement cohort, transitively. It either proves the requested cut,
reports the enlarged cohort for explicit owner acceptance, or rejects it. A
prebuilt consumer may lack source spans, but its artifact manifest must still
identify the edge and declaration that enlarged the cohort.

The boundary trait is an interface, not a runtime value. Calls through an
independent slot require a routed authority carrier, `Service<R> in Bound`,
established by installation/publication. A fused selection may erase that
carrier and call directly. An independent selection resolves the current era
at every call entry. Multiplicity belongs to `Service<R>`, not to `R`.

A runtime candidate is existential behind its service contract. Its capsule
retains canonical Terminal Psi, reconstructed obligation evidence, symbolic
imports and exports, lifecycle and resource demand, and optional target-native
realizations. It may execute as verified interpreted Psi, as native code with a
checked Psi-to-target realization, or as opaque admitted native code. Local and
remote native lowering are the same trust modality; opaque native has no Omega
semantic subject and enters the disclosed executable TCB.

The initial build freezes a deployment envelope for each independent slot. A
runtime verifier may accept a future capsule only when it fits that envelope;
widening imports, authority, observation policy, target semantics, resources,
execution modality, or admissions requires a new owner-controlled composition.
The candidate never authorizes its own installation.

Quiesce, capture, upgrade, install, resume, and rollback are ordinary machines
coordinated by a package. The replacement plan declares its drain/coexistence
policy and point of no return before publication. Before that point an abort
must restore the old arrangement; afterward recovery is roll-forward or a
separately admitted reverse replacement.

Hot-swappability is not a trait or marker. The service contract publishes what
callers may observe across replacement, provider projections and selected
migration machines prove any promised continuity, and composition checks the
whole closure. When no continuity is promised, a stateful candidate may publish
first and the old era may drain afterward. A continuity-preserving transfer may
require a linear entry-freeze token or an explicitly blocking slot contract;
Prop evidence cannot stand in for that operational state.

Every live old-era activation, continuation, state object, registration,
authority, and external claim receives an explicit disposition: drain, retain
with the old era, migrate, restart/cancel under contract, redirect, or transfer
to a named receiver that acknowledges ownership. Reclamation requires the
runtime ledger's residual for the relevant lifetime cohort to be empty.

That residual is explicit accounting, not a heap-reachability query. Any value
whose meaning pins an era must be affine or linear, carry one accounted pin,
and have an unavoidable terminal disposition that releases it. Such a carrier
cannot be `[copy]`; checked explicit duplication creates another non-copy
carrier and another pin. Detached plain data and permanent gateways that resolve
through process-lifetime state pin no provider era.

Capture owns device, clock, scheduler, and other boundary reach. Replayable
upgrade code operates on owned old state and captured context, writes an
exclusive output, observes no shared or atomic racing state, and calls only
deterministic providers. An empty reach row alone is necessary but not
sufficient to establish determinism.

Semantic requirement compatibility is separate from resource admission. Each
candidate carries target-specific realized stack, work, and machine-state
demand. The runtime admits it only after provisioning the peak of every retained
era plus the candidate. A fixed resource budget enters requirement identity
only when policy intentionally forbids reprovisioning at replacement.

Entry switching is era-safe rather than instant by assumption: a racing caller
is accounted either in the closing era or the new era. Visibility of new code
gates entry; quiescence of the old era gates reclamation. Parked continuations
ordinarily pin the code and metadata of their era. "New routing is active" and
"the old era is reclaimed" are therefore separate completion states.

The generic entry ledger enforces those states directly. Publication binds the
new era's admitted entry plan and profile-accepted executable manifest, exposes
it as current, and closes the previous era to future entry. Already-entered
calls retain their old era until exact leave. A noncurrent era and its manifest
remain live until active entries and residual cohort holds are both zero, every
disposition is complete, and a fresh release receipt retires the cohort.

Candidate admission also checks its selected-provider TCB manifest. Coexisting
eras contribute the union of their known executable entries and the weakest
scope-completeness and containment evidence. Fully checked Omega eras coexist
as separate owned state trees; process-static services define their own
versioned-registration or atomic-handover semantics for shared logical names.

The generic service carrier implements that choice without selecting it. A
service contract either rejects duplicate logical keys, admits distinct exact
versions, or requires a non-replayed atomic-transfer receipt binding the old
and new registrations and eras. Atomic handover separately proves publication,
retirement of the old registration, and transfer of its obligations; successful
handover records that those obligations moved rather than disappeared.

The current live-manifest carrier admits only profile-accepted manifests,
retains the process-static baseline separately from exact component eras, and
attributes every unioned executable entry to its contributing sources. The
weakest source completeness governs the live scope, and containment is claimed
only when every contributing row supplies independent evidence. It does not
invent a combined selected-provider closure identity.

An opaque provider may retain a callback into replaceable code only through one
of two admitted routes. A process-lifetime gateway binds an exact installed
entry and dispatches through the current-era contract while permanently pinning
that gateway code. A direct reclaimable callback remains an installed external
root until the provider proves exact unregistration and the root ledger
separately proves that entry is unreachable and all executions are quiescent.
Neither proof substitutes for the other.

A reclaimed mapping may reuse its virtual address only after quiescence proves
that no live authority reaches it. Bare `addr` and sealed inert `Ptr<T>` values
cannot recreate such authority. An incomplete drain or a possible untracked
opaque holder leaves the mapping reserved and
unmapped/trapping until the containing execution domain retires. That
quarantine detects stale entry; it does not discharge outstanding obligations.
The installation carrier enforces this distinction directly: only proved
quiescence returns reusable placement. Incomplete drain or possible opaque
custody instead consumes the installed realization into a reserved trapping
range, preserves the attributed reason and exact capacity loss, and can produce
only a non-discharging stale-entry fault.

Arena-backed pools, era ledgers, coexistence policy, migration graphs, and
replacement orchestration are runtime/package concerns. Cathedral is the
planned first customer. Its implementation validates that ordinary data,
machines, traits, domains, ownership, and boundary providers express the
required protocol.

The owner-authorized build state and the runtime deployment journal are not one
atomic record. Runtime installation uses durable intent, activation, and
finalization phases, with defined restart reconciliation for an interrupted
update. The journal retains the accepting envelope, evidence, admissions, slot
publication history, and live-era state. Omega specifies the checkable record
and linear lifecycle transitions; Cathedral selects rollback, roll-forward,
cohort, scheduler, device-quiescence, and retention policy.

The canonical Rust product implementation record is deliberately non-authoritative when
decoded: it can replay exact installation, slot, envelope, and admission
identity, but cannot recreate live custody. `Prepared` publication failure
returns every input, `Activated` finalization requires the exact retained live
occurrence, and restart reconciliation leaves rollback versus roll-forward to
the selected runtime policy. Its generic durable adapter publishes one new
canonical record without replacing an existing path, synchronizes both file and
directory, and returns a non-clonable exact-byte replay receipt. A visible record
whose staging cleanup or directory synchronization is incomplete remains an
explicit recovery state; the adapter does not choose a Cathedral path,
retention rule, cohort, rollback, or roll-forward action.

## Component Implementation Work

The remaining representation work includes the artifact and mapping-cohort
manifest, durable encoding for runtime-selected entry-ledger/disposition
receipts, concrete entry-acquisition algorithms, outbound calls from old
continuations, exact liveness accounting, and optional continuation migration.
These complete the component-runtime protocol described above and are tracked
in `../../TASKS.md`; they are not unresolved language design.
