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
6. Freeze the filesystem view the plan supplies to the next compilation.
  Check the Product from that view, including the requested behavior exclusions.
7. Discover registered test roots, admit their execution authority, and run all
  enabled [test groups](testing.md).
8. Check required-output completion and commit one immutable result set only
  after required tests pass.

The application coordinator owns this lifecycle; package management owns source
resolution and review; Psi owns checking and admission; the Psi interpreter
owns suspension, provider-slot installation, and continuation resumption. A
dependency declaration may complete in the background when Build code never
awaits its provider slot. Build return is still a mandatory join, so no unresolved request leaks
into product compilation or publication.

[Scoped execution](scoped_execution.md) defines the execution-profile/product-target
distinction, deterministic snapshot and staging protocol, and linear output
lifecycle. The running Build cannot inspect its unfinished component or execute
source it has just written for the next stage.

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

The parent can [supply a filesystem for one dependency](scoped_execution.md#supplying-a-filesystem-to-a-child-build)
when it registers that dependency. It may compose a restricted or virtual
provider without the child knowing its physical representation. Alternative
providers are legal only with explicitly supplied backing within the parent's grant;
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

A generator writes ordinary files through the filesystem view its caller gives
it. The Build plan supplies the next compilation's source view, which can
contain both handwritten files and files the generator wrote. A generator
needs write access to its destination. It can run as a Build helper or as an
ordinary program with the same backing.

For example, a generator writes `parser.omg` into the next stage's package
root. A handwritten Product file imports it in the ordinary way:

```omega
use parser.omg as parser;
```

At handoff, freeze the selected view with its exact logical paths and bytes,
whether the backing is in memory or on disk. Later writes cannot change the
inputs to that compilation. A write outside the supplied view, including a
file exported to the host, contributes no source to it. Writes inside the
view supply files. Ordinary roots and imports select code. The rules for
[validation subjects](validation_subjects.md) and
[registered test discovery](testing.md#registration-and-discovery) also apply
to the selected package.

The compiler checks handwritten and generated Product files together under
the same [file identity and visibility rules](../language/modules.md#file-identity).
Either can import the other by its logical filename. A file has one identity
within its package and source context. Producing it grants no extra access
to other packages or runtime services. The Build activation keeps its own
admitted code and inputs throughout. Its dependency-preparation continuations
still follow the [checked Build stage rules](scoped_execution.md#checked-build-stages).

The `product-source-view` decision (2026-11-22) retires `Build::include_source`,
per-file source registration, and the requirement to append generated units
to an earlier compiler checkpoint. Product checking can reuse valid prior
work as long as checking the same frozen inputs gives the same result.

## Dependency handoff

Review compiles dependencies first. For each dependency in the reviewed
closure the consumer checks the compiler-issued record of the producer's
frozen source view under its package identity and logical paths. Its
source-consumption record commits to the exact paths and contents, producing
package and dependency closure, Build or Product purpose, execution profile
and selected target. It retains enough evidence to validate that handoff,
including when the producer generated no files.

The consumer checks that the view belongs to the expected dependency and
review. Missing required inputs, duplicate file identities, a foreign or
root-self substitution, or a mismatch in purpose, profile, target or closure
reject. Direct dependency and visibility rules apply to every import.
Consumption uses the retained snapshot without rerunning the generator or
reopening a mutable output directory. A package cannot construct this review
record by supplying its own bytes. The view itself grants no package admission
or host authority.
