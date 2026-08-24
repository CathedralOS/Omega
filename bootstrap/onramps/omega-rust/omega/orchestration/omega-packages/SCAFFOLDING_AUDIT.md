# Omega package scaffolding audit

Status: complete inventory, 2026-08-23. This audit classifies the current Rust
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
| `src/source.rs` Git resolution | Rewrite | Cache keys are full and policy-versioned; staged entries bind exact resolver metadata, origin, and a strict local config; resolver access is locked; Git configuration is sealed; and submodule declarations/gitlinks reject before materialization. Validated tree/blob objects are copied into an atomically published read-only snapshot without checkout, filters, hooks, submodules, or package execution, and the snapshot is re-hashed before reuse. Git subprocesses have null stdin, bounded concurrent stdout/stderr capture, and deadlines. Cooperative locks and permissions do not exclude hostile same-user cache writers; SSH retains an external configuration surface; and the subprocess still lacks an OS sandbox, descendant containment, and CPU/memory/process/transfer ceilings. Retain the snapshot work behind a future isolated helper and resolver receipt. |
| `src/resolver.rs` source-cache policy records | Rewrite | Limits and rejection receipts are useful diagnostics, but the record describes an unhardened resolver and exposes mutable cache paths. A future sealed resolver receipt must bind adapter kind, lineage, immutable resolution, snapshot/content identity, policy, and resolver/tool identity. Parsed records never authorize source. |
| `src/identity.rs` and `src/package_source.rs` | Retain | Typed `PackageName`, `PackageKey`, conservative source lineage, and immutable source resolution exist. Git/external-local custody binds declarations from immutable snapshots to `PackageKey` without pretending toolchain/compiler evidence exists. `PackageKey` derives a domain-separated opaque identity carried by target-neutral Psi and managed source metadata; full lineage policy remains in the package layer. The caller-constructible placeholder `PackageInstance` was removed; its replacement must come only from sealed compiler evidence. |
| `src/dependency_projection.rs` | Retain | The strict extractor reads only the immutable root `build.omg`, recognizes canonical literal Path/Git requests, and rejects hidden, computed, malformed, or unsupported dependency shapes without executing build code. The compiler now has a distinct resolved-graph mode that does not call the transitional scanner. Recursive source traversal and orchestration wiring remain. |
| `src/graph.rs` | Retain | The typed graph validates exact-key source topology, resolution conflicts, aliases, reachability, and v1 acyclicity without persistence or admission claims. The compiler-side handoff independently validates canonical roots and requester-local routing. Recursive dependency projection, translation into compiler inputs, and compiler evidence must populate it before production use. |
| `src/manifest.rs` | Rewrite | `PackageCapabilityManifest`, `SourceIdentity`, and their constructors/JSON parsing are caller-authored security artifacts keyed by name and free strings. The module now compiles only for isolated crate tests and is absent from the release library API; it must be replaced by sealed evidence regenerated by the selected local compiler over `PackageKey` and the future sealed `PackageInstance`. |
| `src/diff.rs` | Rewrite | The responsibility survives, but section fingerprints use Rust `Debug` output and deltas compare coarse sections or aggregate counts without checked-row identity/provenance. Replace with canonical row-specific conflicts over compiler evidence. |
| `src/review.rs` | Delete/quarantine | Free-form reviewer/reason receipts approve manifest sections constructed by callers. Replace with row-specific root-policy decisions bound to candidate, toolchain provenance, evidence, conflict, and every resolved row. Such a record authorizes the selected project state; it does not prove an audit occurred. |
| `src/lock.rs` | Rewrite | Closure checks are useful. The schema is name-keyed, trusts caller manifests, stores only manifest fingerprints, and lacks `PackageKey`, `PackageInstance`, normalized accepted evidence, provenance, observations, and exact conflict resolutions. Temporary persistence uses predictable non-exclusive names and does not synchronize file/directory state, so even its atomic-replace shape requires hardening. No current lock is an accepted lock. |
| `src/audit.rs` | Rewrite | Deterministic graph/path rendering and reachability traversal are useful. Inputs and joins are name-keyed caller manifests, only one discovered dependency path is retained per package, and unrestricted evidence strings are rendered directly. Port the algorithms only after the lock/evidence rewrite and retain every requesting path. |
| `src/install.rs` | Delete/rewrite | The plan requires caller-supplied alias and package identity and assembles candidate state from fabricated manifests. Preserve only the future transaction rule: resolve, compile, admit, then atomically edit `build.omg` and lock. |
| `src/update.rs` | Delete/rewrite | Update decisions trust the superseded manifest/receipt model and can admit source-only changes without mandatory provenance triage. Worse, the plan compares only the named target package and then assembles the entire candidate closure, so an untargeted transitive package can change without old/new admission comparison. Rebuild from graph-wide accepted lock baselines, compiler evidence, source triage, and exact row resolutions. |
| `src/commands.rs` source audit | Rewrite | Diagnostic rendering is useful. Explicit `local`/`git` adapter selection has replaced locator guessing, but execution still uses the unhardened resolver. Keep marked unhardened until the remaining resolver P0 work lands. |
| `src/commands.rs` manifest/lock/plan/review commands | Delete/quarantine | These expose standalone manifest JSON, lock assembly, caller package names/aliases, and free-form receipts. Remove them from the production CLI; library-only tests may retain isolated legacy values while replacements are built. |
| `src/lib.rs` | Rewrite | Superseded manifest, lock, receipt, install, update, and graph-audit constructors are no longer exported and their modules compile only in crate tests. The arbitrary `PackageInstance`/compiler-fingerprint constructor was removed. Production orchestration must instead receive opaque compiler/resolver evidence; the crate remains experimental until those replacements exist. |
| `apps/omega-cli/src/main.rs` package command routing | Delete/quarantine | Warning text is insufficient because standalone JSON can still produce a file named `omega.lock` or a reusable receipt. Remove manifest-based audit, review, plan, and lock routing until locally regenerated compiler evidence exists. |

## Trust-path findings

| Required area | Current evidence | Ruling |
| --- | --- | --- |
| Source/cache process isolation | `source.rs` seals Git configuration, validates resolver-owned cache state, serializes resolver access, checks submodule policy before materialization, and consumes Git and local requests through validated immutable snapshots. It still invokes an unsandboxed Git/transport process and trusts same-user cache custody between checks. | Partial only; hostile-process custody, resource ceilings, a hardened execution boundary, clean local-source policy, and a resolver receipt remain P0. |
| Identity | Typed source-derived `PackageKey`, immutable resolution, source graph, and an opaque compiler-visible key commitment exist; managed source same-package checks use the commitment instead of path spelling. Managed authored symbols and provider-plan/trust rows retain exact realizing-machine, provider-type, service-schema, and requirement-owner package identities. Post-resolution compiler symbols require an existing derivation origin and inherit its exact package/toolchain provenance; truly source-free symbols remain unresolved. No release API can fabricate the future accepted `PackageInstance`; legacy manifests and locks remain test-only name/free-string scaffolding. | Continue through provider binding/selection identities, remaining boundary identities, exact toolchain identity, and the sealed checked-semantic admission projection; replace the legacy lock/admission graph rather than adapting its strings. Terminal Psi remains a separate requirement for final-realization evidence. |
| Manifests/evidence | Public structs and JSON parsers construct capability claims without compiler custody. | No production parser or constructor may accept these as evidence. |
| Locks/persistence | Atomic temporary-file replacement is useful, but schema v1 is not an accepted lock and fingerprints do not retain baselines. | Rewrite schema; reject legacy locks at the future production boundary. |
| Install/update plans | Plans operate on caller-selected names, aliases, manifests, and receipts. | Delete as workflow; rebuild after identity/evidence/lock foundations. |
| Graph audit | Closure and dependency-path algorithms are deterministic but name-keyed. | Port after identity rewrite; current reports are diagnostic only. |
| Review receipts | Whole-section free-form receipts are candidate-insufficient and reusable in the wrong trust model. | Delete; replace with exact row-resolution decisions, without presenting reviewer strings, signatures, or state labels as proof of audit. |
| CLI exposure | At audit start, warnings existed but mutating lock/receipt commands remained discoverable. | The manifest/lock/plan/review routes are now quarantined before parsing or writes; source diagnostics remain explicitly unhardened until resolver P0 closes. |
| Persistence | Manifest, receipt, lock, and resolver records use predictable temporary names and rename without exclusive creation or durability synchronization. | Rebuild persistence around destination containment, exclusive staging, validated canonical bytes, file synchronization, atomic publication, and parent-directory synchronization where required. |

## Test provenance

The current unit tests establish behavior of exploratory data structures only.
The integration test that fabricated manifests from fixture intent has been
removed. `tests/remote_fixtures.rs` validates source pins, immutable custody,
declared package identity, and local correspondence but does not compile
locally regenerated package evidence. It does not satisfy package-admission
acceptance. End-to-end tests must eventually compile each fixture, consume
sealed evidence, and reject any path that supplies manifest or identity values
directly.

## Production fence

Until every P0 item is complete:

1. `omega install` and `omega update` do not mutate project state.
2. Standalone manifest, receipt, and legacy lock files are never accepted as
   compiler/resolver evidence.
3. Legacy manifest/lock/review structs compile only inside isolated crate tests;
   no release-library path can construct or parse them.
4. New implementation work begins at resolver, declaration, identity, and
   sealed compiler-evidence boundaries; it does not extend the legacy schemas.
