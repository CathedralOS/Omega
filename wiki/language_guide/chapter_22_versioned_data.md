# Chapter 22: Evolution, Migration, And Replacement

Omega does not have a first-class “versioned data” type. Evolution is a pattern
composed from ordinary data, sums, machines, traits, domains, contracts, and
toolchain artifacts. Live component replacement additionally needs loader and
runtime services, but it does not require a `replace` statement.

The old chapter title and file path remain temporarily so existing links reach
the ruling that supersedes them.

## Three Different Problems

| Problem | Stable anchor | Omega representation |
|---|---|---|
| Persisted or wire history | Schema and codec-plan identities | Named protocol shapes, identity metadata, codecs, ordinary sums |
| Runtime state transformation | Input/output types and machine contract | Ordinary migration machine, optionally satisfying a library trait |
| Live component replacement | Component artifact and normalized machine-contract identities | Runtime/provider operations coordinated by ordinary machines |

The problems may share transformation code. They do not share one universal
version number or one compiler-owned container.

## Historical Shapes Are Ordinary Data

Published formats remain nameable as independent declarations:

```omega
data CounterDiskV1 {
    1: counter: i32;
}

data CounterDiskV2 {
    1: counter: i32;
    2: timestamp_millis: i64;
}

data CounterRuntime {
    counter: AtomicI32;
    timestamp: DateTime;
}
```

These are three ordinary types. `CounterDiskV1` is not a magical historical
view of `CounterRuntime`, and editing the runtime type cannot mutate either
published disk schema.

When one decode operation recognizes several formats, its result is an
ordinary sum:

```omega
data DecodedCounter {
    case Invalid;
    case DiskV1(value: CounterDiskV1);
    case DiskV2(value: CounterDiskV2);
}
```

Ordinary exhaustive dispatch supplies the coverage guarantee:

```omega
transition decoded {
    DecodedCounter::DiskV1 { value } -> import_v1(value, out);
    DecodedCounter::DiskV2 { value } -> import_v2(value, out);
    DecodedCounter::Invalid -> reject();
}
```

A wildcard remains an explicit choice to accept future source cases without
case-specific behavior. A lineage package that requires one explicit route per
known era can validate that structural rule in its normalized route plan; it
must not pretend ordinary match exhaustiveness forbids `_` globally.

## Migration Is Ordinary Checked Behavior

A project may use direct machines:

```omega
machine import_v1(old: CounterDiskV1, out: &mut CounterRuntime)
    ensures out in CounterRuntime::Valid
{
    out.counter = AtomicI32::new(old.counter);
    out.timestamp = DateTime::zero();
}
```

or share a library trait:

```omega
trait Upgradable<Old, New, Context = Nothing> {
    machine upgrade(old: Old, context: Context, out: &mut New)
        ensures out in New::Valid;
}
```

`Upgradable` is an ordinary trait pattern, not a privileged compiler trait.
Projects may instead choose fallible migrations, direct-to-current routes,
stepwise chains, reversible transforms, lossy downgrades, or negotiated
protocols. Those policies are genuinely different and should not be fused into
one language-defined history model.

Migration work that observes clocks, devices, files, or the network is
captured before a replayable transformation when replay matters:

```text
effectful capture -> owned context -> checked transformation
```

An empty effects row alone does not prove determinism. Replayability also
requires owned inputs, exclusive output, no shared or atomic observation, and
deterministic callee contracts.

## Provenance Is Separate From Constructibility

Historical values can be useful to construct directly in migration tests. A
trusted load path should nevertheless distinguish decoded/validated input from
fabricated values.

That distinction uses the ordinary domain model: a package may expose an
abstract predicate established only by its decoder's postcondition. Consumers
can match and transform the public shapes while security-sensitive machines
require the provenance domain. Private payload types are not needed merely to
protect the boundary.

## Histories Belong To Packages

One runtime type may have independent disk, network, cache, save-game, and
component-state histories. A package can encapsulate each behind `load`,
`save`, `decode`, `encode`, `upgrade`, and `downgrade` machines.

The language/toolchain supplies reusable enforcement hooks:

- stable field identities and tombstones;
- deterministic normalized schema and codec-plan identities;
- publish-time predecessor comparison;
- ordinary sum exhaustiveness;
- contract and law checking for migrations/codecs;
- domain evidence and introduction authority; and
- compatibility/refinement reports.

The package supplies policy: framing, unknown-era behavior, route topology,
rollback guarantees, and release cadence. A repeated missing mechanism found
across several serious packages may later earn promotion. Forecasted
convenience does not.

## Historical Shape Is Not Historical Behavior

A machine written today over `CounterDiskV1` is current code that understands
an old shape. It is not evidence of what the old binary did. Historical
behavior exists only in retained component artifacts or an independently
specified contract.

This distinction matters for replay, audits, rollback, and live coexistence:
source beside an old data declaration does not reconstruct an old executable.

## Live Replacement Is An Orchestration Protocol

Replacing a running component requires a phase protocol such as:

```text
admit -> quiesce -> capture/prepare -> commit -> resume or retire
```

The protocol should be implemented by ordinary machines over explicit
capabilities and linear phase tokens once the generic and multiplicity
substrates can express it. Omega does not reserve `replace`, `quiesce`,
`capture`, `upgrade`, or `install` as replacement grammar.

The point of no return is a safety law, not a syntax choice:

- before commit, every failure path must leave old state meaningfully
  resumable;
- preparation either borrows old state or retains enough ownership to restore
  it;
- commit consumes the old/new phase tokens only when installation can become
  atomic; and
- a destructive, non-rollbackable handoff must be a separately declared
  protocol with all fallible work completed first.

Linear consumption prevents silently abandoning a quiesced component. It does
not, by itself, prove that a nominal `resume` still has the state required to
resume; the phase contracts must prove recoverability.

## What The Runtime Still Must Provide

Dissolving replacement syntax does not dissolve irreducible runtime services.
A loadable-component provider may still need to own:

- artifact loading and typed identity verification;
- normalized import slots and deterministic provider admission;
- atomic dispatch installation;
- liveness pins for frames, callbacks, borrows, capabilities, and interrupts;
- per-version activation storage and bounded coexistence;
- retirement, cancellation, revocation, and eviction operations; and
- reports explaining retained versions and trust receipts.

Those are boundary/runtime operations with contracts, not keywords. Cathedral
is the only planned first consumer. It will prototype the orchestration and
force the later decision between Cathedral-local code, a target-neutral Omega
library, and any truly irreducible language/runtime primitive.

## Component Coexistence

The leading runtime direction permits bounded old/new coexistence when draining
is impractical. Existing continuations remain pinned to the code and frame
layout that created them; new dispatch uses an admitted current provider.

Import slots pin normalized requirement contracts rather than entire old
worlds. A provider can advance only by deterministic refinement admission.
Still-open component questions include outbound calls from old continuations,
version-memory budgets and eviction, exact artifact/linking mechanics, and the
future boundary between coexistence and continuation migration.

These are component-artifact/runtime questions. They do not justify attaching
era state to every ordinary value.

## Typed Identities, Not One Version Number

The toolchain may reuse deterministic normalization and hashing infrastructure,
but identities remain typed:

- lineage or schema identity;
- codec/wire-grammar identity;
- normalized machine-contract identity;
- component-artifact identity; and
- provider identity.

Two structurally equal schemas can mean different things; compatible schemas
are normally not identical; and a compatible provider is not the same artifact
as its predecessor. Explicit compatibility/refinement certificates connect
these identities.

## Retired Language Machinery

The semantic model retires:

- version blocks embedded in `data`;
- `Type::vN` and `Type.prev` historical paths;
- compiler-synthesized `Versioned<T>` and `.era`;
- special version-match arms and exhaustiveness;
- compiler-owned migration-chain discovery; and
- the `replace ... quiesce ...` DSL.

The current compiler and canary corpus still contain some of this machinery.
`TASKS.md` owns its deliberate removal and conversion of useful tests into
ordinary sums, patterns, machines, codec policies, and component-provider
tests.

## Working Rules

- Versioning is not first-class in the Omega type system.
- Protocol identity metadata is the one special evolution surface, owned by
  chapter 21 and consumed by layout/serialization policies.
- Breaking formats use explicit named shapes and ordinary sums.
- Migration is ordinary checked machine code; traits organize it when useful.
- Live replacement is a package/runtime protocol over general Omega
  mechanisms, with Cathedral as the first proving customer.
- The compiler normalizes identities, checks contracts, and reports
  compatibility; it does not infer durable meaning or deployment policy.
