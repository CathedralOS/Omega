# slice_0108

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0831 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact/provider_execution.rs:135-138 | CLOSED | Import/Table/Component bindings without same-stack or syscall yield `requirement_identity == None`, so binding check errors before `Ok(())` at :170. |
| A0832 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact/provider_execution.rs:37-40 | UNDECIDED | `unwrap_or_default()` maps missing `text` to `""`; not traced whether an ill-formed boundary identity can reach replay with a plan that keys on `""`. |
| A0833 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs:992-1024 | CLOSED | Unmapped requirements pass through by design; physical replay requires exactly one plan whose roster contains the requirement string (:261-270 in evidence.rs). |
| A0834 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact/identity.rs:322-325 | UNDECIDED | Out-of-range identity indexes hash as empty string; not traced whether two distinct invalid indexes can both reach sealing. |
| A0835 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs:147-156 | UNDECIDED | Foreign-call custody equality runs only when `fragment_publication` is present; not traced whether non-empty custody arises without publication on seal paths. |
| A0836 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs:201-205 | UNDECIDED | Missing boundary text becomes `""` then plan filter; not traced how selected-provider plans are built or whether `""` can satisfy `:261-270`. |
| A0837 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/evidence.rs:60-65 | UNDECIDED | `Blocked(gap)` artifacts seal with gap in identity; `PreparedExecutablePublication::new` does not inspect gap—emit path does not call `validate()`. |
| A0838 | omega-rust/omega/compilation/backend/artifacts/native-artifact/src/physical/derivation/provider_custody.rs:29-37 | UNDECIDED | No matching family row returns `Ok(())`; not traced whether a family requirement can reach here without a plan row. |

## A0831

**Cited site (re-located):** `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact/provider_execution.rs:121-171` (`validate_foreign_provider_binding`, read).

**Enclosing function:** Full function read at `provider_execution.rs:121-171`.

**Suspected hole:** Foreign bindings using Import/Table/Component with neither `same_stack_contribution` nor `system_call` skip stack/kernel checks and return `Ok(())` at `:170`.

**Mechanism check:** `target_operations::ProviderExecutionBinding::requirement_identity` (`omega-rust/omega/compilation/representations/target-operations/src/boundary/realizations.rs:194-203`, read) returns `None` when both `same_stack_contribution()` and `system_call()` are absent.

**Closing check:** At `provider_execution.rs:135-138`, `foreign.binding.requirement_identity(binding_identities) != Some(selected_requirement)` is true when the left side is `None`, so the function returns `Err("native external call binding differs from its requirement")` before `:140-169`.

**Callers:** Search `\bvalidate_foreign_provider_binding\b` in `omega-rust` — **4 matches in 2 files** (definition, use at `:103`, two test references in `tests.rs`); **1 production call site** at `provider_execution.rs:103` inside `replay_provider_executions`.

**Upward:** `replay_provider_executions` (`:20-119`) is invoked from `NativeArtifact::validate` at `native_artifact.rs:556-562` (read) and `DynamicElfNativeArtifact::validate` at `dynamic_elf.rs:147-154` (read). Search `\breplay_provider_executions\b` — **4 matches in 3 files** (definition + 2 production callers).

**Input that reaches the branch:** A foreign call in `object.foreign_calls()` / image foreign-call roster during provider-execution replay on artifact validation, only if binding checks at `:135-139` pass; Import/Table/Component without same-stack/syscall do not pass `:135-138`.

**Verdict:** **CLOSED** — stack/kernel omission at `:140-169` is unreachable for those mechanisms because `:135-138` fails first (`read`).

## A0832

**Cited sites (re-located):** `provider_execution.rs:37-40` and `:99-102` inside `replay_provider_executions` (`:20-119`, read).

**Behavior:** `module.identities.text(boundary.identity).unwrap_or_default()` supplies the requirement string for settlements and foreign calls (`read`).

**Identity table:** `TerminalIdentityTable::text` returns `None` for out-of-range indexes (`omega-rust/psi/compilation/representations/terminal-psi/src/identity.rs:354-356`, read); index `0` (`ABSENT`) is the empty string in the default table (`:299-303`, read).

**Downstream:** Requirement strings feed `required_report_key` (`provider_execution.rs:227-247`) and `validate_foreign_provider_binding`; plan roster checks use string compare / `binary_search` elsewhere in this module (`:257-268`, read).

**Callers of `replay_provider_executions`:** Same as A0831 (2 production callers).

**What would decide REACHABLE vs CLOSED:** Whether a Terminal module boundary with missing/invalid `boundary.identity` can reach native artifact sealing with a selected provider plan whose `requirement_identities` contains `""`, causing wrongful acceptance instead of an error.

**Verdict:** **UNDECIDED** — `""` fallback is confirmed (`read`); upstream Psi/artifact construction paths that admit invalid identity indexes were not traced.

## A0833

**Cited site (re-located):** `native_artifact/tests.rs:287` exercises `NativeSelectedProviderPlan::selected_requirement_identity` (`read`).

**Definition (re-located from digest “native_artifact.rs”):** `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs:992-1024` (`selected_requirement_identity`, read). Unlisted `requirement` strings fall through to `map_or(requirement, …)` at `:1020-1023`.

**Test behavior:** `plan.selected_requirement_identity(other)` equals `other` when `other` is not in assumed/external/family/closed rows (`tests.rs:286-287`, read).

**Closing check (physical evidence):** For each boundary occurrence, `derive_physical_evidence` filters plans with `binary_search` on `requirement` (`evidence.rs:261-267`, read) and errors unless exactly one plan matches (`:269-270`, read).

**Closing check (provider execution reports):** `validate_provider_execution_reports` requires each execution’s identity to match `plan.selected_requirement_identity(identity)` for some roster entry (`provider_execution.rs:298-301`, read).

**Verdict:** **CLOSED** — unknown requirements unchanged only when still constrained by plan roster and execution-report equality checks on validation paths (`read`).

## A0834

**Cited site (re-located):** `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact/identity.rs:317-326` (`hash_identity_text`, read).

**Behavior:** `identities.text(identity).unwrap_or_default().as_bytes()` is hashed (`:322-325`, read).

**Callers:** Search `\bhash_identity_text\b` under `omega-rust/omega/compilation/backend/artifacts/native-artifact` — **9 matches, 1 file** (`identity.rs` only); uses include plan/family row hashing in `derive_native_artifact_identity` (`:182-194`, read).

**Suspected hole:** Two different out-of-range indexes both hash as empty string, collapsing digest distinctions.

**What would decide:** Whether callers can pass distinct invalid `TerminalIdentityIndex` values into `hash_identity_text` on artifacts that still pass `NativeArtifact::validate`.

**Verdict:** **UNDECIDED** — collision mechanism confirmed (`read`); reachability of distinct invalid indexes at seal time not traced.

## A0835

**Cited site (re-located):** `evidence.rs:147-156` inside `derive_physical_evidence` (`:67-462`, read).

**Behavior:** `retained_custody` is `optimized.foreign_call_custody()` only for `ValidatedOptimizedProjection`; otherwise `&[]` (`:147-152`, read). Equality `retained_custody != object.foreign_calls()` is checked only when `fragment_publication.is_some()` (`:154-156`, read).

**Caller:** Search `\bderive_physical_evidence\b` — production calls at `native_artifact.rs:456-466`, `:483-493`, `dynamic_elf.rs:73-83` (read).

**Suspected hole:** Non-empty foreign-call custody might diverge from the object without comparison when fragment publication is absent.

**Verdict:** **UNDECIDED** — conditional check confirmed (`read`); not traced whether non-empty custody arises without `fragment_publication` on paths that still seal or publish.

## A0836

**Cited sites:** `evidence.rs:201-205` (boundary identity map) and `:261-267` (plan filter), read.

**Behavior:** Boundary requirement text uses the same `text(...).unwrap_or_default()` pattern as A0832 (`:201-205`). `matching_plans` keeps plans whose sorted `requirement_identities` contain the requirement via `binary_search` (`:261-267`).

**Suspected hole:** Empty requirement text could associate an occurrence with the wrong sole selected plan.

**What would decide:** How `NativeSelectedProviderPlan::requirement_identities` is populated during emission and whether `""` can appear alongside other requirements.

**Verdict:** **UNDECIDED** — control flow read; plan construction and Psi boundary identity invariants not traced.

## A0837

**Cited sites:** `evidence.rs:60-65` (`blocked` helper) and returns such as `:170-175`, `:255-259`, read; parallel seal path `native_artifact/dynamic_elf.rs:63-65` (`from_emitted_parts`, read).

**Behavior:** `derive_physical_evidence` returns `Ok(NativePhysicalEvidenceDerivation::Blocked(gap))` instead of `Err` for unsupported subjects (`evidence.rs:60-65`, read). `NativeArtifactEmissionParts::into_parts` stores the gap and seals (`native_artifact.rs:297-300`, `:508-518`, read). Gap identity is hashed in `derive_native_artifact_identity` (`identity.rs:237-238`, read).

**Publication:** `PreparedExecutablePublication::new` (`publication.rs:277-296`, read) checks output path and compiler validation evidence only — **no** `physical_evidence_gap` check (`read`). `NativeArtifact::emit` uses `from_emitted_parts` without calling `validate()` (`native_artifact.rs:201-202`, read).

**Verdict:** **UNDECIDED** — gap recording and seal path confirmed (`read`); whether downstream publication or product policy treats a sealed gap as acceptable shipment was not fully traced (publication entry at `omega-rust/omega/compilation/products/src/publication.rs:78` not read in full).

## A0838

**Cited site (re-located):** `provider_custody.rs:20-85` (`validate_family_provider_custody`, read).

**Behavior:** `plan.family_realization_rows()` search at `:29-35`; if no row’s head/member text matches `identity`, the function returns `Ok(())` at `:36` with no further checks (`read`).

**Caller:** Search `\bvalidate_family_provider_custody\b` — **2 matches in 2 files** (definition + `evidence.rs:272`); **1 production call site** (`read`).

**Upward:** Called for every boundary occurrence after a sole `selected_plan` is chosen (`evidence.rs:261-279`, read).

**Suspected hole:** A family-related requirement whose identity text is not represented in `family_realization_rows` would skip family custody validation.

**Verdict:** **UNDECIDED** — early-return behavior confirmed (`read`); not traced whether plan emission can omit a row while still selecting that plan for the occurrence.

## What I did not read

- Full `omega-rust/omega/compilation/representations/target-operations/src/boundary/realizations.rs` beyond `requirement_identity` / `has_valid_entry_mechanism`.
- Full `physical/derivation/foreign_calls.rs` derivation path beyond cited binding checks.
- Psi boundary construction invariants for `boundary.identity` indices.
- Selected-provider plan construction (where `requirement_identities` and `family_realization_rows` are populated).
- Entire `publication.rs` and full `publish_compilation` policy beyond `PreparedExecutablePublication::new`.
- `git show 457adf4a11f:` versions of cited lines (current checkout lines were opened directly; symbols matched digest descriptions).

SLICE COMPLETE
