# slice_0051

Area digest preamble (read): Area 13 is the first half of `psi/compilation/validation/`, centered on `contract_entailment.rs` (`ce`) through proof/engine/ranking/specification walks. Words `read` / `inferred` per site. Digest pinned at `457adf4a11f`; cited `ce` line ranges matched this checkout (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0389 | ce:16029-16086 | REACHABLE | Multi-segment names only test the first segment as a data name; fenced proof-only `ensures` filtering can miss nested mentions. |
| A0390 | ce:18278, ce:19153 | REACHABLE | `machine_states(...).first()` missing returns silently on quotient and law-guarantee paths without the trait satisfier’s no-states diagnostic. |
| A0391 | ce:21185-21286 | REACHABLE | Scalar loan handback checks `continue` when the place is not a resolved restricted-record field segment. |
| A0392 | ce:21947-21966 | REACHABLE | `CarrierFit::Pending` makes `retain` return true and defer fit to a later pending `IntegerRangeObligation`. |
| A0393 | ce:23093-23103 | REACHABLE | `OperandFit` can leave `range_complete` true after an `Unknown` judgment on an unobserved operand. |
| A0394 | ce:23537 | REACHABLE | With `!has_source_metadata()`, any user `Nat` with Zero/Succ shape is treated as proof `Nat`. |
| A0395 | ce:24048 | REACHABLE | Quotient requires matching accepts a callable whose state symbol coincides with the relation’s last-segment name. |
| A0396 | ce:29298-29300 | REACHABLE | `Modulo` without an admitted integer carrier skips engine binding but `install_nonpolynomial_terms` still succeeds. |

## A0389

**Cited site (read):** `omega-rust/psi/compilation/validation/src/contract_entailment.rs:16029-16086` — `fact_mentions_proof_only_data` on `ExpressionNode::Name`: `[single]` checks state parameters or reserved result type; `[first, ..]` calls `proof_only_definition(first)` only (ce:16062); `ExpressionNode::Member` is handled via `find_in_composite_children` at ce:16127, which recurses the receiver only (ce:16127).

**Enclosing function (read):** `fact_mentions_proof_only_data` (ce:16017-16113) and `find_in_composite_children` (ce:16115-16138).

**Callers (search):** pattern `\bfact_mentions_proof_only_data\b` in `omega-rust` — 1 definition (`contract_entailment.rs:16017`), 2 production calls in `validate_machine_contract_entailment` (`contract_entailment.rs:90715`, `90864`); 1 re-export line (`contract_entailment.rs:90297`).

**Upward trace (read):** `validate_machine_contract_entailment` (`contract_entailment.rs:90324`) filters `ensures` facts (`90710-90859`). When `mention` is `None` and `judge_structural` is `Proven`, the fact is recorded in `proven.entry` or `proven.normal_returns` and dropped from remaining `ensures` without fencing (`90764-90777`). When `mention` is `Some`, `StructuralJudgment::Unknown` pushes the fenced diagnostic (`90824-90855`). If `fenced_structural || ensures.is_empty()`, entailment returns early (`90869-90870`).

**Input that reaches the branch:** An `ensures` expression that mentions proof-only payload through a multi-segment place where the first segment is a parameter or local name (not the proof-only data definition name), while structural judging still returns `Proven`.

**Smallest program (plain words):** A checked machine whose `ensures` states a property of a proof-only field reached through an ordinary parameter path, where the mention detector never names proof-only data, so the contract is admitted without the proof-only fence error.

## A0390

**Cited sites (read):** `contract_entailment.rs:18278-18279` — `check_proposition_law_conformance` returns when `program.machine_states(machine).first()` is `None`. `contract_entailment.rs:19153-19154` — `check_law_conformance` returns on the same condition after collecting non-empty law conjuncts or proposition laws (`19143-19145`).

**Enclosing functions (read):** `check_proposition_law_conformance` (ce:18264+); `check_law_conformance` (ce:19110-19186+).

**Callers (search):** pattern `\bcheck_law_conformance\b` in `omega-rust` — definition `contract_entailment.rs:19110`; calls at `traits.rs:1552`, `quotients.rs:179`, `contract_entailment.rs:17917` (law guarantees). pattern `\bcheck_proposition_law_conformance\b` — definition `contract_entailment.rs:18264`, call `contract_entailment.rs:19174` only.

**Upward trace (read):** `traits.rs:1449-1454` pushes `"has no states"` and returns before `check_law_conformance` (`1552`). `quotients.rs:169-179` has no `machine_states` guard before `check_law_conformance`. `matched_machine_law_guarantees` (`contract_entailment.rs:17877`) calls `check_law_conformance` at `17917` with no empty-states check; empty diagnostics after silent return allow the loop to continue (`17926-17928`).

**Input that reaches the branch:** A machine with zero states but non-empty proposition laws or law conjuncts on the requirement signature, reached through quotient conformance or law-guarantee indexing rather than `validate_machine_single_requirement`.

**Smallest program (plain words):** A quotient conformance row pointing at a realization machine with no states while the inherited requirement still carries proposition laws; law conformance returns without diagnostics and never compares laws to the empty body.

## A0391

**Cited sites (read):** `contract_entailment.rs:21185-21187`, `21188-21190`, `21206-21207`, `21217`, `21257-21281`, `21283-21287` — early `continue` when call-site contract, parameters, or argument arity fail; empty `places` after origin resolution; `parent.segments.pop()` not a `PlaceSegment::Field`; `any_field_owner` missing.

**Enclosing function (read):** `validate_scalar_loan_handbacks` (ce:21167-21358).

**Callers (search):** pattern `\bvalidate_scalar_loan_handbacks\b` in `omega-rust` — definition `contract_entailment.rs:21167`, production call `program_validation.rs:4706` inside `validate_machine_scalar_loan_handbacks` (`4696-4712`).

**Upward trace (read):** `validate_machine_scalar_loan_handbacks` walks every statement in every state and accumulates diagnostics from handback validation only when `diagnostics.len()` increases (`4705-4713`).

**Input that reaches the branch:** A call that lends an exclusive primitive reference whose argument place canonicalizes to a local or root without a trailing record-field segment the checker can attribute to a restricted field.

**Smallest program (plain words):** A machine that lends a scalar reference tied to a local or non-field place to a callee with exclusive primitive ref parameters; handback preservation for restricted record fields is never run and no error is emitted.

## A0392

**Cited site (read):** `contract_entailment.rs:21947-21966` — `CarrierFit::retain` on `Pending { interval }` pushes `IntegerRangeObligation::retain_address_fit` and returns `true`; `Proven` also returns `true` (`21946`).

**`Pending` producer (read):** `CarrierFit::Pending` is constructed only in `integer_value_fit` at `contract_entailment.rs:4961-4963` when the address-model bound matches a constant `u64` interval edge case.

**Enclosing function (read):** `impl CarrierFit` `retain` (ce:21938-21969); used from anonymous address-cast checking at `contract_entailment.rs:41480` (`retain` call) where failure to retain pushes the diagnostic (`41482-41488`).

**Callers of `retain` on `CarrierFit` (search, read):** `contract_entailment.rs:41480`, `41584`, `41690` (formation obligation paths).

**Upward trace (read):** Obligations are collected into `formation_obligations` in `program_validation.rs` (`4547`, `4621`, `4900-4902`). Pending obligations are carried on `checked_trees` facts (`contract_and_obligation_facts.rs:738-739` `is_pending`). Stage 06 lowering turns pending rows into scalar range conditions (`attached_unit.rs:7461-7494`, read). `IntegerRangeObligation::discharge` exists at `contract_and_obligation_facts.rs:765-827` (read); I did not trace every compile-time caller of `discharge` on range obligations.

**Input that reaches the branch:** An anonymous integer cast to `addr` where `integer_value_fit` yields `Pending` and `retain` records an address-model obligation instead of refusing formation at the call site.

**Smallest program (plain words):** A declaration-site anonymous address literal whose fit judgment is `Pending`; validation skips the immediate “cannot prove fit” error because `retain` returns true and records a deferred range obligation.

## A0393

**Cited sites (read):** `contract_entailment.rs:23088-23094` — on unobserved operands, sets `judgment` to `Unknown` and `continue`s without setting `range_complete = false` (contrast `23084` which clears `range_complete`). `contract_entailment.rs:23101-23103` — missing observed types set `Unknown` and `continue` similarly.

**`range_complete` semantics (read):** `OperandFit::has_positive_proof` (`22879-22886`) treats `range_complete && range_recipe.is_target_dependent()` as positive proof alongside `CarrierFit::Proven`. Initial `range_complete` is `true` (`22839`, `22861`).

**Enclosing impl (read):** `StructuralPrefixVisitor for OperandFit` (`22878-23176`), driven by `observe_operand_fit_at_occurrence` (`22770+`).

**Callers (search):** `observe_operand_fit_at_occurrence` used from exact binary formation (`22373`), `operand_fits_at_occurrence` (`22584`), and additional prefix walks in the same module (grep listing read).

**Input that reaches the branch:** A binary completed-operation operand-fit walk where one observation entry is `Unknown` for an unobserved operand while a prior entry left `range_complete` true and a target-dependent `range_recipe`.

**Smallest program (plain words):** A machine body binary whose operand-fit prefix walk sees one unobserved operand arm; validation may still treat integer formation as range-complete for a dependent recipe even though the judgment at that site is `Unknown`. (inferred; no concrete program executed — digest [094] aligns.)

## A0394

**Cited site (read):** `contract_entailment.rs:23532-23537` — `proof_nat_type` returns true for name `"Nat"` when `symbol_source_origin` is `None` and `!program.symbols.has_source_metadata()`, without requiring `SourceOrigin::Toolchain`.

**Enclosing function (read):** `proof_nat_type` (ce:23526-23554); `has_source_metadata` is `self.sources.is_some()` (`symbols/src/table.rs:1716-1718`, read).

**Callers (search):** `proof_nat_type` referenced only at `contract_entailment.rs:23628` inside `proof_nat_cast` (`23623-23629`). `proof_nat_cast` called from `expression_types_and_fields.rs:67` and multiple sites in `contract_entailment.rs` (grep read).

**Upward trace (read):** `proof_nat_cast` gates proof-integer cast treatment; when true, `proof_integer_nonnegative` is invoked for cast values (`expression_types_and_fields.rs:67-84`, read).

**Input that reaches the branch:** A checking input whose symbol table has no source metadata (`sources` absent) and a user-defined data type named `Nat` with `Zero` and `Succ` shape.

**Smallest program (plain words):** A program compiled without retained source custody where the author defines a local `Nat` algebra; validation treats casts to that type as proof-natural casts like the toolchain `Nat`.

## A0395

**Cited site (read):** `contract_entailment.rs:24041-24057` — `relation_call_matches_quotient` returns true on quotient relation symbol equality (`24038-24039`), last-segment name match (`24048-24049`), or any machine where `machine_or_state_symbol(machine, relation_symbol)` and `state_by_symbol(machine_states, call.target_symbol)` succeeds (`24051-24057`).

**Enclosing function (read):** `relation_call_matches_quotient` (ce:24033-24058); used from `quotient_equality_from_requires` (`23858-23860`).

**Callers (search):** `quotient_equality_from_requires` at `contract_entailment.rs:23849`; `quotient_equality_from_requires` referenced at `90828-90834` (ensures quotient-equality diagnostic branch, read).

**Upward trace (read):** `quotient_equality_from_requires` scans machine `requires` facts for relation calls matching the quotient goal carriers (`23858-23868`).

**Input that reaches the branch:** A `requires` fact calling a machine state that shares the quotient relation’s textual last segment but is not the declared relation symbol.

**Smallest program (plain words):** A quotient proof goal whose `requires` cites an unrelated machine state with the same name as the quotient’s relation constructor; validation treats the call as the quotient relation match.

## A0396

**Cited site (read):** `contract_entailment.rs:29271-29300` — inside `install_nonpolynomial_terms` on `Binary`, when `admitted_carrier(...)?` is `None`, non-`Modulo` operators `return None` (`29298-29299`), but `Modulo` falls through without engine binding.

**Enclosing function (read):** `install_nonpolynomial_terms` (`29203-29306`); `unselected_or_builtin_arithmetic` at `29167-29187` (empty selection occurrence list yields `all()` true — digest [093] checked note).

**Callers (search):** pattern `\binstall_nonpolynomial_terms\b` in `omega-rust/psi/compilation/validation/src/contract_entailment.rs` — 12 occurrences (definition plus recursive calls and ranking/install sites; count mode read).

**Upward trace (read):** Function returns `Some(())` at `29305` after the modulo no-carrier branch, installing child terms recursively for binary operands (`29251-29259`) but skipping `bind_strict_integer_division` / shift / bitwise bindings when no carrier.

**Input that reaches the branch:** A `%` binary expression in a proof/ranking install walk where neither operand admits an integer carrier at the current depth.

**Smallest program (plain words):** A ranking or proof polynomial install path containing modulo on values with no admitted integer carrier; validation accepts the walk without binding modulo in the engine.

## What I did not read

- Whether typed-tree construction can deliver machines with zero states to `quotients.rs:179` or `matched_machine_law_guarantees` without an earlier error (would decide reachability of A0390 beyond the silent-return sites).
- Full compile pipeline discharge of every pending `IntegerRangeObligation` from `CarrierFit::Pending` retain (would decide whether A0392 is harmless deferral vs end-to-end acceptance).
- Concrete corpus programs exercising A0393 `range_complete` staleness.
- Production programs with `symbols.sources == None` outside bootstrap/test harnesses (would narrow A0394).

SLICE COMPLETE
