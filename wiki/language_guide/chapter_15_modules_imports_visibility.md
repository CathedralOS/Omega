# Chapter 15: Modules, Imports, And Visibility

A package declares which other packages its source may use. Visibility then
determines which declarations those dependencies expose. Neither importing a
name nor carrying a value grants additional authority.

The [source naming rules](../spec/language/modules.md) and
[package boundary](../spec/packages/boundaries.md) specify the contract.
This chapter explains how to use it; [using source packages](packages.md)
covers installation and review.

## Packages

A package is a directory with a `build.omg`. Files belong by location rather
than repeating a package declaration. The package declares its own name:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

Use `builder.application("name")` for an application, executable by default.
Add direct `builder.artifact_only()` for an artifact-only application; it must
complete at least one required output and cannot select executable roots/providers.
A workspace instead lists
`builder.member("path")` calls; each member is an independently selectable
root, not part of one combined dependency graph. An application can depend on
packages, but cannot itself be imported as a library.

The build root is a free `machine build(builder: &mut Build)`, not an attached
`Owner::build`. Project kind, members, and dependencies are direct unconditional
statements: the package manager reads them before executing any build code.
Helpers may perform later admitted build work, but cannot hide dependency edges.

A package name is not globally unique. Its stable identity combines its declared
name with its source lineage. A revision identifies the selected source, not a
new nominal package: two incompatible resolutions of one package cannot coexist
merely because a caller gives them different aliases. For a Git workspace,
moving a member within the same repository does not rename it.

The default import alias for `arithmetic-kernels` is `arithmetic_kernels`.
Aliases belong to the requester: your rename does not change names used inside
the dependency. See [source selection](../spec/packages/sources.md) for exact
Git pins, workspace member selection, and reconciliation.

Commit `omega.lock` under the project's review controls. It records the graph,
pins, accepted policy, and decisions. Install/update reviews changed findings;
ordinary compilation uses the accepted baseline rather than asking for the same
permission again. Source changes remain visible even when permissions do not
change. Acceptance neither proves the dependency safe nor bypasses compiler
proof, reach, or artifact checks.

### Build and product dependencies

Use `depend`/`depend_as` for product code and `build_depend`/`build_depend_as`
for build-only libraries. Both are direct unconditional root declarations:

```omega
machine build(builder: &mut Build) {
    builder.application("generated-tables");
    builder.build_depend_as("generator", Source::Path {
        location: "../table-generator"
    });
    builder.depend_as("tables", Source::Path {
        location: "../table-protocol"
    });
    configure_products(builder);
}
```

This illustrates the specified dependency surface, not a claim of current
compiler support or a complete runnable build. The local `configure_products`
helper can import `generator`; product source cannot. Generated code using
`tables` needs the ordinary product edge. Using std or another library in both
contexts requires both declarations. A build library uses its own ordinary
dependencies for its host implementation, not the consumer's product aliases.

Imports remain file-local. Build helpers can live in ordinary local files;
importing a file in both contexts checks it twice under the corresponding scope.
Two scopes may use the same alias differently without lookup fallback. A legacy
`depend` remains product-only; missing build edges need explicit declarations,
not permission inferred from an old lock or dead-code elimination.

The build evaluator's execution profile and the product target are distinct.
A macOS helper generating Windows code is checked for the build execution profile;
`Build.target` still describes Windows. Build-only source acquisition is reusable,
but host-generated configuration is not target evidence. The
[scoped execution contract](../spec/build/scoped_execution.md) fixes these rules;
[implementation tasks](../../TASKS.md#scoped-build-execution) track availability.

Targets are explicit invocation inputs. They do not change the dependency graph.
Executable applications bind target entries with
`roots.bind(target::ProgramEntry, entry)` rows. The
[multi-target contract](../spec/build/configuration.md) requires an explicit,
nonempty target set and independent outcomes; it does not infer an `all` set
from declarations or the compiler catalog.

For macOS GUI output, `Subsystem::Gui` selects the
[application publication contract](../spec/build/macos_application.md):
one bundle, with an authored application identifier supplied before signed
native emission. Bundle production and its new identifier field remain
implementation work; the current output is a flat executable and std requests
foreground activation. Producing Terminal Psi alone does not need those native
inputs.

### Optional proof output

The specified Build interface selects portable proof products independently:

```omega
// Inside the root's build machine; both default to false.
builder.pcc.psi = true;
builder.pcc.native = true;
```

These fields remain implementation work. Psi PCC retains the Psi artifact and
its `.proof` sidecar; native PCC supplies a standalone sidecar next to the final
executable. Neither enables ordinary checking—it already runs—and neither
selects the receiver's trust policy. See
[shipping proofs](chapter_10_compile_time_proofs.md#shipping-independently-checkable-proofs).

### Build orchestration is not semantic evaluation

Both can run before the final program, but they have different authority:

| Work | Inputs and effects |
| --- | --- |
| `build.omg` | Admitted build-host services, staging, generated source, and provider/root selections. |
| Semantic evaluation | Hermetic target-semantic constants, proofs, plans, and generators. |

A host observation becomes a semantic input only through recorded build-input
custody. A constant or proof cannot directly read the build host's filesystem
or environment. Selecting a runtime service provider does not grant build-host
authority.

Product entry/provider operands in designated Build operations resolve against
the product scope without importing or executing target code on the host.
Ordinary host calls and same-named authored methods do not get that exception.
Reusable helpers can receive restricted, non-callable product descriptions.
Passing the entire Build does not grant access to the caller's private names.
Own generated entries and this invocation's final component are unavailable
during its build; inspect them in a separate, later build instead.

Default build I/O is a read-only captured input tree and fresh append-and-seal
staging, with deterministic logical paths and metadata. No home directory,
network, subprocess, environment, or implicit symlink traversal is granted.
Pass a template view and narrow output writer to a generator rather than the
whole builder. A required-output token is linear: complete it with a sealed file
or explicitly fail it. Publication waits for all required outputs and final
product checks; logging an error is not the same as recording failure.
Artifact-only mode changes entry requiredness, not target identity or authority.

Build observations distinguish `Hermetic`, `Receipted`, and `Volatile`
operations. An exact build may be replayable from retained inputs without being
rebuildable from source: recording an input does not establish its origin.
See [build execution](../spec/build/execution.md),
[observations](../spec/build/observations.md), and
[semantic evaluation](../spec/language/evaluation.md).

### Build-level behavior exclusions

A library can keep conservative `crashes Trap` contracts while the build requires
its selected executable to have no possible Trap. Source contract checking and
this product check are independent: the build neither removes handwritten clauses
nor permits a callee to violate an intermediate contract.

For example, use the same library source and public crash allowances with two
ordinary assertion implementations:

| Build selection | Additional requirement | Result |
| --- | --- | --- |
| Checking implementation | No added crash exclusion | May Trap where its ordinary contracts permit. |
| Checking implementation | Exclude Trap | Reject if an assertion can fail. |
| No-op implementation | Exclude Trap | Pass only if the entire selected product excludes Trap. |

These are configuration choices, not built-in debug/release modes. No special
assertion macro or diagnostic crash cause is needed. Exact Build API spellings
and implementation coverage are tracked by
[the implementation tasks](../../TASKS.md#build-level-behavior-exclusions).

The no-op does not skip eager argument evaluation. A predicate that can Trap
still prevents a no-Trap result; logging, mutation, divergence and resource
disposition are not erased because an assertion ignores their result. A shared
checking/no-op contract also cannot establish the asserted condition on return.
Validation needed for safe subsequent operations must remain enabled. General
optimization can remove redundant pure work, but optional optimization switches
must not decide whether the product satisfies its exclusions.

Service exclusions work similarly, but abstract services and physical mechanisms
remain distinct. An ordinary silent logger can exclude Console when its verified
implementation never invokes it. A call to Console still counts even with a silent
Console provider. Excluding the physical process-output class is a separate choice.

The compiler needs complete selected-code evidence, including dependencies,
generated code, callbacks and cleanup. An opaque dependency promising only that
it may Trap cannot certify absence. A verified no-Trap product does not turn its
public may-Trap functions into no-Trap callables for unrelated consumers. Its
guarantee binds the exact composition and must survive installation/replacement
checks. See [behavior exclusions](../spec/build/behavior_exclusions.md).

## Path separator: `::` for names, `.` for values

Use `::` to resolve a static name: a package, module, type, or associated
declaration. Use `.` to access a field or call through a value.

```omega
let room = dungeon::rooms::create();
room.describe();
```

Here `dungeon::rooms::create` selects a declaration. `room.describe()`
selects a machine through the value `room`; that still requires the machine's
declaring package to be an authorized dependency.

## Files And Modules

Files organize declarations; module paths name them:

```omega
module dungeon::combat;
```

Module paths participate in name resolution and artifact identity. They are
not filesystem escape paths or additional package reach boundaries. The Rust
implementation supports selected nominal/free-machine namespace paths; remaining
forms, including module-owned domains, still reject pending namespace-aware
normalization. See [source resolution](../../omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization).

## Imports

Imports select logical names, not filesystem paths:

```omega
use dungeon::combat::CombatSystem;
use dungeon::rooms::Room;
```

An external package must be a direct declared dependency. Fully qualifying its
name does not bypass that rule. Imports affect resolution; they do not execute
the imported code. Tooling may discover an enclosing build directory, but source
imports cannot reach upward through directories to evade package declarations.

Imports are local to this source file, even when sibling files declare the same
logical module. They do not spread transitively or change visibility of the
module's own declarations. Importing a module exposes its directly declared
public foreign domains; importing one domain exposes only that declaration.
Importing a machine or carrier exposes no sibling or foreign domains merely
because their source was loaded. Descendant modules and the imported module's
own imports do not add extension candidates.

For example, `use ui_policy::screen::Point::OnScreen;` selects an independently
owned domain in `ui_policy::screen`. Its carrier attachment resolves in the
declaring context, not against the caller's same-spelled `Point`. Direct qualified
selection exposes no siblings. The domain's exact owner and the carrier's owner
remain distinct. Importing grants no membership, minting authority, private
representation access, or implicit operator change.

Two distinct exposed declarations competing for one carrier-qualified name
reject; repeating the same exact import does not. Neither import order nor
carrier ownership gives priority. A broad import may collide after module growth;
a narrow import avoids unrelated additions. Diagnostics show both owners and
the imports responsible. See [ordinary import rules](../spec/language/modules.md#import-scope-and-exposure).

### Declaration selection and carried foreign types

Suppose `filesystem` is your direct dependency, and its API returns a handle
type owned by one of its dependencies. You may use that API without directly
depending on the handle's owner:

```omega
let handle = filesystem::open(path);
filesystem::read(&handle);
filesystem::close(handle);
```

This example is schematic: the selected API supplies the concrete signatures
and ownership modes. Moving, borrowing, storing, returning, and passing the
inferred value does not select its owner's declarations. Multiplicity and
compiler-planned cleanup still apply.

The same distinction preserves an exact foreign-domain qualification and its
evidence when received through an API. Carrying it does not import the domain
for authored lookup or activate its declaring package's other extensions.

Selecting the handle owner's fields, methods, cases, operators, or conformances
is different. That needs a direct dependency on the owner, even if its name is
hidden by receiver inference. An attached declaration such as
`machine Handle::inspect` also selects `Handle`; qualification does not
provide missing access.

Automatic cleanup is carried type behavior. The compiler uses the exact
owner-attached cleanup hook, not an arbitrary same-spelled machine. Authored
early cleanup uses an ordinary consuming operation such as
`omega::core::drop(value)`, with its ordinary dependency requirements.

Public signatures preserve transitive nominal identity. Carrying a private
conformance chosen by another package does not grant permission to name it.
The [boundary contract](../spec/packages/boundaries.md) specifies these
distinctions; the [review contract](../spec/packages/review.md) explains their
API and rebuild consequences.

## Visibility

Independently nameable declarations are private unless marked `pub`.
Qualification does not inherit visibility: a public `Player` does not
automatically publish `Player::take_damage`. A genuine field, case, state, or
trait requirement follows its exact containing declaration.

```omega
pub data Player {
    health: i32;
}

pub const MAX_DAMAGE: i32 = 100;

pub machine Player::take_damage(&mut self, amount: i32)
requires amount >= 0
requires self.health >= amount
{
    self.health = self.health - amount;
}
```

The preconditions also keep this subtraction within range. Public visibility
does not waive numeric, ownership, or proof rules.

A public contract cannot expose a private declaration for consumer selection.
A public machine's body
may use private implementation details; its exposed contracts may not. Likewise,
`terminates` is the public promise, while its ranking measure is private
proof machinery. Measures are not published with `pub`.

Named conformances have independent visibility. A public conformance may
retain private implementing machine identities: its consumer selects the public
conformance and its row map, not those machines independently. A requirement-local
`as Name` label does not create another public declaration.

A public domain may similarly retain private trait requirements or exact machines
in its issuer catalog. This [authorization metadata](../spec/resources/authority.md#private-issuer-routes)
does not make the private route callable or externally implementable, and does
not expose private carrier types. The verifier still checks exact issuer identity;
private does not mean secret or omitted from evidence.

Omega has no re-export item that changes dependency ownership. To present a
dependency's behavior under your package's API, write an ordinary public wrapper.
A public lookalike also cannot grant access to compiler intrinsics.

### Public data shape

A public structural data declaration exposes its fields and shape. Authorized
consumers can construct, read, and update it while preserving its field types,
invariants, borrow rules, and qualification requirements.

Visible fields are not authority. A caller may assemble the geometry of a public
linear `Extent`, but that does not establish `Extent::Granted`. Linearity
tracks the value's use; the routed qualification establishes the authority.

Changing public shape changes the source/API contract. Source visibility is not
an ABI guarantee; independent replacement uses explicit
[component publication](../spec/build/component_publication.md) contracts.

### Authority visibility and custody

Handle bits and range fields can be public while the state they refer to stays
in provider custody. Reconstructing the bits does not reproduce validation,
provenance, or permission. Checked operations require the applicable evidence.

Use invariants for ordinary data correctness, routed qualifications for authority
that needs an establishment route, and provider custody for confidential state.
See [authority and qualification](../spec/resources/authority.md).

## Name Resolution

Resolution considers lexical bindings and parameters before imported names.
Receiver fields are accessed through `self`; a fully qualified package path
still obeys direct-dependency and visibility rules. Ambiguous imported names
reject rather than selecting one by traversal order.

The [source naming rules](../spec/language/modules.md#resolution) give the
resolution order. Importing names never changes the meaning of an already
selected declaration.

## Build Reports

Reports expose the package/import graph, public API, boundary imports, and
published wire/versioned declarations. Review also preserves source changes
and authority/assumption findings without treating report wording as identity.
Read [using source packages](packages.md) for inspecting and accepting changes.
