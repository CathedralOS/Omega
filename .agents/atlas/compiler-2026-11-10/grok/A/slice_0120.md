# slice_0120

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0921 | omega-rust/omega/packages/package-manager/src/resolution/graph/accepted.rs:57-74 | REACHABLE | Catalog fallback `find` matches lineage, navigation, and ordinal but not purpose, then can return `Ok` without comparing catalog row purpose to the issued purpose. |
| A0922 | omega-rust/omega/packages/package-manager/src/resolution/graph/mod.rs:60-65 | UNDECIDED | Release builds skip lineage pairing except `debug_assert!`; I did not find a normal acquisition path that supplies mismatched key/resolution, and I did not exhaust every `from_resolved_parts` site. |
| A0923 | omega-rust/omega/packages/package-manager/src/resolution/graph/subject/catalog.rs:31-35 | CLOSED | `WorkspaceMemberObservation::new` forces `selections` and `dependency_graphs` to the same length before catalog projection zips them. |
| A0924 | omega-rust/omega/packages/package-manager/src/review/candidate/review_cache.rs:197-200 | REACHABLE | `decode_record` checks only an unkeyed payload SHA-256 trailer; cache hits trust stored `build_handoff` and policy bytes after lineage/resolution field match, without recomputing the handoff. |
| A0925 | omega-rust/omega/packages/package-manager/src/review/candidate/error.rs:184 | CLOSED | Panic paths abort compilation; they do not turn a rejecting check into acceptance (assembly slice indexing is roster-sized). |
| A0926 | omega-rust/omega/packages/resolver-execution/src/resolver_execution/path_custody.rs:77-81 | REACHABLE | When a package-controlled root fails `canonicalize`, only lexical `starts_with` is used while the executable path is canonical. |
| A0927 | omega-rust/omega/packages/review-evidence/src/capture/authority.rs:111 | REACHABLE | Core services absent from `CORE_CRITICAL_SERVICES` and the aggregate-binding branches fall through to `Ok(None)`, so review treats them as non-dangerous ordinary services. |
| A0928 | omega-rust/omega/packages/review-evidence/src/capture/api/data/evidence.rs:154-155 | REACHABLE | Contexts or symbol sets whose fact span is missing and that are not tied to a data symbol are dropped from rechecked evidence instead of failing the check. |

## A0921

Digest preamble (area_06): `PM` is `omega-rust/omega/packages/package-manager/src` (read `digests/area_06.md:8-17`).

Cited site matches at `accepted.rs:57-74` (read). Enclosing function `admit_locked_issued_source_request` (read `accepted.rs:42-87`).

Search: `rg -w 'admit_locked_issued_source_request' omega-rust` — 5 lines, 1 production call at `resolution/source/acquisition.rs:187` (read); definition and re-export only elsewhere.

Trace (read): When no execution-graph owner matches (`matching.next()` is `None` at `accepted.rs:56`), catalog lookup uses `catalog_dependencies().iter().find(...)` keyed on lineage, navigation, and ordinal only (`accepted.rs:60-64`). It does not require `dependency.purpose == purpose`. On match, `purpose != DependencyPurpose::Build` is the only purpose gate (`accepted.rs:66-74`); a `Build` issuance can pair with the first catalog row that shares lineage/navigation/ordinal even if that row’s purpose differs, when resolution and `request_matches` still agree. A subject can legally hold two catalog rows differing only in purpose (`catalog.rs:278-300` test, read). Input: locked `CanonicalSourceClosureSubject` with such duplicate catalog rows, catalog-only requester (no execution graph owner), and `BuildSourceAcquisitions::issue_build_source_request` with `locked_source` set (`acquisition.rs:186-187`, read) for the `Build` purpose at that ordinal.

Verdict: REACHABLE — wrong-purpose catalog row can be admitted for a `Build` issuance.

## A0922

Cited sites match at `mod.rs:60-65` and `resolution/source/custody.rs:101` (read). Enclosing `ResolvedSourceIdentity::from_validated_parts` and `PackageSourceCustody::from_resolved_parts` (read `mod.rs:48-66`, `custody.rs:87-116`). `new` enforces `matches_lineage` in release (`mod.rs:52-55`, read); `from_validated_parts` / `from_resolved_parts` use `debug_assert!` only (`mod.rs:64`, `custody.rs:101`).

Search: `rg -w 'from_validated_parts' omega-rust` — count 3 (1 definition, 2 call sites: `reconcile/resolution.rs:245`, `subject/catalog.rs:46`, read).

Search: `rg -w 'from_resolved_parts' omega-rust` — production uses include `git/recorded.rs:90` after explicit lineage/resolution checks (`recorded.rs:78-85`, read) and `acquisition.rs:558` pairing `self.lineage` / `self.resolution` (read). Finish path builds graph nodes via `from_validated_parts` (`resolution.rs:245-248`, read).

What I could not close: whether any release production path can place a lineage/resolution mismatch into `accepted` before `from_validated_parts`, without relying on `debug_assert!` firing.

Verdict: UNDECIDED.

## A0923

Cited `catalog.rs:31-35` (read). Enclosing `project` (read `catalog.rs:20-85`).

Zip pairs `observation.selections` with `observation.dependency_graphs()` from custody selection evidence (read). `WorkspaceMemberObservation::new` rejects `result.demanded_sources().len() != dependencies.len()` (`workspace_acquisition.rs:40-47`, read); observations fed to catalog projection are built through `new` from build execution (`build-system/orchestration/.../readiness.rs:661`, inferred from grep only — not opened).

Verdict: CLOSED — length mismatch cannot reach the zip on the constructed observation path enforced by `new`.

## A0924

Cited `review_cache.rs:197-200` (read). `store` appends `content_identity(&payload)` as trailer (`review_cache.rs:197-200`, read). `decode_record` verifies last 32 bytes against payload only (`review_cache/record_codec.rs:855-858`, read); no binding to `compiled_review_key` filename.

Cache hit path: `occurrences.rs:559-569` calls `lookup` then `into_review_and_facts` (read). `into_review_and_facts` checks package identity, name, lineage digest, resolution digest (`review_cache.rs:100-105`, read) and recovers policy from stored bytes (`review_cache.rs:108`, read); it copies stored `build_handoff` (`review_cache.rs:123`, read). `verify_current_source_consumption` re-derives only the source-consumption commitment, not build handoff (`occurrences.rs:121-137`, read). `decode_record` checks handoff against embedded observation summary (`record_codec.rs:892-899`, read) but not against live compilation inputs.

Input reaching acceptance: a file at the expected cache path (keyed lookup) whose payload has a valid trailer and embedded lineage/resolution digests matching the live package, but altered policy or handoff content consistent with decode’s internal cross-checks.

Verdict: REACHABLE for a writable compiled-review cache directory.

## A0925

Cited `error.rs:184` and `compare/assembly.rs:73-75` (read). `render_package_diagnostics` slices `name[..shown_name]` (`error.rs:180-184`, read) — display path on compile failure, not acceptance. `assembly.rs:73-76` slices `accepted.occurrences()` using counts from `PackageOccurrenceRoster::purposes` (`assembly.rs:64-76`, read); roster comes from `PackageOccurrenceRoster::derive(source)` on the same subjects (`assembly.rs:28-31`, read).

These paths panic or format errors; they do not return success on invalid input.

Verdict: CLOSED for ACCEPT (no wrongful acceptance).

## A0926

Cited `path_custody.rs:77` (read). Enclosing `require_outside_roots` (`path_custody.rs:68-93`, read). `ResolverExecutionBackend::open` calls it for the authored path and again for the canonical executable (`resolver_execution.rs:40-49`, read). For each root, `canonical_root = root.canonicalize().ok()` (`path_custody.rs:77`, read); if `None`, only `path.starts_with(root)` applies (`path_custody.rs:78-81`, read). The executable itself is canonicalized before storage (`resolver_execution.rs:41-51`, read).

Production caller: `source-acquisition/.../executor.rs:155` `ResolverExecutionBackend::open` (read). `ExcludedRoots` uses the same `canonicalize().ok()` pattern for package roots (`selection.rs:177-193`, read).

Input: package-controlled root path that does not canonicalize at open time, while the canonical resolver executable lies under the real directory that root names but not under the lexical prefix.

Verdict: REACHABLE.

## A0927

Cited `authority.rs:111` (read). Enclosing `dangerous_authority_class` (read `authority.rs:42-111`). `CORE_CRITICAL_SERVICES` table (`authority.rs:25-40`, read) covers named `omega::core` modules; aggregate branch only handles `ConsoleExitProcessI32` and `FilesystemHostService` (`authority.rs:84-108`, read). `TimeHostService` appears in package-compilation roles (grep) but not in this authority table (read `authority.rs:84-108` only).

Unlisted core service with valid toolchain provenance: table `find` fails, aggregate branch fails, function returns `Ok(None)` (`authority.rs:111`, read) — no dangerous authority class.

Verdict: REACHABLE — new or unlisted critical service is reviewed as ordinary.

## A0928

Cited `evidence.rs:154-155,181-182` (read). Enclosing `rechecked_data_definition_evidence` (`evidence.rs:93-209`, read). For contexts, missing `facts.refs.span` with `None if at_data_definition => return Some(None)` else `None => return None` (`evidence.rs:152-156`, read) — non-data contexts with missing spans are omitted from the vector. Same pattern for symbol sets (`evidence.rs:179-182`, read).

Caller `require_rederived_data_definition_facts` fails only when `rechecked_...` returns `None` or `retained != expected` (`evidence.rs:73-89`, read). Symmetrically malformed plans on both sides can compare equal while omitting skipped contexts.

Verdict: REACHABLE — skipped non-data context/symbol-set spans can yield passing equality with incomplete evidence.

## What I did not read

- Full `record_codec.rs` encode path and all policy recovery validators beyond `decode_record` tail (`record_codec.rs:848-934` partially read).
- `PackageOccurrenceRoster::derive` implementation body.
- `build-system/orchestration/.../readiness.rs` around workspace observation construction (only grep hit).
- `facts::FactPlan` invariants beyond `fact_plan_arena_links_are_well_formed` header (`evidence.rs:211+` not read).
- `git show 457adf4a11f:` paths (current checkout lines matched cited symbols without relocation).

SLICE COMPLETE
