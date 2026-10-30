# Build execution and generated source

[Build declarations](declarations.md) define project description and the
activation-scoped Build operations. Build execution uses explicitly delegated
filesystem backing, with a confined snapshot/staging profile by default; effects outside that baseline require
explicit opt-in. It is not the hermetic evaluator used for constants and proofs.

## Admission before execution

The order is:

1. Parse Build syntax and establish lexical structure, then check the initial
  executable prefix against core, local declarations and available exact
  dependencies. Surface and admit that stage's restricted requests before
  producing its prepared Terminal continuation.
2. Interpret the checked stage once. Atomic dependency registration fixes each
  source, purpose, alias and delegated context before coordination may schedule
  acquisition and preparation. An import alone performs none of those actions.
3. For each request, the application coordinator asks package management to
  resolve and review source, recursively activates the dependency Build,
  prepares its exact declarations and selected implementations in that context,
  and asks Psi to check and admit the dependent executable closure. No satisfier
  search, ambient import or default chooses a service implementation.
4. At explicit dependency preparation, suspend the same activation if necessary.
  Resolve the dependent imports, types, contracts and exact provider bindings,
  check and admit the selected continuation, then resume without replaying
  earlier work. Repeat stage preparation as ordinary control flow discovers
  dependencies; do not execute unchecked source or rerun the Build as a scan.
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

Before each stage executes, its exact name/type resolution, semantic prechecks,
operational/reach plans and effect admission are complete. Before a provider is
installed, its selected implementation's review, checking, restricted-request
admission and exact binding are complete. Checkpoints retain stage code,
declaration/environment identities, source and filesystem observations, live
loans/resources and authority verdicts. Execution uses those exact entries,
not later lookup that can retarget an earlier alias. This does not claim that
all deferred branches of the whole Build were checked before its first effect;
[checked stages](scoped_execution.md#checked-build-stages) define that boundary.

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
provider selection is not a route around delegated backing and stage admission;
additional host effects require actual supplied capabilities and accepted policy.

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

The parent supplies each child's filesystem backing and selected implementation
under ordinary service contracts. It may compose a restricted or virtual provider
without the child knowing its physical representation. Alternative providers
are legal only with explicitly supplied backing within the parent's grant;
importing a real-filesystem API cannot create such a grant or bypass a virtual
view. Unsupported operations do not open a host route. Lock agreement and
actual capability checks remain separate obligations.

[Install/update review](../packages/acceptance.md#restricted-build-acceptance)
surfaces those requests before restricted execution. Explicit project decisions
are retained in `omega.lock`; the executor separately supplies actual scoped
resources. Neither lock acceptance nor a callee's own lock manufactures a host
grant. A root's grant is not automatically available to dependency builds.

Checking and audit cannot run an unaccepted restricted action merely to learn
what it requests. Check each reached Build stage or requested implementation
before its effects; if further checking needs such an action, retain a
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

Path lookup, filesystem writing and compiler publication are distinct
operations. `Build::include_source` accepts an authorized logical generated name
and exact retained UTF-8 bytes under the current activation's resources; it does
not reopen a mutable host pathname or execute those bytes. Filesystem helpers
produce the bytes through their ordinary capability contracts. The compiler
captures the published snapshot and exact generating context; subsequent writes
to a file or overlay cannot change it. Source consumption retains the matching
compiler-owned generated-source namespace and product purpose. This replaces
the BuildOutput filesystem-specific source-publication route.

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
