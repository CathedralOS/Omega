# slice_0019

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0141 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/products.rs:809,819 | CLOSED | Post-check pruning keeps expression-site nominal/interrupt rows as documented dead evidence; not an acceptance gate. |
| A0142 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:8387-8586 | REACHABLE | Call-site proposition facts publish with invalid `instantiated` when normalization fails; only boolean expressions are filtered. |
| A0143 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:6796-6806 | UNDECIDED | Unknown constraint names are silently skipped; whether earlier validation requires every name to be domain or carry not read. |
| A0144 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:10387-10391 | UNDECIDED | Invalid member symbols are dropped from published places; whether such paths can reach here after name resolution not traced. |
| A0145 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:9732 | UNDECIDED | `ProofObligationOwner::Unknown` maps to `ProgramPoint::Global`; whether checked programs can carry unknown owners not read. |
| A0146 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:4465-4521 | CLOSED | Classification failures error only when outcome proof selectors are non-empty; empty selector arms intentionally skip. |
| A0147 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:2483-2485 | UNDECIDED | Machines with no states skip evidence definite-assignment validation; whether such machines can still have forwarding outputs not read. |
| A0148 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published.rs:790-792 | UNDECIDED | Invalid call signature makes `owns_contract` true for every non-`CallSignature` contract; full contract admission path not traced. |

## A0141

**Digest (read):** area_09 — nominal-use and interrupt rows at an expression site are kept regardless of owning machine (`PROD:809`, `819`).

**Cited site (read):** `products.rs:801-820` inside `rewrite::apply_fact_pruning` (`products.rs:653-869`). Statement-site rows use `statement_machine` and `retained.contains(owner)`; `NominalMachineUseSite::Expression(_)` arms at `:809` and `:819` always return `true` from `retain` predicates (lines match digest cite at this checkout).

**Enclosing function (read):** `apply_fact_pruning` rebuilds machine-keyed fact tables after root selection. Nominal-machine-use and installed-interrupt registration tables use the expression-site exception above.

**Callers (read):** `rg '\bapply_fact_pruning\b' /agent/repos/omega` → 2 matches in `products.rs` (definition `:653`, call from `rewrite::apply_pruning` at `:649`). `apply_pruning` is called from `prune_checked_tree_product` at `products.rs:1184` (1 production path).

**Upstream context (read):** `product_pruning` module comment at `products.rs:215-278` states pruning runs only after authored checking is complete (`:218-222`) and that arena rows keyed by non-machine identities may remain as “retained dead evidence” unreachable through the pruned declaration surface (`:270-274`). `interface_retention_set` at `:380-394` still retains machines referenced from nominal-use rows (including expression sites) via `retain_symbol`.

**Input that reaches the branch (read):** Product pruning with a pruned caller machine and a nominal-machine-use or interrupt registration whose `site` is `Expression(_)`.

**Verdict:** CLOSED — this phase does not decide program acceptance; it mutates an already-checked product. Expression-site rows staying in fact tables matches the documented dead-evidence policy (`products.rs:270-274`), not a compile-time accept path.

## A0142

**Digest (read):** area_09 — a call-site proposition fact whose instantiation failed is still published with `instantiated` invalid (`PUB:8387-8586`).

**Cited sites (read):** `published.rs:8387-8610` in module `published::semantic::contracts`. `instantiate_call_contract_payload` (`:8387-8552`) sets `instantiated` for propositions only when `normalize_proposition_application` returns `Some` (`:8543-8551`); otherwise the payload keeps `Handle::invalid()` from `semantic_contract_payload` (`:7289-7295`). `append_call_semantic_contract_refs` (`:8554-8657`) skips publish only for `ContractBooleanExpression` with invalid `instantiated` (`:8583-8586`); proposition payloads are not filtered there and still reach `facts.append_fact` (`:8601-8608`).

**Callers (read):** `rg '\binstantiate_call_contract_payload\b' /agent/repos/omega` → 2 matches (`published.rs`, definition + one call at `:8575`). `rg '\bappend_call_semantic_contract_refs\b' /agent/repos/omega` → 3 matches (`published.rs`: calls at `:7974`, `:8011`, definition `:8554`). Invoked from `build_semantic_facts` contract-call loop (`:7971-8019`).

**Downstream label behavior (read):** `FactPlan::proposition_fact_label` in `omega-rust/psi/compilation/representations/checked-trees/facts/src/lib.rs:1388-1408` uses the instantiated label when valid; when `instantiated` is invalid it falls back to `program.normalize_proposition_application(application, None)` on the declaration fact (`:1404-1407`), not call-site substitution labels.

**Input that reaches the branch (read):** A checked `ContractCallFact` with a proposition ensures/requires contract where `instantiate_call_contract_payload` cannot build call-site labels (early `return`s in `:8412-8450`, `:8516-8520`, or failed `normalize_proposition_application` at `:8543-8551`).

**Verdict:** REACHABLE — the semantic fact row is still published at the call point with invalid call-site instantiation, while proof consumers can still derive a proposition label from the generic declaration via `proposition_fact_label`, without the failed call-site substitution.

## A0143

**Digest (read):** area_09 — a parameter constraint whose name is neither a domain nor a carry permission adds no obligation (`PUB:6796-6806`).

**Cited site (read):** `published.rs:6778-6808` in `published::semantic::calls::parameter_domains::call_parameter_domains` (`:6740-6810`). For each non-declared domain constraint, invalid `domain_symbol` enters the `else` branch (`:6796`); `CarryPermission::from_name` failure hits `continue` at `:6804` without pushing a requirement.

**Callers (read):** `rg '\bcall_parameter_domains\b' /agent/repos/omega` → 4 matches in `published.rs` (`:6740` def, `:969`, `:6812` re-export, `:8721` in `append_parameter_domain_requirements`).

**Upstream (inferred, not read):** Constraints originate from `parameter_domain_constraints` in validation (`proof_contracts_core.rs:5820` per grep); whether every surviving constraint name must resolve to a domain symbol or carry permission was not read.

**Input that reaches the branch (read):** A call site where generic binding yields a domain constraint row with invalid `domain_symbol` and a `name` string that is not a carry-permission name.

**Verdict:** UNDECIDED — the branch silently omits an obligation; deciding REACHABLE requires reading whether validation already rejects unknown constraint names before stage-05 fact publication.

## A0144

**Digest (read):** area_09 — invalid member symbols are skipped, giving a shorter place (`PUB:10387-10391`).

**Cited site (read):** `published.rs:10382-10392` in `published::semantic::places::append_place_from_name_path`. The loop at `:10387-10390` pushes `PlaceSegment::Field` only when `symbol.is_valid()`.

**Callers (read):** `rg '\bappend_place_from_name_path\b' /agent/repos/omega` → 3 matches in `published.rs` (`:10382` def, `:10438` in `receiver_place_for_call`, `:10653` re-export).

**Input that reaches the branch (read):** A `NamePath` whose tail member symbols include invalid handles while the head symbol is valid.

**Verdict:** UNDECIDED — published contract/receiver places may omit invalid segments; whether invalid member symbols can appear on paths used for obligations after earlier name-resolution checking was not traced.

## A0145

**Digest (read):** area_09 — an unknown owner is published at global scope (`PUB:9732`, also `9780`, `9822` in digest).

**Cited sites (read):** `published.rs:9702-9733` `points::proof_obligation_point` — `ProofObligationOwner::Unknown` maps to `ProgramPoint::Global` at `:9732`. `contract_fact_point` (`:9736-9781`) maps `FactOwner::Unknown` to `ProgramPoint::Global` at `:9780`. `contract_fact_origin` (`:9784-9823`) maps `FactOwner::Unknown` to `FactOrigin::Unknown` at `:9822`.

**Callers (read):** `rg '\bproof_obligation_point\b' /agent/repos/omega` → 3 matches in `published.rs` (`:9702`, `:9835` import, `:9943` use in `append_proof_obligation_semantic_facts`). `rg '\bcontract_fact_point\b' /agent/repos/omega` → 3 matches (`:9736`, `:7879`, `:7890`). `rg '\bcontract_fact_origin\b' /agent/repos/omega` → 4 matches (`:9784`, `:7879`, plus uses in semantic fact build).

**Publication path (read):** `append_proof_obligation_semantic_facts` at `:9941-9953` calls `proof_obligation_point` then `facts.append_fact_context` for every proof obligation row.

**Input that reaches the branch (read):** Any `ProofObligationFact` or contract fact whose checked owner enum is still `Unknown` when semantic facts are built (after `build_proof_facts` / `bind_outcome_specific_arm_facts` in `published_facts.rs:11014-11025`).

**Verdict:** UNDECIDED — global program points are assigned for unknown owners, but whether the checking pipeline can leave unknown owners on obligations/contracts for programs that still reach `CheckedTrees` was not read (`ProofObligationOwner::Unknown` also appears in `obligations.rs:2084`).

## A0146

**Digest (read):** area_09 — an outcome arm with no selector that cannot be classified is skipped without error (`PUB:4465-4521`).

**Cited site (read):** `published.rs:4437-4535` in `outcome_arms::bind_outcome_specific_arm_facts`. Each failure path (`:4464-4470`, `:4472-4480`, `:4492-4498`, `:4500-4509`, `:4511-4520`) pushes diagnostics only inside `if !selectors.is_empty()`; otherwise `continue`. At `:4533-4534`, `matching_rows.is_empty() && selectors.is_empty()` also `continue`s without error.

**Callers (read):** `rg '\bbind_outcome_specific_arm_facts\b' /agent/repos/omega` → 4 matches (`published.rs:4437`, `published.rs:6309` re-export, `published_facts.rs:11022`, `checks.rs:10272` comment reference).

**Upstream (read):** `selectors` from `program.statement_table.outcome_proof_selectors(transition.proof_selectors)` at `:4453-4455`.

**Input that reaches the branch (read):** A transition arm with empty outcome proof selectors and a guard that is not `When`, or that fails `exact_outcome_case_test`, saved-result-call lookup, target resolution, or case resolution.

**Verdict:** CLOSED — for arms with no selectors, skipped arms do not surface errors by design; errors are gated on non-empty selectors, so this is not an undocumented accept path for selector-bearing arms.

## A0147

**Digest (read):** area_09 — a machine with no entry state is skipped in definite-assignment validation (`PUB:2483-2485`).

**Cited site (read):** `published.rs:2440-2485` in `evidence_forwarding::validate_evidence_forwarding_definite_assignment`. After building forwarding work for a machine (`:2449-2481`), `states.first()` at `:2483-2485` yields `continue` when the machine has zero states.

**Callers (read):** `rg '\bvalidate_evidence_forwarding_definite_assignment\b' /agent/repos/omega` → 2 matches (`published.rs:2440`, call from `bind_evidence_forwarding_facts` at `:2206`).

**Preceding guard (read):** The loop at `:2449-2481` only proceeds when the machine has unconditional ensures outputs and/or outcome-specific guarantee rows; machines with neither are skipped earlier at `:2479-2480`.

**Input that reaches the branch (read):** A machine symbol that still has evidence-forwarding or outcome-specific rows in `ProofFacts` but `program.machine_states(machine)` is empty.

**Verdict:** UNDECIDED — definite-assignment validation is bypassed for zero-state machines; whether checked programs can have forwarding/guarantee facts without any machine states was not read.

## A0148

**Digest (read):** area_09 — a call with no retained signature owns every non-signature contract (`PUB:790-792`). Reader class not stated.

**Cited site (read):** `published.rs:784-837` in `published::proof::contracts::call_signatures::owns_contract`. When `!call.signature.is_valid()` (`:790-791`), the function returns `!matches!(contract.owner, ContractProofFactOwner::CallSignature { .. })`, so machine/state/operator contracts return `true` and signature-scoped contracts return `false`.

**Callers (read):** `rg '\bowns_contract\b' /agent/repos/omega` → 5 matches (`published.rs:784`, call at `:8566` inside `append_call_semantic_contract_refs`; `published_facts.rs` ×2; `checks_content.rs:2351`).

**Signature acquisition (read):** In proof call recording (`published.rs:936-951`), missing `call_site` yields `signature: Default::default()` (invalid handle) at `:949-950`.

**Downstream use (read):** `append_call_semantic_contract_refs` filters contracts with `owns_contract` before publishing (`:8566-8571`). `checks_content.rs:2351-2358` uses the same predicate when matching exact calls to contracts.

**Input that reaches the branch (read):** A `ContractCallFact` whose `signature` handle is invalid while semantic publishing or content checks iterate contracts not owned by `CallSignature`.

**Verdict:** UNDECIDED — the early return deliberately admits non-signature contracts without a retained logical view; whether that can attach callee contracts to calls that should have been rejected or signature-filtered was not traced through the full `retain` / admission path.

## What I did not read

- `git show 457adf4a11f:` line diff for each cite (current checkout lines matched digest descriptions on read).
- `validation::parameter_domain_constraints` implementation and error behavior for unknown constraint names.
- Full proof obligation owner assignment in `proof/src/obligations.rs` and whether `Unknown` survives checking.
- Entire `checks_contracts` / `checks_contracts_prover` paths for invalid `ContractPropositionApplication` instantiation beyond `proposition_fact_label`.
- Whether machines with zero states can pass earlier validation while retaining machine-level ensures or outcome guarantees.

SLICE COMPLETE

