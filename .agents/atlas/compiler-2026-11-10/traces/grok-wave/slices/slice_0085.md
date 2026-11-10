slice_0085

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0650 | omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/validation.rs:315-327 | CLOSED | Deferred control rows omit bytes at encode time; stage-08 branch validation runs in the same native realization gate before publish. |
| A0651 | omega/compilation/products/src/checked_compilation.rs:162-165 | CLOSED | Empty `root_grant_selectors` means the evaluated build issued no trust roots; skipping `reconstruct_trust_obligations` matches the grant-driven trust contract. |
| A0652 | omega/compilation/products/src/evaluated_build.rs:104-118 | REACHABLE | `same_source_choices` treats native evaluations as equal on `terminal_code` only, so cohort/batch equality can ignore usage and observation digests. |
| A0653 | omega/compilation/products/src/package_build.rs:185-193 | REACHABLE | Post-seal `with_independent_component_descriptions` mutates `inputs` but leaves the sealed `build_handoff` and does not re-run provider settlement verification. |
| A0654 | omega/compilation/products/src/package_build/retention_identity.rs:685-689 | REACHABLE | Invalid `SymbolHandle` values contribute only `digest.bool(false)`, so distinct invalid handles collide in the retention digest. |
| A0655 | omega/compilation/products/src/prepared_build.rs:127 | CLOSED | Sole production caller invokes `adopt_derived_code` only after `shares_terminal_publication` already succeeded. |
| A0656 | omega/compilation/realization/provider-admission/src/admission_source.rs:209-217 | REACHABLE | `check_current` accepts a current cohort that dropped fatal entries, anchor, or installation assumptions relative to the admitted scope. |
| A0657 | omega/compilation/realization/provider-admission/src/provider_admission.rs:437-440 | REACHABLE | Missing plan row or missing identity text for a reached boundary silently drops the no-provider diagnostic via `filter_map` `?`. |

### A0650

1. Read `validate_row` in full at `omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/validation.rs:160-364` (read).
2. Search `\bvalidate_row\(` in `omega-rust/`: 7 hits (read count): stage-07 `validation.rs` (2), stage-07 `row_encoding.rs` test helper (1), stage-08 `validation.rs` (2), `image-elf/import_sections.rs` (2).
3. Read callers `validate_ordinary` (`validation.rs:109-147`) and `validate` (`validation.rs:74-107`); read `validate_optimized_layout_independent_selected_form_encoding` at `function_realization.rs:444-458` chained with stage-08 layout validation (read).
4. Read producer path `encode_row` / `RowRoute::DeferredControlFlow` at `row_encoding.rs:316-347` (read): `draft.footprint` is `None`, so `code` and `footprint` default; cited arms at `validation.rs:315-327` only check `row.address` and `DeferredControl` state, then `Ok(())`.
5. Input reaching the accepting branch: pre-layout `SelectedFormEncodingRow` for `Jump` / conditional branch kinds with `RequiresResolvedBranchLayout`, no address, typically zero/default `code`/`footprint` from the stage-07 encoder.
6. **Verdict: CLOSED** — `function_realization.rs:444-458` runs stage-07 then stage-08 validators; stage-08 `validate_branch` (`08_.../validation.rs:407-422`) checks resolved branch evidence. Deferred rows are not meant to carry byte/footprint obligations at stage 07 (`row_encoding.rs:316-321`, `335-337`).
7. Unsure: whether any production path calls stage-07 `validate` without stage-08; only read `function_realization.rs` among emission gates.

### A0651

1. Read `admit_trust` at `checked_compilation.rs:161-179` (read).
2. Search `\.admit_trust\(` in `omega-rust/`: 4 hits — `product_execution.rs` (3), `product_boundary.rs` (1 string).
3. Read `product_execution.rs:248-252`, `:317-319`, and trust gate on non-terminal path `:314-318` (read); did not read third `admit_trust` site at `:381` body.
4. Input: `CheckedCompilation` whose `ComputedBuildConfig.config.grants` is empty, so `root_grant_selectors()` returns `[]` (`evaluated_build.rs:139-143`, read).
5. **Verdict: CLOSED** — `reconstruct_trust_obligations` is grant-list-driven (`trust-model/admission_settlement.rs:33-50`, read); empty selectors means no root-grant obligations at this gate. Callers treat failure as `CompilationProductError::TrustAdmission` (`product_execution.rs:251-252`).
6. Unsure: whether a program could carry trust declarations that ought to fail without grants while still reaching `admit_trust`; did not trace build evaluation grant emission.

### A0652

1. Read `BuildConfig::same_source_choices` at `evaluated_build.rs:84-126` (read).
2. Search `same_source_choices\(` in `omega-rust/`: 3 hits — `evaluated_build.rs`, `package_build.rs:70`, `prepared_build.rs:121`.
3. Read `same_product_source` (`package_build.rs:64-98`) and `shares_terminal_publication` (`prepared_build.rs:112-124`); read `select_build_batch` (`product_batch.rs:24-28`, `:66-68`).
4. Input: two `BuildConfig` values whose `native_binding_evaluation` and per-binding `evaluation` differ in `usage` / `observation_digest` but share `terminal_code` (`native_evaluation.rs:9-67`, read).
5. **Verdict: REACHABLE** — Smallest harmful case: two build continuations with identical selections and terminal Psi code but different native evaluation usage counters; `same_source_choices` returns true, so they may batch as same product source and share terminal publication via `adopt_derived_code`. Native cache keys still use `identity_digest()` (`retention_identity.rs:287-295`, read), but batching/reuse decisions at `same_source_choices` ignore that.
6. Unsure: whether downstream always re-validates full `NativeBuildEvaluation` before trusting merged publication.

### A0653

1. Read `with_independent_component_descriptions` at `package_build.rs:185-193` and `PackageCompilationInputs::with_independent_component_descriptions` at `package-compilation/package_compilation.rs:443-474` (read).
2. Read `seal` at `package_build.rs:101-155` (read) and build loop `build_execution.rs:268-295` (read).
3. Read `verify_independent_components` only inside `settle_checked_providers` (`provider_settlement.rs:160-187`, `:422-434`, read) — not re-invoked after post-seal input mutation.
4. Input: sealed `PreparedPackageBuild` later receives component descriptions on `inputs` while `build_handoff` stays the pre-mutation handoff; `frontier.products` stores `build.build_handoff().clone()` (`build_execution.rs:295`).
5. **Verdict: REACHABLE** — A build finishing component completion can attach descriptions after `PreparedPackageBuild::seal` without recomputing handoff or re-running `verify_independent_component_descriptions`; a description that would fail verification could remain unchecked on that path.
6. Unsure: whether `review_package_build` or other hooks always re-verify before publish when descriptions are added late (`package_review.rs:168-174`, read only header).

### A0654

1. Read `BuildOutputIdentity::symbol` at `retention_identity.rs:685-707` (read).
2. Search `fn symbol` definition in products: single site `retention_identity.rs:685` (read).
3. Call sites for `self.symbol(` in same file: many field hashes in `config()` and related (read list via grep, did not re-read each caller body).
4. Input: invalid `SymbolHandle` in grants, selections, or native bindings hashed through `symbol()`.
5. **Verdict: REACHABLE** — Two different invalid handles both contribute only `digest.bool(false)` before return (`685-688`); digests collide unless other fields differ, weakening cache-key discrimination for native product retention.
6. Unsure: whether invalid handles can appear in production `BuildConfig` fields after earlier validation.

### A0655

1. Read `adopt_derived_code` at `prepared_build.rs:126-131` and `shares_terminal_publication` at `112-124` (read).
2. Search `adopt_derived_code\(`: 2 hits — definition and `product_batch.rs:68`.
3. Read `product_batch.rs:64-69` (read).
4. Input: `CheckedProductSource` accepted into a batch cohort after `shares_terminal_publication` matched a canonical source.
5. **Verdict: CLOSED** — Production path calls `adopt_derived_code` only when `shares_terminal_publication(&source)` already held (`product_batch.rs:66-68`); `debug_assert!` at `prepared_build.rs:127` duplicates that predicate, not the only guard.
6. Unsure: none for production; release builds rely on the `find` predicate, not the assert.

### A0656

1. Read `check_current` in full at `admission_source.rs:169-222` (read).
2. Search `check_current\(` in `omega/`: 3 hits in `admission_source.rs`; `require_current(`: 2 hits (`admission_source.rs`, `image-emission/entries.rs:5709`, read snippet).
3. Read `require_current` use at `entries.rs:5708-5710` (read); did not read full `entries.rs` emission function.
4. Input: `current` `AdmittedInstalledInterruptCohort` scope paired at emission time where `received` omits fatal entries present in admitted `scope`, clears `context_anchor`, or empties `installation_assumptions` while other fields match (`209-217`, read).
5. **Verdict: REACHABLE** — A substituted cohort weaker than the admitted installation can return `Ok(())` from `check_current` because checks require extra fatal entries in `received` but not missing ones, and anchor/assumption checks are one-sided (`213-217`).
6. Unsure: whether `installed` at emission can diverge from admitted scope in production builds without failing earlier static interrupt admission.

### A0657

1. Read `require_reached_providers` at `provider_admission.rs:421-491` (read).
2. Search `require_reached_providers\(`: 1 hit — `admit_native_providers` at `provider_admission.rs:215` (read).
3. Read `admit_native_providers` caller chain start at `177-221` (read); did not read all upstream native admission callers.
4. Input: each `boundary` in `reached.boundaries` (`reachability.rs:126-136`, read) passed to `filter_map`; if `plan.boundary_machines.find` fails (`437-440`) or `plan.identities.text(declaration.identity)` is `None` (`472`), the boundary produces no diagnostic.
5. **Verdict: REACHABLE** — A reachable boundary with no matching plan row or missing identity text bypasses the “no provider selected” error while other boundaries still check normally.
6. Unsure: whether terminal plan invariants always place every reachable boundary id in `boundary_machines` with resolvable identity text; did not prove from `AuthorityGraph` construction alone.

### What I did not read

- Full bodies of stage-07 `validation.rs` helpers below `validate_row` (e.g. `validate_baseline`, ISA decoders) except as cited.
- Third `admit_trust` call site in `product_execution.rs` (~381) and full `admit_permissions` paths.
- All upstream callers of `admit_native_providers` beyond `provider_admission.rs:177-221`.
- Full `image-emission/entries.rs` private-handler installation path around `require_current`.
- `AuthorityGraph` construction and plan/`boundary_machines` invariants beyond `reachability.rs:126-136`.
- Test-only callers of stage-07 `validate_row` in `row_encoding.rs` and `image-elf/import_sections.rs`.

SLICE COMPLETE
