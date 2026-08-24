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
- `PackageInstance` joins the key to exact source content, evidence-schema
  identity, compiler/toolchain provenance, and the compiler-derived
  package-evidence fingerprint.

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

## Authored requests versus accepted lock state

`build.omg` records update intent: source locator, revision selector, explicit
alias override, targets, roots, providers, and build orchestration. `omega.lock`
records the accepted resolution: exact commits/trees/content, `PackageKey` and
`PackageInstance`, dependency closure, evidence-schema identity,
compiler/toolchain provenance, normalized capability baseline, build
observations, trust evidence, and policy-resolution references. Compiler and
toolchain identifiers make evidence reproducible and comparable; they do not
authorize its truth or prove that anyone audited it.

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

Ordinary admission derives from the compiler's coherent checked semantic graph
through a total internal `PackageAdmissionProjection`. The projection is not a
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

Implementation should consume the earliest coherent checked compiler state
that already contains each required fact. Different evidence rows may therefore
come from different internal representations; totality belongs to the final
projection, not to one frozen source stage. Because the projection ships with
the compiler, depending on compiler-internal representations is ordinary
internal coupling, not a promise that those representations are stable public
APIs. These representations remain part of the Psi-owned semantic pipeline:
"earlier" means earlier than Terminal Psi, not a new owner or a pre-Psi
semantic path. The projection and its tests move with those representations.

There is no nominal Chi stage merely to stabilize this report. A distinct IR is
justified later only if multiple independent consumers need the same semantic
stage or it acquires its own transformations, invariants, and verification
rules.

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
explicit toolchain marker, or an unresolved marker; generic binders remain
owner-free and alpha-normalized. Public signature identity separately layers
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
commitment in review evidence; this is narrower than the whole-toolchain
commitment required for admission. Compiler carry aliases
expand to explicit toolchain-unbound atoms until exact toolchain commitment
lands. Predicate-body presence and the currently representable structural
expression/membership facts retain the domain carrier, package-qualified
members/domains, and canonical fact ordering. Each fact joins its exact typed
handle to one checked definition row and one checked ownership record; nested
members additionally require exact fact-keyed dependency places. Missing,
duplicate, wrong-origin, private-domain, and member-spoofed evidence rejects.
Callable/proposition-shaped applications, semantic roles, and domain operators
reject until their authority and exact rows are settled; none is inferred from
the domain name. Compiler-owned
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
reselecting names. Public trait requirements retain unnamed `requires` and
`ensures` through the same closed structural fact/expression vocabulary as
public callables, joined to their exact checked state-signature owner. Their
abstract published crash ceilings are projected from exactly one checked
trait/requirement capsule into canonical cause-and-guard routes; no realized
body sites or calls are fabricated. Generic selected-conformance telescopes,
public-trait invariants, named evidence contracts, boundary clauses, and
unsupported expression forms reject until complete canonical rows land.
Requirements also retain whether their checked declaration supplies a default
realization; the implementation body remains source subject to universal update
triage, while its checked operational behavior must fit the requirement
envelope and any instantiated use contributes ordinary compiler-derived
evidence.

Package-owned boundary and ordinary public machines, plus the selected build
machine, retain the exact canonical entry signature alongside their authority
rows. Their checked-body, boundary, and accepted supply tiers remain distinct:
a bodyless boundary guarantee is an explicit trust-bearing accepted claim,
while a claim-free boundary symbol asserts nothing. This includes lifetime
arity, alpha-normalized type/const parameters,
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

Public callable `requires`, `ensures`, and boundary clauses now retain exact
structural rows for the closed boolean/integer expression subset over parameter
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
join, and unsupported call and aggregate expressions still reject rather than
falling back to text or a hash. Contract casts retain their structural operand,
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
- declared and realized transitive service reach for every public callable;
- declared and realized build-machine reach and build observations;
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
platform alternatives remain distinct transcript identities. Rooted evidence
must account for potentially absolute `read_link` output and necessarily
absolute `canonicalize`/`final_path_name_by_handle` output. These observations
stay separate from capability/API comparison bytes. Observation schema v2
retains each completed operation's exact provider, stable tag, scalar result,
and post-error in successful-run call-start order. Denial-shaped returns remain
visible but are not yet typed as grant-policy refusal versus host error. It
omits failed evaluator attempts, arguments, rooted paths, mutable byte regions,
logical handles, and content, so it remains an incomplete trace and makes no receipt,
replayability, or source-rebuildability claim. Canonical operation transcripts,
recorded inputs, staged-output commitments, and replay checking remain required
before any `Receipted` verdict.

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
visible contract-slack row. Dangerous slack is suspicious because it reserves
authority that a later implementation may begin exercising without changing
the public ceiling. The manifest pins both declared and realized reach, so
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
as `admitted-with-audit-recommended`. Suspect authority, trust, executable
introduction, dangerous contract slack, or build-host reach creates a blocking
conflict.

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

## Audit authority and compiler provenance

Omega cannot prove that a human or LLM performed a serious audit. A signed
resolution proves only that a key signed bytes; a recorded reviewer or reason
proves only that strings were recorded. A proof certificate can establish its
explicit mechanically checked proposition, but not that the surrounding source
was understood, that an LLM resisted manipulation, or that an upgrade is safe.

The selected local compiler and the people and infrastructure allowed to land
accepted project state are therefore trust roots. Package evidence is always
regenerated locally so a dependency cannot declare its own capability result.
Evidence-schema, compiler, source, verifier, and target identities remain in
the lock for comparison, replay, cache correctness, and reproducibility—not as
proof that the producer or review process was honest. A compiler change may
require regeneration or an explicit schema migration, but hashing the compiler
does not confer authority on it.

Omega's responsibility is to produce deterministic, bounded review facts,
recommend an audit for dangerous retained authority, stop on unresolved policy
conflicts, and expose hooks for project policy. A project that needs stronger
assurance must enforce its chosen process around Omega: protected branches,
required reviewers or signatures, isolated builds, independently bootstrapped
toolchains, reproducibility checks, or other controls appropriate to its threat
model. The committed and merged decision authorizes the update; Omega does not
manufacture a portable “proof of audit.”

## Implementation trust status

The current `omega-packages` Rust crate is exploratory scaffolding. Its source
fetching, hashing, normalization, and graph routines may be reusable after
review, but its trust model is not accepted. In particular, production code
must not:

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

The existing fixture package purposes remain useful, but every fixture must
gain an explicit `PACKAGE` declaration and compiler-derived evidence. Tests
must stop fabricating package manifests from fixture intent. Remote fixtures
must exercise transport-normalized lineage, immutable commit/tree identity,
missing-old-source review, missing-lock fresh admission, retained dangerous
authority triage, and same-name/different-lineage spoof rejection.
