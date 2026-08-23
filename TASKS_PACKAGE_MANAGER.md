# Tasks: Package Manager

Status: first planning slice, 2026-08-23.

This file tracks the Cargo-like package manager for the `omega` command. The
package manager is a resolver, fetcher, auditor, and `build.omg` editing tool;
it is not a hosted registry and not a second dependency language.

Design owner:

- `wiki/design_briefs/build_and_package_model.md`
- `wiki/language_guide/chapter_15_modules_imports_visibility.md`
- `wiki/language_guide/chapter_19_capabilities_effects_boundaries.md`
- `wiki/design_briefs/package_manager_first_draft.md`
- `bootstrap/onramps/omega-rust/omega/orchestration/omega-packages/README.md`

## Model

- Packages are directories with `build.omg`.
- `build.omg` owns dependency aliases, pins, root/provider selections, and build
  authority.
- Source retrieval uses resolver-owned authority. Downloaded package code does
  not inherit resolver network, archive, filesystem, process, signing, or
  acceptance authority.
- The lock artifact records the resolved source closure, package capability
  manifests, trust/admission receipts, and reproducibility evidence.
- Updates are fail-closed when a dependency's normalized capability manifest
  changes. A separate explicit acceptance flow records the review.

## P0 - Package Evidence Schema

Completed:

- **BUILD-OMG-SCOPED-FS.** Switch `build.omg` execution from the current
  transitional real-unscoped filesystem grant to scoped real filesystem grants:
  read roots are the package source tree and declared read-only inputs; write
  roots are the package build/staging directories only.

  Done 2026-08-23: root `build.omg` evaluation now receives a
  checked-interpreter `RealScoped` grant: source tree read root, build directory
  write root. Focused coverage verifies a write under `build/` succeeds and a
  write under the source root is denied without creating the file. Dependency
  `build.omg` execution is not implemented yet; package-to-root isolation
  remains tracked by `NO-AMBIENT-DEPENDENCY-EXECUTION`.

- **PACKAGE-MANIFEST-MODEL.** Add the first `omega-packages` crate slice with
  canonical package/alias naming, a normalized package capability manifest
  data model, deterministic JSON rendering, SHA-256 manifest fingerprints, and
  severity-ranked manifest diffs.

  Done 2026-08-23: package identities validate as kebab-case, in-code aliases
  validate as snake_case, equal evidence renders byte-identical manifest JSON,
  and service-reach changes produce high-severity manifest diffs.

- **LOCAL-SOURCE-IDENTITY.** Add the first resolver-owned local path identity
  pass before network fetch support.

  Done 2026-08-23: `omega-packages` can resolve a local source directory to a
  deterministic SHA-256 content identity, skipping `.git`, enforcing file,
  byte, and depth limits, and rejecting symlink escapes outside the package
  root.

- **GIT-SOURCE-IDENTITY.** Add resolver-owned Git transport into an isolated
  cache checkout, resolving mutable user input to exact commit/tree identity.

  Done 2026-08-23: `omega-packages` can clone/fetch a Git source, resolve the
  requested revision through `FETCH_HEAD` to an exact commit and tree, detach
  the cached checkout at that commit, reject `.gitmodules` until submodules are
  explicit package edges, and reuse local content hashing for the checkout.

- **PACKAGE-LOCK-MODEL.** Add the machine-written package closure lock data
  model before CLI integration.

  Done 2026-08-23: `omega-packages` has a deterministic lock model keyed by
  canonical package names. Lock entries retain source kind/locator/identity,
  package manifest fingerprints, build observation class, dependency aliases,
  trust receipts, stable JSON, and a SHA-256 lock fingerprint.

- **CAPABILITY-REVIEW-RECEIPT-MODEL.** Add explicit acceptance receipts for
  manifest-changing updates before CLI integration.

  Done 2026-08-23: `omega-packages` can create deterministic review receipts
  from non-empty manifest diffs, requiring reviewer and reason text, binding
  the exact old/new source identities, old/new manifest fingerprints, accepted
  delta fingerprints, severity, stable JSON, and receipt fingerprint.

- **PACKAGE-SOURCE-AUDIT-COMMAND-API.** Add the first internal command API that
  the CLI can later wrap without duplicating resolver policy.

  Done 2026-08-23: `omega-packages` exposes `audit_package_source` for local
  and Git requests. It reports transport kind, locator, requested rev, resolved
  commit/tree when applicable, deterministic content identity, file count, byte
  count, and a concise text summary.

- **AUTHOR-GUIDANCE-DIFF-MODEL.** Add concrete author/reviewer guidance to
  package manifest diffs before CLI wiring.

  Done 2026-08-23: manifest deltas now carry audit guidance for newly exported
  public services, new build-host services, provider requirement additions, and
  increased capability-flow verbs. The text directs reviewers to public
  boundary declarations, `build.omg`, provider origin/plan evidence, optional
  package splitting, and authority storage/return/acquire/derive paths.

- **SOURCE-CACHE-POLICY-RECORDS.** Add resolver-owned policy evidence for
  local and Git source resolution before install/lock wiring.

  Done 2026-08-23: `omega-packages` emits deterministic source-cache policy
  records for accepted and rejected local/Git requests. Records include source
  kind, locator, requested rev, resolved commit/tree when present, content
  identity, cache/root path, file/byte counts, traversal limits, submodule
  policy, path-containment policy, rejection reason, stable JSON, and a
  SHA-256 record fingerprint.

- **PACKAGE-LOCK-CLOSURE-VALIDATION.** Add fail-closed validation for assembled
  package lock closures before compiler/CLI lock wiring.

  Done 2026-08-23: `omega-packages` validates that a package lock contains its
  root package, has no duplicate package entries, has no duplicate dependency
  aliases within a package, has no dependency edges to packages absent from the
  closure, and retains non-empty source identities and manifest fingerprints.

- **PACKAGE-LOCK-PERSISTENCE.** Add strict lock artifact read/write support
  before install/update mutates `omega.lock`.

  Done 2026-08-23: `omega-packages` can write normalized locks through a
  same-directory temporary file and atomic rename, parse the machine-written
  JSON schema with fail-closed errors for duplicate/unknown/missing/invalid
  fields and unsupported schema versions, normalize locks after read, and
  reject invalid closures before writing or accepting a lock from disk.

- **DEFAULT-UPDATE-ADMISSION.** Add the default update decision model before
  CLI mutation.

  Done 2026-08-23: `omega-packages` can compare old/new normalized package
  capability manifests, admit source-only updates, reject package identity
  changes, and reject non-source manifest deltas with severity-ranked diff text
  and explicit review guidance.

- **REVIEWED-UPDATE-ADMISSION.** Add exact receipt matching for explicit
  capability-changing updates before CLI mutation.

  Done 2026-08-23: `omega-packages` can re-evaluate a rejected update with a
  capability-change receipt, admit only receipts bound to the exact old/new
  source identities, old/new manifest fingerprints, and accepted delta
  fingerprints, and reject mismatched receipts with guidance to regenerate the
  receipt for the actual candidate diff.

- **LOCAL-PACKAGE-FIXTURE-CORPUS.** Add the first local package corpus for
  resolver, install, update, and audit tests.

  Done 2026-08-23: `fixtures/packages/` contains normal kebab-case package
  directories for `arithmetic-kernels`, `generated-table`, `file-journal`,
  `network-overreach`, `axiom-ledger`, `provider-switchboard`,
  `capability-vault`, and `graph-workbench`. Each fixture has `build.omg`,
  `main.omg`, package notes, and focused intent. `omega-packages` resolves all
  fixture directories as distinct local source identities in tests.

Remaining:

- **PACKAGE-CAPABILITY-MANIFEST.** Define the normalized manifest produced for
  each resolved package. It should include source identity, public API contract
  identity, exported service reach, build-machine reach and observation class,
  dependency aliases, provider requirements/selections, routed qualifications,
  capability-flow counts and source rows, unresolved installation rows, and
  trust/admission receipts.

  Acceptance: compiling one package can emit a deterministic machine-readable
  package capability manifest and a concise human diff. Equal source/evidence
  emits byte-identical manifest content.

- **PACKAGE-LOCK-CLOSURE.** Extend the existing `omega.lock` direction from
  trust receipts only to the full package closure without making it a second
  hand-authored manifest.

  Remaining after `PACKAGE-LOCK-MODEL`,
  `PACKAGE-LOCK-CLOSURE-VALIDATION`, and `PACKAGE-LOCK-PERSISTENCE`: wire
  compiler/package admission output into the existing `omega.lock` artifact.

  Acceptance: the lock records exact repository revisions/content identities,
  package manifest fingerprints, dependency edges, build observation verdicts,
  and trust receipts for the entire resolved graph.

## P1 - Resolver And Fetch Boundary

Remaining:

- **SOURCE-RESOLVER.** Add resolver-owned fetch support for explicit Git URLs
  and local paths first. Treat protocol and hosting provider as transport:
  GitHub, GitLab, SSH, HTTPS, and file paths all resolve to exact content
  identity before package code is loaded.

  Remaining after `LOCAL-SOURCE-IDENTITY`, `GIT-SOURCE-IDENTITY`, and
  `SOURCE-CACHE-POLICY-RECORDS`: add install-command integration and lock
  wiring.

  Acceptance: `omega install alias <source>` resolves a candidate to an exact
  commit/tree or local content identity and stores it in an isolated source
  cache with path traversal, symlink escape, submodule, and archive expansion
  limits checked before compile.

- **NO-AMBIENT-DEPENDENCY-EXECUTION.** Ensure dependency retrieval precedes
  dependency build execution and that each dependency build receives only its
  own explicitly admitted package-scoped build providers.

  Acceptance: a dependency cannot use the root build's network/filesystem
  authority, cannot name undeclared aliases, and cannot make an unpinned
  response into a dependency identity.

## P2 - CLI

Remaining:

- **OMEGA-INSTALL.** Add `omega install <alias> <source> [--rev <rev>]` as a
  guided edit to the nearest package `build.omg`. The command fetches the
  source, derives the package capability manifest, previews the graph/capability
  diff, and writes the dependency binding plus lock entry only after the
  candidate passes policy.

  Acceptance: adding a dependency produces a pinned alias in `build.omg`, an
  updated lock entry, and an audit summary that names new reachable services,
  provider/trust receipts, and capability-flow verbs.

- **OMEGA-UPDATE.** Add `omega update [alias...] [--to <rev>]`. The default
  update path may move source pins only when the dependency's normalized
  package capability manifest is unchanged.

  Remaining after `DEFAULT-UPDATE-ADMISSION`: wire candidate resolution,
  package-admission manifest derivation, `build.omg` pin editing, and lock
  persistence around the update decision.

  Acceptance: changing source bytes with the same capability manifest updates
  the pin and lock. Any manifest delta rejects before changing `build.omg` or
  the lock and prints a severity-ranked diff.

- **OMEGA-AUDIT-PACKAGES.** Add a read-only audit command for the package graph.

  Remaining after `PACKAGE-SOURCE-AUDIT-COMMAND-API`: derive package manifests
  for a resolved graph and expose the graph audit through the `omega` CLI.

  Acceptance: `omega audit packages` prints the resolved graph, source pins,
  service reach, build observation classes, provider origins, trust receipts,
  capability-flow verbs, and first failed provenance edge when policy rejects.

## Fixture Packages

Add a small package corpus before implementing the full resolver. The first
resolver can point at local directories and later reuse the same packages
through Git remotes. Use normal package names; the fact that they are fixtures
belongs in their repository description and test harness path, not in the
package name. Standardize package and repository names on hyphen-separated
lowercase words. Local `build.omg` aliases may use underscore-separated Omega
identifiers if the language surface requires identifier spelling.

Naming convention: external package identity names, repository names, and lock
package keys are kebab-case (`generated-table`). In-code names are snake_case
where the host language requires identifiers (`generated_table` in Omega or
Rust). Do not allow both spellings to name distinct packages in the same
package namespace.

Local package fixtures live under `fixtures/packages/`. The remote mirror task
remains open until these contents are pushed to exact-pinned repositories under
the GitHub `CathedralOS` organization.

- **arithmetic-kernels.** Library with checked proof/helper machines, no `build.omg`
  host reach, no boundary claims.

  Proves the empty-capability baseline and unchanged-update path.

- **generated-table.** Library whose `build.omg` reads a package-local input
  file and writes generated Omega source under its build directory.

  Proves scoped build-time reads/writes, receipts, and explicit handoff from
  build output to normal source checking.

- **file-journal.** Library exposing a public API that reaches `FilesystemHost`
  through a normal boundary/service path.

  Proves public service-reach manifests and update rejection when a previously
  pure package gains filesystem reach.

- **network-overreach.** Library that declares a wider public reach
  ceiling than its body currently uses.

  Proves the "declared but unused capability" warning or strict-profile
  rejection without treating the package as semantically unsound.

- **axiom-ledger.** Library with a bodyless accepted proof/boundary claim.

  Proves imported accepted claims are inert until the root accepts the package
  claim set, and that open deferrals are fatal at package-release/admission.

- **provider-switchboard.** Library with two provider candidates for one boundary
  requirement and a `build.omg` provider selection.

  Proves provider-selection identity, selected-plan grants, and update rejection
  when provider origin or selected-plan evidence changes.

- **capability-vault.** Library that accepts, stores, derives, returns, and
  acquires capability-bearing values through small separate machines.

  Proves capability-flow manifests and severity-ranked diffs for authority
  retention or propagation changes.

- **graph-workbench.** Root test package depending on two local packages,
  one harmless and one capability-bearing.

  Proves graph reporting names the dependency path that introduced a capability
  and that package policy admits the final transitive set rather than approving
  each edge independently.

Mirror these under the GitHub `CathedralOS` organization once local package
resolution works. The first remote tests should use exact commit pins from
these repositories, not branch names:

- `CathedralOS/arithmetic-kernels`
- `CathedralOS/generated-table`
- `CathedralOS/file-journal`
- `CathedralOS/network-overreach`
- `CathedralOS/axiom-ledger`
- `CathedralOS/provider-switchboard`
- `CathedralOS/capability-vault`
- `CathedralOS/graph-workbench`

## P3 - Review And Acceptance UX

Remaining:

- **CAPABILITY-CHANGE-REVIEW.** Add an explicit acceptance path for package
  capability changes. Default `omega update` must reject any manifest change;
  a deliberate command records reviewer identity, old/new fingerprints, diff,
  source revision pair, and acceptance reason.

  Remaining after `CAPABILITY-REVIEW-RECEIPT-MODEL` and
  `REVIEWED-UPDATE-ADMISSION`: wire receipt creation/loading into
  `omega update` and persist accepted receipts in or beside `omega.lock`.

  Acceptance: higher-capability updates require an acceptance receipt. New
  root-memory, DMA/IOMMU, executable-installation, interrupt-publication,
  dynamic-loader, process, filesystem, network, signing, or secret reach is
  elevated in the diff with the dependency path that introduced it.

- **AUTHOR-GUIDANCE.** Add diagnostics that advise package authors to keep
  unrelated capabilities in separate packages and publish reach ceilings on
  public APIs.

  Remaining after `AUTHOR-GUIDANCE-DIFF-MODEL`: surface this guidance through
  package-admission, `omega install`, `omega update`, and `omega audit
  packages` once those flows derive package manifests.

  Acceptance: a package that adds a public service row, build-host service,
  provider requirement, or authority-flow verb gets a concrete audit message
  explaining why dependents must review it.

## Deferred

- Version solving. Omega uses exact pins and explicit updates.
- A hosted package index. URLs and content identity are enough for the first
  implementation.
- General archive formats beyond the resolver boundary needed for Git/local
  path support.
- Workspace inheritance and shared ceiling ergonomics.
- Final `Build` API spelling for dependency operations.
