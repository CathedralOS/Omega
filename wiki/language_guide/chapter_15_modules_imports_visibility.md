# Chapter 15: Modules, imports, and visibility

A package declares its dependencies. Modules organize its declarations, and
imports give a file convenient names for declarations it may use.

## Declare a package

A package is a directory containing `build.omg`:

```omega
machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_kernels");
}
```

Use `Build::application` for an application. A workspace lists its members with
`Build::member`; each member is an independently selectable build root.
An application may depend on packages but is not itself imported as a library.

A dependency's default import alias keeps its declared snake_case name, such as
`arithmetic_kernels`. A requester can choose another alias without renaming the
package inside its own source. Package identity
includes source lineage, so aliases cannot make incompatible revisions of the
same package coexist.

Commit `omega.lock` with the project. It retains the dependency graph, source
pins and accepted review decisions. [Using source packages](packages.md) covers
declaring dependencies, installation and updates.

## Organize files and modules

Put a module declaration at the top of a file:

```omega
module dungeon::combat;
```

Files declaring the same module share its declarations. Files with no module
declaration form one logical module within the package. Imports remain local
to each file.

Use `::` for static names and `.` for fields:

```omega
let room: dungeon::rooms::Room = dungeon::rooms::create();
dungeon::rooms::describe(&room);
```

`describe` receives the room explicitly. Module paths resolve declarations;
they are not paths for reaching outside the package directory.

## Import a declaration

```omega
use dungeon::combat::CombatSystem;
use dungeon::rooms::Room;
```

External packages must be direct declared dependencies. A fully qualified name
obeys the same rule as an import. Importing a name does not run its code.

Prefer a narrow import when you need one declaration. Importing a module also
exposes its directly declared public foreign domains; importing a specific
domain exposes only that domain. Neither imports from sibling files nor the
imported module's own imports spread into this file.

Conflicting imports produce an ambiguity error with both owners and import
sites. Import order never chooses a winner. See [import scope](../spec/language/modules.md#import-scope-and-exposure)
for domain attachments and extension lookup.

## Publish an API

Independently nameable declarations are private unless marked `public`:

```omega
public data Player {
    health: i32;
}

public machine take_damage(player: &mut Player, amount: i32)
requires amount >= 0
requires player.health >= amount
{
    player.health = player.health - amount;
}
```

Publishing `Player` exposes its fields and shape. Publishing `take_damage` is
a separate choice. Its preconditions keep the subtraction valid.

Fields, cases, nested data declarations and trait requirements follow their containing declaration's
visibility. Nested machines have lexical scope inside their enclosing machine;
exporting the outer machine does not expose its helpers.

A public body may use private implementation details. Its public signature and
contracts must expose names consumers are permitted to select. Named public
conformances can retain private implementing machines because consumers select
the conformance's published row map.

## Public fields and access policies

Consumers can inspect and construct public structural data while satisfying its
types, invariants, borrowing and access policies. Field visibility alone does
not grant a routed qualification. Reconstructing the range fields of an
`Extent`, for example, does not establish `Extent::Granted`.

Use domains and access policies for controlled establishment and mutation.
Provider-owned confidential state can remain behind a checked handle. Changing
public structure changes the source interface; binary replacement additionally
uses [component contracts](../spec/build/component_publication.md).

There are no private fields, and you do not need them to keep a representation
replaceable. In one package, the access plan is the abstraction: it names the
operations that may establish, read and write the governed value, so every
other machine, inside the package or out, goes through those operations. A
client that uses only the operations keeps compiling and verifying when you
reorganize the fields; it is rebuilt, not edited. A client that names a field
has chosen to depend on it, exactly as it would by depending on a public
function. The shape stays visible because data is data, and because the access
plan already says precisely who may touch it.

If you also want to deny selecting the fields at all, that is a package
boundary: a type carried from a package the client does not depend on directly
can be held, moved, borrowed and stored, but its fields, cases and conformances
cannot be selected. The next section shows the client side.

## Use a type returned by a dependency

An API may return a type owned by one of its own dependencies. You can pass that
value back through the API without directly depending on its original owner:

```omega
// Sketch: filesystem supplies the signatures and handle contract.
let handle = filesystem::open(path);
filesystem::read(&handle);
filesystem::close(handle);
```

Moving, borrowing or storing the inferred value preserves its type and
obligations. Selecting its owner's declarations, such as fields, cases,
operators or named conformances, requires a direct dependency on that owner.
Automatic cleanup still follows the exact owner-selected operation.

To expose behavior through your own API, write a public wrapper. See
[package boundaries](../spec/packages/boundaries.md) for carried foreign types
and declaration selection.

## Run a build helper

Product dependencies supply code to the program. Build dependencies supply
helpers interpreted during the build. Configure the dependency, register it,
then prepare it before calling its code:

```omega
use generator;

machine build(builder: &mut Build)
suspends
{
    Build::application(builder, "generated_tables");
    let mut request = dependency::from_source(Source::Path {
        location: "../table-generator"
    });
    dependency::with_alias(&mut request, "generator");
    let registered = Build::build_dependency(builder, request);

    suspend Build::prepare_dependency(builder, registered);
    generator::generate_tables(builder);
}
```

This build API sketch assumes the dependency helpers are in scope. Preparation
resolves and checks the generator's continuation before it executes.
Registration fixes its source and alias. The build waits for outstanding
dependency requests before compiling the product.

Passing the whole `Build` delegates its registration operations. A small
generator can instead accept input bytes and a narrow output writer or
filesystem view. Build dependencies do not inherit the resolver's network or
credential authority.

A helper runs for the build host, even when generating source for another
target. Semantic evaluation of constants and proofs uses hermetic inputs.
The [build execution reference](../spec/build/execution.md) covers staging,
generated outputs and delegated filesystem views; [configuration](../spec/build/configuration.md)
covers target entries and artifact-only applications.

## Inspect the package

Build reports show dependencies, imports, public APIs and boundary requirements.
Package review also shows source changes and authority findings. Use
[package audits](packages.md) to inspect the selected program, and
[behavior exclusions](../spec/build/behavior_exclusions.md) when a build should
forbid a service or crash cause throughout its selected code.
