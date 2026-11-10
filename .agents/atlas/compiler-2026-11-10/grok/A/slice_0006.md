# slice_0006

Digest: `area_08` (Psi stage 05 borrow ledger and checks; basenames under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` unless noted). Citations read at current checkout; digest anchor commit `457adf4a11f`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0041 | checks_content.rs:5395-5409 | REACHABLE | Partition conservation exits early with no diagnostic when input frontiers are closed but carry no owned qualified content, while outputs still need partition checks. |
| A0042 | checks_content.rs:6270 | REACHABLE | `check_callable` `continue`s when exactly one owned source is found via `owned_source_can_supply`, skipping ambiguous/borrowed-only retention errors. |
| A0043 | checks_content.rs:6005 | REACHABLE | `append_sources` treats several type forms with `_ => return true`, so nested qualified content inside them never enters borrowed/partition source lists. |
| A0044 | checks_content.rs:3834 | REACHABLE | `contract_place_matches` accepts parameter roots on spelling equality even when `root_symbol` differs from the expected parameter symbol. |
| A0045 | checks_contracts.rs:1201-1209 | REACHABLE | `bind_call_evidence_arguments` `continue`s with no diagnostic when `find_call_site` fails for a recorded contract call. |
| A0046 | checks_contracts.rs:244-261 | REACHABLE | Assembly-fact checking returns silently when machine/state lookup or per-statement flow facts are missing. |
| A0047 | checks_contracts.rs:1566-1587, 690-696 | REACHABLE | Nominal field checks and contextual cleanup both return without diagnostics on missing call target metadata or sites. |
| A0048 | checks_contracts.rs:2873, 2876 | REACHABLE | `instantiated_live_value_proves` passes `&\|_, _\| true` as `comparison_is_admitted` to scalar `evaluate`, admitting every comparison pair during requires proof. |

## A0041

**Claim (read):** Early return in partition result checking accepts bodyless boundary signatures whose returns carry qualified nested owned outputs while no input supplies owned qualified content.

**Site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_content.rs:5359-5554`, function `partitioned_results::check_boundary_partition_results`. At `5395-5408`, when `complete` holds and every parameter’s `parameter_sources` expansion has `frontier_is_closed()` and every source is either borrowed or lacks a content qualification row, the function `return`s before `valid` / error at `5550-5553`.

**Callers (read):** Search `\bcheck_boundary_partition_results\b` in `omega-rust`: 5 matches in `checks_content.rs` (1 definition, 1 re-export, 2 production calls at `7249`, `7277`, 1 `use`). Production path: `check_retained_content_custody` (`7211-7294`) calls it for boundary trait signatures (`7248-7258`) and for machine state contract frames when `!validation::call_target_has_checked_body_contract` (`7276-7286`).

**Upstream (read):** `check_retained_content_custody` is invoked from `checks.rs:12144` inside `check_checked_facts_recording` (`12022+`); diagnostics prevent publishing retained rows but do not by themselves re-run partition logic elsewhere in this trace.

**Input (read):** A bodyless boundary or declaration-only callable (no checked body per `7276`) whose return type yields nested owned qualified `outputs` (non-empty after filter `5372-5387`) while each parameter’s `domain_sources` / `parameter_sources` (`5799+`, `5884+`) is `complete` with only borrowed or unqualified owned carriers (`5397-5405`). The early `return` emits no diagnostic; later partition error paths (`5471+`, `5550+`) are not reached.

**Verdict:** REACHABLE.

## A0042

**Claim (read):** One owned source is accepted on `owned_source_can_supply` alone.

**Site (read):** `checks_content.rs:6150-6354`, function `check_callable`. At `6243-6258`, a parameter joins `owned_sources` when multiplicity is `Linear` and `owned_source_can_supply` (`6361-6383`) returns true for some compatible plan. At `6270-6272`, if `owned_sources.len() == 1` OR both owned and borrowed source lists are empty, the loop `continue`s—skipping errors at `6317-6336` (ambiguous owned or borrowed-only retention).

**Callers (read):** Search `\bcheck_callable\b` in `omega-rust/psi`: 5 matches, all in `checks_content.rs` (definition plus calls at `7185`, `7235`, `7264`, and internal `use`).

**Upstream (read):** Same `check_retained_content_custody` → `check_checked_facts_recording` chain as A0041.

**Input (read):** Callable with one linear parameter for which `owned_source_can_supply` is true (including `OwnedCarrierLoanRelation::ViewFree` → `true` at `6380`) and a content-bearing result domain, with no second owned or borrowed source triggering the later error arms.

**Verdict:** REACHABLE.

## A0043

**Claim (read):** Owned qualified content nested inside a type form not handled in the `match` is invisible to borrowed-source and partition checks.

**Site (read):** `checks_content.rs:5903-6050+`, function `structural_sources::append_sources` inside `partitioned_results`. After constrained/reference/array arms, `TypeReferenceNode::Named` / `Generic` fall through to member walk; at `6002-6005`, `FixedArray { .. }` (non-literal length) returns `false`, but `_ => return true` at `6005` completes without recursing into carriers for other node kinds. `domain_sources` (`5880-5899`) and `parameter_sources` (`5799+`) depend on `append_sources` for `complete` and source lists used by `check_boundary_partition_results` and `check_structural_borrow_sources`.

**Callers (read):** Search `\bappend_sources\b` in `checks_content.rs`: 7 matches (definition plus internal recursion and `domain_sources`).

**Input (read):** Return or parameter type whose outer shape hits the `_ => return true` arm while qualified domain constraints live only on an inner path that traversal never reaches, yielding `complete == true` and an empty or incomplete owned-source set for partition/borrowed checks.

**Verdict:** REACHABLE.

## A0044

**Claim (read):** Contract membership whose place text spells the same as the structural subject but resolves to a different symbol still matches.

**Site (read):** `checks_content.rs:3821-3837`, `contract_place_matches`. For `ContentPlaceRoot::Parameter`, `3833-3834` sets `root_matches` when symbols agree and are valid **or** when `*name == root_name` regardless of symbol agreement. Used from `contracts_establish_domain` (`3774-3817`) at `3816`, which feeds `applicable_projection_plans` (`3713-3742`).

**Callers (read):** Search `\bcontract_place_matches\b` in `omega-rust`: 2 matches (definition + use in `checks_content.rs`). Search `\bcontracts_establish_domain\b`: 4 matches (`checks_content.rs` + duplicate logic name in `completed_storage_and_conservation.rs`, not read).

**Upstream (read):** `applicable_projection_plans` used from content claim checking around `6809-6824` in the same file (read at `6785-6824`).

**Input (read):** `requires`/`ensures` membership expression whose root name string equals a parameter’s `name` but whose resolved `root_symbol` is a different valid symbol, with matching segment paths via `content_paths_match` (`3890+`).

**Verdict:** REACHABLE.

## A0045

**Claim (read):** Evidence binding skipped when the call site is not found.

**Site (read):** `checks_contracts.rs:1186-1259+`, `evidence::bind_call_evidence_arguments`. For each `facts.proof.contract_calls` row (`1193-1198`), `find_call_site` (`1201-1207`); on `None`, `continue` (`1208-1209`) with no diagnostic—later binding/count checks for that call are skipped.

**Callers (read):** Search `\bbind_call_evidence_arguments\b` in `omega-rust`: 4 matches (`checks_contracts.rs` wrapper at `11868-11874`, inner at `1186`, comment/reference at `8`, `checks.rs:12055`).

**Upstream (read):** `check_checked_facts_recording` (`checks.rs:12055`) runs binding before `check_flow_call_contracts` (`12080`). Missing binding does not automatically fail compilation.

**Input (read):** Proof facts contain a `contract_calls` entry whose caller machine/state/statement/call ordinal does not resolve via `validation::find_call_site`, so evidence argument arity/shape checks never run for that handle.

**Verdict:** REACHABLE.

## A0046

**Claim (read):** Assembly fact not checked when machine, state, or statement flow is missing.

**Site (read):** `checks_contracts.rs:228-320`, module `assembly`. `check_assembly_fact_contracts` (`228-235`) iterates flow states and calls `check_state_assembly_facts` (`238-320`). `check_state_assembly_facts`: `machine_state_by_symbols` failure → `return` (`244-249`); per statement, missing `facts.flow.state_statement` → `continue` (`258-260`), skipping proof and diagnostics for that asm fact.

**Callers (read):** Search `\bcheck_assembly_fact_contracts\b` in `omega-rust`: 2 matches (definition + `check_flow_call_contracts` at `11630`).

**Upstream (read):** `check_flow_call_contracts` (`11607+`) runs from `checks.rs:12080` after borrows; failures append diagnostics only when the check reaches `314-319`.

**Input (read):** Program state with an `AssemblyFact` statement in the typed tree but no matching `FlowStateFact` machine/state pair or no `state_statement` flow row for that index—asm contract obligations are not discharged and no error is emitted.

**Verdict:** REACHABLE.

## A0047

**Claim (read):** Nominal field check skipped; cleanup check skipped when `target_machine` is `None`.

**Sites (read):**

- `checks_contracts.rs:1558-1587`, `nominal_inputs::check`: early `return` for empty requirements (`1566-1567`), missing `call_target_parameters` (`1569-1571`), no parameter with requirements (`1573-1577`), or missing `find_call_site` (`1579-1586`).
- `checks_contracts.rs:681-696`, `contextual_cleanup::check`: if `facts.contextual_cleanup.entry_requirements` non-empty but `target_machine(program, call)` is `None` (`694-695`), `return` with no diagnostic.

**Callers (read):** Search `nominal_inputs::check` and `contextual_cleanup::check` in `omega-rust`: 2 matches each, both in `checks_contracts_calls.rs:3213-3231` inside `check_call_requires`. `target_machine` defined at `6634-6644` (read).

**Upstream (read):** `check_call_requires` called per call from `check_flow_call_contracts` (`11722-11734`). A skipped check leaves call requires to other prover routes; skipped cleanup never emits errors from this module.

**Input (read):** Flow call whose `target_symbol` does not resolve to a machine (`target_machine` `None`), or whose statement index/ordinal does not map to a call site for nominal field requirements—declared field domain capture checks at `1599+` never run.

**Verdict:** REACHABLE.

## A0048

**Claim (read):** Third closure `&|_, _| true`; what it gates was not traced in digest.

**Site (read):** `checks_contracts.rs:2786-2877`, `instantiated_live_value_proves`. Nested calls to `evaluate_scalar` (re-export of `checked_trees::facts::scalar_predicates::evaluate`, read `facts.rs:270-274`) pass `&|_, _| true` as `comparison_is_admitted` at `2873` and `2876`. In `numerics::scalar_predicates::evaluate_nodes` (read `lib.rs:744-746`), comparisons fail when `!comparison_is_admitted(&left, &right)`; always-true closure admits all pairs.

**Callers (read):** Search `\binstantiated_live_value_proves\b` in `omega-rust`: 2 matches (definition + call from `direct_context_proves_instantiated_boolean_expression` at `2704-2717`). Search `\bdirect_context_proves_instantiated_boolean_expression\b`: 2 matches (`checks_contracts.rs:2654`, `checks_contracts_prover.rs:867`).

**Upstream (read):** `semantic_context_proves_instantiated_boolean_expression` (`checks_contracts_prover.rs:830+`) calls `direct_context_proves_instantiated_boolean_expression` first (`867-882`); used on call-requires boolean proof paths (`checks_contracts_prover.rs` module header read at `1-18`).

**Input (read):** Callee `requires` boolean expression proved via the instantiated live-value route where scalar subexpressions need comparison admission; supplying always-true admission allows `evaluate_scalar` to produce `Boolean(true)` (`2877`) even when leaf values would not satisfy a restricted admission predicate.

**Verdict:** REACHABLE (read: `comparison_is_admitted` gates scalar comparisons in `evaluate`).

## What I did not read

- `validation::borrow_contracts::owned_carrier_loan_relation` implementation behind `owned_source_can_supply` (`6380-6382`).
- Full body of `nominal_inputs::check` after `1599` (field proof success/failure paths).
- Duplicate `contracts_establish_domain` in `omega-rust/psi/compilation/validation/src/completed_storage_and_conservation.rs`.
- Whether any later stage re-validates partition geometry or evidence binding for the skipped cases above (no closing check identified in the traced upward paths).

SLICE COMPLETE

