# Build execution and generated source

[Build declarations](declarations.md) define project description and the
activation-scoped Build operations. Build execution uses explicitly delegated
filesystem backing, with a confined snapshot/staging profile by default; effects outside that baseline require
explicit opt-in. It is not the hermetic evaluator used for constants and proofs.

## Admission before execution

1. The invoker supplies the root Build's source, package bindings and execution
   backing. Package management resolves and reviews the Build libraries. The
   compiler resolves the imports it needs to check this Build. Admission checks
   its executable closure, selected services and restricted requests before
   the interpreter runs its first statement.
2. Interpret the Build once. Each executed product-library registration fixes
   its source, alias and delegated context before the coordinator may schedule
   its acquisition. An import alone performs none of those actions.
3. For each dependency, review its source and apply the same pre-entry checks
   to its child Build using the bindings and backing supplied for that child.
   Child libraries and generated files can feed later compilations. They cannot
   extend the running parent's or child's imports.
4. When the entry returns, join every outstanding dependency job. Reject failed,
   cyclic, unresolved or unadmitted edges and freeze the normalized plan.
5. Freeze the filesystem view for the next compilation. Check the Product and
   its selected declarations, targets and behavior exclusions against that view.
6. Discover registered test roots, admit their execution authority and run
   enabled [test groups](testing.md). Check required outputs and publish one
   immutable result set only after required tests pass.

The application coordinator schedules this work. Package management owns source
resolution and review, the compiler checks each program, and the interpreter
executes and resumes its admitted code.

[Scoped execution](scoped_execution.md) defines the execution-profile/product-target
distinction, deterministic snapshot and staging protocol, and linear output
lifecycle. The running Build cannot inspect its unfinished component or execute
source it has just written for the next stage.

[Behavior exclusions](behavior_exclusions.md) are checked against the product's
complete selected execution scope before its successful publication. They do not
authorize a build-time query of that unfinished product or change the build
entry's own admission. Physical exclusions additionally require complete selected
mechanism classification before realized-product admission.

Before each Build starts, its exact imports, contracts and authority requirements
are known. Each supplied provider must satisfy its selected requirement and
pass review and admission before use. Ordinary suspension preserves the
checked program, live loans and resources. [Checked Build stages](scoped_execution.md#checked-build-stages)
describes how a new compilation receives different inputs.

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

A Build can admit [native borrow operations](../resources/native_borrows.md)
for the product it produces. Its own interpreted execution remains subject to
its caller's ceiling and interpreter support.

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

The root invocation sets the [live network ceiling](scoped_execution.md#live-networking-in-a-build).
The parent chooses whether to delegate its grant on each dependency registration.

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
admitted code and imports throughout. A separately requested Build can also use
generated source when its invoker supplies the frozen view before compilation.

For an interpreted successor, the parent can supply a directory it holds as
the [successor package root](declarations.md#successor-package-and-directories).
A confined disk directory suffices for this handoff.

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
