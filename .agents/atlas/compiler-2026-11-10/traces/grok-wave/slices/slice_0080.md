slice_0080

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0610 | S01/specialization.rs:478-508 | REACHABLE | `call_result_constant` (read) takes the sole callee `Return` and a constant fact without dominator, crash-continuation, or `InvokeNoNormal` checks. |
| A0611 | S01/validation.rs:2557-2564 | REACHABLE | (read) Outside-loop catch-all sets `confined` when `member_place_references` is false, so unlisted ops are not checked against `picked`. |
| A0612 | S01/validation.rs:1086-1087 | CLOSED | (read) `value_definition_sites` last-wins only after `collect_value_definitions` rejects duplicate `ValueId` at optimization-unit ingress (unit_validation.rs:14326-14328). |
| A0613 | S02/control_flow_operations.rs:557-570 | CLOSED | (read) `PortWrite` copies `value` from an abstract row already admitted from Terminal Psi; stage-03 legalization re-pairs the same `value` id (legalization_source.rs:1758-1767). |
| A0614 | S02/control_flow_storage.rs:1320;3569;2613-2616 | REACHABLE | (read) Descriptor-member arguments skip `subtree_leaves` reference relocation (3569), which can accept calls without moving reference provenance. |
| A0615 | S02/function_signature.rs:546-552;228 | CLOSED | (read) `prepare_function_signature` failures become `Ok(None)` only on the optional mixed-ABI side path; mandatory lowering still errors (control_flow.rs:2574-2575). |
| A0616 | S02/references.rs:1818-1827;2534-2540;2235-2237 | REACHABLE | (read) Sum-case carrier paths `continue` instead of error when provenance is missing, skipping destination tracking for those incoming/call-result references. |
| A0617 | S02/structural_layout.rs:184-189 | REACHABLE | (read) Unresolved index prefix `continue`s, so later segments use an unchanged `root` and can yield `Some` regions for invalid paths. |

### A0610

1. Read `specialization.rs:478-509` (`call_result_constant` in full) and `351-412` (`admit_incoming_edge` in full, partial `dispatch_evidence` `285-332`).
2. Search: `rg 'call_result_constant' /agent/repos/omega/omega-rust` → 3 hits (definition `specialization.rs:478`, call `specialization.rs:410`, mirror `candidates.rs:9744`/`9824-9850`).
3. Search: `rg 'admit_incoming_edge' /agent/repos/omega/omega-rust` → 2 hits (definition and `propose::plan` `531`).
4. Read `rules.rs:1156-1197` (rule proposes patch), `candidates.rs:10126-10162` (replay requires `patch.edges == replayed`; same constant logic).
5. Read `candidates.rs:9223-9266` (validator SCCP seeds facts; no crash-edge filter read in this slice).

**Input to accepting branch:** Dispatch block whose last node is `Conditional`, incoming edge binds the compared parameter to a value defined by `Call { result, .. }` where the callee has exactly one `Return { value }` node anywhere in the callee and `constants.facts` (or validator replay facts) maps that return value to a constant (`specialization.rs:406-412`, `491-508`).

**Verdict: REACHABLE.** Smallest rejected program (plain words): a machine with a state-dispatch loop specialized on a parameter fed by a call that only completes on a crash/no-normal path, while the callee still has a single syntactic `Return` carrying a constant that never executes on the normal edge; specialization treats the call result as that constant and folds the dispatch (`specialization.rs:495-508` without reachability). Uncertainty: did not read full SCCP transfer through `InvokeNoNormal`/`Crash` terminators to see whether facts are still emitted for the dead return value.

### A0611

1. Read `validation.rs:2490-2608` (`scalar_case_result_contained` in full).
2. Read `validation.rs:1645-1723` (`member_place_references` in full).
3. Search: `rg 'scalar_case_result_contained' /agent/repos/omega/omega-rust` → 8 hits; read call sites `validation.rs:312`, `924`, `1062`, `1845` (admission gates), not every downstream rewrite body.

**Input to accepting branch:** `picked` is an affine case-result place inside an optimizer SCC; a node in a block **not** in `component.members` has an operation variant absent from the explicit `match` arms and absent from `member_place_references` (e.g. not listed at `1649-1720`), so the catch-all uses `!member` → `confined = true` (`2557-2563`). Entry description matches (read). Extra guard at `2597-2607` only checks successor cleanup on non-member blocks.

**Verdict: REACHABLE.** Smallest rejected program: loop member produces affine case result at `picked`, but a block outside the SCC runs an unlisted operation that still observes or aliases `picked` (e.g. an operation type hitting `_ => return false` in `member_place_references`); containment returns true and unlocks invariant rewrites (`validation.rs:312-315`). Uncertainty: did not enumerate every `O` variant hitting `_` in `member_place_references`.

### A0612

1. Read `validation.rs:1074-1088` (`value_definition_sites` in full).
2. Search: `rg 'value_definition_sites' /agent/repos/omega/omega-rust` → 5 hits (definition plus uses in `validation.rs` and `ranked_rewrites.rs:806`).
3. Read `unit_validation.rs:14316-14332` (`collect_value_definitions`); read `graph.rs:183-185` (`definitions()` iterator, read).

**Input to accepting branch:** Iterator yields two `ValueDefinition` entries with the same `value`; `.collect()` into `BTreeMap` keeps the last site (`1086-1087`).

**Verdict: CLOSED.** Check: `collect_value_definitions` returns `DuplicateValue` on second insert (`unit_validation.rs:14326-14328`). Stage-01 `pass_manager.rs` (grep hit, not read in full) validates the optimization unit before analyses; duplicate ids should not reach `value_definition_sites`. Uncertainty: did not read every post-rewrite revalidation path after applied candidates.

### A0613

1. Read `control_flow_operations.rs:557-570` (`PortWrite` arm in full) and neighboring `RegisterAccess` `540-542` (read).
2. Search: `rg 'PortWrite' .../pipeline/02...` → lowering arm only; read `unit.rs:1043-1057` (boundary metadata check, read).
3. Read `lowering.rs:369-377` (stage-00 lowers Terminal `PortWrite`, read) and `legalization_source.rs:1752-1768` (read).

**Input to accepting branch:** `AbstractOperation::PortWrite { value, .. }` where `value` is already a `ValueId` on the abstract row.

**Verdict: CLOSED.** Terminal-to-abstract admission supplies the operand (`lowering.rs:369-377`); stage-02 does not re-resolve scalars because the target row is paired by id in legalization (`1758-1767`). Boundary `MetadataOnlyPort` additionally pins the value (`unit.rs:1048-1056`). Uncertainty: did not trace non-boundary `PortWrite` through stage-03 register allocation for undefined `value` ids.

### A0614

1. Read `control_flow_storage.rs:1318-1323` (byte-store source test, read), `3493-3580` (`descriptor_member` / `subtree_leaves` gate, read), `2613-2624` (boolean `scalar_values` loop, read).
2. Did not read full enclosing functions `byte_sequence_store` (from `1240`) or case-establishment (`~3370`) end-to-end.

**Inputs:** (a) `1320`: byte-sequence store accepts `source` when `function.arrival_parameter(*source).is_some()` OR `live.views.contains_key(source)`. (b) `3569`: `reference_bearing && !descriptor_member` gates `subtree_leaves`; descriptor members skip relocation. (c) `2613-2616`: `live.scalar_block_parameters` IEEE-float entries are skipped in the boolean value map.

**Verdict: REACHABLE** (descriptor-member limb). Smallest rejected program: structural call passes a reference-bearing value into a descriptor-member field; lowering accepts the call without `subtree_leaves` (`3569-3580`), so destination provenance is not established while compilation continues. Sub-claims (a) and (c) look like guarded special cases (arrival-param path is paired with length/view checks `1318-1327`; float skip avoids boolean typing `2617-2619`); the accepting harm is the skipped relocation. Uncertainty: did not prove an end-to-end compile success without relocation.

### A0615

1. Read `function_signature.rs:546-552` and `73-158` (`prepare_function_signature`, read), `228-229` (`capture` `filter_map`, read), `232-252` (`capture_type`, read).
2. Search: `rg 'prepare_function_signature' .../pipeline/02` → 3 hits; read `control_flow.rs:2574-2575` (propagates `Err`, read) and `coordination.rs:508-513` (`derive_mixed_structural_scalar_function_abi`, read).

**Input to accepting branch:** `derive_mixed_structural_scalar_function_abi` maps `prepare_function_signature` `Err` to `Ok(None)` (`546-551`); `capture` drops roots when `capture_type` returns `None` (`228`).

**Verdict: CLOSED** for mandatory lowering. Primary function lowering calls `prepare_function_signature?` and fails closed (`control_flow.rs:2574-2575`). `Ok(None)` only omits optional `mixed_structural_scalar_abis` entries (`coordination.rs:508-513`). `capture_type` drop is layout optionalization for borrowed-storage capture, not plan acceptance. Uncertainty: did not trace every consumer when mixed ABI map lacks an entry but lowering succeeded.

### A0616

1. Read `references.rs:1790-1828` (`Source::Incoming` in `stage`, read), `2522-2541` (call-result reference leaf, read), `2228-2249` (`discard_subtree`, read).
2. Did not read full `stage` / apply functions above `1725` or all callers of `discard_subtree`.

**Inputs:** Incoming reference with `Case` in path and missing `live.references` entry → `continue` (`1818-1825`). Call-result path with case in carrier path and missing leaf → `continue` (`2534-2540`). `discard_subtree` with `source_contract` `None` → `Ok(false)` (`2235-2236`).

**Verdict: REACHABLE** (first two limbs). Smallest rejected program: call passes an incoming reference rooted in a sum-case path without live provenance at the leaf; lowering skips the slot instead of failing (`1818-1825`), so destination tracking/provenance update is dropped. Third limb (`2235-2237`) is a no-op discard when no contract—likely harmless if callers require contract elsewhere; not fully traced. Uncertainty: did not read apply path after `stage` for whether another check errors.

### A0617

1. Read `structural_layout.rs:165-233` (`reference_projection_regions` in full).
2. Search: `rg 'reference_projection_regions' .../pipeline/02` → 12 hits; read caller sample `control_flow_calls.rs:738-741` (errors on `None` only, read), not all call sites.

**Input to accepting branch:** Path segment is `FixedIndex`/`RuntimeIndex`/`Referent`, `runtime_descriptor_projection` on prefix fails (`selected` `None`), segment is not `Referent` → `continue` (`184-188`) without updating `root`.

**Verdict: REACHABLE.** Smallest rejected program: structural path with a non-referent segment before an index where the prefix does not resolve; walker keeps the old root and may still return `Some(ReferenceProjectionRegions)` (`227-232`) instead of rejecting, so downstream projection uses the wrong carrier. Some call sites map `None` to error (`control_flow_calls.rs:738-739`) but the function can succeed with a skewed region. Uncertainty: did not prove a full lowering success on such a path.

### What I did not read

- Full SCCP/constant-analysis transfer for calls with crash/no-normal continuations (`candidates.rs` beyond `9223-9266`).
- Complete `prepare_function_signature` / `StorageRecipes::collect` error paths and all `mixed_structural_scalar_abis` consumers.
- Full `references::stage` apply/commit after `1725`.
- All `reference_projection_regions` call sites and dominator/reachability proofs for callee `Return` nodes.
- Stage-00 Terminal validation rules for `PortWrite` operands beyond `lowering.rs:369-377`.

SLICE COMPLETE
