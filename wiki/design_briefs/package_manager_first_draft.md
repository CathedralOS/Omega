# Design Brief: Package Manager First Draft

Status: working design record, 2026-08-30. This brief preserves design context
while implementation vocabulary settles. Where it conflicts with
`build_and_package_model.md` or a current subsystem contract, those current
documents govern.

## Intent

Omega needs a Cargo-like source workflow without a hosted registry and without
trusting package-authored identity or capability claims. It resolves user-named
Git, URL, or local sources to immutable content, discovers package identity
from the fetched package, derives security evidence with the compiler,
reconciles the complete closure, and admits the result before changing project
or lock state.

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

Every package declares its own human name in its `build.omg` through the
ordinary build surface:

```omega
machine build(builder: &mut Build) {
    builder.package("arithmetic-kernels");
}
```

This is the same surface that carries `depend_as`, `select_provider`, and
`roots.bind` — not a new grammar form and not a specially parsed literal. The
kind is always stated: `builder.member(path)` for a workspace root,
`builder.application(name)` for an application. No role is inferred from an
absent declaration. The declaration must be unique, effect-free, independent of
dependencies and generated files, and use canonical kebab-case spelling.

The selected project entry is always the free
`machine build(builder: &mut Build)`. A scoped `Owner::build` is not a manifest:
the owner name establishes no project role, provider relationship, or receiver
instance. Package and standalone readers reject it consistently when it appears
as a selected build root. In ordinary source it remains an ordinary machine.

The free root may call ordinary helpers with its borrowed `&mut Build` for
evaluated composition. Their complete transitive contracts compose into the
root, so delegation cannot hide undeclared service reach or other authority.
Static role, member, and dependency declarations remain direct root statements;
helpers cannot manufacture or condition the source graph.

> Superseded 2026-08-25: this brief previously specified a `const PACKAGE:
> Package` literal extracted by a bespoke static parser. See
> [Build And Package Model](build_and_package_model.md) for the settled form.
Canonical spelling begins with an ASCII lowercase letter, contains only
lowercase ASCII letters, digits, and single hyphen separators, and therefore
maps mechanically to a valid snake-case Omega alias.

Three identities remain deliberately separate:

- `PackageName` is the package-authored human name, such as
  `arithmetic-kernels`; it is not globally unique.
- `PackageKey` joins that name to canonical source lineage. It is the stable
  graph, lock, and nominal-symbol identity across updates. Git lineage names the
  canonical repository namespace and excludes the requested revision, resolved
  commit, tree, and content; those belong to `PackageInstance`.
- `PackageInstance` joins the key to exact source content, produced artifact
  identity, each closure subject's obligation-semantics identity, locally
  re-derived discharge results, and disclosed open assumptions. Exact
  certificate routes and compiler/toolchain identity remain derivation and
  review provenance rather than semantic authority.

`PackageKey` is shared by package and application declarations because both
supply the same name and source-lineage inputs. Importability is a role rule,
not an identity fork: a selected root may be `Package` or `Application`, while
every dependency edge must resolve to `Package`. One root/non-root admission
path enforces that invariant uniformly across source kinds. The admitted root
role remains explicit in closure evidence, the lock, review, and compiler
handoff; it is never inferred from an entry binding and never hashed into the
key.

For Git, source lineage normalizes transport spellings only when a resolver
adapter can establish that they designate the same repository namespace. A
matching host/path is not universal proof that HTTPS and SSH serve the same
repository. Unknown equivalence remains distinct. Exact commit, tree, and
content identities remain instance evidence. A different lineage or declared
name is package replacement, not an ordinary update; a matching declared name
never converts one lineage into another.

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

Local snapshot issuance is one resolver-owned operation rather than a bundle of
caller-assembled fields. While the snapshot entry lock remains held, the
resolver reconciles the exact request with its canonical live root, verifies
the retained storage and immutable publication, rechecks the live source, and
rehashes the published exact tree. The resulting read-only snapshot carries an
opaque observation binding those facts plus the compiler-bounded source limits
and custody identity. This is the successful local-source receipt. Package
review and lock admission remain separate decisions over that exact source.

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
        revision: "main",
        selection: PackageSelection::Root
    });
}
```

For a repository workspace, the target source spelling selects by declared name
rather than member path:

```omega
builder.depend_as("linear_algebra", Source::Git {
    repository: "https://example.invalid/math.git",
    revision: "main",
    selection: PackageSelection::Named { package: "matrix" }
});
```

The spelling may omit selection as shorthand for `Root`, but projection
normalizes it immediately to explicit `Root`. Acquisition and selection remain
separate internally: two selected members at one revision share one content-verified
repository fetch, tree, and `SourceIdentity`.

For `Named`, the resolver verifies the repository root, projects only its
declared member paths, reads each member's own `builder.package` declaration, and
requires one exact name match. It never recursively searches for `build.omg` or
accepts a caller-authored member path. The resolved member path is retained as
navigation/replay custody and as the base for relative dependencies, but it does
not enter `PackageKey`; relocating the member does not replace the package.

Omega syntax remains a manager responsibility: source acquisition exposes
bounded verified declaration bytes to the manager planner and receives
only validated member paths and the selected path in return. Source then opens
and publishes the selected subtree from the already verified graph. The
raw declarations remain replay evidence outside the compilation root.

Dependency requests are unconditional, directly projectable build rows.
Graph-forming control flow on `builder.target`, `depend_when`, and
`depend_as_when` is retired; the package manager does not execute or statically
interpret a build-machine state graph to discover edges. Aliases are unique in
the one declared dependency set.

Platform variation ordinarily lives in target-scoped declarations inside the
selected packages, while application entry selection uses flat unconditional
`roots.bind(target::ProgramEntry, entry)` rows. One invocation may request one
exact target or a nonempty caller-supplied canonical set. It acquires and
parses shared source once, then forks at the first target-sensitive stage and
produces independently identified exact-target children. Identical immutable
Psi/PCC products may be strongly rejoined and forwarded to several lowerers;
unresolved target branches never enter one Psi subject. Package resolution
does not manufacture an `all` set from roots, dependencies, or the toolchain
catalog, and `target X { }` declarations are activation facts rather than a
support matrix. A future
target-specific dependency surface, if a concrete customer requires one, must
be an unconditional row naming the exact target; it cannot restore conditional
dependency discovery.

The resolver obtains the selected package's name from its own
`builder.package` declaration. The default in-code alias is the mechanical
kebab-to-snake mapping, `arithmetic-kernels` to `arithmetic_kernels`. Only a genuine local collision
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
row. Reserved owner-attached `T::drop` is compiler-only; authored early disposal
instead selects the ordinary consuming `omega::core::drop(value)` machine. The
exact concrete cleanup plan remains carried semantics. Compiler-planned layout,
multiplicity, and automatic cleanup do not grant source authority.

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

Occurrence identity belongs to the exact source token, not to a transient IR
copy or provisional target. Compiler normalization may carry that occurrence
onto multiple expressions, but those copies must reconcile to one declaration
or one closed compiler intrinsic; conflicting resolved targets reject. Private
ranking witnesses retain exact typed expression custody separately from their
stable rendered artifact identity, so neither checking nor package review joins
them back by text.

Public/private disposition follows the declaration-owned source position.
Public machine contracts, public data/domain predicates, and public trait
contracts are public interface; executable states, bodies, and `terminates by`
ranking witnesses are private implementation. `terminates` is the published
promise, while the ranking is its proof. Proof-membership custody includes the
selected domain path, not the lexical value parameter. Every independently
nameable declaration owns ordinary `pub`; carrier qualification does not imply
visibility inheritance, while genuine nested members inherit their one exact
semantic owner's visibility. A public-interface selection of a private
declaration rejects. A qualification cast's authored semantic-domain path is
one such exact selection: it enters the ledger at the cast expression, is
finalized only by the typed domain resolution, and therefore passes the same
visibility and direct-dependency gates before package review.
Expression-owned type positions follow the same disposition. Symbol-resolved
expressions retain their exact authored public/private position; cast targets,
cast domain indices, and `zero_value<T>()` lower type selections under that
position instead of a private default. Proposition casts resolve those types
through the same exact symbol path as machine casts. Public contracts therefore
cannot hide a private or transitive-only nominal inside an expression type.

In a generic conformance bound, the subject and optional evidence binder are
lexical. The right-hand trait is authored declaration authority, and a
qualified `Carrier::Evidence` bound selects both exact declarations. Bounds on
machines and traits take the enclosing declaration's public/private
disposition; declaration publication remains a separate visibility rule.

Trait composition follows the same rule. Header parents and body `requires`
clauses normalize to one semantic edge, and each source-backed edge retains the
exact resolved trait as a type-reference selection with the enclosing trait's
disposition. A transitive-only parent therefore rejects at the ordinary direct-
dependency gate. Its separate `trait_parent` source coordinate is review
provenance, not admission.

An attached declaration head such as `machine Data::operation` also selects the
exact carrier declaration. Its type-reference row inherits the machine's
interface disposition, including exported boundary supply without `pub`.
Qualification does not relabel or implicitly admit a transitive carrier.

Quotient formation likewise retains each authored coordinate independently:
carrier, right-hand relation, repeated static-`where` relation subject, sealed
`Equivalence` trait and arguments, and named proof conformance. Relation and
trait rows inherit the quotient data declaration's disposition. The proof
conformance alone remains private formation custody and outside quotient API
identity, without bypassing ordinary visibility or direct admission.

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

Owned erased carriers extend that same row rather than introducing a cleanup
conformance. Their descriptor transports payload custody, layout, movement, and
the exact owner-derived cleanup plan together. A borrowed erased view transports
dispatch only and never becomes responsible for cleanup of its referent.

## Dependency planning before build execution

Dependency-source projection must be hermetic even though later build staging
may use admitted host services. Dependency rows cannot depend on filesystem or
network observations, clocks, generated files, imported code, or package build
outputs. The initial implementation may accept only direct canonical rows; a
later implementation may evaluate a broader compile-time-admissible projection.

Resolution and admission proceed in this order:

1. Resolve and fetch through the host-routed Git/SSH environment, then validate
   and publish source under resolver-owned custody.
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
native-image command remains gated until
`PACKAGE-NATIVE-GENERATED-SOURCE-TRANSACTION` consumes the same sponsored
package transaction.

When a later package consumes that dependency, review orchestration does not
reopen Output or rerun `build.omg`. Dependency-first compilation retains one
opaque compiler-issued bundle per package, including explicit empty bundles,
bound to the producer identity, exact target, producer dependency closure,
producer source-consumption commitment, and that producer's own generated
paths and bytes. The consumer's initial frontend loads the complete bundle set
under the original producer identities and compiler-owned logical paths, so
generated imports participate in ordinary resolution and in the consumer's
source-consumption commitment. The orchestration join rejects missing,
duplicate, foreign, root-self, wrong-target, wrong-closure, and mismatched
review/custody bundles. This handoff is ephemeral compiler custody, not lock or
admission evidence. The real filesystem-producing package canary remains
engineering-blocked on moving sponsored read/write operations from std's
`FilesystemHost` onto the compiler-owned `BuildSource`/`BuildOutput` facets. It
requires no staging-authority package role, and no name/path compatibility
exception is admitted.

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
literals select nothing. A named const in a static-argument lane selects its
declaration directly and rejoins its canonical value; a named const reduced in
ordinary expression position retains declaration provenance through the
existing substitution row. Unresolved static paths remain explicit obligations
and fail closed at package admission.

## Authored requests versus accepted lock state

`build.omg` records update intent: source locator, revision selector, explicit
alias override, targets, roots, providers, and build orchestration. `omega.lock`
records the accepted resolution: exact commits/trees/content, `PackageKey`, the
selected root's explicit package/application role, `PackageInstance`, dependency
closure, per-subject obligation-semantics and
evidence-schema identity, exact certificate provenance, normalized capability
baseline, transitive open obligations, build observations, and policy-resolution
references. Cross-invocation compatibility is governed by those semantic
identities and the explicit review and row encoding versions. The bytes
readable through the running process's executable pathname are not review,
lock, conflict, or admission identity.

The lock contains independently populated closure/review sections for exact
target-profile identities. The projected request map is complete for each
fetched package, but an unresolved dependency's transitive map remains unknown.
Ordinary resolution populates the selected profile column; an explicit command
may populate all columns. Locked use of an absent column fails without network
access. Common immutable instances may deduplicate across columns, while an
inactive retained column grants no current resolver, import, build, alias, or
capability authority.

The compiler always builds from the lock and never silently re-resolves a
mutable selector. `omega.lock` is generated but should normally be committed;
source caches and expanded artifacts may be ignored. A fingerprint alone is
not an admission baseline: the lock must embed the normalized accepted security
projection or retain a mandatory content-addressed copy.

The accepted lock begins with fixed magic and one outer format version. Omega
checks both before allocating for or interpreting the remaining payload. The
outer version covers the complete accepted payload contract, including its
nested source-subject, reconstruction-question, obligation, review, and row
schemas and encodings. Any incompatible nested change bumps the outer version.
The ordinary decoder accepts only its exact current version; an unknown version
rejects with guidance to regenerate `omega.lock` from the exact source closure.
The payload's own identities remain available for internal reconstruction and
corruption diagnosis, but they are not a multi-version compatibility surface.

The first resolver does not solve semantic-version ranges. Requests for the
same `PackageKey` that reach the same immutable source resolution deduplicate,
including differently spelled requests that resolve to the same commit/tree.
Different immutable resolutions fail with every conflicting dependency path;
there is no undefined intermediate notion of a "compatible" request.
Multiple simultaneous instances per key are unsupported. Supporting them would
move nominal type, conformance, provider-selection, and evidence identity from
`PackageKey` to `PackageInstance`, so it is an identity-substrate redesign rather
than an alias feature. Package dependency cycles reject in v1, keeping build
order and request-path provenance finite; supporting a cycle later requires an explicit
semantic and custody model rather than accidental graph acceptance.

The compiler handoff contains the reconciled root's opaque stable identity,
explicit package/application role, one canonical source root per graph node, and
requester-local alias edges between those identities. Package-aware compilation validates that
closed graph again and never combines it with `build.omg` scanning. Canonical
paths are import-custody locations only; the opaque `PackageKey` commitment is
the semantic identity that survives source loading.

An application artifact used as a build tool would be an artifact-consumption
edge with its own provenance and authority contract. It is not a package source
dependency and is outside this role rule.

Aliases remain requester-local edges. Different packages may bind different
aliases to one key; an ancestor cannot rename a child's internal edge. If a
consumer names a dependency-owned declaration, it declares its own edge. If it
only possesses an inferred value whose foreign type it never selects, the
transitive lock still contains the dependency but no direct source alias is
required.

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

Ratified by D46: package orchestration and compiler review run in one `omega`
process. Rereading the bytes at `current_exe()` before and after review observes
a pathname target that cannot alter the image already loaded for that review.
No compiler-executable path-byte commitment enters the envelope, its rows,
comparison, conflicts, locks, or admission. Internal reconstruction validates
canonical and semantic joins; it is not process isolation or executable
attestation.

Reviews produced by different invocations use the explicit obligation-
semantics, evidence-schema, review-encoding, and row-encoding identities for
compatibility. A meaning-changing revision changes its semantic identity; a
pure encoding change changes the corresponding encoding version. Executable
byte equality is neither required nor substituted as a proxy. No executable
digest is retained speculatively for caching. A future cache must first define
whether it keys exact implementation artifacts or semantically reusable
results and then use an identity that states that claim.

This does not forbid exact artifact custody. When a compiler or tape is itself
the subject of a bootstrap, reproduction, or deployment proof, that subject's
bytes are bound exactly. That is distinct from observing the pathname of a
same-process producer. Real process/image attestation likewise remains
separate deployment evidence until a concrete claim and verifier consume it.

Ratified 2026-08-26: implementation should consume the earliest coherent
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

There is no nominal Chi stage merely to collect or stabilize this report. Add a
named stage only if implementation discovers a genuine reusable semantic
invariant boundary. Additional consumers or transformations may reveal such a
boundary; stability, layer purity, or local simplification alone do not.
Implementation discovery may also collapse rows into an existing coherent
representation, including `Exact`, when that removes machinery without losing
semantic distinctions. Psi may independently repeat an invariant as a
downstream backstop; that does not require the package checker to discard an
earlier semantically complete fact and reconstruct it from Psi.

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
fingerprints. Exact nested use sites may be added through their existing
typed/checked owners and compiler sidecars without creating a report-only
stage. Public-trait parent requirements are the first implemented carrier.
Their typed owner already retains the exact authored identifier span, which now travels
with the trait row under `trait_parent` through sorting. Syntax, resolved, and
typed contracts now retain the exact authored clause keyword independently from
their semantic facts. Direct machine, public trait/top-level-requirement, and public-
operator contracts carry it under `contract_clause`, and accepted-claim rows
reuse the callable sidecar. Every projected declaration family recursively
collects the same anchor from structural static-machine parameter contracts.
Checked body calls add
`body_call` anchors by joining checked-flow coordinates to exact typed
statement, expression, and named-transition sites during checked lowering,
before provider settlement may rewrite typed call identity. Statement and transition
sites carry explicit authored call-selection occurrences; expression sites
reuse their existing attached occurrences. The join verifies target, receiver,
receiver shape, and operational acknowledgement at capture, rejects missing or
contradictory provenance, and emits no source location for generated calls. A
legitimate late-bound target does not invalidate source custody: the span proves
where the call was authored, not that target finalization has occurred.
Authored `invokes` targets now enter typed trees as one record binding the
diagnostic name, exact parameter-symbol/ordinal or exact boundary-trait symbol,
and exact target-name span. Invocation inference consumes the retained target,
never a later same-spelled trait scan. Callable, public trait/top-level-requirement, and
recursively structural machine-parameter rows carry those spans under
`synchronous_invocation`; top-level projection joins them to the exact checked
plan and rejects missing, malformed, duplicate, aliased, or stale custody.
Checked invocation facts retain exact symbolic published and inferred targets
before provider settlement; package review does not re-infer effects from the
transformed typed tree.
Authored `reaches` clauses retain every keyword and member occurrence through
syntax, resolution, typed lowering, copying, and specialization. Resolution
binds each member once to its exact boundary-trait symbol; semantic
normalization remains an idempotent parent-closed row over authored targets and
invocation-contributed services. A private memberless authored clause is a
published empty ceiling rather than omitted private inference. Projection
rederives that row, joins it exactly to typed and checked facts, and carries
each authored member span—or each keyword span for an empty row—under
`service_reach`. It invents no coordinates for parent closure, inference, or
invocation-only reach. Recovery envelope v6, conflict fingerprint v9, and
renderer V8 bind that reach-source schema. Authored `suspends` and `blocks`
keyword occurrences now follow the same custody rule through syntax,
resolution, typing, trait-default synthesis, copying, and specialization.
Callable, public trait/top-level-requirement, and recursively structural machine-
parameter rows use distinct `suspension` and `blocking` roles. Projection
requires the authored boolean, retained keywords, and exact checked interface
to agree; omission and inference acquire no invented location. For public or
otherwise contract-supplied machines, the checked operational fact is the
published may-ceiling, not a claim that the current body exercised it. Review
v75/row v33, recovery v13, conflict fingerprint v16, and renderer V15 bind the
current source schema. External executable leaves retain the exact authored
`via` keyword on the same conformance as the normalized binding identity.
Projection requires binding/span parity and carries that occurrence under
`external_binding` for public and private trait, operator, or top-level
requirement supply. Semantic
row bytes remain unchanged; missing, source-free, or contradictory custody
rejects. Public const declarations additionally retain the exact parsed
initializer-expression span through symbol resolution and typed lowering,
before substitution erases the value tree. `PublicConst` rows carry it as
`const_initializer` beside the declaration-name anchor. Relocation changes the
explanatory coordinates but not semantic row bytes. Transparent public
propositions similarly retain their complete authored formula extent at the
parser boundary under `proposition_formula`, before application lowering or
operator-root narrowing can erase it. Primitive and witness propositions
receive no invented formula location. Every authored proof fact now retains
its full semantic-token extent under `proof_fact` through syntax, resolution,
typed lowering, generic synthesis, and checked specialization. Public
domain/data facts require that custody, as does every fact beneath an authored
public contract clause; source-free compiler synthesis receives no invented
coordinate. Coordinates remain explanatory rather than semantic identity.
Missing spans must be retained before they are erased, never recovered by
parsing source text in package orchestration.
Public trait rows additionally retain every exact machine-requirement
declaration under `trait_requirement`; public data rows retain fields, sum
cases, and payload fields under `data_member`. These roles consume the existing
typed declaration symbols. Direct declarations use their authored spans;
generated declarations expose only their real derivation origin.
Reviewed package callables, public operators, and public trait/top-level requirements
likewise retain every value-parameter declaration under `callable_parameter`.
The same compiler-owned walk covers value parameters nested in structural
static-machine contracts. These coordinates bind what review displays without
changing semantic row identity.

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
toolchain marker. The 22 exact compiler-installed root builtin types and 71
compiler-installed builtin functions use closed compiler atoms selected by root
slot and symbol kind, never package-controlled spelling. Same-named package
declarations and source-free generated symbols do not classify as those atoms;
other source-free compiler semantics still require closed structural carriers.
Public signature identity separately layers
alpha-normalized erased-lifetime topology over runtime type identity, so a
renamed lifetime is stable while changing which region a field or result
borrows changes package evidence. Public data rows include their complete
structural surface, lifetime arity, and stable numbered/retired identities.
Numbered ordinary `data` is also the wire contract—the retired standalone
`wire data` form does
not create a parallel package surface. Public data now has a closed
ordinary/quotient discriminant. Quotient identity contains the exact
carrier-family type and package-qualified public relation declaration; package
review reruns the complete quotient-formation judgment before issuing it. The
relation's existing public-proposition row binds its semantic body. A selected
equivalence conformance remains private admission custody and does not change
quotient API identity. Default-domain proof facts or static callable/
proposition contracts that cannot be encoded exactly still reject.
Public domain rows retain exact declaring-package identity, alpha-normalized
type/const binders, carrier type, and closed index arguments. A synthesized
domain path retains its semantic spelling separately from the authored span
that supplies package provenance. Transparent aliases recursively flatten to
sorted, deduplicated package-qualified atomic domains. Authored toolchain
nominals bind a canonical toolchain-relative source path plus exact source-byte
commitment in review evidence; this records semantic origin without treating
the running executable pathname as review identity. Compiler carry aliases
expand to closed
`CarryPermission` atoms in a distinct tagged lane, not fabricated nominal
declarations. Only compiler-reserved unresolved constituents enter that lane;
a valid package declaration remains package-owned regardless of a resembling
diagnostic path. Exact compiler/toolchain artifact custody remains separate
and applies only when that artifact is itself a bootstrap or deployment
subject.
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
proposition/evidence arguments in selected conformance applications remain
fail-closed; selected type, const, and machine arguments use this same
categorized vocabulary.
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
Review v49 and canonical row v9 admit public-trait proposition-family
parameters with their mandatory declaration-site value signature. Each retains
the ordered, package-qualified and alpha-normalized value-parameter types.
Trait, proposition, and value-parameter binder renames are stable, while
changing a signature type changes canonical evidence. Non-default
`const`/`mut`/`self` value-parameter modes remain fail-closed because current
proposition-family compatibility checking does not certify those modes.
Proposition-valued or evidence contract-call static arguments remain
fail-closed, as do symbolic const declarations or expressions, true nested
machine/conformance applications, quotients, and compiler intrinsics.
Review v50 and canonical row v10 admit unnamed public contract facts whose
proposition endpoint is a containing proposition-family parameter. The fact
retains the exact static-telescope ordinal and ordered, checked contract
expressions supplied to that family. Static-binder renames are stable, while
selecting another proposition-family slot or changing its value arguments
changes canonical evidence. Compiler validation rejects named generic
proposition evidence because the unresolved family has no exact witness
interface; proposition-valued contract-call static arguments remain a separate
incomplete form. Generic proposition law conformance now compares the exact
normalized proposition declaration and structural application; rendered labels
are diagnostic only, and a same-spelled foreign endpoint cannot discharge the
selected law. This compiler result still does not become standalone package
proof until it is carried by the total recheckable package evidence artifact.
Review v51 and canonical row v11 admit the four compiler-owned byte-sequence
predicate calls in public contract facts. The checked authored-selection row
now retains the exact closed predicate instead of one undifferentiated
intrinsic tag; projection cross-checks that identity against an unresolved,
receiver-free call before encoding it. Changing the predicate changes
canonical evidence, while a package declaration with the same spelling remains
an ordinary package-qualified callable.
Review v77 and canonical row v35 admit compiler-installed builtin-function calls
in public contract expressions. The projector rejoins the exact checked call
selection to the same fixed root slot and symbol kind, and encodes the stable
closed builtin ordinal rather than its spelling. Same-spelled late-root, nested,
package-authored, and generated symbols remain non-builtin; static arguments or
target-symbol custody disagreement reject.
Review v78 and canonical row v36 extend that closed identity to selected
boundary-operator provider execution. Compiler settlement retains the exact
builtin function beside, but distinct from, the authored realization machine;
projection rederives it from the checked overload and fixed builtin root slot.
Missing, mismatched, or non-intrinsic spoofed state rejects. Primitive-expression
intrinsics remain fail-closed until they receive their own closed atoms.
Review v79 and canonical row v37 add the first such primitive-expression atom:
named-float negation retains the exact checked `f32` or `f64` format. The atom
is selected by compiler dispatch from the exact checked boundary overload and
external realization join, never parsed from the authored realization-machine
name; that machine remains a separate package-qualified nominal. Projection
rederives and cross-checks the atom, while absent, cross-format, non-intrinsic,
and otherwise spoofed state rejects.
Review v80 and canonical row v38 close named-float conversion as one atom
containing the exact checked numeric source type, numeric target type, and
arithmetic domain. This distinguishes float-width conversion from every
float-to-integer width and signedness and distinguishes `Exact`, `Saturating`,
and `Trapping` integer results. The compiler derives all three coordinates from
the exact checked overload; changing or omitting any coordinate rejects during
review reconciliation rather than degrading to an authored name.
Review v82 and canonical row v40 retain the v81 primitive float binary atoms
and add exact atomic boundary-operator family-selection evidence. Each family
row binds the exact package-owned family path, nominal provider, selected
target, selection authority, complete-declaration coverage, and canonical
coordinate-to-plan mapping. Projection rejoins every coordinate against the
selected plan and its retained declaration provenance; absent, duplicate,
cross-family, or provider-drifting mappings reject. Generic/exact-application
coverage remains fail-closed until it receives a distinct compiler-owned
carrier.
Review v83 and canonical row v41 admit width-landed float literals in public
contract expressions, including transparent propositions whose comparison
operand is a typed named parameter and callable contracts whose comparison root
is `result`. Result landing comes from the exact return type of the owning
state, operator, or trait requirement. The row contains the checked `f32` or
`f64` format and its exact IEEE bits; decimal source spelling never enters
identity. Equivalent spellings therefore compare equally, while format or bit
changes alter the package contract. A float literal that reaches review without
one exact checked width landing remains fail-closed.
Review v84 and canonical row v42 admit explicit denotational reference
formation in public contract expressions. The row retains shared, mutable, or
write-only access plus the recursively projected target; runtime loan identity,
lifetime spelling, and diagnostic text remain absent. Proposition applications
recheck an explicit reference argument against the exact declared parameter
type before projection, so access or referee disagreement introduced after
checking rejects. Omega's implicit shared lending remains a plain argument
whose receiving parameter already carries shared-reference identity; review
does not invent syntax the typed expression does not contain. Operator-law
conformance and package rederivation compare reference access as well as the
borrowed target, preventing shared/mutable drift from satisfying the same law.
At the same v84/row-v42 schema, named operator calls in public contracts reuse
the structural call row. Projection invokes typed Psi's exact named-operator
resolver, rejoins its symbol with the authored call-selection occurrence, and
encodes the package-qualified operator target. A static namespace such as
`Token` in `Token::ordered(left, right)` is path qualification, not a value
receiver. Target drift and explicit reference arguments inconsistent with the
selected callable telescope reject. No new canonical discriminant is needed.
Review v85 and canonical row v43 admit exact atomic-load expressions in public
contracts. The row binds the recursively projected loaded value and one closed
load-valid ordering: `NoOrdering`, `Receive`, or `GlobalOrder`. Projection
requires an invalid result handle because loads have no secondary result
carrier. Store, read-modify-write, swap, compare-exchange, publish-bearing load
ordering, missing value, and post-check result-carrier drift reject rather than
being generalized into a package claim.
Review v75 and canonical row v33 likewise admit the compiler-owned collection-
length projection in public contract expressions. Checked proof-static member
resolution derives the receiver type from its retained declaration symbol,
prefers an actual package field, and selects `CollectionLength` only for `len`
on a fixed array or slice. Projection requires that exact public-interface
selection occurrence and encodes the structural receiver without inventing a
package owner. A package field named `len` remains nominal. Other compiler
intrinsics remain fail-closed.
Authored `!` and `~` likewise retain the exact operator token through checked
selection custody, including when nested in a public contract expression.
Review requires that public-interface occurrence to finalize as the closed
builtin-operator meaning before projecting the existing structural unary
operator. That custody-only change did not alter the then-current v76/row v34
bytes; it closes a source-custody join rather than adding a semantic
discriminant.
Transparent public proposition bodies now use the same recursive declaration
resolution for nominal constructors as machine contracts. Constructor type and
field selections finalize before package authority checks, and a member selected
from the resulting computed nominal value is rejoined to its exact finalized
member-selection row. The existing constructor and member expression encoding
already carries the complete semantic identity, so this custody completion does
not change review schema bytes.
Review v91 and canonical row v49 extend that exact member lane to checked
`requires` and `ensures` whose computed receiver is already in the closed
structural expression vocabulary. Projection recursively retains the receiver,
rejoins one public-interface member selection to the typed declaration, and
derives any case identity from that declaration. Missing, duplicate,
redirected, or typed-symbol-mismatched custody rejects.
Review v92 and canonical row v50 admit compiler-owned shared-slice,
mutable-slice, text-view, and bytes calls in already-checked public facts. The
existing call row retains the receiver and a new closed operation target.
Projection requires one public-interface call selection and one retained
checked intrinsic fact, then freshly rederives the operation from the final
typed receiver and checked owner environments. Same-spelled package callables
remain nominal. This does not widen the compiler's denotational-call surface;
unsupported call compositions still reject before package review. Recovery
remains v14.
Review v93 and canonical row v51 admit a named public const in a checked
contract-call const slot. The authored path retains one exact static-argument
selection to the const declaration; projection rejoins that declaration's
closed canonical integer encoding and emits the existing const-value row.
Changing the declaration value changes review identity. Private declarations,
missing or malformed canonical values, non-integer consts, and selection drift
reject. Source names and diagnostic displays are not value identity; recovery
remains v14.
Review v95 and canonical row v53 extend opaque external executable-supply keys
with a third exact requirement tag for public top-level boundary requirements.
Supported leaves are bodyless, nongeneric, lifetime-free, and payload-bearing.
Projection retains the normalized requirement-overload identity and
independently rejoins exact satisfies symbols, binding, provider type, selected
plan when present, and realization declaration. Unselected disclosure still
implies neither selection nor audit. Review v97/canonical row v55 closes the
first compiler-intrinsic execution identity for exact selected Linux
`Console::exit_process(i32) -> Unit`; targetless, non-Linux, wrong-symbol,
wrong-signature, sibling, and uncatalogued intrinsic rows remain fenced.
Recovery remains v14. This closes physical provider selection and emission,
not D39 semantic external termination. One explicit checked terminal-effect
completion identity must survive from the boundary contract through Terminal
and the selected target realization before the path can issue that observation;
Unit and backend nonreturning knowledge are insufficient.
Selected payload-bearing leaves also cross the provider-plan ABI extractor by
the same exact top-level requirement and normalized overload. Its semantic
`self` is the satisfier's explicit carrier argument, not an erasable trait
receiver. This publishes a calling row only; installed invocation and era
replay remain separate unfinished custody.
Review v61 and canonical row v19 admit exact raw byte-sequence literals in
public contract expressions. The projector uses typed Psi's decoded octets
directly and assigns them no text encoding. Escape-equivalent source spellings
therefore have identical canonical identity, while changing any octet changes
the reviewed contract. At that revision aggregate and advanced call forms
remained fail-closed; v67 and v68 later close every typed aggregate-literal
node through ordered arrays and exact nominal record/case constructors.
Review v62 and canonical row v20 admit inherited requirement surfaces for
lifetime-generic public conformances when the selected trait has no lifetime
telescope of its own. Requirement rows apply the complete inherited type
substitution before deriving alpha-normalized lifetime topology. Renaming
binders or changing private realization bodies is stable; selecting another
lifetime ordinal changes canonical identity. Review v86 and canonical row v44
extend that identity to lifetime-parameterized target traits. The conformance
header supplies every target-trait lifetime explicitly;
each resolves to an alpha-normalized declaration-order ordinal in the
conformance telescope and is retained beside the target type arguments through
checked closure, inherited requirement substitution, public review, and
canonical encoding. Binder renames are stable and another ordinal is a different
public conformance. Package review consumes the already-resolved mapping and
never repeats application-site inference.

D55 applies the same explicit source syntax and validation to one machine's
exact `satisfies Trait<...>::requirement` edge while using a different public
normal form. Raw ordinals into the realizing machine telescope remain checked
substitution custody. Review first-occurrence-normalizes them in target-trait
parameter order and retains only the equality partition: implementation binder
renames, reordering, and unused-binder insertion are stable, while `[0,0]` and
`[0,1]` remain different borrow contracts. Checked and external realizations
share that edge identity, and opaque external supply remains non-proof.
Review v63 and canonical row v21 admit selected generic-conformance
applications in public generic bounds. The row retains the exact
package-qualified conformance declaration, alpha-normalized lifetime
arguments, categorized type/const/machine arguments, instantiated subject, and
the exact public trait with its instantiated type arguments. Checked closure
first verifies the complete declaration telescope; review then independently
rejoins those semantic declarations rather than trusting display strings.
Binder renames remain stable, while any changed application argument changes
canonical identity. Proposition/evidence arguments and non-public selections
remain fail-closed.
Review v64 and canonical row v22 admit the proof-only representation
observation `zero_value<T>()` in public contracts. Its row retains the exact
package-qualified, alpha-normalized target type; no derived layout bytes,
diagnostic spelling, or checker verdict become identity. Proposition-local type
binders receive exact symbols before typed lowering, so renames are stable and
changing the observed type changes canonical evidence. The existing quotient
observer fence rejects quotient targets before package review.
Review v65 and canonical row v23 retain outcome-specific `ensures` as guarded
proof-interface rows rather than silently omitting or publishing them as
unconditional facts. Each row carries exact package-qualified result-data and
result-case identity, the public selector when named, checked evidence-lane
position, and canonical fact. Review requires exactly one matching checked
producer carrier. Group/row reordering is stable; changing the arm or public
selector changes comparison identity.
Review v66 and canonical row v24 retain public-operator crash ceilings. Checked
lowering emits one operator-symbol-keyed row for every root and domain-homed
operator, including an explicit empty ceiling, and review rederives the whole
table before projection. Canonical cause buckets contain truth or exact
structural guard expressions, preserving package-qualified calls, members, and
declared overloads instead of runtime-predicate display fallbacks. Reordering
or duplicating routes is stable; changing a cause or guard changes identity.
Review v67 and canonical row v25 admit ordered array literals in public
contract expressions. Every element recursively projects through the same
closed structural vocabulary and limits, including nested arrays. Element
order is semantic identity, and an unsupported child keeps the whole row
fail-closed.
Review v68 and canonical row v26 admit nominal record and sum-case constructor
expressions. Rows use exact package-qualified data, optional case, and field
identities with recursively projected values. Fields sort by exact identity,
so authored order is stable; changing the case, field, or value changes
comparison identity. Unresolved/mismatched symbols and private public-interface
selections reject.
Review v69 and canonical row v27 admit indexed and ranged contract expressions.
The authored `[` token enters declaration-selection custody, and checked
lowering must finalize one exact public-interface `Index` or `Range` meaning
before review. Canonical rows retain builtin versus declared meaning, the
collection, scalar index or optional range endpoints, and inclusivity. Changes
to any of those semantic fields change comparison identity; missing, ambiguous,
or mismatched checked custody rejects. Indexed children compose recursively
inside arrays.
The v69/row-v27 encoding remains unchanged while checked custody now covers
public operator declarations. Each non-crash operator `requires`/`ensures` fact
must rejoin exactly one `OperatorDeclaration` owner keyed by the declaration's
exact symbol before its existing structural row projects. Missing, duplicate,
or mismatched owner/kind/fact custody rejects. This introduces no named
operator-contract syntax or evidence lane; operator contracts remain unnamed.
Review v52 and canonical row v12 add one blocking standalone row for every
package-owned `pub proposition`, including an unused bodyless declaration.
The row retains alpha-normalized binders, parameter types, witness interface,
or normalized transparent expansion. A primitive row publishes vocabulary and
does not create a checked fact or admission. Transparent aliases remain source
compatibility surface even though proposition applications normalize through
their expansion.
Review v53 and canonical row v13 add one blocking `PublicConst` row for every
package-owned public const, including an unused declaration. Its compatibility
identity is the exact package-qualified declaration, exact typed declared
type, and canonical structural value. Source spelling, rendered values, and
runtime storage identity do not enter the row. If the declared type exposes
private data or the compiler cannot yet canonicalize the declaration value,
publication rejects instead of manufacturing a weak identity. Type or value
changes therefore become source-backed `public_const` conflicts; private
const-v0 declarations remain unchanged and unprojected. The parsed initializer
occurrence survives value substitution as source custody and is rendered under
the closed `const_initializer` role; its spelling remains outside semantic row
identity.
Ordinary `pub operator` visibility now survives checked compilation as
declaration-owned metadata, independent of a carrier-qualified path. The
operator symbol keeps its own authored source provenance, and proof-static
late selections may finalize only from exact typed operands before visibility
is checked again. Cross-package private selection rejects; same-owner private
implementation use remains legal. Review v54 and canonical row v14 add a
blocking `PublicOperator` row keyed by package-qualified declaration identity
plus the compiler's canonical operand and result-dispatch identities. The row
retains boundary status, fixed spelling, complete signature shape, and directly
projected declaration contracts even when unused. Binary contract expressions
now name the exact declared overload or explicit builtin meaning. Unresolved
proof-static selections reject closed.
Complete name-first conformances now carry declaration-owned `pub` through
syntax, source profiling, resolved/typed/checked trees, and stage snapshots.
The common exact-symbol gate rejects private cross-package selection and
public-interface citation; public headers also reject private carrier or trait
selection, while private member realization stays private implementation.
Lexical conformance-binder requirement symbols inherit their enclosing
declaration instead of acquiring package visibility. Explicit conformance-row
machine references enter authored-selection custody after exact row
normalization and obey ordinary package visibility. The canonical
review v55/canonical-row v15 `PublicConformance` lane is blocking and keyed
only by the exact package-qualified conformance identity. Its value retains the
alpha-normalized lifetime/static telescope, subject, exact trait application,
and complete normalized inherited requirement interface. Requirement overloads
use canonical compiler callable identity rather than ordinal or display path.
Closed and attached-machine realization forms encode identically; realization
names, bodies, and physical code remain private implementation. Validation
checks every realization signature and substituted trait law before projection,
while the referenced `PublicTrait` row owns the law text. Recovery and
fixed-vocabulary upgrade conflicts recognize the row. Unsupported
lifetime-parameterized target traits, inherited lifetime substitutions, and
proof-static trait parameters reject closed instead of producing partial
identity.
Exact requirement-local `satisfies` edges remain authored selections even
though they do not mint a whole conformance. Trait edges retain the exact trait
application and result-dispatch-selected requirement; operator edges retain the
exact signature-selected overload. Lifetime-parameterized trait edges retain
raw machine-binder ordinals for checking and the D55 normalized equality
partition for public identity. The realizing machine's interface exposure
governs both rows. Identity settles before checked, boundary, accepted, or
external supply policy, and downstream consumers cross-check the retained
symbol instead of reselecting by spelling.
Domain `established by Trait::requirement` paths retain the same exact trait
and requirement coordinates at signature-free normalization, after uniqueness
and subject authorization are proved. Each source occurrence inherits the
domain's exposure even when the normalized semantic route set deduplicates an
equivalent alternative.
Nominal callable machine-parameter contracts preserve the complete authored
`Trait::requirement` path after signature-free resolution. Typed lowering emits
the exact trait and requirement selections under the enclosing declaration's
exposure, including recursively nested contracts; transitive-only and private
public-interface selections reject before package review.
In particular, true nested machine static applications such as
`consumer<family<Selected>>()` now reject during compiler validation, before
checked lowering. Treating the argument as the uninstantiated `family`
declaration checked the wrong callable shape; monomorphization also has no
closed recursive application identity in conflict equality, specialization
keys/fingerprints, or retained specialization evidence. Supporting this form
requires recursive specialization plus exact declaration-telescope, lifetime,
and static-argument identity throughout those paths. This is distinct from
already coherent bare generic-machine selection and call-target use such as
`Schema<Selected>(...)`.
Proposition applications use their exact checked rows. A simple total, pure
callable application retains its optional receiver, exact checked package-
qualified entry target, and ordinary arguments after joining one public-
interface declaration-selection row. The whole-source commitment separately
pins the helper body; a callable signature is not body identity. Symbolic const
declarations or expressions, proposition/evidence static arguments, quotient
calls, true nested machine/conformance applications, and other compiler-
intrinsic calls reject until exact rows are settled. Public domain semantic
roles project from the exact typed declaration as closed compiler-owned tags.
Each contribution must point back to the declaration's own typed semantic
identity; canonical evidence retains the package-qualified domain and role,
never the compiler-private semantic ID or a role inferred from its name.
Public domain operators remain separate exact `PublicOperator` rows.
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
explicitly binder-free rather than fabricating evidence. A selected conformance
retains its exact package-qualified declaration, complete alpha-normalized
application, instantiated subject, and underlying public-trait application;
semantic declarations own those symbols rather than report code reselecting
names. Public trait requirements retain named and unnamed
`requires` and `ensures` through the same closed structural fact/expression and
evidence vocabulary as public callables, joined to their exact checked
state-signature owner. Named inputs retain ordered proposition and evidence-
interface identity while treating their source aliases as local. Named outputs
also retain their public selector identity. Their
abstract published crash ceilings are projected from exactly one checked
trait/requirement capsule into canonical cause-and-guard routes; no realized
body sites or calls are fabricated. Selected generic-conformance applications
use the complete v63 canonical row; proposition/evidence application arguments
and unsupported expression forms remain fail-closed.
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
machine/proposition parameters, and non-public or lifetime-parameterized trait
realizations reject until their complete canonical forms are represented,
except that generic binder-free requirements, explicit evidence binders, and
selected conformances with representable complete applications use the same
canonical conformance row as public traits. Review v71/canonical row v29 also
binds an unaliased checked-body realization to the exact package-qualified
coordinate of one public ordinary nongeneric, lifetime-free operator, whether
or not that declaration owns a fixed token. Checked lowering
retains the exact machine/operator symbols, conformance/admission form,
normalized overload shape plus exact lifetime-bearing type nodes, both complete canonical contract sets, and exact
typed semantic snapshots of their contract graphs in full; projection requires
exact rederivation before rerunning the checked signature resolver and equality/
`&&` `requires`/`ensures` contract judgment. Post-check
redirection and coordinated provider-contract mutation both reject. This
retained compiler-private baseline is neither a hash nor persisted package
evidence, and trusted compiler components remain inside the TCB. Operators with
outcome-specific or crash contracts, and providers with any nonempty checked
crash behavior, reject until their refinement rules exist.
Private, generic/lifetime-parameterized, aliased, and bodyless checked
realizations remain fail-closed. Operator-bound external supply uses the
separate tagged v72/canonical-row-v30 trust association described below rather
than a trait-conformance shape. A fixed-token checked realization points to
the same exact declaration coordinate as its named call surface; the joined
public-operator row owns the closed compiler spelling. The callable edge does
not duplicate it or create another identity. Checked-body boundary realizations
use the same satisfaction edge, while the existing selected-provider set alone
identifies the active target plan and rejoins its exact operator requirement and
realizing machine. Projection repeats the exact symbol, slot, checked-adapter
binding, package, and machine join. A named-boundary canary covers unique
selection. Review v94/canonical row v52 admits selected, unaliased, nongeneric,
lifetime-free checked adapters for fixed-token binary arithmetic/comparison and
indexing declarations. Dispatch rejoins the exact checked use, token, operand
shape, compact selected-plan coordinate, and strong plan commitment. Range,
unsupported arities, aliases, bodyless/externalized realizations, generics,
lifetimes, and evidence drift remain fail-closed. This lane makes no Terminal
or native-realization claim. Authored override of a same-path overloaded
operator family is atomic: package review retains the canonical coordinate set,
selected nominal provider, complete per-coordinate mapping, and independently
reconstructed exact-application coverage. Adding a family coordinate
invalidates an incomplete recorded override. The projection never substitutes
an overload display name, declaration order, or a runtime-layout-only type
identity for this contract surface.

Production exact package applications remain fail-closed until D29 is
implemented. A checked use owns an ordered type/const application; const
identity is its canonical evaluated value in the declared carrier. Generic
artifacts may export typed symbolic demands, but coverage exists only after
final substitution closes every binder and rejoins the exact selected plan.
Checked generic bodies use ordinary authoritative specialization, while
bodyless or external supply requires independently admitted concrete authority.
The retained realization is a role-tagged sum rather than a common row with
optional specialization fields. A boundary operator whose telescope has
length zero has one cheap canonical empty application; an ordinary boundary-
trait machine has no telescope and never uses that value. Bootstrap lowering
cannot publish authoritative coverage.
Under D28, every emitted artifact retains this finite exact set even if a future
checked generic body proves universal semantic selection coverage. No such
checked generic operator realization exists today, so generic coverage remains
deliberately unrepresented; provider assertions and one successful
specialization grant nothing.

D32 keeps semantic evidence separate from native physical realization. The
immutable canonical Terminal artifact feeds a validated optimization
projection. Every surviving settled boundary occurrence has one
`NativeArtifact` physical child. Its role-tagged `PhysicalChildParent` is
either an `OperatorApplicationCoverageRef` to reconstructible D29 coverage or
a complete replayable D41 `BoundaryTraitSettlement`. Equal D29 applications
may share a semantic parent row but not a physical child. Native replay derives
the survivor set and rejects missing, duplicate, stale, substituted, padded,
or role-swapped children. Package review does not claim assigned homes,
relocation, or emitted bytes merely because its semantic D29 row is complete.

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
semantic-place row and exactly one finalized public-interface member-token
selection to the same field. Missing, duplicate, or mismatched custody rejects.
Computed members, proposition-argument members without that
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
- exact provider requirements, selected realizations, origins, disclosed
  execution scope, root-owned admissions, and executable TCB entries;
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
Observation-summary schema v20
carries operation-attempt schema v18, retaining each completed operation's exact
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
Resolved/Null/Unknown logical lifetimes immediately after successful typing. A
later preparation failure keeps the completed prefix, while a fully prepared
call must reproduce the exact logical-handle plan. Successful opens mint monotonic IDs;
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
validated at-family component bytes. Mutable byte and i64 carriers retain a
distinct complete resolution-time snapshot as their operands are evaluated, so
a later preparation halt keeps the prefix. Mutable byte carriers separately
retain complete provider pre/post capacity, including unchanged tails, and
mutable i64 carriers retain exact provider pre/post values. Provider pre-state
follows evaluation of every authored argument because a later argument may
alias an earlier carrier; resolution and provider snapshots need not match.
Post-state follows provider return or halt, including unchanged input-only ABI carriers.
Rooted/path-alias spellings stay out of the payload lane. A separate 256 MiB
aggregate operand-evidence sponsor covers immutable bytes, exact path-like and
rooted-resolution bytes, exact returned-path prefixes, one mutable resolution
copy, and both provider copies. Directory-
entry names, symlink targets, find patterns, and other non-rooted path-like
operands occupy their own ordinal-tagged lane rather than impersonating rooted
authorization or payload. Each successfully typed non-handle scalar, immutable
payload, path-like operand, and rooted path is retained as preparation advances,
so a later preparation halt keeps the completed ordinal prefix. Rooted rows
carry exact ordinal, closed Source/Output identity, and canonical relative bytes
before physical provider-path lowering; they are input resolution rather than
authorization, which separately carries access and may select a different
canonical rooted location. The fully prepared call must reproduce each complete
compiler-private semantic sidecar before provider access. Prior or nested
staging effects remain cleanup-contained. Package commitments hash immutable, path-like,
rooted, returned-path, and mutable rows without rendering them. Successful
provider branches retain exact meaningful output bytes without terminators or
stale tails, plus output ordinal, closed kind, and Complete/LimitReached state.
Provider-known target length distinguishes exact-fit from truncated `read_link`;
failure and insufficient-capacity returns add no row. Package-rooted builds
reject canonical and final absolute output, while `read_link` remains inert.
Successful `read`/`read_at` calls designate the exact zero-offset region of the
already-retained mutable post-carrier as sequential or positioned file content.
The length equals the nonnegative result; EOF retains an empty row and failure
retains none. The row copies no bytes and adds no sponsor charge. Package
commitments bind its kind and coordinates plus the referenced mutable
post-state. `read_dir` similarly designates exact `DirectoryRecords`, while
`find_first` and entry-producing `find_next` designate complete 320-byte
`FindEntry` records. Directory EOF and no-entry find returns retain empty rows;
failed enumeration retains none. Successful path, descriptor, and no-follow
metadata operations retain one target-neutral canonical row containing all 14
`StatRecord` fields. The compiler extracts and validates the selected target's
already-checked `StatLayout<StatRecord>` from its earliest coherent private
typed/layout state, then gives only that closed descriptor to the Psi evaluator.
The evaluator zeroes and serializes the complete authored ABI carrier (whose
API minimum is 144 bytes) through the descriptor and checks it against the
semantic row; package commitment binds both representations. Filesystem-
reaching builds load and check the standard layout policy before execution.
This does not publish an internal IR contract
or justify nominal Chi. Complete replay remains absent, so this makes no
receipt, replayability, or source-rebuildability claim. Sponsored
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
neither retained nor leaked through the count by this staged-tree
representation: every linked name is committed as ordinary regular-file
content. This is output-tree custody and replay only. Canonical operation
replay, recorded observed inputs, generated-
output handoff, and the complete record replay checker remain required before
any `Receipted` verdict. This rung does not claim hostile same-user race
exclusion.
For one or more complete, non-interleaved Source-rooted source-read chains, the
compiler performs bounded replay with no filesystem provider. Each chain
contains one flags-zero `open`, one or more `read`/`read_at` calls on its
distinct created descriptor, and its exact retiring `close`. Sequential reads
advance that chain's implicit zero-based cursor by their successful result;
positioned reads bind an exact nonnegative offset without advancing it. Ordered
operation kinds, counts, offsets, results, carriers, and observed regions
determine cursor semantics without separately trusted fields. Zero reads,
failed reads, descriptor reuse, cross-chain operations, interleaving, and
incomplete chains reject. Replay consumes rows in order, supplies recorded
results and read bytes, reconstructs descriptor lifetimes, and requires exact
event exhaustion, observations, and final result. Summary v22 binds that
partial replay fact. Compiler replay-record v4 preserves every lane of the
verified chains in a bounded canonical binary form, rejects stale semantic
schemas and inconsistent operation-specific state, and is retained by review-
baseline capsule v2 under
a parent-observation association and aggregate byte ceiling. This is durable
review-only custody, not proof of authenticity, review, or admission. Checked
compilation can now strictly decode the reopened bytes into the PSI executor's
exact typed source-read chains and evaluate the build machine without a host
filesystem provider. Retained source bytes serve the replay even after host
source drift; changed authored paths, counts, positioned offsets, operation or
region kinds, and event structure reject. No public IR stage or nominal Chi is
introduced. Direct unsponsored host execution remains `Volatile`; broad
operation replay, output mutation and staged-output reproduction, package-
command integration, and receipts for other grammars remain absent.

Observation summary v23 and compiler replay-record v5 generalize this custody
to ordered source-input events. Successful Source-rooted `read_metadata` and
`read_symlink_metadata` calls may appear around the closed read chains. Their
authored rooted path and separately authorized target, follow/no-follow kind,
all 14 canonical metadata fields, and complete target carrier survive restart.
Recovery validates the exact operation lanes and both canonical relative
paths. Provider-free replay reconstructs the complete selected checked
`StatLayout` carrier, compares every field, zero padding, and tail byte, and
then requires exact event/result exhaustion. Failed and descriptor-backed
metadata remain outside this rung. This changes neither the `Volatile` verdict
nor the absence of an audit, authenticity, admission, or receipt claim.

Observation summary v24 and compiler replay-record v6 close one deliberately
narrow end-to-end case. After one or more admitted Source-input events, the
only accepted Output suffix is a direct-child ordinary file created with mode
438, written once in full from one immutable payload, closed with exact
descriptor retirement, and handed off once as that exact generated source.
Source observations remain record-served; Output operations execute in a fresh
virtual namespace. Exact replay equality, exact handoff, namespace quiescence,
and exact reproduced path/bytes are mandatory. On the initial run, the compiler
also requires the independently reconstructed one-file tree to equal sponsored
physical staged-tree custody before the realized class becomes `Receipted`.
Unsponsored execution cannot publish such a record. Reopened custody repeats
the no-host execution and reconstructs generated source after host Source and
Output drift. The static filesystem ceiling remains `Volatile`, and every
broader operation or tree shape remains outside this receipt. This is build-
operation evidence; it makes no claim that a human or LLM audit occurred. A
separate 16 MiB aggregate replay-retention ceiling rejects before cloning, and
validated attempt custody is shared across evaluator handoff.

Observation summary v27 adds the zero-mutation completion of that grammar. A
successful Source-input-only build reconstructs the canonical empty Output
tree after complete provider-free event replay, exact result equality, empty
generated-source handoff, and replay-namespace quiescence. Direct initial
issuance still requires equality with independently sponsored physical empty
Output custody; unsponsored host execution stays `Volatile`, and any
unexplained sponsored entry rejects. Reopening replay-record v8 executes in a
fresh virtual namespace and reconstructs the empty tree without consulting
host Output. The record remains non-authoritative, and package admission
separately rejoins its canonical Source-metadata identity to current compiler-
validated package custody. Earlier summary schemas reject through the existing
semantic-schema binding; record framing is unchanged.

Observation summary v28 and compiler replay-record v9 distinguish an ordinary
build artifact from generated source. After admitted Source-input events, the
same exact direct-child ordinary-file `create(438)`/full-write/close chain may
finish without `include_source`. Replay executes the Output file in a fresh
virtual namespace, requires the handoff to remain absent, and reconstructs the
exact one-file tree. Initial issuance still requires equality with independent
sponsored staged-tree custody, and unexplained entries reject. Reopening ignores
host Output drift and does not add the artifact to the Omega source set. Record
v9 retains an explicit absent-or-present handoff disposition rather than
inferring publication from the output file. It remains non-authoritative and
must still rejoin its Source-metadata identity to current compiler-validated
package custody.

Observation summary v29 and compiler replay-record v10 generalize only the
ordinary-artifact cardinality. After the Source-input prefix, a nonempty
sequence of distinct direct-child files may each use the same exact
`create(438)`/full-write/close chain, with no generated-source handoff. Paths
and logical descriptors must be distinct, chains cannot interleave, and replay
executes every operation in authored order in the fresh virtual namespace. The
final namespace must equal exactly those files and bytes with no live handles
or other namespace state. Canonical tree identity sorts files independently of
chain order before exact sponsored-custody comparison. Existing replay,
staged-entry, path, and unique-content ceilings remain in force. Nested paths,
directories, other operations, handled failures, and multiple generated-source
handoffs remain outside this rung.

Observation summary v30 and compiler replay-record v11 close the explicit-
publication cardinality for that repeated-file grammar. Any ordered subset of
the distinct output files may be handed to `include_source`, while unselected
files remain ordinary artifacts. Each summary and record row binds the exact
Output-relative path and completed-filesystem-attempt ordinal in authored call
order. Ordinals are nondecreasing, every path is unique, and no handoff may
precede its matching successful close; multiple calls may share an ordinal and
handoff order may differ from output-chain order. Replay authorizes only the
next exact path at that exact ordinal and requires complete sequence equality
at teardown. Existing generated-source filename, regular-file, reserved-name,
final-frontend, sponsored-custody, and resource checks remain unchanged. A
filename still cannot implicitly publish source.

Observation summary v31 and compiler replay-record v12 generalize each file
chain to `create(438)`, one or more complete sequential writes, and close. Each
write uses the fresh descriptor, returns its complete immutable operand length,
and preserves zero post-error state; zero-length writes remain valid. Replay
executes the exact ordered calls against the fresh virtual cursor and
reconstructs final bytes by checked concatenation. Interleaving, partial or
failed writes, seek, positioned writes, descriptor duplication, and reopen
remain outside the grammar. Handoff validation now uses each variable-length
chain's actual close ordinal. Existing resource, tree, custody, publication,
and final-frontend gates remain unchanged.

Observation summary v32 and compiler replay-record v13 admit complete
positioned writes in those fresh-file chains. A chain may mix sequential
`write` and absolute-offset `write_at` calls in authored order. Positioned
writes overwrite or extend with zero filling, do not advance the sequential
cursor, and bind their exact nonnegative offset in the retained operation row;
zero-length positioned writes are retained no-ops and do not extend the file.
Final bytes are reconstructed by the same ordered cursor/extent semantics and
must equal both fresh virtual replay and independent sponsored Output custody.
Negative or malformed offsets, partial or failed writes, extent overflow or
retention excess, interleaving, seek, descriptor duplication, and reopen remain
outside the grammar. Existing handoff and frontend gates are unchanged.

Observation summary v33 and compiler replay-record v14 admit a freshly created
empty Output file without requiring a synthetic zero-byte write. The file
grammar is now `create(438)`, zero or more complete sequential or positioned
writes, then close. `create` followed immediately by `close` reconstructs one
ordinary zero-byte file and must still agree with fresh virtual replay and
independent sponsored Output custody. A zero-byte write remains a distinct
retained operation when authored; the compiler never invents one. Missing or
failed close, interleaving, and every otherwise unsupported operation still
reject. Internal replay vocabulary now names the unit an Output file rather
than a write chain.

Observation summary v34 and compiler replay-record v15 admit successful
`sync` and `sync_data` operations anywhere between a fresh Output file's create
and close. They bind the same live descriptor, exact authored operation kind,
zero result, and zero post-error state. Replay preserves their order while they
leave bytes, extent, and cursor unchanged, then still requires exact operation,
namespace, tree, and sponsored-custody equality. Failed syncs, malformed lanes,
other descriptors, and sync after close remain non-receipted.

Observation summary v35 and compiler replay-record v16 admit successful
nonnegative `set_len` operations anywhere between a fresh Output file's create
and close. Each row binds the exact requested length and same live descriptor,
returns zero with zero post-error state, truncates or zero-extends the replayed
file, and leaves the sequential cursor unchanged. Resource policy binds peak
extent across the authored operation sequence, not merely final extent, so a
large extension followed by truncation cannot evade replay limits. Negative or
unrepresentable lengths, failures, malformed lanes, and wrong descriptors
remain non-receipted.

Observation summary v36 and compiler replay-record v17 admit successful
canonical seeks within a fresh Output file. `SEEK_SET`, `SEEK_CUR`, and
`SEEK_END` bind exact signed offset, whence, same live descriptor, nonnegative
result, and zero post-error state. The checker recomputes the result from the
current cursor and extent with checked arithmetic before accepting it; replay
then updates only the sequential cursor. Unsupported whence values, negative or
overflowing results, mismatched claimed results, failures, malformed lanes, and
wrong descriptors remain non-receipted.

Observation summary v37 and compiler replay-record v18 admit successful
descriptor-scoped `set_file_permissions` between a fresh Output file's create
and close. The retained row binds the exact authored `u32` mode, same live
descriptor, zero result, and zero post-error state. Replay retains the final
permission operand and derives the canonical staged-tree ordinary/executable
class from its execute bits without consulting physical Output metadata. It
does not alter bytes, extent, or cursor. Failed calls, malformed lanes, wrong
or closed descriptors, and path-based permission changes remain
non-receipted.

The current exact Output-tree increment, observation summary v45 and compiler
replay-record v26, admits successful hard links at portable tag 19 and Win32
tag 27. Both canonical names must remain under the same Output root with write
authorization for the existing and new names. The existing name must be an
earlier regular-file or hard-link entry; authored order, provider-specific
operand order, successful result spelling, zero post-error state,
parent-before-child ordering, and destination collision checks remain exact.
Provider-free replay recreates the link relation. The sponsored staged-output
commitment deliberately normalizes each linked name to duplicate regular-file
content and does not retain inode identity or hard-link topology. Missing,
late, directory, or symbolic-link sources, cross-root names, insufficient
authority, collisions, alternate operations, and failures remain
non-receipted.

Observation summary v46 and replay-record v27 additionally admit exact
successful Source-rooted `read_link` events at tag 21 within the ordered Source
prefix. The record binds the authored rooted symlink name, separately
authorized no-follow target, requested count, scalar result, post-error state,
complete mutable resolution/pre/post carrier, and exact returned target bytes.
Complete targets and capacity-limited prefixes remain distinct; no unseen
suffix is inferred from a truncated result. Provider-free replay restores the
exact carrier and event order. Returned target bytes remain inert and require a
new checked root resolution before any path use.

Observation summary v47 and replay-record v28 also admit a nonempty exact
Output tree beginning at filesystem attempt zero. Constant-generating builds
need no synthetic Source filesystem event; the empty Source-event prefix is
replayed vacuously. Canonical package Source metadata and compiler custody are
still mandatory and independently revalidated. Generated-source ordinals begin
at zero, while the existing exact ordered directory, file, symbolic-link, and
hard-link grammar remains unchanged. Empty streams, malformed prefixes,
unexplained physical Output, and changed canonical Source identity remain
non-receipted.

Observation summary v48 and replay-record v29 additionally admit exact Source
directory-enumeration chains: one flags-zero Source open, one or more successful
tag-23 `read_dir` calls, and exact descriptor retirement. Every call binds its
count, result, post-error state, exact record-byte region, complete byte-carrier
resolution/pre/post states, and complete mutable cursor resolution/pre/post
states. Provider-free replay restores both carriers in authored order. Packed
records are target-specific inert bytes; they confer no name/path authority and
make no claim about entries the build did not observe. Failed calls, incomplete
chains, malformed tails, changed counts, and reordered calls remain
non-receipted.

Observation summary v49 and replay-record v30 additionally admit the first
exact failed-operation lane: one or more authorized tag-9 removes of canonical
Output-rooted paths, each returning `-1` with post-error state `2` and leaving
the fresh virtual Output namespace empty. Replay binds the rooted operand and
matching write authorization, permits an optional exact Source prefix, and
forbids generated-source handoffs. It retains at most 4,096 attempts and 16 MiB
of aggregate path spelling. Refused or unrooted paths, other errors, successful
removes, and mixed mutation/failure lifecycles remain non-receipted.

Observation summary v50 and replay-record v31 additionally admit an optional
exact Source prefix followed by exactly one failed tag-8 close of an Unknown
descriptor. The closed row is scoped-real provider, scalar result `-1`,
post-error state `9`, and one operand-zero `Descriptor/Unknown` logical input;
all paths, raw tokens, mutable carriers, outputs, retirements, refusals,
diagnostic strings, and generated-source handoffs are absent. Provider-free
replay reproduces the exact failure against a fresh virtual handle table and
verifies empty namespace and teardown. That complete no-effect sequence may
receive an empty staged-output commitment on its initial run; Source-only replay
remains partial. Null/resolved inputs, alternate failures, repetitions, and
mixed lifecycles remain non-receipted.

Observation summary v51 and replay-record v32 generalize that exact row to the
complete operand-free unknown-descriptor family: tag-8 `close`, tag-43 `sync`,
tag-44 `sync_data`, or tag-45 `duplicate`. The optional exact Source prefix and
single-operation bound remain fixed. Every member retains scoped-real provider,
scalar `-1`, post-error state `9`, and exactly one operand-zero
`Descriptor/Unknown` logical input while every other lane is empty. The exact
tag is replayed against a fresh virtual descriptor table and receives empty
staged-output custody only after exact attempt and teardown equality. Operations
with additional authored operands, alternate handle kinds, repeated failures,
and mixed lifecycles remain non-receipted.

Observation summary v52 and replay-record v33 replace the two replay booleans
with one version-1 closed disposition: `NotReplayed`, `SourceInputsOnly`, or
`Complete`. The partial disposition claims only provider-free Source-input
execution with exact result and observation equality. `Complete` additionally
requires the exact attempted operation sequence, generated-source handoffs,
virtual namespace and teardown, and matching staged-output commitment or
sponsored custody. The compiler fails closed if complete replay lacks source
replay or staged-output custody. Package observation identity binds the verdict
schema and disposition with the attempts, handoffs, and tree. This remains
compiler observation evidence, not an audit attestation or package-admission
decision. Host CPU and RSS controls are deployment availability policy; they
do not strengthen the evidence or turn review into authority.

Observation summary v53 and replay-record v34 additionally admit an optional
exact Source prefix followed by one failed tag-10 `seek` on an unknown
descriptor. The row binds scoped-real provider, scalar `-1`, post-error `9`,
operand-zero `Descriptor/Unknown`, and the authored operand-one `i64` offset
and operand-two `i32` origin; every other lane and generated-source handoff is
empty. Provider-free replay uses the exact scalars in a fresh virtual descriptor
table and issues empty staged-output custody only after attempt, result,
namespace, and teardown equality. Alternate scalar shapes, handles, failures,
repetition, and mixed lifecycles remain non-receipted.

Observation summary v54 and replay-record v35 additionally admit one exact
write-gated scalar operation on an unknown descriptor after the optional Source
prefix: tag-17 `set_file_permissions(u32)`, tag-41 `set_len(i64)`, tag-46
`lock_file(i32)`, or tag-49 `change_file_owner(i32, i32)`. Every row binds
scoped-real provider, scalar `-1`, post-error `9`, operand-zero
`Descriptor/Unknown`, and exact authored scalar ordinals and values while all
other lanes and handoffs remain empty. Missing-descriptor rejection occurs at
the compiler write-grant lookup before host mutation; provider-free replay must
reproduce the selected operation, namespace, and teardown before empty
staged-output custody issues.

Observation summary v55 and replay-record v36 additionally admit one failed
tag-42 `set_file_times` on an unknown descriptor after the optional Source
prefix. The row binds operand one's complete authored mutable carrier as equal
resolution and provider pre/post bytes, requires at least the 32-byte timespec
pair, and fixes scoped-real provider, scalar `-1`, post-error `9`, and
operand-zero `Descriptor/Unknown`; all other lanes and handoffs remain empty.
Missing-descriptor rejection occurs at write-grant lookup before host mutation.
Provider-free replay restores the exact carrier and must reproduce the attempt,
namespace, and teardown before empty staged-output custody issues.

Observation summary v56 and replay-record v37 additionally admit one failed
tag-4 `read` or tag-6 `read_at` on an unknown descriptor after the optional
Source prefix. The row binds the authored `u64` count, the positioned read's
`i64` offset, and operand one's complete unchanged mutable carrier, with the
count bounded by that carrier. It fixes scoped-real provider, scalar `-1`,
post-error `9`, and operand-zero `Descriptor/Unknown`; no failed transfer
region, other lane, or handoff is present. Compiler-owned descriptor lookup
rejects before a host read. Provider-free replay must reproduce the attempt,
namespace, and teardown before empty staged-output custody issues.

Observation summary v57 and replay-record v38 additionally admit one failed
tag-5 `write` or tag-7 `write_at` on an unknown descriptor after the optional
Source prefix. The row binds operand one's complete authored immutable payload
and the positioned write's operand-two `i64` offset. It fixes scoped-real
provider, scalar `-1`, post-error `9`, and operand-zero
`Descriptor/Unknown`; every other lane and handoff is empty. Compiler-owned
write-grant lookup rejects before sponsor accounting or host mutation.
Provider-free replay must reproduce the attempt, namespace, and teardown before
empty staged-output custody issues.

Observation summary v58 and replay-record v39 additionally admit one failed
tag-39 `read_file_metadata` on an unknown descriptor after the optional Source
prefix. The row binds operand one's complete authored mutable carrier as equal
resolution and provider pre/post states after the preparer's 144-byte
metadata-ABI minimum. It fixes scoped-real provider, scalar
`-1`, post-error `9`, and operand-zero `Descriptor/Unknown`; no metadata
observation, other lane, or handoff is present. Compiler-owned descriptor
lookup rejects before host metadata access. Provider-free replay must reproduce
the attempt, namespace, and teardown before empty staged-output custody issues.

Observation summary v59 and replay-record v40 additionally admit one failed
tag-30 `get_osfhandle` on an unknown descriptor after the optional Source
prefix. The row fixes scoped-real provider, scalar `-2`, unchanged post-error
`0`, and operand-zero `Descriptor/Unknown`; every other lane and handoff is
empty. Both evaluators consult compiler-owned synthetic descriptor tables, so
provider-free replay checks only Omega's modeled bridge. It claims neither
native-handle custody nor a Windows security property.

Observation summary v60 and replay-record v41 additionally admit one failed
tag-29 `close_handle` on an unknown native handle after the optional Source
prefix. The row fixes scoped-real provider, scalar `0`, post-error `6`, and
operand-zero `Native/Unknown`; every other lane and handoff is empty.
Provider-free replay checks only the compiler-owned synthetic handle model,
not native-handle custody or a Windows security property.

Observation summary v61 and replay-record v42 additionally admit one failed
tag-31 `final_path_name_by_handle` on an unknown native handle after the
optional Source prefix. The row binds the complete unchanged mutable carrier,
its bounded `u64` capacity, and `u32` flags, while fixing scoped-real provider,
scalar `0`, post-error `6`, and `Native/Unknown`. No returned path exists.
Provider-free replay checks only the compiler-owned synthetic handle model,
not native path/handle custody or a Windows security property.

Observation summary v62 and replay-record v43 additionally admit exactly one
failed synthetic-native mutation after the optional Source prefix: tag 32
`set_file_time`, tag 33 `lock_file_ex`, or tag 34 `unlock_file`. Exact authored
scalars and complete FILETIME or OVERLAPPED carriers are retained; every row
fixes `Native/Unknown`, scalar `0`, and post-error `6`. Both evaluators reject
before sponsor accounting or host mutation. This is only modeled
invalid-handle replay, not native handle, lock, timestamp, or Windows security
custody. Provider-state reads such as tag 35 remain outside the family.

Observation summary v71 and replay-record v51 complete the separate ordered
error-state grammar. Any already-receipted exact unknown-native-handle failure—
tag-29 `close_handle`, tag-31 `final_path_name_by_handle`, or tag 32 through 34
mutation—may be followed immediately by operand-free tag-35
`get_last_error`, returning modeled error `6` with unchanged post-error `6`.
Provider-free replay reconstructs the selected typed failure before the read.
Standalone, delayed, reordered, repeated, or altered reads remain
non-receipted. This is compiler-evaluator sequencing evidence, not native
handle custody, Windows error-state custody, or an operating-system security
claim.

Observation summary v64 and replay-record v44 generalize the failure-only
Output sequence to exact absent tag-9 `remove` and tag-12 `remove_dir`
attempts. Each row binds the selected operation, canonical compiler-rooted
Output path, matching write authorization, scalar `-1`, and post-error `2`.
Mixed ordered file/directory sequences replay against a fresh namespace and
retain empty staged-output custody. This receipts those attempts only; it does
not claim that a host path is globally or durably absent.

The Windows `find_first`/`find_next`/`find_close` family remains non-receipted.
Its current plain-byte `directory/*` operand embeds the physical Source root;
exact retention is location-dependent, while ignoring it would weaken replay
input equality. The root-aware compiler-owned Build path facet must first
replace it with a Source-root-relative pattern coordinate.

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
A distinct version-7 evaluation sponsor now accounts deterministic evaluator
work, compiler-owned BuildLog bytes, filesystem operation attempts, and
concurrently live compiler-owned filesystem resources across that same
closure. The compiler policy grants 100,000,000 total fuel units, 16 MiB of
BuildLog output, 65,536 canonical filesystem operation attempts, 4,096 live
filesystem handles, 1,048,576 live semantic interpreter cells, 64 MiB of live
interpreter Text backing payload, 1,048,576 successful result cells, and 64 MiB
of successful result Text bytes,
preserves the 100,000-unit effect-free and 10,000,000-unit granted
per-invocation ceilings for initial evaluation and automatic replay, and
prevents dependencies or the ambient interpreter development override from
raising package-policy limits. Usage receipt v7 binds those limits and
separates initial from replay fuel, BuildLog, filesystem-attempt, result-cell,
and result-Text charges, each invocation's peak live-cell and live-Text-byte
counts, and the shared session's peak live-handle, live-cell, and live-Text-byte
counts. Owned handle outputs
reserve before provider entry; provider failure, successful close, and
evaluator teardown release through compiler ownership. Borrowed native views
do not consume a second resource slot. Successful closure review requires
exact cumulative-charge and peak reconciliation with the shared sponsor and
zero live reservations at completion. This is not a CPU-time, process-memory,
or process-wide descriptor-table claim. The live-Text account measures logical
backing payload, not `Vec` capacity, allocator overhead, or temporary non-Text
copies. A generic temporary-payload ceiling is
deliberately rejected; only named byte domains with complete compiler-owned
allocation lifetimes may acquire peak-live accounts.

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

Every package-owned bodyless external realization, including a private
implementation leaf, is consequently one separate blocking
executable-supply trust row. It binds the exact package-qualified callable and
tagged requirement application—trait conformance, operator overload
coordinate, or top-level boundary-requirement overload—to a closed compiler-
owned mechanism identity: import library and
symbol, syscall number, compiler intrinsic, vtable slot, vtable field, or table-
function field. The projector cross-checks machine supply mode, satisfies
binding, and the external-binding table. Missing, duplicate,
mismatched, or unsupported state rejects. The row is not callable API, reach,
boundary representation, an accepted proof, or Terminal evidence, and does not
claim that anyone audited the supplied code or verified its realization.

Projection reads each component from the earliest coherent compiler-owned
representation in which it is semantically settled. Private pre-Terminal binding
identity may join the checked callable/requirement association only after
successful compilation. Only the versioned canonical row crosses into package
orchestration; the checker may move with compiler internals. Psi may repeat the
invariant as a backstop but is not the mandatory reconstruction source. Do not
introduce nominal Chi for this seam. A new stage is warranted only by a real
shared semantic boundary with independent consumers, transformations, or
invariants; reuse a coherent existing stage such as Exact when it is simpler.

Review v70/canonical row v28 now implements the lane. An external leaf must be
bodyless and carry exactly one conformance application; supply mode,
conformance binding, mechanism tag, and structural binding-table identity must
agree. Malformed import/syscall/vtable/table payloads and table fields without
an exact attached data owner reject. The row key is callable plus complete
conformance application and its value is the structural binding, preserving
callable API bytes across a binding-only update while producing one
`OpaqueBlocking` supply conflict. Private leaves receive the trust row without
becoming public callable API. Recovery, source accounting, and conflict
rendering retain the row without making an audit or Terminal claim.

Review v72/canonical row v30 tags the exact requirement in that same row as
either a trait conformance or an existing package-qualified operator overload
coordinate. The first operator lane accepts bodyless external supply for a
public, named, nongeneric boundary operator. Public realization machines retain
the coordinate in callable API; private leaves remain absent from public API.
Selected-provider evidence remains separate and must rejoin the exact operator,
realization symbol, package, normalized machine identity, and binding, so an
opaque supply row never implies selection. Compiler-known intrinsics are the
first executable mechanism. Ordinary or private operators, aliases, generic/
lifetime applications, and fixed-token boundary operators reject.

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
Producer availability reports the exact package-qualified declaration and
ordinary public conformance/carrier surface without claiming acceptance of a
consumer selection. For every demanded runtime by-value occurrence, the
selecting consumer reports its exact boundary requirement application, named
representation conformance or compiler-owned target-semantics source, carrier,
target/version, closed shape graph or sealed ABI leaf, physical movement,
role-tagged lifecycle disposition, evidence origin, and strong conformance and
boundary-plan commitments. Foreign demand rejoins the producer's exact rows and selected
immutable source instance. Explicit `Unbound` is complete only when no active
runtime by-value crossing demands a shape. Initial introduction or material
change strongly recommends code/ABI audit but does not, by opacity alone,
create a blocking trust-claim conflict.

One compilation activation permits at most one selection per opaque
declaration, including unused selections. Only use creates demand evidence.
Selections made during an earlier package-as-root review are not inherited as
consumer policy, because dependency build machines are not rerun and their
generated-source bundles carry no active selection. Equality is required
within the active compilation and at each future independently compiled
by-value composition edge, not among unrelated historical review rows.
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
review findings may receive the review-only result `no-review-blocker`;
claim-free opacity alone may receive
`no-review-blocker-with-audit-recommended`. Neither result admits the package.
Accepted-claim, dangerous-authority, and external-executable-supply rows block
for exact root-policy resolution on initial admission or when a package is
newly introduced. Their conflict baseline is explicitly empty: it carries no
invented old resolution, source commitment, or review row. An unchanged
accepted baseline does not require blanket reapproval. Suspect authority,
trust, executable introduction, dangerous contract slack, or build-host reach
recommends audit; retained dangerous authority and external executable supply
remain audit-relevant even when unchanged. The exact capability, claim,
compatibility, or root-policy row determines whether admission also blocks.

The first review-only root-policy object requires one closed accept/reject
decision for every exact blocking fingerprint and binds the canonical decision
set to the complete candidate-closure commitment. That commitment covers the
source graph plus every candidate package's target, compiler,
source-consumption, build-observation, and whole-review evidence; each conflict
also binds its baseline and candidate package observations. Missing, duplicate,
stale/foreign, wrong-candidate, and non-blocking decisions reject. Accepting a
row is policy for that exact candidate delta, not proof that a human or model
performed an audit. The object reports only whether all blocking rows were
accepted; it cannot decide whether the wider transaction may proceed or issue
accepted evidence or lock state. The review-only object now has a bounded
canonical fixed-vocabulary text record: candidate closure, sorted conflict
fingerprint plus closed disposition rows, and the reconstructed resolution
commitment. Strict recovery maps every row back to the current compiler-derived
conflict and owning package, reruns the complete validator, and requires
byte-identical canonical re-encoding. At that layer this closed restart-stable
encoding, not policy-origin/file custody, governance evidence, accepted-lock
reference, or transaction authority.

Policy-directory file custody now wraps that record without changing its
meaning. Trusted command orchestration supplies an already-open root-owned
directory capability and one bounded lowercase portable canonical filename;
nested paths are unrepresentable and package dependencies are never searched
for policy. Every operation is a direct child of that handle. Case-alias,
symlink/non-regular-leaf, and existing-destination forms reject. Reads retain
the file through semantic recovery, then reread and compare its bytes and live
name identity. Persistence prepares and synchronizes a private same-directory
stage before atomic no-overwrite hard-link publication, then requires directory
synchronization. A later failure reports `published but unconfirmed`, because
the complete canonical file may remain recoverable. The library cannot prove
that an arbitrary caller supplied a root-owned command directory. This provides
filesystem
custody, not proof of review, governance, accepted-lock state, or permission for
the install/update transaction; no final UX directory or filename is fixed yet.
The reread and identity checks detect ordinary concurrent change, not a hostile
process already holding the root author's filesystem credentials and alternating
valid states between observations. Final command transaction locking and
immediate policy revalidation own that boundary.

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
`no-review-blocker-with-audit-recommended`; a package that also introduces accepted
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
now has a runner-neutral advisory boundary: fixed system instructions remain
separate from bounded rendered evidence; the package library chooses no model
and supplies no package-derived network capability. Any network or credential
authority already available to the runner is operator policy. The runner
streams response bytes into an Omega-owned sink enforcing the caller-supplied
output ceiling. Only the exact canonical result envelope
with one of two tokens—`recommend_audit` or `no_additional_audit`—is accepted,
without prose. The recommendation is monotone: it may add audit, but cannot
suppress compiler recommendations, alter blockers, prove an audit, resolve
conflicts, admit a package or evidence, set policy, or mutate state.
The outcome is bound to the exact rendered input by a domain-separated
commitment. Provider/configuration and CLI wiring remain. The implemented join
requires a
bijection between the complete candidate closure and compiler rows by exact key
and immutable resolution. Its shared validator also rejects duplicate reviews,
package/projection identity mismatch, mixed deployment targets, and
incompatible obligation-semantics, evidence-schema, review-encoding, or row-
encoding identities before either capability comparison or source rendering.
It validates every recovered baseline custody against its row and
derives unavailable-old-source state from absence. Initial and newly transitive
source packets follow compiler-recommended audit policy; changed or unavailable
existing update sources receive an exact diff or standalone candidate packet.
The aggregate byte ceiling retains
separate compiler-only and hostile-source frames. No output can construct
accepted lock evidence or attest that review happened.

Review can resume after process restart without refetching the old source. A
versioned, bounded binary review-baseline capsule retains the complete resolved
`PackageKey` graph and immutable resolutions, comparison commitments, every
canonical comparison row plus its source-explanation sidecar, and any verified
bounded source-read replay record. The compiler owns
strict row-envelope recovery; orchestration treats row values as opaque, and a
recovered row remains distinctly review-only rather than becoming newly
compiler-issued evidence. The capsule checksum detects accidental corruption,
not authenticity or proof of review, while canonical decode rechecks graph
closure, row/package/target identity, ordering, singleton rows, replay semantic
schemas and operation-specific lanes, parent-observation association, and all
resource ceilings including aggregate replay bytes. The association is also a
consistency check rather than authenticity. It is deliberately a non-admitting checkpoint: no API converts it to
`PackageInstance`, a conflict resolution, project mutation, or accepted lock.
The future lock may contain the same normalized material only after a consumer
reconstructs the exact source-and-artifact obligations, checks the retained
certificates, propagates every dependency's open obligations, and records its
own admission decisions. Producer provenance cannot promote the checkpoint.

Useful result states include:

```text
no-review-blocker
no-review-blocker-with-audit-recommended
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
observations, and local decisions remain separately bound. No review schema,
including the current one, can be promoted merely because the future artifact
reuses its row vocabulary.

That local reconstruction may read the earliest coherent compiler-owned IR in
which an obligation is semantically complete, including private pre-Psi or
pre-Terminal state. The checker is part of the compiler and may move with those
internals; only its versioned canonical obligation ledger and exact replay
subjects cross the persistence boundary. There is no nominal Chi stage merely
to stabilize this seam. Add one only if implementation discovers a genuine
reusable semantic boundary, and prefer an existing coherent stage such as Exact
when it can carry the same meaning with less machinery.

The current ordinary reconstruction ledger binds the exact source-path-free
dependency closure consumed by package-aware compilation alongside package,
target, and canonical rows. It is projected only from validated compiler inputs
and retains every reachable package identity and requester-local alias edge,
but no separately copied package display name, source root, immutable
resolution, or source byte. Each opaque package identity still binds its
declared name and source lineage. Recovered row envelopes must be joined to that
separately reconstructed closure. Renaming an unused alias or adding/removing an
unused reachable package invalidates ledger equality; relocating the same graph
does not. The ledger's obligation-semantics schema is explicit and independent
from its outer codec and review-row versions. A bounded canonical whole-ledger
frame carries the schema, package, target, complete package/alias closure, and
exact rows. Decode rejects unsupported vocabularies, malformed or noncanonical
graphs and row framing, resource-limit violations, and trailing state. Row
payload meaning remains opaque until exact local reconstruction. A domain-
separated fingerprint names this complete framed replay question, and compiler-
issued closure review retains the locally reconstructed ledger under one 64 MiB
aggregate session ceiling. Neither decode nor a matching fingerprint
establishes a discharge result. This closes a schema-bound subject coordinate
in the current replay gate, not transitive certificate/open-obligation
composition or lock authority.

Resolved-source custody separately retains the exact validated root request and
exposes a zero-copy request-set view that joins the root plus every requester-
owned dependency row, by authored ordinal, to the selected package key and
immutable resolution. Distinct selectors converging on one package are
therefore not collapsed into a fabricated primary request. Aliases remain
requester-local edge names; transport observations remain provenance rather
than package identity. Git adapters now follow this path for both repository
roots and declared named workspace members. They preserve acquisition and
package selection independently from the resolved commit/tree/content tuple,
share exact acquisitions within one traversal, and confine member-relative Path
rows to the verified root's declared members. This is resolver custody only—not lock
encoding, compiler evidence, admission, or `PackageInstance` construction—and
the ordinary obligation ledger intentionally remains source-selector-free.
Git dependency requests now normalize omitted selection to `Root` and retain
explicit `Named(PackageName)` selection in this custody.

The versioned `CanonicalSourceClosureSubject` is the bounded canonical form of
that source-selection question. It retains the exact root request and every
requester/ordinal dependency occurrence, resolved alias, selected package key,
immutable resolution, content identity, and one stable root/member navigation
value per package. Version 3 binds both root and dependency package selectors;
cache and snapshot paths remain excluded. Recovery reconstructs one strictly
ordered closed graph and rejects malformed, mismatched, noncanonical, or over-
limit state. Its fingerprint names the question only: use requires independent
resolution and snapshotting followed by complete reconstruction and exact
equality. Snapshot/cache paths, raw source bytes, transport execution
observations, compiler-consumption/build observations, artifacts, certificates,
admissions, and open obligations remain separately bound. This is neither an
accepted lock nor a package instance. Package selection is an explicit request
coordinate and does not change repository source identity.

`CanonicalPackageReconstructionQuestion` is the first canonical association of
that source-selection question with the current ordinary obligation questions.
It retains the complete source-subject bytes and, in strict full-`PackageKey`
source order, one complete canonical ledger frame for every package. Each
ledger root, target, transitive package set, and requester-local alias edge must
match the closure independently derived for that package; missing, foreign,
swapped, colliding-identity, mixed-target, or graph-drifted associations reject.
Fresh matching reconstructs the aggregate from current resolver custody and a
new compiler-issued review set. Decode and the aggregate's domain-separated
fingerprint remain inert: the type contains no compiler pedigree, build
observation, artifact, certificate, result, open obligation, admission,
accepted-lock state, or `PackageInstance` promotion route.

The first ordinary result lane is deliberately smaller than accepted evidence.
For every bodyless accepted claim, local reconstruction rejoins the typed
callable—including its exact signature and contracts—to the matching canonical
obligation row and assigns only `OpenRootAdmission`. The manager then composes
those open claims over the exact reconstruction question; a dependency claim
reaches the selected root with its original package owner. No certificate or
producer decision is representable, and the result has no codec, lock-promotion
route, or `PackageInstance` constructor.

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

Compiler-owned native builtins are not package provider executions. The first
complete lane rejoins a demanded Linux `exit_group(i32)` proposal to one exact
selected intrinsic row, one Terminal boundary, and the consuming lowerer's
local ELF catalog. Physical custody retains the closed role
`CompilerBuiltin(LinuxExitGroupI32)` through image and installation framing
while provider-execution reports remain empty. Installed and foreign
implementations continue through their separate admitted-provider role. The
planner-to-lowerer catalog conversion is one exhaustive `match` returning an
optional `CompilerBuiltinExecution`, and the admitted D41 settlement remains
complete replay input for its D32 physical child rather than collapsing to a
commitment.

Dependency evidence composes transitively. Each subject retains its own
obligation-semantics identity. Checked obligations compose upward. Missing or
unproved obligations also compose upward as open rows, never as a producer's
already-accepted decision; each consuming project applies its own admission
policy. Accepted locks and evidence are exact-current generated artifacts. A
semantic-schema mismatch forces complete local reconstruction and fresh
admission; no old discharge or policy decision is reused. Unsupported lock
versions reject and regenerate rather than entering a compatibility or
migration classifier. Historical bytes may be retained for their matching old
toolchain or separate audit tooling, but current admission never grandfathers
them. Unavailable old source continues through standalone review and audit
recommendation rather than migration.

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
outer trust boundary. Package review is regenerated locally so a dependency
cannot declare its own capability result; package acceptance reconstructs the
question and checks the certificates. In the current pipeline the package
manager drives that compiler review inside the same `omega` process, so this is
an internal canonical and semantic consistency check, not isolation from the
compiler executable. Obligation-semantics, evidence-schema, review-encoding,
row-encoding, source, artifact, and target identities define or scope the
check. D46 excludes the bytes readable through `current_exe()` from review and
cache identity. Exact compiler artifact custody remains separate when that
artifact is itself a bootstrap, reproduction, or deployment subject.

Omega's responsibility is to produce deterministic, bounded review facts,
recommend an audit for dangerous retained authority, stop on unresolved policy
conflicts, and expose hooks for project policy. A project that needs stronger
assurance must enforce its chosen process around Omega: protected branches,
required reviewers or signatures, isolated builds, independently bootstrapped
toolchains, reproducibility checks, or other controls appropriate to its threat
model. The committed and merged decision authorizes the update; Omega does not
manufacture a portable “proof of audit.”

## Implementation trust status

The `omega-package-manager` release surface now contains reviewed corrected-model
building blocks for immutable source custody, typed identity and closure,
compiler handoff/review, exact row conflicts, and review-only triage. Its final
admission model is not yet accepted. The legacy manifest, name-keyed lock,
whole-section receipt, caller-constructed instance, and install/update
scaffolding and standalone dependency scanner were deleted rather than retained
as compatibility paths. Standalone compilation now resolves only ordinary
root-relative and toolchain imports; package aliases exist only in the
validated requester-local compiler handoff. Production code must not
reintroduce or depend on any path that:

- key locks or symbols by package-authored name alone;
- ask the installer for both alias and package name;
- accept caller-constructed package capability manifests;
- accept standalone manifest JSON as compiler evidence;
- treat a free-form reviewer/reason receipt as conflict resolution;
- store only a capability fingerprint without the accepted baseline; or
- syntactically scan dependency calls while silently skipping malformed
  dependency builds.

The corrected recheckable evidence, accepted-lock, and transaction paths must
exist before `omega install` or `omega update` can mutate project state.

## Test packages

The existing fixtures now declare identity through
`builder.package("canonical-name")`, use coherent build parameters, and
regenerate currently representable compiler review evidence from resolver
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
