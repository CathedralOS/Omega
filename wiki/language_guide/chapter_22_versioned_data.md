# Chapter 22: Historical Data And Component Replacement

Omega has no first-class versioned-data container and no replacement DSL.
Historical formats are ordinary data plus checked conversion machines. Live
component replacement is a library protocol over boundary runtime services.
The language supplies the safety substrate; packages choose policy.

## Historical Formats Are Ordinary Data

Runtime state and durable format identity are different things. A program may
have several independent histories for one runtime type: a disk lineage, a
network protocol, a cache format, and a live-state snapshot. None is the type's
intrinsic "version."

Each published era is an immutable ordinary data declaration:

```omega
data CounterDiskV1 {
    counter: i32;
}

data CounterDiskV2 {
    counter: i64;
    timestamp_seconds: u64;
}

data CounterDiskKnown {
    case V1(value: CounterDiskV1);
    case V2(value: CounterDiskV2);
}

data Counter {
    counter: AtomicI64;
    last_update: DateTime;
}
```

The durable shapes contain durable carriers. Runtime-only types such as
atomics, locks, handles, and capabilities are converted deliberately rather
than persisted by accident. Publishing a new era means declaring a new shape,
adding its envelope case, and writing its conversion. Old declarations are not
edited in place.

Stable field/case identities, tombstones, layout policies, codecs, and
publish-time predecessor comparisons are Chapter 21 layout/serialization
machinery. They are metadata over ordinary declarations, not a versioning type
system.

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
`CounterDiskKnown in Decoded`. The security boundary is validated provenance,
not an inability to construct ordinary data.

Exhaustive matching makes adding a known era loud. A package that needs the
stronger rule "every known era has an explicit route to the runtime shape" may
express it as trait-law conformance over the envelope cases. A wildcard arm is
ordinary matching and does not prove that stronger law.

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

## Live Replacement Is A Separate Protocol

Replacing a running component concerns executions, borrows, authorities,
requirement bindings, and runtime state. It is not a wire-era operation. Omega keeps
only the irreducible substrate first-class:

- normalized typed artifact and machine-contract identities;
- deterministic validation, refinement admission, and pinned requirement
  bindings;
- liveness pins for frames, borrows, callbacks, registrations, and authorities;
- admitted boundary operations for loading and atomic installation; and
- ownership, linear obligations, effects, trust receipts, and checked machines.

The replaceable unit is a selected provider realization plus the closed code,
state, resource, and version graph it owns. It is not intrinsically a package.
Calls across that closure name requirements; concrete calls remain legal
inside it. Whether a requirement is statically fused or preserved as a
replaceable edge is a build/deployment choice.

Quiesce, capture, upgrade, install, resume, and rollback are ordinary machines
coordinated by a package. The replacement plan declares its drain/coexistence
policy and point of no return before publication. Before that point an abort
must restore the old arrangement; afterward recovery is roll-forward or a
separately admitted reverse replacement.

Every live old-era activation, continuation, state object, registration,
authority, and external claim receives an explicit disposition: drain, retain
with the old era, migrate, restart/cancel under contract, redirect, or transfer
to a named receiver that acknowledges ownership. Reclamation requires the
runtime ledger's residual for the relevant lifetime cohort to be empty.

Capture owns device, clock, scheduler, and other boundary reach. Replayable
upgrade code operates on owned old state and captured context, writes an
exclusive output, observes no shared or atomic racing state, and calls only
deterministic providers. An empty effects row alone is necessary but not
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

Arena-backed pools, era ledgers, coexistence policy, migration graphs, and
replacement orchestration are runtime/package concerns. Cathedral is the
planned first customer. If that implementation discovers a semantic requirement that
ordinary data, machines, traits, domains, ownership, and boundary providers
cannot express, that demonstrated requirement may justify new language
surface. Repeated boilerplate alone does not.

## Deliberately Deferred Component Work

The remaining representation work includes the artifact and mapping-cohort
manifest, entry-acquisition algorithm, era-ledger and disposition receipts,
bounded live-era policy, outbound calls from old continuations, exact
liveness accounting, and optional continuation migration. These are
component-runtime questions, not reasons to restore `Versioned<T>`, `.prev`
type paths, or `replace` syntax.
