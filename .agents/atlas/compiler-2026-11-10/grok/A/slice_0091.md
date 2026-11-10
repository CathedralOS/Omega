# slice_0091

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0696 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:187-191 | REACHABLE | Empty slot origins are removed before `derive`, so a contradictory empty-plus-captured listing can compile after unit validation. |
| A0697 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:1123 | REACHABLE | One failed `adapt` yields `None` for the whole graph; reference validation is skipped when graph construction fails. |
| A0698 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:357 | REACHABLE | With no activation claims, unlisted structural operations return `Ok(None)` and never enter activation flow validation. |
| A0699 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:883 | REACHABLE | Recorded retained-ownership edge checks are skipped when `ownership_frontier_facts` on the unit is empty. |
| A0700 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2057-2063 | CLOSED | Range pass `continue`s on shape mismatch, but `case_establishment_matches` / `record_establishment_matches` reject wrong shapes earlier in `validate_values`. |
| A0701 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2067 | CLOSED | `field_initializers_match` requires equal non-erased field counts before service-contract validation runs ahead of the zip loop. |
| A0702 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2235-2237 | REACHABLE | Missing kernel range requirement returns `Ok` from `validate_range_fact` even when an obligation index is present on the operation. |
| A0703 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2637 | REACHABLE | Jump, conditional, and case nodes accept any provenance vector when expected provenance is empty. |

## A0696

Digest: area_05 item 33 (`DR:187-191`). Read `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:53-214` (`access_results::summarize`, `resolve`, `derive`). At `descriptor_results.rs:184-191` (read), `DescriptorStoredOrigin::Empty` becomes `Some(None)` and `filter_map` drops those entries, so a slot whose origins are `[Empty, Captured(…)]` leaves only the captured source in `sources`. At `descriptor_results.rs:219-237` (read), `derive` then picks that single interpretation when no conflicting candidate remains.

Search `access_results::summarize` under `omega-rust`: 2 production call sites (`omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/references.rs:1595`, `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_reference_custody.rs:2917`). No call in `unit_validation.rs` (read via grep on `access_results` under `optimization-unit/semantics`).

Unit acceptance: `validate_psi_optimization_unit_with_control_cycles` at `unit_validation.rs:18260-18269` (read) never calls `summarize`. Input that reaches the branch: an optimization unit whose boundary-return descriptor slot lists both empty and captured origins for the same path passes validation, then lowering calls `summarize`/`resolve` and treats the slot as unambiguously captured.

Smallest program (plain words): a function’s shared return view slot is recorded with both “no stored origin” and a captured formation for the same path; validation accepts the unit; lowering resolves the view as if only the capture existed.

## A0697

Digest: area_05 item 34 (`DR:1123`). Read `descriptor_results.rs:1089-1142` (`catalog_graph`, closure `build`). At `descriptor_results.rs:1105-1123` (read), `functions.iter().map(... adapt ...).collect::<Option<Vec<_>>>()` propagates the first `None` from `adapt` as `None` for the entire graph. At `descriptor_results.rs:1016-1024` (read), `optimized_graph` returns that `Option`.

Search `optimized_graph(` under `omega-rust`: 4 matches — definition `descriptor_results.rs:1016`; uses `descriptor_results.rs:1151` (`call_origins`), `unit_validation.rs:1225`, `unit_validation.rs:15567`.

At `unit_validation.rs:1214-1226` (read), reference membership runs only when some function has `EstablishReference` **and** `optimized_graph(unit)` succeeds; `.flatten()` turns failure into `None`, so `reference_memberships` stays unset. At `unit_validation.rs:15566-15570` (read), `None` is an error only when `consumes_snapshots` is true; otherwise an empty graph is used.

Input: a unit that establishes references but contains one machine `adapt` cannot summarize (broken boundary/formation metadata). Validation skips `ReferenceMembershipValidation` on the happy path at `1225-1226`.

Smallest program: a unit with reference-establishing operations and internally inconsistent descriptor metadata such that graph reconstruction returns `None`; the catalog pass still completes without reference-membership checking.

## A0698

Digest: area_05 item 35 (`UV:357`). Read `unit_validation.rs:17-90` (`activation_storage::validate`) and `unit_validation.rs:92-369` (`operation` projection). At `unit_validation.rs:27-32` (read), `active` is false when the function has no `EstablishActivationClaim` / `ReleaseActivationClaim`. At `unit_validation.rs:352-357` (read), when `active` is false, structural operations with a structural result that lack an explicit match arm hit `_ => return Ok(None)` and are omitted from the replayed block at `unit_validation.rs:57-59`. At `unit_validation.rs:72-81` (read), `validate_flow` sees only the projected subset.

Search `activation_storage::validate` under `omega-rust`: 1 call in semantics (`unit_validation.rs:1247`); terminal foundation also calls kernel `validate_module` (not re-read).

Called from `catalogs::index_and_validate_unit_catalogs` per-function loop at `unit_validation.rs:1247` (read), inside `validate_psi_optimization_unit_with_control_cycles` at `unit_validation.rs:18265-18268`.

Smallest program: a machine with no activation-claim operations but with structural-result operations absent from the activation projection table; activation storage validation succeeds without replaying those operations.

## A0699

Digest: area_05 item 36 (`UV:883`). Read `unit_validation.rs:880-940` (`validate_retained_ownership_authority`) in full. At `unit_validation.rs:883-885` (read), empty `unit.ownership_frontier_facts` returns `Ok(())` before any edge walk. The loop at `unit_validation.rs:897-939` (read) compares declared edge cleanup against recorded frontier snapshots on the **unit**, not the vectors collected during `validate_current_ownership_frontier_collecting` (`current_ownership.rs:9839-9868`, read header only).

Search `validate_retained_ownership_authority` under `omega-rust`: 2 matches — definition and sole call `unit_validation.rs:18268`.

`index_and_validate_unit_catalogs` still replays ownership into a local `ownership_frontiers` vec (`unit_validation.rs:1231-1244`, read) but does not equate that vec to `unit.ownership_frontier_facts` before `validate_retained_ownership_authority` (`validate_final_authorities` at `unit_validation.rs:2247-2269` only index-checks stored facts).

Smallest program: a unit whose stored ownership-frontier fact list is empty while structural edges carry affine cleanup actions inconsistent with recomputed custody; retained-ownership validation returns success without examining edges.

## A0700

Digest: area_05 items 37 (`UV:2057-2063`, `UV:2089-2099`). Read `unit_validation.rs:1482-2120` (`validate_structural_field_range_authority`) and establishing-operation matchers.

At `unit_validation.rs:2057-2063` and `2089-2099` (read), non-`Sum` / non-`Record` shapes `continue`, skipping `validate_field_range_authority` in this pass only.

Closing checks: `validate_values_and_bindings` calls `operation_service_contract_matches` at `unit_validation.rs:10481-10498` (read). For `EstablishCase`, `case_establishment_matches` at `unit_validation.rs:10591-10598` (read) returns `false` when the result type is not a sum (`_ => return false`). For `EstablishRecord`, `record_establishment_matches` at `unit_validation.rs:10997-10999` (read) returns `false` when the shape is not `Record`.

Every function node passes through `validate_values_and_bindings` (`unit_validation.rs:9647-9658` before `validate_structural_field_range_authority` at `1248`, read). Wrong-type establishments fail `OperationServiceContractMismatch` before the range pass runs.

Verdict: gap in the range-only loop is harmless for acceptance because the service-contract gate rejects the same input.

## A0701

Digest: area_05 item 38 (`UV:2067`, `UV:2100`). Read zip loops at `unit_validation.rs:2067-2081` and `2100-2114` (read).

Closing check: `field_initializers_match` at `unit_validation.rs:11144-11158` (read) requires `declarations.clone().count() == fields.len()` before zipping; mismatch returns `false`. That predicate is used from `case_establishment_matches` / `record_establishment_matches` and is enforced via `operation_service_contract_matches` at `unit_validation.rs:10481-10498` (read), which runs before `validate_structural_field_range_authority` at `unit_validation.rs:1248`.

Truncating zip in the range pass cannot accept units whose field roster length disagrees with initializers.

## A0702

Digest: area_05 item 39 (`UV:2235-2237`). Read `unit_validation.rs:2228-2245` (`validate_range_fact`) and callers `validate_scalar_range_authority` / `validate_initialization_range_authority` at `unit_validation.rs:2141-2172` (read).

At `unit_validation.rs:2235-2237` (read), `None` requirement from the kernel returns `Ok(())` without calling `require_accepted_goal_fact`.

Upstream at `unit_validation.rs:2035-2049` (read), `StructuralScalarFieldStore` ensures `scalar_field_requires_evidence` matches `range_obligation.is_some()`, then calls `validate_field_range_authority`. When the kernel’s `scalar_field_requirement` / initialization requirement is `None` but an obligation id is still present, `validate_range_fact` succeeds without checking the accepted obligation fact index.

Smallest program: a scalar field store carries a range obligation reference and matching evidence-required type metadata, but the kernel returns no range proposition for that value; the range-fact helper accepts the operation.

## A0703

Digest: area_05 item 40 (`UV:2637`, `UV:2641`). Read `unit_validation.rs:2602-2642` (`expected_provenance`, `provenance_matches_operation`).

At `unit_validation.rs:2604` (read), jump, conditional, and `StructuralCase` expect an empty provenance vector. At `unit_validation.rs:2635-2638` (read), when expected is empty, those control operations return `true` for **any** `provenance` slice. At `unit_validation.rs:2640-2641` (read), other operations with nonempty expected prefixes accept extra trailing provenance via `starts_with`.

Search `provenance_matches_operation` under `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics`: definition plus use at `unit_validation.rs:5689` inside `validate_nodes_and_edges` (read `unit_validation.rs:5686-5698`), which runs inside per-function validation before unit acceptance.

Smallest program: a jump (or conditional or structural case) node whose recorded provenance lists arbitrary extra `PsiProvenance` entries that do not reflect authored operations; node metadata validation accepts it.

## What I did not read

- Full bodies of `adapt` and terminal-semantics kernels behind `scalar_field_requirement` (`terminal_semantics`, not opened).
- Pipeline lowering paths in `references.rs` / `legalization_reference_custody.rs` beyond the `summarize` call sites.
- `terminal_semantics::activation_storage::validate_flow` implementation (only the call at `unit_validation.rs:72-81` was read).
- Digest commit `457adf4a11f` pin; all line numbers are from the current checkout and matched the cited symbols on read.

SLICE COMPLETE
