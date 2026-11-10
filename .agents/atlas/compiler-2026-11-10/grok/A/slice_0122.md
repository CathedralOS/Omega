# slice_0122

Area digest preamble (read): Area 6 is terminal-runtime and packages under `omega-rust/omega/` with prefix `RE` → `packages/review-evidence/src` (`digests/area_06.md:1-17`). Rows A0937–A0944 are the **review-evidence (034, 035, 036) accept cluster** for GROUP review-evidence (034, 035, 036) (`digests/area_06.md:236-254`). Citations pinned at `457adf4a11f`; cited line ranges matched on read in the current checkout unless noted.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0937 | semantic_dependencies.rs:143-145 | REACHABLE | Invalid symbol handles in the nominal walk are skipped without error, so dependency edges can be omitted from policy. |
| A0938 | package_review.rs:53-63 | REACHABLE | When the build handoff has no `selection_observation`, the observation digest cross-check does not run. |
| A0939 | application_realizations.rs:339-346 | REACHABLE | Empty-span boundary expressions with no authored selection return false and the use is dropped from policy, not rejected. |
| A0940 | policy/rows.rs:111-114; intrinsics.rs:264-265 | REACHABLE | Unsupported intrinsic rows can enter policy with no execution identity while the application path requires a closed identity. |
| A0941 | semantic_bindings.rs:120-122 | CLOSED | Non-singleton `ProgramEntry` boundary definitions only skip a binding candidate; ambiguity still errors. |
| A0942 | ownership.rs:145-155 | REACHABLE | Missing `source_file` for the provenance span is treated as product scope via `is_none_or`. |
| A0943 | framing.rs:73-75 | UNDECIDED | Non-digit `visit_name` paths return success without membership descent; full membership call graph not read. |
| A0944 | binding_validation.rs:180 | REACHABLE | `ToolchainSource` owner with `package: None` always passes `matches_owner` during structural validation. |

## A0937

**Cited site (read):** `omega-rust/omega/packages/review-evidence/src/capture/package_policy/semantic_dependencies.rs:143-145` — in `nominal_declarations`, `Visit::Symbol` does `if !symbol.is_valid() { continue; }` before custody checks.

**Enclosing function (read):** `nominal_declarations` (`semantic_dependencies.rs:125-295`) DFS-walks symbols and types and returns `Ok(output)` of `PackageReviewNominalIdentity` rows; invalid symbols are skipped rather than rejected.

**Callers (search):** pattern `\bnominal_declarations\b` in `omega-rust` — 2 lines, both in `semantic_dependencies.rs` (definition `:125`, call `:86`).

**Upward trace (read):** `semantic_dependencies::project` (`:26-122`) calls `nominal_declarations(compilation, fact.dependency)?` for non-cleanup-machine facts (`:82-87`); derived rows must match checked rederivation (`:31-36`), but an empty or partial `nominal_declarations` result simply adds no dependency rows for that fact. `project_checked_package_policy` (`capture/package_policy.rs:88`) always includes the vector in the baseline (`:103`) without requiring every fact to emit a row. `project_package_review` assembles policy via `project_checked_package_policy` (`capture/package_review.rs:71-72`).

**Input that reaches the branch:** A retained semantic-dependency fact whose type walk reaches a `SymbolHandle` that is not valid (including nested `Named` symbols during `Visit::Type`, not only the root).

**Smallest program (plain words):** A checked package whose semantic-dependency graph points through a stale or default symbol handle; policy projection drops that subgraph silently and publishes a package policy missing dependency edges that should have blocked or exposed the edge.

## A0938

**Cited site (read):** `omega-rust/omega/packages/review-evidence/src/capture/package_review.rs:53-63` — `build_handoff.selection_observation().is_some_and(|original| Some(original) != checked.build().observation_summary.as_ref().map(|summary| summary.identity().digest()))` only runs the comparison when the handoff carries `Some` observation.

**Enclosing function (read):** `project_package_review` (`package_review.rs:30-103`) rejects on `SourceConsumptionMissing`, selected build-machine identity, handoff/package identity, policy budget, and semantic-binding projection errors.

**Callers (search):** pattern `\bproject_package_review\b` in `omega-rust` — 5 matching lines: definition `capture/package_review.rs:30`, re-exports `capture.rs:22`, `lib.rs:34`, production call `omega/compilation/compiler/src/checked/package_review.rs:86`.

**Upward trace (read):** `CompilationSession::review_package_build` (`checked/package_review.rs:75-92`) checks the package, then calls `project_package_review`. Handoff construction can leave `selection_observation: None` (`package-compilation/build_handoffs.rs:79`, read). Provider activation admission requires `bundle.selection_observation().is_none()` to error when validating activations with selections (`package-compilation/provider_selections.rs:102-108`, read), but that gate is separate from `project_package_review`. Checked compilation can retain `build().observation_summary` regardless (`package_review.rs:59-61`, read).

**Input that reaches the branch:** A package build whose handoff has no `selection_observation` while the checked compilation still holds a build observation summary digest.

**Smallest program (plain words):** A package review where the build handoff omits the selection-observation digest but the checked build record still carries an observation summary; review evidence is assembled without comparing those digests and the review product is accepted.

## A0939

**Cited site (read):** `omega-rust/omega/packages/review-evidence/src/capture/providers/application_realizations.rs:338-346` — when `span.start >= span.end`, the function requires an authored selection of the expected kind; if `!has_authored_selection`, it returns `Ok(false)`.

**Enclosing function (read):** `expression_is_owned_by_package` (`application_realizations.rs:330-374`) otherwise resolves `source_file` and compares package identity.

**Callers (search):** pattern `\bexpression_is_owned_by_package\b` in `omega-rust` — 6 matching lines: definition `application_realizations.rs:330`, calls `application_realizations.rs:55-62`, `:152-160`, and `compiler_intrinsics.rs:22` (import), `:150` (call in filter).

**Upward trace (read):** `project_boundary_application_realizations` (`application_realizations.rs:30-123`) `continue`s when `expression_is_owned_by_package` is false (`:55-63`, `:152-160`); the loop still succeeds and returns staged realizations. `project_checked_boundary_application_policy` (`capture/providers/boundary_policy.rs:36-39`) embeds those realizations into the baseline (`capture/package_policy.rs:89-90`). Failing paths return diagnostics for malformed realizations (`:102-105`), not for skipped ownership.

**Input that reaches the branch:** A boundary operator application expression whose stored source span is empty and whose authored-selection ledger has no matching selection for the use kind.

**Smallest program (plain words):** A package that applies a boundary operator at an expression site the compiler retained but left without a non-empty span and without an authored-selection entry; review drops that application from boundary policy instead of rejecting package review.

## A0940

**Cited sites (read):** `capture/providers/policy/rows.rs:111-114` — when `realization_symbol` is `None` and `retained.row_compiler_intrinsic_executions[index].is_none()`, `compiler_intrinsic_execution` is set to `None` without calling the projector. `capture/providers/intrinsics.rs:264-265` — `SelectedCompilerIntrinsicExecutionIdentity::Unsupported` with `retained: None` yields `Ok(None)` in `reconcile_compiler_intrinsic_execution`.

**Enclosing function (read):** Policy row projection closure in `policy/rows.rs` (read `:70-128`); contrast `application_realizations/compiler_intrinsics.rs:264-280` — `project_compiler_intrinsic_execution(...)?` followed by `.or_evidence(...)` requires a closed execution identity for actual intrinsic applications.

**Callers (search):** pattern `\bproject_compiler_intrinsic_execution\b` in `omega-rust` — 4 matching lines: definition `intrinsics.rs:16`, policy call `policy/rows.rs:112-113`, application call `compiler_intrinsics.rs:264`.

**Upward trace (read):** Selected-provider policy is built in `project_checked_package_policy` (`package_policy.rs:81-82`). Structural validation allows `CompilerIntrinsic` bindings without requiring `row.compiler_intrinsic_execution` to be `Some` (`record/package/provider_policy/binding_validation.rs:68-72`, read). Boundary validation only requires execution match for `ExactCompilerIntrinsic` realizations (`record/package/boundary_policy/validation.rs:134-146`, read); if no such realization is projected, the row can remain with `None` execution.

**Input that reaches the branch:** A selected provider plan row for an unsupported compiler intrinsic where policy projection supplies no retained execution and no package-owned boundary application is projected (or the application path is not taken).

**Smallest program (plain words):** A package whose provider plan includes an unsupported compiler-intrinsic binding without a closed execution sidecar and without a boundary application realization that would force rejection; package policy and review projection succeed with an intrinsic row that omits execution identity.

## A0941

**Cited site (read):** `capture/semantic_bindings.rs:120-122` — `let [definition] = definitions.as_slice() else { continue; }` when iterating `program_entry_service_requirements`.

**Enclosing function (read):** `candidate_service_bindings` (`semantic_bindings.rs:88-156`) builds host-service binding candidates and returns `SemanticBindingProjectionError::Ambiguous` when multiple packages qualify (`:132-139`).

**Callers (search):** pattern `\bcandidate_service_bindings\b` in `omega-rust` — 6 matching lines; production call `capture/package_review.rs:80-82` inside `project_package_review`.

**Upward trace (read):** Skipping leaves one fewer `SemanticBindingReviewCandidate` in `PackageReviewEvidence` (`package_review.rs:86-98`); it does not add a false binding or bypass `checked.execution().candidate_service_binding` (`semantic_bindings.rs:141-144`) for the paths that do match. `ProgramEntry` requirements with zero or multiple matching boundary trait definitions therefore omit nomination only.

**Input that reaches the branch:** A consumer package with a `ProgramEntry` service requirement path that does not match exactly one boundary trait definition in the traits table.

**Smallest program (plain words):** Not applicable for wrongful acceptance; the branch withholds a nominated service binding candidate rather than accepting an incorrect one.

**Verdict rationale (read):** Effect is omission-only; ambiguity still fails closed (`:132-139`).

## A0942

**Cited site (read):** `capture/semantics/declarations/ownership.rs:145-155` — `is_product_scope_instance` ends with `.and_then(... source_file(span)).is_none_or(|file| file.dependency_scope != DependencyScope::Build)` (`:153-155`); digest anchor `145` is the function start (re-located from digest wording).

**Enclosing function (read):** `is_product_scope_instance` requires `is_checked_source_instance` first (`:149-150`).

**Callers (search):** pattern `\bis_product_scope_instance\b` in `omega-rust` — 5 matching lines; production uses `ownership.rs:41` (`owned_identity`) and `capture/api/operators.rs:73-74` (`project_public_operators`).

**Upward trace (read):** `project_public_operators` (`operators.rs:70-78`) includes public operators only when `is_product_scope_instance` is true; `owned_identity` filters declarations for package-owned identities (`ownership.rs:36-45`). A symbol with a provenance span but no `source_file` resolves `is_none_or` to true, so Build scope is not required.

**Input that reaches the branch:** A declaration symbol selected in checked source with valid provenance span coordinates but no retained `source_file` mapping for that span.

**Smallest program (plain words):** A build-scoped declaration handle that still passes checked-source instance tests but has no resolvable source file; it is reviewed as product-scope API surface instead of being excluded as build-only.

## A0943

**Cited site (read):** `encoding/encode/membership/framing.rs:72-75` — inside `visit_name`, when the name does not start with an ASCII digit, the nested closure returns `Ok(())` without further field parsing or calling `runtime`.

**Enclosing function (read):** `visit_name` (`framing.rs:68-124`) parses digit-prefixed structured names; contrast `visit_type` (`:50-55`) which calls `self.runtime(identity)` for non-digit type identities (read).

**Callers (search):** pattern `\bvisit_name\b` under `encoding/encode/membership` — 7 matching lines in `framing.rs` and `visitor.rs:138`, `:170` (`Observer::nominal_path`, `embedded_name`, read).

**Upward trace (partial, read):** `Visitor::runtime` (`visitor.rs:62-80`) runs `visit_type_identity_package_owners` and can return `UnknownPackage` / `InvalidPolicy`. For non-digit names, `visit_name` does not invoke that path. Module header states membership verification purpose (`framing.rs:1`, read). Digest notes the external `visit_type_identity_package_owners` visitor was not traced (`digests/area_06.md:253`); `encoding/encode/membership.rs:24-34` entry `validate_package_membership` not read in full.

**Input that reaches the branch:** A nominal path or embedded name in encoded policy text that does not begin with a digit and is passed to `visit_name`.

**Undecided:** Whether policy encoding can place package-owner-bearing text only through digit-prefixed structured names (so this branch is harmless) or whether opaque non-digit names can appear in positions that should enforce closure membership without being checked elsewhere.

## A0944

**Cited site (read):** `record/package/provider_policy/binding_validation.rs:174-182` — `matches_owner` returns `true` for `(PackageReviewNominalOwner::ToolchainSource(_), None)`.

**Enclosing function (read):** `matches_owner` is used from `validate_producer` (`:167-171`) and provider structural validation (`record/package/provider_policy/validation.rs:64`, `:73`, read).

**Callers (search):** pattern `\bmatches_owner\b` in `review-evidence` — 5 matching lines, all in `binding_validation.rs` and `provider_policy/validation.rs`.

**Upward trace (read):** `PackagePolicyProviderRow::binding.validate_canonical_structure` (`binding_validation.rs:32-72`) does not re-derive toolchain digest identity. `project_checked_package_policy` returns the baseline without calling `validate_canonical_structure` (`capture/package_policy.rs:111`). Recovery re-encodes and validates (`encoding/recovery/policy/baseline.rs:54`, cited by search only, not read in full).

**Input that reaches the branch:** Structural validation of a provider row or native producer where `producer.package` / `plan.realizing_package` is `None` and the declaration owner is `ToolchainSource(_)`.

**Smallest program (plain words):** A recovered or hand-built policy provider row with toolchain-source nominal owner and no realizing package context; owner consistency checks accept any toolchain digest label without tying it to the reviewing closure.

## What I did not read

- `omega-rust/omega/packages/review-evidence/src/encoding/encode/membership.rs` (`validate_package_membership` and encoder traversal) in full.
- `visit_type_identity_package_owners` implementation outside `review-evidence`.
- Remainder of `nominal_declarations` beyond the DFS structure already used for A0937 (lines after `:295` are EOF).
- Full `check_package_build` / handoff pairing logic beyond cited `build_handoffs.rs` and `provider_selections.rs` fragments.
- `derive_checked_semantic_dependencies` in `typed-trees-to-checked-trees` for whether invalid symbol handles can appear in checked facts.

SLICE COMPLETE
