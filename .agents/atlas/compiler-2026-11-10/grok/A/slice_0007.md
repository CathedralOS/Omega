# slice_0007

Digest preamble: **area_08** (Psi stage 05 borrow ledger and checks; contract checks in `checks_contracts*.rs`). Basenames under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` unless noted. Commit cited in digest: `457adf4a11f`; lines read at current checkout and match by symbol unless noted.

| id | file:line | verdict | one-line reason |
|----|-----------|---------|-----------------|
| A0049 | checks_contracts_calls.rs:3407 | UNDECIDED | Unproved `requires` can `continue` into deferred obligations without a diagnostic; whether every such obligation is discharged before acceptance was not fully traced. |
| A0050 | checks_contracts_calls.rs:4542 | REACHABLE | Caller lends a state **parameter** storage (not a `LocalData` local) through an exclusive reference; domain restoration lookup skips it and emits nothing. |
| A0051 | checks_contracts_calls.rs:4175 | REACHABLE | `selected_evidence_proves_requirement` treats a witness as proven if **any** `outcome_specific_arms` row selects the binding source, not only the call’s arm. |
| A0052 | checks_contracts_exits.rs:9696 | UNDECIDED | Ensures facts are skipped when entailment or machine-level evidence-term flags fire; did not verify those skips always imply proof elsewhere. |
| A0053 | checks_contracts_exits.rs:9655 | REACHABLE | Exit whose return expression `static_type_context_is_unreachable` marks unreachable skips the entire ensures loop with no alternate check in this function. |
| A0054 | checks_contracts_exits.rs:4037 | REACHABLE | Non-empty result field-domain paths with an invalid return expression leave before any domain proof runs. |
| A0055 | checks_contracts_exits.rs:2079 | REACHABLE | A header-entering transition whose `find_call_site` fails is dropped from `arrivals`, so cyclic-header admission need not account for that edge. |
| A0056 | checks_contracts_writes.rs:1590 | UNDECIDED | Early return on failed `machine_state_by_symbols` and formation-membership short-circuit at :205-214; did not show reachable misuse or a single closing alternate. |

## A0049

**Cited:** `checks_contracts_calls.rs:3407-3474` (stated `checks_contracts.rs:102-106`, read).

**Enclosing function (read):** `check_call_requires` (`checks_contracts_calls.rs:3188-3527`). For each requires fact, `satisfied` is computed (`:3246-3387`); when `!satisfied` (`:3406`), the branch at `:3407-3474` may `recipe_obligations.extend` and `continue`, or push `constant_obligations` via `ConstantPredicateObligation::retain` (`:3447-3473`) and `continue`, avoiding the error diagnostic at `:3507-3523`.

**Callers:** `rg '\bcheck_call_requires\b'` over `omega-rust` → count 3 (1 definition `checks_contracts_calls.rs:3188`, 1 production call `checks_contracts.rs:11722`, 1 `use` `checks_contracts.rs:11584`).

**Upward trace (read):** `checks_contracts.rs:11722-11743` calls `check_call_requires` per flow call; if diagnostics length unchanged, pushes `DecidedCallRequires`. `checks.rs:12080-12099` on `Ok(decided)` stores `decided.constant_obligations` into `facts.constant_predicate_obligations`. `checks_contracts.rs:11851-11864` returns `Ok(ContractDecisions { ... })` when `diagnostics.is_empty()`.

**Partial downstream (read, not complete):** `published_facts.rs:7175-7228` records obligation owners; `build-time-evaluation/src/machine_execution.rs:952-1025` errors on pending predicate/range obligations in a demanded constant-invocation closure; `06_checked-trees-to-lowered-psi/src/attached_unit.rs:7496-7524` lowers pending predicate obligations.

**Input that reaches the branch:** A call whose boolean `requires` is not proved by the `proves_case` / `captured_boolean` routes, but matches `has_builtin_operators`, captured integer actuals, and `argument_intervals::recipe_requirements` or `ConstantPredicateObligation::retain` (needs closure / non-literal structure per `contract_and_obligation_facts.rs:49-125`, read).

**Verdict UNDECIDED:** Deferred obligation without stage-05 diagnostic is read at `:3407-3474`. Whether a program that should be rejected is still accepted depends on tracing every consumer of `facts.constant_predicate_obligations` beyond the partial reads above (digest: “downstream discharge not traced”).

## A0050

**Cited:** `checks_contracts_calls.rs:4542-4560`, `:4617-4621`, `:4638-4652` (inferred in digest).

**Enclosing function (read):** `check_lender_domain_restoration` (`checks_contracts_calls.rs:4535-4723`). Early exits at `:4542-4544`, `:4552-4559`, `:4569-4570`. For each exclusive parameter, `:4617-4621` requires `captured_values`; for each place, `:4638-4651` resolves `local_type` only from `StatementNode::LocalData` with `l.symbol == symbol`; otherwise `continue` with no diagnostic.

**Callers:** `rg '\bcheck_lender_domain_restoration\b'` over `omega-rust` → count 2 (definition `:4535`, call from `check_call_requires` `:3234`).

**Upward trace (read):** Invoked from `check_call_requires` before the requires loop; acceptance still gated by `check_flow_call_contracts` empty diagnostics (`checks_contracts.rs:11851-11864`).

**Reachable shape (plain words):** A machine state holds a mutable parameter qualified with a default domain; the caller passes that parameter to a callee through an exclusive reference; the callee mutates storage so the declared domain no longer holds at call exit; the lender-side lookup never binds the parameter to `LocalData`, so restoration membership is never required and no error is emitted here.

**Verdict REACHABLE:** Digest inference matches the read control flow at `:4638-4651`.

## A0051

**Cited:** `checks_contracts_calls.rs:4175-4178`.

**Enclosing function (read):** `selected_evidence_proves_requirement` (`checks_contracts_calls.rs:4154-4191`). Inner test at `:4175-4178`: `facts.proof.outcome_specific_arms.iter().any(|(_, arm)| arm.rows.iter().any(|row| row.selected_term == Some(binding.source)))` without scoping `arm` to the evidence call or lane.

**Callers:** `rg '\bselected_evidence_proves_requirement\b'` over `omega-rust` → count 2 (definition `:4154`, use in `check_call_requires` `:3271` as first disjunct of `satisfied`).

**Upward trace (read):** When this returns true, `proves_case` short-circuits satisfied (`:3270-3271`); unsatisfied branch with diagnostic not taken.

**Contrast (read):** Evidence binding in `checks_contracts.rs:1245-1266` resolves a term by name across `outcome_specific_arms` with diagnostics on failure; the requires prover here does not tie `binding.source` to a single arm row.

**Reachable shape (plain words):** A call carries Type-style evidence with multiple outcome arms; one arm’s selected witness term equals an evidence binding source listed on another arm; a `ContractPropositionApplication` requires fact is marked satisfied even though the witness belongs to a different outcome lane.

**Verdict REACHABLE:** Cross-arm `any` at `:4175-4178` is read; digest “cross-arm sharing open” not refuted.

## A0052

**Cited:** `checks_contracts_exits.rs:9696`, `:9713`.

**Enclosing function (read):** `check_exit_ensures` (`checks_contracts_exits.rs:9641-9919+`). Per ensures fact: `checked_entailment` at `:9690-9697` (`entailment.expressions.contains(&expression)` → `continue`); `evidence_assignment` at `:9699-9714` (machine-owned Ensures contract with `evidence_term.is_some()` → `continue`).

**Callers:** `rg '\bcheck_exit_ensures\b'` over `omega-rust` → count 5 (definition `:9641`, `use` `checks_contracts.rs:11592`, production call `:11812`, module comments `checks_contracts_exits.rs:11-14`).

**Upward trace (read):** `checks_contracts.rs:11811-11825` per exit inside `check_flow_call_contracts`; compilation accepts when diagnostics empty (`:11851-11864`).

**Input reaching skip:** Ensures boolean expression already listed in `entailment.expressions`, or a matching Ensures proof fact on the machine with `evidence_term` set (`:9702-9709`, read).

**Verdict UNDECIDED:** Skips are read at `:9696-9714`. Did not read full `entailment` construction or all writers of `evidence_term` to show skipped facts are always proved by those mechanisms (digest: “set elsewhere” not traced).

## A0053

**Cited:** `checks_contracts_exits.rs:9655-9659`.

**Enclosing function (read):** `check_exit_ensures` (`:9641+`). At entry, `:9655-9658` calls `program.static_type_context_is_unreachable` on `exit_return_expression(program, exit_flow)` and `return`s before the ensures fact loop.

**Callers:** Same as A0052.

**Upward trace (read):** No other ensures checking in this function after early return; parent only aggregates diagnostics.

**Reachable shape (plain words):** A value-returning exit still present in flow facts carries authored `ensures` clauses, but the static type context classifies its return expression as unreachable, so none of those ensures are checked in this pass.

**Verdict REACHABLE:** Early return at `:9655-9658` is unconditional for that predicate (read).

## A0054

**Cited:** `checks_contracts_exits.rs:4037`, `:4042`.

**Enclosing function (read):** `check_result_field_domains` (`checks_contracts_exits.rs:4000+`). Builds `paths` (`:4015-4035`); `:4037-4038` returns if empty; `:4041-4043` returns if `!returned.is_valid()` for `exit_return_expression`, before `:4045+` domain proofs.

**Callers:** `rg '\bcheck_result_field_domains\b'` over `omega-rust` → count 5 (definition `:4000`, re-exports/comments in `checks_contracts_exits.rs`, production call `checks_contracts.rs:11828`).

**Upward trace (read):** Called after `check_exit_ensures` for each exit (`checks_contracts.rs:11827-11836`).

**Reachable shape (plain words):** Exit has a nominal or proof-nominal return type with declared field-domain identities so `paths` is non-empty, but the flow exit’s return expression handle is invalid, so result field domains are not checked here.

**Verdict REACHABLE:** Read order `:4037-4043` skips checking when paths exist but return handle invalid.

## A0055

**Cited:** `checks_contracts_exits.rs:2079-2088`, `:2096`.

**Enclosing function (read):** `Header::arrivals` (`checks_contracts_exits.rs:2074-2146`). Loop over callers and flow calls; `:2080-2087` `find_call_site(...) else { continue }`; `:2089-2096` requires `CallSite::TransitionNamed`.

**Callers:** `Header::arrivals` invoked only via `headers.iter().map(Header::arrivals)` in `components::admitted` (`checks_contracts_exits.rs:1421-1424`, read). `rg` on `arrivals` in this file: definition `:2074`, use `:1423`.

**Upward trace (read):** `components::admitted` feeds `CyclicHeaderInvariants` (`:1817`); `cyclic_headers.proves` used from `check_exit_ensures` (`:9852`, read).

**Reachable shape (plain words):** Control-flow records a call that transitions into a cyclic header state, but `find_call_site` fails for that statement ordinal, so the transition is omitted from `arrivals` and header induction need not treat it as an arriving edge.

**Verdict REACHABLE:** Comment at `:2067-2071` matches `continue` on failed site lookup (`:2086-2087`, read).

## A0056

**Cited:** `checks_contracts_writes.rs:1590-1594`, `:205-214`.

**Enclosing function (read):** (1) `check_domain_field_writes` (`:1580+`) — `:1590-1594` `machine_state_by_symbols(...) else { return }`. (2) `ConstructionWalk::check_constructor_membership` (`:190-214`) — `:205-213` returns when `facts.qualifications.content.publishes_formation_membership(...)` is true.

**Callers:** `rg '\bcheck_domain_field_writes\b'` over `omega-rust` → count 3 (definition `:1580`, `use` `checks_contracts.rs:11596`, call `:11749`). `check_constructor_membership` called from construction walk sites in same file (`:2002`, `:2025`, `:2103`, read).

**Upward trace (read):** `check_domain_field_writes` runs per state in parallel block `checks_contracts.rs:11749-11758`; parent accepts on empty diagnostics.

**Verdict UNDECIDED:** (1) Did not show a `FlowStateFact` that reaches stage-05 contract checks with symbols failing `machine_state_by_symbols` (inferred: likely absent). (2) Formation-membership short-circuit at `:205-213` — did not trace whether publication always implies an equivalent write or predicate check elsewhere (`published.rs` / qualification pipeline not read for equivalence).

## What I did not read

- Full bodies of `argument_intervals::recipe_requirements`, `boolean_requirements::proves`, and all `proves_case` routes beyond the unsatisfied branch entry.
- Complete discharge path for `ConstantPredicateObligation` and `ConstantRecipeObligation` outside the cited sites in `machine_execution.rs`, `attached_unit.rs`, and `published_facts.rs`.
- `entailment` module: how `expressions` is populated and whether membership implies scalar ensures proof.
- All consumers of `publishes_formation_membership` and whether they duplicate `check_constructor_membership` obligations.
- Corpus or sample programs exercising state-parameter lending, cross-arm evidence, or failed `find_call_site` on header transitions.
- `git show 457adf4a11f:` diffs for line drift (symbols at cited lines matched on read).

SLICE COMPLETE
