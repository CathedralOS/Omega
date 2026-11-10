slice_0096

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0736 | unit_validation.rs:18044 | REACHABLE | Explicit trivial-affine-local witness path returns `Ok` without reading `ReturnStructural.cleanup_actions`. |
| A0737 | unit_validation.rs:18076-18078 | REACHABLE | `continue` skips affine-discard checking when both compressed locals and catalog expected locals are empty. |
| A0738 | construction.rs:608,741,910 | REACHABLE | `_ => None` / empty vec wildcards omit use/def/place metadata for unlisted operation variants; validator mirrors the same helpers. |
| A0739 | construction.rs:488 | REACHABLE | `operation_obligation` `_ => None` matches validator `reconstruct_fact_index` `_ => {}`; unlisted obligation ops emit no fact on both sides. |
| A0740 | dominance.rs:66-72 | REACHABLE | Unreachable blocks keep the full-block dominator bitset; `contains` is true for every in-graph block on those rows. |
| A0741 | identity.rs:1272-1364,2361-2458 | REACHABLE | `encode_boundary_machine` and `encode_unit` omit several boundary fields and `unit.identities`; identity can collide while semantics differ on unvalidated fields. |
| A0742 | manifest.rs:116-203 | REACHABLE | `decode` checks ledger internal chain and manifest identity recompute, not `initial_unit`/`final_unit` vs ledger endpoints. |
| A0743 | observation.rs:313-317 | REACHABLE | Missing `ownership_frontier_facts` row yields `identity: None` with no error; rewrite region compare treats that as equal semantics. |

### A0736

1. Read `validate_trivial_affine_local_witnesses` in full: `unit_validation.rs:18000-18170` (module `witnesses` inside `structural_catalog`).
2. Search: `rg 'validate_trivial_affine_local_witnesses' omega-rust` → **4** hits (all in `unit_validation.rs`).
3. Caller trace: `validate_function` `unit_validation.rs:9604` → `index_and_validate_unit_catalogs` loop `unit_validation.rs:1233` → `validate_psi_optimization_unit_with_control_cycles` `unit_validation.rs:18260-18269`.
4. Accepting branch input: `!explicit.is_empty()` with `explicit` EstablishTrivialAffineLocal rows matching `expected_locals` and compressed return locals all empty (`unit_validation.rs:18027-18044`); function returned to caller as `Ok(())` without entering the loop that checks `cleanup_actions` (`unit_validation.rs:18155-18166`).
5. Upward: `validate_psi_optimization_unit_with_control_cycles` rejects only on `Err` from per-function validation; no other read of return `cleanup_actions` on this explicit-only path (`validate_retained_ownership_authority` `unit_validation.rs:958-987` checks hidden locals in `trivial_affine_locals`, not top-level `cleanup_actions`).
6. **Verdict: REACHABLE** — A machine with cataloged trivial affine locals established only via explicit `EstablishTrivialAffineLocal` nodes and a `ReturnStructural` whose `cleanup_actions` disagree with the cataloged discard list should be rejected but passes once the explicit/expected place-type check succeeds at `18044`.
7. Entry description matches the read at `18044`.

### A0737

1. Same function read: `unit_validation.rs:18065-18167`.
2. Same caller search/count as A0736.
3. Accepting branch input: each `ReturnStructural` where `trivial_affine_locals.is_empty() && expected_locals.is_empty()` (`unit_validation.rs:18076-18078`).
4. **Verdict: REACHABLE** — A return with no cataloged trivial affine locals and empty compressed locals can carry arbitrary `cleanup_actions`; the discard check at `18155-18166` is never run for that node.
5. Entry matches the read; I did not fully trace whether the producer always emits empty `cleanup_actions` in this shape (inferred only from `no_normal_calls` helper at `9861-9877` requiring empty cleanup for one special case).

### A0738

1. Read in full: `operation_definition` `construction.rs:527-610`, `direct_uses` `construction.rs:623-743`, `collect_places` `construction.rs:745-912`, and seed loop `construction.rs:197-247`.
2. Searches: `rg 'operation_definition\(' omega-rust` → **4** call sites (construction + validator `expected_definitions` + one pipeline file); `operation_uses`/`collect_places` similarly used from `validate_nodes_and_edges` `unit_validation.rs:5689-5692`.
3. Accepting branch input: any `AbstractOperation` variant hitting `_ => None` / `_ => Vec::new()` / `_ => {}` at `608`, `741`, `910` during unit construction.
4. Downstream check: `validate_nodes_and_edges` compares `node.definitions`/`node.uses` to the same helpers (`unit_validation.rs:5689-5692`) — so a **forged** inconsistent node is rejected (`OperationMetadataMismatch`), but a **new** variant omitted from both sides stays self-consistent with empty metadata.
5. **Verdict: REACHABLE** — A newly added operation variant that defines or uses scalars but is not wired into these matches is serialized with no uses/definitions/places and still passes validation.
6. I did not enumerate all `AbstractOperation` variants to prove one current production variant is uncovered (would decide CLOSED vs REACHABLE for today’s enum).

### A0739

1. Read `operation_obligation` in full `construction.rs:406-490` and `collect_fact` `construction.rs:345-360`; read validator `reconstruct_fact_index` `unit_validation.rs:5840-5991`.
2. Search: `rg 'operation_obligation\(' omega-rust` → **2** hits (`construction.rs` only).
3. Accepting branch input: operations whose obligation fields are not listed in the `match`, producing `None` and no `OperationObligationReference` fact at construction; validator’s parallel match also skips (`_ => {}` at `5957`).
4. `fact_index::validate_fact_index` `unit_validation.rs:5828-5837` requires `function.facts == reconstruct_fact_index(function)` — both sides omit the same obligations.
5. **Verdict: REACHABLE** — An obligation-carrying operation missing from both tables validates with no obligation fact recorded.
6. Note: `operation_obligation` includes `EstablishPrimitiveLocal` / `WriteOnlyPrimitiveStore` / `AtomicEvent` arms (`construction.rs:409-428`) not mirrored in the validator’s first obligation `match` (`unit_validation.rs:5847+`); I did not trace whether production always omits those obligations or another path errors (partial inference only).

### A0740

1. Read `solve` in full `dominance.rs:53-111` and `DominatorSet::contains` `dominance.rs:27-32`; comment at `dominance.rs:1-3` documents unreachable-block behavior.
2. Search: `rg 'dominator' omega-rust/omega/compilation/representations/abstract-operations/optimization-unit` → **94+** hits; read consumers `validate_node_uses` `unit_validation.rs:14356-14385` and `validate_structural_place_availability` `unit_validation.rs:6718-6734`.
3. Accepting branch input: unreachable block `B` (not on paths from entry) whose dominator row remains the initialized universe (`dominance.rs:66-72`); `contains` returns true for any in-roster predecessor block.
4. `validate_values_and_bindings` iterates **all** `function.blocks` (`unit_validation.rs:14476`), not only reachable ones.
5. **Verdict: REACHABLE** — In an unreachable block, a use of a value defined in a non-dominating reachable block is treated as dominating because `contains` is true for all roster blocks.
6. I did not read every dominator consumer beyond the two above.

### A0741

1. Read `encode_boundary_machine` `identity.rs:1272-1364` against `BoundaryMachineDeclaration` fields `terminal-psi/src/boundary.rs:766-794`.
2. Read `encode_unit` / `recompute_psi_optimization_unit_identity` `identity.rs:2349-2458` and `PsiOptimizationUnit.identities` `lib.rs:184`.
3. Not hashed (read): `declaration`, `numeric_intrinsic`, `crash_routes`, `scalar_domain_requirements`, `scalar_requires`, `scalar_ensures`, `result_reference_regions` on boundaries; entire `unit.identities` table.
4. `validate_identity_and_fact_indexes` recomputes identity with the same encoder (`unit_validation.rs:9687-9692`) — colliding units are not distinguished.
5. Semantic checks: `crash_routes` referenced in validation (`unit_validation.rs:10865+`); **no** reads of `scalar_requires`, `scalar_ensures`, `scalar_domain_requirements`, `numeric_intrinsic`, or `boundary.declaration` in `optimization-unit/semantics` (search `rg` → no matches).
6. **Verdict: REACHABLE** — Two units identical on hashed material but differing on unhashed, unvalidated boundary fields share `OptimizationUnitIdentity` and both pass `validate_psi_optimization_unit`.
7. Whether some omitted fields are derivable from hashed data: **not traced** (per entry).

### A0742

1. Read `PrePhysicalOptimizationManifest::decode` in full `manifest.rs:116-203`.
2. Read `PsiTransformationLedger::new` chain check `ledger.rs:119-151` (decode uses this).
3. Accepting branch input: decoded `initial_unit` / `final_unit` (`manifest.rs:146-147`, `185-186`) independent of `transformation_ledger.input` / `.output` inside embedded ledger.
4. No cross-check at decode between manifest unit ids and ledger endpoints (read); manifest identity includes both manifest fields and ledger blob (`manifest.rs:215-228`) but not equality between them.
5. **Verdict: REACHABLE** — Manifest bytes can decode with ledger revision chain internally valid yet manifest `final_unit` not equal to ledger’s terminal `output`.
6. I did not read every post-decode consumer of the manifest.

### A0743

1. Read `reconstruct_psi_closed_region_observation` through frontier assembly `observation.rs:144-333`; accepting branch at `observation.rs:313-317`.
2. Search: `rg 'reconstruct_psi_closed_region_observation' omega-rust` → production use in `candidates.rs:2431-2478` (rewrite validation).
3. Accepting branch input: `frontier_sites` site with no matching `unit.ownership_frontier_facts` entry → `identity: None`, observation still `Some(...)`.
4. Rewrite gate compares `input_region.semantics == output_region.semantics` (`candidates.rs:2478`) including `retained_source_frontiers` with `Option` identities (`observation.rs:84-87`).
5. Separate path: `validate_retained_ownership_authority` returns early when `ownership_frontier_facts.is_empty()` (`unit_validation.rs:883-885`).
6. **Verdict: REACHABLE** — Region observations (and rewrite comparison) accept absent frontier identities instead of failing; pairs with units that keep an empty attached facts table through validation.
7. Entry matches the read.

## What I did not read

- Full dominator consumer set beyond `validate_node_uses` and `validate_structural_place_availability`.
- Every `AbstractOperation` variant for coverage against `operation_definition` / `operation_obligation` / `reconstruct_fact_index`.
- Post-decode manifest consumers beyond `manifest.rs`.
- Complete proof that production never emits the explicit trivial-affine-local + wrong-cleanup shape (A0736/A0737).
- Whether unhashed boundary fields are always derivable from hashed catalog data (A0741).

SLICE COMPLETE
