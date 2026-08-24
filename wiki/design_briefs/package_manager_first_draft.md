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
source-rebuildable release profile. Archive and future protocol adapters must
define their own canonical lineage and immutable-content receipt; an unknown
URL is never guessed to be Git or delegated to an ambient protocol helper.

The first implementation deliberately normalizes only GitHub's established
HTTPS and SSH repository namespace. Other Git hosts retain transport, user,
port, path case, and suffix distinctions until a host adapter can prove more;
conservative duplication is preferable to false package identity.

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

Implementation should consume the earliest coherent checked compiler state
that already contains each required fact. Because the projection ships with the
compiler, depending on compiler-internal representations is ordinary internal
coupling, not a promise that those representations are stable public APIs. The
projection and its tests move with those representations.

There is no nominal Chi stage merely to stabilize this report. A distinct IR is
justified later only if multiple independent consumers need the same semantic
stage or it acquires its own transformations, invariants, and verification
rules.

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
