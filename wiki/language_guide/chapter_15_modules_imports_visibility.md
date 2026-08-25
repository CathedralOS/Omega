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
content, produced artifact identity, per-subject obligation-semantics identity,
re-derived verification results, and disclosed open assumptions form a
`PackageInstance`. A same-spelled package or boundary from another lineage is
therefore a different identity. Compiler and toolchain provenance remain
separate review metadata for reproduction, cache partitioning, and incident
response; it never seals the instance or proves that the producer or an audit
was trustworthy.

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

After reconciliation, the compiler receives the complete requester-local alias
graph together with an opaque commitment to each `PackageKey` and each
resolver-owned source root. It does not rediscover dependencies from package
build code. The commitment, not the cache path, governs same-package and future
nominal-identity checks; source roots only constrain where imports may load.

`omega.lock` is machine-written accepted state: it records the reconciled
closure, exact commits/trees/content, source-qualified package identities,
compiler-derived capability/API baselines, representation-TCB rows, build
observations, and admission evidence. Claim-free opaque boundary representation
remains visible and audit-recommended without being mislabeled as an accepted
claim; Chapter 19 defines the distinction. The compiler consumes the lock
rather than silently resolving mutable selectors. The lock should normally be
committed; source caches may be ignored. The first implementation performs no
semantic-version solving and rejects incompatible requests for one `PackageKey`
with their complete dependency paths.

Package review is proposed by the selected local compiler rather than accepted
from dependency source. This prevents a package from declaring its own
capability result, but compiler output remains review-only until a consumer
reconstructs the exact obligations from the exact requested source and produced
artifact and checks the retained certificates. The evidence record is a cache
of a re-derivable result, not authority. Review status, producer provenance,
signatures, and reasons are organization policy records; none is a portable
proof that a human or LLM performed a sound audit.

Dependency evidence composes transitively. Each subject retains its own
obligation-semantics and evidence-schema identity, exact certificate
provenance, discharged obligations, and still-open obligations. A parent cannot
absorb a dependency's open obligation merely because its producer accepted it.
Each consuming project rechecks the certificates and independently decides
every disclosed admission before creating accepted lock state. A checked schema
migration may reuse only the obligation classes it proves unchanged and exposes
new or changed classes as open obligations; an unknown migration forces
re-derivation.

Here, the produced artifact is the exact canonical package artifact whose
claims are being checked, not necessarily a final executable. A claim about
native lowering or final realization additionally binds the corresponding final
artifact and Terminal evidence under the ordinary hardened rules.

The compiler derives the ordinary admission baseline from the earliest coherent
compiler-owned representation in which each evidence fact is semantically
established, through a total internal package-admission projection. Different
rows may use different private representations; the lock stores only the final
versioned canonical rows. This does not make any internal IR a public
compatibility surface or require another nominal IR stage.
The projector may use private pre-Psi typed or resolved structure when that is
where an exact identity is semantically established, then join checked
acceptance, effects, proofs, and realization from the stage that establishes
them after successful compilation. Terminal Psi evidence is additional and is
required only for claims about final realization or by a hardened profile;
absence of that evidence never implies a weaker Terminal
guarantee. A new named stage is warranted only by a reusable semantic boundary,
not by package-report format stability.

Compiler-issued package review also retains a separate commitment to the exact
reconciled package/alias graph and source bytes consumed by the frontend.
Absolute cache locations and load order are not identity. This commitment
changes on a source-only edit without asserting that the normalized public
capability/API contract changed.

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

Selecting the target also closes symbolic target-semantic observations and
target-scoped realization applications used by constants, proofs, plans, or
const-indexed types. Those dependencies are part of the normalized public
signature when they escape a package. Independently closed artifacts must agree
on them; adding, removing, or changing one in a public signature is a breaking
semantic-API revision. A private dependency instead invalidates the target
artifact without changing the public contract. Folding a target observation to
an integer never erases its dependency or diagnostic origin.

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

### Declaration selection and carried foreign types

A direct dependency authorizes authored source to select declarations owned by
that package. This includes static paths and declarations selected through an
inferred receiver: fields, cases, methods, operators, conformances, and an
ordinary explicitly named consuming call all retain their declaring package.
Compiler-selected automatic `T::drop` is carried type semantics; whether source
may invoke that reserved machine is not yet part of the settled ownership rule.
A package absent from the requester's direct dependency set cannot be selected
by hiding its name behind a value whose type was inferred.

Nominal identity may nevertheless flow through another package's API without
granting that selection authority:

```omega
let handle = filesystem::open(path);   // inferred lower-package handle type
filesystem::read(&handle);
filesystem::close(handle);
```

The caller may move, borrow, store, return, and pass the value through declared
dependencies. This remains legal for copyable, affine, and linear values;
multiplicity is checked from the carried type contract and does not create a
source dependency. Compiler-planned layout, move/copy behavior, and automatic
affine cleanup likewise travel with the type. An authored call to the owning
package's operation is different and requires that package as a direct
dependency.

The foreign nominal identity is never hidden from artifacts. The transitive
lock closure retains its owning package, and semantic dependency records retain
the exact declaration when available. A private occurrence affects rebuild and
artifact identity; an occurrence in a public signature also affects public
compatibility identity. Whole-package dependency keying is a sound conservative
implementation because it only over-rejects reuse; exact declaration edges are
the normative form.

The compiler retains authored selection occurrences while source spans and
public-versus-private position are still exact, then joins each occurrence to
its final selected declaration after successful checking. Static paths and
ordinary members may settle during resolution; receiver-dispatched calls,
overloads, operators, and inferred conformances may settle later. This is one
compiler-internal ledger finalized from the stages that own those facts, not a
new language-visible IR stage. An unresolved or unjoinable authored occurrence
rejects rather than disappearing from the gate.

Expressions owned by a public declaration's published contract or predicate
are public-interface selections. This includes public machine contracts and
ranking expressions, public data/domain predicates, and public trait contracts.
Executable machine states and bodies remain private implementation even when
the machine is public. A membership fact selects its domain declaration; its
value parameter or local is a lexical place and does not become a package row.
Nested declaration families inherit public exposure only after their source
visibility rule says so.

Generic conformance bounds apply the same distinction. Their subject and
evidence binder are lexical; the right-hand trait, or both declarations in a
qualified `Carrier::Evidence` bound, are authored selections. Bounds on public
machines and traits are public-interface selections, while bounds on private
declarations remain private implementation.

A Unit-producing or explicitly discarded call statement follows exactly the
same rule as a value-producing call expression. Its target token selects the
callee declaration, each explicit static conformance argument selects its own
declaration, and a uniquely inferred generic conformance is attributed to the
call token. Compiler-owned build markers and lowered assembly operations retain
closed intrinsic meanings instead of fictional package owners.

Every explicit static argument path also selects its declaration, recursively
through nested static applications. Conformance paths remain evidence
selections; type, static-machine, and forwarded-binder paths use the common
static-argument category while retaining their exact symbol. Integer literals
select no declaration. Named const reduction retains the selected const's
provenance separately, so erasing its value does not erase dependency custody.

Only authored selection rows are checked against the direct dependency set.
Carried nominal identity, compiler-planned layout and move/copy behavior, and
automatic cleanup produce semantic dependency evidence but never manufacture
authored selection authority. No package or build-time code selected by such an
occurrence may execute before the finalized selection gate succeeds.

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

Omega has no `export` item. `pub` exposes declarations owned by the current
package; it does not relabel dependency-owned identity. A package presents
dependency behavior under its own API through an ordinary public wrapper. A
future ownership-preserving path alias would be name presentation only, never
visibility widening or an implicit dependency edge. `export` is not reserved
and remains available as an ordinary identifier where an identifier is expected.

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
