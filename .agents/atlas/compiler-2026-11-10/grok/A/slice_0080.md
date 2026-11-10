# slice_0080

Area digest preamble (read): Area 4 is stages 00–09 of `omega-rust/omega/compilation/pipeline/` plus `products` and `provider-admission`; path shorthand `S01/` … `S09/` maps to `pipeline/<NN>_…/src/` under `omega-rust/omega/compilation/`. Digest citations at `457adf4a11f`; cited line ranges in this checkout matched on read unless noted.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0610 | S01/specialization.rs:478-508 | REACHABLE | Callee return constants are read without requiring a reachable return or ruling out crash continuations on the call. |
| A0611 | S01/validation.rs:2557-2564 | REACHABLE | Outside the loop component, unlisted operations are treated as confined even when they reference the picked place. |
| A0612 | S01/validation.rs:1086-1087 | REACHABLE | The definition-site map is built with `collect`, so duplicate value ids keep only the last pair silently. |
| A0613 | S02/control_flow_operations.rs:557-570 | REACHABLE | `PortWrite` copies the abstract `value` id through without `scalar_sources::source` lookup. |
| A0614 | S02/control_flow_storage.rs:1320, :3569, :2613-2616 | REACHABLE | Byte-store sources may be arrival parameters; descriptor-member fields skip reference relocation; float block parameters are skipped in boolean materialization. |
| A0615 | S02/function_signature.rs:546-552, :228 | REACHABLE | Signature preparation failure becomes `Ok(None)` for mixed ABI; unbuildable layout roots are dropped in `capture`. |
| A0616 | S02/references.rs:1818-1827, :2534-2540, :2235-2237 | REACHABLE | Case-path missing provenance and missing contracts skip errors instead of failing discard or result checks. |
| A0617 | S02/structural_layout.rs:184-189 | REACHABLE | When a prefix before an index segment does not resolve, the walker `continue`s and still returns projection regions. |

## A0610

**Cited site (read):** `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/specialization.rs:478-508` — `call_result_constant` finds a `Call` whose `result` equals `argument`, takes the sole `Return` operand in the callee (`499-501` requires at most one `Return` operation), and reads a constant fact for that return value (`503-508`). No reachability or crash-route check appears in this function.

**Enclosing function (read):** `call_result_constant` (specialization.rs:478-509), module `state_specialization::admission`.

**Callers (search):** pattern `call_result_constant` in `omega-rust` — 2 call sites in `specialization.rs:410` (`admit_incoming_edge`); 2 additional occurrences in `optimization-unit/semantics/src/candidates.rs:9744,9824` (parallel validator logic, read count). `admit_incoming_edge` is called only from `propose::plan` (specialization.rs:531-537, search count 2).

**Upward trace (read):** `admit_incoming_edge` (351-424) uses `call_result_constant` when the edge binding has no direct constant fact (`406-411`). `propose::plan` (517-550) feeds admitted edges into `StateArgumentSpecializationRewrite`; `rules.rs:1167` invokes this plan under `StateArgumentSpecializationRule`. No caller inspects callee return reachability or call crash continuations before accepting the constant.

**Input that reaches the branch:** A dispatch block specializing on a parameter fed from a `Call` result, where the callee’s only `Return` node is not on any path from the call site (for example it sits in a block only reached after a crash continuation that does not pass the scalar result through), yet constant facts attach to the return operand.

**Smallest program (plain words):** A state-machine dispatch is specialized using a constant taken from a called function’s return value, where that return appears only on a path that never runs for a normal completion of the call (for example after an unavoidable crash route), so the specialization treats an impossible outcome as definite.

## A0611

**Cited site (read):** `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation.rs:2557-2564` — in `scalar_case_result_contained`, the default arm calls `member_place_references`; when that returns `false`, `confined` becomes `!member` (`2561-2563`), which is `true` for nodes outside the loop component (`2499`).

**Enclosing function (read):** `scalar_case_result_contained` (validation.rs:2490-2608).

**Callers (search):** pattern `\bscalar_case_result_contained\b` in `omega-rust` — 9 occurrences, all in `validation.rs` (312, 663, 924, 1033, 1062, 1399, 1845, 2490 definition, plus import lines; read count).

**Upward trace (read):** `structural_call_result_tolerated` (306-317) rejects invariant structural-call admission when `scalar_case_result_contained` is false for an affine case result. `invariant_trivial_affine_local_admission` (1056-1063) gates relocation on the same predicate.

**Input that reaches the branch:** `O::StructuralLeafCopy { source: picked, .. }` in a block whose id is not in `component.members` (`structural_layout` confirmed `StructuralLeafCopy` is not handled in `member_place_references`, validation.rs:1645-1723 returns `false` for unlisted ops).

**Smallest program (plain words):** A loop-invariant relocation admits an affine case result as fully contained while a leaf-copy operation outside the loop body still reads from that result’s root place, because the default arm treats the outside node as confined without enumerating the copy’s source reference.

## A0612

**Cited site (read):** `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/validation.rs:1086-1087` — `value_definition_sites` maps every definition site with `.map(|definition| (definition.value, definition.site)).collect()` into a `BTreeMap`, so later entries with the same `ValueId` overwrite earlier ones with no error.

**Enclosing function (read):** `value_definition_sites` (validation.rs:1074-1088), module `member_blocks`.

**Callers (search):** pattern `\bvalue_definition_sites\b` in `omega-rust` — 6 occurrences: 5 in `validation.rs` (1258, 1319, 1396 re-export, 1959, 1074 definition) and 1 in `ranked_rewrites.rs:806` (read count).

**Upward trace (read):** `invariant_member_parameters` (1253-1297) and `member_scalar_operand_substitution` (1312-1334) call `sites.get(&binding.argument)` and branch on the single stored `ValueDefinitionSite`. SCCP in `semantic_analyses.rs:1770-1771` inserts every node definition as `Overdefined` without rejecting duplicate value keys (read), which is consistent with the representation allowing multiple defining rows for one value id even though the map keeps only one.

**Input that reaches the branch:** One optimization function lists the same scalar `ValueId` on two node `definitions` vectors (or on both a block parameter and a node definition with colliding ids).

**Smallest program (plain words):** Two nodes in one function both define the same value identity; loop-invariant relocation resolves member parameters using whichever definition site happened to be inserted last into the map, without reporting the duplication.

## A0613

**Cited site (read):** `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow_operations.rs:557-570` — `AbstractOperation::PortWrite` pushes `TargetUnitOperation::PortWrite { …, value: *value }` with no `scalar_sources::source` call; neighboring `RegisterAccess` at `540-542` does resolve inputs via `scalar_sources::source`.

**Enclosing function (read):** the `match operation` arm inside the lowering function in `control_flow_operations.rs` (same module as `scalar_sources` at 1323-1364).

**Callers (search):** production lowering reaches this through the shared operation-lowering dispatch in stage 02; `PortWrite` abstract operations originate from stage-00 lowering (`lowering.rs`, read grep). Target plan consumption: `legalization_source.rs:1752-1768` copies the same raw `value` id into legalized form without resolving homes.

**Upward trace (read):** Stage-03 selection (`selection_scalar_graph.rs:7288-7296`) emits `SelectedInstructionKind::PortWrite { port, value }` using the carried value id directly. No stage between abstract operations and selection re-validates that the value id exists in `LiveDefinitions` integer, home, or parameter tables for this operation kind.

**Input that reaches the branch:** An abstract `PortWrite` whose `value` operand is a `ValueId` not present in the caller’s live scalar maps but still present in the verified abstract plan.

**Smallest program (plain words):** A port write is lowered and selected using a value identity that was never registered in live scalar storage for that function, because lowering never runs the same source-resolution path used for register writes and other scalar operands.

## A0614

**Cited sites (read):**

1. `control_flow_storage.rs:1318-1322` in `records::replace` (`replace` starts 1236) — after choosing `source` for a byte-sequence store, the guard rejects when `!(function.arrival_parameter(*source).is_some() || live.views.contains_key(source))`, so any block arrival structural parameter id satisfies the source check without a live view entry (`1320`).

2. `control_flow_storage.rs:3569-3580` in `records::establish` (`establish` starts 3321) — for each record field argument, `reference_bearing && !descriptor_member` gates `subtree_leaves` relocation; when `descriptor_member` is true (`3493-3503`), reference-bearing nested values skip relocation (`3569` condition false).

3. `control_flow_storage.rs:2613-2616` in `scalar_values` (`2521-2625`) — the loop over `live.scalar_block_parameters` `continue`s when the parameter type is `ScalarType::IeeeFloat(_)` (`2614-2615`), skipping boolean materialization for that block parameter.

**Callers (search):** `records::replace` and `records::establish` are invoked from stage-02 storage lowering paths in `control_flow_storage.rs` / `control_flow.rs` (read dispatch). `scalar_values` is used when building boolean expressions during lowering (read module references).

**Upward trace (read):** For (1), byte stores accept a source place that is only known as a function block entry parameter, not as an established view in `live.views`. For (2), aggregate record establishment can omit moving reference leaves inside descriptor-member fields while still completing the establish operation. For (3), boolean lowering omits IEEE float block parameters entirely from the `values` map rather than erroring.

**Inputs:**

- (1) Byte sequence store whose `source` place is a block structural parameter at function entry.
- (2) Record establishment copying a reference-bearing value into a view-descriptor member field (`descriptor_member == true`).
- (3) Block entry carrying an IEEE float block parameter reused where boolean materialization walks `scalar_block_parameters`.

**Smallest program (plain words):** (1) A bounded byte sequence store uses an entry-block structural parameter as the byte source without that parameter ever appearing in live view storage. (2) A record initializer places a reference-bearing value into an element-view descriptor member field and lowering skips relocating its reference leaves. (3) Boolean lowering for a block ignores an IEEE float block parameter instead of rejecting it.

## A0615

**Cited sites (read):**

1. `function_signature.rs:546-552` — `derive_mixed_structural_scalar_function_abi` maps `prepare_function_signature` failure to `Ok(None)` via `let Ok(prepared) = … else { return Ok(None) }`, swallowing `Err` diagnostics.

2. `function_signature.rs:226-229` — `borrowed_storage::capture` builds the layout vector with `.filter_map(|root| capture_type(root, declarations))`, dropping roots whose recipe cannot be built (`capture_type` returns `None` at `236` or `243` on failure, read).

**Enclosing functions (read):** `derive_mixed_structural_scalar_function_abi` (532-566); `borrowed_storage::capture` (175-230).

**Callers (search):** `derive_mixed_structural_scalar_function_abi` — 3 matches: definition plus `coordination.rs:510` (plan lowering). `prepare_function_signature` — 6 matches in stage-02 (read count). `borrowed_storage::capture` — used when preparing borrowed storage layouts for lowering (read imports in `function_signature.rs`).

**Upward trace (read):** `coordination.rs:508-513` only inserts mixed ABI entries when `derive_mixed_structural_scalar_function_abi` returns `Some`; `Err` from signature preparation becomes absence of mixed ABI, and lowering continues. Separately, `capture` silently omits unbuildable structural roots from the captured layout list rather than failing the plan.

**Input that reaches the branch:** (1) A function with both structural parameters and a fixed native scalar result where `prepare_function_signature` would error (for example ABI parameter count or placement mismatch). (2) A requested layout root whose `StorageRecipes::collect` fails during `capture_type`.

**Smallest program (plain words):** (1) A mixed structural-and-scalar entry function has an ABI mismatch; lowering proceeds with no mixed ABI instead of failing. (2) A borrowed layout root needed for reference storage is omitted from the captured layout table because recipe collection failed, while compilation continues.

## A0616

**Cited sites (read):**

1. `references.rs:1817-1827` — in `stage` (`1725-`), `Source::Incoming` with `live.references.get(&source)` missing: if the coordinate path contains a `Case` segment, execution `continue`s (`1822-1825`) instead of `Err` (`1826-1827`).

2. `references.rs:2528-2540` — stored-reference call result transfer: when `moved`/`reference_leaf` miss and the carrier path contains `Case`, `continue` (`2534-2540`) instead of erroring (contrast non-case path `2538`).

3. `references.rs:2235-2237` — `discard_subtree`: when `source_contract` is `None`, returns `Ok(false)` without discarding references (`2236-2237`).

**Enclosing functions (read):** `references::stage` (1725+); call-result reference binding block inside the same module (~2480+); `discard_subtree` (2228-2249).

**Callers (search):** `discard_subtree` — 10 matches across stage-02 and stage-03 custody (read count). `stage` for stored-reference effects is used from call lowering in `references.rs` / `control_flow_calls.rs` (read module structure).

**Upward trace (read):** Skipping an incoming stored-reference effect when provenance is missing allows the callee staging list to omit a replacement that should have failed (`1813-1815` still errors for non-case paths). `continue` on call-result case paths drops a leaf move instead of reporting missing provenance. `discard_owned` (`2212-2225`) treats `discard_subtree` returning `false` as “nothing discarded”, so places without contracts leave reference maps unchanged.

**Input that reaches the branch:** A stored-reference call passes an incoming reference living under a sum-case path without live provenance; or a discard targets a place with no structural contract in live maps.

**Smallest program (plain words):** A call copies a stored reference into a destination when the incoming value lives in a case payload path and the compiler has no live reference provenance for that path—it skips the effect instead of failing. A separate discard of a place with no contract returns success without removing tracked references underneath it.

## A0617

**Cited site (read):** `structural_layout.rs:182-189` in `reference_projection_regions` (165-233) — for `Referent`, `FixedIndex`, or `RuntimeIndex` segments, `runtime_descriptor_projection` failure on the prefix before the segment yields `None` for `selected`; if the segment is not `Referent`, control hits `continue` (`188-189`) rather than `return None`.

**Enclosing function (read):** `reference_projection_regions` (165-233).

**Callers (search):** pattern `\breference_projection_regions\b` in `omega-rust` — 19 occurrences across `structural_layout.rs`, `scalar.rs`, `control_flow.rs`, `control_flow_calls.rs`, `control_flow_storage.rs` (read count).

**Upward trace (read):** The function still returns `Some(ReferenceProjectionRegions { … })` at `227-232` after skipping unresolved index steps, shortening `path` via `segment_start` updates only when a segment resolves (`224-225`). Callers such as `control_flow_calls.rs:738-744` reject only when `pointer_slots` is empty after the call, not when slots were skipped mid-path.

**Input that reaches the branch:** A projected path containing a `RuntimeIndex` or `FixedIndex` where the type path before that index does not resolve through `runtime_descriptor_projection`, but a later suffix still allows a `Some` return.

**Smallest program (plain words):** A reference path includes an index step whose type prefix cannot be resolved as a descriptor projection; the walker skips that step and still produces projection regions as if the index were an ordinary later segment, potentially admitting a reference operation that should be rejected.

## What I did not read

- Full bodies of every caller of `records::replace`, `records::establish`, and `references::stage` beyond the traced sites.
- Whether stage-00 Terminal Psi admission mechanically forbids every concrete witness program for each REACHABLE case (no end-to-end compile run; read-only atlas).
- `optimization-unit/semantics/src/candidates.rs` `call_result_constant` body beyond the call site line (only the parallel call at 9744 was noted).
- Stage-03 register-allocation behavior for `PortWrite` values with no live home (only legalization and selection sites cited above).

SLICE COMPLETE
