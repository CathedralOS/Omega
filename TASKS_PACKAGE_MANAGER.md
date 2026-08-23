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

- **PACKAGE-MANIFEST-PERSISTENCE.** Add strict package capability manifest
  JSON read/write support before compiler-derived manifests are wired into CLI
  audit flows.

  Done 2026-08-23: `omega-packages` can parse package capability manifest JSON
  with schema-version, required/unknown field, package-name, dependency-alias,
  array/object, optional string, and integer checks; normalize parsed
  manifests; and read/write standalone manifest files through same-directory
  temporary files and atomic rename.

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

- **SOURCE-CACHE-POLICY-PERSISTENCE.** Add strict source-cache policy record
  parsing and file persistence before install/lock wiring consumes resolver
  evidence.

  Done 2026-08-23: `omega-packages` can parse source-cache policy record JSON
  with strict schema-version, required/unknown field, verdict, optional
  string, and integer checks; read/write standalone record files through
  same-directory temporary files and atomic rename; expose command-level
  locator-to-record-file writing; and preserve deterministic record
  fingerprints across read/write round trips.

- **PACKAGE-LOCK-CLOSURE-VALIDATION.** Add fail-closed validation for assembled
  package lock closures before compiler/CLI lock wiring.

  Done 2026-08-23: `omega-packages` validates that a package lock contains its
  root package, has no duplicate package entries, has no duplicate dependency
  aliases within a package, has no dependency edges to packages absent from the
  closure, retains non-empty source identities and manifest fingerprints, and
  contains only packages reachable from the root package.

- **PACKAGE-LOCK-REACHABILITY-VALIDATION.** Move graph reachability into lock
  closure validation before any lock read/write path can accept an unreachable
  package row.

  Done 2026-08-23: `PackageLock::validate_closure`, lock persistence, lock
  assembly, and graph audit now reject package entries that are not reachable
  from the lock root through dependency aliases.

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

- **CAPABILITY-CHANGE-RECEIPT-PERSISTENCE.** Add standalone receipt JSON
  read/write support before deciding final receipt placement.

  Done 2026-08-23: `omega-packages` can parse capability-change receipt JSON
  with strict schema-version, field, severity, and non-empty review checks,
  reject empty or duplicate accepted-delta lists, and read/write standalone
  receipt files through same-directory temporary files and atomic rename.

- **CAPABILITY-CHANGE-REVIEW-COMMAND-API.** Add the command seam that creates
  explicit capability-change review receipts from old/new package manifests.

  Done 2026-08-23: `omega-packages` exposes a command-style receipt creation
  API that compares normalized old/new package manifests, rejects package
  mismatches, no-op changes, and source-only updates, binds the receipt to the
  exact old/new source identities and manifest delta fingerprints, and returns
  reviewer-facing diff text without choosing final receipt storage.

- **OMEGA-REVIEW-CAPABILITY-CHANGE-CLI.** Expose explicit capability-change
  receipt creation through the Rust on-ramp `omega` binary.

  Done 2026-08-23: `omega review capability-change --old-manifest
  <manifest.json> --new-manifest <manifest.json> --reviewer <id> --reason
  <text> --out <receipt.json>` reads strict package manifest files, rejects
  no-op, source-only, and package-mismatched updates, writes the exact
  standalone review receipt requested by the caller, and prints the
  reviewer-facing diff summary without editing `build.omg` or `omega.lock`.

- **LOCAL-PACKAGE-FIXTURE-CORPUS.** Add the first local package corpus for
  resolver, install, update, and audit tests.

  Done 2026-08-23: `fixtures/packages/` contains normal kebab-case package
  directories for `arithmetic-kernels`, `generated-table`, `file-journal`,
  `network-overreach`, `axiom-ledger`, `provider-switchboard`,
  `capability-vault`, and `graph-workbench`. Each fixture has `build.omg`,
  `main.omg`, package notes, and focused intent. `omega-packages` resolves all
  fixture directories as distinct local source identities in tests.

- **REMOTE-PACKAGE-FIXTURE-MIRRORS.** Mirror the local package corpus to GitHub
  under the `CathedralOS` organization for network/package-manager tests.

  Done 2026-08-23: created private repositories for the eight fixture packages
  under `CathedralOS` and pushed initial package contents. Exact commit pins are
  recorded in `fixtures/packages/REMOTE_PINS.md`; remote acceptance tests should
  use those commits rather than branch names.

- **PACKAGE-GRAPH-AUDIT-CORE.** Add the internal graph-audit core before CLI
  exposure.

  Done 2026-08-23: `omega-packages` can audit a validated package lock plus
  supplied package manifests, reject invalid locks, missing/duplicate
  manifests, manifest fingerprint drift, and unreachable lock entries, and
  report dependency paths for exported service reach.

- **SOURCE-REQUEST-PARSING.** Add CLI-ready source locator parsing without
  performing install/update mutation.

  Done 2026-08-23: `omega-packages` can classify local path locators,
  `file://` locators, HTTPS Git URLs, and SSH/scp-style Git locators into
  `PackageSourceRequest`, reject empty locators, and reject revision arguments
  for local sources.

- **REMOTE-FIXTURE-RESOLUTION-TESTS.** Add exact-pin tests for the private
  GitHub package fixture mirrors without making normal CI depend on private
  network access.

  Done 2026-08-23: `omega-packages` has a `remote_fixtures` integration test
  that validates `fixtures/packages/REMOTE_PINS.md` in normal test runs and an
  ignored network test that resolves all eight private `CathedralOS` fixture
  repositories by exact commit over SSH and verifies the fetched content
  identity matches the checked-in local fixture.

- **PACKAGE-GRAPH-AUDIT-DETAILS.** Surface existing manifest and lock evidence
  through the internal graph-audit model before CLI exposure.

  Done 2026-08-23: graph audit package rows now include source kind, source
  locator, dependency aliases, exported service reach, provider requirements,
  provider selections/origins/plans, capability-flow verb counts, lock trust
  receipts, and manifest trust receipts. Text output reports those fields for
  future `omega audit packages` CLI wrapping.

- **PACKAGE-GRAPH-AUDIT-COMMAND-API.** Add the lock-file backed command seam
  for future `omega audit packages` CLI wiring.

  Done 2026-08-23: `omega-packages` exposes a command-style graph audit API
  that reads and validates an `omega.lock` path, accepts compiler-supplied
  package manifests, runs the graph audit core, returns report text, and
  preserves distinct lock-persistence versus graph-consistency errors.

- **PACKAGE-GRAPH-AUDIT-MANIFEST-FILE-API.** Add the file-backed graph audit
  command seam for future `omega audit packages --manifest <path>` style CLI
  wiring.

  Done 2026-08-23: `omega-packages` can read the current `omega.lock`, load
  package capability manifests from strict JSON files, preserve distinct lock,
  manifest-file, and graph-consistency errors, and return the existing graph
  audit text without requiring compiler manifest derivation.

- **OMEGA-AUDIT-PACKAGES-MANIFEST-FILE-CLI.** Expose the first read-only
  package graph audit surface through the Rust on-ramp `omega` binary.

  Done 2026-08-23: `omega audit packages [--lock <omega.lock>] --manifest
  <manifest.json>...` reads an existing package lock and precomputed package
  capability manifest files, runs the package graph audit, and prints the
  audit report without deriving manifests, executing dependency `build.omg`,
  editing `build.omg`, or writing `omega.lock`.

- **PACKAGE-LOCK-ASSEMBLY-FROM-MANIFESTS.** Add a lock assembly helper for
  compiler-supplied package manifests before full compiler/CLI lock wiring.

  Done 2026-08-23: `omega-packages` can assemble a validated `PackageLock`
  from a root package identity plus normalized package capability manifests.
  Lock entries copy exact source kind/locator/identity, manifest fingerprints,
  build observation class, dependency aliases, and trust receipt identities.
  Assembly rejects duplicate manifest packages and open dependency edges.

- **OMEGA-LOCK-ASSEMBLE-MANIFEST-FILE-CLI.** Expose a file-backed package-lock
  assembly surface through the Rust on-ramp `omega` binary before compiler
  manifest derivation and full install/update mutation.

  Done 2026-08-23: `omega lock assemble --root-package <package> --manifest
  <manifest.json>... --out <omega.lock>` reads strict package capability
  manifest files, assembles a closed and reachable package lock rooted at the
  requested package, graph-audits the result, writes the explicit output lock
  atomically, and prints the lock fingerprint plus graph audit summary. It does
  not derive manifests, execute dependency `build.omg`, edit `build.omg`, or
  choose final install/update mutation semantics.

- **PACKAGE-LOCK-UPDATE-PLAN.** Add a non-mutating update dry-run plan for
  future `omega update` wiring.

  Done 2026-08-23: `omega-packages` can validate the current lock against
  current manifests, compare a target package's current and candidate
  manifests, apply default or receipt-backed update admission, and assemble a
  candidate lock only when policy admits the update. Rejected updates produce a
  plan without a candidate lock.

- **PACKAGE-LOCK-INSTALL-PLAN.** Add a non-mutating install dry-run plan for
  future `omega install` wiring.

  Done 2026-08-23: `omega-packages` can validate the current package graph,
  reject an already-bound root dependency alias, require candidate
  compiler-supplied manifests to bind the requested alias to the requested
  package, assemble and audit a candidate package lock, and report newly added
  package identities without editing `build.omg` or writing `omega.lock`.

- **PACKAGE-PLAN-COMMAND-APIS.** Add lock-file backed command seams for
  non-mutating install and update plans before full CLI mutation.

  Done 2026-08-23: `omega-packages` exposes command-style APIs that read the
  current `omega.lock`, accept compiler-supplied current/candidate package
  manifests, load an optional standalone capability-change receipt for update
  planning, and return install/update plan text without editing `build.omg` or
  writing `omega.lock`.

- **OMEGA-PLAN-INSTALL-UPDATE-CLI.** Expose non-mutating package install/update
  plan commands through the Rust on-ramp `omega` binary before full
  install/update mutation.

  Done 2026-08-23: `omega plan install --lock <omega.lock>
  --current-manifest <manifest.json>... --candidate-manifest <manifest.json>...
  --alias <alias> --package <package>` and `omega plan update --lock
  <omega.lock> --current-manifest <manifest.json>... --candidate-manifest
  <manifest.json>... --package <package> [--receipt <receipt.json>]` read an
  existing lock, explicit current/candidate package capability manifest files,
  and optional capability-change receipt, then print the plan without fetching
  sources, deriving manifests, executing dependency `build.omg`, editing
  `build.omg`, or writing `omega.lock`.

- **UPDATE-PLAN-CANDIDATE-GRAPH-AUDIT.** Ensure non-mutating update plans fail
  closed if the admitted candidate lock is not graph-auditable.

  Done 2026-08-23: `plan_package_lock_update` now audits the assembled
  candidate lock before returning it and rejects unreachable or otherwise
  graph-invalid candidate package closures instead of handing a bad lock to
  later CLI persistence.

- **PACKAGE-SOURCE-AUDIT-LOCATOR-API.** Add the command seam that combines
  source locator parsing with resolver-owned source audit.

  Done 2026-08-23: `omega-packages` exposes a source-audit API that accepts a
  locator string plus optional revision, reuses the centralized source-request
  parser, resolves through existing local/Git resolver policy, and preserves
  parse failures separately from source-resolution failures.

- **OMEGA-AUDIT-SOURCE-CLI.** Expose resolver-owned source identity audit
  through the Rust on-ramp `omega` binary before install/update mutation.

  Done 2026-08-23: `omega audit source <locator> [--rev <rev>] [--cache-dir
  <dir>]` resolves local paths and Git locators through the package source
  audit API, prints source kind, locator, requested/resolved revision evidence,
  content identity, file count, and byte count, and exits without editing
  `build.omg` or writing `omega.lock`.

- **SOURCE-CACHE-POLICY-LOCATOR-API.** Add the command seam that combines
  source locator parsing with resolver-owned source-cache policy records.

  Done 2026-08-23: `omega-packages` exposes a source-cache policy API that
  accepts a locator string plus optional revision, reuses the centralized
  source-request parser, resolves through existing local/Git source-cache
  policy records, and preserves parse failures separately from policy records.

- **OMEGA-AUDIT-SOURCE-CACHE-POLICY-CLI.** Expose resolver-owned source-cache
  policy records through the Rust on-ramp `omega` binary before install/update
  mutation.

  Done 2026-08-23: `omega audit source-cache-policy <locator> [--rev <rev>]
  [--cache-dir <dir>] [--out <record.json>]` resolves local paths and Git
  locators through the source-cache policy API and prints the deterministic
  JSON policy record, including accepted or rejected verdict, limits, submodule
  policy, path policy, cache path, source identity, and content counts when
  available. With `--out`, it writes the exact record to the requested path.

- **LOCAL-FIXTURE-GRAPH-AUDIT-COVERAGE.** Exercise graph audit against the
  checked-in local package corpus.

  Done 2026-08-23: `omega-packages` has an integration test that resolves real
  local fixture source identities for `graph-workbench`, `arithmetic-kernels`,
  and `file-journal`, constructs package manifests from fixture intent,
  assembles a package lock, and verifies audit output reports the dependency
  path and capability-flow row for `graph-workbench -> file-journal`.

Remaining:

- **PACKAGE-CAPABILITY-MANIFEST.** Define the normalized manifest produced for
  each resolved package. It should include source identity, public API contract
  identity, exported service reach, build-machine reach and observation class,
  dependency aliases, provider requirements/selections, routed qualifications,
  capability-flow counts and source rows, unresolved installation rows, and
  trust/admission receipts.

  Remaining after `PACKAGE-MANIFEST-MODEL` and
  `PACKAGE-MANIFEST-PERSISTENCE`: settle the owner question "What exact checked
  evidence defines a package capability manifest?", then wire compiler evidence
  extraction so compiling one package emits the manifest rather than requiring
  tests or callers to construct it manually. The existing executable
  capability manifest is entry-oriented and is not a package manifest.

  Acceptance: compiling one package can emit a deterministic machine-readable
  package capability manifest and a concise human diff. Equal source/evidence
  emits byte-identical manifest content.

- **PACKAGE-LOCK-CLOSURE.** Extend the existing `omega.lock` direction from
  trust receipts only to the full package closure without making it a second
  hand-authored manifest.

  Remaining after `PACKAGE-LOCK-MODEL`,
  `PACKAGE-LOCK-CLOSURE-VALIDATION`, `PACKAGE-LOCK-PERSISTENCE`, and
  `PACKAGE-LOCK-ASSEMBLY-FROM-MANIFESTS`, and
  `PACKAGE-LOCK-REACHABILITY-VALIDATION`, and
  `OMEGA-LOCK-ASSEMBLE-MANIFEST-FILE-CLI`: settle the package-manifest evidence
  boundary, then wire compiler/package admission output into the default
  `omega.lock` artifact without requiring explicit manifest-file arguments.

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
  `SOURCE-CACHE-POLICY-RECORDS`, `SOURCE-CACHE-POLICY-PERSISTENCE`,
  `SOURCE-REQUEST-PARSING`, `REMOTE-FIXTURE-RESOLUTION-TESTS`, and
  `SOURCE-CACHE-POLICY-LOCATOR-API`, and `OMEGA-AUDIT-SOURCE-CLI`, and
  `OMEGA-AUDIT-SOURCE-CACHE-POLICY-CLI`: add install-command integration and
  lock wiring.

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

  Remaining after `PACKAGE-LOCK-INSTALL-PLAN`, `PACKAGE-PLAN-COMMAND-APIS`,
  and `OMEGA-PLAN-INSTALL-UPDATE-CLI`: settle package-manifest evidence,
  `build.omg` dependency API, review-receipt placement, and dependency
  `build.omg` admission sequence, then wire source resolution,
  package-admission manifest derivation, `build.omg` alias/pin editing, and
  lock persistence around the install plan.

  Acceptance: adding a dependency produces a pinned alias in `build.omg`, an
  updated lock entry, and an audit summary that names new reachable services,
  provider/trust receipts, and capability-flow verbs.

- **OMEGA-UPDATE.** Add `omega update [alias...] [--to <rev>]`. The default
  update path may move source pins only when the dependency's normalized
  package capability manifest is unchanged.

  Remaining after `DEFAULT-UPDATE-ADMISSION` and
  `PACKAGE-LOCK-UPDATE-PLAN`, `CAPABILITY-CHANGE-RECEIPT-PERSISTENCE`, and
  `PACKAGE-PLAN-COMMAND-APIS`, and
  `UPDATE-PLAN-CANDIDATE-GRAPH-AUDIT`, and
  `OMEGA-PLAN-INSTALL-UPDATE-CLI`: settle package-manifest evidence,
  `build.omg` dependency API, review-receipt placement, and dependency
  `build.omg` admission sequence, then wire candidate resolution,
  package-admission manifest derivation, `build.omg` pin editing, and lock
  persistence around the update decision.

  Acceptance: changing source bytes with the same capability manifest updates
  the pin and lock. Any manifest delta rejects before changing `build.omg` or
  the lock and prints a severity-ranked diff.

- **OMEGA-AUDIT-PACKAGES.** Add a read-only audit command for the package graph.

  Remaining after `PACKAGE-SOURCE-AUDIT-COMMAND-API`,
  `PACKAGE-GRAPH-AUDIT-CORE`, `PACKAGE-GRAPH-AUDIT-DETAILS`, and
  `PACKAGE-GRAPH-AUDIT-COMMAND-API`, and
  `PACKAGE-GRAPH-AUDIT-MANIFEST-FILE-API`, and
  `OMEGA-AUDIT-PACKAGES-MANIFEST-FILE-CLI`: settle package-manifest evidence,
  derive package manifests for a resolved graph, and make the default `omega
  audit packages` flow find them without explicit manifest-file arguments.

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

Local package fixtures live under `fixtures/packages/`; remote GitHub mirror
pins live in `fixtures/packages/REMOTE_PINS.md`.

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

  Remaining after `CAPABILITY-REVIEW-RECEIPT-MODEL`,
  `REVIEWED-UPDATE-ADMISSION`, and
  `CAPABILITY-CHANGE-RECEIPT-PERSISTENCE`, and
  `CAPABILITY-CHANGE-REVIEW-COMMAND-API`, and
  `OMEGA-REVIEW-CAPABILITY-CHANGE-CLI`: wire receipt loading into `omega
  update` and persist accepted receipts in or beside `omega.lock` once receipt
  placement is settled.

  Acceptance: higher-capability updates require an acceptance receipt. New
  root-memory, DMA/IOMMU, executable-installation, interrupt-publication,
  dynamic-loader, process, filesystem, network, signing, or secret reach is
  elevated in the diff with the dependency path that introduced it.

- **AUTHOR-GUIDANCE.** Add diagnostics that advise package authors to keep
  unrelated capabilities in separate packages and publish reach ceilings on
  public APIs.

  Remaining after `AUTHOR-GUIDANCE-DIFF-MODEL`: settle package-manifest
  evidence, then surface this guidance through package-admission, `omega
  install`, `omega update`, and `omega audit packages` once those flows derive
  package manifests.

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
