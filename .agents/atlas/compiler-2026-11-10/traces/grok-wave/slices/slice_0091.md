slice_0091

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0696 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:187-191 | CLOSED | `Empty` filtering sits in lowering-time `access_results::summarize`; `validate_psi_optimization_unit` does not call it. |
| A0697 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/descriptor_results.rs:1123 | CLOSED | Whole-graph `None` fails validation when descriptor requirements or shared-descriptor origins need a graph. |
| A0698 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:357 | CLOSED | `Ok(None)` only omits ops from activation replay when `active` is false; those ops are still checked in per-node contracts earlier in `validate_function`. |
| A0699 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:883-884 | REACHABLE | Empty stored `ownership_frontier_facts` skips the only structural-edge affine transition check in this crate. |
| A0700 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2057-2098 | CLOSED | `continue` on type mismatch is subsumed by `case_establishment_matches` / `record_establishment_matches` in `validate_values_and_bindings` before range authority runs. |
| A0701 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2067,2100 | REACHABLE | Range zip uses unfiltered `case.fields` / record `declarations` while establishment matches use erased-filtered rosters, so pairing can skew. |
| A0702 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2235-2236 | CLOSED | `Ok` when the kernel yields no requirement is gated by `require_scalar_receiving_fact` restriction/obligation pairing at 2184-2188. |
| A0703 | omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2635-2641 | CLOSED | Permissive `provenance_matches_operation` is paired with `validate_provenance_work_effects` work-set equality at 6461-6469. |

### A0696

1. Read `access_results::summarize` in full (`descriptor_results.rs:53-196`) and `derive` (`descriptor_results.rs:219-237`).
2. Search: `rg 'access_results::summarize' /agent/repos/omega/omega-rust` → **2** hits (production pipeline only: `pipeline/02_abstract-operations-to-target-operations/src/references.rs:1595`, `pipeline/03_.../legalization_reference_custody.rs:2917`).
3. Read callers upward: no path into `validate_psi_optimization_unit` (`unit_validation.rs:18204-18210`); validation finishes before those pipeline stages run.
4. Input on the accepting branch: `DescriptorStoredOrigin::Empty` maps to `Some(None)` at `descriptor_results.rs:184`, then `filter_map` drops it at `187-188`; remaining sources go to `derive` / `resolve`.
5. **Verdict: CLOSED** — `validate_identity_and_fact_indexes` / `validate_psi_optimization_unit_with_control_cycles` (`unit_validation.rs:18264-18269`) never invoke `summarize`; empty-origin dropping affects post-validation lowering, not unit acceptance.

### A0697

1. Read `catalog_graph` in full (`descriptor_results.rs:1089-1142`); read `adapt` through its setup and early returns (`descriptor_results.rs:1215-1285`, not the full ~1400-line body).
2. Search: `rg 'optimized_catalog_graph\(' /agent/repos/omega/omega-rust` → **6** hits; `rg 'optimized_graph\('` → **4** hits.
3. Call chain (read): `CallAccesses::new` (`current_ownership.rs:2865-2871`) → `index_and_validate_unit_catalogs` (`unit_validation.rs:1208-1249`) → `validate_psi_optimization_unit_with_control_cycles` (`unit_validation.rs:18265-18269`). On `None`: `validate_compatibility` errors if requirements are nonempty (`current_ownership.rs:736-741`); shared-descriptor `descriptor_origins_valid` fails without `view_graph` (`unit_validation.rs:7227-7236`, `7315`); `runtime_evidence` errors on missing graph when snapshots are consumed (`unit_validation.rs:15566-15568`).
4. Input: any `adapt(...)?` failure collapses `build`’s `collect::<Option<Vec<_>>>()` to `None` at `descriptor_results.rs:1122-1123`.
5. **Verdict: CLOSED** — paths that need descriptor structure reject `None`; I did not exhaust every `adapt` `return None` site to prove impossibility for units with empty descriptor requirements.

### A0698

1. Read `activation_storage::validate` and `operation` in full (`unit_validation.rs:23-369`).
2. Search: `rg 'activation_storage::validate' /agent/repos/omega/omega-rust/omega-rust/omega` → **1** production caller at `unit_validation.rs:1247` (plus terminal-psi codec paths).
3. Read upward: `validate_function` runs `control_flow::validate_nodes_and_edges` and `validate_values_and_bindings` first (`unit_validation.rs:9594-9658`), then `activation_storage::validate` (`unit_validation.rs:1247`).
4. Input: when `active` is false (`unit_validation.rs:27-32`), unmatched ops hit `_ => return Ok(None)` at `357` and are omitted from the replayed block; when `active` is true, structural results without a projection error at `352-355`.
5. **Verdict: CLOSED** — dropped replay projections are not the acceptance gate; the same nodes were already validated in `validate_values_and_bindings` / node contracts. I did not read every `AbstractOperation` variant for a structural result absent from both lists.

### A0699

1. Read `validate_retained_ownership_authority` in full (`unit_validation.rs:880-991`).
2. Search: `rg '\bvalidate_retained_ownership_authority\b' /agent/repos/omega/omega-rust` → **2** hits (`unit_validation.rs:880`, `18268`).
3. Read upward: only caller is `validate_psi_optimization_unit_with_control_cycles` (`unit_validation.rs:18268`); `StructuralEdgeAffineDiscardsMismatch` is emitted only inside this function (`unit_validation.rs:905-953`).
4. Input: empty `unit.ownership_frontier_facts` returns `Ok(())` immediately (`883-884`); no other read site compares recomputed `ownership_frontiers` from `validate_function` to stored facts for edge transitions.
5. **Verdict: REACHABLE** — a unit whose edge cleanup contradicts frontier snapshots but ships with empty stored frontier facts should be rejected yet passes retained-ownership validation. I did not confirm whether the producer always attaches nonempty facts for legitimate programs.

### A0700

1. Read `validate_structural_field_range_authority` `EstablishCase` / `EstablishRecord` arms (`unit_validation.rs:2051-2116`); read `case_establishment_matches` and `record_establishment_matches` in full (`unit_validation.rs:10555-10618`, `10962-11012`).
2. Search: indirect via `validate_values_and_bindings` → `node_contracts::validate_node_operation_contracts` (`unit_validation.rs:14486-14498`).
3. Phase order (read): `validate_function` calls `validate_values_and_bindings` at `9647-9658`; `validate_structural_field_range_authority` runs later in the per-function loop at `1248`.
4. Input: non-`Sum` / non-`Record` result types hit `continue` at `2062-2063` / `2097-2098`; establishment matchers return `false` for those shapes at `10597-10598` / `10997-10999`.
5. **Verdict: CLOSED** — wrong result types fail establishment matching before the range-authority loop’s `continue` can hide an obligation.

### A0701

1. Read range loops at `unit_validation.rs:2067-2082` and `2100-2114`; read `field_initializers_match` count check (`unit_validation.rs:11153-11158`).
2. Search: `case_establishment_matches` roster construction (`unit_validation.rs:10601-10616`) filters `!field.relevance.is_erased()`.
3. Establishment runs in `validate_values_and_bindings` (order as A0700).
4. Input: range authority zips unfiltered `case.fields` / record `declarations` with the initializer vector; establishment zips erased-filtered declarations with the same vector.
5. **Verdict: REACHABLE** — a sum case with a leading erased field can pass establishment while range `zip` pairs the wrong declaration with a scalar initializer and skips or mis-checks range authority on the real field. I did not trace a concrete corpus witness.

### A0702

1. Read `validate_range_fact` in full (`unit_validation.rs:2228-2245`) and callers `validate_scalar_range_authority` / `validate_initialization_range_authority` (`2141-2172`); read `require_scalar_receiving_fact` (`2175-2205`).
2. No separate caller search beyond the above (same module).
3. Called from `validate_structural_field_range_authority` during unit validation (`unit_validation.rs:1248`).
4. Input: `requirement: None` returns `Ok(())` at `2235-2236` without calling `require_accepted_goal_fact`.
5. **Verdict: CLOSED** — when an obligation is present, `require_scalar_receiving_fact` requires matching `restriction` and `obligation` at `2184-2188` before range facts matter; `None` requirement means no proposition to prove.

### A0703

1. Read `provenance_matches_operation` in full (`unit_validation.rs:2630-2642`); read `validate_nodes_and_edges` check (`5689-5698`); read `validate_provenance_work_effects` work/provenance equality (`6461-6469`, `6504-6517`).
2. Search: `rg 'provenance_matches_operation' /agent/repos/omega/omega-rust` → **3** hits (definition + import + `5689`).
3. Order: `validate_nodes_and_edges` at `9594`; `validate_provenance_work_effects` at `9645`.
4. Input: empty `expected` gives `true` for jump/conditional/case at `2636-2638`; nonempty expected allows trailing provenance via `starts_with` at `2641`.
5. **Verdict: CLOSED** — extra provenance entries still require matching `work` settlements; I did not read the remainder of `validate_provenance_work_effects` after `6529`.

### What I did not read

- Full body of `adapt` in `descriptor_results.rs` (only through ~1285 of ~1500+ lines).
- All `adapt` / `summarize` internal `return None` sites beyond sampled grep.
- Complete `validate_provenance_work_effects` after `unit_validation.rs:6529`.
- Terminal-psi `activation_storage::validate_flow` implementation.
- Whether the unit producer always populates `ownership_frontier_facts` for programs with structural-edge cleanup.

SLICE COMPLETE
