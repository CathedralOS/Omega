# slice_0064

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0489 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:3158-3167 | REACHABLE | `canonical_template_contract_bytes` errors become `None` via `.ok()` and checking continues without that diagnostic. |
| A0490 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:4767-4784 | REACHABLE | Memo key omits `operational`/`service_reaches` while later `PreparedEffectPlans` can differ from prologue inference. |
| A0491 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:5688-5712 | REACHABLE | `validate_asm_discharge` only sees statement/assignment call shapes, not asm in a local initializer or return value. |
| A0492 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:6359-6369 | REACHABLE | `expression_contains_call` ignores calls under Indexed/Binary/Unary/Match, so fact projection gates can miss them. |
| A0493 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:6611-6613 | REACHABLE | Statement calls with a non-empty receiver skip `validate_runtime_proof_citations` entirely. |
| A0494 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:7083-7105 | REACHABLE | Empty `matching` skips refinement diagnostics when the conformance requirement name does not match any trait signature name. |
| A0495 | omega-rust/psi/compilation/validation/src/machine_calls_core.rs:8911-8913 | REACHABLE | Spanless synthesized casts never get a conversion binding in `bind_float_conversion_casts`. |
| A0496 | omega-rust/psi/compilation/validation/src/machine_parameters_and_bindings.rs:4770-4785 | REACHABLE | Invalid symbol plus a recognized Build-operation name returns from `form_call_selection` without static-argument checks. |

## A0489

**read** `machine_calls_core.rs:3158-3167` in `canonical_template_bytes_for_symbol` (whole function `3158-3167`, parent `canonical_template_contract_bytes` `2939-3134` read through `?` returns at `3056-3063`, `3117`, `3131`).

**read** `canonical_template_contract_bytes` returns `Result<Vec<u8>, Diagnostic>`; failures include missing target-family operational/reach summaries (`3056-3063`, `inferred`).

**read** `.ok()` at `3166` drops the `Diagnostic` and yields `None`, which propagates from `generic_machine_template_report_fingerprint` (`3150-3156`) and `generic_machine_template_commitment` (`3180-3186`).

**Caller search** `\bcanonical_template_bytes_for_symbol\b` in `omega-rust`: 2 matches (both `machine_calls_core.rs`, definitions/callers at `3154`, `3184`). `\bgeneric_machine_template_report_fingerprint\b`: 2 matches (`machine_calls_core.rs:3150`, `pipeline/05_typed-trees-to-checked-trees/src/checking.rs:1196`).

**read** `checking.rs:1186-1210`: for each `AdmissionClaim` machine, `.zip` of fingerprint and commitment; if either `canonical_template_bytes_for_symbol` returns `None`, the row’s `identity` is `None` with no diagnostic from the swallowed error.

**read** `representations/checked-trees/src/providers_and_trust.rs:704-719`: missing `identity` becomes `TrustDeclarationCardinality::Missing` at trust packaging (read), not at the swallow site.

**Input that reaches the branch:** a generic `AdmissionClaim` machine whose template-byte computation would return `Err(Diagnostic)` (for example a target-family template missing published reach/operational summary per `3056-3063`).

**Verdict REACHABLE:** a program that should be rejected for template canonicalization instead passes typed-to-checked checking with `identity: None` and no error from `3166`; the original failure text never surfaces on this path (read).

## A0490

**read** `machine_calls_core.rs:4760-4787` (`checked_value_call_candidate` in full); memo insert/get at `4767-4783` keys only `(machine.symbol, state.symbol)` (read).

**read** `unchecked_value_call_candidate` (`4789+`) takes `operational` and `service_reaches` and decides closed-value candidacy (read call at `4774-4780`).

**Caller search** `\bchecked_value_call_candidate\b` in `omega-rust`: 11 matches across 4 files (1 definition `machine_calls_core.rs:4760`, 1 re-export `validation/src/lib.rs:1729`, 5 in `contract_entailment.rs`, 1 `checks_contracts.rs:2013`, plus internal `machine_calls_core.rs` at `4604`, `6410`, `6421` — read).

**read** `program_validation.rs:230-232`: prologue `conformance_operational` is `infer_operational_may(program)` (read).

**read** `published_facts.rs:11209-11224`: `prepared_effects` may replace operational with `infer_operational_may_for_dependencies` when dependency edges are non-empty (read).

**read** `checking.rs:1098`: `with_prepared_effect_plans` attaches those plans after the main validation build (read).

**read** `machine_calls_core.rs:6409-6427`: later proof citation closure uses `prepared_effect_plans()` when present, else fresh inference (read).

**Input that reaches the branch:** same `(machine, state)` consulted first during prologue/specification validation with the initial operational plan, then again after `PreparedEffectPlans` differ (read dependency refinement path).

**Verdict REACHABLE:** a cached `Ok` from the first plan can be returned for a second call that would `Err` under the refined plan; the memo never invalidates on plan identity (read; `frozen_program_and_queries.rs:1442-1452` `matches_effect_plans` exists but is not used by this memo — not read in full, inferred from absence in `4767-4783`).

## A0491

**read** `machine_calls_core.rs:5590-5624` (`validate_asm_discharge` in full) and `5680-5714` (`statement_asm_intrinsic` in full).

**read** `statement_asm_intrinsic` matches only `StatementNode::Call` or `Assignment` whose value is directly `ExpressionNode::Call` (`5688-5697`); other statement shapes return `None` (read).

**read** `5594-5596`: `AsmAuthorityAdmission::MACHINE_OWNER` returns `Ok(())` immediately without scanning statements (read).

**Caller search** `\bvalidate_asm_discharge\b` in `omega-rust`: 4 matches (`machine_calls_core.rs` define/re-export, `lib.rs`, `preparation.rs:1960-1961` — read).

**read** `preparation.rs:1960-1961`: hosted-build authority runs this before typed-tree checking (read).

**read** `program_validation.rs:1572`: `validate_asm_value_destination` only handles assignment values that are a direct call node (`calls.rs:4374-4376` read); it does not replace discharge for local initializers (read).

**Input that reaches the branch:** non-`MACHINE_OWNER` admission, port-I/O or IDT-class asm intrinsic in a local initializer or return expression (not a top-level call/assignment-as-call statement).

**Verdict REACHABLE:** such a program clears `validate_asm_discharge` with no diagnostic (read); digest scenario at area_14.md:258 matches code (read).

## A0492

**read** `machine_calls_core.rs:6359-6369` (`expression_contains_call` in full) and `6339-6357` (`expression_contains_call_projection` — read: prunes `Member`/`Match` when receiver is not call-bearing per this helper).

**read** `5881-6096` (`validate_fact_call_projections`, `validate_expression` — read): `5920-5921` skips all projection work when `has_projection` is false; `6084-6088` emits nested-projection error only when `expression_contains_call` is true on the member receiver; `6093-6095` descends via `children` for other shapes but the detection gate still uses the limited call walk.

**Caller search** `expression_contains_call` in `machine_calls_core.rs` fact_call_projections module: 4 internal uses (`5919`, `6084`, `6121`, definition `6359`; read). Separate homonym in `contract_entailment.rs` and `domains_and_predicates.rs` (grep listed; not read — not relied on for closure).

**read** `program_validation.rs:511-513`, `4883`: prologue publishes `validate_fact_call_projections` results into checking facts (read).

**Input that reaches the branch:** a proof-fact or proposition root where the only call sits under Indexed/Binary/Unary/Match inside a member receiver (so `expression_contains_call` stays false).

**Verdict REACHABLE:** `has_projection` can be false or the nested-projection error bypassed, admitting or failing to validate fact-position call projections that should be rejected (read).

## A0493

**read** `machine_calls_core.rs:6592-6648` (`validate_runtime_proof_citations` — read through first diagnostic arms) and `6692-6714` (`is_fact_only_citation_call` — read).

**read** `6611-6613`: `if !call.receiver.is_empty() { continue; }` before proof-machine checks (read).

**read** `6708`: fact-only citation path requires `call.receiver.is_empty()` (read); qualified-receiver runtime proof calls are not classified here.

**Caller search** `\bvalidate_runtime_proof_citations\b` in `omega-rust`: 3 matches (`machine_calls_core.rs:6592`, `lib.rs:1510`, `checking.rs:1158` — read).

**read** `checking.rs:1158-1164`: pipeline invokes this after checking facts exist; errors fail compilation (read).

**Input that reaches the branch:** runtime machine body with a statement call to a proof machine using a non-empty namespace receiver.

**Verdict REACHABLE:** the call is skipped with no diagnostic (read); not a fact-only citation under `6708` and not rejected here.

## A0494

**read** `machine_calls_core.rs:7077-7167` (`validate_conformance_refinement` in full).

**read** `7083-7105`: builds `matching` by conformance requirement name or last `::` segment of machine name vs trait signature names; `7084-7087` `continue` when trait definition missing (read).

**read** `7106-7165`: loop runs only for `matching` entries; empty `matching` performs no `report_missing_targets` (read).

**Caller search** `validate_conformance_refinement`: 2 matches (definition and call at `6989` inside `validate_invocation_contracts` — read). `\bvalidate_invocation_contracts\b`: 2 calls in `program_validation.rs:4154`, `4870` (read).

**read** `program_validation.rs:4145-4154`: runs when `EffectAdmission::Complete` during `validate` (read).

**Input that reaches the branch:** machine with a trait conformance whose `requirement` name does not equal any trait machine signature name and whose machine name tail also matches none.

**Verdict REACHABLE:** synchronous-invocation refinement for that conformance is skipped with no error (read).

## A0495

**read** `machine_calls_core.rs:8878-8913` (`selected_conversion` through span guard) and `8737-8760` (`bind_float_conversion_casts` entry — read).

**read** `8910-8912`: empty/inverted `source_span` forces `return None` before operator selection (read).

**read** `8842-8875`: only casts with `Some` from `selected_conversion` become `ConversionCastBinding` rows (read).

**read** `preparation.rs:2060` (grep): `bind_float_conversion_casts` runs in typed-tree preparation (read cite only).

**Caller search** `\bselected_conversion\b`: 2 matches (definition and call in same module — read). `\bbind_float_conversion_casts\b`: 3 matches (`machine_calls_core.rs`, `lib.rs`, `preparation.rs` — read).

**Input that reaches the branch:** float/int conversion cast node with valid types per `8898-8908` but `source_span.span.start >= source_span.span.end` (synthesized, no source span).

**Verdict REACHABLE:** cast is left unbound to a boundary conversion operator and preparation continues unless another check fails (read; later stages for spanless casts not traced).

## A0496

**read** `machine_parameters_and_bindings.rs:4716-4785` (`form_call_selection` through early `return` blocks — read).

**read** `4770-4784`: invalid `target_symbol` with `BuildOperation::from_call_target(target_name)` matching listed build ops returns without pushing diagnostics (read).

**read** `4786+`: normal callee resolution and static-argument validation follow only when the early returns do not trigger (read).

**Caller search** `\bform_call_selection\b` in `omega-rust`: 5 matches, all `machine_parameters_and_bindings.rs` (read: `1290`, `4061`, `4691`, definition `4716`, plus re-export path in module).

**read** `4453+` / `program_validation.rs:4137-4138`: `form_static_machine_selections` drives compile-time machine parameter selection during `validate` (read).

**Input that reaches the branch:** nominal static machine use with invalid symbol handle but callee name string recognized as a Build operation (ProviderSelection, BoundaryAcceptance, etc. per `4772-4781`).

**Verdict REACHABLE:** static machine argument formation returns unchecked for that call site (read).

## What I did not read

- Full `canonical_template_contract_bytes` body below line 3134 beyond cited error arms.
- Whether trust/packaging `Missing` always runs for every compile that sets `identity: None` (`providers_and_trust.rs` beyond `719`).
- `contract_entailment.rs` homonym `expression_contains_call` bodies.
- Post-`bind_float_conversion_casts` operator resolution for unbound spanless casts.
- Corpus samples exercising each REACHABLE shape.

SLICE COMPLETE

