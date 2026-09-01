# Bootstrap minimization

> **Status: design and measurement work queued.** The current
> `Alpha -> Beta -> Gamma -> Delta -> Epsilon -> Omega` spine is the comparison
> baseline, not a constraint. Whole rungs, compiler edges, checker placements,
> feature surfaces, and boundary protocols are open to removal or replacement.
> Existing implementation and corpus usage price migration but do not justify
> retention. Candidate topologies and their shared experiment are defined in
> [Bootstrap chain alternatives](bootstrap_chain_alternatives.md).

## Objective

The bootstrap process constructs the first full Omega compiler. It is not a
family of public compiler services, and its intermediate languages do not need
general-purpose completeness or even permanent existence. The optimization
target is the smallest whole path that remains readable, expressive enough for
its exact required programs, locally auditable, bounded, and capable of carrying
the required checked claims.

Source bytes alone are not the objective. Moving repeated source into a more
powerful lower-rung primitive can shorten one compiler while enlarging every
later trust argument. Each proposal is evaluated over the complete retained
surface:

```text
written semantic rules
  + trusted implementation and artifact bytes
  + successor compiler complexity
  + proof obligations and checker rules
  + wire/profile/sidecar contracts
  + permanent validation and host plumbing
```

Complexity nearer Alpha carries greater cost because every later edge inherits
it. Code golf that obscures control flow, relies on clever encodings, or moves
complexity into an opaque primitive is a regression even when it saves bytes.

## Non-negotiable properties

Every candidate must preserve:

- deterministic written semantics for every retained language construct;
- exact identity for every admitted seed and generated artifact;
- a closed source envelope and bounded execution for every retained language;
- `Complete`, invalid-source, insufficient-capacity/support, and compiler-
  contradiction outcomes as semantically distinct facts;
- exact successful tape bytes and fail-closed publication;
- independent reconstruction of the proposition checked for each exact source
  and tape subject; and
- the absence of hidden host semantic stages and source-specific accelerators.

A candidate may replace an immediate-predecessor compiler with a directly
audited seed, delete a language, interpret rather than compile an early source,
or move the checker. Those choices count in full against its root audit.

The work may change syntax, compiler internals, diagnostic detail, resource
profiles, certificate formats, and the checker calculus. Existing decisions
are superseded explicitly when the whole-chain comparison demonstrates a
smaller design.

## Audit method

Work backward from the exact successor source rather than forward from the
features an intermediate language currently offers.

For each rung and each skip-rung alternative:

1. Freeze the exact successor source closure used for measurement.
2. Inventory every instruction, grammar production, value category, control
   form, built-in, failure distinction, mutable table, wire field, profile,
   proof rule, and host helper.
3. Map each retained item to the exact successor construct or checked edge
   property that needs it. Current users establish migration cost only.
4. Compare five shapes: retain it; express the need with existing machinery;
   restructure the successor to remove the need; add one narrower lower-rung
   mechanism; or delete the entire rung and connect its neighbors directly.
5. Measure source/tape size, semantic rule count, mutable state and table
   count, proof/certificate burden, checking work, and permanent tests/tools
   for every viable shape.
6. Select one design, state its deletion conditions, rebuild the affected
   artifact, and rerun the immediate edge plus all downstream consumers.

A feature survives only with a language reason and a favorable whole-chain
comparison. Familiarity, present implementation, hypothetical reuse, and the
number of corpus occurrences are not language reasons.

## Compiler outcome and profile reduction

The current early compilers use a small semantic outcome underneath a much
larger public protocol: versioned 40-byte failure frames, reason/resource
codes, coordinate spaces, request profiles, and manually synchronized TSV
projections. That shape treats a one-customer bootstrap compiler like a public
multi-client service.

The reduction must specify:

- the exact observable representation of `Complete`, invalid source,
  incomplete capacity/support, and internal contradiction;
- whether failed compilations publish anything beyond the outcome tag;
- which judgments must agree across independent compiler implementations;
- whether detailed reasons and source coordinates remain private diagnostics;
- whether Delta needs runtime profile selection at all, rather than one fixed
  Epsilon-compiler entry plus test-owned conformance adapters;
- the source/output and internal bounds that remain compiler-artifact policy;
- the separation between compiler outcomes and execution outcomes of the
  generated program; and
- the atomic migration of Gamma's persisted tape and unfinished Delta/Epsilon
  adapters.

`Reject` and `Incomplete` may not be merged: the former is a language judgment,
while the latter makes no judgment outside the compiler's capacity or accepted
bootstrap subset. Exact successful bytes remain the primary cross-
implementation observation. Detailed error taxonomy earns a stable ABI only
when a named non-test consumer requires it.

The five current TSV files are manually maintained duplicates of constants
embedded in compiler source. The target design has no tracked sidecar registry
unless a concrete external consumer requires a machine-readable contract.
Human-readable rules belong in the language/compiler contract; executable
constants belong in compiler source; gates exercise the behavior directly.

## Root checker reduction

The Alpha-owned checker has a sound architectural role: one low executable
validates derivations without trusting their producer. Its present feature set
is not presumed minimal. The audit must derive the smallest calculus and
subject-binding format needed by the canonical compiler edges.

The checker audit inventories:

- proposition and term constructors actually required by the edge theorems;
- logical, equality, induction, reduction, lemma, and transport rules;
- exact-source/tape subject binding and its byte-tree representation;
- parser, arena, context, function, lemma, fuel, and stack machinery;
- certificate size and proof-production consequences; and
- generic rules currently retained for product/Psi claims rather than the
  bootstrap compiler edges.

For each candidate, measure checker source and tape bytes, semantic rule count,
mutable tables, maximum live memory, certificate size, checking time, and the
complexity displaced into artifact-specific proposition reconstruction or
proof production. A smaller checker that makes certificates unreviewable or
requires a larger trusted proposition generator is not a reduction.

The checker remains a service beside the chain, not a language rung. It does
not search for proofs, choose obligations, or decide deployment policy. Product
proof needs do not automatically enlarge the bootstrap root; a separate
checked extension or later product checker must be compared when a rule is not
needed by a compiler edge.

## Sequencing

1. Establish the common inventory and candidate experiment before selecting a
   permanent topology.
2. Settle the minimal compiler outcome/profile contract before completing
   Delta's `DCOUT`/`ECOUT` adapters or adding further profile machinery.
3. Audit the checker against explicit candidate edge propositions before
   investing in large certificates for its current calculus.
4. Minimize from the upper exact source closures downward, but implement and
   rebuild from the lowest changed rung upward.
5. Delete superseded sidecars, rules, code, gates, and prose atomically. Git
   history, not compatibility scaffolding, owns the retired design.

## Completion criteria

The minimization program closes only when:

- every retained rung feature and checker rule has an exact successor/edge
  justification;
- selected alternatives include comparable whole-chain measurements;
- detailed bootstrap diagnostic/profile ABI exists only where a named consumer
  requires it;
- every tracked sidecar has a non-documentation machine consumer or is gone;
- the canonical artifacts reconstruct exactly under the selected smaller
  chain; and
- all source-to-tape obligations are restated against the selected semantics
  rather than inherited from the larger implementation.
