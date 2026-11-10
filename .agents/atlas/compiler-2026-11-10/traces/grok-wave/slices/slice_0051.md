slice_0051

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0389 | ce:16029-16086 | REACHABLE | Multi-segment name paths only treat the first segment as a data definition name, so proof-only mentions via parameters can miss the fence and skip proof-only diagnostics. |
| A0390 | ce:18278; ce:19153 | UNDECIDED | Trait satisfiers error on no states before `check_law_conformance`, but quotient conformance calls it with no entry-state gate; I did not trace whether stateless realizations exist. |
| A0391 | ce:21185-21288 | REACHABLE | Handback validation `continue`s when the resolved place's last segment is not a `Field` or `any_field_owner` fails, so no diagnostic is emitted for that loan site. |
| A0392 | ce:21947-21966; ce:4961 | CLOSED | `Pending` is only built at ce:4961; `retain` defers via `IntegerRangeObligation` and later lowering rejects still-pending obligations (ce:06 pipeline read). |
| A0393 | ce:23093; ce:22879-22887 | UNDECIDED | `CompletedOperation` can set `Unknown` without clearing `range_complete`, but I did not trace a concrete program through `observe_operand_fits_batch`. |
| A0394 | ce:23537 | REACHABLE | With `SourceOrigin::None`, missing source metadata makes `proof_nat_type` accept any `Nat` with Zero/Succ shape as proof `Nat`. |
| A0395 | ce:24048 | REACHABLE | `relation_call_matches_quotient` matches `call.target` text to the relation's last path segment without requiring the quotient relation symbol. |
| A0396 | ce:29298-29300 | REACHABLE | `Modulo` alone falls through when `admitted_carrier` is absent while other non-polynomial operators return `None`. |

### A0389

1. Read `fact_mentions_proof_only_data` in full at `contract_entailment.rs:16017-16113` (read) and `find_in_composite_children` at `16115-16138` (read).
2. Search `rg -w 'fact_mentions_proof_only_data' /agent/repos/omega/omega-rust` → 5 hits (read).
3. Read callers `90710-90868` inside `validate_machine_contract_entailment` (`90324+`; read filter loop, did not read full engine path except `90912-90965`).
4. At `16062-16063` (read), `[first, rest]` calls `proof_only_definition(first)` only when the first spelling names a data definition; a single-segment parameter path is handled at `16045-16060`, but multi-segment paths starting with a parameter name do not recurse into parameter proof-only types before the data-name test.
5. Input reaching the accepting branch: an `ensures` expression handle in the filter at `90712-90715` where mention is `None` because the path's first segment is not the proof-only data name.
6. Verdict: **REACHABLE** — a checked-body machine whose ensures states a false property over proof-only storage accessed only through a parameter-qualified multi-segment path should be rejected by the proof-only fence (`90845-90854`) but can enter generic structural/ engine entailment instead.
7. Unsure: whether `judge_structural` ever returns `Proven` for such facts without a separate veto (did not read full `judge_structural` closure).

### A0390

1. Read `check_proposition_law_conformance` at `18264-18279` (read full early returns, not full law loop).
2. Read `check_law_conformance` at `19110-19155` (read through entry-state guard).
3. Search `rg -w 'check_law_conformance' /agent/repos/omega/omega-rust/psi/compilation/validation` → 4 call sites (read names only).
4. Read `traits.rs:1449-1455` (read): trait satisfier path pushes an error when `machine_states(machine).first()` is `None` before `check_law_conformance` at `1552`.
5. Read `quotients.rs:179-187` (read): quotient conformance calls `check_law_conformance` with no prior entry-state check.
6. Read `law_guarantees::matched_machine_law_guarantees` `17877-17925` (read): same silent skip if no entry state.
7. Verdict: **UNDECIDED** — silent return is real on the quotient/guarantee paths, but I did not determine whether a stateless realization machine can reach validation success.
8. Unsure: whether quotient realization machines are always stateful.

### A0391

1. Read `validate_scalar_loan_handbacks` in full at `21167-21359` (read).
2. Search `rg -w 'validate_scalar_loan_handbacks' /agent/repos/omega/omega-rust` → 3 hits: definition, `pub use`, `program_validation.rs:4706` (read caller header `4696-4724`, not full `judge_machines`).
3. Caller passes diagnostics mut; skipped sites do not append errors.
4. Input: call site with exclusive primitive parameter (`21192-21198`) but canonical place whose last segment is not `PlaceSegment::Field` (`21283-21285`) or field owner unresolved (`21286-21288`).
5. Verdict: **REACHABLE** — a machine that lends an exclusive primitive referent into a callee when the argument place cannot be popped to a field should be rejected on handback but passes this function silently.
6. Unsure: whether other validation rejects the same call pattern earlier (did not read `calls.rs` write frames for this shape).

### A0392

1. Read `CarrierFit::retain` at `21938-21968` (read) and `integer_value_fit` `Pending` construction at `4952-4964` (read).
2. Search `rg '\.retain\(' contract_entailment.rs` with `CarrierFit` context → production uses at `41480`, `41584-41589`, `41690-41695` (read those branches).
3. Read `IntegerRangeObligation::retain_address_fit` and `is_pending` at `checked-trees/.../contract_and_obligation_facts.rs:703-740` (read).
4. Read lowering gate `06_checked-trees-to-lowered-psi/.../attached_unit.rs:7461-7494` (read): pending integer range obligations become scalar conditions; undischarged pending blocks lowering.
5. Read `program_validation.rs:4900-4903` (read): obligations are collected, not discharged in `finish_diagnostics`.
6. Verdict: **CLOSED** — `retain` returning `true` for `Pending` only records a deferred obligation; compile acceptance still requires later discharge or lowering fails.
7. Unsure: full target-selection discharge path in 05 (did not read `IntegerRangeObligation::discharge` callers outside `contract_and_obligation_facts.rs:765-827`).

### A0393

1. Read `OperandFit::has_positive_proof` at `22879-22887` (read) and `observe` loop `22942-23175` (read; `CompletedOperation` branch in full).
2. At `23088-23094` and `23101-23103` (read), `Unknown` is set and `continue` without `range_complete = false` (contrast `23084-23085`, `22897-22899`).
3. Read `observe_operand_fits_batch` `22702-22734` (read): acceptance uses `has_positive_proof()` regardless of `Unknown` judgment.
4. Input: `OperandSource::CompletedOperation` where scalar operands fail `scalar_operand_is_observed` after a prior visitor pass left `range_complete == true` and a target-dependent `range_recipe`.
5. Verdict: **UNDECIDED** — code path is plausible (inferred) but I did not connect it to a specific ill-typed program or see `observe_operand_fits_batch` drive a compile accept.
6. Unsure: whether `range_recipe` can survive the `continue` pattern in real traversals.

### A0394

1. Read `proof_nat_type` at `23526-23558` (read) and caller `proof_nat_cast` at `23623-23630` (read).
2. Origin rule at `23534-23537` (read): `SourceOrigin::None` ⇒ `!program.symbols.has_source_metadata()`.
3. Read `symbols/table.rs:1716` reference via grep only (inferred): `has_source_metadata` exists on symbol table.
4. Input: `TypeReferenceHandle` named `Nat` with Zero/Succ shape and symbols lacking source metadata.
5. Verdict: **REACHABLE** — a user-authored `Nat` shaped like proof naturals can be treated as proof `Nat` for `proof_nat_cast` / proof integer formation when metadata is absent.
6. Unsure: which validation inputs lack source metadata in production hosts (not traced).

### A0395

1. Read `relation_call_matches_quotient` at `24033-24058` (read) and consumer `quotient_equality_from_requires` `23849-23876` (read).
2. Search `rg -w 'quotient_equality_from_requires' omega-rust` → 3 hits; entailment use at `90594-90605` (read).
3. Accepting branch at `24048-24049` (read): `call.target.as_str() == name` for relation's last path segment only.
4. Input: `requires` fact that is a call whose target spelling equals the quotient relation's last segment but binds a different callable (`call.target_symbol != relation_symbol` and machine-state fallback false).
5. Verdict: **REACHABLE** — a false quotient equality in `ensures` can be marked structurally proven via mismatched `requires` relation call that shares only the last name segment.
6. Unsure: whether call target resolution prevents duplicate last-segment names in practice (not traced).

### A0396

1. Read `install_nonpolynomial_terms` at `29203-29306` (read).
2. Read caller `arrival_scalar_actual` `33020-33036` (read): `?` propagates `None` from install failure.
3. At `29271-29300` (read): without `admitted_carrier`, non-`Modulo` operators `return None`; `Modulo` falls through to `Some(())`.
4. Input: ranking/arrival expression with `BinaryOperator::Modulo` on operands where `admitted_carrier` returns `None` at depth 0.
5. Verdict: **REACHABLE** — modulo in a proof ranking term can be installed into the engine without strict integer division binding that other operators require.
6. Unsure: downstream effect on termination proof acceptance (did not read full ranking judge after install).

### What I did not read

- Full `validate_machine_contract_entailment` above `90710` (structural judge closure definition).
- Complete `inductive_transition_entailment` body past `16290`.
- `check_proposition_law_conformance` law loop after `18288`.
- 05 pipeline integer-range obligation discharge call sites beyond grep.
- Whether typed-tree formation always gives machines ≥1 state on quotient realizations.

SLICE COMPLETE
