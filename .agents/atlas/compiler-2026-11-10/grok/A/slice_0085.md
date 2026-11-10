# slice_0085

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0650 | omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/validation.rs:315 | REACHABLE | Deferred branch rows only check state and address; arbitrary `code`/`footprint` still pass S07. |
| A0651 | omega-rust/omega/compilation/products/src/checked_compilation.rs:162 | REACHABLE | Empty `root_grant_selectors` returns `Ok(())` and never runs trust reconstruction. |
| A0652 | omega-rust/omega/compilation/products/src/evaluated_build.rs:104 | REACHABLE | Native binding sameness ignores evaluation fields beyond `terminal_code`. |
| A0653 | omega-rust/omega/compilation/products/src/package_build.rs:185 | REACHABLE | `inputs` change after `seal` without rebuilding `build_handoff`. |
| A0654 | omega-rust/omega/compilation/products/src/package_build/retention_identity.rs:685 | REACHABLE | Invalid symbol handles contribute only `digest.bool(false)`. |
| A0655 | omega-rust/omega/compilation/products/src/prepared_build.rs:127 | REACHABLE | `adopt_derived_code` equivalence is guarded only by `debug_assert!`. |
| A0656 | omega-rust/omega/compilation/realization/provider-admission/src/admission_source.rs:209 | REACHABLE | Current cohort may drop fatal entries, anchor, or assumptions and still pass `check_current`. |
| A0657 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission.rs:472 | REACHABLE | Missing boundary identity text skips the reached-provider diagnostic via `filter_map`. |

## A0650

Digest (read): area_04.md:241 — `S07/validation.rs:315-327`: deferred row `code` and `footprint` are not examined.

Opened `validate_row` at `validation.rs:160-327` (read). For `Jump` and conditional branch kinds, `:315-327` checks `row.address` is absent and `row.state` is `DeferredControl { RequiresResolvedBranchLayout }`, then returns `Ok(())` without reading `row.code`, `row.footprint`, or the `encoded` store (parameters at `:166-167` unused on this arm).

Producer (read): `row_encoding.rs:316-347` — `RowRoute::DeferredControlFlow` builds a draft without bytes; `code`/`footprint` default via `Default::default()` at `:335-337`.

Caller search: `rg '\bvalidate\(' omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding` → 2 matches (read): definition `validation.rs:74`, call `lib.rs:119`.

Downstream S08 (read): `08_.../validation.rs:407-422` — deferred pre-layout rows call `validate_branch` and do not require `candidate.bytes == encoded.bytes(pre.code)` (contrast `Encoded` arm at `:341-349`).

Input in plain words: a stage-07 `SelectedFormEncoding` artifact whose branch row still has deferred control state but whose `code` or `footprint` fields point at non-empty or wrong encoded payload (not what the producer emits).

Verdict **REACHABLE**: that artifact should fail independent S07 replay but is accepted; S08 does not reconcile `pre.code` on the deferred arm.

## A0651

Digest (read): area_04.md:244 — `PR/checked_compilation.rs:162-165`: no root grants, `reconstruct_trust_obligations` is skipped.

Opened `admit_trust` at `checked_compilation.rs:161-178` (read). `:162-164` — when `self.build().config.root_grant_selectors()` is empty, returns `Ok(())` without calling `trust_model::reconstruct_trust_obligations` at `:166-177`.

`root_grant_selectors` (read): `evaluated_build.rs:139-143` maps `self.grants` to selector strings only.

Caller search: `rg '\badmit_trust\b' omega-rust` → 5 matches (read): definition `checked_compilation.rs:161`; uses `product_execution.rs:251`, `:318`, `:381`; string mention `product_boundary.rs:237`.

Trust reconstruction (read): `trust-model/admission_settlement.rs:33-50` — `reconstruct_trust_obligations` runs only when `admit_trust` invokes it; work is keyed on `root_grants`.

Input in plain words: a checked compilation that retains non-empty `trust_declarations` in execution custody but whose evaluated build config has an empty `grants` list, so no selectors are passed to trust admission.

Verdict **REACHABLE**: trust obligations that should be reconstructed against declarations and provider plans are not checked, yet product paths call `admit_trust` and proceed on `Ok(())`.

## A0652

Digest (read): area_04.md:245 — `PR/evaluated_build.rs:104-118`: native binding evaluations compared by `terminal_code` only.

Opened `same_source_choices` at `evaluated_build.rs:84-126` (read). `:104-110` compares optional `native_binding_evaluation` using only `evaluation.terminal_code`. `:111-118` requires matching `native_bindings` length and pairs `left.evaluation.terminal_code == right.evaluation.terminal_code` without comparing other evaluation fields.

Caller search: `rg '\bsame_source_choices\b' omega-rust` → 3 matches (read): definition `evaluated_build.rs:84`; calls `prepared_build.rs:121` (`shares_terminal_publication`), `package_build.rs:70` (`same_product_source`).

Input in plain words: two build configs whose native binding evaluations differ in every field except `terminal_code` (same length and pairing of selections).

Verdict **REACHABLE**: they are treated as the same source choices, so equivalence and reuse gates that call `same_source_choices`/`shares_terminal_publication` accept the mismatch.

## A0653

Digest (read): area_04.md:246 — `PR/package_build.rs:185-193`: inputs replaced after sealing without recomputing the handoff.

Opened `PreparedPackageBuild` at `package_build.rs:18-26` (read) — stores both `inputs` and sealed `build_handoff`.

`seal` at `:101-154` (read) computes `build_handoff` from `inputs` passed into `seal`, then stores it in the returned struct at `:147-154`.

`with_independent_component_descriptions` at `:185-192` (read) replaces `self.inputs` via `PackageCompilationInputs::with_independent_component_descriptions` but does not update `self.build_handoff`.

Caller search: `rg 'with_independent_component_descriptions' omega-rust` → 6 matches (read): `package_build.rs:185`; `package-compilation/package_compilation.rs:443` (mutates inputs only); `build_execution.rs:269` (on `PreparedPackageBuild` after build completion); `package_review.rs:169` (before `review_package_build`); `compiler/.../package_compilation.rs:229`; `product_boundary.rs:255` (string only).

Input in plain words: a sealed `PreparedPackageBuild` that later attaches independent component descriptions to `inputs` while consumers still read the handoff frozen at seal time.

Verdict **REACHABLE**: downstream review or compilation can observe stale handoff relative to updated `inputs` (for example `package_review.rs:168-173` calls `with_independent_component_descriptions` then `review_package_build` with unchanged `build_handoff`).

## A0654

Digest (read): area_04.md:247 — `PR/package_build/retention_identity.rs:685-689`: invalid symbols hash identically.

Opened `symbol` at `retention_identity.rs:685-706` (read). `:686-688` writes `digest.bool(symbol.is_valid())` and, when invalid, returns `Ok(())` without encoding symbol identity, kind, or spans.

Caller search: `rg 'fn symbol\(' omega-rust/omega/compilation/products/src/package_build/retention_identity.rs` → 1 definition (read); `rg '\.symbol\(' .../retention_identity.rs` → 24 call sites (read) at lines including `:135`, `:201`, `:586`, etc.

Input in plain words: two distinct invalid `SymbolHandle` values hashed at the same retention step where `symbol` is the only distinguishing call.

Verdict **REACHABLE**: both contribute only `false` to the digest stream, so retention identity cannot distinguish which invalid handle was present.

## A0655

Digest (read): area_04.md:248 — `PR/prepared_build.rs:127`: `adopt_derived_code` guards equivalence only with `debug_assert!`.

Opened `CheckedProductSource` at `prepared_build.rs:111-131` (read). `adopt_derived_code` at `:126-131` clones terminal publication fields from `canonical` after `debug_assert!(canonical.shares_terminal_publication(self))` at `:127`; no `Result` or runtime check in release builds.

Caller search: `rg '\badopt_derived_code\b' omega-rust` → 2 matches (read): definition `prepared_build.rs:126`; call `compiler/src/checked/product_batch.rs:68`.

`shares_terminal_publication` (read): `prepared_build.rs:112-124` — includes `same_source_choices` on build config among other fields.

Input in plain words: a batch reuse path that passes a non-canonical `CheckedProductSource` as `canonical` when `shares_terminal_publication` would be false.

Verdict **REACHABLE**: in non-debug builds the assert is omitted and derived code is adopted anyway, so terminal publication can be merged from a non-equivalent source.

## A0656

Digest (read): area_04.md:251 — `PA/admission_source.rs:209-217`: current cohort may drop fatal entries, anchor, or assumptions while keeping the rest.

Opened `check_current` at `admission_source.rs:169-221` (read). `:209-212` rejects only when `received.fatal_entries()` contains an entry not in `scope.fatal_entries()` — fewer fatal entries in `received` are allowed. `:213-215` — `received.context_anchor()` differing from `scope.context_anchor()` errors only when `received` has `Some` anchor; `None` in `received` when `scope` had `Some` passes. `:216-217` — assumption mismatch is diagnosed only when `received.installation_assumptions()` is non-empty and differs; empty `received` assumptions pass even if `scope` had assumptions.

Callers (read): `for_entry` `:150-159` and `require_current` `:162-167` both route through `check_current`.

Caller search: `rg 'check_current' omega-rust/omega/compilation/realization/provider-admission` → 3 matches (read): definition and two call sites in `admission_source.rs`.

Input in plain words: an admitted interrupt scope with fatal entries, a context anchor, or installation assumptions, paired with a current installed cohort that matches on service, boundary, and other fields but omits those fatal entries, clears the anchor, or clears assumptions.

Verdict **REACHABLE**: `check_current` returns `Ok(())` and execution continues as if the installation were unchanged.

## A0657

Digest (read): area_04.md:252 — `PA/provider_admission.rs:437-440`, `472`: reachable boundary without provider when declaration identity text is missing from the plan.

Opened `require_reached_providers` at `provider_admission.rs:421-491` (read). `:433-485` builds diagnostics with `filter_map`; `:472` uses `let requirement = plan.identities.text(declaration.identity)?` so `None` ends the closure with no diagnostic.

Caller search: `rg 'require_reached_providers' omega-rust` → 2 matches (read): definition and call at `:215` inside provider admission (before settlements return).

Upstream reachability (read): `:428-432` builds `reached.boundaries` from `AuthorityGraph::reachable_authority_edges`; boundaries without matching provider selection are candidates unless earlier filters at `:441-470` return `None`.

Input in plain words: a program whose entry reaches a boundary machine declaration whose identity has no resolvable text in `plan.identities`, and no selected provider or assumed binding covers that boundary.

Verdict **REACHABLE**: that boundary is silently dropped from the diagnostic list and `require_reached_providers` returns `Ok(())` at `:487-488` when no other boundaries error.

## What I did not read

- Full `retention_identity.rs` hasher paths beyond call sites cited for A0654 (inferred).
- `trust-model` grant-resolution helpers beyond `admission_settlement.rs:33-50` (inferred).
- Entire `08_selected-form-encoding-to-resolved-layout/src/validation.rs` beyond the deferred branch arm (inferred).
- `static_interrupts::admit` cohort construction for concrete witness programs (inferred).

SLICE COMPLETE
