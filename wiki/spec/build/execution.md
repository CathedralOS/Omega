# Build execution and generated source

[Build declarations](declarations.md) define project description and the
activation-scoped build facets. Build execution uses the benign confined
snapshot/staging baseline by default; effects outside that baseline require
explicit opt-in. It is not the hermetic evaluator used for constants and proofs.

## Admission before execution

The order is:

1. Check package-local Build source against `omega::core` and every pre-known
  requirement used by `Build::await_provider<R>`; surface and admit its own
  restricted requests, then produce one prepared Terminal entry.
2. Interpret that entry once. Each dependency declaration mutates the same
  `Build` and submits an immutable purpose-qualified request without suspending
  the Build continuation.
3. For each request, the application coordinator asks package management to
  resolve and review source, recursively activates the dependency Build, and
  asks Psi to compile and admit any implementation requested through
  `Build::await_provider<R>`.
4. When provider readiness encounters an implementation that is not yet
  installed, the Terminal interpreter suspends that exact continuation. After
  package coordination installs the admitted implementation into the exact
  Build-scoped static slot, the interpreter resumes without replaying prior work.
5. When the Build entry returns, join every outstanding dependency job, reject
  any failed, cyclic, unresolved, or unadmitted edge, resolve inert product
  selections, and freeze the normalized Build output.
6. Incorporate generated source and finish all requested product checks,
  including behavior exclusions.
7. Discover registered test roots, admit their execution authority, and run all
  enabled [test groups](testing.md).
8. Check required-output completion and commit one immutable result set only
  after required tests pass.

The application coordinator owns this lifecycle; package management owns source
resolution and review; Psi owns checking and admission; the Terminal interpreter
owns suspension, provider-slot installation, and continuation resumption. A
dependency declaration may complete in the background when Build code never
awaits its provider slot. Build return is still a mandatory join, so no unresolved request leaks
into product compilation or publication.

[Scoped execution](scoped_execution.md) defines the execution-profile/product-target
distinction, deterministic snapshot and staging protocol, and linear output
lifecycle. No current activation can inspect its own final component or select
its generated entries; those customers require separately staged compilation.

[Behavior exclusions](behavior_exclusions.md) are checked against the product's
complete selected execution scope before its successful publication. They do not
authorize a build-time query of that unfinished product or change the build
entry's own admission. Physical exclusions additionally require complete selected
mechanism classification before realized-product admission.

Before step 2, root Build symbol/type resolution, semantic prechecks, and its
known operational/reach plans are complete. Before any dependency provider slot
is installed in step 4, that selected implementation's source resolution, review,
checking, restricted-request admission, and exact dynamic-call binding are
complete. The root checkpoint binds its base typed program, prepared Terminal
entry, source snapshot, and authority verdict; each installed slot carries
the corresponding admitted dependency checkpoint. Execution uses those exact
entries, not later name lookup, and the root Build runs once.

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

## Restricted build requests

Registering a root [assumed binding contract](assumed_binding_contracts.md)
selects a runtime product relationship only. The build receives no callable
binding or usable premise from registration. Execution through it, or using its
assumptions to justify build computation, rejects across the complete build,
generator and automatic-test closure. This is not a restricted request that
lock acceptance or a wider host grant can enable. Independently checked calls
that do not rely on the relationship retain their ordinary admission rules.

Ordinary computation, admitted immutable inputs, private staged outputs, and
bounded diagnostics use the [default build protocol](scoped_execution.md).
They do not require an unsafe-build approval merely because a dependency supplies
the code. This baseline does not provide arbitrary live filesystem, process,
network, credential, or resolver access. Unsupported host operations remain
unsupported; acceptance cannot create a missing execution protocol.

Checked build contracts expose requests outside that baseline, including requests
reached through local helpers, installed dependency bindings, or prerequisite
dependency Build activations. No nested call, dependency edge, or accepted
runtime permission hides or authorizes a build-host action. Retain the originating
package, build purpose, operation, logical resource scope, bounds, and applicable
profile/target context. Delegation uses explicitly supplied capabilities and
cannot widen their authority.

The root selects the filesystem implementation. Descendants receive scoped views
of that selection, not independent backend choices. Importing a real-filesystem
API cannot bypass a root-supplied virtual filesystem; an unsupported operation
rejects instead of opening a host route. Lock agreement and actual capability
checks govern execution, not replay or a reproducibility classification.

[Install/update review](../packages/acceptance.md#restricted-build-acceptance)
surfaces those requests before restricted execution. Explicit project decisions
are retained in `omega.lock`; the executor separately supplies actual scoped
resources. Neither lock acceptance nor a callee's own lock manufactures a host
grant. A root's grant is not automatically available to dependency builds.

Checking and audit cannot run an unaccepted restricted action merely to learn
what it requests. Check each authored Build or requested binding implementation
before its first execution; if further checking needs such an action, retain a
pending review/incomplete audit until acceptance and actual grants permit
continuation. Generated source is then checked and audited normally, not trusted
because its generator was accepted. Benign work needs no approval ceremony;
unchanged accepted requests need no recurring approval.

This governs existing helper delegation and dependency build scheduling; it adds
no recursive build API or permission to import another activation's build entry.
The executor, not the offered program, enforces actual host confinement.

The same admission and attenuation rules govern build-time tests, including
fixtures and selected providers. Completing the build entry does not grant its
test roots ambient host authority. Restricted test requests join ordinary
install/update review before test execution. Tests may use delegated virtual
services without restricted approval, but cannot replace them with live-host
backing. Runtime permissions and a child package's lock are not build-test grants.

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
[beside the typing stage](../../../omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/README.md).
