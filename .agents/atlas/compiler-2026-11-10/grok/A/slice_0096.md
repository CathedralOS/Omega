# slice_0096 (area_05, rows A0736–A0743)

Preamble read: `.agents/atlas/compiler-2026-11-10/digests/area_05.md:1-27` (`UV` → `unit_validation.rs`; `OU/` → `optimization-unit/src/`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0736 | UV:18044 | REACHABLE | Explicit trivial-affine path returns `Ok` without `cleanup_actions` or return-parameter shape checks. |
| A0737 | UV:18076-18078 | REACHABLE | Both-empty `ReturnStructural` skips discard validation via `continue`. |
| A0738 | OU/construction.rs:608,741,910 | REACHABLE | Wildcard extractors and validator `expected_*` share the same omissions, so metadata can match while operands are unrecorded. |
| A0739 | OU/construction.rs:406-490 | REACHABLE | `operation_obligation` `_ => None` drops obligation references at construction with no generic repair. |
| A0740 | OU/dominance.rs:66-72 | REACHABLE | Rows start as the full-block universe; unreachable blocks can keep oversized dominator sets used by `contains`. |
| A0741 | OU/identity.rs:1272-1364,2361-2458 | REACHABLE | `encode_boundary_machine` omits seven boundary fields; `encode_unit` does not hash `unit.identities`. |
| A0742 | OU/manifest.rs:116-203 | REACHABLE | Decode checks manifest self-identity only, not `initial_unit`/`final_unit` against the ledger chain. |
| A0743 | OU/observation.rs:313-317 | REACHABLE | Missing ownership-frontier facts become `identity: None` inside an `Option` observation, not an error. |

## A0736

- **Cited site (read):** `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:18000-18170`, function `validate_trivial_affine_local_witnesses`.
- **Branch (read):** When `explicit` `EstablishTrivialAffineLocal` nodes exist and zip-match `expected_locals`, `unit_validation.rs:18037-18044` returns `Ok(())` immediately. `cleanup_actions` from `ReturnStructural` is collected at `18022` but never compared on this branch. Parameter/shape and discard checks run only in the later loop (`18101-18166`), including `StructuralReturnAffineDiscardsMismatch` at `18162` (sole occurrence in repo per search).
- **Callers (read):** Search `\bvalidate_trivial_affine_local_witnesses\b` in `omega-rust`: 4 matches — definition `18000`, production call `9604`, re-exports `2747`, `18187`. One production call site.
- **Upward (read):** `9604` is in per-function validation; `validate_psi_optimization_unit_with_control_cycles` `18260-18269` ends in `Ok` only if that chain succeeds. No other file references `StructuralReturnAffineDiscardsMismatch`.
- **Input (read):** A unit whose function uses explicit trivial-affine establishment nodes with matching places/types but whose sole `ReturnStructural` carries wrong `cleanup_actions` (or wrong return parameter shape) hits `18044` and is accepted.
- **Verdict:** REACHABLE — should reject on cleanup/shape; accepted on the explicit early return.

## A0737

- **Cited site (read):** `unit_validation.rs:18076-18078` — `if trivial_affine_locals.is_empty() && expected_locals.is_empty() { continue; }`.
- **Discard check (read):** `18142-18166` compares `cleanup_actions` to derived `expected_discards`; skipped when the `continue` fires.
- **Callers:** Same as A0736 (`9604`, one production call).
- **Input (read):** Function with no catalogued trivial-affine locals (`expected_locals` empty) and `ReturnStructural` with empty `trivial_affine_locals` but non-empty or wrong `cleanup_actions` never reaches `18155-18166`.
- **Verdict:** REACHABLE.

## A0738

- **Cited sites (read):** `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/construction.rs:608` (`operation_definition` `_ => None`), `741` (`direct_uses` `_ => Vec::new()`), `910` (`collect_places` `_ => {}`). Construction wires `definitions`/`uses` from `operation_definition`/`operation_uses` at `209-227`.
- **Validator (read):** `unit_validation.rs:5689-5692` requires `node.definitions == expected_definitions(...)` and `node.uses == expected_uses(...)`, which call the same `optimization_unit::operation_definition` / `operation_uses` (`2649-2673`). Digest check 7 at `area_05.md:728` matches this (read).
- **Search (read):** `\boperation_uses\b` in `omega-rust`: 4 matches (`construction.rs`×2, `lib.rs` re-export, `unit_validation.rs:2670`).
- **Upward (read):** Metadata check is inside control-flow validation on the path to `validate_psi_optimization_unit_with_control_cycles` (`18260-18269`). `validate_node_uses` (`14335-14395`) only inspects values listed in `node.uses`.
- **Input (read):** An `AbstractOperation` variant that reads or defines scalars but is missing from all three `match` arms is built and validated with empty uses/definitions on both sides, so `OperationMetadataMismatch` does not fire and dominator/use checks never see the real operands.
- **Verdict:** REACHABLE (symmetric omission is not a closing check).

## A0739

- **Cited site (read):** `construction.rs:406-490` — `operation_obligation` ends `_ => None`; `collect_fact` `345-350` pushes `OptimizationFact::OperationObligationReference` only when `Some`.
- **Search (read):** `\boperation_obligation\b` in `omega-rust`: 2 matches, both `construction.rs` (definition + call in `collect_fact`).
- **Validator (read):** Per-operation obligation checks use `require_accepted_goal_fact` / `unit.accepted_obligation_fact` in targeted `match` arms (e.g. `2219-2224`); `identity_indexes::validate_identity_and_fact_indexes` at `9697-9706` checks ordering/canonicality of existing facts, not completeness for every operation variant.
- **Upward (read):** Construction feeds function facts; full unit validation at `18264-18269` does not re-run `operation_obligation` for all nodes generically.
- **Input (read):** Obligation-carrying operation variant absent from `operation_obligation` and from the validator’s obligation `match` arms is accepted with no obligation reference fact and no error on that path.
- **Verdict:** REACHABLE.

## A0740

- **Cited site (read):** `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/dominance.rs:53-111`, `solve`. Comment at `1-3` (read): every declared block participates, including unreachable. Initialization at `66-72` sets each row to the full-block bit universe before intersection (`86-107`).
- **Consumers in optimization-unit semantics (read):** `dominators(` call sites in `unit_validation.rs` at `6718` (structural place availability) and `14469` (value use validation); `DominatorSet::contains` at `6731-6733`, `14356-14358`, `14382-14385`. `structural_places.rs` builds dominators at `571+` (read header `293`).
- **Search (read):** `\bdominators\(` in `omega-rust`: 20 call sites across 15 files (count mode); 2 direct solver invocations in `unit_validation.rs` above.
- **Input (read):** CFG block not reachable from `function.entry` can retain an oversized dominator set; `contains(producer_block)` in `validate_node_uses` or place availability can return true for producers that should not dominate.
- **Verdict:** REACHABLE.

## A0741

- **Boundary encoding (read):** `identity.rs:1272-1364` `encode_boundary_machine` hashes id, owner, role, parameters, `requires`, `normal_qualifications`, `reference_outputs`, etc. Fields on `BoundaryMachineDeclaration` at `omega-rust/psi/compilation/representations/terminal-psi/src/boundary.rs:766-794` include `declaration`, `numeric_intrinsic`, `crash_routes`, `scalar_domain_requirements`, `scalar_requires`, `scalar_ensures`, `result_reference_regions` — none appear in `encode_boundary_machine` (read; re-located by symbol name; lines match digest intent at current checkout).
- **Unit encoding (read):** `identity.rs:2361-2458` `encode_unit` hashes psi marker, fingerprint, catalogs, `boundary_machines` via `encode_boundary_machine`, functions, etc. No `bytes`/`slice` of `unit.identities` (`PsiOptimizationUnit.identities` at `lib.rs:184`, read).
- **Closing check (read):** `recompute_psi_optimization_unit_identity` `2349-2358` + `validate_psi_optimization_unit` identity check `9687-9692` — mismatch only if encoded fields differ.
- **Input (read):** Two units identical on all hashed fields but differing in unhashed boundary fields or identity tables share one `OptimizationUnitIdentity` and pass the content-identity check.
- **Verdict:** REACHABLE (whether unhashed fields are always derivable from hashed ones: not traced — would need a separate derivation proof).

## A0742

- **Cited site (read):** `manifest.rs:116-203` `PrePhysicalOptimizationManifest::decode` reads `initial_unit` and `final_unit` at `146-147`, decodes `transformation_ledger` at `167-168`, returns `Ok(manifest)` after `manifest.identity != manifest.recomputed_identity()` at `199-200` only.
- **Ledger (read):** `ledger.rs:77-78`, `101-104` — records carry `input`/`output` `OptimizationUnitIdentity`; no decode-time hook in `manifest.rs` comparing manifest endpoints to ledger endpoints (read `116-203` in full).
- **Input (read):** Wire manifest whose `initial_unit`/`final_unit` disagree with the first/last ledger record identities but whose body still satisfies `recomputed_identity` is accepted on decode.
- **Verdict:** REACHABLE.

## A0743

- **Cited site (read):** `observation.rs:309-318` in `reconstruct_psi_closed_region_observation`: `identity: unit.ownership_frontier_facts.iter().find(...).map(|fact| fact.identity)` — missing fact yields `None`, not `Err`.
- **Function (read):** Whole function returns `Option<PsiClosedRegionObservation>` (`144-148`); outer failures are `None` for bad block sets (`154-158`), not missing frontier rows.
- **Consumers (read):** `candidates.rs:2431-2479` compares `input_region.semantics == output_region.semantics`; `retained_source_frontiers` is part of `PsiClosedRegionSemantics` (`observation.rs:96-105`). `validate_retained_ownership_authority` `883-885` returns `Ok(())` immediately when `ownership_frontier_facts.is_empty()` (read).
- **Input (read):** Unit with frontier sites implied by region reconstruction but no matching `ownership_frontier_facts` entry produces observations with `identity: None` and can still yield `Some(...)`; validation does not require a fact per site before observation use.
- **Verdict:** REACHABLE.

## What I did not read

- Full `validate_node_operation_contracts` and every obligation `match` arm in `unit_validation.rs` (only sampled `1565-1605`, `2180-2225`, `5692`).
- `encode_boundary_machine` callers beyond `encode_unit` and test fixtures in `identity.rs:2210+`.
- All 20 `\bdominators\(` call sites outside `unit_validation.rs` and `structural_places.rs`.
- `AbstractOperation` enum completeness vs the three construction wildcards (no variant-by-variant crosswalk).
- `recomputed_identity` / `encode_manifest_content` beyond `manifest.rs:199-218` for ledger binding elsewhere in the pipeline.

SLICE COMPLETE
