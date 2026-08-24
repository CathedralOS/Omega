# Chapter 15: Modules, Imports, And Visibility

Programs are made of source files grouped into packages.

This chapter defines source organization, names, imports, and visibility.

## Packages

A package is the compilation and dependency unit — and the **reach boundary**:
it declares what it may import, and imports resolve only against that
declaration. Visibility and hot-swap points nest *within* a package; a part that
needs a different reach-set is, by that fact, a different package.

**A package is a directory with a `build.omg`.** Source files are members **by
location** and do **not** re-declare it. There is no per-file `package X` line.
One directory = one package = one `build.omg`.

The package declares its human name once through ordinary build vocabulary:

```omega
const PACKAGE: Package = Package {
    name: "arithmetic-kernels"
};
```

The compiler evaluates this declaration hermetically before dependency
resolution or build execution. The directory and repository names do not
establish identity. The declared name is qualified by canonical source lineage
to form the stable `PackageKey` used by locks and nominal symbols; exact source
content and compiler evidence form a `PackageInstance`. A same-spelled package
or boundary from another lineage is therefore a different identity.

Packages expose public data, machines, traits, domains, wire schemas, and
boundary surfaces.

A package's dependencies — the external packages it may reach — are declared in
its **`build.omg`**, a capability-checked build-entry machine that augments a
`Build` (see
[`../design_briefs/build_and_package_model.md`](../design_briefs/build_and_package_model.md)).
Each dependency row requests a source and update selector. After fetching it,
Omega reads the dependency's own `PACKAGE` and derives the default local alias
by mapping kebab-case to snake_case. Explicit aliases are exceptional local
renames and never package identity.

`omega.lock` is machine-written accepted state: it records the reconciled
closure, exact commits/trees/content, source-qualified package identities,
compiler-derived capability/API baselines, build observations, and admission
evidence. The compiler consumes the lock rather than silently resolving mutable
selectors. The lock should normally be committed; source caches may be ignored.
The first implementation performs no semantic-version solving and rejects
incompatible requests for one `PackageKey` with their complete dependency
paths.

### Build orchestration is not semantic evaluation

Two kinds of Omega code run before the final program:

| | `build.omg` | Compiler semantic evaluation |
|---|---|---|
| World | build host | selected target semantics |
| Reach | explicit admitted capabilities | hermetic |
| Work | dependencies, target selection, staging | constants, proofs, plans, generators |
| Output | `Build`, staged artifacts, receipts | values and checked evidence |

`build.omg` is Omega's capability-audited build-script surface. Its entry may
receive selected `Filesystem`, `Network`, `Console`, process, signing, or other
build providers. None is ambient, and each operation remains visible in the
normalized contract and artifact. Semantic evaluation cannot call those
services. A host observation reaches a proof, type, layout, or constant only
after `build.omg` turns it into an explicit recorded build input.

Those names describe authority classes, not a requirement to mint one public
boundary trait per build operation. The concrete build library should use the
smallest ordinary Omega surface that preserves explicit authority, checked
reach, trust evidence, and observations; one-purpose services may remain
narrow toolchain-owned operations. Build logic likewise uses ordinary Omega
arithmetic rather than a package-specific numeric-policy layer.

Build operations publish an observation ceiling:

```text
Hermetic < Receipted < Volatile
```

The compiler records the join of statically reachable operations, the narrower
class actually reached, and the receipts. A release may reject a
volatile-capable build before running it. The artifact separately reports:

- **Replayable from record:** this exact compilation can be replayed from the
  stored inputs and receipts.
- **Rebuildable from source:** the complete dependency/toolchain/provider graph
  traces to declared reproducible roots.

A hash-pinned dependency artifact can satisfy the first even when its own build
used a volatile observation, in which case the graph fails the second. See the
[build/package brief](../design_briefs/build_and_package_model.md) and the
[semantic-evaluation brief](../design_briefs/build_time_evaluation.md).

## Path separator: `::` for names, `.` for values

Two different operations use two separators:

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
routed qualification such as `Tree::Valid`.

Changing a published source shape changes package-instance and public-contract
identity and causes dependents to rebuild or fail loudly. It does not silently
alter an ABI.
Only behavior declared `pub` is nameable outside the package. This preserves a
determinate component entry set for replacement and quiescence.

### Authority visibility and custody

Runtime authority uses ordinary data fields plus domain evidence. A value's
published geometry or handle bits remain inspectable; reconstructing those
fields does not reproduce its authority, validation, or provenance facts.
Checked operations require the qualification they consume.

An admitted provider may originate a routed qualification when it satisfies
an exact boundary requirement named in the domain declaration; admission
records the receipt. Checked resource transformations preserve or divide that
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
