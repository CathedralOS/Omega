# Chapter 14: Modules, Imports, And Visibility

Programs are made of source files grouped into packages.

This chapter defines source organization, names, imports, and visibility.

## Packages

A package is the compilation and dependency unit — and the **reach boundary**:
it declares what it may import, and imports resolve only against that
declaration. Visibility and hot-swap points nest *within* a package; a part that
needs a different reach-set is, by that fact, a different package.

```text
package dungeon_crawler_cli
```

Packages expose public data, machines, traits, domains, wire schemas, and
boundary boundaries.

A package's dependencies — the external packages it may reach — are declared in
its **`build.omg`**, an effect-free function returning a build description (see
[`../design_briefs/build_and_package_model.md`](../design_briefs/build_and_package_model.md)).
Each dependency is a local alias bound to a pinned source (content hash), so
code names a stable alias while the binding is what moves. There is no version
solving and no separate lockfile — the pins live in `build.omg`.

## Files And Modules

Files organize declarations. A module path gives a stable name to declarations
inside those files.

```omega
module dungeon.combat;
```

The exact module syntax is provisional. The important rule is that module paths
are part of name resolution and build artifacts.

## Imports

Imports make external names available — but only from **declared
dependencies**. An import names a package (by its local alias) and a symbol
within it; a package not declared in `build.omg` is not nameable, so undeclared
reach is a resolution error, not a lint. Imports designate by logical name,
never by filesystem path — there is no reaching "up" the directory tree from
code. (The *build* may walk up to discover the enclosing package boundary; code
may not.)

```omega
use dungeon.combat.CombatSystem;
use dungeon.rooms.Room;
```

Imports do not execute code. They only affect name resolution.

## Visibility

Declarations are private by default unless marked public.

```omega
pub data Player {
    health: i32;
}

pub machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

Visibility is a source-level API boundary. It does not bypass proof,
ownership, or boundary checks.

## Name Resolution

Names resolve in this order:

- local bindings,
- state parameters,
- machine parameters,
- receiver fields through `self`,
- imported names,
- fully qualified package/module paths — **within the declared dependency set
  only.** A fully-qualified path does not bypass the reach boundary: naming a
  package the current package did not declare in its `build.omg` is a resolution
  error, not an ambient reach. (This gate is the build-time analog of the
  capability model; see
  [`../design_briefs/build_and_package_model.md`](../design_briefs/build_and_package_model.md).)

Ambiguity is an error. The compiler should not guess between two imported
declarations with the same visible name.

## Build Reports

Compiler artifacts should report:

- package graph,
- import graph,
- public API surface,
- boundary imports,
- versioned and wire declarations exported by a package.
