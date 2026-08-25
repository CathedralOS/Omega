# Design Brief: Package Manager First Draft

Status: corrected first design, 2026-08-24. This brief is temporary until the
implementation vocabulary is established and the settled model is folded into
`build_and_package_model.md`.

## Intent

Omega needs a Cargo-like source workflow without a hosted registry and without
ambient trust. It resolves user-named Git, URL, or local sources to immutable
content, discovers package identity from the fetched package, derives security
evidence with the compiler, reconciles the complete closure, and admits the
result before changing project or lock state.

The package manager does not accept package-authored capability manifests or
caller-authored package identities as evidence.

The security invariants in this brief are firmer than its implementation
vocabulary. The implementation should reuse the smallest coherent Omega
mechanism that proves the required property and collapse provisional
distinctions when experience shows that one existing mechanism is sufficient.
For example, package/build arithmetic should use ordinary Omega arithmetic and
may use `Exact` throughout when its actual obligations are provable. Likewise,
explicit build-host authority and checked reach are required, but this brief
does not require every one-purpose tool service to become a new public boundary
trait. Concrete fixtures should decide whether an ordinary machine/provider,
an existing boundary, or a narrower toolchain-owned operation is the simplest
honest surface.

## Package declaration and identity

Every package declares its own human name in its `build.omg` through one
well-known, hermetically evaluated constant:

```omega
const PACKAGE: Package = Package {
    name: "arithmetic-kernels"
};

machine build(builder: &mut Build) {
}
```

This uses ordinary `const` and data syntax. `Package` is toolchain-provided
build vocabulary, not a new grammar form. Omega extracts the declaration before
executing `build`, resolving imports, or supplying build-host services. The
declaration must be unique, compile-time evaluable, effect-free, independent of
dependencies and generated files, and use canonical kebab-case spelling.
Canonical spelling begins with an ASCII lowercase letter, contains only
lowercase ASCII letters, digits, and single hyphen separators, and therefore
maps mechanically to a valid snake-case Omega alias.

Three identities remain deliberately separate:

- `PackageName` is the package-authored human name, such as
  `arithmetic-kernels`.
- `PackageKey` joins that name to canonical source lineage. It is the stable
  graph, lock, and nominal-symbol identity across updates.
- `PackageInstance` joins the key to exact source content, produced artifact
  identity, each closure subject's obligation-semantics identity, locally
  re-derived discharge results, and disclosed open assumptions. Exact
  certificate routes and compiler/toolchain identity remain derivation and
  review provenance rather than semantic authority.

For Git, source lineage normalizes transport spellings only when a resolver
adapter can establish that they designate the same repository namespace. A
matching host/path is not universal proof that HTTPS and SSH serve the same
repository. Unknown equivalence remains distinct. Exact commit, tree, and
content identities remain instance evidence. A different lineage or declared
name is package replacement, not an ordinary update. Mirrors require explicit
relocation/delegation evidence; a matching declared name is never sufficient.

Workspace path packages use the workspace source lineage plus normalized
member-relative path. Paths outside the workspace are explicitly non-portable
development sources scoped to the consuming lock and cannot satisfy a
source-rebuildable release profile. Resolution of an explicitly selected
external-local root carries one consuming context through its recursive
relative or absolute local Path closure; each package retains its own canonical
absolute lineage and immutable snapshot. The resolver does not discover a
parent workspace or lock from the ambient filesystem. A live workspace may
route an escaping Path request into this lane only when its caller supplies the
same consuming context explicitly; context-free workspace traversal and all
fetched Git snapshots remain confined. Archive and future
protocol adapters must define their own canonical lineage and immutable-content
receipt; an unknown
URL is never guessed to be Git or delegated to an ambient protocol helper.

The implementation normalizes GitHub's and hosted GitLab's established HTTPS
and SSH repository namespaces. GitLab nested namespace paths remain exact and
case-sensitive in lineage. Self-hosted and other Git hosts retain transport,
user, port, path case, and suffix distinctions until a host adapter can prove
more; conservative duplication is preferable to false package identity.

Canonical symbol and boundary identities include `PackageKey`. A package that
declares a lookalike trait or package name therefore cannot impersonate an
admitted boundary owned by another source lineage.

## Authored dependency requests

`build.omg` names sources, not externally asserted package identities. The
target ordinary-library shape is:

```omega
machine build(builder: &mut Build) {
    builder.depend(Source::Git {
        repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
        revision: "main"
    });
}
```

The resolver fetches the source and obtains its name from that package's own
`PACKAGE`. The default in-code alias is the mechanical kebab-to-snake mapping,
`arithmetic-kernels` to `arithmetic_kernels`. Only a genuine local collision
or deliberate rename uses the exceptional `builder.depend_as(alias, source)`
form. The alias is local name resolution only and never contributes security
identity.

The first command surface is consequently:

```text
omega install <source> [--rev <revision>] [--as <alias>]
omega update [package-or-alias...] [--to <revision>]
omega audit packages
```

The CLI may conservatively edit only canonical direct dependency rows. For a
more elaborate `build.omg`, it emits a proposed source patch and performs no
mutation.

Direct rows authorize authored selection of declarations from that package;
they are not required merely to carry an inferred foreign nominal type returned
through another declared dependency. Moving, borrowing, storing, returning, or
passing such a value back through the visible surface does not make its owner
source-nameable. Selecting an owner-declared field, case, method, operator,
conformance, or ordinary explicit consuming machine does and requires a direct
row. Reserved `T::drop` remains automatic carried semantics unless and until
the language defines an authored invocation and its ownership event.
Compiler-planned layout, multiplicity, and automatic cleanup remain carried
type semantics.

There is no `export` item. `pub` exposes only package-owned declarations, and an
ordinary public wrapper presents dependency behavior without relabeling the
dependency's nominal identity. The accepted lock still retains the complete
transitive closure. Compiler evidence ultimately records exact declaration
dependencies, distinguishing private artifact/rebuild edges from dependencies
that enter public compatibility identity; whole-package keying is a sound
conservative implementation until then.

The compiler captures a package-agnostic ledger of authored selection
occurrences during resolution, while exact source spans and public/private
position remain available, and finalizes it after successful checking. The
final join supplies late-bound method, overload, operator, and conformance
identities from the semantic stage that settles each one. This internal sidecar
is deliberately not nominal Chi. Every authored occurrence must ultimately
finalize to a known declaration or reject. Selected package code cannot run
before the finalized ledger passes; earlier effect-free compiler evaluation
must first admit an exact early target or fail closed unless the compiler can
confine the complete candidate set to admitted owners.

Public/private disposition follows the declaration-owned source position.
Public machine contracts and ranking expressions, public data/domain
predicates, and public trait contracts are public interface; executable states
and bodies are private implementation. Proof-membership custody includes the
selected domain path, not the lexical value parameter. Nested declaration
visibility is not inferred while its owner rule remains unsettled.

In a generic conformance bound, the subject and optional evidence binder are
lexical. The right-hand trait is authored declaration authority, and a
qualified `Carrier::Evidence` bound selects both exact declarations. Bounds on
machines and traits take the enclosing declaration's public/private
disposition; declaration publication remains a separate visibility rule.

Carried nominal types, automatic cleanup, and compiler-derived layout and
move/copy behavior are collected separately as exact semantic dependencies.
They enter private artifact identity or public compatibility identity according
to where they occur, but do not participate in the authored direct-dependency
gate and do not make the owning package source-nameable.

The first implementation carrier lives in checked flow. It joins machine-head
types, exact checked call-result targets, and ownership places after successful
checking, retaining nominal identity, layout, ownership behavior, automatic
cleanup, and the exact attached cleanup-machine declaration. Public occurrences
promote an otherwise private row. A root-middle-leaf canary confirms that this
evidence does not turn transitive carried identity into authored source
authority. The carrier remains package-neutral compiler state; canonical owner
qualification and evidence encoding occur in the package projection, not by
persisting checked-tree handles or introducing Chi. The current review
projection emits one blocking row keyed by exact consumer, exact dependency,
and dependency kind, with private/public exposure as the compared value and
source anchors for both declarations. Accepted-lock issuance still requires
the total admission projection.

## Dependency planning before build execution

Dependency-source projection must be hermetic even though later build staging
may use admitted host services. Dependency rows cannot depend on filesystem or
network observations, clocks, generated files, imported code, or package build
outputs. The initial implementation may accept only direct canonical rows; a
later implementation may evaluate a broader compile-time-admissible projection.

Resolution and admission proceed in this order:

1. Resolve and fetch source under resolver-owned authority.
2. Extract the package declaration hermetically.
3. Extract its hermetic dependency-source projection.
4. Recursively resolve the complete source closure.
5. Type-check the closure and derive static build/runtime reach.
6. Stop for admission before supplying any suspect build-host provider.
7. Execute `build.omg` with package-scoped admitted providers only.
8. Compile generated Omega source as ordinary source.
9. Emit final compiler-derived package evidence and reconcile the closure.
10. Mutate `build.omg` and `omega.lock` only after admission succeeds.

Downloaded code never receives resolver fetch/archive authority, the root
package's providers, or authority to alter its own dependency graph during
build execution.

Implemented package-review sequencing freezes the resolved source closure and
runs a complete checked preflight over the ordinary graph. Package-aware
checked and native compilation reject unresolved or unauthorized authored
selections before the selected build machine can execute. The build prepass
then executes exactly once and joins only explicit Output-rooted
`include_source` handoffs to retained staged-tree bytes. Those bytes receive
ordinary final parsing, resolution, typing, checking, and the repeated
selection gate without rerunning dependency discovery or build execution. The
native-image command remains gated until it consumes the same sponsored package
transaction.

Psi's target-neutral const-generic, fixed-array, const-domain, laid/placed
layout, wire-policy, and calling-policy evaluators consume the same reconciled
direct-dependency authority through a package-neutral compiler interface. Each
invocation retains exact source or symbol custody. Before evaluation, the gate
admits the authored caller-to-callee edge, walks the concrete build-time call
closure, and admits every direct call edge and declaration selection in each
reachable body. Shared policy results are computed only after all authored
application sites pass. A selection not yet settled by later checking rejects
unless its complete compiler-derived candidate set is confined to toolchain,
self, or direct dependencies; operators use the checked layer's conservative
intrinsic classification rather than spelling. Substituted const values retain
a provenance-only declaration symbol and occurrence, so erasure does not erase
package custody.

This gate intentionally reads the earliest coherent private typed/probe state
where those facts exist. It is compiler-internal and may move with the compiler;
it neither waits for Terminal Psi nor creates nominal Chi. A distinct stage is
warranted only by a real shared invariant, transformation boundary, or
independent consumer. Production package mutation remains disabled for the
remaining public-expression and authored conformance/cleanup coverage.

Nominal type spellings now enter the same ledger at symbol-resolved-to-typed
lowering, where both the exact symbol and declaration exposure are coherent.
Type references in public data, domains, machine-head signatures, traits, and
wire surfaces are public-interface selections; private declarations, internal
state signatures, local type annotations, casts, and a public machine's owned
storage are private-implementation selections. Generic bases and explicitly named dynamic-trait
conformances follow the same rule. Binders, locals, primitive types, and
source-free compiler nodes are not package selections. This closes explicit
nominal type custody, including a public API's direct-dependency gate, without
claiming the separate carried-semantic-dependency projection is complete.

Source-backed static conformance arguments are likewise authored selections:
`choose<Card, Ascending>(...)` records `Ascending` at that argument's token,
including when the argument is a nested static application. If a generic
`where Element satisfies Trait` bound has no explicit evidence argument, the
checker must retain the exact unique conformance it selected while validating
the specialization; counting candidates and discarding the winner is
insufficient. That inferred declaration is fingerprinted with package-qualified
identity and attached to the authored call occurrence. Trait-backed operator
tokens retain their checked selected conformance independently. Thus explicit,
inferred-call, and operator selections remain distinguishable review rows, and
each requires direct authority from the package containing its own source
token. Root-middle-leaf canaries enforce this even when ordinary type checking
would otherwise find the transitive conformance.

Void/discarding statement calls are not a second authority model. Resolution
records an exact statement target when available and otherwise retains a
checked-call obligation at the authored target span. Checked flow supplies the
late target and any unique generic conformance inferred for that call. Static
conformance arguments remain independent rows at their own argument spans.
Compiler-owned build markers and lowered inline-assembly operations finalize as
closed intrinsic meanings; they neither invent a package symbol nor disappear
from the ledger. Ordinary statement calls still require the selected
declaration's owner as a direct dependency.

All source-backed static argument paths are declaration selections too,
including recursively nested applications. Explicit conformance evidence keeps
its dedicated kind; type, static-machine, and forwarded-binder paths share a
static-argument kind while retaining the exact selected symbol. Integer
literals select nothing. A named const that is reduced before ordinary
resolution retains its const-declaration provenance through the existing
substitution row. Unresolved static paths remain explicit obligations and fail
closed at package admission.

## Authored requests versus accepted lock state

`build.omg` records update intent: source locator, revision selector, explicit
alias override, targets, roots, providers, and build orchestration. `omega.lock`
records the accepted resolution: exact commits/trees/content, `PackageKey` and
`PackageInstance`, dependency closure, per-subject obligation-semantics and
evidence-schema identity, exact certificate provenance, normalized capability
baseline, transitive open obligations, build observations, and policy-resolution
references. Compiler and toolchain identifiers remain separately labeled
review metadata that supports reproduction and cache partitioning; they do not
authorize truth or prove that anyone audited it.

The compiler always builds from the lock and never silently re-resolves a
mutable selector. `omega.lock` is generated but should normally be committed;
source caches and expanded artifacts may be ignored. A fingerprint alone is
not an admission baseline: the lock must embed the normalized accepted security
projection or retain a mandatory content-addressed copy.

The first resolver does not solve semantic-version ranges. Requests for the
same `PackageKey` must reconcile to one immutable instance or fail with every
conflicting dependency path. Multiple-version composition is a later explicit
feature. Package dependency cycles reject in v1, keeping build order and
request-path provenance finite; supporting a cycle later requires an explicit
semantic and custody model rather than accidental graph acceptance.

The compiler handoff contains the reconciled root package, one opaque stable
identity plus canonical source root per package, and requester-local alias
edges between those identities. Package-aware compilation validates that
closed graph again and never combines it with `build.omg` scanning. Canonical
paths are import-custody locations only; the opaque `PackageKey` commitment is
the semantic identity that survives source loading.

## Compiler-derived package evidence

Package capabilities are derived from the candidate repository after the
complete source/build closure is available. The package author writes ordinary
Omega contracts; it cannot author or patch the admission manifest.

Ordinary admission derives from coherent compiler-owned semantic state after
successful checking through a total internal `PackageAdmissionProjection`.
Individual rows may draw from different private representations, as described
below. The projection is not a
new public IR or execution stage: it owns no transformations or independent
semantics. It normalizes only package-visible semantic identities and evidence
rows, rejects any required fact that is unresolved or cannot be projected, and
emits a versioned canonical evidence encoding. Locks persist that encoding, not
raw checked-tree nodes, arena handles, display strings, or compiler-private IDs.
Compiler internals may change freely provided the projection remains equivalent
or the evidence schema changes explicitly.

The compiler-issued review envelope separately commits to its canonical
reconciled package/alias graph and every exact package or toolchain source path
and byte sequence retained by the frontend. Absolute custody locations and load
order do not enter that commitment. This source-consumption identity is not a
capability/API comparison row: source-only changes alter it without pretending
the normalized public contract changed. Resolver custody retains immutable
source resolutions independently and verifies both whole snapshots and the
compiler-retained bytes around compilation.

The envelope also retains a separate compiler-executable commitment. Package
orchestration derives it from the bytes readable at the current producer's
executable path before reviewing the closure, derives it again after review,
and rejects a changed observation. Every review row from that operation carries
the same verified commitment. It is provenance, not capability/API comparison
material, and it neither certifies the compiler, identifies the compiler's
source closure, nor proves that the observed file is exactly the process image
already loaded by the operating system. Complete compiler/toolchain source and
rebuild provenance remain admission work.

Ratified 2026-08-24: implementation should consume the earliest coherent
compiler-owned representation in which each required fact is semantically
settled. Exact structural identity may come from private pre-Psi typed or
resolved state, while checked acceptance, effects, proofs, and realization come
from the stage that establishes them. The projector joins those facts only
after successful checking. Different evidence rows may therefore come from
different internal representations; totality belongs to the final projection,
not to one frozen source stage. Because the projection ships with the compiler,
depending on compiler-internal representations is ordinary internal coupling,
not a promise that those representations are stable public APIs. Unchecked
syntax, diagnostic renderings, and convenient-but-unsettled shapes are not
admission evidence. The projection and its tests move with the representations.

There is no nominal Chi stage merely to collect or stabilize this report. A
distinct IR is justified later only if multiple independent consumers need the
same semantic boundary or it acquires its own transformations, invariants, and
verification rules. Implementation discovery may also collapse rows into an
existing coherent representation, including `Exact`, when that removes
machinery without losing semantic distinctions.

Conflict explanation follows the same ownership rule. The compiler attaches
canonical package-relative UTF-8 paths and exact byte spans to its canonical
rows, separately from semantic row bytes. Declaration movement therefore does
not create a capability change. When a row does change, its old/new explanatory
coordinates are included in the conflict fingerprint and escaped bounded
rendering. Dangerous-authority rows identify both the canonical toolchain
declaration and each reviewed package callable exposing it. Generated symbols
follow their authored derivation origin; genuinely compiler-derived rows carry
a closed reason. Ordinary rows retain exact declaration symbols through
canonical sorting, and dangerous-authority derivation retains both the exact
service declaration and exact exposing callable symbols; no later source join
reconstructs them from reduced nominal identity. Provider candidate derivation
captures a compiler-internal
sidecar beside each semantic plan: exact schema and optional nominal-provider
symbols, plus the exact requirement and realizing machine for every external or
checked-adapter row. Review v41 encodes those declarations as exact
package-qualified nominal identities; readable plan and overload strings remain
operational/audit data. Projection verifies each declaration against the
selected plan's exact package owner, or against an exact authored
toolchain-source identity when the plan carries no package owner. Package-less
user source, unresolved/source-free ownership, and owner drift reject.
Explanatory source custody records every exact requirement declaration
separately from its realizing machine, so the reviewer receives both sides of
each provider row.
Selection and sorting preserve the pair and add exact
authored build/target-default call sites or a closed implicit-selection reason.
The selected-provider row may therefore mix authored coordinates and compiler-
derived reasons without reconstructing them from reduced names, schemas, or
fingerprints. Exact nested
use sites may be added through their existing typed/checked owners and compiler
sidecars without creating a report-only stage.

Proposition and named-evidence rows apply that rule as an explicit join. The
typed application owns the structural proposition declaration, binder
arguments, and ordinary value-expression arguments. Checked proof state owns
whether the application was accepted, how an evidence term or witness
interface is routed, and its proof/admission disposition. Canonical package
evidence is projected from both; neither representation is required to absorb
the other's job. Checked display strings are diagnostics, never declaration,
binder, argument, trait, or requirement identity. If a checked witness
interface currently retains an argument only as text, its existing typed or
checked owner must retain a structural coordinate before package projection can
accept that form. The projector must not parse the text back into semantics.

The current review projection follows this rule directly. Package-visible type
identity qualifies each non-binder nominal by exact package ownership, an
exact source-backed toolchain commitment, or an unresolved marker; generic
binders remain owner-free and alpha-normalized. Private `SourceId` state joins
the declaration to its source but never enters canonical review bytes. A
missing join rejects exact review rather than falling back to the weaker generic
toolchain marker. The 22 exact compiler-installed root builtin types use closed
compiler atoms selected by root slot and symbol kind, never package-controlled
spelling; same-named package declarations and source-free generated symbols
remain unresolved. Other source-free compiler semantics still require closed
structural carriers. Public signature identity separately layers
alpha-normalized erased-lifetime topology over runtime type identity, so a
renamed lifetime is stable while changing which region a field or result
borrows changes package evidence. Public data rows include their complete
structural surface, lifetime arity, and stable numbered/retired identities.
Numbered ordinary `data` is also the wire contract—the retired standalone
`wire data` form does
not create a parallel package surface. If a public data form contains quotient
semantics, default-domain proof facts, or static callable/proposition contracts
that the projection cannot yet encode exactly, projection rejects it.
Public domain rows retain exact declaring-package identity, alpha-normalized
type/const binders, carrier type, and closed index arguments. A synthesized
domain path retains its semantic spelling separately from the authored span
that supplies package provenance. Transparent aliases recursively flatten to
sorted, deduplicated package-qualified atomic domains. Authored toolchain
nominals bind a canonical toolchain-relative source path plus exact source-byte
commitment in review evidence; this records semantic origin without making
producer pedigree authoritative. Compiler carry aliases expand to closed
`CarryPermission` atoms in a distinct tagged lane, not fabricated nominal
declarations. Only compiler-reserved unresolved constituents enter that lane;
a valid package declaration remains package-owned regardless of a resembling
diagnostic path. Whole compiler/toolchain commitment remains separate.
Predicate-body presence and the currently representable
structural expression/membership facts retain the domain carrier, package-qualified
members/domains, and canonical fact ordering. Each fact joins its exact typed
handle to one checked definition row and one checked ownership record; nested
members additionally require exact fact-keyed dependency places. Missing,
duplicate, wrong-origin, private-domain, and member-spoofed evidence rejects.
Arithmetic domains and aggregate carry policy are already closed compiler enums;
their diagnostic labels are not package authority. Typed domain constraints now
distinguish declared, carry, value-domain, and `OmegaLayout` subjects. Layout
retains a closed grammar and exact structural schema argument; symbol-backed
declarations remain declared regardless of spelling. Review v41 encodes those
subjects structurally and rejects legacy/unclassified layout names, malformed
compiler subjects, residual const calls, unsupported index forms, and missing,
duplicate, or incomplete checked index selections. This remains an internal
checked-compilation join, not a reason for nominal Chi.
Review v41 additionally decodes compiler-reserved canonical-const transport
atoms into closed type/value terms, omits diagnostic display text, and treats
decimal leaves as numeric values. They are admitted only beneath an exact
declared const parameter. Const binders must reconcile uniquely to the projected
alpha-normalized telescope; residual const declarations and unrelated textual
leaves reject. Neither the legacy atom nor a binder spelling enters review
identity.
Review v42 and canonical row v2 close the proposition-binder distinction:
concrete type arguments use exact structural type identity, including closed compiler builtin atoms,
while machine arguments remain exact nominal declarations. Unresolved ownership
may remain visible to ordinary compiler-local diagnostics, but exact package
review rejects it during type projection and again at canonical encoding.
Review v43 and canonical row v3 add static-machine declaration parameters.
Structural contracts retain the recursively alpha-normalized nested telescope,
complete value signature, proof/crash contracts, reach, invocation, suspension,
blocking, and termination envelope. Nominal contracts retain exact public trait
and requirement identities. Every structural binder, including a nested one,
must join exact checked contract and crash rows; missing custody, excessive
depth, or a private nominal requirement rejects. Proposition parameters and
static-machine/proposition arguments in selected conformance applications remain
fail-closed.
Review v44 and canonical row v4 extend contract-call rows with static-machine
arguments. Each retains either the exact caller machine-binder ordinal or the
exact concrete machine entry identity.
Review v45 and canonical row v5 rejoin each contract call to exactly one
selected callee static telescope. Supported arguments retain their category as
a direct concrete type identity, parser-canonical integer const literal, caller
machine-binder ordinal, or exact concrete machine entry identity. Nested static
applications, forwarded or symbolic type/const binders, proposition/evidence
static arguments, quotient calls, compiler intrinsics, and malformed or
ambiguous joins remain fail-closed.
Review v46 and canonical row v6 add bounded recursive generic data-type static
arguments in contract calls. Each application base rejoins exactly one checked
data declaration, whose telescope is recursively classified; changing a nested
type changes canonical evidence. This rung admits zero-lifetime generic data
applications only. Lifetime-bearing applications, generic machine/conformance
applications, unresolved forwarded type/const binders, proposition/evidence
static arguments, quotient calls, and compiler intrinsics remain fail-closed.
Review v47 and canonical row v7 admit lifetime-bearing recursive generic data
static arguments in contract calls after an exact data-declaration lifetime-
arity join. Lifetime arguments retain alpha-normalized caller lifetime-binder
ordinals: renames are stable, while selecting a different lifetime changes
canonical evidence. Generic machine/conformance applications, unresolved
forwarded type/const binders, proposition/evidence static arguments, quotient
calls, and compiler intrinsics remain fail-closed.
Review v48 and canonical row v8 admit contract-call forwarding of caller type
and const binders. Each argument is validated against the exact caller and
selected-callee telescope categories and encoded by its alpha-normalized caller
static-telescope ordinal: binder renames are stable, while selecting a different
binder changes canonical evidence. The frontend now resolves const-parameter
carrier types on machines and traits. Symbolic const declarations or
expressions, proposition/evidence static arguments, true nested
machine/conformance applications, quotient calls, and compiler intrinsics
remain fail-closed.
In particular, true nested machine static applications such as
`consumer<family<Selected>>()` remain fail-closed: checked monomorphization has
no closed application identity and currently omits recursive arguments and
lifetimes from conflict equality, specialization keys and fingerprints, and
retained specialization evidence. Admission requires exact declaration-
telescope validation plus recursive lifetime and static-argument identity
throughout those compiler paths. This is distinct from already coherent bare
generic-machine selection and call-target use such as `Schema<Selected>(...)`.
Proposition applications use their exact checked rows. A simple total, pure
callable application retains its optional receiver, exact checked package-
qualified entry target, and ordinary arguments after joining one public-
interface declaration-selection row. The whole-source commitment separately
pins the helper body; a callable signature is not body identity. Symbolic const
declarations or expressions, proposition/evidence static arguments, quotient
calls, true nested machine/conformance applications, compiler-intrinsic calls,
semantic roles, and domain operators
reject until exact rows are settled; none is inferred from the domain name.
Compiler-owned
classifications and authorized establishment routes retain the exact route kind and
package-qualified trait/requirement identities; alternative routes normalize
as a sorted set.

Package-owned public traits retain exact identity, boundary status,
alpha-normalized lifetime/type/const binders, ordered package-qualified parent
applications, and ordered machine/operator requirement signatures. Each
parent retains exact lifetime-binder arguments independently from runtime type
arguments. Each requirement retains its lifetime arity, parameter names and
modes, package-qualified lifetime-sensitive signature types, and fixed operator
spelling plus exact declared service reach, installation-bound
status, synchronous invocations as exact non-`self` parameter ordinals or
package-qualified services, suspension, blocking, and termination. Progress
premises retain package-qualified public profile identity, receiver/non-`self`
parameter roots, and package-qualified field projections. Trait or requirement
generic conformance requirements retain an optional alpha-normalized evidence-
binder ordinal, exact subject ordinal, package-qualified public trait identity,
and structural type arguments. Binder-free `where T satisfies Trait` remains
explicitly binder-free rather than fabricating evidence. A non-generic selected
conformance retains exact package-qualified conformance, carrier, and
underlying public-trait identities plus its carrier and trait applications; the
semantic declaration owns exact carrier/trait symbols rather than report code
reselecting names. Public trait requirements retain named and unnamed
`requires` and `ensures` through the same closed structural fact/expression and
evidence vocabulary as public callables, joined to their exact checked
state-signature owner. Named inputs retain ordered proposition and evidence-
interface identity while treating their source aliases as local. Named outputs
also retain their public selector identity. Their
abstract published crash ceilings are projected from exactly one checked
trait/requirement capsule into canonical cause-and-guard routes; no realized
body sites or calls are fabricated. Generic selected-conformance telescopes and
unsupported expression forms reject until complete canonical rows land.
Trailing `boundary host` / `boundary Name`
clauses and trait `invariant` clauses are retired rather than awaiting package
rows. Trait requirement witnesses remain ordinary explicit contracts rather
than package-only evidence syntax.
Requirements also retain whether their checked declaration supplies a default
realization; the implementation body remains source subject to universal update
triage, while its checked operational behavior must fit the requirement
envelope and any instantiated use contributes ordinary compiler-derived
evidence.

Package-owned boundary and ordinary public machines, plus the selected build
machine, retain the exact canonical entry signature alongside their authority
rows. Their checked-body, boundary, and accepted supply tiers remain distinct:
a bodyless boundary guarantee is an explicit trust-bearing accepted claim,
while a claim-free boundary symbol asserts nothing. Canonical review emits a
separate blocking row for that exact accepted callable and its complete
published envelope, allowing admission policy to distinguish trust acceptance
from ordinary callable compatibility without parsing compiler bytes. This
includes lifetime arity, alpha-normalized type/const parameters,
ordered parameter names and `const`/mutable/`self` modes, package-qualified
lifetime-sensitive parameter types, and result type. Renaming binders is
stable; changing a parameter, result, generic bound, or borrow relationship is
not. Checked realizations of public, ordinary, lifetime-free traits retain exact
package-qualified trait and requirement identities, alpha-normalized arguments,
and any explicit conformance alias. Callable conformance bounds, static
machine/proposition parameters, and non-public, external, operator, or
lifetime-parameterized realizations reject until their complete canonical forms
are represented, except that generic binder-free requirements, explicit
evidence binders, and non-generic selected conformances use the same canonical
conformance row as public traits. The projection never substitutes an overload
display name or a runtime-layout-only
type identity for this contract surface.

The older standalone trust-lock lane cannot admit package claims. Domain names
and unmatched strings reject rather than becoming FNV receipts or bare accepted-
fact rows, and domains are absent from trust reports. Exact selected-provider
grants remain valid. Exact accepted-machine grants remain temporary standalone
compatibility only; package-aware compilation rejects them because selecting one
machine cannot admit the package's complete exact accepted-claim inventory.

Public callable `requires` and `ensures` retain exact structural rows for the
closed boolean/integer expression subset over parameter
ordinals, `result`, generic binders, and package-qualified nominals. Domain-
membership rows additionally retain that exact value expression and the
package-qualified public domain; exposing a private package domain rejects.
This is read from the earlier typed semantic tree only after checked
compilation succeeds. Proposition applications retain the package-qualified
primitive endpoint, alpha-normalized binder schema, parameter types, structural
binder/value arguments, and fact-only or witness classification. Transparent
aliases expand before identity. A witness interface retains its exact root
arguments and complete package-qualified direct/inherited requirement surface.
Named contracts join the checked evidence term and positional lane: local
`requires` binding names are omitted from identity, while public `ensures`
selectors remain. Checked diagnostic renderings are deliberately absent from
review bytes and adversarial mutation tests enforce that boundary.
A proof-static `evidence.member` binder argument retains the source named-
`requires` lane, exact package-qualified declaring trait, structural
requirement-argument template, and exact requirement. The lane binds that
template to the source proposition application's concrete arguments; the local
evidence alias is omitted. It is accepted only when checked evidence-term,
interface, and projection facts all match the structural typed declaration.
Direct parameter-rooted member paths in ordinary contracts retain the receiver
ordinal and exact package-qualified case/field chain after joining one checked
semantic-place row. Computed members, proposition-argument members without that
join, unsupported advanced call forms, and aggregate expressions still reject
rather than falling back to text or a hash. Contract casts retain their structural operand,
alpha-normalized target type, arithmetic policy, package-qualified semantic
domain and arguments, and value/recast form. Diagnostic spellings are omitted;
a private package domain cannot be exposed through a public cast.
This join introduces no report-only Chi stage. A distinct stage remains
available only if later consumers or transformations expose a real semantic
boundary.
The legacy 64-bit machine-contract fingerprint has left package-review bytes,
so private state-machine shape no longer contaminates public package contract
identity. Exact crash, reach, invocation, termination, signature, and
conformance rows remain independently encoded. The remaining unsupported
contract forms and exact proof/admission dispositions must land before the
projection can be sealed.

The eventual normalized package-admission evidence must include, with exact
provenance:

- public API contract identity;
- declared reach and either exact checked-body transitive reach or an explicit
  no-checked-body disposition for every public callable;
- declared reach, exact checked-body transitive reach, preselection concrete
  reach, and build observations for the build machine;
- authority `uses`, `stores`, `acquires`, `returns`, and `derives` rows;
- exact provider requirements, selected realizations, origins, trust classes,
  containment, and executable TCB entries;
- routed qualifications and accepted boundary evidence;
- proof-kernel verdicts, accepted opaque claims, and open/deferred obligations;
- installation-bound rows; and
- suspension, blocking, crash, failure, termination, and reproducibility facts.

The first build-observation rung is intentionally narrower than that completed
model. Checked compilation and compiler-issued package review retain the exact
selected build machine's static observation ceiling and realized class. The
current real scoped filesystem provider has no replay transcript and is
therefore `Volatile`; pure and console-only runs are `Hermetic`, and console-
only execution receives no real filesystem provider. Authored filesystem reach
with no statically reachable operation remains a hermetic ceiling. These facts
are driven only by exact canonical toolchain requirement symbols; package-
authored same-named traits and methods cannot select the provider in statement
or value position. Exact canonical signatures then map to a closed, explicitly
tagged 50-operation set exhaustively handled by both providers; aliases and
platform alternatives remain distinct transcript identities.

The one `Build` activation exposes an immutable package-source root and a fresh
writable staging root. These capabilities are absent from the durable build
projection. Checked resolution joins one exact root occurrence to canonical
relative `Path` bytes; bare bytes and virtual prefixes confer no authority.
Authorized path-returning operations preserve that root or reject, while
`read_link` returns inert payload whose use requires a new checked resolution.
Only an explicit successful handoff may introduce staged content into
compilation. Stable `/source/...` and `/output/...` spellings are transcript
serialization, never package-facing paths.

These observations stay separate from capability/API comparison bytes.
Observation schema v10
carries operation-attempt schema v10, retaining each completed operation's exact
provider, stable tag, normalized result, post-error, and every direct scoped path
authorization in successful-run call-start order. Authorized paths retain exact
operand/access, closed Source/Output root, and canonical relative UTF-8 bytes
without physical root spellings. Grant denials remain distinct; host errors
retain prior authorization without fabricating one. Ambiguous/unresolved roots,
unrepresentable rooted paths, and retained-path budget exhaustion reject before
host access; budget exhaustion non-catchably halts evaluation. Partial typed
outcomes survive evaluator failure, while worker
failure marks evidence unavailable; Omega emits fixed non-admission counts and
no review row. Descriptor, native, and find operands retain exact
Resolved/Null/Unknown logical lifetimes. Successful opens mint monotonic IDs;
duplicates and borrowed native views retain their source, successful closes
retain every invalidation, failed closes retire nothing, and provider-token
reuse after close receives a fresh ID. A token live in another logical domain
rejects before provider access; provider acceptance of an otherwise Unknown
token traps. Virtual duplicates share the source cursor;
real descriptors retain rooted write authority through duplicate and borrowed
views, denying content, extent, metadata, ownership, and host-lock mutation
before sponsor or host access when admitted only for source reads;
`open_at`/`unlink_at` names are one portable relative component; real path
outputs reconstruct lossless root-relative values or reject. Successful
descriptor/find/native-handle
results retain only their logical identity in evidence; provider token integers
do not survive. Non-handle
results and failed handle-result sentinels remain exact scalar values, and
package commitments type-tag both result lanes. Fully prepared calls whose
evidence reservation succeeds retain ordinal-ordered non-handle
I32/U32/I64/U64 scalars, exact authored immutable write/FILETIME payloads, and
validated at-family component bytes. Mutable byte carriers retain their
complete pre/post capacity, including unchanged tails, and mutable i64 carriers
retain exact pre/post values. Pre-
state follows evaluation of every authored argument; post-state follows
provider return or halt, including unchanged input-only ABI carriers.
Rooted/path-alias spellings stay out of the payload lane. A separate 256 MiB
aggregate operand-evidence sponsor reserves immutable bytes and both mutable
copies before that call's provider access. Each successfully typed non-handle
scalar and immutable payload is retained as preparation advances, so a later
preparation halt keeps the completed ordinal prefix; the fully prepared call
must reproduce those rows exactly before provider access. Prior or nested staging effects
remain cleanup-contained. Package commitments hash immutable and mutable rows
without rendering them. Path-like bytes not represented by rooted evidence,
retained returned-path bytes, preparation-failure path/logical-handle/mutable
prefixes, and complete content remain absent, so
this makes no receipt, replayability, or source-rebuildability claim. Sponsored
package review separately commits its complete fresh Output tree after
successful evaluator/provider teardown and before cleanup-gated publication.
Sorted canonical entries bind Output-relative portable UTF-8 paths, empty
directories, canonical file-kind/mode, file length and content digest, and
validated self-contained relative symlink spelling. Host roots, ambient
metadata, inode identity, and hard-link topology are excluded. The compiler
cross-checks sponsor namespace kinds, extents, hard-link groups, and quiescence;
review rejects mismatch, unknown kinds, external symlinks, or ceiling excess. A
successful empty build has an explicit empty-tree commitment. Package
observation identity binds the tree digest and topology-independent unique-
content count. The compiler-owned review row now retains the complete canonical
tree behind private fields and can materialize it into an existing empty
concrete directory, then independently re-inspect exact paths, kinds, modes,
targets, and bytes before returning the same commitment. Hard-link topology is
neither retained nor leaked through the count. This is output-tree custody and
replay only. Canonical operation replay, recorded observed inputs, generated-
output handoff, and the complete record replay checker remain required before
any `Receipted` verdict. This rung does not claim hostile same-user race
exclusion.
Raw byte-valued inputs are evaluated once by the shared preparer and reject
above the current 16 MiB evaluator sponsor ceiling before provider cloning/
allocation. Read/count capacities reject negative, wrapped, or
above-ceiling values through one checked conversion. This is not a language
limit. A shared closed preparer checks exact arity, consumes every authored
operand once from left to right, rejects wrong kinds, and retains validated
mutable cells/capacities, including fixed ABI inputs such as Win32 `OVERLAPPED`,
before provider or grant access. It includes otherwise-
unused ABI operands and is source-checked against all 50 canonical signatures
and result widths. Canonicalize enforces its declared 1024-byte `PATH_MAX`
carrier at that gate. Process memory and CPU quotas remain open.
Scoped hard links require write authority on both names, so a read-only source
inode cannot be introduced into writable staging by alias. One compiler-owned
sponsor is shared across the whole disposable review session. It accounts for
package output roots, namespace names, unique object extents, symlink payloads,
and open-but-unlinked objects under a 4,096-entry, 256-MiB-total, and
256-MiB-per-object ceiling. Provider mutations reserve before touching the OS
and commit after success; ceiling refusal is resource exhaustion. Neither
per-package limits nor path-summing bound the actual resource.

Terminal evidence is a separate stronger lane. It is required only for rows
that claim checked properties of final realization—Omega-emitted executable
code, native or externally supplied code, lowering- or ABI-dependent
guarantees, fixed native resource bounds, or a hardened release profile that
explicitly requires independently replayable final-code evidence. Opaque
executable supply may instead remain an explicit trust/TCB row making no
Terminal claim. Ordinary reach, authority-flow, provider, proof-status, and
build-contract admission does not wait for blanket Terminal coverage. Evidence
rows state their exact class; missing Terminal evidence cannot be represented
as a weaker “complete enough” bit or mistaken for a Terminal-verified claim.

Underdeclared effective reach is a compiler error. Overdeclared reach remains a
visible contract-slack row. “Realized” here is the exact inferred transitive row
of an actual checked body, never the authored public ceiling. Bodyless supply
has no checked realization. The separately retained concrete row is the
preselection body base; it excludes authority contributed only by unresolved
installation bounds and does not claim final provider selection. Dangerous
slack is suspicious because it reserves authority that a later implementation
may begin exercising without changing the public ceiling. Exact dangerous
callable-and-service slack is audit-recommended even when retained across an
update. The manifest pins declared, checked-body, and slack evidence, so
unused-to-used authority still changes evidence.

Open or deferred proofs reject package admission. The current compiler has no
explicit deferred-proof status, however, and its contract-entailment engine may
stand down on facts outside that engine's language. The admission profile must
reject an unresolved stand-down or retain the exact later checked obligation
that discharged it. The package-aware checked path now retains exact
machine/contract/fact coordinates with a closed stand-down reason from the
pristine typed graph, and review rejects every checked-implementation row.
Accepted and opaque supply remains in the trust lane. Sealing and any exact
later-discharge ledger are still unfinished; a successful ordinary compilation
is not by itself a complete proof verdict. Checked proofs are rechecked by the
proof kernel. Terminal propagation remains necessary only when an admitted row
actually makes a final-realization claim.
Accepted axioms and opaque boundary claims must remain explicit trust-bearing
evidence and require admission; authored postconditions are obligations, never
proof. Boundary realization must use exact package-qualified nominal identities
and reject same-spelled declarations from another lineage. Currently the
compiler joins package identity for the realizing machine, provider type,
selected service schema, and requirement owner into provider plans and provider
trust rows. Provider binding/selection identities and sealed admission evidence
remain unfinished.

Risk classification must be compiler-owned metadata attached to exact admitted
boundary/capability identities. It must never be inferred from
package-controlled strings such as `Filesystem` or `Network`.

Claim-free opaque boundary data occupies a distinct representation-TCB lane.
The compiler reports the exact package-qualified declaration, target,
representation/ABI commitment, selected external mechanism or explicit unbound
status, and provenance. Its initial introduction or material change strongly
recommends code/ABI audit but does not, by opacity alone, create a blocking
trust-claim conflict.
Unchanged rows remain visible without requiring repeated blanket approval.
Deployment policy may elevate an exact compiler-owned mechanism to blocking
when that mechanism is intrinsically dangerous.

Accepted propositions, boundary or provider guarantees, qualification/
authority establishment, executable mechanisms, and dangerous derived reach
remain separate blocking or dangerous-authority rows. A public ABI change may
also block compatibility policy independently. Omega never classifies an
opaque type from its package-controlled spelling, infers safety from absent
current use, or omits it merely because it declares no `reaches` service.

## Update, install, and missing baselines

An install compares the new dependency closure against an empty admission
baseline. A completely checked package with neither blocking evidence nor
review findings may pass as `admitted`; claim-free opacity alone may complete
as `admitted-with-audit-recommended`. An accepted-claim row blocks for exact
root-policy resolution on initial admission or when newly introduced, while an
unchanged accepted baseline does not require blanket reapproval. Suspect
authority, trust, executable introduction, dangerous contract slack, or
build-host reach recommends audit;
the exact capability, claim, compatibility, or root-policy row determines
whether admission also blocks.

An update derives candidate evidence and compares it with the normalized
accepted baseline in `omega.lock`:

- a blocking capability/API evidence change creates an exact conflict;
- a claim-free representation-TCB change recommends code/ABI audit unless
  compatibility or exact-mechanism policy independently blocks it;
- unchanged evidence permits resolution to continue;
- retained intrinsically dangerous authority always emits an audit
  recommendation; and
- source-lineage or declared-name change is package replacement.

Every source update also receives automated/LLM provenance and source-diff
triage. Equal capabilities do not imply safe behavior: code with existing
filesystem and network authority can become malicious without changing its
authority set.

Representation-TCB rows participate in the same integrated review. A package
with only new claim-free opacity may finish as
`admitted-with-audit-recommended`; a package that also introduces accepted
claims, dangerous authority, or policy-blocked representation mechanisms
remains unresolved until those exact rows are reconciled. There is no generic
approval prompt for either case.

The old source is useful for focused code review but is not the capability
baseline. If the old source cannot be fetched from its exact commit or cache,
capability comparison still uses the lock while source review escalates to a
standalone candidate audit. If the accepted lock baseline is absent, the whole
closure undergoes fresh admission. Missing old source and missing admission
evidence are distinct conditions and are reported separately.

## Conflict and audit UX

Admission is not a yes/no prompt. Omega emits a compact conflict containing the
exact package/source identity, dependency path, changed checked rows, risk
classification, source provenance, and unresolved decisions. A resolution
must address each blocking row and is bound to the exact candidate source,
toolchain, old/new evidence, and conflict fingerprint. It cannot be reused for
another update. It is accepted only through the root project's configured
policy workflow; matching bytes supplied by dependency source have no standing.

LLM triage receives only Omega-rendered, bounded, escaped identifiers and
evidence rows. Package prose, comments, commit messages, and README text do not
enter the triage prompt. A later code audit necessarily reads attacker-
controlled source and is treated as a separate hostile-input activity.

The current review-only implementation provides the deterministic envelope for
that flow. It compares compiler-issued closure rows, blocks capability/API and
source-lineage changes, recommends audit for unavailable old source, changed
representation-TCB evidence, changed build observations, and retained dangerous
authority, and renders only fixed reason/disposition tokens plus canonical
package-key commitments under a caller-supplied byte ceiling. The separate
source packet accepts only resolver-issued custody for one exact `PackageKey`,
binds both full immutable resolutions, and renders raw tree changes with fixed
three-line context. It retains directories, executable bits, symlink spellings,
entry-kind transitions, exact line endings, and raw path order without rename,
whitespace, Unicode, or Git normalization. Independent entry, source-byte,
metadata-byte, line, diff-work, trace-memory, and output ceilings reject rather
than truncate. Binary or non-UTF-8 changes expose only size and a
domain-separated content commitment and require standalone audit. Fixed
grammar and byte escaping prevent source from forging renderer structure, but
cannot prevent semantic prompt injection in code under review. Model invocation
remains future work. The implemented join requires a bijection between the
complete candidate closure and compiler rows by exact key and immutable
resolution. Its shared validator also rejects duplicate reviews,
package/projection identity mismatch, mixed deployment targets, and mixed
compiler-executable commitments before either capability comparison or source
rendering. It validates every recovered baseline custody against its row and
derives unavailable-old-source state from absence. Initial and newly transitive
source packets follow compiler-recommended audit policy; changed or unavailable
existing update sources receive an exact diff or standalone candidate packet.
The aggregate byte ceiling retains
separate compiler-only and hostile-source frames. No output can construct
accepted lock evidence or attest that review happened.

Review can resume after process restart without refetching the old source. A
versioned, bounded binary review-baseline capsule retains the complete resolved
`PackageKey` graph and immutable resolutions, comparison commitments, and every
canonical comparison row plus its source-explanation sidecar. The compiler owns
strict row-envelope recovery; orchestration treats row values as opaque, and a
recovered row remains distinctly review-only rather than becoming newly
compiler-issued evidence. The capsule checksum detects accidental corruption,
not authenticity or proof of review, while canonical decode rechecks graph
closure, row/package/target identity, ordering, singleton rows, and all resource
ceilings. It is deliberately a non-admitting checkpoint: no API converts it to
`PackageInstance`, a conflict resolution, project mutation, or accepted lock.
The future lock may contain the same normalized material only after a consumer
reconstructs the exact source-and-artifact obligations, checks the retained
certificates, propagates every dependency's open obligations, and records its
own admission decisions. Producer provenance cannot promote the checkpoint.

Useful result states include:

```text
admitted
admitted-with-audit-recommended
blocked-capability-change
blocked-missing-admission-baseline
blocked-provenance-change
```

Organizations may attach their own review status, signers, quorum, tickets, or
reason text. Those are governance records, not compiler facts.

## Re-derivable package evidence

A compiler-issued review is permanently review-only. A package instance is
sealed only when a consumer takes the exact requested source, exact produced
artifact, canonical semantics, and subject-specific evidence schema;
reconstructs the complete obligation set; and checks the exact retained
certificates. The stored result is a cache of that reproducible check rather
than authority supplied by either compiler or verifier. Certificate identity,
proof route, and checking dependencies remain derivation provenance outside
semantic compatibility identity.

The ordinary produced artifact is the complete versioned package-admission
semantic row set under one exact package key, target, dependency closure, and
obligation schema. It is neither native code nor a renamed compiler review.
Review may carry candidate bytes in the same canonical vocabulary, but a
consumer gives them force only by independently reconstructing the total set
from exact source and comparing bytes exactly. Source, proof route, compiler
observations, and local decisions remain separately bound. Current incomplete
review-v48 bytes cannot be promoted merely because the future artifact reuses
their row vocabulary.

That local reconstruction may read the earliest coherent compiler-owned IR in
which an obligation is semantically complete, including private pre-Psi or
pre-Terminal state. The checker is part of the compiler and may move with those
internals; only its versioned canonical obligation ledger and exact replay
subjects cross the persistence boundary. There is no nominal Chi stage merely
to stabilize this seam. Add one only if implementation discovers a genuine
reusable semantic boundary, and prefer an existing coherent stage such as Exact
when it can carry the same meaning with less machinery.

Terminal Psi now provides the first concrete replay ledger: one ordered,
owner-tagged set covers executable operations, call and nominal-cleanup
requirements, and contract guarantees, retaining each exact proposition,
obligation class, assumption list, and reconstructed axiom order. The verifier
consumes and retains that set. Its canonical bytes bind exact Terminal-Psi and
source-backed verifier trust-graph identities but exclude proof route, so
different valid certificates preserve semantic identity. A decoded producer
ledger is accepted only after exact local reconstruction. This remains a narrow
Terminal component. Its artifact manifest retains a separate ledger fingerprint,
and replay lowering consumes semantic, ledger, and proof sections in that order.
It is not whole-package evidence or lock authority.

Dependency evidence composes transitively. Each subject retains its own
obligation-semantics identity because one closure may contain evidence produced
under several versions. Checked obligations compose upward. Missing or
unproved obligations also compose upward as open rows, never as a producer's
already-accepted decision; each consuming project applies its own admission
policy. A checked schema-delta relation may reuse classes proven unchanged and
derive the precise new gaps for added or strengthened classes. Reinterpreted or
unknown classes force re-derivation, and no gap becomes admitted implicitly.

Mechanical verification, local admissions, and producer metadata are separate
report sections. A `verified` verdict contains only locally re-derived facts.
Compiler/toolchain closure, reproduction, signatures, and audit records remain
useful provenance but never appear as support for that verdict.

## Audit authority and compiler provenance

Omega cannot prove that a human or LLM performed a serious audit. A signed
resolution proves only that a key signed bytes; a recorded reviewer or reason
proves only that strings were recorded. A proof certificate can establish its
explicit mechanically checked proposition, but not that the surrounding source
was understood, that an LLM resisted manipulation, or that an upgrade is safe.

The people and infrastructure allowed to land accepted project state remain an
outer trust boundary, but the selected local compiler is an untrusted producer
for package soundness. Package review is regenerated locally so a dependency
cannot declare its own capability result; package acceptance independently
reconstructs the question and checks the certificates. Obligation-semantics,
schema, source, artifact, and target identities define or scope that check.
Compiler, toolchain, and execution observations remain in review metadata for
replay, cache correctness, and reproducibility—not as proof that the producer
or review process was honest. A compiler change may require regeneration, but
hashing the compiler does not confer authority on it.

Omega's responsibility is to produce deterministic, bounded review facts,
recommend an audit for dangerous retained authority, stop on unresolved policy
conflicts, and expose hooks for project policy. A project that needs stronger
assurance must enforce its chosen process around Omega: protected branches,
required reviewers or signatures, isolated builds, independently bootstrapped
toolchains, reproducibility checks, or other controls appropriate to its threat
model. The committed and merged decision authorizes the update; Omega does not
manufacture a portable “proof of audit.”

## Implementation trust status

The `omega-packages` release surface now contains reviewed corrected-model
building blocks for immutable source custody, typed identity and closure,
compiler handoff/review, exact row conflicts, and review-only triage. Its final
admission model is not yet accepted. Legacy manifest, name-keyed lock,
whole-section receipt, and install/update scaffolding remains isolated behind
crate tests. Production code must not:

- key locks or symbols by package-authored name alone;
- ask the installer for both alias and package name;
- accept caller-constructed package capability manifests;
- accept standalone manifest JSON as compiler evidence;
- treat a free-form reviewer/reason receipt as conflict resolution;
- store only a capability fingerprint without the accepted baseline; or
- syntactically scan dependency calls while silently skipping malformed
  dependency builds.

Those seams must be replaced before `omega install` or `omega update` can
mutate project state.

## Test packages

The existing fixtures now declare `PACKAGE`, use canonical build parameters,
and regenerate currently representable compiler review evidence from resolver
custody. Tests no longer fabricate package manifests from fixture intent.
Remote fixtures
must exercise transport-normalized lineage, immutable commit/tree identity,
missing-old-source review, missing-lock fresh admission, retained dangerous
authority triage, and same-name/different-lineage spoof rejection.
The local production-path spoof canary already proves that byte-identical,
same-declared-name packages from distinct lineages retain separate physical
compiler custody and package-qualified provider evidence. Remote transport
coverage must preserve the same result rather than relying on content equality.
The canonical `Console`/process case must additionally prove that classification
comes from the exact compiler-owned declaration identity and that a package-
owned lookalike cannot mint the same risk class. This is semantic origin
checking, not producer-pedigree authority.
