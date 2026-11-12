# Soundness mutations shard 4

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (measured `d580b1bdc60cfe1eb379b58a499c3ac1bf137a06`). Host: linux cloud agent. Generated 2026-11-12 13:22 UTC.

## Bucket table (mutants only)

Total mutants run: **161** (must equal sum of buckets).

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 37 |
| `disagreement-valid` | 17 |
| `discarded-valid` | 107 |
| **Sum** | **161** |

- Runner stats: generated=3650, run=161, logged=161, current_rejected=?, not_applicable=0, failed_run=?. Delta between generated and logged = early rejects + unlogged kinds before drop_requires rule.


## Fixture / mutant reconcile (fix4)

- Fixtures in shard: **1130**
- Fixtures with no logged mutant row: **1076**
- Fixtures with ≥1 mutant row: **54**
- Mutant rows in TSV: **161**
- Fixtures with >1 mutant row: **46**

| Fixture | Mutant rows |
| --- | ---: |
| `tests/omega/pass/contracts/contradictory_branch_closes_obligation_exit/main.omg` | 2 |
| `tests/omega/pass/domains/runtime_stored_call_inequality_addition_exit/main.omg` | 3 |
| `tests/omega/pass/facts/abi_multi_arg_spill_through_local_exit/main.omg` | 3 |
| `tests/omega/pass/facts/aggregate_struct_return_abi_through_call_result_exit/main.omg` | 3 |
| `tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg` | 4 |
| `tests/omega/pass/facts/bounded_scalar_sums_through_other_machine_exit/main.omg` | 4 |
| `tests/omega/pass/facts/callee_saved_across_nested_calls_through_call_result_exit/main.omg` | 3 |
| `tests/omega/pass/facts/case_payload_transport_through_call_result_exit/main.omg` | 3 |
| `tests/omega/pass/facts/completed_integer_carrier_bounds_through_parameter_exit/main.omg` | 4 |
| `tests/omega/pass/facts/five_variant_enum_payload_bound_through_other_state_exit/main.omg` | 3 |
| `tests/omega/pass/facts/four_variant_enum_payload_bound_through_call_result_exit/main.omg` | 3 |
| `tests/omega/pass/facts/integer_width_truncation_mask_bound_through_local_exit/main.omg` | 2 |
| `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_local_exit/main.omg` | 4 |
| `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_state_exit/main.omg` | 3 |
| `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_machine_exit/main.omg` | 3 |
| `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_call_result_exit/main.omg` | 2 |
| `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_parameter_exit/main.omg` | 2 |
| `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_other_state_exit/main.omg` | 4 |
| `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_other_machine_exit/main.omg` | 4 |
| `tests/omega/pass/facts/narrow_to_wide_unsigned_32to64_extension_through_local_exit/main.omg` | 4 |
| … | (26 more) |

- Confirmed holes by mutation kind: delete_divisor_guard=1, drop_requires=2, remove_transition_guard=2, widen_requires_lt=32
- Confirmed holes by accepted_by: both=21, current=16

## Commands

```bash
bash tools/agent_setup.sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison < /dev/null
export OMEGA_LIBRARY_ROOT=$PWD/source/library
python3 .agents/four-jobs-2026-11-12/soundness/shard4_runner.py
python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py
```

## Hand verification — confirmed holes (fix4)

### `tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg` / widen_requires_lt → confirmed-hole

Route reason: `route_us=9633 discharge_us=9148 reverify_us=7477 reverified=10 obligations=13 judged=10 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=1 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_products_through_other_state_exit_main_omg_widen__7f73055451/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_products_through_other_state_exit_main_omg_widen__7f73055451/main.omg	current=accepted	current_us=3339502	root=compilation	route=rejected	route_us=22718	discharge_us=15960	reverify_us=7679	reverified=10	obligations=13	judged=10	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/bounded_scalar_sums_through_other_machine_exit/main.omg` / widen_requires_lt → confirmed-hole

Route reason: `route_us=9304 discharge_us=8933 reverify_us=7732 reverified=12 obligations=15 judged=12 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=3 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_scalar_sums_through_other_machine_exit_main_omg_w_b741bcc800/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_scalar_sums_through_other_machine_exit_main_omg_w_b741bcc800/main.omg	current=accepted	current_us=2997477	root=compilation	route=accepted	route_us=9902	discharge_us=9471	reverify_us=7709	reverified=12	obligations=15	judged=12	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=3	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/case_payload_transport_through_call_result_exit/main.omg` / widen_requires_lt → confirmed-hole

Route reason: `route_us=9718 discharge_us=9248 reverify_us=8000 reverified=9 obligations=10 judged=9 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=2 published_held=1 uses=4 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_case_payload_transport_through_call_result_exit_main_omg__4f7b57452e/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_case_payload_transport_through_call_result_exit_main_omg__4f7b57452e/main.omg	current=accepted	current_us=3020783	root=compilation	route=accepted	route_us=10028	discharge_us=9577	reverify_us=7840	reverified=9	obligations=10	judged=9	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=2	published_held=1	uses=4	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/completed_integer_carrier_bounds_through_parameter_exit/main.omg` / widen_requires_lt → confirmed-hole

Route reason: `route_us=1615 discharge_us=1167 reverify_us=615 reverified=5 obligations=5 judged=5 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=3 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_completed_integer_carrier_bounds_through_parameter_exit_m_d598833429/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_completed_integer_carrier_bounds_through_parameter_exit_m_d598833429/main.omg	current=accepted	current_us=2996514	root=compilation	route=accepted	route_us=1495	discharge_us=1131	reverify_us=582	reverified=5	obligations=5	judged=5	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=3	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/five_variant_enum_payload_bound_through_other_state_exit/main.omg` / widen_requires_lt → confirmed-hole

Route reason: `route_us=11045 discharge_us=10247 reverify_us=8214 reverified=12 obligations=13 judged=12 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=5 published_held=1 uses=7 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff

```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_five_variant_enum_payload_bound_through_other_state_exit__c77a915c63/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_five_variant_enum_payload_bound_through_other_state_exit__c77a915c63/main.omg	current=accepted	current_us=4682082	root=compilation	route=accepted	route_us=10247	discharge_us=9522	reverify_us=8329	reverified=12	obligations=13	judged=12	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=5	published_held=1	uses=7	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

## Hand verification — discarded drop_requires (sample)

### `tests/omega/pass/facts/projection_call_premise_through_parameter_exit/main.omg` / drop_requires → discarded-valid

Route reason: `route_us=822 discharge_us=372 reverify_us=55 reverified=2 obligations=2 judged=2 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=4 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
Discard reason: ``
Obligation: ``

```diff
--- original
+++ mutant
@@ -6,5 +6,4 @@
 
 machine project(mark: Mark) -> bool
-requires mark.kept
 {
     mark.kept
@@ -12,5 +11,4 @@
 
 machine hold(direct: bool) -> bool
-requires direct == true
 {
     true
@@ -18,5 +16,4 @@
 
 machine observe(mark: Mark) -> bool
-requires mark.kept
 {
     project(mark)
```

Current checker exit=0:

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_projection_call_premise_through_parameter_exit_main_omg_d_032ef57012/main.omg; wrote_output=false

```

Route (first program line + diagnostics):

```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_projection_call_premise_through_parameter_exit_main_omg_d_032ef57012/main.omg	current=accepted	current_us=3032137	root=compilation	route=accepted	route_us=1008	discharge_us=496	reverify_us=69	reverified=2	obligations=2	judged=2	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=4	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

## Hand verification — known-gap-q7-mut (sample)

_None in this category._

## Confirmed holes and disagreements

### 1. `tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_products_through_other_state_exit_main_omg_widen__7f73055451`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=9633 discharge_us=9148 reverify_us=7477 reverified=10 obligations=13 judged=10 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=1 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_products_through_other_state_exit_main_omg_widen__7f73055451; wrote_output=false

```

### 2. `tests/omega/pass/facts/bounded_scalar_sums_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_scalar_sums_through_other_machine_exit_main_omg_w_b741bcc800`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=9304 discharge_us=8933 reverify_us=7732 reverified=12 obligations=15 judged=12 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=3 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_bounded_scalar_sums_through_other_machine_exit_main_omg_w_b741bcc800; wrote_output=false

```

### 3. `tests/omega/pass/facts/case_payload_transport_through_call_result_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_case_payload_transport_through_call_result_exit_main_omg__4f7b57452e`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=9718 discharge_us=9248 reverify_us=8000 reverified=9 obligations=10 judged=9 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=2 published_held=1 uses=4 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_case_payload_transport_through_call_result_exit_main_omg__4f7b57452e; wrote_output=false

```

### 4. `tests/omega/pass/facts/completed_integer_carrier_bounds_through_parameter_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_completed_integer_carrier_bounds_through_parameter_exit_m_d598833429`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=1615 discharge_us=1167 reverify_us=615 reverified=5 obligations=5 judged=5 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=3 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_completed_integer_carrier_bounds_through_parameter_exit_m_d598833429; wrote_output=false

```

### 5. `tests/omega/pass/facts/five_variant_enum_payload_bound_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_five_variant_enum_payload_bound_through_other_state_exit__c77a915c63`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=11045 discharge_us=10247 reverify_us=8214 reverified=12 obligations=13 judged=12 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=5 published_held=1 uses=7 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_five_variant_enum_payload_bound_through_other_state_exit__c77a915c63; wrote_output=false

```

### 6. `tests/omega/pass/facts/four_variant_enum_payload_bound_through_call_result_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_four_variant_enum_payload_bound_through_call_result_exit__f0dc8346ba`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=10944 discharge_us=10085 reverify_us=9045 reverified=11 obligations=12 judged=11 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=4 published_held=1 uses=6 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_four_variant_enum_payload_bound_through_call_result_exit__f0dc8346ba; wrote_output=false

```

### 7. `tests/omega/pass/facts/live_across_two_calls_preservation_bound_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_live_across_two_calls_preservation_bound_through_local_ex_23b1dbfb91`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=10449 discharge_us=9857 reverify_us=8497 reverified=12 obligations=14 judged=12 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=2 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_live_across_two_calls_preservation_bound_through_local_ex_23b1dbfb91; wrote_output=false

```

### 8. `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_join_multi_variant_bound_through_other_state_exit_m_bdeeda2396`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=2451 discharge_us=1966 reverify_us=1161 reverified=8 obligations=10 judged=8 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=5 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_match_join_multi_variant_bound_through_other_state_exit_m_bdeeda2396; wrote_output=false

```

### 9. `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_multi_exit_callee_saved_restoration_through_other_machine_4cb483c24c`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=234064 discharge_us=233389 reverify_us=228790 reverified=45 obligations=51 judged=45 core_refused=0 unproved=4 refused_other=0 needed_relation=0 needed_branch=0 published_held=2 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_multi_exit_callee_saved_restoration_through_other_machine_4cb483c24c; wrote_output=false

```

### 10. `tests/omega/pass/facts/narrow_to_wide_signed_16to64_extension_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_signed_16to64_extension_through_other_stat_bfc8ace1ac`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=54619 discharge_us=54023 reverify_us=54271 reverified=10 obligations=10 judged=10 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_signed_16to64_extension_through_other_stat_bfc8ace1ac; wrote_output=false

```

### 11. `tests/omega/pass/facts/narrow_to_wide_signed_8to64_extension_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_signed_8to64_extension_through_other_machi_bcd7823b29`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=56425 discharge_us=55975 reverify_us=50266 reverified=11 obligations=13 judged=11 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=2 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_signed_8to64_extension_through_other_machi_bcd7823b29; wrote_output=false

```

### 12. `tests/omega/pass/facts/narrow_to_wide_unsigned_32to64_extension_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_unsigned_32to64_extension_through_local_ex_299fa51bf2`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=3995 discharge_us=3615 reverify_us=2974 reverified=7 obligations=7 judged=7 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_narrow_to_wide_unsigned_32to64_extension_through_local_ex_299fa51bf2; wrote_output=false

```

### 13. `tests/omega/pass/facts/nested_record_value_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_nested_record_value_through_other_state_exit_main_omg_wid_2fd59f8122`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=9587 discharge_us=9074 reverify_us=7628 reverified=9 obligations=12 judged=9 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=1 uses=10 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_nested_record_value_through_other_state_exit_main_omg_wid_2fd59f8122; wrote_output=false

```

### 14. `tests/omega/pass/facts/range_fact_delivery_through_other_machine_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_d_af04234eab`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=1721 discharge_us=1292 reverify_us=713 reverified=7 obligations=7 judged=7 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires index <= 4
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // For u64 index, index < 4 proves index <= 4 at count(index).`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -1,7 +1,6 @@
 // For u64 index, index < 4 proves index <= 4 at count(index).
 // Expected exit: 0.
 machine count(index: u64) -> u64
-requires index <= 4
 { index }
 machine forward(index: u64) -> u64 requires index < 4 { count(index) }
 machine probe(input: u64) -> u64 requires input < 4 { forward(input) }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_d_af04234eab; wrote_output=false

```

### 15. `tests/omega/pass/facts/range_fact_delivery_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_w_9adee219a0`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=2289 discharge_us=1637 reverify_us=778 reverified=8 obligations=8 judged=7 core_refused=1 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_range_fact_delivery_through_other_machine_exit_main_omg_w_9adee219a0; wrote_output=false

```

### 16. `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_64bit_bitwise_and_mask_bound_through_other_machine_68b66c02c2`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=36914 discharge_us=36419 reverify_us=32716 reverified=7 obligations=8 judged=7 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_64bit_bitwise_and_mask_bound_through_other_machine_68b66c02c2; wrote_output=false

```

### 17. `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_64bit_bitwise_xor_bound_through_local_exit_main_om_ec6faddc9b`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=53462 discharge_us=53071 reverify_us=50554 reverified=8 obligations=9 judged=8 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_64bit_bitwise_xor_bound_through_local_exit_main_om_ec6faddc9b; wrote_output=false

```

### 18. `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_division_quotient_bound_through_other_state_e_9046c49bbf`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=4014 discharge_us=3556 reverify_us=2570 reverified=7 obligations=8 judged=7 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_division_quotient_bound_through_other_state_e_9046c49bbf; wrote_output=false

```

### 19. `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_division_remainder_bound_through_other_machin_b8422ea4dd`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=6479 discharge_us=5888 reverify_us=4958 reverified=8 obligations=11 judged=8 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=2 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_division_remainder_bound_through_other_machin_b8422ea4dd; wrote_output=false

```

### 20. `tests/omega/pass/facts/signed_8bit_exact_multiplication_bound_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_exact_multiplication_bound_through_local_exit_0d0d86fcd1`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=8894 discharge_us=8329 reverify_us=6301 reverified=7 obligations=7 judged=7 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_exact_multiplication_bound_through_local_exit_0d0d86fcd1; wrote_output=false

```

### 21. `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_parameter_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_shift_right_bound_through_parameter_exit_main_0c01ce58c8`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=3777 discharge_us=3061 reverify_us=2297 reverified=5 obligations=6 judged=5 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_8bit_shift_right_bound_through_parameter_exit_main_0c01ce58c8; wrote_output=false

```

### 22. `tests/omega/pass/facts/signed_arithmetic_shift_right_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_arithmetic_shift_right_through_other_machine_exit__ec0dfa5c7d`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=2522 discharge_us=1797 reverify_us=1182 reverified=6 obligations=8 judged=6 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_arithmetic_shift_right_through_other_machine_exit__ec0dfa5c7d; wrote_output=false

```

### 23. `tests/omega/pass/facts/signed_exact_subtraction_bound_through_call_result_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_exact_subtraction_bound_through_call_result_exit_m_6fb80e914f`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=19114 discharge_us=18553 reverify_us=16769 reverified=9 obligations=10 judged=9 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=1 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_exact_subtraction_bound_through_call_result_exit_m_6fb80e914f; wrote_output=false

```

### 24. `tests/omega/pass/facts/signed_shift_left_bound_through_parameter_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_shift_left_bound_through_parameter_exit_main_omg_w_cf47e847eb`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=15315 discharge_us=14641 reverify_us=12186 reverified=9 obligations=16 judged=12 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=2 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_signed_shift_left_bound_through_parameter_exit_main_omg_w_cf47e847eb; wrote_output=false

```

### 25. `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_32bit_division_remainder_bound_through_other_mac_66fd7aa6f8`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=2833 discharge_us=2408 reverify_us=1078 reverified=6 obligations=8 judged=7 core_refused=1 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_32bit_division_remainder_bound_through_other_mac_66fd7aa6f8; wrote_output=false

```

### 26. `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_64bit_bitwise_or_bound_through_local_exit_main_o_dc6d5f538d`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=9810 discharge_us=9401 reverify_us=7977 reverified=6 obligations=7 judged=6 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_64bit_bitwise_or_bound_through_local_exit_main_o_dc6d5f538d; wrote_output=false

```

### 27. `tests/omega/pass/facts/unsigned_byte_to_signed_case_domain_through_other_machine_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_byte_to_signed_case_domain_through_other_machine_eb9a8c2830`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=7944 discharge_us=7190 reverify_us=6030 reverified=9 obligations=9 judged=9 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=1 published_held=0 uses=8 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_byte_to_signed_case_domain_through_other_machine_eb9a8c2830; wrote_output=false

```

### 28. `tests/omega/pass/facts/unsigned_division_remainder_through_local_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_division_remainder_through_local_exit_main_omg_w_8d369a68f1`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=5775 discharge_us=5378 reverify_us=4610 reverified=6 obligations=8 judged=6 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_division_remainder_through_local_exit_main_omg_w_8d369a68f1; wrote_output=false

```

### 29. `tests/omega/pass/facts/unsigned_exact_subtraction_interval_through_call_result_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_exact_subtraction_interval_through_call_result_e_88e014d07b`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=15185 discharge_us=14725 reverify_us=13651 reverified=10 obligations=11 judged=10 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=1 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_exact_subtraction_interval_through_call_result_e_88e014d07b; wrote_output=false

```

### 30. `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit/main.omg` (drop_requires)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__64ddccca7c`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=1277 discharge_us=904 reverify_us=394 reverified=8 obligations=12 judged=10 core_refused=0 unproved=2 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **probe-not-built** — no automated probe for: requires val >= 400 && val <= 400 && shift >= 2 && shift <= 2
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): `1: // Logical right shift on a bounded u64 value by a bounded shift count establishes the exact post-shift interval, discharging exact subtraction shifted - 100u64.`
- Spec file/heading/quote (fix4a): `wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md` — (Nested machines and identity) A written entry contract remains complete even when all currently visible callers happen to establish stronger facts. | (Private entry inference) An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,6 @@
     }
 
     machine compute(val: u64, shift: u64)
-    requires val >= 400 && val <= 400 && shift >= 2 && shift <= 2
     {
         let shifted: u64 = val >> shift;
         let res: u64 = shifted - 100;
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__64ddccca7c; wrote_output=false

```

### 31. `tests/omega/pass/facts/unsigned_shift_left_bound_through_parameter_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_left_bound_through_parameter_exit_main_omg_9231e6323e`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=8774 discharge_us=8214 reverify_us=6852 reverified=4 obligations=5 judged=4 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_left_bound_through_parameter_exit_main_omg_9231e6323e; wrote_output=false

```

### 32. `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **current**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__093442af7b`
- Current: **accepted**; route: **rejected**
- Route hint (not validity): `route_us=2364 discharge_us=1991 reverify_us=1428 reverified=10 obligations=13 judged=12 core_refused=0 unproved=1 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_unsigned_shift_right_bound_through_other_state_exit_main__093442af7b; wrote_output=false

```

### 33. `tests/omega/pass/facts/wrapping_integer_arithmetic_through_parameter_exit/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_wrapping_integer_arithmetic_through_parameter_exit_main_o_9ea0b18dee`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=1415 discharge_us=1095 reverify_us=483 reverified=4 obligations=4 judged=4 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_facts_wrapping_integer_arithmetic_through_parameter_exit_main_o_9ea0b18dee; wrote_output=false

```

### 34. `tests/omega/pass/optimizer/rollback_control_flow_cleanup_control/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_control_flow_cleanup_control_main_omg_remove_3f2efb158d`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=342 discharge_us=96 reverify_us=0 reverified=0 obligations=0 judged=0 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `12: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -9,7 +9,7 @@
 machine main() {
     let decided: bool = true;
 
-    transition decided {
+    transition {
         true -> taken()
         _ -> untaken()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_control_flow_cleanup_control_main_omg_remove_3f2efb158d; wrote_output=false

```

### 35. `tests/omega/pass/optimizer/rollback_proof_check_elision_control/main.omg` (remove_transition_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_proof_check_elision_control_main_omg_remove__6cc0016cdc`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=535 discharge_us=270 reverify_us=26 reverified=1 obligations=1 judged=1 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): transition guard is a proof obligation at check time
- Obligation line (fix4c): `17: transition {`
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Live facts) Caller-visible mutation summaries include operand evaluation and callee writes.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff
--- original
+++ mutant
@@ -14,7 +14,7 @@
     let produced: i32 = source();
     let folded: i32 = produced - produced;
 
-    transition folded == 0 {
+    transition {
         true -> done()
         _ -> done()
     }
```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_optimizer_rollback_proof_check_elision_control_main_omg_remove__6cc0016cdc; wrote_output=false

```

### 36. `tests/omega/pass/places/requires_window_closed_before_use/main.omg` (widen_requires_lt)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_places_requires_window_closed_before_use_main_omg_widen_require_ab968768cf`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=1795 discharge_us=1475 reverify_us=823 reverified=2 obligations=3 judged=2 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=3 unestablished=0 windows=1 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): spec quote targets proof-time establishment (requires/ownership/contracts)
- Obligation line (fix4c): ``
- Spec file/heading/quote (fix4a): `wiki/spec/language/state_contracts.md` — (Requirement clauses) Machine entry obligations established at every invocation under exact actual-argument substitution.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_pass_places_requires_window_closed_before_use_main_omg_widen_require_ab968768cf; wrote_output=false

```

### 37. `tests/omega/run/facts/signed_division_quotient_truncation_not_floor_traps/main.omg` (delete_divisor_guard)

- Bucket: `confirmed-hole`; accepted_by: **both**
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_run_facts_signed_division_quotient_truncation_not_floor_traps_main_o_c531e9d86c`
- Current: **accepted**; route: **accepted**
- Route hint (not validity): `route_us=18998 discharge_us=18696 reverify_us=16533 reverified=5 obligations=5 judged=5 core_refused=0 unproved=0 refused_other=0 needed_relation=0 needed_branch=0 published_held=0 uses=0 unestablished=0 windows=0 consumed=0 closed=0 weakened_joins=0`
- hole_subtype (fix5): **** — 
- Check-time note (fix4a): nonzero divisor must be proved before formation (numeric_values.md)
- Obligation line (fix4c): `2: // -7i64 / 2i64 is -3i64 (truncation), not -4i64 (floor). This program asserts`
- Spec file/heading/quote (fix4a): `wiki/spec/language/numeric_values.md` — (Mathematical division) The divisor must be nonzero before formation, including for symbolic operands.
- Check-only evidence (fix4b): `omega --check` exit **0** on mutant; no runtime witness run in this shard

```diff

```

```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-4/tests_omega_run_facts_signed_division_quotient_truncation_not_floor_traps_main_o_c531e9d86c; wrote_output=false

```

