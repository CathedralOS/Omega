slice_0069

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0529 | proof_contracts_core.rs:8971-8977 | REACHABLE | Boolean ensures labels are compared only by rendered string in `BTreeSet` known/required sets. |
| A0530 | proof_contracts_core.rs:9095 | REACHABLE | `Propagated` always yields `Some` qualification source, so callee ensures memberships are kept without CheckedBody/Admitted checks. |
| A0531 | proof_contracts_core.rs:9684-9686 | REACHABLE | After full identity fails, only nominal head symbols are compared for boundary qualification authorization. |
| A0532 | proof_contracts_core.rs:10571-10582 | REACHABLE | Unresolvable receiver projection returns without diagnostic before erased-member checks. |
| A0533 | proof_contracts_core.rs:10464-10469 | REACHABLE | Missing data definition skips erased-field omission checks; only field values are visited. |
| A0534 | proof_contracts_core.rs:10996-10999 | REACHABLE | Meaning guarantee conclusion compares result to itself; lowering still builds an implication from premise to that tautology. |
| A0535 | quotients.rs:477-479 | REACHABLE | Dependency symbol with no resolvable machine is skipped; `checked_proof_dependency` stays `Ok`. |
| A0536 | quotients.rs:3586-3588 | CLOSED | `proof_facts_match` requires equal `(rendered, trace)` pairs; render includes operators. |

### A0529

1. Read `proposition_labels` in full at `omega-rust/psi/compilation/validation/src/proof_contracts_core.rs:8956-8987` (read); boolean `ProofFact::Expression` with `include_boolean_expressions` formats via `render_proof_expression` at `8970-8977` (read).
2. Search: `rg '\bproposition_labels\(' /agent/repos/omega/omega-rust` — 18 hits (read): 8 in `proof_contracts_core.rs` (definition + 7 call sites), 3 in `evidence_and_inspection.rs`, 5 in `published.rs`, plus related `contract_proposition_labels` names.
3. Read callers: `validate_proposition_entailment` at `8506-8710` (read); labels enter `BTreeSet` known/required at `8596-8604`, `8650`, `8668-8669` (read); scheduled from `program_validation.rs:377` (read).
4. Input reaching branch: ensures/requires `ProofFact::Expression` booleans with `include_boolean_expressions == true` (e.g. `intake_call_propositions` at `8911-8917` with `true`) (read).
5. Verdict: **REACHABLE** — two distinct boolean contract expressions that `render_proof_expression` spells the same (e.g. shadowed locals mapped to identical substitution labels via `call_parameter_substitutions` at `8933-8953`) share one label; one established fact can satisfy a different required goal via `known.contains(goal)` at `8604`/`8650`.
6. Unsure: exact shadowing scenarios that survive earlier typing; did not exhaust `render_expression` name-path cases beyond `evidence_and_inspection.rs:1779-1819`.

### A0530

1. Read `CallQualificationTarget::resolve` and `fact_source` in full at `proof_contracts_core.rs:9066-9111` (read); fall-through `Self::Propagated` at `9095` (read).
2. Search: `rg 'call_qualification_source|CallQualificationTarget::resolve' omega-rust/psi` — 6 hits (read).
3. Read callers: `call_qualification_source` at `9047-9052` (read); `published_facts.rs:9570-9592` and `9664-9680` map `Propagated` to `propagated_evidence` (read); `flow_calls.rs:926-934` keeps ensures memberships when `fact_source` is `Some` (read).
4. Input: call `target_symbol` that is not a checked/boundary machine, not a closed boundary requirement, and not a boundary trait — none of the branches at `9068-9094` match (read).
5. Verdict: **REACHABLE** — such a call target still gets `Some(CallQualificationSource::Propagated)` at `9109`, so qualification membership facts on the callee contract are not dropped by `visit_qualified_ensures` filtering at `926-932` (`flow_calls.rs`).
6. Unsure: whether any ill-typed call targets reach this stage; did not read full call typing in pipeline 04.

### A0531

1. Read `return_type_matches_domain_target` and caller `boundary_qualification_authorization` at `proof_contracts_core.rs:9606-9687` (read).
2. Search: `rg '\breturn_type_matches_domain_target\b' omega-rust` — 2 hits, both in `proof_contracts_core.rs` (read).
3. Read caller only (`boundary_qualification_authorization`); no further callers beyond `CallQualificationContract::fact_source` path at `9235-9244` (read, not fully traced through entailment).
4. Input: boundary-trait ensures `ProofFact::Membership` after route/argument checks pass; paired return type and domain `target_type` whose full `normalized_type_identity` differs but `closed_nominal_symbol` heads match (read `9675-9686`).
5. Verdict: **REACHABLE** — same nominal head with different generic arguments fails the strict check then succeeds on head-only equality, yielding `BoundaryQualificationAuthorization` at `9664-9667`.
6. Unsure: whether domain establishment routes always pair incompatible generics in practice.

### A0532

1. Read `validate_statement_call_receiver` in full at `proof_contracts_core.rs:10537-10595` (read); early `return` when `owner` is `None` at `10571-10572` and when `exact_data_member_field` is `None` at `10574-10581` (read).
2. Search: `rg '\bvalidate_statement_call_receiver\b' omega-rust` — 2 hits: definition and call at `9796-9802` (read).
3. Read caller `relevance` statement walk at `9784-9803` (read); only `Context::Runtime` calls (read); wired from `program_validation.rs:485` (read).
4. Input: runtime statement call whose receiver name-path projects through a segment whose type does not yield a `DataDefinition` (`owner` becomes `None`) or member lookup fails (read).
5. Verdict: **REACHABLE** — path can reach an erased field in the source name-path, but the loop exits with no diagnostic instead of `report_runtime_erased_field`-style errors at `10583-10588`.
6. Unsure: how often receiver typing leaves `owner` unresolved at this stage; did not read `receiver_root_type` / place validation closures.

### A0533

1. Read `validate_struct_literal` at `proof_contracts_core.rs:10441-10535` (read); `data_definition_by_symbol` miss branch at `10464-10468` (read).
2. Search: `rg '\bvalidate_struct_literal\b' omega-rust/psi/compilation/validation` — prologue `validate_struct_literal_fields` in `literals_and_struct_literals.rs:5388` vs relevance `validate_struct_literal` at `10380` (read).
3. Read relevance caller `validate_expression` struct-literal arm at `10380` (read); prologue runs at `program_validation.rs:563` before relevance at `485` (read).
4. Input: struct literal whose `type_symbol` has no entry from `data_definition_by_symbol` (read).
5. Verdict: **REACHABLE** — omitted erased declared fields are not diagnosed on that path; only child expressions are visited under the caller’s context (read).
6. Unsure: whether typed programs can retain such literals past earlier gates; did not prove impossibility from pipeline 04 typing.

### A0534

1. Read `meaning_consequence` at `proof_contracts_core.rs:10870-11000` (read); tautological `compare(Equal, result.clone(), result)` at `10996-10998` (read).
2. Search: `rg '\bderive_scalar_boundary_guarantees\b' omega-rust` — 3 hits: definition, `lib.rs` re-export, `execution_terminal_unit_control.rs:436` (read).
3. Read consumption: `derive_scalar_boundary_guarantees` builds `scalar_ensures` (read); `attached_unit.rs:520-540` lowers `Implication` premises and conclusions to propositions (read); `scalar_lowering.rs:1787-1790` excludes `SourceCheckedGuarantee` from runtime requirement projection (read, partial).
4. Input: machine with closed float semantic ensures equality that passes `source_equation_matches` at `10907` (read).
5. Verdict: **REACHABLE** — emitted guarantee’s conclusion does not encode the authored equality, only finiteness/nonzero-divisor premises in the implication premise; program acceptance keeps that weakened checked artifact.
6. Unsure: full downstream use of `scalar_ensures` beyond boundary lowering snippet; did not read all `CheckedBoundaryScalarGuarantee::Implication` consumers.

### A0535

1. Read `checked_proof_dependency` at `quotients.rs:463-485` (read); `continue` on unresolved machine at `477-478` (read).
2. Search: `rg '\bchecked_proof_dependency\b' omega-rust` — 3 hits: definition, quotient selection error at `203`, `require_checked_proof_closure` at `7285` (read).
3. Read caller `validate_equivalence_selection` diagnostic at `203-210` only on `Err` from non-checked-body machine name (read `468-469`).
4. Input: `machine_call_dependency_symbols` dependency handle with no `machine_by_own_or_state_symbol` result (read `475-478`).
5. Verdict: **REACHABLE** — that dependency is treated as satisfied; quotient/conformance proof-closure checks do not flag an unresolved callee as admitted/boundary.
6. Unsure: complete enumeration of symbols in `machine_call_dependency_symbols` (`call_cycles.rs:3458-3472`, read header only).

### A0536

1. Read `expression_symbol_trace` Binary/Unary arms at `quotients.rs:3586-3588` and `3616` (read); read `proof_expression_identity` at `3462-3492` and `proof_facts_match` at `3351-3362` (read).
2. Search: `rg '\bproof_expression_identity\b' omega-rust` — 3 hits, all in `quotients.rs` (read).
3. Read render side: `evidence_and_inspection.rs:1739-1744` (Binary with operator) and `1837-1838` (Unary with operator) (read).
4. Input: paired proof facts compared via `proof_facts_match` (quotient equivalence / law matching) (read).
5. Verdict: **CLOSED** — matching requires tuple equality `(rendered, trace)` at `3362` (read); `normalized_expression_identity` gate at `3467` (read); distinct Binary/Unary operators change `rendered` even when `trace` omits operators, so false “same fact” acceptance from trace alone does not occur on this path.
6. Unsure: none material for verdict given paired-string gate (read).

### What I did not read

- Full `render_expression` beyond unary/binary/name-path slices in `evidence_and_inspection.rs`.
- Entire `contract_entailment.rs` qualification propagation beyond `59706-59730`.
- Pipeline 04 typing rules for struct literals and call targets.
- All consumers of `scalar_ensures` after `execution_terminal_unit_control.rs:436`.
- Body of `collect_expression_dependency_symbols` in `call_cycles.rs` beyond `collect_statement_dependency_symbols`.

SLICE COMPLETE
