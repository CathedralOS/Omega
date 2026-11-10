# slice_0052

Digest: `area_13` (`omega-rust/psi/compilation/validation/src/contract_entailment.rs`, abbreviated `ce`). Preamble read. Rows A0397–A0404.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0397 | ce:32895-32909 | REACHABLE | Invalid range plus Distance forces `membership_and_pinning` true without that facet being proved |
| A0398 | ce:34118-34176 | REACHABLE | Structurally refuted callee `requires` that matches a skip rule emits no diagnostic |
| A0399 | ce:23648 | UNDECIDED | Whether satisfiable hypotheses can set `requires_unsatisfiable` was not shown |
| A0400 | ce:40706-40708, 41730-41732 | REACHABLE | DynamicSlot parameter-count mismatch makes `owner_parameter_scope` fail and silently skips checks |
| A0401 | ce:41529-41533 | REACHABLE | Type-predicate binaries and other expression kinds return before specification checks run |
| A0402 | ce:41770-41784 | REACHABLE | Named-evidence Proposition requirements `continue` without a proof attempt here |
| A0403 | ce:46198 | REACHABLE | Crash transitions `continue` before body transfer, so no leaf arm must carry the conclusion |
| A0404 | ce:54329 | UNDECIDED | `Some(true)` shortcuts in `prepared_predicate_proofs` were not soundness-checked |

## A0397

**Cited:** `omega-rust/psi/compilation/validation/src/contract_entailment.rs:32895-32909` (read; lines match digest at current checkout).

**Enclosing function:** `prove_ranking_range_entry_with_formation` (`ce:32888-32914`, read).

**Branch:** `unranged_distance` is true when `!range.is_valid()` and the measure is `RankingRangeMeasure::Distance { .. }` (`ce:32895-32896`, read). After `prove_edge` returns `Some(proof)` (`ce:32897-32908`, read), `membership_and_pinning` becomes `proof.membership_and_pinning || unranged_distance` (`ce:32909`, read), so membership/pinning can be reported true even when `proof.membership_and_pinning` is false.

**Callers (search `\bprove_ranking_range_entry`, whole `omega-rust`, read):** 1 definition `ce:32877`, 1 delegate `ce:32884`, 1 re-export `validation/src/lib.rs:1947`, 2 production call sites in `checks_termination.rs:2040` and `6047` (5 listing lines total; 2 pipeline call sites).

**Acceptance path:** Termination/ranking entry formation consults `entry_proof.membership_and_pinning` (`checks_termination.rs:6049-6050`, read). If `prove_edge` succeeds but does not prove membership/pinning, an invalid range handle with a Distance measure still sets `membership_and_pinning` via the OR.

**Input in plain words:** A termination witness using bounded-distance ranking where the rank-range expression handle is invalid, while `prove_edge` still returns a proof object without membership/pinning.

**Verdict:** REACHABLE — digest claim matches the OR at `ce:32909`; no closing check in the read callers invalidates the shortcut.

## A0398

**Cited:** `ce:34118-34126`, `34129`, `34152-34176`, `35086-35098` (read).

**Enclosing function:** `reject_refuted_value_call_requires` (`ce:34108` onward, read through `34199`).

**Branch:** Only `StructuralJudgment::Refuted` facts reach the diagnostic (`ce:34149-34150`, read). Several `continue` skips apply before `diagnostics.push` (`ce:34152-34176`, read): boolean-false requirement with case-membership guard on caller; terminal transfer with field-read requirement; closed literal equation paired with authored clause (`authored_clause_is_closed_literal_equation`, `ce:35080-35103`, read). `hypotheses_contradictory` causes early return with no per-requirement work (`ce:34129-34130`, read).

**Callers (search `\breject_refuted_value_call_requires\b`, `omega-rust`, read):** 1 definition `ce:34108`, 1 re-export `ce:90264`, 1 production call `calls.rs:40439` (3 matches).

**Input in plain words:** An executing value call whose instantiated callee `requires` clause is structurally false (Refuted), but satisfies one of the skip predicates (for example literal `false` with a case-membership guard on the caller state, or a terminal field-read skip).

**Verdict:** REACHABLE — the diagnostic path explicitly skips refuted facts that match those rules; the function only reports Refuted judgments that survive skips.

## A0399

**Cited:** `ce:23648` (read); digest also lists 38 other `requires_unsatisfiable` sites and `is_contradictory` at `ce:53127`.

**Enclosing function (witness site):** `proof_integer_nonnegative` (`ce:23632-23649`, read): `engine.requires_unsatisfiable || engine.prove_at_least(...)` (`ce:23648`, read).

**Flag writers (read):** `Engine::install_hypotheses` (`ce:6664-6743`) sets `requires_unsatisfiable` when a constant lowered bound contradicts (`ce:6707-6710`) or when both a difference and its negation have nonnegative lower bounds (`ce:6732-6733`). `PreparedIntegerPremises::is_contradictory` (`ce:53127-53135`) reads `engine.requires_unsatisfiable` or a bound query.

**Search:** `\brequires_unsatisfiable\b` in `contract_entailment.rs`, count mode: **39** matches (read; matches digest [094] grep count).

**What was not traced:** The other 38 use sites, and whether any `prove_at_least` true/false pair can mark satisfiable hypotheses unsatisfiable.

**Verdict:** UNDECIDED — a wrong pass requires a false `requires_unsatisfiable`; the read setter paths look tied to constant contradictions or simultaneous nonnegative proofs (`ce:6709`, `ce:6733`), but no concrete satisfiable hypothesis set that still sets the flag was exhibited.

## A0400

**Cited:** `ce:40706-40708`, `41730-41732` (read).

**Enclosing functions:** `validate_declaration_value_arguments` (`ce:40694-40761`, read) returns immediately when `owner_parameter_scope` is `None` (`ce:40706-40708`). `validate_specification_expression_in_scope` (`ce:41256` onward, read) hits the same guard before the requires-proof loop (`ce:41730-41732`).

**`owner_parameter_scope`:** For `SelectedCallContract::DynamicSlot`, returns `None` when `declared.len() != instantiated.len()` (`ce:42165-42170`, read).

**Callers of specification validation (read):** `validate_declaration_call_requirements` / `validate_specification_call_requirements` called from `arithmetic_domains.rs` (grep: 3 call sites at `24549`, `24646`, `25269`).

**Separate path (read, not a closing gate here):** `AppliedResultPresentation` for DynamicSlot returns `None` on several mismatches including parameter lengths (`ce:39958-39962`), but the specification walk at `41730` still exits silently without diagnostics when `owner_parameter_scope` is `None`.

**Input in plain words:** A retained specification or declaration call resolved to `DynamicSlot` whose signature parameter count differs from the slot’s instantiated `data_parameters` count.

**Verdict:** REACHABLE — both argument typing (`40706-40708` via `41703-41714`) and callee `requires` proof (`41719` onward) are skipped when scope is `None`.

## A0401

**Cited:** `ce:41529-41533`, `41537-41540`, `41659` (read).

**Enclosing function:** `validate_specification_expression_in_scope` (`ce:41256` onward, read), call/operator arm of the `match node`.

**Branches (read):** `ExpressionNode::Binary(_)` with `program.static_type_predicate_operands(expression).is_some()` returns immediately (`ce:41529-41532`). Binary/indexed operators with no `binary_operator_spelling` return (`ce:41537-41540`). Any other `ExpressionNode` kind hits `_ => return` (`ce:41659`).

**Callers:** `validate_specification_expression` / `validate_specification_expression_in_scope` — 13 references in `contract_entailment.rs` only (grep count, read); external entry via `validate_*_call_requirements` in `arithmetic_domains.rs` (above).

**Input in plain words:** A specification `requires` or `ensures` root that is a static type-predicate binary, an unspellable operator form, or a non-call expression kind handled only by the default arm (literals, unary, etc.).

**Verdict:** REACHABLE — those forms leave the function without argument correspondence, operator resolution, or requires proof in this walk.

## A0402

**Cited:** `ce:41770-41784` (read).

**Enclosing function:** Same `validate_specification_expression_in_scope` requires loop (`ce:41719` onward, read).

**Branch:** `has_named_evidence_lane` is true for concrete contracts with named evidence requirements and evidence-only machine arguments (`ce:41770-41777`, read). For `(requirement, named_evidence)` with `named_evidence`, that lane, and `ProofFact::Proposition(_)`, the loop `continue`s (`ce:41778-41783`, read) with no proof in this block.

**Input in plain words:** A specification call to a concrete member with proposition requirements designated as named evidence, where evidence argument count matches and there is no static requirement dispatch or quotient operation.

**Verdict:** REACHABLE — the `continue` skips proof for those propositions in this validator.

## A0403

**Cited:** `ce:46198` (read).

**Enclosing function:** `recognize_structural_state_leaves` → inner walk in `recognize_structural_state_leaves_from` (`ce:45421` onward, read). On `StatementNode::Transition`, after optional `SiteArm::deliver`, crash exits hit `continue` (`ce:46198-46199`, read) before `terminal_body_transfer` and downstream leaf completion.

**Callers (search `recognize_structural_state_leaves`, `omega-rust`, read):** definition `ce:45366`, delegates `45387`, `45421`, internal uses `44078`, `44472`, `44534`, `46318`, `46515`, `58973` (9 listing lines; all in `contract_entailment.rs` per grep).

**Input in plain words:** A proof-only structural trace through a state body that takes a transition whose exit is `Crash`, while site visitors still expect leaf delivery along non-crash paths.

**Verdict:** REACHABLE — crash transitions do not proceed to body transfer in this loop; digest note that callers were not traced does not negate the local skip.

## A0404

**Cited:** `ce:54329`, `54342`, `54377`, `54419` (read).

**Enclosing function:** `prepared_predicate_proofs` (`ce:54159` onward, read through `54427`).

**Branches (read):** Multiple early `return Some(true)` paths, including when `!holds || work.operations.is_empty()` (`ce:54328-54329`, `54363-54364`), when all linear operations admit (`ce:54341-54342`, `54376-54377`), and when `comparison_holds` after premise install (`ce:54417-54419`).

**Caller:** Invoked from integer predicate proving in the same impl (`ce:54144-54154`, read).

**Verdict:** UNDECIDED — digest [095] states these shortcuts were not verified; read shows the shortcuts exist but no trace shows they accept a predicate that should fail, nor a closing check elsewhere.

## What I did not read

- Full bodies of all 39 `requires_unsatisfiable` use sites beyond `proof_integer_nonnegative` and `Engine::install_hypotheses`.
- `traits.rs` / other DynamicSlot admission paths beyond `ce:39932-39962` skimming.
- Upstream callers of `recognize_structural_state_leaves` beyond grep listing (no upward proof acceptance trace).
- Entire `prepared_predicate_proofs` prerequisite chain and runtime witnesses for false `Some(true)`.
- `git show 457adf4a11f:` re-verification (current checkout lines matched cited symbols).
- Any file outside the cited `ce` regions except caller grep targets named above.

SLICE COMPLETE

