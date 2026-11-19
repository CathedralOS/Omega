# Unit 34 notes

Unit file: units/unit_034.tsv (122 rows, 21,881 lines). All paths relative to
`<checkout>/`. Short prefixes used below:

- `PM` = `omega-rust/omega/packages/package-manager/src`
- `RE` = `omega-rust/omega/packages/review-evidence/src`
- `RX` = `omega-rust/omega/packages/resolver-execution/src`

Files covered (in unit order):
PM/package_manager/proposal/framing.rs; PM/package_manager/source_review.rs; PM/package_manager/source_review/old_sources.rs;
PM/package_manager/state.rs; PM/resolution/compiler_input.rs; PM/resolution/graph/{accepted,git_pins,mod,root_request}.rs;
PM/resolution/graph/reconcile/{dependency_paths,mod,resolution,resolved_closure}.rs and resolution/tests.rs;
PM/resolution/graph/subject.rs and subject/{catalog,construction,request_view,usage}.rs; subject/encoding/{framing,mod,selection,source}.rs;
subject/text/{framing,mod,record,requests,source,values}.rs; subject/validation/{closure,dependency,mod,root,source,walk}.rs;
PM/resolution/mod.rs; PM/resolution/source/{acquisition,build_resolution,custody,local,mod,selection,workspace_acquisition,workspace_path}.rs;
source/git/{mod,recorded,request,workspace}.rs; source/local/recovery.rs and recovery/tests.rs;
PM/review/audit/{mod,snapshot}.rs; PM/review/candidate/{custody,error,mod,passes,review_cache,review_set,semantic_bindings}.rs;
candidate/passes/occurrences.rs; candidate/review_cache/{key,record_codec}.rs; PM/review/compare/{assembly,fingerprints,limits,merge,mod,paths,projection,replacements}.rs;
PM/review/decision/{document,mod}.rs; PM/review/{mod,restricted_build_grants,timings}.rs;
RX/{lib,resolver_execution}.rs, RX/resolver_execution/path_custody.rs, RX/resolver_execution/tests/{mod,unix,validation}.rs;
RE/capture.rs; RE/capture/api.rs and api/{conformances,constants,data,domains,operators,policy,propositions,slot_metadata,traits}.rs;
api/data/{evidence,projection}.rs; api/domains/{aliases,facts,projection}.rs and domains/facts/checked_rows_tests.rs; api/policy/{declarations,traits}.rs;
RE/capture/authority.rs; RE/capture/behavior.rs; behavior/crash/{expressions,mod,permissions}.rs; behavior/{invocations,reach,termination}.rs;
behavior/policy/{crash,flow,mod,mutation,receivers,termination}.rs and policy/flow/row_tests.rs;
RE/capture/callables.rs; callables/{boundary_operators,boundary_requirements,conformance_order,conformances,external_policy}.rs; callables/external_policy/bindings.rs.

Method note: every line of every file was read in full (Read tool). Greps were run with the Grep tool across `omega-rust`
(counts below say which pattern). Nothing was built or run.

---------------------------------------------------------------------------------------------------

## Part A. package-manager: proposal / source-review / state (4 files)

### PM/package_manager/proposal/framing.rs (129 lines)
1. Purpose: length-plus-LF ASCII framing for the pending package proposal file (write side and recovery side).
2. Shape: `Writer{output,maximum}` (6-48) with `append` (19), `row` (35), `section` (39); `Reader{remaining}` (50-121) with `line` (59), `expect` (68), `field` (75), `count` (86), `section` (103), `finish` (115); `write_digest`/`read_digest` (123-129).
3. I/O: pure strings. `read_digest` calls `crate::hex_digest`; `write_digest` uses `crate::Hex`.
4. Decides: byte budget (`total > self.maximum`, 24); count canonicality: <=20 bytes, no leading zero, all ASCII digits (88-93), then `count > maximum` (97). Label equality and non-empty value (80).
5. Cases by shape: none.
6. Same job elsewhere: three other `Writer`/`Reader` framing pairs exist (grep `struct (Writer|Reader)\b` in `omega/packages`): `PM/lock/text/framing.rs:6,66`, `PM/resolution/graph/subject/text/framing.rs:8,118` (read here), plus review-evidence `encoding/recovery/policy/text/framing.rs:31`, `.../reader.rs:6`, `.../text/writer.rs:14` (suspected, not read). The subject text framing quotes bytes; this one uses a length prefix.
7. Unused/one-caller: all items are `pub(super)`; not grepped for call counts (inferred used by the proposal encoder/decoder next to it, not read).
8. Stated limits: none.
9. Fail-open: none seen. `count` rejects noncanonical input.

### PM/package_manager/source_review.rs (112 lines)
1. Purpose: builds the `source-diff.txt` diagnostic (hostile source data, kept apart from the editable review doc) by diffing each candidate package source against a recovered baseline.
2. Shape: one fn `prepare` (18-112); consts `SOURCE_DOCUMENT` (15), `MAXIMUM_OUTPUT_BYTES = 8 MiB` (16).
3. I/O: reads `accepted: Option<&PackageLock>`, `candidate: &ResolvedPackageSourceClosure`, `storage`, `acquisition`; calls `old_sources::recover` (41) and `render_package_source_patch` (57); writes via `state::write` (98); returns a report string.
4. Decides: baseline pairing is keyed on `PackageKey` equality: `subject.packages().find(|source| source.key() == custody.key())` (33-38). Takes only `lock.targets()[0]` as the accepted subject (26) (index 0, no check that other targets agree). Output-size gate: `document.len().saturating_add(patch.len()) <= MAXIMUM_OUTPUT_BYTES` (60). Error text classification by variant (78-84): `SourceCustody|SourceSelectionCustody` => "source custody could not be verified"; `PackageKeyMismatch`; everything else (`_`) => "resource limit exceeded" (83).
5. Cases by shape: `targets()[0]` (26) assumes the first lock target stands for all.
6. Same job elsewhere: `audit/mod.rs:40` has its own `MAXIMUM_OUTPUT_BYTES = 32 MiB` per patch; this file's 8 MiB is a second, separate aggregate cap on the same document (different values, same name).
7. Unused: n/a (`pub(super)`).
8. Stated limits: "obtain the exact sources for standalone audit" (87) - fallback text when a patch cannot be rendered.
9. Fail-open: when the baseline cannot be recovered (`Err(reason)`, 51) it silently continues with `baseline = None` and renders a standalone candidate patch (56-57), noting "Source diff unavailable" in the report (51-53). Reaches it: any old-source recovery failure. Kind: "stated" (text at 96: "Capability comparison uses accepted lock policy independently of old-source availability."). This file only writes diagnostics, so no acceptance decision is made here (inferred; the consumer is not read).

### PM/package_manager/source_review/old_sources.rs (109 lines)
1. Purpose: recover the baseline `PackageSourceCustody` for an accepted package so a source diff can be rendered; reuses current custody when identical, else recovers from the local cache or by exact Git pin.
2. Shape: `recover` (16-109).
3. I/O: reads `CanonicalSourceClosureSubject` (accepted), `ResolvedSourceIdentity` (expected), the candidate closure, `SourceResolver`; calls `recover_cached_external_local_source` (42) and `recover_recorded_git_source` (83). Returns `Result<PackageSourceCustody,&'static str>`.
4. Decides: (a) reuse `current` when `current.resolution()==expected.resolution()` and navigation equal (27-31); (b) match on `expected.resolution()` variant: `ExternalLocal` => cache recovery (33-43), `Git` => find Git request by **source lineage equality** `edge.selected().key().source_lineage() == expected.key().source_lineage()` (56) across `subject.dependency_requests()`; the first matching edge wins (`find_map`, 55); any other variant (Workspace) => `Err("accepted local snapshot is missing from the available cache")` (97-99). (c) Final check that key, resolution, navigation equal expected (102-107).
5. Cases by shape: "role" is `subject.root_role()` only if root key matches, else `Package` (37-41, 78-82). The Git branch requires a recorded `CanonicalDependencySourceRequest::Git` edge; the root's own `CanonicalRootSourceRequest::Git` is not consulted (60-67), so a Git root with no dependency edge to its own lineage fails with "no recorded Git acquisition request is available" (69). Read.
6. Same job elsewhere: the role-selection `if subject.root().selected().key()==expected.key() {root_role} else {Package}` appears twice in this file (37-41, 78-82) and the same shape in `PM/resolution/compiler_input.rs:122-126`.
7. Unused: n/a.
8. Stated limits: header comment (2): "recovered ... rather than inferring package identity from a name or decoding a lock path."
9. Fail-open: line 24-26 `.expect("candidate package selected by key")` is a panic (not accept) when the caller passes a key absent from the candidate. Line 27-31 returns the *current* custody as the "baseline" when resolution and navigation match: intended (identical sources diff as empty). Kind: stated by structure; not traced further.

### PM/package_manager/state.rs (86 lines)
1. Purpose: retained-root record helpers for `proposal` and `review-<target>.txt` inside `build/package-manager`.
2. Shape: `PROPOSAL` (10), `LIMITS` 128 MiB (11-13), `file_failure` (15), `read` (19), `text` (28), `write` (32), `write_proposal` (51), `review_name` (60), `review_path` (64), `discard` (71).
3. I/O: `RecordFileRoot` `read_optional`/`replace`/`write_new`/`remove`; returns `PackageCommandOutcome` from `discard`.
4. Decides: `write` replaces if the file exists else creates (37-48). `write_proposal` uses `write_new` only (fails if one exists). Keyed on file name string.
5. Cases by shape: none.
6. Same job: `review_path` (64) and `source_review.rs:105-108` both build `project_root/build/package-manager/<name>` by hand-joined string literal `"build/package-manager"`.
7. Unused: not grepped.
8. Stated limits: `discard` message "accepted project files were not rolled back" (77).
9. Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## Part B. package-manager: resolution (compiler_input, graph, subject, source)

### PM/resolution/compiler_input.rs (209 lines)
1. Purpose: `PackageCompilationScope` re-roots an immutable closure at one package and assembles `PackageCompilationInputs` (bindings, dependency bindings, dependency acquisitions) for one compile.
2. Shape: `PackageCompilationScope{closure,root,reachable:BTreeSet<PackageKey>}` (25); `new` (32); `compilation_inputs_reusing` (55-159); `binding_with_canonical_source_metadata` (162); `reachable_package_keys` (186-209).
3. I/O: reads closure custodies/graph; mutates `root_binding: &mut Option<PackageSourceBinding>` as a per-root memo; builds `PackageCompilationInputs::new(...).with_dependency_acquisitions(...)`.
4. Decides: reachability: `Product` edges reach from every package, `Build` edges only from the compilation root (`dependency.purpose().is_product() || is_root`, 204; `requester == root.identity()`, 109). Memo hit keyed on `identity()==` and `source_root()==` (73-76). Source metadata path: captured tree, else checked-source cache dir, else filesystem (171-183): if the cache dir exists but `open_or_create` fails it silently falls through to `with_canonical_source_metadata` (176-183).
5. Cases by shape: root role: `if root == closure.graph().root() { closure.root_role() } else { Package }` (122-126).
6. Same job elsewhere: `reachable_packages` (`PM/resolution/graph/mod.rs:302`, all edges) and `reachable_package_keys` here (purpose-filtered); a third `reachable_packages` at `omega/packages/package-compilation/src/package_compilation/dependency_graph.rs:216` (grepped by fn name, not read); `PM/build_session/order.rs:11 dependency_first_package_order` (grepped, not read).
7. Unused: `PackageCompilationScope::new` callers: `occurrences.rs:221`, `candidate/custody.rs:20`, `build_resolution.rs:252`, `operations/accepted_compilation.rs:548` (grep `PackageCompilationScope::new`: 4 sites). `closure()`/`root()`/`packages()` accessors are used by `candidate/custody.rs:29-31` and `occurrences.rs`.
8. Stated limits: none.
9. Fail-open: (a) `packages` filter: a custody not in `reachable` is silently dropped (65); intended. (b) `reachable_package_keys` `let Some(node) = ... else { continue }` (196-198): a reachable key with no graph node is inserted into `reachable` but contributes no edges. Kind: "open" (closure is validated at construction, `ResolvedPackageClosure::new`, graph/mod.rs:159, so I infer it cannot occur; not traced to all constructors).

### PM/resolution/graph/accepted.rs (285 lines)
1. Purpose: compares executed source graphs to the accepted lock subject without acquiring anything: admits a locked request, an issued request, and checks whether a mutable local root graph still matches.
2. Shape: `admit_locked_source_request` (13), `admit_locked_issued_source_request` (42), `checked_custody` (89), `edge` (120), `checked_request` (141), `local_root_context_matches` (172), `mutable_local_graph_matches` (182), `request_matches` (215), `AcceptedSourceError` (250).
3. I/O: reads `CanonicalSourceClosureSubject` (incl. `execution_source_graphs()` and `catalog_dependencies()`), `PendingSourceResolution`, `SourceAcquisitionOrigin`, `DependencySourceRequest`; returns `Result`.
4. Decides: locate graph containing the requester by **package key** (19-24); locate `requester` edge by binary search on `(requester key, purpose, dependency_index)` (148-155); request equality by field compare of alias/location/repository/revision/selection (215-248). Issued-request path matches packages by `(source_lineage == origin.lineage && navigation == origin.navigation)` (52-55). `mutable_local_graph_matches` compares keys, resolutions (except root, 205), navigations, dependency projections, requests, catalog deps (195-212).
5. Cases by shape: `request_matches` handles Path/Path and Git/Git only; anything else `_ => false` (246). `local_root_context_matches` only matches `ExternalLocal` vs `ExternalLocal` (176-179).
6. Same job elsewhere: key/resolution/navigation/projection equality is repeated in `checked_custody` (95-116), `mutable_local_graph_matches` (199-210), `subject.rs::same_source_graph` (65-84), and `old_sources.rs:102-107`.
7. Unused: grep `mutable_local_graph_matches|admit_build_descent|admit_locked_issued_source_request`: `mutable_local_graph_matches` 1 outside call (`operations/inspect_packages/sources.rs:40`); `admit_locked_issued_source_request` 1 outside call (`acquisition.rs:187`); `admit_locked_source_request` is used at `acquisition.rs:110` (inferred from import at 13). `grep fn admit_locked_source_request`: defined once.
8. Stated limits: none.
9. Fail-open: `admit_locked_issued_source_request` (56-75): if no package in any execution graph matches `(lineage, navigation)`, it falls into the catalog branch and returns `Ok(())` if a catalog dependency matches `observer.lineage`, `navigation`, `ordinal`, AND `purpose == Build`, resolution equal and request equal (66-74). Not blind accept: requires all those. The `Ok(())` at 74 is reached only after those checks. Kind: traced (read in this file). Note the catalog branch compares `ordinal` but takes the first matching `find` (60) without checking `purpose` during the search; purpose is checked afterwards (66), so two catalog entries differing only by purpose would see only the first. Kind: open.

### PM/resolution/graph/git_pins.rs (168 lines)
1. Purpose: policy for reusing accepted Git revisions: unselected repositories keep their recorded pin; repositories named by `updates` refresh; offline mode forbids new Git selections.
2. Shape: `GitResolutionOptions{pins,offline}` (14) with `recorded` (25); `RecordedGitSource` (19); `GitDependencyPins{accepted,updates,acquisition}` (52) with `new` (59), `resolution` (82); `pin_error` (137); `GitDependencyPinsError` (146).
3. I/O: reads accepted `CanonicalSourceClosureSubject`; returns `Option<&ImmutableSourceResolution>`.
4. Decides: updates are keyed by **source lineage**: `package.source_lineage() == request.lineage()` (86-90) => refresh (return `None`). Otherwise a pin is any accepted root/edge whose `(requested_locator, requested_revision)` strings equal the request (93-94, 98-103, 115-120); two different resolutions for the same request string => error (106, 123).
5. Cases by shape: `new` rejects update packages not in any execution graph (65-70) and duplicates (71).
6. Same job elsewhere: `PM/resolution/source/git/recorded.rs` recovers a recorded pin by key/lineage; this file selects the pin by locator/revision string.
7. Unused: `GitDependencyPins::new` callers (grep, 5 sites): `operations/prepare_project.rs:346`, `operations/inspect_packages.rs:196`, `package_manager.rs:280`, `catalog.rs:330` (test). `GitResolutionOptions::recorded` called at `acquisition.rs:304` and `workspace_acquisition.rs:233`.
8. Stated limits: "offline resolution forbids new or refreshed Git selection; an unchanged recorded pin is required" (44).
9. Fail-open: `recorded` returns `Ok(None)` when no pin and not offline (47), meaning "resolve fresh". Intended: stated at line 1 comment. When pins are `None` and `offline` is false everything is fresh.

### PM/resolution/graph/mod.rs (394 lines)
1. Purpose: core graph types (`ResolvedSourceIdentity`, `ResolvedDependency`, `ResolvedPackageNode`, `ResolvedPackageClosure`) and validation (finite, reachable, acyclic, no duplicate aliases, no conflicting resolutions).
2. Shape: `ResolvedSourceIdentity` (43) with `new` (49) checking `matches_lineage`, `from_validated_parts` (60, `debug_assert` only), `index_in` binary search (68), `exact_index_in` (74); `ResolvedPackageClosure::new` (159-243); `PackageClosureValidationError` (268); `reachable_packages` (302); `find_dependency_cycle` (333); `visit_for_cycle` (352, recursive DFS).
3. I/O: pure; `package_indices: BTreeMap<PackageKey,usize>` (155).
4. Decides: duplicate key with equal resolution => `DuplicatePackage`, unequal => `ConflictingResolution` (171-178); alias duplicates keyed by `(purpose, alias)` (187); `Workspace` root role rejected (197); missing target / unreachable / cycle.
5. Cases by shape: none.
6. Same job elsewhere: acyclic/reachable validation is re-implemented for the subject in `subject/validation/walk.rs:11-45` (iterative DFS) and for compile inputs in `package-compilation/.../dependency_graph.rs:216` (grepped by name). `package_indices: BTreeMap` (155) while the file's own comment in AGENTS.md says dense IDs prefer flat tables (observation only).
7. Unused: `ResolvedSourceIdentity::from_validated_parts` uses a `debug_assert` (64) only; in release, an invalid pair is not rejected. Callers: `resolution.rs:245`, `catalog.rs:46`, others not counted.
8. Stated limits: none.
9. Fail-open: `from_validated_parts` (60-66) skips the lineage check in release builds (`debug_assert!` only). What gets through: a key/resolution pair with mismatched lineage when a caller passes unvalidated parts. Kind: open (callers `resolution.rs:245-248` pass custody fields; custody asserts the same in `custody.rs:101` also `debug_assert`).  `reachable_packages`/`visit_for_cycle` skip targets absent from `package_indices` (318, 365) but `MissingDependencyTarget` was already recorded at 205-215, so `Ok` is not returned. Kind: traced.

### PM/resolution/graph/reconcile/dependency_paths.rs (74 lines)
1. Purpose: one BFS predecessor tree over a `ResolvedPackageSourceClosure` to give shortest root-to-package explanation paths in batch.
2. Shape: `DependencyRequestPaths{closure,root_position,predecessors}` (10), `new(closure, stop_at)` (17), `path` (50).
3. I/O: reads `graph.packages()`; produces `DependencyRequestPath`.
4. Decides: first-found edge in authored order wins ties (36 `predecessors[target].is_some()`); `stop_at` ends traversal early (38-40).
5. Cases by shape: none. Does not filter by purpose (all edges, Product and Build).
6. Same job elsewhere: `PM/review/compare/paths.rs:13-142` (`Paths` over `CanonicalSourceClosureSubject`, comment: "first canonical breadth-first dependency path") does the same shortest-path job over the subject; `reconcile/resolution.rs:402-512` (`paths_for_origin`/`collect_paths`) enumerates all paths by DFS for conflicts; `resolved_closure.rs:187 dependency_path` wraps this file.
7. Unused: `DependencyRequestPaths` re-exported at reconcile/mod.rs:24; users in this unit: `resolved_closure.rs:195` only (inferred; not grepped repo-wide).
8. Stated limits: none.
9. Fail-open: `path` returns `None` if a predecessor is missing (56); not an accept.

### PM/resolution/graph/reconcile/mod.rs (223 lines)
1. Purpose: module header, error and limit types for source-closure reconciliation.
2. Shape: `DependencyRequestPathStep` (39), `DependencyRequestPath` (58), `PackageSourceClosureConflict{Candidate}` (74, 82), `PackageSourceClosureLimits{max_packages:1024,max_dependency_requests:16384,max_depth:128}` (90-104), `PackageSourceClosureLimitKind` (107), `PackageSourceClosureResolutionError<E>` (114) with Display (149).
3. I/O: types only.
4. Decides: n/a.
5. Cases by shape: n/a.
6. Same job elsewhere: the numeric limits 1024 / 16*1024 are repeated in `CanonicalSourceClosureSubjectLimits::default` (`subject.rs:327-331`).
7. Unused: three empty inherent impls `impl PackageSourceClosureConflictCandidate {}` (79), `impl PackageSourceClosureConflict {}` (87), `impl<E> PackageSourceClosureResolutionError<E> {}` (147); the conflict structs' fields `custody`/`requesting_paths`/`key`/`candidates` have no accessors in this file (read). Counted by reading only.
8. Stated limits: module comment (1-12): "conflicting custodies for a single `PackageKey`, alias collisions, and exceeded package/request/depth ceilings all fail closed."
9. Fail-open: none seen.

### PM/resolution/graph/reconcile/resolution.rs (512 lines)
1. Purpose: drives the bounded source-closure traversal as a suspend/resume state machine: it issues one `PendingSourceResolution` per authored dependency request, and joins each answer back into the graph.
2. Shape: `SourceClosureTraversal` (54), `RequesterResolution` (77), `PendingSourceResolution` (84) with `complete`/`fail`/`source`/`acquired_source`; `begin_source_closure` (91); `advance` (209, 274); `finish` (228); `join` (308-371); `collect_conflicts` (374); `paths_for_origin` (402); `collect_paths` (458, recursive, `#[allow(clippy::too_many_arguments)]`).
3. I/O: mutates `accepted: BTreeMap<PackageKey,Custody>`, `acquired: BTreeMap<(SourceLineage,Navigation),Custody>`, `observed`, `dependencies`, `depths`, `pending: VecDeque`.
4. Decides: package first-seen wins in `accepted` (355); a second custody for the same key is recorded in `observed` unless `semantically_equivalent` (344-346) and later reported as `ConflictingCustody` if `candidates.len()>1` (382). Role gate: dependency must be `Package` (179). Alias validation per requester and purpose (310-327). Limit checks: dependency request count (285), depth (292), package count (356).
5. Cases by shape: `RequesterResolution::advance` walks `DependencyPurpose::ALL` positionally (276-304). `acquired` reuse is keyed `(lineage, navigation)` (192-196, 145).
6. Same job elsewhere: conflict-path enumeration duplicates `DependencyRequestPaths` (see above).
7. Unused: `PendingSourceResolution::root_request` (133) called by `acquisition.rs:98-102`.
8. Stated limits: header (1-9): "Empty dependency rosters are drained iteratively so a wide frontier never turns into a chain of recursive resumes."
9. Fail-open: `.entry(...).or_insert_with` on `acquired` (192-196) keeps the first custody when a later one has the same `(lineage, navigation)`; a different resolution at the same key is only caught later via `observed`/`ConflictingCustody` and `PendingBuildSource::select` (build_resolution.rs:94-103). Kind: traced.

### PM/resolution/graph/reconcile/resolution/tests.rs (214 lines)
Unit tests of the state machine: suspended request ordering and shared custody (79), limits (100), adapter failure context (142), role/alias/conflict/cycle checks on resume (165). Builds sources from a fixture directory `tests/omega/pass/build/separate_build_product_imports_exit` found via `CARGO_MANIFEST_DIR.ancestors().nth(4)` (11-15). No production logic. Fail-open: n/a.

### PM/resolution/graph/reconcile/resolved_closure.rs (197 lines)
1. Purpose: validated closure value and zero-copy request views.
2. Shape: `ResolvedPackageSourceClosure{root_request,graph,custodies,custody_indices}` (24); `ExactTargetPackageSourceClosure{source_closure,target_profile}` (32); `ResolvedRootPackageSourceRequest` (48); `ResolvedDependencySourceRequest` (64); `ResolvedPackageSourceRequestSet` (100) with `root` (105) and `dependencies` (118); `for_exact_target` (155); `custody` (177); `dependency_path` (187).
3. I/O: reads graph + custodies; `.expect("validated closure contains its root package")` (110) etc.
4. Decides: `dependency_path(target)`: root returns empty path (189); otherwise BFS (195).
5. Cases by shape: n/a.
6. Same job: see dependency_paths.rs.
7. Unused: `for_exact_target` users: `accepted.rs:188`, `catalog.rs:42`, others; `ExactTargetPackageSourceClosure` constructed only here (private fields); it is the type-level distinction between "source custody" and "target identity" (header 3-7), carried by value everywhere as `&ExactTargetPackageSourceClosure`.
8. Stated limits: none.
9. Fail-open: `debug_assert_eq!` on authored counts (125-128) only.

### PM/resolution/graph/root_request.rs (16 lines)
`PackageRootSourceRequest { Git(GitPackageSourceRequest), ExternalLocal{requested_root, source_context} }` (10). Note: no `WorkspaceMember` variant here while `CanonicalRootSourceRequest` has one (`subject.rs:160`); `canonical_root_request` (`validation/root.rs:17-34`) maps only the two, so `WorkspaceMember` is only reachable from text recovery (`text/requests.rs:86`). Read.

### PM/resolution/graph/subject.rs (357 lines)
1. Purpose: defines `CanonicalSourceClosureSubject`, the canonical, recoverable subject for one closure: packages, navigations, dependency projections, edges, catalog deps, canonical bytes and fingerprint; plus request/selection types, limits, error, fingerprint.
2. Shape: struct (52); `same_source_graph` (65); `execution_source_graphs` (90); `CanonicalCatalogDependency` (144); `CanonicalRootSourceRequest{Git,WorkspaceMember,ExternalLocal}` (154); `CanonicalDependencySourceRequest{Path,Git}` (193); `CanonicalDependencySourceSelection` (242); error (278) with `&'static str`; fingerprint (297); limits (316-350) incl. `compiler_bounded` (337); `hex_length` (352).
3. I/O: consts `SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION = 9` (47), `ABSOLUTE_*` limits (309-313).
4. Decides: `same_source_graph` compares everything except target and bytes: root, packages, navigations, projections, requests, and catalog entries including their provider subgraphs (65-84).
5. Cases by shape: encoding version by catalog shape: catalog-free => v7, Build-only catalog => v8, any non-Build purpose => v9 (header 11-15, implemented at `encoding/selection.rs:66-72`). Text versions 1-4 (see text/mod.rs).
6. Same job elsewhere: `same_source_graph` call sites: `compare/mod.rs:193`, `lock/text/reader.rs:82`, `lock/mod.rs:212` (grep: 3 production callers plus 3 uses inside this file's tests/recursion).
7. Unused: n/a.
8. Stated limits: header (7-11): "Snapshot roots, cache paths, ... are intentionally absent."
9. Fail-open: none seen. Note `catalog_dependencies.len()` equality plus zip (71-76) is OK.

### subject/catalog.rs (400 lines)
1. Purpose: projects and validates "catalog" (workspace-member) dependency executions: sources executed for Git workspace members that are not in the Product closure.
2. Shape: `project` (20-85): BFS over closures through `selection_evidence().observations()` collecting `CanonicalCatalogDependency` and sorting by (observer key, purpose, ordinal); `validate` (87-202); tests (204-399).
3. I/O: reads custodies' `selection_evidence`.
4. Decides: duplicates keyed `(observer key, purpose, ordinal)` (56-60); mismatch => error "catalog observations disagree on executed source custody" (62); limits (68, 105-112); canonical order strict (113-124); flat provider required (97-100); "catalog and Product sources disagree on immutable custody" by sorting all sources by key and comparing pairs (192-200).
5. Cases by shape: provider must be `Package` role, same target, with no nested catalogs (97-103): "catalog provider must be a flat closed package source graph at the same target". Read.
6. Same job elsewhere: this file's `validate` re-implements per-provider validation by calling `validate_subject` (170).
7. Unused: n/a.
8. Stated limits: header: "Nested catalog provider closures are flattened ... verified for consistent immutable custody"; test at 355 asserts nested records are refused.
9. Fail-open: `project` uses `zip(observation.selections, observation.dependency_graphs())` (31-35): if the two lengths differ, extra entries are silently dropped by `zip`. Kind: open (not traced into `WorkspaceMemberObservation::new` which ties `demanded_sources().len() == dependencies.len()` at `workspace_acquisition.rs:40-47`, so I infer it matches for observations built by that constructor).

### subject/construction.rs (192 lines)
Builds a subject from a resolved closure and re-validates: `validate_recovery_limits` (20), `from_resolved` (43), `from_resolved_without_catalog` (53), `finish` (121), `with_catalog_dependencies` (166). Sorts packages by key and edges by `(requester, purpose, index)` (70, 103). `with_catalog_dependencies` returns self unchanged when `dependencies.is_empty()` (173) so catalog-free identity stays v7 (stated in subject.rs header). `.expect("validated closure retains every package custody")` at 77, 86. Fail-open: none seen.

### subject/encoding/{framing,mod,selection,source}.rs (125 + 18 + 276 + 125)
- `framing.rs`: `Encoder` with `scratch`/`fixed`/`u16`/`u32`/`count`/`bytes_bounded`; it records failure in a `failed: bool` flag (24, 46-53, 65, 80) rather than returning errors from each push; callers must call `check()`. `fingerprint` (8) = `DomainDigestEncoder` over domain `OMEGA-SOURCE-CLOSURE-SUBJECT-FINGERPRINT\0`.
- `mod.rs`: header comment only (module list).
- `selection.rs`: `encode_subject` (24), version selection 7/8/9 (62-72), root selection tags 0/1/2 (139-183), dependency requests tags 0 Path/1 Git (203-231). `encode_dependency_selection` encodes `dependency_index` as `u32` with range check (192).
- `source.rs`: package key, `SourceLineage` tags 0 GitHub, 1 GitLab, 2 Git, 3 Workspace, 4 ExternalLocal (36-94), `ImmutableSourceResolution` tags 0/1/2 (98-125). ExternalLocal lineage path must be UTF-8 (85-90).
Same job elsewhere: the lineage/resolution writers have a parallel text form in `text/source.rs:98-160` (write) and 162-252 (read). Two parallel encodings (binary and text) of the same fields; any new `SourceLineage` variant must be added to both plus `validation/source.rs:107-132` (read). Stated limit: none. Fail-open: none seen (unknown enum variants would be a compile error since matches are exhaustive).

### subject/request_view.rs (78 lines) and subject/usage.rs (71 lines)
`Request<'a>` borrowed view (9) implemented `From` for both canonical and declaration request types (22, 47). `usage.rs`: `Budget{maximum,usage}` with `charge`/`reserve`/`copy_string` (30-71) and `CanonicalSourceClosureSubjectRecoveryUsage` (8). Both are mechanical. Same job: `CanonicalDependencySourceRequest::from(&DependencySourceRequest)` (`subject.rs:216`) and `into_authored` (`text/requests.rs:174`) and `Request::from` x2 are four conversions between the same two enums. Fail-open: none seen.

### subject/text/{framing,mod,record,requests,source,values}.rs (276 + 38 + 348 + 239 + 264 + 40)
1. Purpose: canonical text record `omega-source-closure <ver>` writer/reader for the subject.
2. Shape: `Writer` with optional `expected` for verifying canonical re-emission (`framing.rs:8-116`); `Reader` with `atom` (<=64 bytes, 148), `number`, `count` (checked against remaining framing, 192-200), `bytes` (quoted, `\xhh` lowercase only, 202-243); `record.rs`: `canonical_text` (32), `write_text` (64), `recover_text(_with_usage)` (172-193), `read_text` (195-326); `requests.rs`: root/request/navigation read-write; `source.rs`: source/key/lineage read-write incl. reconstructing a Git locator by joining strings (162-252); `values.rs`: `package_name`, `alias`, `relative_path`, `hex_string`.
3. I/O: text in/out; recovery re-emits and compares bytewise (`record.rs:189-192`), so only canonical text is accepted.
4. Decides: version tokens: `"1"` legacy product-only, `"2"` purposes, `"3"` Build-only catalog, `"4"` Product-query catalog (`record.rs:26-29`). Writer refuses to express catalog under legacy versions (65-81). Provider graphs inside a catalog use version 2 text and `allow_catalog=false` (`record.rs:165`, `read_text` 274-278) so nesting is refused at depth 2.
5. Cases by shape: purposes decoded by string compare: `"product" | "build"` (`record.rs:248-250, 288-291`); lineages decoded by atom `"github"|"gitlab"|"git"|"workspace"|"external-local"` (`text/source.rs:168-245`); Git transports `"https"|"ssh"|"scp"` with `scp` requiring empty port (219). Git/GitHub/GitLab lineages are reconstructed by building a URL string and re-parsing (`join` + `SourceLineage::git`, 172-180, 247-251). Read.
6. Same job elsewhere: parallel framing in `PM/lock/text/framing.rs` and the proposal framing (see above). Parallel binary encoder in `encoding/`.
7. Unused: `into_authored` is `pub(crate)` (`requests.rs:174`), used in `record.rs:345`.
8. Stated limits: `text/mod.rs` header: "Legacy `omega-source-closure 1` records remain readable as the versioned lock migration."
9. Fail-open: legacy v1 text decodes to product-only requests and edges (`record.rs:228-232, 245-254`); stated (`record.rs:4-6`: "retained locks keep their legacy product meaning rather than silently gaining build scope"). `read_authored_scope` does `limits.maximum_dependency_requests - *total_authored` (336) which cannot underflow because the total is checked after each addition (340). Kind: traced.

### subject/validation/{closure,dependency,mod,root,source,walk}.rs (173 + 105 + 24 + 130 + 145 + 49)
1. Purpose: invariants for a canonical subject: counts, strict canonical order of packages and edges, root present and exact, edge-to-projection agreement, alias uniqueness per `(requester, purpose)`, and closed reachable acyclic graph.
2. Shape: `validate_subject` (`closure.rs:18-164`), `selected_edges` (166, `partition_point` on requester), `validate_dependency_selection_kind` (`dependency.rs:15`), `validate_dependency_request` (67), `validate_root_request` and `canonical_root_request` (`root.rs:17, 36`), `git_request` (`source.rs:12`), `validate_package_navigation` (29), `validate_git_selection` (46), `validate_source_identity` (83), `validate_package_key`, `validate_source_lineage` (107), `walk::validate` iterative DFS with states 0/1/2 (`walk.rs:11-45`).
3. I/O: reads subject fields, charges a `Budget`.
4. Decides: path request selecting Git-lineage package requires same lineage and same resolution as requester and `Member` navigation (`dependency.rs:32-41`); path request selecting Workspace/ExternalLocal requires `Root` navigation (24-31); Git request must reproduce the selected lineage via `GitSourceRequest::new` and the selected resolution must be `Git` (50-56); alias must equal the explicit alias, else the selected package name with `-` replaced by `_` (`closure.rs:99-108`). Git selection vs navigation: `Root`/`Root` ok, `Named(p)`/`Member` with equal name ok (`source.rs:51-68`).
5. Cases by shape: `validate_root_request` handles three root kinds with distinct rules; Workspace root role rejected (`root.rs:42`). `validate_dependency_selection_kind` ignores `_requester_navigation` (`dependency.rs:18`, underscore-named unused parameter).
6. Same job elsewhere: alias uniqueness at `graph/mod.rs:185-194` and `resolution.rs:310-327` (`validate_aliases`); cycle detection at `graph/mod.rs:333-394` and `walk.rs`. Default alias derivation `-`=>`_` at `closure.rs:101-107` and `catalog.rs:135` (`default_alias()`), two spellings of one rule.
7. Unused: `_requester_navigation` parameter; all validation fns are `pub(super)`/`pub(in ...)`.
8. Stated limits: none.
9. Fail-open: `validate_optional_alias` (`dependency.rs:92-105`) checks only that an explicit alias is not empty (`alias.as_ref().is_some_and(...)`), not validity of the alias string (validity is a type-level `AliasName`, inferred). `validate_dependency_request` ignores `selection` for Git (`dependency.rs:83`). `validate_package_navigation` (`source.rs:29-44`) only constrains `Member`; any package with `Root` navigation passes. All traced by reading.

### PM/resolution/mod.rs (19 lines)
Module header; `mod compiler_input; pub mod graph; pub mod source; pub(crate) use PackageCompilationScope`. Note `graph` and `source` are `pub mod` but their inner types are mostly `pub(crate)`.

### PM/resolution/source/acquisition.rs (579 lines)
1. Purpose: acquires immutable sources for `Build` invocations and records the first capture of each `(lineage, navigation)`; binds returned declarations into `PackageSourceCustody`.
2. Shape: `BuildSourceAcquisitions` (31) with `admit_build_descent` (43), `new` (60), `require_locked_graph` (77), `retain_root` (84), `acquire` (94), `acquire_issued`/`acquire_completed` (133, 148), `acquire_requests` (158), `acquire_source` (192), `retain_source` (266), `acquire_git` (289); `AcquiredBuildSource` (360) with `capture_staged` (402), `capture_local` (433), `capture_git` (471), `declare_build`/`declare_source_result`/`declare_requests` (522-578); `SourceAcquisitionOrigin` (352).
3. I/O: reads `SourceResolver`, git resolution functions `resolve_git_source_at_revision_in_lane` / `from_pin_in_lane`; writes `git_sources`, `sources`, `workspaces`.
4. Decides: locked-graph gate `admit_locked_source_request`/`admit_locked_issued_source_request` when `locked_source` is `Some` (109-112, 186-188); reuse of an existing Git source keyed on `selected == request` (297-302); conflict check on `(lineage, navigation)`: `existing.resolution != acquired.resolution || existing.capture != acquired.capture` => error (274-279); package count limit `PackageSourceClosureLimits::default().max_packages` (282); build-descent cycle: `same_source` = lineage and navigation equal (389-391, 48).
5. Cases by shape: Path requests: `SourceLineage::ExternalLocal` => relative-to-requester directory (199-219); otherwise => Git-relative workspace path (221). Git requests: `PackageSelection::Named` => workspace member, else whole repo (233-259). Read.
6. Same job elsewhere: entry selection `main.omg` vs `build.omg` is `package_entry` (`source/mod.rs:59-65`, used `acquisition.rs:427`, `build_resolution.rs:132`, `operations/accepted_compilation.rs:560`; grep `package_entry\(`: 4 sites incl. definition); the same rule is hand-written again on the filesystem at `PM/review/candidate/passes/occurrences.rs:476-481` (uses `main_entry.is_file()` on the physical snapshot root rather than the captured tree).
7. Unused: `retain_root` (84), `acquire_completed` (148), `workspace_members`/`complete_workspace` are `pub`; not grepped repo-wide except `admit_build_descent` (grep: used by `build-system/orchestration/.../readiness.rs:197,580,632`; 3 outside calls).
8. Stated limits: `acquire_git` error "Git requests for one source lineage select different immutable content" (252).
9. Fail-open: `retain_source` (274-281): when a source with the same `(lineage, navigation)` already exists and equals, it returns the *new* `acquired` (not stored). Intended. `acquire` ignores `locked_source` when `None`. Kind: stated by `require_locked_graph` naming (77) (the lock is opt-in per operation); which operations call it was not traced.

### PM/resolution/source/build_resolution.rs (256 lines)
1. Purpose: suspend/resume protocol for resolving executed `Build` sources: `DeclaredBuildSource`, `ExecutedSourceResolution{Request,Complete}`, `PendingBuildSource::select`, `AcquiredBuildDependency::complete/complete_declared`, `ResolvedBuildSources::{join_build, join_source_result, compilation_inputs}`.
2. Shape: lines 15-256 as named.
3. I/O: wraps `PendingSourceResolution`/`begin_source_closure`; `compilation_inputs` builds `PackageCompilationScope::new(...).compilation_inputs_reusing(&mut None)` (252-253).
4. Decides: reuse an already-acquired custody when `custody.resolution()==acquired.resolution` and materialization equals (94-99); else error "source requests select conflicting immutable content" (100-103). `complete_declared` checks custody against the acquired capture: lineage, navigation, resolution, tree equality, snapshot root (155-166). `join_source_result` compares `DependencyProjections::from_executed_requests(result.requests())` to custody projections (216-221).
5. Cases by shape: none.
6. Same job elsewhere: executed-source custody equality duplicates `acquisition.rs:277`.
7. Unused: `AcquiredBuildDependency::entry_path` (130) duplicates `AcquiredBuildSource::entry_path` (`acquisition.rs:426-431`): both call `package_entry` on the capture.
8. Stated limits: none.
9. Fail-open: none seen. `join_source_result` returns `Err` on mismatches.

### PM/resolution/source/custody.rs (205 lines)
`PackageSourceCustody` (22): key, role, resolution, materialization, `snapshot_root`, `captured_source: Option<Arc<CapturedSourceTree>>`, navigation, `selection_evidence`, `source_limits`, `dependency_projections`, plus two storage-only cache directories ignored by equality (33-34). `PartialEq` (70-82) omits `source_limits` and the cache dirs; `semantically_equivalent` (185-193) additionally omits `snapshot_root` and `captured_source`. `from_resolved_parts` uses `debug_assert!(resolution.matches_lineage(...))` (101). `SourceCacheDirectories::retain` (47-61) swallows errors with `.ok()` (60) so cache coordinates silently disappear. `retain_compiled_review_storage` (196) called from `retain` (53) only (grep not run). Fail-open: `retain(...).ok()` returns `None` and callers treat as "no cache" (cache miss, not acceptance). Kind: traced by reading (`acquisition.rs:270-273` sets `Option`).

### PM/resolution/source/git/{mod,request,recorded,workspace}.rs (18 + 39 + 103 + 374)
- `mod.rs`: module list and re-exports.
- `request.rs`: `GitPackageSourceRequest{acquisition: GitSourceRequest, selection: PackageSelection}` (7).
- `recorded.rs`: `recover_recorded_git_source` (28-103): refuses if request lineage != recorded key lineage (39), requires `Git` resolution (44), fetches the exact revision (root: `resolve_git_source_at_revision_in_lane`; member: `GitWorkspaceSession::open_at_revision(...).open_members(...).materialize(...)` 51-72), then checks lineage, commit, tree, navigation (78-86) and returns custody with `PackageSourceSelectionEvidence::HistoricalGit` (97). Header (3-4): "without issuing a fresh `Build` declaration or re-interpreting old source text."
- `workspace.rs`: `BuildDeclarationCommitment::derive` (SHA-256 with domain `omega-git-workspace-build-declaration-v1`, 34), limits `MAX_BUILD_DECLARATION_BYTES=1 MiB`, `MAX_TOTAL=32 MiB`, `MAX_WORKSPACE_MEMBERS=4096` (26-28), `account_declaration_bytes` (109), `GitWorkspaceSelectionPlan`/`Evidence` (207, 238), `executed_selection_plan` (291-374): requires settled root, workspace declaration, member count == observed members (302), each member's captured tree must equal the exact declared subtree of the root catalog (316-327), member declarations are packages/applications only, no repeated name (345-353), selected name must exist (367). `revalidate` replays the plan and compares (271-288).
Decides on: member path string prefix `Path::new(path.as_str())` strip (315-323) and declared package name equality (354).
Same job: `workspace_acquisition.rs:137-186` also checks that each member subtree equals the captured projection (`expected` vs `materialized`); both compare a member subtree to the root catalog subtree (`workspace.rs:316-327`, `workspace_acquisition.rs:145-186`).
Unused: `BuildDeclarationEvidence` fields `repository_path` (137) and `byte_count` (138) and `GitWorkspaceMemberPlan.role` (156) are written and compared through derived `PartialEq` only; I found no accessor in this file (read); not grepped repo-wide: "suspected" they feed only equality in `revalidate`.
Fail-open: `recorded.rs`: none seen. `workspace.rs` `selected_member()` `.expect("selection plan retains its selected declared member")` (233): panic, not accept.

### PM/resolution/source/local.rs (22), local/recovery.rs (72), recovery/tests.rs (173)
- `local.rs`: `external_local_dependency_root` (9-22): rejects empty or ASCII-control location; absolute paths kept, relative joined onto requester root. No `..` normalization (the path is later canonicalized by `ExternalLocalLineage::canonicalize` in `acquisition.rs:205`). Callers: grep `fn external_local_dependency_root`: defined once (used `acquisition.rs:200`).
- `recovery.rs`: `recover_cached_external_local_source` (22-69): only for `SourceLineage::ExternalLocal` (29) else `Ok(None)`; finds cached snapshot for `canonical_absolute_path` + expected digest in the lane (33-38), calls `verify_package_source_snapshot` to re-hash bytes (42), then builds custody using the *recorded* role/dependencies and `PackageSourceNavigation::Root`. Fail-open: `Ok(None)` for non-local lineages and for cache miss means "caller falls back" (`old_sources.rs:47-101`). Traced.
- `tests.rs`: two tests: recorded role and ledgers retained (119); tampered snapshot bytes rejected (147).

### PM/resolution/source/mod.rs (128 lines), selection.rs (69), workspace_path.rs (75)
- `mod.rs`: re-exports; `package_entry` (59); `ResolvePackageSourceError{Source,RecordedGitPin}` (68); `PackageSourceMaterialization{content,file_count,byte_count}` (102); `PackageSourceNavigation{Root,Member(SourceRelativePath)}` (123) with an empty `impl PackageSourceNavigation {}` (128). Header says `package_entry` "falls back to the physical root only for legacy persisted-custody consumers" (4-6).
- `selection.rs`: `PackageSourceSelectionEvidence{Root,GitWorkspace(evidence),HistoricalGit{_source: Arc<ResolvedGitSource>}}` (9); `revalidate` returns `Ok(())` for `Root` and `HistoricalGit` (26-28); only `GitWorkspace` replays. Fail-open: `Root`/`HistoricalGit` always revalidate OK; "stated": header "recheckable source-selection evidence" (1-3) and `HistoricalGit` field is underscore-prefixed (retention only). Callers of `revalidate`: `audit/snapshot.rs:76,186`, `candidate/custody.rs:48`.
- `workspace_path.rs`: `normalize_member_path` (8-39): requires relative `/`-separated path, no `\\`, no control bytes, no empty component, `.` skipped, `..` pops (escaping the workspace is an error, 27-30), root result rejected. Test at 50-74. `source_relative_path` (41) `.expect(...)` on `SourceRelativePath::parse`.

### PM/resolution/source/workspace_acquisition.rs (414 lines)
1. Purpose: for Git `PackageSelection::Named(...)` requests, open the repository catalog (workspace), run its declarations, and select one member.
2. Shape: `WorkspaceMemberObservation` (25, `new` 33), `AcquiredWorkspaceBuild` (69), `AcquiredWorkspaceMembers` (75), `ExecutedWorkspace` (82), `AcquiredWorkspaceBuild::members` (108-207), `acquire_workspace_member` (217-282), `workspace_members` (284), `complete_workspace` (296-338), `acquire_workspace_path` (340-384), `ExecutedWorkspace::select` (388-414).
3. I/O: reads `GitWorkspaceSession`, `SourceBuildDeclaration`; mutates `self.workspaces`.
4. Decides: a Git-relative `Path` request selects a member by normalized path string equal to a declared member path, then by the member's declared package name (360-383). Observation consistency: `declaration.declaration() == result.declaration()`, demanded sources length equals dependency graph count and roots equal (39-48). Limits: entries and bytes accumulate against `self.root.limits.max_entries/max_bytes` (170).
5. Cases by shape: "BuildDeclaration::Workspace" members rejected (373-375). Read.
6. Same job elsewhere: member-subtree inventory comparison also in `git/workspace.rs:316-327`.
7. Unused: `AcquiredWorkspaceMembers::sources` (211) pub; not grepped.
8. Stated limits: none.
9. Fail-open: `acquire_workspace_path` reuses `requester.resolution` as the key to find an executed workspace (`workspace.resolution == requester.resolution`, 350); a miss errors "Git-relative Path has no executed workspace catalog". None seen.

---------------------------------------------------------------------------------------------------

## Part C. package-manager: review

### PM/review/audit/mod.rs (685 lines) and audit/snapshot.rs (341 lines)
1. Purpose: renders a bounded, content-verified source patch between baseline and candidate snapshots (Myers diff), output named `OMEGA_PACKAGE_SOURCE_PATCH_V1`.
2. Shape: constants (34-40): 4096 entries, 16 MiB bytes, 1 MiB metadata, 65,536 lines, 8,000,000 diff work, 16,000,000 trace cells, 32 MiB output; `render_package_source_patch` (157-242); `BoundedOutput` with `push_escaped` (255-310); `DiffBudget` (367); `myers_diff` (438-521); `reconstruct_edits` (537); `render_hunks` (611); `snapshot.rs`: `capture_snapshot` (70), `captured_entries` (123), `revalidate_snapshot` (181), `render_entry` (200), `file_content_requires_standalone_audit` (266), `is_model_text` (278: no NUL and valid UTF-8).
3. I/O: reads `PackageSourceCustody` (tree or physical root), `selection_evidence().revalidate()`.
4. Decides: package key equality (162); changed paths = union of paths whose entry differs (171-176); text vs binary by `is_model_text` (`snapshot.rs:242`); files equal by bytes only, executable bit differences are printed but `content_review unchanged` is decided on bytes (`snapshot.rs:240-241`).
5. Cases by shape: entry kinds Directory/File/Symlink (`snapshot.rs:319-326`). Escape: `\\` and bytes outside 0x20-0x7e become `\xhh` (`mod.rs:287-303`).
6. Same job elsewhere: `captured_entries` re-implements limit checking (entries, depth, bytes) over a captured tree (`snapshot.rs:123-179`) while the physical path uses `capture_verified_package_source_snapshot`; `candidate/custody.rs:35-47` verifies the physical snapshot with `verify_package_source_snapshot`.
7. Unused: n/a. `PackageSourcePatch::changed_entries`/`requires_standalone_audit` used by `source_review.rs:69,71`.
8. Stated limits: header: "Both snapshots are verified ... during capture and revalidated after rendering."
9. Fail-open: `revalidate_snapshot` returns `Ok(())` immediately when `captured_source().is_some()` (`snapshot.rs:189-191`): in-memory captures are trusted without physical verification. Kind: stated (candidate/custody.rs:1-4: "Retained in-memory captures already own the exact bytes consumed by compilation"). `captured_entries` depth: `path.components().count() - usize::from(!matches!(node, Directory))` (137-138): a root-level file counts depth 0. `unreachable!` at `mod.rs:520` for Myers.

### PM/review/candidate/custody.rs (58), error.rs (354), mod.rs (37)
- `custody.rs`: `verify_transitive_source_custody` (13) and `verify_selected_source_custody` (25-58): for each package in the compilation scope, if there is no captured tree it verifies the physical snapshot, then `selection_evidence().revalidate()`. `PackageSourceVerificationPhase` has a single variant `AfterCompilation` (`review_set.rs:112-114`).
- `error.rs`: `CompileResolvedPackageReviewsError` with 25 variants (34-128); `from_compilation` (141-170) converts `CompileFailure::{Diagnostics, BuildConsentRequired}`; `render_package_diagnostics` truncates names at 256 bytes (31, 180) via `&name[..shown_name]` (184): byte slicing of a `&str`, which panics on a non-char boundary. Kind: open (name validity assumed ASCII by `PackageName` parse, inferred not read).
- `mod.rs`: re-exports; `build_observation_commitment` (33) = `summary.identity().digest()`.
Fail-open: none seen.

### PM/review/candidate/passes.rs (182 lines)
1. Purpose: orchestrates review of a target closure in two passes: a discovery pass (`TargetEntryDiscovery::Dependencies`) then, if semantic bindings were discovered, a bound pass (`Disabled`) that carries unchanged occurrences forward.
2. Shape: `CandidateReview` (38), `review_resolved_packages` (43), `review_candidate` (53-119), `note_pass_accounting` (121), `review_pass` (148-182).
3. I/O: builds `CanonicalSourceClosureSubject::from_resolved` (159), `PackageOccurrenceRoster::derive` (164), `semantic_bindings_by_consumer`, `PackageReviewPass::admit`; runs the injected executor `execute(plan, source_bindings)` (181).
4. Decides: second pass iff `discovered` is non-empty (93-95). Execution profile `target::TargetProfile::host_if_supported()` (158).
5. Cases by shape: exactly two passes (preliminary and bound); no loop to a fixed point (stated implicitly by structure). Read.
6. Same job elsewhere: none.
7. Unused: n/a.
8. Stated limits: header (1-14).
9. Fail-open: `host_if_supported()` returns `Option`; `None` means no Build occurrences can run, enforced in `admit` (`occurrences.rs:187-199`) as `UnsupportedBuildActivation`. Traced.

### PM/review/candidate/passes/occurrences.rs (936 lines)
1. Purpose: admits each (package, purpose) occurrence in dependency-first order, assembles its compile inputs, carries forward an identical earlier compile (same pass, prior pass, or persistent cache), or hands out a `PackageReviewWork` request, then joins compiler evidence into reviews.
2. Shape: `PackageReviewPass::admit` (174-237), `occurrences` (239), `begin_round` (261); `PackageReviewRound` (291) with `admit_occurrence` (329), `assemble_occurrence_inputs` (362-497), `carry_prior_compile` (499-618), `component` (620), `retain` (657-809), `finish` (811-860), `into_retained`; `RetainedOccurrenceCompile` (90); `OccurrenceCompile::{Live,Cached}` (103); `projection_failure` (47); cap `MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES = 64 MiB` (45).
3. I/O: reads `PackageOccurrenceRoster`, semantic bindings per `(PackageKey, purpose)`, prior `RetainedPackageReviews`, compiled-review cache dir; writes `reviews`, `review_positions: Vec<[Option<usize>;2]>` (281), `component_candidates`, `target_entry_candidates`.
4. Decides: carry-forward match keyed on `(position, purpose, entry path, complete inputs equality, build_snapshot equality)` and not a "fresh component" unless Live (527-535); else cache lookup keyed by `compiled_review_key` over digest of inputs (559-567). A unique target-entry candidate is chosen only when exactly one candidate fits (`if let [candidate] = candidates.as_slice()`, 404). Dependency handoff accepted only if `review.resolution()==custody.resolution()`, `bundle.package()==dependency.identity()`, commitments equal (453-457).
5. Cases by shape: `review_positions` is `vec![[None; 2]; n]` indexed by `purpose_slot` (281, 428, 599): fixed two purposes (Product, Build). `root_role == Application` is the only role that gets target-entry discovery (390). Root gets `root_build_snapshot`; every other package gets an empty `BuildSnapshotRequest` (482-487).
6. Same job elsewhere: entry-file choice `main.omg` else `build.omg` at 476-481 duplicates `package_entry` (`resolution/source/mod.rs:59`); semantic-binding permission handling is also in `semantic_bindings.rs:118-191`.
7. Unused: `PackageReviewRound::component` (620) and `PackageReviewCompletion` methods are `pub`; no grep run.
8. Stated limits: `fresh_components` flag; error text "an independent provider selection names dependency package ..., which this closure never compiled" (649).
9. Fail-open: (a) `carry_prior_compile` reuse for `recovered` uses `.rev().find_map` first match (521-543). (b) Persistent cache hit path calls `verify_current_source_consumption` re-deriving the commitment over stored units plus live inputs (576-583, 121-137); a `Live` carried compile skips that check (`Self::Live(_) => Ok(())`, 126) because the compile ran in this process. Kind: stated (the header says Live holds "opaque live source custody"). (c) Cache read failures return `None` via `?` inside `or_else` (557-572) and fall through to a fresh compile. Kind: traced (cache miss path). (d) `into_review_and_facts` (`review_cache.rs:95-129`) validates only identity/name/lineage/resolution digest, not purpose/target/entry (those are inside the file-name hash key `key.rs:58-73`). Kind: traced.

### PM/review/candidate/review_cache.rs (328), review_cache/key.rs (162), review_cache/record_codec.rs (1112)
1. Purpose: persistent content-addressed cache of compiled reviews (`OMGCREV\x01`, version 16): key derivation, file storage with SHA-256 trailer and eviction, binary record codec.
2. Shape: `CompiledReviewCache{directory,limits}` (136) with `open` (142), `lookup` (158), `store` (180), `make_room` (229), `record_path`; defaults 256 records, 512 MiB (43-44); `CachedCompiledReview` (55), `CachedOccurrenceFacts` (47), `PackageReviewCacheWrite::store` (87); `compiled_review_key(_with_identity)` (`key.rs:26,48`, domain `OMEGA-COMPILED-REVIEW-CACHE-KEY-V3\0`); `lineage_digest`, `resolution_digest` (76, 82); `record_codec.rs`: `encode_record` (774), `decode_record` (848), per-type encoders/decoders (service schema 499/533, build handoff 617/656, consumed unit 693/720...).
3. I/O: reads/writes `<dir>/<hex key>.rec`; temp file `<pid>.<seq>.tmp` then `rename` (209-226), mode 0444 on unix; oldest-mtime eviction (229-257).
4. Decides: key binds package identity, purpose tag, target, build profile, entry path bytes, compiler input identity (`crate::operations::compiler_input_identity()`; failure => `None` => miss, `key.rs:35`), inputs digest, snapshot digest (58-73). On decode failure the file is deleted (`review_cache.rs:174`). The record rejects unknown tags, retired versions (test 998-1002), trailing bytes, count > `1<<20` (42, 65).
5. Cases by shape: service-entry authority flow decodes only the string `"accepts"` (`record_codec.rs:401-404`); completion decodes `0` NormalReturn / `1` External(ProcessExit) only (414-420); termination subject tag `1` = `Parameter` only (440-443). Calling operand kinds tags 0-5 (360-367). These are the only values the in-memory enums have per the match arms (I did not read the enum definitions).
6. Same job elsewhere: the aggregate record limit `DEFAULT_MAX_RECORD_BYTES` appears in both `review_cache.rs:44` and `record_codec.rs:10,849`.
7. Unused: `compiled_review_key_with_identity` (grep): defined `key.rs:48`; re-exported `review_cache.rs:23`, `candidate/mod.rs:28`, `review/mod.rs:19`; 1 production caller outside `key.rs` (`operations/accepted_compilation.rs:576`). `PackageReviewCacheWrite::component_source` (83) pub.
8. Stated limits: header (5-16): "A record never replays a checked verdict by name ... SHA-256 trailer rejects corruption and truncation"; "sealed compiler program storage is not serializable".
9. Fail-open: the trailer is an unkeyed SHA-256 (`review_cache.rs:197-200`, `record_codec.rs:855-859`), so it detects corruption but not tampering by a writer with directory access: a hand-built record with a correct trailer and matching lineage/resolution digests would be accepted, subject to the re-derivation of the source-consumption commitment over live inputs (`occurrences.rs:576-583`) and `build_handoff` recomputation is NOT redone. Kind: open for the handoff/policy content (not traced to the consumer). `decode_record` check that handoff `selection_observation` equals the summary's identity (889-900). `lookup` returns `None` on any read error (160).

### PM/review/candidate/review_set.rs (139), semantic_bindings.rs (219)
- `review_set.rs`: `CompilerIssuedPackageReview` (22) and `CompilerIssuedPackageReviewSet` (107); no public constructor ("deliberately", 6-9); `policy_rows_and_commitment` (96); lookup helpers `reviews_for`, `review_occurrence` (121, 130) match by key and `checked_context().purpose()`.
- `semantic_bindings.rs`: `semantic_bindings_by_consumer` (59-116): rejects unknown consumer (78), context mismatch (91-94: `roster.is_occurrence`, expected target by purpose, `build_execution_profile == execution_profile`), duplicate `(consumer, purpose, role)` (102). `candidate_semantic_binding_inputs` (118-191): for the root's Product occurrence, only `FilesystemHostService` and `TimeHostService` roles get permission rows via `filesystem_host_permission_rows`/`time_host_permission_rows` (132-146, 147-169); for other roles at the root they keep the binding unchanged; for non-root or Build occurrences the permissions are cleared (170-181).
  - Stated vs code: header (7-9) names "`ProcessExit` / `Console` exit-binding leaves or settled `FilesystemHost` / `TimeHost` cohort permission tables"; the code branch (132-151) names only Filesystem and Time roles; `ProcessExit`/`Console` bindings at the root fall into the `else`-free path and keep whatever the candidate binding carries. Read.
  - `reviews_for` (121): grep not run.
Fail-open: `settled_permission_rows.ok()` (138, 144) converts a failed row computation to `None`, then `ok_or_else(InvalidCandidateSemanticBinding)` (152) turns it back into an error for the two roles. None seen.

### PM/review/compare/{assembly,fingerprints,limits,merge,mod,paths,projection,replacements}.rs (188+202+211+123+448+142+144+166)
1. Purpose: compares a fresh candidate review set to the accepted lock policy (or empty baseline) and produces `PackagePolicyChangeSet`: per-package, per-purpose row changes, root-role change, and source replacements; every item gets a domain-separated SHA-256 fingerprint.
2. Shape: `compare_package_policy_changes` (`mod.rs:163-214`); `assembly::packages` (`assembly.rs:17-188`) merges the sorted union of old/new packages and, per purpose, rows via `merge::append_rows` (`merge.rs:14-85`); `projection::candidate` (`projection.rs:15-123`) validates the review set covers the exact occurrence roster; `paths::Paths` (13); `replacements::compare` (47); `fingerprints::{context,package_context,finish_package,source_replacement}`; `limits::{PackagePolicyChangeLimits (default 4096 packages, 131072 rows, ...), Budget}`.
3. I/O: `accepted: Option<&PackageLockTarget>`, candidate reviews, `ExactTargetPackageSourceClosure`.
4. Decides: row pairing key `(row.kind(), row.key_bytes())` (`merge.rs:89`); a row change exists iff rows differ or the occurrence context changed (`merge.rs:27, 46`); `requires_decision`: existing package => `update_requires_decision` of either side, new package => `initial_requires_decision` of the candidate (`merge.rs:55-66`); `audit_recommended` includes `source_changed`, `source_association_changed`, `occurrence_contexts_changed` only when an accepted baseline exists (`assembly.rs:158-161`); root change keyed on selected key and `root_role` (`mod.rs:188-191`); role change is flagged only for `Package<->Application` with the same root key (`mod.rs:224-231`); source replacements are keyed `(requester key, purpose, alias)` (`replacements.rs:124-132`), neither package name nor row position (header 3-4). Target equality required (`mod.rs:171`).
5. Cases by shape: `ReviewOnlyRootRoleContract` has exactly two contracts (`mod.rs:53-56`). `DependencyPurpose::ALL` iterated: Product and Build. Execution profile taken from `reviews.first()` (`projection.rs:63-65`): all reviews must equal that profile (90-95).
6. Same job elsewhere: BFS path `paths.rs:13-66` vs `reconcile/dependency_paths.rs` (above); the policy-row key order `(kind, key_bytes)` also appears in `merge.rs::compare` only.
7. Unused: n/a. `PackagePolicyChangeSet::fingerprint` consumed by `decision/document.rs:119`. `ReviewOnlyRootRoleChange` getters: not grepped.
8. Stated limits: `limits.rs` 8 ceilings (16-25); error strings "normalized policy comparison exceeds {resource} limit".
9. Fail-open: (a) `assembly.rs:73-75`: `baselines = accepted.map_or(&[][..], |a| &a.occurrences()[old_occurrence_index..old_occurrence_index + old_count])`: slice indexing panics rather than accepts if the lock's occurrences are misaligned with the roster. Kind: open. (b) `projection::candidate` `count != roster.occurrence_count()` => error (57-62). (c) `role_change` returns `None` when root keys differ (221-223) because `root_changed`/source replacement already cover it (`mod.rs:188-191, 198-203`). Kind: traced. (d) `requires_decision` for an unchanged package with `occurrence_changed` false emits no row => no decision needed. By design.

### PM/review/decision/{mod,document}.rs (208 + 524)
1. Purpose: renders the restartable review document and recovers user decisions; resolves decisions to `PackagePolicyResolution`.
2. Shape: `resolve_package_policy_decisions` (`mod.rs:124-208`); `render_package_policy_review` (`document.rs:57`), `recover_package_policy_review` (65-121), `template` (181), `render` (190-301).
3. I/O: regenerates the canonical template from `PackagePolicyChangeSet` and compares line-by-line with the user's text; only `decision ... pending` lines may change to `accept`/`reject`.
4. Decides: decision subjects keyed by a fingerprint digest (`Subject::Row/SourceReplacement(digest)`) or `RootRole` (`mod.rs:35-39`). Required decisions: root role, every source replacement, rows with `requires_decision` (`mod.rs:150-166`). Rejects too many, unknown, non-blocking, duplicate and missing decisions (`mod.rs:174-203`). Recovery refuses any text without a trailing `\n` or containing `\r` (`document.rs:75`), any changed non-decision line (109), unresolved `pending` (116-118).
5. Cases by shape: first line `comparison <hex>` mismatch is reported as `WrongComparison` rather than `ChangedFindings` (86-90).
6. Same job elsewhere: none.
7. Unused: grep `resolve_package_policy_decisions|all_required_changes_accepted`: `resolve...` 1 outside call (`document.rs:119`); `all_required_changes_accepted` 2 outside calls (`package_manager/change_decisions.rs:71`, `operations/package_change.rs:68`).
8. Stated limits: `ReviewOnlyRootPolicyDisposition` only `Accept/RejectCandidateChange` (`mod.rs:29-32`).
9. Fail-open: `all_required_changes_accepted` (`mod.rs:62-66`) returns `true` for an empty decision list: correct when no decision was required; if `resolve_package_policy_decisions` had returned an empty list for a change set requiring decisions it would be wrong, but line 195-203 errors `MissingDecision` first. Kind: traced. `expected.strip_suffix("pending").expect(...)` (93) panic on generated template only. `recover` uses `lines()` which also splits on `\r\n` (but `\r` rejected earlier, 75).

### PM/review/mod.rs (43), restricted_build_grants.rs (163), timings.rs (91)
- `mod.rs`: module list and re-exports.
- `restricted_build_grants.rs`: `UngrantedRestrictedBuildRequest::project` (40) renders acceptance text for a request via `restricted_build_request_acceptance_text`; `RestrictedBuildCheckpoint::derive` (96) collects `RestrictedBuildRequest` row `canonical_text` strings per occurrence `(package identity, context)`; `grants` (120) finds the entry by `identity == package && context == context` (133-135) or `unwrap_or_default()` empty set (137); `CheckpointRestrictedBuildGrants::admit` (150-162) admits iff the request's acceptance text is in the set. Fail-open: none; a missing occurrence yields an empty set so every request is refused (fail-closed). Traced.
- `timings.rs`: stderr timing output controlled by env `OMEGA_REVIEW_TIMINGS` or a thread-local `REQUESTED`; `StageTiming` prints on drop (75). Read.

---------------------------------------------------------------------------------------------------

## Part D. resolver-execution (6 files, 534 lines)

### RX/lib.rs (29), RX/resolver_execution.rs (99), path_custody.rs (110)
1. Purpose: freeze the resolver (git) executable path outside any package-controlled root, then prepare per-phase bounded processes at a validated absolute working directory.
2. Shape: `ResolverExecutionBackend{executable}` (`resolver_execution.rs:32`) with `open` (37-53), `prepare` (65-83), `executable`; `ResolverExecutionPhase` 4 variants (24-29); `resolver_process_limits` (86-96): 120 s CPU, 8 GiB address space, 1 GiB file size, 256 open files, 16 active processes, 2 GiB per-process, 4 GiB aggregate; `path_custody.rs`: `require_absolute`, `require_lexically_canonical_bounded_path` (32 KiB, 8), `require_regular_file`, `require_outside_roots` (68-93). `lib.rs` re-exports `bounded_process` types under resolver names.
3. I/O: `std::fs::metadata`, `canonicalize`; builds `std::process::Command` with `current_dir(working_root)`.
4. Decides: executable must be absolute, lexically canonical (no `.`/`..`, normalized equals original, 43), outside every root both lexically and canonically (40, 49: `path.starts_with(root)` or `canonical_root`), regular file after canonicalization (47). `prepare` requires the working root to be absolute, canonical, and that the executable is not inside the working root (79).
5. Cases by shape: `phase` only changes the error name string (70-76); all four phases get the same limits. Read.
6. Same job elsewhere: none seen in this unit. `source-acquisition/.../executor.rs:155` is the one production caller of `open` (grep `ResolverExecutionBackend::open`: 1 production caller, 6 test callers incl. `source-acquisition/src/test_support.rs:282`).
7. Unused: n/a.
8. Stated limits: header: "Nothing in this crate produces canonical package evidence; git command policy belongs to `source-acquisition`."
9. Fail-open: `require_outside_roots` (77): `root.canonicalize().ok()`; if a package-controlled root does not exist, only the lexical `starts_with(root)` is checked, so a symlink pointing into a not-yet-existing root passes the canonical branch. What gets through: an executable whose canonical path lies under a root that cannot be canonicalized at open time but whose lexical path does not. Kind: open (not traced to callers). `phase` is not used for any check (70-76).

### RX/resolver_execution/tests/{mod,unix,validation}.rs (28 + 54 + 144)
Test harness (`/bin/sh` or `C:\Windows\System32\cmd.exe`), a unix-only test of `ulimit -n <= 256` (11-54), and validation tests: frozen executable reuse and limits (15-40), executable inside a package root rejected (43), symlink into a package root rejected (67, unix only), relative/noncanonical/oversized/executable-parent working roots rejected per phase (96-129), directory executable rejected (141).

---------------------------------------------------------------------------------------------------

## Part E. review-evidence capture (41 files)

General shape (read across the files): each `project_public_*` function walks one kind of checked declaration (`compilation.traits()`, `conformances()`, `domain_definitions()`, `const_declarations()`, `data_definitions()`, `propositions()`, `all_operators()`), keeps only those with `is_public`, skips those not owned by the reviewed package (`owned_identity(...)? else continue`), projects them to `Package*Shape` rows, and returns a `ProjectedReviewRow{row, declaration}` vector sorted by identity. Many functions enforce "exactly one" by slice patterns `let [x] = matches.as_slice() else { Err }`.

### RE/capture.rs (68), api.rs (13), api/data.rs (5), api/domains.rs (7), behavior.rs (18), callables.rs (16), behavior/policy/mod.rs (16), behavior/crash/mod.rs (42), behavior/crash/permissions.rs (11)
Module headers and re-exports. `capture.rs` defines the `refusal!` macro (36-47) and `EvidenceOption::or_evidence` (50-58); `capture.rs` re-exports `project_package_review`, `candidate_*_binding` fns, and the `project_checked_*_policy` entries. `crash/mod.rs`: `project_crash_routes` (15) sorts and dedups routes; `project_crash_cause` (37) maps Trap/Abort. `permissions.rs`: `project_crash_predicate` copies `predicate.canonical_bytes()` (5-11). `callables.rs` declares `mod external_supply; mod policy; mod policy_parameters; mod reach_dependency; mod surface; mod target_context;` which are NOT in this unit (suspected: read by another reader).

### api/conformances.rs (177)
`project_public_conformances` (19-177): per public conformance, resolves its trait (exactly one, 46-52), requires the trait to be public (53), lifetime-argument arity (59-68), computes inherited evidence requirements through `collect_evidence_requirements` (88-98), builds interface (113), then for each conformance row requires valid realization machine/state (121), resolves the declaring trait and matches exactly one inherited requirement (141-155), and requires the realized set to equal the interface requirements with no duplicates (157-164). Decides keyed on identity records (`nominal_identity`, `trait_requirement_identity`), not names. Fail-open: none seen.

### api/constants.rs (118)
`project_public_consts`: requires one unique immutable declaration (37-48); requires the `pending_target_observation` flag to equal custody (49-57); target-dependent initializers are projected with `PrivateImplementation` exposure (77), evaluated ones emit the canonical encoding or `semantic-nan:<format>` (94-101). Fail-open: none seen.

### api/data/evidence.rs (290), api/data/projection.rs (216)
- `evidence.rs`: re-derives data-definition invariant facts (`require_rederived_data_definition_facts` 64-91) and compares retained evidence to the rederivation (`retained != expected`, 85); `rechecked_data_definition_evidence` (93-209) builds a normalized view (definitions, semantic facts, refs, contexts, symbol_sets) and returns `None` on any dangling arena link; `fact_plan_arena_links_are_well_formed` (211). In the `contexts`/`symbol_sets` `filter_map` closures, a context or set whose `refs.span` is missing and which is not about a data symbol returns outer `None` and is *skipped* (154-155 / 181-182), while a data one yields `Some(None)` which makes the whole result `None`. Grep for the three fn names: each used only inside this file and `projection.rs:5,90`.
- `projection.rs`: `project_public_data` (86-216): requires rederived facts and quotient formation validation (90-91); quotient data requires exactly one formation row (114-120), exactly one relation declaration (121-132) and public relation (133-138); members projected as `Field`/`Variant` (154-194); `project_data_invariant_facts` (25-60) checks every `where_facts` handle through `require_exact_checked_data_fact` (61-84), which delegates to `api/domains/facts.rs::require_exact_checked_fact`.
Fail-open: the skip semantics above (stated by shape, not commented). Kind: open (what program shape yields an unrelated skipped set: a symbol set with an invalid span whose symbol is not a data symbol; I infer it is irrelevant to data review).

### api/domains/{aliases,facts,projection}.rs (174 + 236 + 115) and facts/checked_rows_tests.rs (182)
- `aliases.rs`: `project_domain_alias_expansion` (24-91): transitive alias expansion; cycle check by a stack (34); `Carry::Portable` expands to all `CarryPermission::ALL` (62-63), other unresolved names must be `CarryPermission::from_name` else error (64-72) -- keyed on the **spelling** `label == "Carry::Portable"` (62) and `CarryPermission::from_name(&label)` (65); empty expansion rejected (85-89). `project_domain_establishment_route` (93-174): three route kinds; exact machine requires unique declaration (107-120); trait routes require exactly one trait and one requirement and `owner.is_boundary == expects_boundary` (145).
- `facts.rs`: `project_domain_predicate_facts` (25-60), `project_definition_contract_fact` (62-121) for `ProofFact::{Expression, Membership, Proposition}`; `require_exact_checked_fact` (150-199): exactly one semantic row at `ProgramPoint::Definition{symbol}` with `QualificationEvidence::default()` and matching payload, exactly one retained ownership record; `semantic_fact_matches_definition_fact` (201-233). `exposes non-public domain` check applies only when the reviewed package owns the domain (91).
- `projection.rs`: `project_public_domains` (19-86), `project_domain_semantic_roles` (88-115): each semantic role identity must equal `definition.semantic_id` (106).
- `checked_rows_tests.rs`: a differential test with a copy of the older full-scan `original_require` (10-58) against the indexed implementation (`rows_at_point`); this duplicates `require_exact_checked_fact` production logic in test code.
Same job elsewhere: `require_exact_checked_fact` is shared by domains (`facts.rs:129`) and data (`data/projection.rs:67`) as its header says ("shared by public domains and data definitions"): 2 callers outside its own file's definition. Propositions (`api/propositions.rs:40-60`) do a similar "exactly one checked declaration row" check by hand without it.
Fail-open: none seen.

### api/operators.rs (220)
`project_operator_coordinate` (35-53) (identity, parameter dispatch text, result dispatch only for named boundary operators: `declaration.is_boundary && declaration.spelling.is_none()`, 45); `project_public_operators` (55-162): requires retained operator-crash contracts to equal the compiler rederivation (62-69), skips non-product-scope instances and non-owned operators (73-79), requires exactly one checked crash row per operator (117-129), rejects duplicate coordinates (153-160); `project_operator_crash_routes` (164-220): unconditional bucket with facts => error, guarded bucket must have expression facts and a non-empty guard set. Fail-open: `continue` at 73-75 and 77-79 (skipped operators are by design not part of this package's public surface). Kind: stated by the owner filter. None else seen.

### api/policy.rs (38), policy/declarations.rs (142), policy/traits.rs (179)
`policy.rs::project` (17-36) assembles `PackagePolicyPublicApi {traits, conformances, domains, propositions, consts, operators, data}`. `declarations.rs` converts the `Package*Shape` rows from `api/*` into `PackagePolicy*Shape` with `signatures::parameters` adding the static telescope (data 15, domains 49, conformances 83, operators 111; operator `return_type` is `source.return_type.is_valid().then_some(...)`, 136). `policy/traits.rs::project` (22-73) and `project_requirement` (75-179): re-projects trait requirements (parameters, return type, contracts, published crash, termination).
Same job elsewhere: `policy/traits.rs::project_requirement` (75-179) and `api/traits.rs::project_trait_requirement` (160-268) both project a trait requirement's contract context, crash, parameters, return type and termination into two record shapes (`PackagePolicyTraitRequirement` vs `PackageReviewTraitRequirement`). Each public declaration kind is thus projected twice: once to `PackageReview*Shape` (`api/*`) and once to `PackagePolicy*Shape` (`policy/*`), the second copying fields from the first (`declarations.rs:33-44, 67-78, 101-106, 129-139`). Kind: confirmed by reading both.
Fail-open: `policy/traits.rs:42-46`: `source_requirements.len() != row.requirements.len()` => error. None seen.

### api/propositions.rs (144)
`project_public_propositions` (19-144): `Primitive`/`Witness` bodies require one checked declaration row in `proposition_vocabulary.declarations` and that it is public (40-60); `Transparent` bodies project their `PropositionFormula` (91-128) with `visiting = [declaration.symbol]` (105). `unreachable!()` at 78. Binder naming scheme `proposition-binder:{position}` (69, 88). Fail-open: none seen.

### api/slot_metadata.rs (147)
`project_requirement_slot_metadata` (19-66): for each dynamic slot plan of a (trait, requirement), checks the plan's returned-reference sources equal what `declared_return_sources` derives (38-47), projects origins and qualification preconditions, requires all selector applications to agree (57-61), returns empty metadata if there are none (51-56). Qualification domains are looked up by `semantic_id` equality, requiring exactly one definition (113-129). Fail-open: with no plans for a requirement the metadata is empty (51-56): "no dynamic slot" means no returned-borrow origins are published. Kind: open (not traced whether a requirement with a returned reference but no slot plan can exist).

### api/traits.rs (320)
`project_public_traits` (44-111), `project_trait_parent` (113-158; kind `ServiceReach` if boundary else `Policy`), `project_trait_requirement` (160-268), `project_parent_arguments` (270-320): `Implementation`-kind arguments that are not conformance parameters are closed with `close_conformance_application` and re-projected (288-319). Requirement projection requires exactly one crash capsule (207-224). Fail-open: none seen.

### RE/capture/authority.rs (112)
`dangerous_authority_class` (42-112): classifies services by exact `(file name, declaration name)` pairs in `CORE_CRITICAL_SERVICES` (25-40) when the service comes from the toolchain source whose `package_root.file_name()=="core"` (57-60) and is not a canonical virtual toolchain path; then by two compiler-resolved aggregate bindings (`ConsoleExitProcessI32`=>Process, `FilesystemHostService`=>Filesystem, 84-109). Keyed on file name string and declaration path string (74-78: `relative == Path::new(path) && identity.path() == *declaration`). Cases by shape: 14 fixed (file, name, class) rows; anything else returns `Ok(None)` (111) = "not dangerous". Fail-open: **a service not in this table or not under the toolchain `core` root is classed non-dangerous** (`Ok(None)`). What gets through: a new core service not listed in the table is reviewed as ordinary. Kind: open (no comment says new core services must be added; stated limit: header says "critical `omega::core` services"). Also `TimeHostService` is not in the aggregate-binding list (only Console exit and Filesystem host), though `semantic_bindings.rs:140` treats `TimeHostService` as a permission-bearing role.

### behavior/crash/expressions.rs (496)
`project_boolean_expression` (31-119) and `project_scalar_expression` (121-292): closed projection of checked crash-predicate expressions; unsupported shapes return `None` (comment 7-9): integer-math comparisons, erased parameters, private storage reads, byte-sequence predicates, constant applications, implementation selectors (37-40, 127-130). Large match arms mapping enums 1:1 (349-496). Users: grep `project_boolean_expression`: 12 occurrences across 3 files (`expressions.rs` 10, `crash/mod.rs` 1, `policy/crash.rs` 1): the one consumer is `policy/crash.rs:105`, which turns any `None` into an error "crash requirements contain an unsupported scalar expression" (107-109). Fail-open: none seen: `None` becomes an error, not a skip (traced to the single caller).

### behavior/invocations.rs (65)
`canonical_checked_invocation_targets` (10-44) produces strings `parameter:{index}` / `service:{definition.name}` (spelling-based; used for comparing checked sets, callers `capture/source/invocations.rs:42,44` and `callables/surface.rs:160-161`: 4 call sites, grep) and `project_synchronous_invocations` (46-65) produces typed `PackageReviewSynchronousInvocation::{Parameter, Service(nominal identity)}`. Two forms of the same target. Fail-open: none seen. Requires exactly one boundary trait per service symbol (24-36).

### behavior/policy/crash.rs (259)
`crash` (35-124): cross-checks `derive_authored_machine_crash_buckets` against the machine contract plan and envelope; `validate_crash_publication` (126-180) for concrete machines and for family heads/members against original custody; `crash_routes` (198-259) projects guarded routes with proof-fact handle `None` (comment 8-10). Rejects a guarded bucket whose source fact is the literal `true` (226-233) and empty guard sets (248). Inferred causes: `matching.next().is_some()` duplicate => error (88-91); no summary => `PackagePolicyInferredCrash::Unknown` (112). Fail-open: absent inferred summary gives `Unknown` (not `Complete`). Kind: stated by the enum value name; consumers not read.

### behavior/policy/flow.rs (204) and flow/row_tests.rs (160)
`CapabilityFlowProjection` (23) builds an index `FlowRows` (machines, envelopes, reaches, owners) once (`OnceCell`, 25, 41) and `reachable_capability_flows` (36-119) traverses the checked call closure (BFS over `pending`), requiring exactly one machine, envelope and reach row per symbol, state rosters equal, each call's target state to have one owner whose symbol equals `call.target_machine` when valid (85-104). `capability_flows` (122-150) requires the capability collection to equal the envelope's. Header (6-9): "without fabricating edges for trait requirements or static machine parameters that lack a concrete machine body": a call with an invalid `target_machine` and no owner passes (`owner.map(..) != call.target_machine.is_valid().then_some(..)` is `None != None` false). Fail-open: that pass-through is stated (header). `row_tests.rs`: differential test indexed vs linear search (79-123).

### behavior/policy/mutation.rs (72), receivers.rs (58), termination.rs (106)
- `mutation.rs`: replays the entry write frame via `resolver.inferred_state_write_frame` and requires equality with the retained frame (56-64).
- `receivers.rs`: `validate_call_receiver_roots` (12-29): for every call with a receiver and a valid authored expression, requires the receiver exist (23) and the root name to keep its resolved head symbol (43-51); `continue` when `call_node` returns `None` (20-22) or the call has no receiver. Cyclic receivers detected by bounding the walk at `expression_count()` (35, 57). Fail-open: `let Some(expression) = ...call_node(...) else { continue }` (20-22): a call whose authored expression is not a call node is skipped. Kind: open.
- `termination.rs` (policy): `termination`, `declared_termination`, `checked_plan` (43-73) requires the plan match typed declaration, entry, envelope; `project_guarantee` (75-106).
Same job elsewhere: `behavior/termination.rs` (below) projects public signatures' termination; both loop over `TerminationGuarantee::Terminates { premises }` and call `progress_profile`/`premise_subject`/`premise_projections`: `policy/termination.rs:81-100` vs `behavior/termination.rs:42-65`, differing in record type (`PackagePolicyProgressPremise` adds `establishment_routes`; the public one also requires `profile.is_public`, line 49).

### behavior/reach.rs (107), behavior/termination.rs (66)
`project_installation_reaches` (14-33), `installation_requirement_identity` (35-80): exactly one of {one trait requirement, one installation-bound requirement machine with supply mode `TopLevelRequirement` or `Boundary`} (55-79), `project_service_row` (82-107): unknown non-empty row ids rejected (87-91). `behavior/termination.rs`: see above. Fail-open: none seen.

### callables/boundary_operators.rs (234), boundary_requirements.rs (181), conformance_order.rs (38), conformances.rs (278), external_policy.rs (66), external_policy/bindings.rs (68)
- `boundary_operators.rs`: validation that a boundary-operator provider plan has exactly one method, row, requirement declaration and authored realization (14-56); `validate_fixed_token_checked_adapter_dispatch_shape` (90-124) supports only two-operand operators among `Add, Subtract, Multiply, Divide, Modulo, Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual, Index` (99-114); `validate_selected_boundary_operator_checked_adapter` (126-184) and `..._external_supply` (186-234).
- `boundary_requirements.rs`: `project_checked_requirement_realization` (16-47); `provider_conformance_bounds_refine` (49-67) a greedy one-to-one matching of provider bounds to requirement bounds (first unmatched bound of equal shape, 55-61); `validate_selected_top_level_requirement_external_supply` (78-181): the same five singleton checks (method, row, requirement, realization) as the boundary-operator version.
- `conformance_order.rs`: sort and duplicate rejection for three vectors (7-38).
- `conformances.rs`: `project_callable_conformances` (57-278): a 220-line branch tree: unresolved trait => top-level requirement realization, or operator realization; resolved trait => exact requirement. Stated limits at 172-177: "realizes lifetime-parameterized operator ... not yet represented by package review"; fixed-token boundary operators with alias or lifetime parameters refused (146-158); operators with `EnsuresForResultCase` contracts refused (178-192).
- `external_policy.rs`: `project_checked_external_supply_policy` (17-64): requires exactly one realization edge (26-32), a binding (`bindings::project`), signature, requirement; and for a top-level requirement checks `provider_conformance_bounds_refine` (43-55).
- `external_policy/bindings.rs`: external binding must be either an exact binding-table identity with no via expression, or an evaluated-via row with none (28-66); anything else "mixes legacy and evaluated supply carriers" (64-66).
Same job elsewhere: the five singleton checks (`plan.schema.declaration.methods`, `plan.rows`, `row_requirements`, `row_realizations`, plus requirement identity equality `plan.schema.identity_text(...) != slot`) are written out three times: `boundary_operators.rs:14-56 + 126-184 + 186-234` and `boundary_requirements.rs:127-174` (the top-level requirement copy inlines them rather than calling `boundary_operator_plan_singletons`). The `plans.len() != provenance.len()` alignment check is also duplicated (`boundary_operators.rs:70-74`, `boundary_requirements.rs:87-91`).
Cases by shape (quoted): `"must contain exactly one schema method"` (boundary_operators.rs:28; boundary_requirements.rs:129), fixed-token operator list (boundary_operators.rs:99-114), `"external leaf has {} realization edges; exactly one is required"` (external_policy.rs:27-31).
Fail-open: `validate_selected_boundary_operator_external_supply` returns `Ok(())` when no selected provider plan lists this machine as an authored realization (`boundary_operators.rs:203-205`, same at `boundary_requirements.rs:108-110`). What gets through: an external leaf that is not a provider-selected realization is not joined to a plan at all. Kind: open (the caller decides; not traced; the sibling `bindings.rs:18-22` requires the machine to be an `ExternalRealization`). `callable machine_callable_identity_text(...).unwrap_or_default()` (`boundary_operators.rs:141-143`, `boundary_requirements.rs:92-94`): a missing identity becomes the empty string, which is then compared with plan text. Kind: open (an empty identity would equal an empty plan identity text).

---------------------------------------------------------------------------------------------------

## Cross-cutting findings (requirement 6/7 summary)

- "Shared/canonical" claims with grep counts:
  - `PackageCompilationScope` (compiler_input.rs header: "Request-local package compilation scope"): 4 production `new` sites (`occurrences.rs:221`, `custody.rs:20`, `build_resolution.rs:252`, `operations/accepted_compilation.rs:548`) (grep `PackageCompilationScope::new`).
  - `package_entry` (selects `main.omg` else `build.omg`): 3 production call sites outside definition (`acquisition.rs:427`, `build_resolution.rs:132`, `operations/accepted_compilation.rs:560`) and one hand-copy in `occurrences.rs:476-481` (grep `package_entry\(` and `"main.omg"`). Other crates have their own main/build lists (`package-compilation/.../project.rs:59`, `build-output/.../staged_output_tree.rs:404`), not read.
  - `require_exact_checked_fact` ("shared by public domains and data definitions"): 2 callers (`api/domains/facts.rs:129`, `api/data/projection.rs:67`) by reading; not grepped by name beyond that.
  - `CompilerIssuedPackageReview` "deliberately no public constructor" (`review_set.rs:6`): constructed inline in `occurrences.rs:742` and `review_cache.rs:113` (both via `pub(super)` fields in the `candidate` module), i.e. 2 construction sites.
- Duplicate jobs inside this unit: (1) shortest root paths: `dependency_paths.rs` vs `compare/paths.rs`; (2) reachability/cycle checks: `graph/mod.rs`, `walk.rs`, `compiler_input.rs`; (3) subject binary + text encoders; (4) review vs policy shape projection for every public declaration kind in review-evidence; (5) three copies of boundary singleton checks; (6) `MAXIMUM_OUTPUT_BYTES` 8 MiB vs 32 MiB.
- Empty inherent impls: `reconcile/mod.rs:79, 87, 147`, `source/mod.rs:128` (read).
- `debug_assert`-only invariants: `graph/mod.rs:64`, `custody.rs:101`, `resolved_closure.rs:125-128`, `resolution.rs:480-485`.

---------------------------------------------------------------------------------------------------

- **Coverage:**
  - Read in full: all 122 files, every line of every range: 21,881 lines (sum of the line-count column; command: `awk -F'\t' '{s+=$4} END {print s, NR}'` on the unit file gave `21881 122`). Files with a first-chunk then second-chunk read: `resolution/tests.rs` (1-60, 60-214), `record_codec.rs` (1-600, 600-1112).
  - Skimmed: 0 lines.
  - Not read: 0 lines.

- **Five things a maintainer most needs to know about this unit:**
  1. The package-manager source subject exists in two serializations (binary `subject/encoding/*`, text `subject/text/*`) plus a validator (`subject/validation/*`); every `SourceLineage`/root-request/resolution variant must be added in all three (`encoding/source.rs:36-125`, `text/source.rs:98-252`, `validation/source.rs:107-132`). Binary versions 7/8/9 and text versions 1/2/3/4 are selected by catalog shape and purpose so old lock identities stay stable (`subject.rs:7-15`, `selection.rs:62-72`, `record.rs:26-29`).
  2. Source resolution is a suspend/resume protocol, not a recursive call: `PendingSourceResolution` (`resolution.rs:84`) is handed to the build-system caller, which executes `Build` for the acquired source and calls `complete`; acquisition keeps the first capture per `(lineage, navigation)` (`acquisition.rs:266-287`) and conflicts are detected after the fact (`resolution.rs:344-382`).
  3. The review pipeline runs two passes (discovery then bound) and carries identical occurrences forward from the same pass, an earlier pass, or a disk cache keyed by SHA-256 of package/purpose/target/profile/entry/compiler identity/inputs/snapshot (`passes.rs:53-119`, `occurrences.rs:499-618`, `key.rs:48-74`); the cache trailer is an unkeyed SHA-256 so it detects corruption, not tampering (`review_cache.rs:197-200`).
  4. Review-evidence projects every public declaration twice (`PackageReview*Shape` in `api/*`, then `PackagePolicy*Shape` in `api/policy/*`) and enforces "exactly one row" by slice patterns plus rederivation checks against retained facts (`evidence.rs:64-91`, `operators.rs:62-69`); unsupported shapes are errors, not skips, in crash expressions (`policy/crash.rs:105-110`), but `owned_identity(...)? else continue` silently omits non-owned declarations (e.g. `constants.rs:34-36`).
  5. Entry-file choice (`main.omg` else `build.omg`) has one helper (`package_entry`) and one hand copy that reads the physical snapshot rather than the captured tree (`occurrences.rs:476-481`), so the compile entry may be chosen from the filesystem while the source bytes come from the capture (`custody.captured_source()`), if they ever diverge (inferred; not traced).

- **What you are unsure about:**
  - Whether `selected_member`'s `.expect` (git/workspace.rs:233), `render_package_diagnostics`'s byte slice (`error.rs:184`), and `old_sources.rs:24-26` `expect` can be reached by untrusted input (not traced).
  - Whether the lock's `occurrences()` always align with `PackageOccurrenceRoster::derive` output (`assembly.rs:73-76` indexes by counts): not traced into `lock/`.
  - The consumers of `CanonicalSourceClosureSubject::package_dependency_projection` mismatch semantics in `checked_custody` (accepted.rs:109-116) versus `catalog` ordinal lookup (`accepted.rs:60-65`): only partly traced.
  - Modules listed in `callables.rs` (`external_supply`, `policy`, `policy_parameters`, `reach_dependency`, `surface`, `target_context`) and `capture/{calling,contracts,input,package_policy,package_review,providers,representation,semantic_bindings,semantics,runtime_permissions,source,terminal_authority_permissions}` are outside this unit; statements about them are inferred from `use` lines only.
  - Whether `DependencyPurpose::ALL` has exactly two members everywhere `[None; 2]`/`purpose_slot` is used (`occurrences.rs:281`): inferred from `PM` code, not from the enum definition.
