slice_0064

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0489 | machine_calls_core.rs:3158-3167 | REACHABLE | Template commitment `Diagnostic` is turned into `None`, so AdmissionClaim rows can ship with missing template identity |
| A0490 | machine_calls_core.rs:4767-4784 | UNDECIDED | Memo key ignores `operational`/`service_reaches`; could not prove two different plan tuples hit the same key in one check |
| A0491 | machine_calls_core.rs:5688-5712 | REACHABLE | `validate_asm_discharge` only sees asm on bare `Call`/`Assignment` statements, not asm inside return/let expression roots |
| A0492 | machine_calls_core.rs:6359-6369 | REACHABLE | `expression_contains_call` does not descend through Indexed/Binary/Unary/Match, so fact projection gate can skip validation entirely |
| A0493 | machine_calls_core.rs:6611-6613 | REACHABLE | Non-empty statement `receiver` skips runtime proof-citation checks even when callee is a proof machine |
| A0494 | machine_calls_core.rs:7083-7105 | REACHABLE | Missing trait definition or zero name matches drops refinement silently with no diagnostic |
| A0495 | machine_calls_core.rs:8911-8913 | REACHABLE | Spanless synthesized casts skip float-conversion binding and stay raw numeric casts |
| A0496 | machine_parameters_and_bindings.rs:4770-4785 | NOT MY CASE | Early return targets Build-operation spellings that never use static machine-parameter `form_call_selection` |

### A0489

1. Read `canonical_template_contract_bytes` at `machine_calls_core.rs:2939-3133` (read): returns `Result<Vec<u8>, Diagnostic>` on missing target-family summaries (`3056-3063`) and other encode failures (`3019-3021`, `3128-3132`).
2. Read `canonical_template_bytes_for_symbol` at `machine_calls_core.rs:3158-3167` (read): calls `canonical_template_contract_bytes(...).ok()`, discarding the `Diagnostic`.
3. Read callers `generic_machine_template_report_fingerprint` / `generic_machine_template_commitment` at `3150-3186` (read); search `\bcanonical_template_bytes_for_symbol\b` in `omega-rust/` → **3 hits**, all in `machine_calls_core.rs`.
4. Read consumer `checking.rs:1186-1209` (read): `identity` is `fingerprint.zip(commitment).map(...)`, so a failed canonicalization becomes `identity: None` while the machine row remains.
5. Read `prepare_specialization_contract_identities` at `3298-3342` (read): same failure surfaces as `Err` on specialization/capture paths — not a universal gate for every AdmissionClaim template.
6. Read `providers_and_trust.rs:711-718` and `admission_settlement.rs:74-94` (read): `Ok(None)` template is admitted on the contract-only trust path, not `Missing`.

**Input to accepting branch:** A generic AdmissionClaim machine whose template bytes cannot be formed (read: e.g. target-family without service-reach/operational summary per `3056-3063`), after earlier validation did not hit specialization capture.

**Verdict:** REACHABLE — smallest rejecting case (plain words): a target-family AdmissionClaim generic template whose service-reach or operational summary is incomplete should fail template fingerprinting, but the program can still be checked because the error is swallowed into `identity: None` and trust settlement falls back without that template.

**Unsure:** Whether every incomplete-summary template always has a specialization/capture that would fail earlier.

### A0490

1. Read `checked_value_call_candidate` at `machine_calls_core.rs:4760-4787` (read): memo key `(machine.symbol, state.symbol)`; stored value is `Result<(), &'static str>` from `unchecked_value_call_candidate` with caller-supplied plans.
2. Read memo type at `frozen_program_and_queries.rs:1367-1371` (read).
3. Search `\bchecked_value_call_candidate\(` in `omega-rust/` → **12 call sites** (read list): `machine_calls_core.rs` (4604, 6410, 6421), `contract_entailment.rs` (44206, 56437, 70780, 70808, 78402), `checks_contracts.rs` (2013).
4. Read `operand_call_plans` / `compute_call_plans` at `calls.rs:14259-14277` (read): frame and global paths both call `compute_call_plans` (same infer).
5. Read `checking.rs:1098` (read): `with_prepared_effect_plans` is set after initial `judge_contract_entailment`, not before full program validation.
6. Did not re-read every entailment caller body beyond the sampled lines above.

**Input to accepting branch:** Second memo hit for same `(machine, state)` where the first caller supplied plans that yielded `Ok(())` but a later caller would pass a different `(operational, service_reaches)` pair (inferred vs prepared).

**Verdict:** UNDECIDED — key omission is real (read), but I could not trace two plan tuples that differ on one frozen program check and produce a false `Ok(())` without building/running.

**Unsure:** Whether `prepared_effect_plans` ever diverges from `infer_operational_may`/`infer_service_reaches` within one memo lifetime.

### A0491

1. Read `validate_asm_discharge` at `5590-5624` (read): walks statement lists only.
2. Read `statement_asm_intrinsic` at `5680-5714` (read): handles `StatementNode::Call` and `Assignment` whose value is a direct `ExpressionNode::Call`; other statement shapes return `None`.
3. Read `validate_asm_intrinsic_declarations` at `5626-5677` (read): same statement walk.
4. Search `\bvalidate_asm_discharge\b` in `omega-rust/` → **4 hits** (`machine_calls_core.rs`, `lib.rs`, `preparation.rs:1961`).
5. Read `statement_value_expression_roots` at `calls.rs:16045-16049` (read): preparation helpers walk return/let expression roots, but asm discharge does not use them (contrast `bind_float_conversion_casts` at `8824-8836`).

**Input to accepting branch:** Hosted build with PortIo/Idt asm inside a state return expression or let initializer (not top-level call/assign statement).

**Verdict:** REACHABLE — smallest rejecting case: freestanding build without privileged port-io/IDT admission, with asm intrinsic only in a return or let initializer, passes `validate_asm_discharge` because `statement_asm_intrinsic` never inspects those roots.

**Unsure:** Whether another validator always scans the same asm mnemonics in expression roots during 04/05.

### A0492

1. Read `expression_contains_call_projection` at `6339-6356` (read): `SubtreeVisit::Found` only when `expression_contains_call` on member receiver; otherwise `Prune` on `Member`/`Match`.
2. Read `expression_contains_call` at `6359-6369` (read): recurses `Call`, `Member`, `Borrow`, `Cast` only — not `Indexed`, `Binary`, `Unary`, or `Match`.
3. Read `validate_fact_call_projections` at `5916-5921` (read): returns empty admitted list when `has_projection` is false.
4. Search `\bexpression_contains_call_projection\b` in `omega-rust/` → **2 hits** (`machine_calls_core.rs:5919`, definition).
5. Read caller `program_validation.rs:4883-4887` (read): prologue records projections only when validator runs.

**Input to accepting branch:** Proof fact expression `member(indexed_expr_with_call)` where the call sits under Indexed/Binary/Unary/Match inside the member receiver.

**Verdict:** REACHABLE — smallest rejecting case: fact-position projection that should be rejected as non-direct, but the pre-scan never sees the nested call and validation is skipped entirely.

**Unsure:** Whether substituted proposition path always covers the same shape (`5967-6027`).

### A0493

1. Read `validate_runtime_proof_citations` at `6592-6678` (read).
2. Read guard at `6611-6613` (read): `if !call.receiver.is_empty() { continue; }` on `TableCall.receiver: HandleSpan<Identifier>` (`calls_and_control_flow.rs:3649`).
3. Read `is_fact_only_citation_call` at `6692-6746` (read): namespace/qualified paths use `receiver_symbol` when `receiver` is empty — never reached when guard continues.
4. Search `\bvalidate_runtime_proof_citations\b` in `omega-rust/` → **3 hits** (`machine_calls_core.rs`, `lib.rs`, `checking.rs:1158`).
5. Did not read all statement-lowering paths that populate `receiver` vs `receiver_symbol`.

**Input to accepting branch:** Runtime statement call to a proof machine with a non-empty qualified `receiver` span and otherwise invalid citation effects.

**Verdict:** REACHABLE — smallest rejecting case: executable machine statement calling a proof theorem with a namespace/qualified receiver syntax that fills `receiver`, bypassing citation diagnostics.

**Unsure:** Exact surface syntax that sets non-empty `receiver` on statement calls vs expression-only qualification.

### A0494

1. Read `validate_conformance_refinement` at `7077-7166` (read).
2. Read `7083-7086` (read): `trait_definition_by_symbol` `None` → `continue` with no diagnostic (unlike `traits.rs:758-773`, which errors unknown traits in a different validator).
3. Read name filter `7089-7105` (read): empty `matching` skips the refinement loop silently.
4. Search `\bvalidate_conformance_refinement\b` in `omega-rust/` → **2 hits** (definition + `6989`).
5. Read caller `validate_invocation_contracts` at `6954-6990` (read): runs during program validation prologue.

**Input to accepting branch:** Machine with trait conformance whose `conformance.symbol` does not resolve to a trait definition in this pass, or whose requirement name matching yields zero signatures while body invocations exceed declared ceilings.

**Verdict:** REACHABLE — smallest rejecting case: machine whose synchronous invokes exceed a conformance requirement but name matching never pairs the requirement, so no refinement error is emitted.

**Unsure:** Whether stale `conformance.symbol` can reach this function after `validate_machine_trait_conformances` (`program_validation.rs:4872`).

### A0495

1. Read `selected_conversion` at `8878-8958` (read).
2. Read span gate `8910-8913` (read): `source_span.start >= source_span.end` → `None`.
3. Read `collect_expression_bindings` at `8861-8871` (read): only pushes bindings when `selected_conversion` returns `Some`.
4. Read `bind_float_conversion_casts` at `8737-8813` (read): preparation at `preparation.rs:2059-2060`; unbound casts are not rewritten to operator calls.
5. Search `\bselected_conversion\b` in `omega-rust/psi/compilation/validation` → **2 hits** (definition + use).

**Input to accepting branch:** Integer/float conversion `Cast` node with valid types but synthesized/empty source span (no real source extent).

**Verdict:** REACHABLE — smallest rejecting case: compiler-generated numeric cast without span stays a raw cast because conversion binding is skipped.

**Unsure:** Whether later exact-arithmetic or float contract validators always reject the unbound cast.

### A0496

1. Read `form_call_selection` at `4716-4785` (read): early `return` when `!target_symbol.is_valid()` and `BuildOperation::from_call_target(target_name)` matches listed build operations.
2. Read post-guard resolution path `4786-4840` (read): would emit “generic callee did not resolve” only when this guard does not fire.
3. Read `classify_expression_call_intrinsic` / build declarations at `mcc:2604-2634` and `calls.rs:6420+` (read): Build intrinsics classified by target spelling and argument shape, not nominal machine static parameters.
4. Search `\bform_call_selection\b` in `omega-rust/` → **5 hits** (all `machine_parameters_and_bindings.rs`).
5. Did not read entire Build operand validator in `calls.rs`.

**Input to accepting branch:** Call site whose `target_name` is a Build operation string, `target_symbol` invalid, processed through `form_call_selection`.

**Verdict:** NOT MY CASE — absence here means Build operations are validated on the Build/declaration-selection route (`mcc:929-1031`, `calls.rs:6420+`), not through static machine-parameter formation; skipping nominal checks is intentional for that call kind.

**Unsure:** Whether invalid `target_symbol` on a non-Build call can share the same spelling list without hitting other errors.

### What I did not read

- Full bodies of all `contract_entailment.rs` `checked_value_call_candidate` callers beyond sampled lines.
- `validate_machine_trait_conformances` beyond the unknown-trait branch (`traits.rs:731+` partial).
- Complete Build call validation in `calls.rs` below `6420`.
- Tests or corpus fixtures for any entry.
- Post-preparation float cast rejection paths outside `machine_calls_core.rs` / `arithmetic_domains.rs` snippets.

SLICE COMPLETE
