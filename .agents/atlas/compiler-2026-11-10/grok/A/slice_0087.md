# slice_0087

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0664 | candidates.rs:1581-1586 | REACHABLE | Shared-terminal fusion admits every binding via `\|_, _\| true`, unlike merge paths that require dominance. |
| A0665 | candidates.rs:1813-1832 | CLOSED | Prune acceptance recomputes custody from `validator_active_source_ordinals` and rejects unless `patch.machines` matches exactly. |
| A0666 | candidates.rs:1834-1879 | UNDECIDED | Reachability is only `validator_machine_references` plus seeds; no concrete unexposed reference kind was verified. |
| A0667 | candidates.rs:4225-4249 | REACHABLE | Compatible-policy keys ignore overflow policy; validator can CSE a saturating leader over an exact redundant op when obligation witnesses align. |
| A0668 | candidates.rs:7315 | CLOSED | `revalidate_rewrite_output` runs full unit validation including `node.uses == expected_uses` at unit_validation.rs:5692. |
| A0669 | candidates.rs:9824-9851 | REACHABLE | `call_result_constant` treats a single scanned `Return` plus facts as const; producer and validator share the same helper logic. |
| A0670 | candidates.rs:2274-2277 | CLOSED | Copy-prop output skips `WriteOnlyPrimitiveStore` with range obligation but region semantics compare rejects inconsistent normalize vs apply. |
| A0671 | candidates.rs:10686-10692 | CLOSED | Every `accept_region_rewrite` path calls `revalidate_rewrite_output`, which always runs `validate_psi_optimization_unit` on the whole unit. |

## A0664

Digest entry `CAND:1581-1586` (area_05). Opened `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/candidates.rs` at `1581-1586` (read); line numbers match digest commit `457adf4a11f` on this checkout (read).

Enclosing function: `shared_jump_fusion::replay::validate` (`1528-1647`, read). At `1581-1586` it calls `checked_scalar_substitutions(candidate, &target.parameters, bindings, |_, _| true)` (read). The closure always returns true, so the `admits` predicate in `checked_scalar_substitutions` (`2007-2036`, read) never enforces replacement dominance.

Search `checked_scalar_substitutions\(` in `omega-rust`: **3** call sites, all in `candidates.rs` (read): `557-569` (adjacent block merge, dominance via `independently_replacement_dominates_uses`), `720-732` (non-adjacent merge, same), `1581-1586` (shared terminal, always true).

Callers of this validate path: search `validate_shared_jump_fusion_candidate` in `omega-rust` → **2** definition/use sites in `candidates.rs` (`1720`, dispatch at `10393`) (read). Upward: `validate_psi_rewrite_candidate` dispatches on candidate kind; success is `Ok(ValidatedPsiRewrite)` only if replay returns `Ok` (read).

Earlier guards in the same function still require patch shape, incoming-edge counts, and `independently_post_dominates` on blocks (`1578-1579`, read), but not per-binding dominance on scalar replacements before substitutions are accepted (`1581-1586`, read). After substitutions pass, `rewrite_scalar_substitutions` applies them on the fused terminal (`1605-1610`, read) and `accept_region_rewrite` revalidates (`1629-1646`, read).

**Input that reaches the branch:** A `FuseSharedTerminalJump` candidate on a shared return block where jump bindings type-check and match `candidate.substitutions()`, but a binding’s argument is not dominated by the uses merged into the shared return (would fail the dominance predicate used at `557-569` on a block-merge candidate).

**Verdict REACHABLE (read + inferred):** Such a candidate can pass `checked_scalar_substitutions` here and proceed toward acceptance unless a later check fails; no dominance closure exists at this call unlike the other two substitution sites.

## A0665

Digest `CAND:1813-1832` and index at `CAND:1782`. Opened `1813-1832` and `1778-1787` (read); lines align with digest.

Enclosing function: `unreachable_private_machines::replay::validate` (`1750-1810`, read). `validator_active_source_ordinals` (`1813-1831`, read) walks ordinals and maps active machines. At `1778-1787`, acceptance builds `expected_custody` with `source_ordinals[machine]` for each expected pruned machine and returns `CandidateReachabilityMismatch` if `patch.machines != expected_custody` (read).

Search `validator_active_source_ordinals` in `omega-rust`: **2** matches in `candidates.rs` (definition `1813`, use `1778`) (read). Caller: `validate_unreachable_private_machines_candidate` → `replay::validate` → `validate_psi_rewrite_candidate` at `10414` (read).

If a machine were missing from the map, `source_ordinals[machine]` at `1783` would panic (read), not return `Ok(ValidatedPsiRewrite)` (inferred). Ordinals for all `unit.functions` entries are assigned when pruned slots and active iterators align (`1823-1829`, read).

**Verdict CLOSED (read):** Every accepting path for this prune compares candidate custody to freshly recomputed ordinals at `1786-1787`; absent a map entry the validator aborts rather than accepting.

## A0666

Digest `CAND:1834-1879`, “not traced.” Opened `1834-1879` and `1881-1900` (read).

Enclosing function: `unreachable_private_machine_complement` (`1834-1878`, read). Reachability worklist seeds entry, dynamic dispatch rows, provider candidates, and attachment machines (`1842-1858`, read), then expands via `validator_machine_references` per function (`1860-1877`, read). References come from cleanup invoke machines and `unit.operation_machine_references(operation)` (`1897-1898`, read). `operation_machine_references` covers `EstablishNativeCodeLayout`, `Call`, `CallStructural`, `SelectImplementation`, and dynamic table rows (`optimization-unit/src/lib.rs:252-280`, read).

Search `unreachable_private_machine_complement` in `omega-rust`: **2** hits (`candidates.rs` definition; `control_flow_cleanup.rs` in pipeline) (read). Validator acceptance uses complement at `1760` and requires agreement with candidate machine sets (`1771-1776`, read).

**Verdict UNDECIDED (read):** A program whose only live edge to a private machine uses a reference channel not collected by `validator_machine_references` or the seed sets was not identified in this slice; whether that yields an accepted erroneous prune remains open.

## A0667

Digest `CAND:4225-4249` and key erasure `CAND:3466-3516`. Opened `3466-3515`, `3366-3405`, `4122-4148`, `4225-4248` (read).

Compatible-policy leaders take wrapping/saturating ops without obligations (`3448-3451`, `3366-3383`, read); redundant ops require exact policies with obligations (`3395-3405`, read). Keys `arithmetic_key` / `shift_key` omit policy (`3487-3515`, `3466-3484`, read). Validator arm `(CompatiblePolicy, None, Some(redundant))` at `4225-4248` checks leader has no obligation-reference facts and witnesses redundant obligation facts (read). Producer-side compatible-policy leader/redundant split matches (`pipeline/.../global_value_numbering.rs:81-103`, read).

Search `ScalarCseProofClass::CompatiblePolicy` in `candidates.rs`: many validator admissions (read). CSE apply uses `rewrite_scalar_value_uses` after retiring redundant node (`4407-4410`, read).

**Input reaching the branch:** Same-block or dominating compatible-policy CSE where leader is saturating (or wrapping) integer add/mul/shift and redundant is the same operands under exact policy with accepted no-overflow obligation witness.

**Verdict REACHABLE (read + inferred):** Keys match despite policy difference; acceptance can eliminate the exact redundant operation and retain the saturating leader unless later stages reject (obligation facts do not re-check policy equivalence at `4225-4248`).

## A0668

Digest `CAND:7315`. Opened `7293-7320` inside `rewrite_scalar_value_uses` (read). `O::StructuralCase { .. } => {}` skips operand rewrites (`7315`, read).

Search `rewrite_scalar_value_uses\(` in `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/candidates.rs`: **15** occurrences (read); production CSE path at `4409` (read).

CSE validation requires some `node.uses` entry for `redundant_result` (`4358-4363`, read), then sweeps with `rewrite_scalar_value_uses` (`4408-4410`, read). `accept_region_rewrite` calls `revalidate_rewrite_output` (`10680`, read), which calls `validate_psi_optimization_unit` (`unit_validation.rs:18225`, read). Per-node metadata must match `expected_*` including `node.uses != expected_uses(...)` → `OperationMetadataMismatch` (`5690-5698`, read).

**Verdict CLOSED (read):** A structural-case operand left stale after sweep yields metadata/definition inconsistency on revalidation rather than a silent `Ok`.

## A0669

Digest `CAND:9824-9851`. Opened `9824-9851` and caller context `9742-9745` in state specialization replay (read).

`call_result_constant` finds a `Call` defining `argument`, loads callee, requires exactly one `Return` among `block_nodes()`, then looks up a scalar constant fact (`9829-9850`, read). No recursion or side-effect guard (read). Used when planning dispatch constant edges (`9742-9745`, read).

Search `call_result_constant` in `omega-rust`: **2** files — `candidates.rs` (definition + use) and `pipeline/.../specialization.rs:478-509` with the same structure (read). Validator `validate_state_argument_specialization_candidate` replays `plan_dispatch` and requires `patch.edges == replayed` (`10155-10161`, read).

**Verdict REACHABLE (read + inferred):** A callee summarized as returning one compile-time constant (single `Return` in node order plus facts) while still being non-constant under recursion or other effects is treated as constant in both producer and validator replay, so a bad specialization candidate can match its own replay.

## A0670

Digest compares `CAND:2274-2277`, `CAND:2113`, `CAND:7293`. Opened `2113-2186`, `2223-2282`, `7293-7320` (read).

`normalize_redundant_parameter_observation_operation` handles `StructuralCase` payloads and falls through to `rewrite_value_operands` for other ops (`2181-2184`, read). `WriteOnlyPrimitiveStore` is rewritten via `primitive_local_triple_pattern!(value, ..)` in `rewrite_value_operands` (`10248-10249`, read). `rewrite_block_parameter_operation` explicitly no-ops `WriteOnlyPrimitiveStore { range_obligation: Some(_), .. }` (`2274-2277`, read).

Search `rewrite_block_parameter_operation` in `omega-rust`: **6** references in `candidates.rs`, including validation apply at `2455` (read).

Validation builds output with `rewrite_block_parameter_operation`, then compares closed-region semantics between normalized input and output (`2426-2479`, read). Mismatched store operands after normalize-vs-apply yield `CandidateRegionObservationMismatch` at `2478-2479` (read).

**Verdict CLOSED (read):** The skipped arm does not open an acceptance path when the redundant parameter appears in a range-obligation store; region observation fails before `Ok(ValidatedPsiRewrite)`.

## A0671

Digest `CAND:10686-10692`. Opened `10662-10698` (read).

`accept_region_rewrite` compares candidate blocks/provenance (`10671-10672`), applies closure, checks affected function exists, runs `revalidate_rewrite_output` on the **whole** unit (`10680`, read), then `validate_outside_region` on block lists (`10681-10685`, read). Loop `10686-10691` only asserts unrelated functions remain in the output roster (read); it does not alone guard `pruned_machines` or `accepted_obligation_facts`.

Search `accept_region_rewrite` in `omega-rust`: **29** matches in `candidates.rs` (read). `revalidate_rewrite_output` always recomputes identity and calls `validate_psi_optimization_unit` (`unit_validation.rs:18213-18226`, read). Unit catalog checks for `pruned_machines` and `accepted_obligation_facts` live in `index_and_validate_unit_catalogs` (`unit_validation.rs:1128-1172`, read). Some rewrites add explicit `accepted_obligation_facts` equality checks (e.g. `4905-4908`, read).

**Verdict CLOSED (read):** Unit-level tables on acceptance paths are gated by full-unit `validate_psi_optimization_unit` after every `accept_region_rewrite`, not only by the function-presence loop at `10686-10691`.

## What I did not read

- Full `validate_psi_optimization_unit` beyond catalog and per-node metadata excerpts.
- Complete `control_flow_cleanup.rs` producer for unreachable-machine pruning.
- Exhaustive inventory of machine-bearing operation variants outside `operation_machine_references`.
- End-to-end proof that compatible-policy CSE survives native execution after acceptance (no build per atlas).

SLICE COMPLETE

