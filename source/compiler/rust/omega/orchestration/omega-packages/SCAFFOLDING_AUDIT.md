# Omega package scaffolding audit

Status: refreshed complete inventory, 2026-08-24. This audit classifies the current Rust
on-ramp against the corrected package identity and admission model. It does not
authorize any listed API as a production trust input.

The governing contracts are
[`TASKS_PACKAGE_MANAGER.md`](../../../../../../../TASKS_PACKAGE_MANAGER.md),
[`package_manager_first_draft.md`](../../../../../../../wiki/design_briefs/package_manager_first_draft.md),
[`build_and_package_model.md`](../../../../../../../wiki/design_briefs/build_and_package_model.md),
and language-guide chapters
[15](../../../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
and
[19](../../../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md).

## Classification

- **Retain** means the implementation has no independent admission authority
  and can survive behind corrected typed inputs after focused hardening.
- **Rewrite** means the responsibility remains, but the current data or trust
  model cannot cross the production boundary.
- **Delete/quarantine** means the surface encodes a superseded workflow and
  must not remain reachable from the production `omega` command.

| Production surface | Classification | Finding and required disposition |
| --- | --- | --- |
| `src/json.rs` | Retain | A small strict internal parser. It grants no authority by itself. Keep it private and use it only for versioned tool-owned encodings. |
| `src/source.rs` local traversal and hashing | Rewrite | Identity uses injective framing over raw path bytes, files, symlinks, every directory including empty ones, symlink spelling, executable mode, length, and contents; it rejects special files and links into excluded Git metadata and bounds reads before allocation. A bounded capture is re-materialized into a content-addressed, read-only, atomically published snapshot and checked against ordinary concurrent mutation. Directory permissions normalize to the canonical snapshot mode. Deliberately hostile same-user races and tool outputs already present beneath the selected source root remain outside this cooperative boundary. |
| `src/source.rs` Git resolution | Rewrite | Cache keys are full and policy-versioned; staged entries bind exact resolver metadata, origin, and a strict local config; resolver access is locked; Git configuration is sealed; and submodule declarations/gitlinks reject before materialization. Validated tree/blob objects are copied into an atomically published read-only snapshot without checkout, filters, hooks, submodules, or package execution, and the snapshot is re-hashed before reuse. Git subprocesses have null stdin, bounded concurrent stdout/stderr capture, deadlines, and whole-container cleanup through Unix process groups or Windows Job Objects. Cooperative locks and permissions do not exclude hostile same-user cache writers; a hostile Unix descendant can deliberately escape its process group; SSH retains an external configuration surface; and the subprocess still lacks an OS sandbox and CPU/memory/process/transfer ceilings. Retain the snapshot work behind a future isolated helper and resolver receipt. |
| `src/resolver.rs` source-cache policy records | Rewrite | Limits and rejection receipts are useful diagnostics. Their bounded canonical persistence now rejects symlink/non-regular reads and existing destinations, uses exclusive synchronized same-directory staging plus no-overwrite atomic publication, and synchronizes the checked parent on Unix. The schema still describes an unhardened resolver through free strings and mutable cache paths. A future sealed resolver receipt must bind adapter kind, lineage, immutable resolution, snapshot/content identity, policy, and resolver/tool identity. Parsed records never authorize source. |
| `src/identity.rs` and `src/package_source.rs` | Retain | Typed `PackageName`, `PackageKey`, conservative source lineage, and immutable source resolution exist. Git/external-local custody binds declarations from immutable snapshots to `PackageKey` without pretending toolchain/compiler evidence exists. `PackageKey` derives a domain-separated opaque identity carried by target-neutral Psi and managed source metadata; full lineage policy remains in the package layer. The caller-constructible placeholder `PackageInstance` was removed; its replacement must come only from sealed compiler evidence. |
| `src/declaration.rs` | Retain | Hermetic project-kind extraction uses the ordinary lexer/parser over immutable root `build.omg`, requires direct canonical `builder.package`, `builder.application`, or workspace `builder.member` statements, and grants no build execution or caller-supplied package identity authority. |
| `src/dependency_projection.rs` and `src/dependency_edit.rs` | Retain | One same-tree projection joins the authoritative package/application/workspace role to canonical literal Path/Git requests and rejects hidden, computed, malformed, unsupported, or role-less shapes without executing build code. Immutable package source consumes that combined result rather than parsing identity and dependencies separately. The editor rewrites only validated canonical direct rows against the expected file digest; it rejects role-less files, and ambiguous/noncanonical placement produces a non-mutating generated patch. Atomic project mutation remains future command work. |
| `src/graph.rs`, `src/closure_resolution.rs`, and `src/source_adapter.rs` | Retain | The typed graph and transport-neutral resolver close exact-key source topology under bounded package/request/depth limits, preserve requester-local aliases and complete conflict paths, and reject open, unreachable, cyclic, or conflicting resolution. Workspace, Git, and explicit external-local adapters supply immutable custody without ambient discovery. These values remain pre-admission and have no lock or persistence authority. |
| `src/compiler_handoff.rs` and `src/compiler_review.rs` | Retain | A validated custody closure is translated into compiler package inputs and compiled dependency-first, re-rooting each package over exactly its reachable subgraph. Review runs use fresh disposable sponsored sessions, revalidate source custody and compiler-consumed bytes, and return non-constructible compiler-issued review rows only after cleanup. Producer provenance and complete admission projection remain unfinished, so these rows cannot mint `PackageInstance`. |
| `src/capability_conflict.rs` | Retain | Review-only comparison operates on complete compiler-owned canonical rows, exact package/resolution/source/compiler commitments, bounded dependency paths, and a complete candidate-closure commitment. Commitment v2 binds every candidate package's target, compiler, source-consumption, build-observation, and whole-review evidence even when the package has no blocking conflict. It emits row-specific conflicts and fixed-vocabulary summaries. |
| `src/conflict.rs` | Retain | Root policy can accept or reject every exact blocking fingerprint in one canonical candidate-bound decision set. Empty, incomplete, duplicate, stale/foreign, wrong-candidate, and non-blocking decisions reject. Its bounded fixed-vocabulary text record recovers only by matching current compiler conflicts, rerunning complete resolution, and requiring canonical re-encoding. Trusted command orchestration supplies an already-open root-owned policy-directory capability; one lowercase portable filename, a no-follow leaf, retained read handle, post-recovery byte reread, and identity check confine the selected direct child without dependency discovery. The library cannot independently prove the caller's root-directory choice or linearize observations against a hostile process with the root author's filesystem credentials; final command locking and revalidation own that boundary. The result cannot prove review, mint evidence, authorize admission, or promote accepted lock state. |
| `src/record_file.rs` | Retain | Two private persistence lanes remain distinct. Non-authoritative resolver diagnostics retain bounded canonical pathname reads and exclusive synchronized same-directory staging with no-overwrite hard-link publication and Unix parent synchronization. Root-policy state instead accepts an already-open directory capability and one direct-child filename. It writes, synchronizes, rereads, and verifies a private same-directory stage before atomic no-overwrite hard-link publication; directory synchronization follows. Reads retain the file, reread bytes after semantic recovery, and recheck live name identity. A post-publication failure is explicitly `published but unconfirmed`; complete canonical bytes may remain recoverable. No generic filesystem authority API is exported. |
| `src/review_evidence.rs`, `src/review_closure.rs`, and `src/review_baseline.rs` | Retain | Internal traits preserve compiler-issued/recovered custody distinctions. The bounded binary baseline capsule stores exact review-only graph and row material for restart-stable comparison and strictly revalidates framing, closure, checksums, ordering, and canonical re-encoding. It cannot construct accepted evidence or authorize source. |
| `src/source_patch.rs`, `src/source_review.rs`, and `src/source_triage.rs` | Retain | Exact resolver snapshots feed bounded deterministic source patches and fixed-vocabulary compiler triage. Initial admission, source changes, unavailable old source, dangerous retained authority, representation TCB, capability drift, and provenance replacement remain distinct. Model output is advisory and no reviewer prose can become admission evidence. |
| removed `src/manifest.rs` | Deleted | Caller-authored, name-keyed `PackageCapabilityManifest` values and standalone JSON parsing were removed after compiler-issued canonical rows replaced every corrected review consumer. |
| removed `src/diff.rs` | Deleted | Rust-`Debug` section diffs were removed. Production review uses bounded exact-row `capability_conflict.rs` plus candidate-bound canonically recoverable decisions and directory-capability file custody. Binding that capability to the actual command invocation root remains command integration, not a reason to retain the old comparison model. |
| removed `src/review.rs` | Deleted | Reusable free-form whole-section receipts were removed. Their eventual replacement is row-specific root-project policy bound to exact candidate state, without claiming proof of audit. |
| removed `src/lock.rs` | Deleted | The name-keyed fingerprint-only lock and its weak persistence path were removed. Useful closure validation already lives in typed `graph.rs`; accepted lock persistence must be built from sealed evidence rather than adapting schema v1. |
| removed `src/audit.rs` | Deleted | Name-keyed manifest joins and unrestricted evidence strings were removed. Corrected shortest-path conflict explanation and fixed-vocabulary source triage remain in production modules. |
| removed `src/install.rs` | Deleted | The fabricated-manifest install planner was removed. Only the future transaction rule survives in governing documentation: resolve, compile, admit, then atomically edit `build.omg` and lock. |
| removed `src/update.rs` | Deleted | The target-only update planner that could overlook transitive changes was removed. Replacement work is graph-wide baseline comparison, source triage, and exact row resolution. |
| `src/source_commands.rs` | Rewrite | Diagnostic rendering is useful. Explicit `local`/`git` adapter selection has replaced locator guessing, but execution still uses the unhardened resolver. Keep marked unhardened until the remaining resolver P0 work lands. |
| removed `src/commands.rs` | Deleted | The test-only manifest/lock/plan/review command facade was removed after production routing had already been quarantined. Source diagnostics remain separately in `source_commands.rs`. |
| `src/lib.rs` | Rewrite | Superseded manifest, lock, receipt, install, update, diff, command, and graph-audit modules are deleted. The arbitrary `PackageInstance`/compiler-fingerprint constructor was removed. Production orchestration must instead receive opaque compiler/resolver evidence; the crate remains experimental until those replacements exist. |
| `apps/omega-cli/src/main.rs` package command routing | Deleted/quarantined | Direct install/update and manifest-based audit, review, plan, and lock names fail closed at dispatch before compiler parsing, resolution, or mutation. Their unreachable implementation bodies and argument parsers are deleted; only the rejecting name gates remain until corrected commands exist. Ordinary `.omg` filenames using those words remain compiler inputs. |

## Trust-path findings

| Required area | Current evidence | Ruling |
| --- | --- | --- |
| Source/cache process isolation | `source.rs` seals Git configuration, validates resolver-owned cache state, serializes resolver access, checks submodule policy before materialization, and consumes Git and local requests through validated immutable snapshots. It still invokes an unsandboxed Git/transport process and trusts same-user cache custody between checks. | Partial only; hostile-process custody, resource ceilings, a hardened execution boundary, clean local-source policy, and a resolver receipt remain P0. |
| Identity | Typed source-derived `PackageKey`, immutable resolution, source graph, and an opaque compiler-visible key commitment exist; managed source same-package checks use the commitment instead of path spelling. Managed authored symbols and provider-plan/trust rows retain exact realizing-machine, provider-type, service-schema, and requirement-owner package identities. Post-resolution compiler symbols require an existing derivation origin and inherit its exact package/toolchain provenance; truly source-free symbols remain unresolved. No release API can fabricate the future accepted `PackageInstance`; the legacy name/free-string scaffolding is deleted. | Continue through provider binding/selection identities, remaining boundary identities, exact toolchain identity, and the sealed checked-semantic admission projection; build the accepted lock/admission graph from corrected types. Terminal Psi remains a separate requirement for final-realization evidence. |
| Manifests/evidence | The caller-authored manifest structs and JSON parsers are deleted. Compiler-issued review rows remain non-admitting until sealed evidence exists. | No production parser or constructor may accept dependency-authored capability claims as evidence. |
| Locks/persistence | The schema-v1 lock and its predictable temporary-file path are deleted. No accepted lock writer exists. | Build persistence from corrected sealed evidence with exclusive staging and durability; reject legacy lock bytes at the future production boundary. |
| Install/update plans | The caller-selected name/alias/manifest/receipt planners are deleted. | Rebuild only after identity/evidence/lock foundations. |
| Graph audit | The old name-keyed graph report is deleted. Typed closure validation and deterministic exact-key dependency paths remain in corrected modules. | Build future audit output from typed custody and compiler evidence only. |
| Review receipts | The whole-section free-form receipt model is deleted. | Replace with exact row-resolution decisions, without presenting reviewer strings, signatures, or state labels as proof of audit. |
| CLI exposure | At audit start, warnings existed but mutating lock/receipt commands remained discoverable. | The manifest/lock/plan/review routes and their test-only facade are deleted; source diagnostics remain explicitly unhardened until resolver P0 closes. |
| Persistence | Manifest, receipt, and lock writers with predictable temporary names are deleted. The diagnostic resolver-record writer now has bounded canonical recovery, canonical-parent checks, exclusive synchronized staging, no-overwrite atomic publication, and Unix parent synchronization, but remains non-authoritative schema-v3 scaffolding. No accepted persistence writer exists. | Build accepted persistence from sealed evidence under the same transaction floor; do not promote the diagnostic record. |

## Test provenance

The synthetic legacy manifest/lock/install/update unit-test model is deleted.
The integration that fabricated manifests from fixture intent had already been
removed. Local fixture
integration now resolves immutable custody and regenerates compiler review for
every package in each tested closure, including provider-selection and lineage-
spoof canaries; these rows remain review-only rather than sealed admission.
`tests/remote_fixtures.rs` always validates checked-in pins and declarations,
while byte-for-byte private-remote parity is an explicitly ignored live-network
test when CathedralOS SSH credentials are unavailable. End-to-end accepted-lock
tests must eventually consume sealed evidence and reject every caller-supplied
manifest or identity path.

## Production fence

Until every P0 item is complete:

1. `omega install` and `omega update` do not mutate project state.
2. Standalone manifest, receipt, and legacy lock files are never accepted as
   compiler/resolver evidence.
3. Legacy manifest/lock/review structs and parsers remain deleted; no test or
   release-library path can construct or parse them.
4. New implementation work begins at resolver, declaration, identity, and
   sealed compiler-evidence boundaries; it does not extend the legacy schemas.
