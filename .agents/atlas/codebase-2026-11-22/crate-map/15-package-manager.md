# Crate map: 15-package-manager

Source: checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d0559`. Paths are relative to it.
Counts of files and lines come from `crates.tsv` (read). Test counts come from
`grep -rc '#\[test\]' --include=*.rs <crate dir>`, with nested crates subtracted. Reverse dependencies come from
`grep -l "^<name> = "` over `source-rust` manifests (`_recycling/` hits left out).

## What the area does

The package manager turns a project's `build.omg` dependency declarations into an exact, immutable source graph,
gathers the compiler's findings about each package, records the owner's accept/reject decisions, and publishes
`build.omg` plus `omega.lock` as one pair. Input: a project directory, declared local/workspace/Git dependencies,
and (during install/update) Build results and compiler review facts returned by the application's Build host.
Output: captured read-only source trees (`CapturedSourceTree`), a resolved package closure, a lock section per target
(`PackageLock`), editable review files, and prepared products for compilation (`project::open_products`). It does
not run Build or the compiler itself; the caller (`application`, `project-build`) runs each requested Build and hands
the evidence back (`source-rust/subsystems/package-manager/README.md:3-8`, `package_manager.rs:1-2`). The area
comprises six crates, all under `source-rust/subsystems/package-manager/`. Two files outside the directory are also
in the area's unit list: `source-rust/subsystems/compiler/tests/final_stage_validators.rs` (122 lines) and
`.../tests/support/schema_customer_source.rs` (152 lines) (read from `units.tsv`; not read for content).

## Crates, in pipeline order

### 1. resolver-execution

- Path `source-rust/subsystems/package-manager/source-acquisition/git/execution`; 6 files, 466 lines.
- Prepares the Git commands that resolution runs: a fixed absolute Git executable kept outside package-controlled
  roots, a closed list of phases, and one working root per phase. Process running and limits live in `bounded-process`.
- Consumes an executable path and roots; produces a prepared command: `ResolverExecutionBackend` (`src/resolver_execution.rs:33`),
  `ResolverExecutionPhase` (`:25`). Entry points: `ResolverExecutionBackend::open` (`:38`), `prepare` (`:66`).
- Production deps: `bounded-process` (foundation). Dependents (2): `omega` (application), `source-acquisition`.
- Modules: `resolver_execution.rs` (phases, command prep), `path_custody.rs` (absolute/canonical/bounded path checks), `tests/{mod,unix,validation}.rs`.
- Tests: 6 `#[test]`s, in-crate unit tests on path custody and validation; no `[[test]]` target.
- Open work: none named in `TASKS.md` or `OWNER_CODE_CONCERNS.md` (grepped).

### 2. source-selection

- Path `.../source-acquisition/capture/selection`; 1 file (`selection.rs`), 635 lines.
- Defines which part of a source tree a caller asks to capture: non-overlapping obligations, canonical paths and namespace
  budgets. Callbacks give member metadata; it opens no files.
- Produces `SourceCaptureRequest` (`selection.rs:38`) with `SourceCaptureObligation` (`:32`), `SourceSelectionError` (`:102`).
  Entry points: `SourceCaptureRequest::entire_tree` (`:45`), `::new` (`:51`), `select_source_members` (`:157`),
  `canonical_filesystem_metadata_path_is_canonical` (`:13`).
- Production deps: none in the repository (empty `[dependencies]`). Dependents (3): `source-capture`'s parent `source-acquisition`,
  `package-compilation` (`compiler/build/inputs`), `filesystem-authority`.
- Modules: single file. Tests: 10 `#[test]`s inline.
- Open work: none named.

### 3. source-capture

- Path `.../source-acquisition/capture`; 5 files, 783 lines (its `selection/` subdirectory is the crate above).
- Holds the in-memory immutable trees that acquisition hands to compilation and a file-system view over several
  captured roots, plus a read counter.
- Produces `CapturedSourceTree` / `CapturedSourceNode` (`tree.rs:17`, `:10`), `SourceFileSystem` (`filesystem.rs:10`);
  `read_accounting::{source_file_read_count, record_source_file_read}` (`read_accounting.rs:7,11`). Entry points: `SourceFileSystem::new/add_capture/join_captures/read_to_string/captured_files` (`filesystem.rs:44,48,23,120,145`), `CapturedSourceTree::new` (`tree.rs:22`).
- Production deps: `record-storage`. Dependents (3): `package-manager`, `source-acquisition`, `package-compilation`.
- Modules: `tree.rs`, `filesystem.rs`, `read_accounting.rs`, `tests.rs`.
- Tests: 6 `#[test]`s in `tests.rs`. Open work: none named.

### 4. source-acquisition

- Path `.../package-manager/source-acquisition`; 142 files, 22460 lines (this count includes the three nested crates above;
  own files = 142-5-1-6 = 130 only if `crates.tsv` rows for children are nested, inferred).
- Turns hostile local or Git input into a read-only, content-verified snapshot with an identity. Owns source lineage and digests,
  bounded no-follow tree capture, snapshot publication, the retained storage lanes and cache, and Git transport and object verification
  (`source-acquisition/README.md:1-8`, `lib.rs:1-20`). It stops at immutable custody; declarations and graphs belong to `package-manager`.
- Produces `ResolvedGitSource` (`observations/resolved.rs:22`), `GitAcquisitionPin` (`:40`), `ResolvedLocalSnapshot`
  (`local/resolution_observations.rs:70`), `ImmutableSourceResolution` (`identity/resolution.rs:24`), `SourceLineage`.
  Consumes a locator/revision (`GitSourceRequest`, `git/request.rs`) or a local path.
- Entry points: `SourceResolver::for_current_user` (`source_resolver.rs:41`, session holds storage lanes and one host-Git choice),
  `local::operations::resolve_local_source` (`local/operations.rs:42`), `capture_local_source_input` (`:50`),
  `capture_selected_local_source_input` (`:59`), `verify_package_source_snapshot` (`:135`), `resolve_local_source_snapshot_in_lane` (`:183`),
  `git::resolution::resolve_git_source_in_lane` (`git/resolution/mod.rs:69`), `GitWorkspaceProjectionPlanner` (re-exported in `lib.rs`).
- Production deps (from `Cargo.toml`): `source-selection`, `source-capture`, `resolver-execution`, `wire`, `record-storage`,
  `bounded-process`, `platform-custody`; external `cap-std`, `cap-fs-ext`, `sha1-checked`, `sha2`, `nix` (unix), `cap-primitives` (windows).
  Dependents (3): `package-manager`, `omega` (application), `package-compilation`.
- Modules: `identity/` (lineage, locator, digests), `tree/` (capture and tree identity), `snapshot/` (construction, permissions, publication),
  `custody/` (locks, retained-tree validation, cache publication), `local/` (adapter, `recovery`, `staging`, `snapshot/address`),
  `git/` (`request`, `cache/`, `executable/`, `git_command`, `objects/`, `resolution/`, `snapshot`, `workspace/`), `observations/`, `storage.rs`, `source_resolver.rs`, `limits.rs`, `error.rs`.
- Tests: 174 `#[test]`s in-crate (`git/tests/`, `local/tests/`, `custody/tests/`, `identity/tests.rs`, `tree/filesystem_read_tests.rs`); feature `test-fixtures` exists; no `[[test]]` target.
- Open work: none named in `TASKS.md`/`OWNER_CODE_CONCERNS.md` (grepped).

### 5. review-evidence

- Path `.../package-manager/review/evidence`; 218 files, 23538 lines.
- Defines the stable, inert vocabulary for package policy (rows, baselines) and the canonical byte/text encoding and bounded recovery of it. Lock
  recovery, policy comparison and permission comparison all use these records. It never inspects live compiler state or admits anything
  (`src/lib.rs:1-2`, `src/record.rs:1-10`, `src/encoding/mod.rs:1-6`).
- Produces `PackagePolicyBaseline` (`src/record/baseline.rs:35`), policy rows and component records (external supply, calling, representation,
  selected-provider, terminal-permission, callable); `TerminalPermissionAcceptance` (`src/permission_acceptance.rs:9`).
  Entry points: `PackagePolicyBaseline::recover_canonical` (`src/encoding/recovery/policy/baseline.rs:22`),
  `TerminalPermissionAcceptance::compare_policy` (`permission_acceptance.rs:39`), `compare_runtime_permissions` (`:25`),
  `restricted_build_request_acceptance_text`, `PackagePolicyRecoveryLimits`, `PackagePolicyMembershipLimits` (`src/encoding/mod.rs`).
- Production deps (19, from `Cargo.toml`): compiler crates (`build-consent`, `provider-acceptance`, `provider-plans`, `service-surface`, `source-provenance`,
  `application-identity`, `numeric-literals`, `layout-plans`, `terminal-psi`, `target`, `semantic-vocabulary`, `psi-effect-contracts`, `language-core`, `numerics`, `symbols`),
  native (`native-intrinsics`, `calling-conventions`), foundation (`wire`, `diagnostics`).
  Dependents (3): `package-manager`, `compiler`, `project-build`.
- Modules: `record/` (value shapes and invariants: baseline, calling, conformances, contracts, domains, representation, signatures, restricted_build, semantic_bindings), `encoding/encode/` (canonical writers, membership checks), `encoding/recovery/policy/` (bounded readers), `permission_acceptance.rs`, `projection_error.rs`. Docs: `EVIDENCE_SCHEMA.md`, `POLICY_BASELINE.md`.
- Tests: 55 `#[test]`s inline; no `[[test]]` target.
- Open work: `OWNER_CODE_CONCERNS.md` entry "Clippy with `-D warnings` cannot lint an application test because four crates it builds carry warnings" (names `review-evidence`: an unused import; also cites `encoding/encode/selected_providers/signature.rs#L4`).

### 6. package-manager

- Path `source-rust/subsystems/package-manager`; 151 files, 29299 lines (own, excluding the five crates above).
- The orchestrator. Resolves declared dependencies against acquired sources, reconciles the graph, reviews and compares policy, resolves
  decisions, reads and writes `omega.lock`, and runs install, update, resume, discard and audit as transactions. It holds the project lock while the
  caller runs Build (`README.md:3-8`).
- Consumes `BuildRequest`-driven source requests, `PackageChangeReview` inputs from the compiler, and the crates above; produces
  `PreparedLocalProject` (`dependencies/prepare_project.rs:58`), `ResolvedPackageProject` (`project/mod.rs:37`), `ProjectProducts` (`project/mod.rs:363`),
  `PackageLock` (`lockfile/mod.rs:209`) with `PackageLockTarget` (`:90`), `PackageChangeReview` (`review/package_change.rs:35`), `PackageCommandOutcome` (`transaction/mod.rs:668`).
- Entry points: `PackageCommandTransaction::open/prepare/acquire_root/source_acquisitions/complete_source_resolution` (`transaction/mod.rs:62,77,250,262,303`),
  `PreparedPackageCommand::review/complete` (`:421,:466`), `discard_package_review` (`:581`); `PackageInspectionTransaction::open/prepare/review_target/finish`
  (`review/inspect_packages.rs:109,122,281,339`); `begin_local_project_preparation` (`dependencies/prepare_project.rs:251`);
  `project::resolve_build_sources` (`project/mod.rs:67`), `open_product` (`:418`), `open_products` (`:442`);
  `review::review_package_change_reusing` (`review/package_change.rs:101`), `PackageChangeReview::propose_lock_target` (`:60`);
  `lockfile::publication::checked::publish_reviewed_package_change` (`lockfile/publication/checked.rs:21`);
  `dependencies::stage_build_dependency_edit` (`dependencies/stage_build_edit.rs:13`); `ReviewedHostPackages` (`dependencies/reviewed_host_packages.rs:6`).
- Production deps: 41 path crates in `Cargo.toml` (read), among them `project-build`, `package-compilation`, `build-declarations`, `build-execution`, `terminal-psi`,
  `review-evidence`, `source-acquisition`, `source-capture`, `source-admission`, plus external `cap-std`, `cap-fs-ext`, `sha2`.
  Dependents (2): `omega` (application) and `project-build` (`compiler/build`) (the Cargo graph names both ways; `project-build` is in both lists, inferred to be a dev-dependency or a cycle through a feature, not checked).
- Modules: `transaction/` (command admission, planning, decision join, proposal and framing, source review, state); `project/` (build-source resolution, `open_products`, services);
  `dependencies/` (`declarations/` reading and editing `build.omg`, `graph/` accepted graph, Git pins, reconcile, subject; `source/` local/Git/workspace binding; `prepare_project.rs`; `compiler_input.rs`);
  `review/` (`candidate/` fresh and cached occurrence review, `compare/` policy comparison, `decision/` choices and editable review document, `audit/` source patches,
  `inspect_packages/`, `inspect_source/`, `build_session`, `restricted_build_grants`); `lockfile/` (`mod.rs` `PackageLock`, `text/` v4 framing reader/writer, `decisions/`, `occurrences.rs`, `publication/` journaled file pair publication).
- Tests: one `[[test]]` target `project_preparation` (`tests/project_preparation.rs`: host vs requested target for build and product dependencies, through `project::open_product`); about 63 in-crate `#[test]`s
  (e.g. `dependencies/prepare_project_tests.rs`, `project/workspace_tests.rs`). `autotests = false`.
- Open work: `TASKS.md` item **NO-IMPLICIT-COMPILER-STORAGE** (delete `review/candidate/review_cache.rs`); `OWNER_CODE_CONCERNS.md` entry "Clippy with `-D warnings` cannot lint an application test..." (names `package-manager`: ten warnings, mostly unused code, `review/restricted_build_grants.rs`).
  `TASKS.md` items **VALIDATION-SUBJECTS** and **ORDINARY-LIBRARY-PACKAGES** touch package handling generally; I did not trace whether they edit this crate.

## How data flows through the area

1. `application` calls `PackageCommandTransaction::open` (`transaction/mod.rs:62`) and `prepare` (`:77`), or `begin_local_project_preparation` (`dependencies/prepare_project.rs:251`) for ordinary builds.
2. `dependencies::declarations` reads the checked dependency declarations from `build.omg`; `dependencies::source` binds each local, workspace or Git location to acquisition.
3. `source-acquisition` resolves each source (`resolve_local_source`, `resolve_git_source_in_lane`) using `source-selection` for the requested subset and `resolver-execution` for Git, and returns verified snapshots and `ImmutableSourceResolution`s.
4. `source-capture` holds the captured trees as `SourceFileSystem` for Build and compilation.
5. `dependencies::graph` reconciles requests into one closure and a canonical subject; `project::resolve_build_sources` / `open_products` give the compiler its products.
6. The caller runs Build and compilation; `review::candidate` joins their results to policy (`review_package_change_reusing`), producing `PackagePolicyBaseline` rows via `review-evidence`.
7. `review::compare` diffs against the accepted lock section; `review::decision` renders the editable review file and recovers the owner's `accept`/`reject` edits.
8. `PackageChangeReview::propose_lock_target` builds the unwritten `PackageLockTarget`; `lockfile::publication::checked::publish_reviewed_package_change` journals and writes `build.omg` and `omega.lock` (version 4 text, `lockfile/text/`).
9. Ordinary accepted compilation reads the lock (`PreparedLocalProject::accepted_deployment`, `prepare_project.rs:111`), checks source and compiler identities and passes the accepted bindings to Build.

## Where this map differs from the documents

- `README.md:124` says "`admit_project_compilation` returns ordered requests and a completion owner". Grepped `source-rust` and `wiki` for that name: the only hit is the README. The
  surrounding functions are `source_acquisitions` and `complete_source_resolution` (`dependencies/prepare_project.rs:454,503`; `transaction/mod.rs:262,303`).
- `README.md` (compare/decision section) names `review::compare_package_policy_changes`, `review::resolve_package_policy_decisions`, `review::render_package_policy_review`, `review::recover_package_policy_review`
  as public-sounding API. In code they are `pub(crate)` (`review/compare/mod.rs:163`, `review/decision/mod.rs:124`, `review/decision/document.rs:57,65`), so only the transaction inside the crate calls them.
- `README.md` names `lock::PackageLock`, `PackageLockRecoveryLimits` and `publish_reviewed_package_change` as package-manager surface; `PackageLock` is `pub(crate)` (`lockfile/mod.rs:209`) and `publication` is a `pub(crate)` module (`lockfile/mod.rs:287`); `publish_reviewed_package_change` is `pub fn` inside it (`checked.rs:21`) but unreachable from outside the crate.
- `dependencies/mod.rs` comment says resolution lives in `resolution/mod.rs`; there is no such file. `package_manager.rs` declares `resolution` as an inline module re-exporting `dependencies::{graph, source}`.
- `README.md` says the crate's tree has `package_manager.rs` plus `project/`, `dependencies/`, `lockfile/`, `review/`, `transaction/`, `source-acquisition/`; it omits that `review/evidence/` is its own crate. `package_manager.rs` also re-exports `transaction as package_manager` and `lockfile as lock` (`package_manager.rs:8-9`).
- `README.md` links `TASKS_OPTIMIZER.md#verification-and-rollout` for `TARGET-MATRICES`; the heading exists (`TASKS_OPTIMIZER.md:358`) but grepped `TARGET-MATRICES` in all `.md` files: only the README uses it.
- `units.tsv` lists a path `package_manager.rs/` with 1 file, 65 lines (it is the library root file, `package_manager.rs`), and two compiler test files under this area; the compiler test files are not part of any crate listed here.

## What I did not read

- Bodies of most modules: `dependencies/graph/reconcile`, `dependencies/source/git`, `review/candidate/*`, `review/compare/*`, `lockfile/text/*`, `lockfile/publication/*`, everything under `review/evidence/src/encoding/` and `source-acquisition/git/objects`, `custody/`, `tree/`.
- The six spec pages under `wiki/spec/packages/` (`sources.md`, `resolver_security.md`, `acceptance.md`, `review.md`, `toolchain.md`) were not opened; only linked by READMEs.
- The two `compiler/tests` files in the area; `lockfile/README.md`, `dependencies/README.md`, `review/inspect_packages/README.md`, `lockfile/publication/README.md`, `EVIDENCE_SCHEMA.md`, `POLICY_BASELINE.md`.

## What I am unsure about

- Whether `source-acquisition`'s 142 files include the nested crates (it appears so from paths, but `crates.tsv` could count them separately); the "own" figure above is inferred.
- Why `project-build` is both a dependency and a dependent of `package-manager` (grepped manifests only: `compiler/build/Cargo.toml` lists `package-manager`, and `package-manager/Cargo.toml` lists `project-build`). A direct cycle would not build, so one of them may be a dev-/target-specific entry; I did not read the build manifest sections.
- Per-crate `#[test]` counts count attributes only; they exclude `cfg(test)` helpers and any test macros.
- `OWNER_CODE_CONCERNS.md` and `TASKS.md` were searched by keyword (`package`, crate names), so an entry that names these crates only by file path in a different form could be missed.
