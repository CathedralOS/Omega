# Build execution and generated source

[Build declarations](declarations.md) define project discovery and the
activation-scoped build facets. Build execution is effectful, admitted work;
it is not the hermetic evaluator used for constants and proofs.

## Admission before execution

The order is:

1. Project the root role, both dependency scopes, and output mode from retained source.
2. Resolve the purpose-specific immutable closures and capture inputs under resolver/sponsor custody.
3. Check the host build entry and helpers; freeze the authored product-selection frontier.
4. Admit the complete build contract, then execute its prepared projection.
5. Incorporate generated source and finish all requested product checks.
6. Check required-output completion and commit one immutable result set.

[Scoped execution](scoped_execution.md) defines the execution-profile/product-target
distinction, deterministic snapshot and staging protocol, and linear output
lifecycle. No current activation can inspect its own final component or select
its generated entries; those customers require separately staged compilation.

Before step 4, pre-resolution evaluation, target filtering, symbol/type
resolution, semantic prechecks, and exact dynamic-call binding are complete.
The admitted checkpoint binds the base typed program, prepared specialized
build projection, operational/reach plans, source snapshot, and authority verdict.
Execution and replay use that exact admitted entry, not a later name lookup.

Reach, authority roots, retained storage, resource bounds, failure, and
termination must fit the executor's policy. A blocking operation must expose
the progress/failure premise needed to satisfy the build entry's termination
contract. Service reach need not be empty, but no host service is ambient.

Resolver authority is separate from build authority. Retrieval validates
requested lineage, revision resolution, object/content identity, archive path
containment, expansion limits, and immutable destination bytes. It does not
attest the ambient transport or executor. Dependency code receives neither
resolver credentials nor the root's filesystem, process, secret, or acceptance
authority. Each dependency build gets its own admitted package scope. Runtime
provider selection is not a route around the compiler-owned build facets;
additional host effects require explicit protocol operations and policy.

## Generated-source boundary

Path resolution, staged output writing, and source publication are distinct
operations. `BuildOutput::include_source` publishes exact retained UTF-8 bytes
only after matching sponsored staged-tree custody. Source consumption binds
those bytes under compiler-owned `.omega/generated/...` logical paths; it does
not reread a mutable output path.

Own generated source is a later resolution stratum:

- It may refer to admitted authored declarations, and generated units may
  resolve together.
- Authored occurrences, including every transitive build helper, cannot select
  generated declarations. Later overloads, conformances, and providers cannot
  alter the already-admitted build meaning.
- Final checking extends the retained checkpoint. It does not rediscover
  dependencies, rerun build execution, or rebuild the authored frontend.

Each generated unit uses ordinary pre-resolution evaluation and its matching
one-shot post-typing continuation. Target filtering validates the complete
base-plus-extension declaration roster while modifying only the extension.
Duplicate selected declarations and missing selected-target members reject;
unselected generated siblings remain inert.

The completed candidate must preserve the admitted base exactly: root graphs,
symbols, names, paths, authored selections, semantic tables, evidence, proof and
ranking records, and layout/placement/wire/specialization data remain prefixes.
An unsupported extension rejects without replacing the owned base.

Final package-declaration admission covers both strata before package-subject
derivation. Every generated selection retains its requesting package and obeys
public visibility and direct-dependency rules. Possessing transitive dependency
source does not authorize importing its declarations. The earlier verdict
continues to identify the frozen base admitted for build execution.

Generated code receives no authority from its generator. Ordinary runtime reach,
crash, work, conservation, and trust checking applies to it independently.

## Dependency handoff

Review compiles dependency-first. A consumer requires one compiler-issued
generated-source bundle for every transitive package, including an explicit
empty bundle when the producer included no source. A bundle binds:

- Producer package, dependency purpose, execution profile, and selected product target.
- Producer dependency closure and source-consumption commitment.
- Canonical generated paths, retained bytes, and their digests.

The consumer loads these bytes in its initial frontend under the producer's
identity and logical paths, without rerunning the producer build or reading its
output tree. Its own source-consumption commitment includes the injected bytes.
Missing, duplicate, foreign, root-self, purpose/profile/target/closure, or same-review custody
mismatches reject. The handoff is opaque compiler-issued state, not a package
instance or canonical admission evidence; it has no public constructor or decoder.

Implementation entry points and bounded continuation support are documented
[beside the compiler](../../../omega-rust/omega/compiler/compiler/generated_source.md).
