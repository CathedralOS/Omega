# slice_0123

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0945 | omega-rust/omega/packages/review-evidence/src/record/package/callable_policy/validation/signature_expressions.rs:125-148 | REACHABLE | Empty or unvalidated `ProofView` call names pass structural validation on recovery. |
| A0946 | omega-rust/omega/packages/review-evidence/src/record/package/provider_policy/validation.rs:30 | REACHABLE | Empty `plan.target` skips the target-mismatch arm and can pass `validate_canonical_structure`. |
| A0947 | omega-rust/omega/packages/review-evidence/src/record/package/callable_policy/validation/structural.rs:106-112 | REACHABLE | Non-empty but ill-formed integer literal text passes; float arms carry no text check. |
| A0948 | omega-rust/omega/packages/review-evidence/src/record/baseline/validation.rs:127-131 | CLOSED | `assumed_bindings` are opaque disclosure strings; baseline structural validation does not police them by design. |
| A0949 | omega-rust/omega/packages/source-acquisition/src/git/cache/custody.rs:187-193 | REACHABLE | On non-Unix, multiply linked repository files are not rejected at this site. |
| A0950 | omega-rust/omega/packages/source-acquisition/src/git/cache/retry.rs:41 | CLOSED | `RecordFileRoot::read` later refuses non-regular `source.identity`; cache is not accepted as verified. |
| A0951 | omega-rust/omega/packages/source-acquisition/src/identity/resolution.rs:77-87 | REACHABLE | Family-only lineage match lets unrelated Git resolutions pass `validate_source_identity`. |

## A0945

**Cited site (read):** `signature_expressions.rs:125-148` — `Call` arm validates only `PackageReviewContractCallTarget::Nominal` with `validate_resolved()`; other targets fall through after receiver/argument walks.

**Enclosing function (read):** `expression` in `signature_expressions.rs:12-171` validates contract expression shape under a `Scope`.

**Callers (read):** Grep `\bexpression\(` under `review-evidence/src/record/package/callable_policy` — `signature_contracts.rs` (facts, proposition args), `signature.rs:175` (`callable` entry), recursive arms in `signature_expressions.rs`. Upward: `signature::validate` → `PackagePolicyCallables::validate_canonical_structure` (`callable_policy/validation.rs:110`) → `PackagePolicyBaseline::validate_canonical_structure` (`baseline/validation.rs:36`) → `recover_canonical_with_usage` (`encoding/recovery/policy/baseline.rs:53-55`).

**Input (read):** Canonical baseline bytes from recovery containing a contract `Call` with `ProofView { name: "" }` (recovery `call_target` tag 4 uses `reader.string()` at `encoding/recovery/policy/expressions.rs:204-206` with no empty guard; `reader.rs:122-124`).

**Verdict reasoning (read):** `BuiltinFunction` and `ByteSequencePredicate` are closed enums validated at decode (`expressions.rs:195-203`); the digest gap is the missing check on non-nominal targets in structural validation. `ProofView` carries a free `String` (`record/contracts/expressions.rs:150`) with no `text()` call in the `Call` arm. A recovered policy whose only defect is an empty `ProofView` name should be rejected but clears `validate_canonical_structure`.

## A0946

**Cited site (read):** `provider_policy/validation.rs:29-39` — invalid-plan disjunction uses `(!plan.target.is_empty() && plan.target != self.target.target_name())`, so empty `plan.target` never trips the target clause.

**Enclosing function (read):** `PackagePolicySelectedProviders::validate_canonical_structure` at `validation.rs:25-127`.

**Callers (read):** Grep `selected_providers.validate_canonical_structure` — `baseline/validation.rs:37`; recovery components via `recover_canonical_with_usage` (`encoding/recovery/policy/baseline.rs:54`).

**Input (read):** Forged or recovered provider policy where one `PackagePolicyProviderPlan` has `target: String::new()` while `methods`, `rows`, and `schema_declaration` are otherwise consistent with `self.target`.

**Verdict reasoning (read):** Capture normally sets `target` from retained plans (`capture/providers/policy.rs:209`) with non-empty target strings from bindings (`capture/providers/policy/bindings.rs:57,74,270,277`), but structural validation explicitly permits empty. Such a row should be rejected if plan target must always spell the profile name, yet passes at `validation.rs:27-40`.

## A0947

**Cited site (read):** `structural.rs:106-112` — `IntegerLiteral` rejects only `canonical_text.is_empty()`; `Float32Literal` / `Float64Literal` return `Ok(())` without inspecting payload.

**Enclosing function (read):** `scalar` in `structural.rs:70-127`; reached from `validate` (`structural.rs:30-33`) on boolean/scalar comparisons.

**Callers (read):** Grep `structural::validate` — sole production site `callable_policy/validation/behavior.rs:136` for `structural_runtime_requirements` on crash policy.

**Input (read):** Recovered callable crash structural requirement with `IntegerLiteral { canonical_text: "not-a-decimal-integer", .. }` after decode (`encoding/recovery/policy/structural_expressions/scalars.rs:35-37`); float literals are raw `u32`/`u64` tags (`scalars.rs:73-74`) with no textual canonical form at this validator.

**Verdict reasoning (read):** Digest claim matches integer text; floats store bits only (`authority_expressions.rs:130-131`). Non-empty garbage integer text should be rejected if literals must be canonical decimal spellings, but passes `structural.rs:106-110`.

## A0948

**Cited site (read):** `baseline/validation.rs:127-131` checks `restricted_build_requests` target only; no loop over `assumed_bindings`.

**Enclosing function (read):** `PackagePolicyBaseline::validate_canonical_structure` at `validation.rs:23-148`.

**Callers (read):** Grep `validate_canonical_structure` on baseline — `encoding/recovery/policy/baseline.rs:53-55` after decoding `assumed_bindings: reader.sequence(1, |r| r.string())?` (`baseline.rs:49`).

**Closing path (read):** `record/baseline.rs:6-7,47` documents `assumed_bindings` as build-host request strings in issue order, not live grants. `capture/package_policy/assumed_bindings.rs:6-8` states audit classification without vetoing accepted links. Encoding writes strings opaquely (`encoding/encode/policy_rows/assembly.rs:269-283`). No other `record/baseline/validation` arm inspects binding text.

**Verdict reasoning (read):** Arbitrary UTF-8 strings in `assumed_bindings` are accepted at recovery and structural validation by design; rejection is not an obligation at this layer.

## A0949

**Cited site (read):** `custody.rs:187-193` — `#[cfg(not(unix))] verify_git_regular_file_link_count` returns `Ok(())` unconditionally.

**Enclosing function (read):** `verify_retained_git_regular_file` at `custody.rs:150-169` always calls link-count check; tree walk `verify_git_repository_tree_from_open_root` at `custody.rs:72-148` invokes it for each file.

**Callers (read):** Grep `verify_git_repository_tree_from_open_root` — `git/cache/repository.rs:186` inside `VerifiedGitRepository::verify_current`.

**Platform (read):** `custody/platform.rs` enforces owner/mode (Unix only at `53-85`, stub at `78-85`), Windows file identity (`189-214`), not hard-link count. Module header at `custody.rs:5-6` documents `nlink != 1` rejection on Unix only.

**Input (read):** On a non-Unix host, a bare Git cache tree containing a regular file with multiple directory entries (hard links) reaches `verify_git_regular_file_link_count` during `verify_current`.

**Verdict reasoning (read):** Such a tree should be rejected per the Unix rule at `custody.rs:178-182` but is accepted on non-Unix at `custody.rs:187-193`. `platform.rs` does not close the gap (read).

## A0950

**Cited site (read):** `retry.rs:40-41` — `entry.symlink_metadata(GIT_CACHE_METADATA)` with `Ok(_) => return Ok(false)` treats any present path (file, symlink, or directory) as “complete.”

**Enclosing function (read):** `discard_incomplete_entry` at `retry.rs:27-63`.

**Callers (read):** Grep `discard_incomplete_entry` — 2 refs in `git/resolution/acquisition.rs` (import `23`, call `155`).

**Downstream (read):** `VerifiedGitRepository::verify_current` reads metadata via `read_bounded_cache_record_from_open_directory` → `RecordFileRoot::read` (`custody/tree.rs:73-97`). `platform-custody/src/record_file.rs:112-113` returns `NotRegularFile` for symlinks and directories.

**Input (read):** Cache entry directory missing a real metadata file but with a directory or symlink named `source.identity` (`limits.rs:18`).

**Verdict reasoning (read):** `discard_incomplete_entry` misclassifies the entry as complete (read), but `verify_current` does not accept bogus metadata bytes (read). Acceptance as a verified snapshot is closed at `record_file.rs:112-113`.

## A0951

**Cited site (read):** `resolution.rs:77-87` — `matches_lineage` pattern-matches only `SourceLineageFamily` (Git vs Workspace vs ExternalLocal).

**Enclosing function (read):** `ImmutableSourceResolution::matches_lineage` in `resolution.rs:77-87`.

**Callers (read):** Grep `matches_lineage` in `omega-rust` — 5 matches: definition `resolution.rs:1`; `package-manager/.../graph/mod.rs:53,64`; `.../source/custody.rs:101`; `.../subject/validation/source.rs:90`.

**Upward (read):** `validate_source_identity` (`subject/validation/source.rs:83-96`) returns error only when `matches_lineage` is false — same family suffices. `ResolvedSourceIdentity::new` (`graph/mod.rs:49-57`) uses the same predicate; `from_validated_parts` (`graph/mod.rs:60-66`) skips it except `debug_assert!`. Closure validation (`subject/validation/closure.rs:41-42,71,80`) calls `validate_source_identity` only. Text `read_source` (`subject/text/source.rs:78-79`) maps lineage mismatch to `matches_lineage` failure, not commit-to-locator binding.

**Lineage families (read):** `identity/git.rs:60-64` maps GitHub, GitLab, and Git lineages to `SourceLineageFamily::Git`.

**Input (read):** A canonical closure text or catalog row whose `PackageKey` names one Git remote but whose `ImmutableSourceResolution::Git { commit, tree, content }` triple was derived from a different repository, with internally consistent content digest.

**Verdict reasoning (read):** The subject should be rejected because resolution does not belong to the named lineage, yet passes `validate_source_identity` and `ResolvedSourceIdentity::new` when both sides are Git-family (read).

## What I did not read

- Full bodies of `signature::validate`, `VerifiedGitRepository::open`, and `invalidate_git_cache_entry_from_open_parent` beyond the acquisition excerpt at `acquisition.rs:130-179`.
- `platform_custody::record_file` beyond `read_optional` (`record_file.rs:100-119`).
- Unit tests cited in area_06 digest for custody link count and cache retry.
- `git show 457adf4a11f` line-by-line comparison (current checkout lines matched cited symbols at read).

SLICE COMPLETE
