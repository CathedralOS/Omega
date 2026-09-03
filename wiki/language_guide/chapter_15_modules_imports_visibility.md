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

The package declares its human name once through ordinary build vocabulary, on
the same `Build` surface that carries dependencies and provider selection:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

Every `build.omg` states its kind explicitly — `builder.package` for a package,
`builder.member` for a workspace root, `builder.application` for an
application. No role is inferred from an absent declaration.

The selected manifest has exactly one free
`machine build(builder: &mut Build)` entry. A scoped `Owner::build` is never a
project root; its owner name proves neither identity nor authority, and the
compiler does not synthesize its receiver. The same declaration in ordinary
source is simply an ordinary machine.

The free root may lend `&mut Build` to ordinary helpers for evaluated
composition. Their transitive checked contracts are charged to the root.
Project role, workspace membership, and dependency requests remain direct
statically projected root statements and cannot be hidden in helpers.

The package manager statically projects this declaration from parsed source
before dependency resolution or build execution; it does not execute the build
machine to discover the graph. Graph-forming calls such as `package`, `member`,
and `depend` therefore use a closed, directly projectable form. Arbitrary build
control flow cannot hide a dependency edge. The directory and repository names
do not establish identity. The declared name is qualified by canonical source lineage
to form the stable `PackageKey` used by locks and nominal symbols; exact source
content, produced artifact identity, per-subject obligation-semantics identity,
re-derived verification results, and disclosed open assumptions form a
`PackageInstance`. A same-spelled package or boundary from another lineage is
therefore a different identity. Explicit compiler/toolchain semantic, build,
and encoding identities remain separately labeled only where a concrete
reproduction, compatibility, or deployment claim consumes them. Bytes read
through the running process's executable pathname are never package review,
lock, cache, or admission identity and never seal the instance.

Packages and applications use the same `PackageKey`: declared name plus source
lineage. Role remains an explicit companion fact. A selected root may be either
role and may own dependencies, but every dependency edge must resolve to a
package; applications are not importable libraries. The resolver enforces that
single root/non-root rule for every source kind and retains the root role in
lock, review, and compiler-handoff evidence. Role is never inferred from an
entry binding and does not alter the key.

A workspace is a catalog rather than a combined dependency graph. Multiple
application members are multiple independently selectable roots. Selecting one
does not pull in the others or package members it cannot reach.

Dependencies are unconditional, directly projectable build rows. Omega has no
graph-forming branch on `builder.target`, `depend_when`, `depend_as_when`, or
string condition. The package manager never executes or statically interprets
build-machine control flow to discover package edges, and the workspace lock
records the one declared dependency set.

The compiler invocation owns exact target identity. Authored `target X { }`
declarations are retired and do not define application support. Target-specific
host and boundary policy is immutable compiler/package input rather than source
activation. The build machine owns roots, dependencies, generated outputs,
subsystem/image facts, and provider selections. Applications bind entries with
flat unconditional `roots.bind(target::ProgramEntry, entry)` rows; selecting an
exact invocation target selects the corresponding entry.

One invocation may request one exact target or a nonempty explicit set. The
compiler reuses target-independent immutable stages and forks at target-
sensitive checking and realization. Each target child remains identical to a
standalone exact-target invocation, and no unresolved target branch enters one
Terminal Psi subject. The caller supplies the set: `all`, wildcards, source or
dependency inference, and compiler-catalog enumeration are invalid. A batch
manifest records only the requested children and their independent outcomes;
it is not application-support or tested-target evidence.

`PackageName` is not globally unique. For Git, `PackageKey` lineage identifies
the canonical repository namespace and
does **not** include the requested revision, resolved commit, tree, or content.
Those exact values belong to `PackageInstance`. Consequently, two revisions of
the same declared package collide on one key and must reconcile rather than
silently becoming two nominal universes.

Packages expose public data, machines, traits, domains, wire schemas, and
boundary surfaces.

A package's dependencies — the external packages it may reach — are declared in
its **`build.omg`**, a capability-checked build-entry machine that augments a
`Build` (see
[`../design_briefs/build_and_package_model.md`](../design_briefs/build_and_package_model.md)).
Each dependency row requests a source and update selector. After fetching it,
Omega reads the dependency's own `builder.package("name")` declaration and
derives the default local alias
by mapping kebab-case to snake_case. Explicit aliases are exceptional local
renames and never package identity.

A Git request has two independent coordinates: acquisition identifies the
repository and revision to fetch, while package selection is explicitly
`Root` or `Named(PackageName)`. Omitted source spelling normalizes immediately
to `Root`; absence is not retained in locks or evidence. `Named` selection
content-verifies the repository root, statically projects its declared workspace
members, and requires exactly one member whose own `builder.package` declaration
matches. Recursive `build.omg` search, caller-authored member paths, escaping
members, and duplicate names reject.

The resolved member path is retained as navigation and replay custody, including
the base for that member's relative dependencies. It is not package identity:
moving a member inside the same repository lineage preserves its `PackageKey`.
The canonical resolved-source question still binds that navigation, so a move
is visible to lock/review reconstruction. Relative dependencies inside a fetched
Git member may target only another exact member declared by the content-verified
repository root; recursive discovery and undeclared directories reject.
Different requesters may use different local aliases for that key, and a parent
cannot rename aliases internal to a child. A consumer that names a dependency's
declaration needs its own dependency edge and alias; one that merely carries an
inferred opaque value through another package's API need not name that package.

After reconciliation, the compiler receives the complete requester-local alias
graph together with an opaque commitment to each `PackageKey` and each
resolver-owned source root. It does not rediscover dependencies from package
build code. The commitment, not the cache path, governs same-package and future
nominal-identity checks; source roots only constrain where imports may load.

`omega.lock` is machine-written accepted state: it records the reconciled
closure, exact commits/trees/content, source-qualified package identities,
compiler-derived capability/API baselines, representation-TCB availability and
demand rows, build observations, and admission evidence. The current standalone
trust-receipt subset stores full domain-separated admission digests; a legacy
compact-only row cannot authorize a build and must be explicitly re-accepted.
Claim-free opaque boundary representation remains visible and audit-recommended
without being mislabeled as an accepted claim. Producer availability accepts no
consumer choice. A consumer-owned demanded by-value row retains the exact
selected or target-derived
representation application and its strong calling-plan commitment; `Unbound`
is complete only when no active by-value use needs one. Chapter 19 defines the
distinction. The compiler consumes the lock rather than silently resolving
mutable selectors. The lock should normally be
committed; source caches may be ignored. The first implementation performs no
semantic-version solving. Requests for one `PackageKey` that resolve to the same
immutable source instance deduplicate, even when their request spellings differ;
different immutable resolutions reject with every conflicting dependency path.
There is no intermediate "compatible version" relation. Multiple simultaneous
instances of one key are unsupported: adding them would require nominal types,
conformances, provider selections, and evidence rows to be qualified by package
instance rather than package key, not merely a second local alias.

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
every disclosed admission before creating accepted lock state. Accepted locks
are exact-current generated artifacts: a semantic-schema mismatch forces
complete local reconstruction and fresh admission. Current admission has no
schema-migration registry or compatibility classifier and never reuses old
discharge or policy decisions.

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
not by package-report format stability. Psi may repeat an invariant as a
downstream compiler check without forcing package admission to reconstruct an
already-settled fact from Psi.

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
Compiler-selected automatic `T::drop` is carried type semantics. The exact
owner-attached hook is compiler-only and authored selection of it rejects;
source code ends a lifetime early through the ordinary consuming
`omega::core::drop(value)` machine or an owner-published protocol operation.
A package absent from the requester's direct dependency set cannot be selected
by hiding its name behind a value whose type was inferred.

The carrier in an attached declaration head is itself a declaration selection:
`machine Data::operation` names the exact `Data` declaration as well as
declaring the independently visible machine. Qualifying the machine does not
inherit the carrier's visibility or make a transitively owned carrier directly
nameable.

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
lock closure retains its owning package. After successful checking, a
package-neutral semantic-dependency sidecar retains exact declarations carried
through machine heads, checked call results, ownership places, and automatic
cleanup. A private occurrence affects rebuild and artifact identity; an
occurrence in a public signature also affects public compatibility identity.
Whole-package dependency keying remains a sound conservative gate while the
compiler-private rows are qualified and encoded for package evidence; exact
declaration edges are the normative form.

The compiler retains authored selection occurrences while source spans and
public-versus-private position are still exact, then joins each occurrence to
its final selected declaration after successful checking. Static paths and
ordinary members may settle during resolution; receiver-dispatched calls,
overloads, operators, and inferred conformances may settle later. This is one
compiler-internal ledger finalized from the stages that own those facts, not a
new language-visible IR stage. An unresolved or unjoinable authored occurrence
rejects rather than disappearing from the gate.

One source token has one occurrence identity even when compiler normalization
copies its expression. All retained copies carry that occurrence, provisional
targets are reconciled to one exact declaration, and conflicting resolved
targets reject. Compiler-owned vocabulary such as collection views and length
is finalized as a closed intrinsic identity rather than by its spelling. A
private ranking witness likewise retains its exact typed expression roots;
termination checking and package custody do not rediscover those roots from
rendered witness text.

Expressions owned by a public declaration's published contract or predicate
are public-interface selections. This includes public machine contracts,
public data/domain predicates, and public trait contracts. Executable machine
states and bodies remain private implementation even when the machine is
public. A `terminates by` ranking expression is likewise private proof evidence:
`terminates` is the public promise, while the rank is how the implementation
discharges it. A membership fact selects its domain declaration; its value
parameter or local is a lexical place and does not become a package row.

Every declaration selected from a public-interface position must itself be
publicly visible. The compiler rejects a public contract or predicate that
names a private declaration rather than silently promoting the target.
For a reviewed nominal member expression, the authored member token and the
checked semantic place or call-result projection must also select the same
exact field. The token cannot disappear merely because the checked expression
already carries a structurally representable receiver and field path.
This applies equally when the receiver is a computed nominal value, including
a record or case constructor inside a transparent public proposition. The
constructor type, every authored constructor field, and the selected result
field retain their independent exact declaration selections; review recurses
through the receiver value and rejoins the member token to its finalized field.

Generic conformance bounds apply the same distinction. Their subject and
evidence binder are lexical; the right-hand trait, or both declarations in a
qualified `Carrier::Evidence` bound, are authored selections. Bounds on public
machines and traits are public-interface selections, while bounds on private
declarations remain private implementation.

A complete name-first conformance owns ordinary declaration visibility. It is
package-private unless marked `pub`; visibility is inherited from neither its
subject nor its trait. Public-interface citation and authored selection from a
direct dependent require `pub`. The conformance's normalized public surface may
retain private member-machine and proof identities because callers select the
authorized row map rather than those implementations.

An exact `machine ... satisfies Requirement` edge is not a standalone
conformance declaration and follows the machine's visibility. Its target may
be a trait/operator requirement or an explicit top-level `boundary
requirement`. Its optional
`as Name` label groups requirement-local satisfiers but does not create a
package-level selectable declaration. The edge nevertheless authors two exact
declaration selections: the trait and its overload-resolved requirement. An
operator requirement similarly selects the exact signature-matched operator.
All selected coordinates require direct dependency authority and must be public when
the realizing machine publishes an interface, including boundary or accepted
supply not separately spelled `pub`. Selection identity is settled before
supply policy: an inadmissible external realization does not erase or replace
the declaration it attempted to realize. Conversely, a value may carry a private
dynamic conformance selected by its producer without granting the receiver
authority to name or select that conformance elsewhere. Carrying compiler-
selected semantics is not authored declaration selection.

A domain's `established by Trait::requirement` entry applies the same rule
directly at the domain declaration. It selects the exact trait and the one
signature-free requirement, and both selections inherit the domain's
visibility. Each comma-separated or repeated authored route remains a source
occurrence even when equivalent semantic alternatives normalize to one route.
A public domain therefore cannot authorize a private trait or requirement;
private same-package domains may use private routes normally.

A nominal callable machine-parameter contract such as `where machine Selected
satisfies Trait::requirement`, and an exact realization spelled `satisfies
Trait<...>::requirement`, likewise author both exact selections. The complete
trait application and requirement token inherit the enclosing declaration's
interface exposure, including exported boundary machines, and each selected
declaration must be directly authorized and visible there. On an exact
realization, raw machine-lifetime binders remain checking custody while the
public edge exposes only their normalized equality partition. Nested machine-
parameter contracts follow the same rule; generic nesting does not hide a
transitive or private requirement.

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
select no declaration. A named const in a static-argument lane selects its
exact const declaration directly; package review rejoins that symbol to the
declaration's canonical value. Named const reduction in ordinary expression
position retains the selected const's provenance separately, so erasing its
value does not erase dependency custody.

Only authored selection rows are checked against the direct dependency set.
Carried nominal identity, compiler-planned layout and move/copy behavior, and
automatic cleanup produce semantic dependency evidence but never manufacture
authored selection authority. No package or build-time code selected by such an
occurrence may execute before the finalized selection gate succeeds.

Package review qualifies each carried dependency by the exact package owning
the consuming machine and the exact package owning the declaration. Nominal,
layout, ownership, and automatic-cleanup dependencies are blocking comparison
rows; whether the occurrence is private implementation or public interface is
part of the compared row. This records artifact/API consequences without
making a transitive package source-nameable.

The checked carrier retains an automatic cleanup machine by its exact
attachment to the nominal declaration. A package-controlled machine with the
same trailing `drop` spelling on another type cannot become that dependency.
The attachment grants no authored source authority: only compiler-planned
cleanup may select it, including cleanup reached through an erased owner's
exact descriptor.

## Visibility

Declarations are private by default unless marked `pub`. Independently
nameable data, domains, traits, machines, top-level boundary requirements, wire schemas, operators,
propositions, and constants support that rule. A declared ranking measure is
private proof machinery for `terminates by`; the parser rejects `pub measure`.
Complete name-first conformances follow the same rule: they are private unless
marked `pub`, independently of their subject and trait. An exact requirement
edge's optional `as Name` label is not a standalone conformance declaration and
acquires no package visibility. Qualification and direct dependency never
grant an implicit exception.

Qualification does not imply visibility inheritance. A declaration such as
`Extent::Granted`, `[u8]::Utf8`, `Vector::add`, or a type-qualified constant is
a standalone declaration with its own visibility, even though its path names a
carrier. Only a genuine nested member with one exact semantic owner inherits
that owner's visibility, such as a field, variant, state, or trait requirement.

```omega
pub data Player {
    health: i32;
}

pub proposition valid_damage(amount: i32) = amount >= 0;

measure Tree::Height(node: &Tree) -> Nat;

pub const MAX_DAMAGE: i32 = 100;

pub machine Player::take_damage(
    &mut self,
    amount: i32
)
requires valid_damage(amount)
{
    self.health = self.health - amount;
}
```

Visibility is a source-level API boundary. It does not bypass proof,
ownership, or boundary checks.

A bodyless `pub proposition related(a: T, b: T);` publishes the proposition
family's vocabulary. It does not assert any application of that family and
does not create an admission. Evidence-producing `requires`, `ensures`, and
boundary edges remain the places where proposition instances are assumed,
proved, or admitted.

Compiler intrinsics are a separate closed selection category. Their
availability is fixed by the language/toolchain and cannot be acquired by a
package declaring a public lookalike.
An authored intrinsic still has source custody. In particular, each `!` or `~`
token retains one exact operator-selection occurrence and the enclosing
declaration determines whether that occurrence is public-interface or private-
implementation use. Package review accepts the structural unary meaning only
after checked lowering rejoins that occurrence to the compiler-owned builtin;
a nested unary expression cannot disappear behind its enclosing binary fact.

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

This is deliberate for core's public linear `Extent`: its fields publish only
range geometry. `Extent::Granted` is the routed authority, and an admitted root
provider decides whether arbitrary caller-constructed geometry receives that
qualification. Linearity tracks the occurrence; it does not make the record
literal an authority mint.

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
