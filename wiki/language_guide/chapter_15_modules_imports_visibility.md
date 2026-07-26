# Chapter 15: Modules, Imports, And Visibility

Programs are made of source files grouped into packages.

This chapter defines source organization, names, imports, and visibility.

## Packages

A package is the compilation and dependency unit — and the **reach boundary**:
it declares what it may import, and imports resolve only against that
declaration. Visibility and hot-swap points nest *within* a package; a part that
needs a different reach-set is, by that fact, a different package.

**A package is a directory with a `build.omg`** (settled 2026-07-04). Its
identity lives in that manifest (and/or the directory name); source files are
members **by location** and do **not** re-declare it — there is no per-file
`package X` line (that was early spelling from before `build.omg` existed; it
duplicated a build concern into every source file, like Java's redundant
`package` statements). One directory = one package = one `build.omg`.

Packages expose public data, machines, traits, domains, wire schemas, and
boundary surfaces.

A package's dependencies — the external packages it may reach — are declared in
its **`build.omg`**, a build-time-admissible machine that augments a `Build` (see
[`../design_briefs/build_and_package_model.md`](../design_briefs/build_and_package_model.md)).
Each dependency is a local alias bound to a pinned source (content hash), so
code names a stable alias while the binding is what moves. There is no version
solving and no separate lockfile — the pins live in `build.omg`.

## Path separator: `::` for names, `.` for values

Settled 2026-07-04. Two different operations, two separators:

- **`::` resolves a compile-time name path** — packages, modules, types,
  associated items. It is the same `::` already used for type-scoped machines
  (`Main::run`, `Arena::allocate`), now used uniformly for *all* static name
  resolution.
- **`.` accesses a runtime value** — a field of a value, a method on an
  instance (`table.con_out`, `player.take_damage(...)`).

This is Rust's rule, and it removes the overload where `.` meant both "navigate
a package" and "access a field." `a::b.c` is unambiguous: package `a`, item `b`,
field `c`.

## Files And Modules

Files organize declarations. A module path gives a stable name to declarations
inside those files.

```omega
module dungeon::combat;
```

Module paths are part of name resolution and build artifacts.

## Imports

Imports make external names available — but only from **declared
dependencies**. An import names a package (by its local alias) and a symbol
within it; a package not declared in `build.omg` is not nameable, so undeclared
reach is a resolution error, not a lint. Imports designate by logical name,
never by filesystem path — there is no reaching "up" the directory tree from
code. (The *build* may walk up to discover the enclosing package boundary; code
may not.)

```omega
use dungeon::combat::CombatSystem;
use dungeon::rooms::Room;
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

### Public data shape

Publishing a structural `data` declaration publishes its field names and shape
to packages allowed to name it. Those packages may read, construct, and update
the value subject to ordinary borrow rules, field types, domains, invariants,
and qualification requirements.

The supporting model is:

- confidentiality comes from custody in memory the observer cannot access;
- unforgeable authority comes from checked domain evidence or an admitted
  provider receipt, not from a record literal;
- construction and mutation preserve the declaration's checked invariants; and
- ABI stability comes from normalized boundary/component representation plans,
  not from source visibility.

Structural access never manufactures an abstract qualification. A public range
record may be freely assembled; an authority *about* that range remains
evidence-backed and cannot be forged by placing the two beside each other.
When an invariant is not structurally expressible, useful operations require a
bodyless qualification such as `Tree in Valid`.

Changing a published source shape changes the pinned package identity and
causes dependents to rebuild or fail loudly. It does not silently alter an ABI.
Only behavior declared `pub` is nameable outside the package. This preserves a
determinate component entry set for replacement and quiescence.

### Authority visibility and custody

Runtime authority uses ordinary data fields plus domain evidence. A value's
published geometry or handle bits remain inspectable; reconstructing those
fields does not reproduce its authority, validation, or provenance facts.
Checked operations require the qualification they consume.

Boundary evidence lets an admitted provider originate an abstract predicate
under a receipt. Checked resource transformations preserve or divide that
evidence while accounting for every linear claim. See
[`authority_values_and_boundary_evidence.md`](../design_briefs/authority_values_and_boundary_evidence.md).

Confidential state remains in provider custody. A public value may carry an
index into that state, while the provider boundary controls lookup and
observation. Structural invariants govern ordinary data correctness, domain
facts govern authority and validation, and normalized boundary/component plans
govern ABI stability.

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
