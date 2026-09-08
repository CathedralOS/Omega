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

Use `builder.application("name")` for an executable. A workspace instead lists
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

Targets are explicit invocation inputs. They do not change the dependency graph.
Applications bind target entries with unconditional
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

Build observations distinguish `Hermetic`, `Receipted`, and `Volatile`
operations. An exact build may be replayable from retained inputs without being
rebuildable from source: recording an input does not establish its origin.
See [build execution](../spec/build/execution.md),
[observations](../spec/build/observations.md), and
[semantic evaluation](../spec/language/evaluation.md).

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
parser retains module declarations, but resolved module namespace behavior is
not yet implemented; parsing this form does not establish its full semantics.

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

A public contract cannot name a private declaration. A public machine's body
may use private implementation details; its exposed contracts may not. Likewise,
`terminates` is the public promise, while its ranking measure is private
proof machinery. Measures are not published with `pub`.

Named conformances have independent visibility. A public conformance may
retain private implementing machine identities: its consumer selects the public
conformance and its row map, not those machines independently. A requirement-local
`as Name` label does not create another public declaration.

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
