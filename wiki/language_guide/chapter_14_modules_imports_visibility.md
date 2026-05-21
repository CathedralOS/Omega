# Chapter 14: Modules, Imports, And Visibility

Programs are made of source files grouped into packages.

This chapter defines source organization, names, imports, and visibility.

## Packages

A package is the compilation and dependency unit.

```text
package dungeon_crawler_cli
```

Packages expose public data, machines, traits, domains, wire schemas, and
trusted boundaries.

## Files And Modules

Files organize declarations. A module path gives a stable name to declarations
inside those files.

```omega
module dungeon.combat;
```

The exact module syntax is provisional. The important rule is that module paths
are part of name resolution and build artifacts.

## Imports

Imports make external names available.

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
ownership, or trust checks.

## Name Resolution

Names resolve in this order:

- local bindings,
- state parameters,
- machine parameters,
- receiver fields through `self`,
- imported names,
- fully qualified package/module paths.

Ambiguity is an error. The compiler should not guess between two imported
declarations with the same visible name.

## Build Reports

Compiler artifacts should report:

- package graph,
- import graph,
- public API surface,
- trusted boundary imports,
- versioned and wire declarations exported by a package.
