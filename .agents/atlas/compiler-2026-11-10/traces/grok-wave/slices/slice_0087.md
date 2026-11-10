slice_0087

| entry id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0664 | `candidates.rs:1581-1586` | UNDECIDED | Shared-terminal fusion admits every type-matched substitution while adjacent merges check per-binding dominance; did not establish post-dominator plus ownership checks close that gap for return parameters. |
| A0665 | `candidates.rs:1813-1832;1782-1783` | UNDECIDED | `validator_active_source_ordinals` can stop before every active function is mapped; did not find unit validation that forces a bijection before `source_ordinals[machine]` indexing. |
| A0666 | `candidates.rs:1834-1879` | REACHABLE | Reachability omits some machine-reference carriers, so an active private machine can be classified unreachable and a matching prune candidate validates. |
| A0667 | `candidates.rs:4225-4249;3448-3456` | UNDECIDED | Compatible-policy CSE deliberately equates policy-divergent ops when keys match; obligation-proof soundness through unit validation was not traced here. |
| A0668 | `candidates.rs:7315` | UNDECIDED | `StructuralCase` is a no-op in `rewrite_scalar_value_uses` but CSE rewrites via sweep; did not prove a redundant-value use can survive only inside unscanned `StructuralCase` sites. |
| A0669 | `candidates.rs:9824-9851` | UNDECIDED | Validator replays `plan_dispatch` using the same `call_result_constant` rule as the producer; did not trace whether obligation/scalar-fact validation excludes unsound call-fold inputs. |
| A0670 | `candidates.rs:2274-2277;2113;7315` | UNDECIDED | `WriteOnlyPrimitiveStore` with `range_obligation: Some` is skipped in block-parameter rewrite only; primitive-store arms are no-ops elsewhere, so path divergence may be benign. |
| A0671 | `candidates.rs:10686-10692` | CLOSED | Region acceptance clones the unit then runs `revalidate_rewrite_output` → full `validate_psi_optimization_unit`; roster-related mutations are guarded in the prune replay path. |

### A0664

1. Read `candidates.rs:1528-1647` (`shared_jump_fusion::replay::validate`) and `candidates.rs:2007-2036` (`checked_scalar_substitutions`); read compare arms at `candidates.rs:557-569` and `candidates.rs:720-732` (adjacent / non-adjacent merge).
2. Search `rg '\bchecked_scalar_substitutions\b' /agent/repos/omega/omega-rust` → 7 matches, all in `candidates.rs` (definition plus three call sites at `557`, `720`, `1581`).
3. Caller chain read: `validate_psi_rewrite_candidate` (`candidates.rs:10362`) → `validate_shared_jump_fusion_candidate` (`candidates.rs:1720-1729`) → `replay::validate` (`candidates.rs:1528`).
4. Input on the accepting branch: `FuseSharedTerminalJump` patch; `patched_jump_bindings` (`candidates.rs:1547-1548`) supplies `bindings`; `checked_scalar_substitutions` (`candidates.rs:1581-1586`) accepts pairs when parameter/binding types align because `admits` is `|_, _| true`. Prior gates include `independently_post_dominates` (`candidates.rs:1578-1579`) and `reconstruct_adjacent_merge_ownership_is_identity` (`candidates.rs:1587-1593`).
5. **Verdict: UNDECIDED.** Entry description matches read at `1581-1586`. Merge validators require `independently_replacement_dominates_uses` (`candidates.rs:3304-3348`); shared-terminal fusion does not. Whether post-dominator plus ownership identity is enough for return-only terminals was not established. **Unsure:** full semantics of `reconstruct_adjacent_merge_ownership_is_identity` with multi-predecessor shared returns (`candidates.rs:1569-1576`).

### A0665

1. Read `candidates.rs:1750-1831` (`unreachable_private_machines::replay::validate`, `validator_active_source_ordinals`).
2. Search `rg '\bvalidator_active_source_ordinals\b' /agent/repos/omega/omega-rust` → 2 matches (`1778`, `1813`).
3. Compared producer `rule_active_source_ordinals` (`control_flow_cleanup.rs:1299-1317`), which uses `.expect` on `active.next()` where the validator omits that (`candidates.rs:1826-1828`).
4. Input on accepting branch: validated prune candidate whose custody rows are built with `source_ordinals[machine]` (`candidates.rs:1779-1785`).
5. **Verdict: UNDECIDED.** Silent omission when `active.next()` is `None` before ordinals exhaust is read at `1823-1829`; indexing at `1783` panics on missing keys (not acceptance). **Unsure:** whether `validate_psi_optimization_unit` (`unit_validation.rs:18260-18269`) forbids non-consecutive `pruned_machines.source_ordinal` values so every active function is always mapped (no `source_ordinal` check found in `validate_identity_and_fact_indexes`, `unit_validation.rs:9684-9729`).

### A0666

1. Read `candidates.rs:1834-1899` (`unreachable_private_machine_complement`, `validator_machine_references`) and `optimization-unit/src/lib.rs:252-280` (`operation_machine_references`).
2. Search `rg '\bunreachable_private_machine_complement\b' /agent/repos/omega/omega-rust` → 2 matches in `candidates.rs`.
3. Read producer counterpart `rule_unreachable_private_machine_complement` (`control_flow_cleanup.rs:1320-1366`); same seed sets, reference harvesting via `operation_machine_references` per node plus call graph.
4. Input: `input` unit entering prune validation; `expected_machines` (`candidates.rs:1760-1777`) must equal candidate machines; accepting branch removes those functions and extends `pruned_machines` (`candidates.rs:1794-1802`).
5. **Verdict: REACHABLE.** Entry is accurate: `operation_machine_references` (`lib.rs:256-280`) only extracts callees from a small operation subset; many operations return no machine references. An active machine referenced only through an unwalked carrier stays out of `reachable` (`candidates.rs:1870-1878`) and lands in the prune set; matching candidate validates and `validate_psi_optimization_unit` runs on output (`candidates.rs:1802`) without re-walking the full program call graph beyond stored tables.

### A0667

1. Read `candidates.rs:3366-3405`, `3448-3456`, `4123-4147`, `4225-4248` (compatible-policy keys and validation arms).
2. Entry description matches read: `arithmetic_key` (`3455`) omits overflow policy; leader requires `obligation.is_none()` (`3377-3383`); redundant may carry `Some(obligation)` (`3397-3404`).
3. Caller context: scalar CSE validator `validate_scalar_common_subexpression_candidate` path through `accept_region_rewrite` (`4370-4413`); did not read producer proposal in this slice.
4. Input on accepting branch: `CompatiblePolicy` CSE with leader wrapping/saturating (no obligation row) and redundant exact with accepted obligation witness (`4225-4248`).
5. **Verdict: UNDECIDED.** Deliberate policy erasure is read, not a accidental fallback. Soundness depends on `independently_accepted_operation_fact` (`3189-3216`) and unit-side obligation catalogs; that proof chain was not traced. **Unsure:** whether a valid unit can carry an obligation fact that does not justify saturating/exact interchange for the same operand pattern.

### A0668

1. Read `candidates.rs:7293-7320` (`rewrite_scalar_value_uses`) in full; read call sites via `rg rewrite_scalar_value_uses omega-rust` (13 references, all in `candidates.rs`).
2. Read CSE application `candidates.rs:4408-4410` (`sweep_output_nodes` + `rewrite_scalar_value_uses`).
3. Compared `normalize_redundant_parameter_observation_operation` `StructuralCase` arm (`2163-2179`) which edits payloads.
4. Input: CSE rewrite replacing `redundant_result` with `leader_result` across function nodes; `StructuralCase` operations are not descended.
5. **Verdict: UNDECIDED.** Entry matches `7315`. Whether a remaining use of `redundant_result` outside operation bodies (e.g. successor metadata) can pass `node_uses` checks at `4358-4363` without being rewritten was not shown. **Unsure:** how `expected_uses` treats `StructuralCase` successor bindings.

### A0669

1. Read `candidates.rs:9824-9851` (`call_result_constant`) and caller `plan_dispatch` (`9742-9744`, `9643-9778`).
2. Search `rg '\bcall_result_constant\b' /agent/repos/omega/omega-rust` → 2 definitions (`candidates.rs:9824`, `specialization.rs:478`) plus call sites.
3. Read `state_specialization::replay::validate` (`10126-10161`): recomputes `replayed` via `plan_dispatch` and requires `patch.edges == replayed`; `cyclic_machines` gate at `10147-10148`.
4. Input: dispatch specialization edges derived when an incoming argument is constant via facts or `call_result_constant` (unique `Return`, fact lookup at `9847-9850`; no recursion/effects check).
5. **Verdict: UNDECIDED.** Entry matches read; shared producer/validator rule means divergence is not caught by replay inequality, but whether unsound inputs can pass unit validation before this rule runs was not traced (`validator_scalar_constant_facts` / obligation validation). **Unsure:** obligation soundness for cross-machine constant facts.

### A0670

1. Read `rewrite_block_parameter_operation` `candidates.rs:2223-2282`; `normalize_redundant_parameter_observation_operation` `2113-2186`; `rewrite_scalar_value_uses` `7293-7320`.
2. Single `range_obligation` occurrence in `candidates.rs` at `2274-2277`.
3. Read `value_operand_rewrites` `10307-10337`: `StructuralCase` and `primitive_storage_observation_pattern` are no-ops (includes `WriteOnlyPrimitiveStore` via pattern at `10321`).
4. Input: redundant block-parameter removal applying rewriters across nodes (`2453-2461`); `WriteOnlyPrimitiveStore { range_obligation: Some(_), .. }` skipped only in `rewrite_block_parameter_operation`.
5. **Verdict: UNDECIDED.** Asymmetric arms are read, but `AbstractResult` in `WriteOnlyPrimitiveStore` (`operations.rs:320-326`) is not updated by `replace` in these rewriters anyway. **Unsure:** whether a block-parameter `ValueId` can appear in a field that one rewriter updates and another skips before `revalidate_rewrite_output` (`2463`).

### A0671

1. Read `accept_region_rewrite` `candidates.rs:10662-10698` and `revalidate_rewrite_output` `unit_validation.rs:18213-18226`.
2. Read prune replay mutation `candidates.rs:1794-1802` and catalog checks `unit_validation.rs:1128-1172`.
3. Sampled obligation-fact equality guards at `candidates.rs:4905`, `6028-6031`, `6387-6390`.
4. Input: most rewrites clone `input` (`10675`) so unit-level tables start identical; rewrite closures mutate functions; `revalidate_rewrite_output` recomputes identity and runs full unit validation.
5. **Verdict: CLOSED.** Entry description is partially inaccurate for default path: `accept_region_rewrite` does not itself diff `pruned_machines` / `accepted_obligation_facts`, but clone-plus-`validate_psi_optimization_unit` (`18225`) re-checks catalog invariants (`1128-1173`, `9684-9729`). Prune is the only `pruned_machines` mutator seen in `candidates.rs` and it calls `validate_psi_optimization_unit` explicitly (`1802`). Cross-function removal is guarded at `10686-10691`.

### What I did not read

- Producer proposal modules for shared-terminal fusion, scalar CSE, and state specialization beyond `specialization.rs:478-509` and `control_flow_cleanup.rs:1299-1366`.
- Full `validate_psi_optimization_unit` catalog phase beyond `index_and_validate_unit_catalogs` / `validate_identity_and_fact_indexes` openings.
- `reconstruct_shared_terminal_fusion_accounting` and complete `independently_replacement_dominates_uses` dominance proofs.
- Call sites of `validate_psi_rewrite_candidate` in `pass_manager.rs` and semantics `lib.rs` beyond grep counts.

SLICE COMPLETE
