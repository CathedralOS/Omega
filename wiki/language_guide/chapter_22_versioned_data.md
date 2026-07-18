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

After validation, a sealed provenance domain can distinguish a decoded
envelope from arbitrary test data:

```omega
domain CounterDiskKnown::Decoded {
    introduction sealed;
}
```

The format package owns the evidence surface. Tests may construct historical
shapes directly; trusted consumers may require `CounterDiskKnown in Decoded`.
The security boundary is validated provenance, not an inability to construct
ordinary data.

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
dispatch slots, and runtime state. It is not a wire-era operation. Omega keeps
only the irreducible substrate first-class:

- normalized typed artifact and machine-contract identities;
- deterministic validation, refinement admission, and pinned import slots;
- liveness pins for frames, borrows, callbacks, registrations, and authorities;
- admitted boundary operations for loading and atomic installation; and
- ownership, linear obligations, effects, trust receipts, and checked machines.

Quiesce, capture, upgrade, install, resume, and rollback are ordinary machines
coordinated by a package. A linear quiescence token can ensure that every path
installs, resumes, or otherwise terminally settles the stopped component. The
point of no return must be explicit: while resume is legal, the old state must
remain recoverable; after it is consumed, the remaining path must be
infallible-or-install by contract.

Capture owns device, clock, scheduler, and other boundary reach. Replayable
upgrade code operates on owned old state and captured context, writes an
exclusive output, observes no shared or atomic racing state, and calls only
deterministic providers. An empty effects row alone is necessary but not
sufficient to establish determinism.

Arena-backed pools, quiescence tokens, coexistence policy, migration graphs,
and replacement orchestration are package concerns. Cathedral is the planned
first customer. If that implementation discovers a semantic requirement that
ordinary data, machines, traits, domains, ownership, and boundary providers
cannot express, that demonstrated requirement may justify new language
surface. Repeated boilerplate alone does not.

## Deliberately Deferred Component Work

The component/runtime design still must choose bounded coexistence and
eviction policy, outbound calls from old continuations, exact liveness-pin
accounting, artifact linking/admission mechanics, and whether later
continuation migration is worthwhile. These are component-runtime questions,
not reasons to restore `Versioned<T>`, `.prev` type paths, or `replace` syntax.
