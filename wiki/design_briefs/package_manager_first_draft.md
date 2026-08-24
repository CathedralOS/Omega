# Design Brief: Package Manager First Draft

Status: corrected first design, 2026-08-23. This brief is temporary until the
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

## Package declaration and identity

Every package declares its own human name in its `build.omg` through one
well-known, hermetically evaluated constant:

```omega
const PACKAGE: Package = Package {
    name: "arithmetic-kernels"
};

machine build(build: &mut Build) {
}
```

This uses ordinary `const` and data syntax. `Package` is toolchain-provided
build vocabulary, not a new grammar form. Omega extracts the declaration before
executing `build`, resolving imports, or supplying build-host services. The
declaration must be unique, compile-time evaluable, effect-free, independent of
dependencies and generated files, and use canonical kebab-case spelling.

Three identities remain deliberately separate:

- `PackageName` is the package-authored human name, such as
  `arithmetic-kernels`.
- `PackageKey` joins that name to canonical source lineage. It is the stable
  graph, lock, and nominal-symbol identity across updates.
- `PackageInstance` joins the key to exact source content, compiler/toolchain
  identity, and the compiler-derived package-evidence fingerprint.

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

Canonical symbol and boundary identities include `PackageKey`. A package that
declares a lookalike trait or package name therefore cannot impersonate an
admitted boundary owned by another source lineage.

## Authored dependency requests

`build.omg` names sources, not externally asserted package identities. The
target ordinary-library shape is:

```omega
machine build(build: &mut Build) {
    build.depend(Source::Git {
        repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
        revision: "main"
    });
}
```

The resolver fetches the source and obtains its name from that package's own
`PACKAGE`. The default in-code alias is the mechanical kebab-to-snake mapping,
`arithmetic-kernels` to `arithmetic_kernels`. Only a genuine local collision
or deliberate rename uses the exceptional `build.depend_as(alias, source)`
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
`PackageInstance`, dependency closure, compiler/toolchain identity, normalized
capability baseline, build observations, trust evidence, and review-resolution
references.

The compiler always builds from the lock and never silently re-resolves a
mutable selector. `omega.lock` is generated but should normally be committed;
source caches and expanded artifacts may be ignored. A fingerprint alone is
not an admission baseline: the lock must embed the normalized accepted security
projection or retain a mandatory content-addressed copy.

The first resolver does not solve semantic-version ranges. Requests for the
same `PackageKey` must reconcile to one immutable instance or fail with every
conflicting dependency path. Multiple-version composition is a later explicit
feature.

## Compiler-derived package evidence

Package capabilities are derived from the candidate repository after the
complete source/build closure is available. The package author writes ordinary
Omega contracts; it cannot author or patch the admission manifest.

The normalized package evidence includes, with exact provenance:

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

Underdeclared effective reach is a compiler error. Overdeclared reach remains a
visible contract-slack row. Dangerous slack is suspicious because it reserves
authority that a later implementation may begin exercising without changing
the public ceiling. The manifest pins both declared and realized reach, so
unused-to-used authority still changes evidence.

Open or deferred proofs reject package admission. Checked proofs are rechecked
by the proof kernel. Accepted axioms and opaque boundary claims remain explicit
trust-bearing evidence and require admission; authored postconditions are
obligations, never proof. Boundary realization uses exact package-qualified
nominal identities and cannot be satisfied by a same-spelled trait.

Risk classification is compiler-owned metadata attached to exact admitted
boundary/capability identities. It is never inferred from package-controlled
strings such as `Filesystem` or `Network`.

## Update, install, and missing baselines

An install compares the new dependency closure against an empty admission
baseline. A completely checked package with no suspect authority may pass
automatically; suspect authority, trust, executable introduction, dangerous
contract slack, or build-host reach creates a blocking conflict.

An update derives candidate evidence and compares it with the normalized
accepted baseline in `omega.lock`:

- an evidence change creates a blocking capability/API conflict;
- unchanged evidence permits resolution to continue;
- retained intrinsically dangerous authority always emits an audit
  recommendation; and
- source-lineage or declared-name change is package replacement.

Every source update also receives automated/LLM provenance and source-diff
triage. Equal capabilities do not imply safe behavior: code with existing
filesystem and network authority can become malicious without changing its
authority set.

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
another update or generated by the dependency itself.

LLM triage receives only Omega-rendered, bounded, escaped identifiers and
evidence rows. Package prose, comments, commit messages, and README text do not
enter the triage prompt. A later code audit necessarily reads attacker-
controlled source and is treated as a separate hostile-input activity.

Useful result states include:

```text
admitted
admitted-after-audit
admitted-with-audit-recommended
blocked-capability-change
blocked-missing-admission-baseline
blocked-provenance-change
```

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
