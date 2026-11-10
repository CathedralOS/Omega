slice_0120

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0921 | omega/packages/package-manager/src/resolution/graph/accepted.rs:57-74 | CLOSED | Catalog fallback is Build-only and compares full request; duplicate-purpose rows are deduped when projecting the subject. |
| A0922 | omega/packages/package-manager/src/resolution/graph/mod.rs:60-66; omega/packages/package-manager/src/resolution/source/custody.rs:101 | CLOSED | Release builds still hit `validate_source_identity` on lock subjects; `from_validated_parts` only wraps custody already produced by resolution. |
| A0923 | omega/packages/package-manager/src/resolution/graph/subject/catalog.rs:31-35 | CLOSED | `WorkspaceMemberObservation::new` rejects selection/graph length mismatch before catalog projection runs. |
| A0924 | omega/packages/package-manager/src/review/candidate/review_cache.rs:197-200 | REACHABLE | Trailer hashes payload only; cache key omits `build_handoff`, and reuse does not re-derive handoff after decode. |
| A0925 | omega/packages/package-manager/src/review/candidate/error.rs:184; omega/packages/package-manager/src/review/compare/assembly.rs:73-75 | UNDECIDED | Cited sites panic on display or slice bounds; they do not admit compilation or resolution success. |
| A0926 | omega/packages/resolver-execution/src/resolver_execution/path_custody.rs:77 | REACHABLE | If a control root’s `canonicalize()` fails, only lexical `starts_with` guards the stored canonical executable path. |
| A0927 | omega/packages/review-evidence/src/capture/authority.rs:111 | REACHABLE | Unlisted core services and `TimeHostService` get `Ok(None)` and are skipped for dangerous-authority projection. |
| A0928 | omega/packages/review-evidence/src/capture/api/data/evidence.rs:154-155,181-182 | REACHABLE | Missing ref spans on non-data contexts/symbol sets are dropped from rechecked evidence instead of failing closed. |

### A0921

1. Read in full: `admit_locked_issued_source_request` (`omega/packages/package-manager/src/resolution/graph/accepted.rs:42-87`), plus `checked_request` and `request_matches` in the same file (read).
2. Search: `rg 'admit_locked_issued_source_request' omega-rust` → 3 hits (definition at `accepted.rs:42`, re-export `graph/mod.rs:27`, call `resolution/source/acquisition.rs:187` read).
3. Caller trace (read): `acquisition.rs:158-189` `acquire_requests` passes live `purpose`, `ordinal`, and `request` from issued Build rows; prior `admit_locked_source_request` at `acquisition.rs:109-112` when setting locked subject (read call site only, not full `admit_locked_source_request` body beyond cited function).
4. Input on catalog branch: locked `CanonicalSourceClosureSubject`, `SourceAcquisitionOrigin` (lineage, navigation, resolution), Build `purpose`, `ordinal`, and the issued `DependencySourceRequest` when the requesting source is absent from execution graphs (`accepted.rs:56-74` read).
5. **Verdict: CLOSED.** The catalog `find` at `accepted.rs:57-64` omits `purpose`, but `accepted.rs:66-73` returns `Err` unless `purpose == DependencyPurpose::Build` and `request_matches` succeeds against the chosen row. Non-Build catalog use on this path is rejected. Catalog projection dedupes `(observer, purpose, ordinal)` and errors on disagreement (`catalog.rs:56-65` read). I did not exhaust every `CanonicalSourceClosureSubject::recover_text` path for adversarial duplicate rows with identical requests across purposes.

### A0922

1. Read in full: `ResolvedSourceIdentity::new`, `from_validated_parts` (`graph/mod.rs:48-66` read); `PackageSourceCustody::from_resolved_parts` (`custody.rs:87-116` read).
2. Search: `rg '\bfrom_validated_parts\b' omega-rust` → 3 hits (definition `mod.rs:60`, calls `reconcile/resolution.rs:245`, `subject/catalog.rs:46` read).
3. Search: `rg 'from_resolved_parts' omega-rust` → production calls in `acquisition.rs:558`, `git/recorded.rs:90`, `local/recovery.rs:55` (read `acquisition.rs:557-568` only).
4. Caller trace (read): `reconcile/resolution.rs:240-255` builds nodes from `self.accepted` custodies after `collect_conflicts`; `catalog.rs:45-49` wraps observer custody from an executed closure; lock subjects use `validate_source_identity` with `matches_lineage` (`subject/validation/source.rs:83-96` read). `ResolvedSourceIdentity::new` enforces lineage on text recovery (`subject/text/source.rs:78` inferred not read in full).
5. **Verdict: CLOSED.** `debug_assert!` at `mod.rs:64` and `custody.rs:101` is not the production check; release validation is `validate_source_identity` on subject materialization. Production `from_validated_parts` call sites only see custody/resolution pairs already produced by source resolution. I did not read full `reconcile/resolution.rs` or all lock recovery code.

### A0923

1. Read in full: `project` (`catalog.rs:20-85` read), especially the `zip` at `catalog.rs:31-35`.
2. Caller trace (read): observations come from `PackageSourceSelectionEvidence::observations()` (`selection.rs:17-23`), built via `WorkspaceMemberObservation::new` (`workspace_acquisition.rs:32-57`), which errors when `result.demanded_sources().len() != dependencies.len()` (`workspace_acquisition.rs:39-47` read).
3. **Verdict: CLOSED.** Mismatched lengths cannot reach the `zip` through production observation construction. `Root` and `HistoricalGit` expose empty observation lists (`selection.rs:18-20` read), so the zip never runs on mismatched pairs there.

### A0924

1. Read: `CompiledReviewCache::store` trailer (`review_cache.rs:197-200` read), `decode_record` trailer check (`record_codec.rs:848-858` read), `into_review_and_facts` (`review_cache.rs:95-129` read), cache key (`key.rs:48-73` read), cache reuse (`passes/occurrences.rs:557-577` read).
2. Input: bytes loaded from on-disk `.rec` under `compiled_review_cache_dir`; after decode, live `PackageKey` and `ImmutableSourceResolution` for `into_review_and_facts`.
3. **Verdict: REACHABLE.** A file with valid payload SHA-256 trailer and matching stored lineage/resolution/name fields decodes; `build_handoff` is deserialized and carried in `CompilerIssuedPackageReview` without re-derivation (`review_cache.rs:123` read). The cache key digests inputs and snapshot (`key.rs:71-72` read) but not `build_handoff`. `verify_current_source_consumption` only re-checks consumption commitment (`occurrences.rs:121-137` read), not handoff or policy body against a fresh compile. A party that can write the package’s review cache directory could get a wrong handoff accepted on cache hit. I did not trace every writer of `compiled_review_cache_dir`.

### A0925

1. Read: `render_package_diagnostics` (`error.rs:173-195` read, cite `184`), `assembly::packages` slice (`assembly.rs:52-76` read, cite `73-75`).
2. `render_package_diagnostics` search: `rg 'render_package_diagnostics' package-manager` → definition and three `Display` arms in `error.rs` (read arms not all in full).
3. `assembly::packages` caller: `compare/mod.rs:183` (read); `PackageLockTarget::validate` aligns occurrence count with roster (`lock/mod.rs:117-125` read).
4. **Verdict: UNDECIDED** under ACCEPT rubric. These sites abort the process on bad bounds or UTF-8 slicing rather than returning acceptance. `error.rs:184` can panic if `name.len() > 256` and byte index 256 is not a `char` boundary (inferred from Rust str slicing rules). `assembly.rs:73-75` can panic if `old_count` does not match `accepted.occurrences()` length; validated locks prevent that on the path I read. Entry description is accurate about panic risk, not silent acceptance.

### A0926

1. Read in full: `require_outside_roots` (`path_custody.rs:68-93` read, cite `77`).
2. Search: `rg 'require_outside_roots' omega-rust` → 4 hits (`path_custody.rs`, `resolver_execution.rs:40,49,79` read).
3. Caller trace (read): `ResolverExecutionBackend::open` checks authored path then canonicalized executable (`resolver_execution.rs:37-49`); `source-acquisition` passes `resolver_package_controlled_roots` (`executor.rs:155` read). `ExcludedRoots` uses the same partial canonical pattern (`selection.rs:177-193` read).
4. Input: canonicalized resolver executable path and each package-controlled root (cwd plus retained snapshot paths from `resolver_package_controlled_roots` at `selection.rs:216-227` read).
5. **Verdict: REACHABLE.** When `root.canonicalize()` fails (`path_custody.rs:77` read), inside-root detection uses only lexical `path.starts_with(root)`. An executable whose canonical path lies under a package tree but whose path string is not a lexical prefix of a non-canonicalizable root record can pass. `open` still canonicalizes the executable itself (`resolver_execution.rs:41-49` read), matching the entry’s symlink scenario. I did not prove a specific OS layout on a hosted target.

### A0927

1. Read in full: `dangerous_authority_class` (`authority.rs:42-112` read).
2. Search: `rg 'dangerous_authority_class' omega-rust` → 2 hits (`authority.rs:42`, `package_policy/authority.rs:61` read).
3. Caller trace (read): `project` skips services when class is `None` (`package_policy/authority.rs:57-63`); `TimeHostService` appears in `semantic_bindings.rs` but not in `CORE_CRITICAL_SERVICES` or the aggregate loop (`authority.rs:84-108` read).
4. **Verdict: REACHABLE.** A checked `core` service with canonical path not in `CORE_CRITICAL_SERVICES` and not matching `ConsoleExitProcessI32` or `FilesystemHostService` aggregate bindings returns `Ok(None)` (`authority.rs:111` read), so review emits no dangerous-authority row for that reach while other review steps continue. I did not read full policy encoding downstream of `project`.

### A0928

1. Read in full: `rechecked_data_definition_evidence` and `require_rederived_data_definition_facts` (`evidence.rs:64-209` read).
2. Caller: `require_rederived_data_definition_facts` called from `capture/api/data/projection.rs:90` (inferred, file not read in full).
3. Input: compiler-rederived and retained `FactPlan` plus data-definition symbol set; contexts/symbol sets where `facts.refs.span(...)` is `None`.
4. **Verdict: REACHABLE.** For contexts, `None if at_data_definition => return Some(None)` (`evidence.rs:154` read); otherwise `None => return None` (`evidence.rs:155` read) drops the context from the collected vector without failing the whole plan. Same pattern for symbol sets (`evidence.rs:181-182` read). If both retained and rederived plans omit the same non-data context with a missing span, `require_rederived_data_definition_facts` can return `Ok(())` (`evidence.rs:85-89` read) without surfacing that context. I did not read whether the compiler can emit such plans in production.

### What I did not read

- Full `reconcile/resolution.rs`, `record_codec.rs` beyond `decode_record` header, `capture/api/data/projection.rs` beyond the grep hit, `PackageLockTarget` recovery from disk, full `compare/mod.rs`, and `PrimaryGitSelection` capture beyond cited sections.

SLICE COMPLETE
