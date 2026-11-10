# slice_0037

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0277 | prf/checker.rs:5427-5473 | REACHABLE | First matching machine data field by member name can classify a signed operand as unsigned and lift its range minimum to zero. |
| A0278 | prf/checker.rs:6522 | UNDECIDED | BoundedValue and GuardedTransition arms are empty here; no production discharge found elsewhere in repository search. |
| A0279 | prf/lib.rs:845 | REACHABLE | Twelve ExpressionNode variants fall through to false, so calls inside them are invisible to call-free guards and dehoisting. |
| A0280 | md:10775-10779 | UNDECIDED | Under-construction family applications become Sort(Type 0); whether final signature checking rejects misuse was not traced end-to-end. |
| A0281 | md:5213-5215 | UNDECIDED | Missing elaboration continues without retaining a theorem; consumer indexed_contract_is_proved only filters ensures, does not alone accept. |
| A0282 | md:2211-2215 | UNDECIDED | Non–three-argument Id normal forms are skipped silently; impact on identity_value consumers not traced to acceptance. |
| A0283 | md:5418-5518 | UNDECIDED | After 128 passes pending aliases remain without diagnostic; whether missing source_evidence lets a program pass was not shown. |
| A0284 | md:1109 | UNDECIDED | Selector maps to Integer carrier here while scalar_operand_reference excludes Selector; no traced proof path that wrongly accepts Selector. |

## A0277

Digest item 9 (read `digests/area_11.md` preamble, path shorthand `prf/` = `omega-rust/psi/compilation/proof/src/`).

Opened `prf/checker.rs:5427-5473` (`operand_is_unsigned`, `operand_declared_primitive`); read enclosing functions through `guard_narrowed_operand_range` and `guarded_integer_range_for_assignment_with_context` (read, lines re-located at current checkout; cited band matches digest at `457adf4a11f`).

`operand_declared_primitive` (`prf/checker.rs:5446-5474`, read) takes `self.receiver` member paths only, then iterates `program.machines()` in order (`5459-5472`), returning the first `data_field_type_by_name` hit for `member.member` without scoping to the obligation’s machine.

`operand_is_unsigned` (`5427-5443`, read) treats U8–U64 and Addr as unsigned.

Call sites: `rg '\boperand_is_unsigned\b|\boperand_declared_primitive\b' omega-rust` → 5 matches, all in `prf/checker.rs` (definitions at `5427`, `5446`; uses at `5221`, `5335`).

Upward: `guarded_integer_range_for_assignment_with_context` (`5190-5305`, read) calls `operand_is_unsigned` when tightening witness bounds (`5220-5224`, `5334-5337`). That range feeds `check_bounded_assignment` on certificate `Uncovered` (`374-397`, read) and named constraints `positive` / `non_negative` / `exact` via `guarded_integer_range_for_assignment` (`6087-6095`, read). Empty diagnostics from `check_proof_plan_with_decisions` allow `prepare_typed_program` to return `Ok` (`pipeline/05_typed-trees-to-checked-trees/src/checking.rs:422-438`, read).

**REACHABLE (read):** Two machines each attach data with the same field spelling; an earlier machine’s data defines that field as unsigned while the current machine’s field is signed. An assignment obligation on the signed field uses that member in a binary expression with witness bounds; the checker walks machines in list order, picks the unsigned field first, sets the operand minimum to zero, and interval reasoning can accept the assignment though the signed field may take negative values.

## A0278

Opened `prf/checker.rs:6522` inside `check_proof_plan_inner` (`6475-6550`, read).

`ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_)` is an empty arm in the `check_obligations!` literal block (`6522`, read). `decided_here` is false for those kinds (`6489-6492`, read), so `record_outcome` does not run for them (`6533-6535`, read). `ProofPlanMeasurements::record_obligation` increments `decided_elsewhere` (`5615-5616`, read; field at `5576`, read).

Caller search: `rg 'check_proof_plan_with_decisions' omega-rust` → 3 matches: definition `prf/checker.rs:6468`, wrapper `6455`, sole pipeline caller `pipeline/05_typed-trees-to-checked-trees/src/checking.rs:422` (read).

`rg 'BoundedValue|GuardedTransition' omega-rust` (read): production uses are obligation collection (`prf/obligations.rs`), no-op discharge (`prf/checker.rs:6522`), enum/storage (`checked-trees`, `published.rs` lowering to semantic facts). No function named to check or discharge these obligations outside the proof checker loop.

**UNDECIDED (read + inferred):** Confirmed no-ops at `6522` and measurement label `decided_elsewhere` (`5576`). Could not locate a closing check that rejects compilation when these obligations remain undischarged; lowering only records semantic facts (`published.rs:4323-4349`, `append_proof_obligation_semantic_facts`, not fully read). Whether the language requires rejection for those facts alone would decide REACHABLE vs CLOSED.

## A0279

Opened `prf/lib.rs:817-846` (`expression_contains_call_node`, read); read `dehoisted_initializer` (`695-712`, read).

Match arms recurse through Call, Binary, Unary, Cast, Borrow, Indexed, Member, Match; `_ => false` at `845` (read). Compared to `typed_trees::ExpressionNode` (`representations/typed-trees/src/values.rs:4199-4220`, read): kinds returning false without subtree scan include TypeExpression, SavedSubject, ArrayLiteral, Atomic, Boolean, Float, UndeterminedNaN, Integer, Name, Range, StructLiteral, String, ZeroValue (read).

Caller search: `rg 'expression_contains_call_node' omega-rust` → 18 matches in `prf/checker.rs` (4), `prf/lib.rs` (11, mostly recursion), `prf/obligations.rs` (3) (read).

Upward: `dehoisted_initializer` refuses dehoisting when the detector is true on the local initializer (`705-707`, read), affecting `dehoisted_operand` used in range folding (`prf/checker.rs:5355`, read). `obligations.rs` uses it for `arguments_are_call_free` (`571-573`, read) and transition return `call_free` (`1057-1070`, read), choosing guarded vs `Always` transition guards for bounded obligations.

**REACHABLE (read):** A transition return value or argument is a single expression node of a kind that falls through to false (for example an atomic wrapper) while still containing a call in a subexpression the walker does not enter. Obligation collection treats the value as call-free, keeps the transition’s when-guard for range narrowing, and the proof pass can accept a bounded return or argument that should fail without the guard refinement that call presence would force.

## A0280

Opened `md:10770-10779` (`indexed_application`, read); read `source_family` push/pop on `indexed_family_stack` (`10849-10853`, read).

When `indexed_family_stack.contains(&symbol)` (`10775-10778`, read), the function returns `Term::Sort(Sort::Type(Level::Constant(0)))` without error.

Caller search: `rg 'indexed_application\(' omega-rust/psi/compilation/proof` → definition `10770`, call `2530` during carrier elaboration for data indexed families (read).

Upward: `Elaborator::check` runs `retain_indexed_contract_theorems` then `check_signature_with_attribution` (`12386-12393`, read). Stub sort is a placeholder during recursive family construction.

**UNDECIDED (read):** Stub typing during stack membership is confirmed. Did not trace every elaboration path from a mid-construction application through final `check_signature_with_attribution` and contract consumers to see whether a program that should be rejected is accepted.

## A0281

Opened `md:4942-5305` (`retain_indexed_contract_theorems`, read), focus `5212-5215` (read).

For each ensures expression, `elaborate_proposition` yielding `None` executes `continue` (`5213-5214`, read), skipping `PreparedIndexedTheorem` append (`5275-5288`, read). Many earlier shape misses also `continue` without diagnostic in the same function (`4953-5195`, read).

Consumer (read): `validation/src/float_and_mathematical_contracts.rs:2311-2327` (`indexed_contract_is_proved`); used in `contract_entailment.rs:90362-90371` to remove ensures already kernel-proved, not to accept programs alone.

**UNDECIDED (read):** Silent drop of a theorem is confirmed. Whether a program that should be rejected passes because the ensure stays unproved but structural entailment still succeeds was not demonstrated.

## A0282

Opened `md:2185-2215` (`prepare_identity_values`, read).

After normalization, rows whose head is not a three-argument `Id` constant application hit `continue` (`2211-2214`, read) with no diagnostic.

Upward: called from `Elaborator::check` application loop (`12488-12504`, read). Identity values are read via `PreparedMathematicsView::identity_value` (`validation/src/float_and_mathematical_contracts.rs:3016-3049`, read).

**UNDECIDED (read):** Drop behavior matches digest. Did not trace a program whose identity evidence is required for rejection but omission lets compilation succeed.

## A0283

Opened `md:5402-5519` (`retain_source_evidence_aliases`, read).

Fixed loop `for _ in 0..128` (`5418`, read); `pending.retain` leaves unresolved names when no alias row is written (`5500-5502` keeps entry, `5515-5516` breaks on no progress). Function returns `Ok(())` (`5519`, read) without reporting leftover `pending`.

**UNDECIDED (read):** Cap and silent stop match digest. Did not show a program that should be rejected because an alias chain longer than 128 or unresolved after 128 is required for evidence/checking.

## A0284

Opened `md:1099-1110` (`scalar_carrier_of_type_reference`, read). Digest note “checked: code matches” for `_ => Some(ScalarCarrier::Integer)` including `Selector` (read).

`PrimitiveType::Selector` exists (`typed-trees/src/type_system.rs:4604`, read). `fixed_integer_type` in proof checker rejects Selector for integer certificates (`prf/checker.rs:2002-2007`, read). `scalar_operand_reference` (`md:4217-4236`, read) lists explicit primitives and omits Selector.

**UNDECIDED (read):** Selector is classified as Integer at `1109`. Did not trace a mathematical elaboration path where a Selector-typed proof operand is accepted as an integer operand although it should be rejected.

## What I did not read

- Full `contract_entailment.rs` and `proof_contracts_core.rs` beyond cited bands.
- `append_proof_obligation_semantic_facts` lowering body in `pipeline/05_typed-trees-to-checked-trees/src/published.rs` (only obligation match arms read).
- Proof-admission bounded denotation and kernel beyond area_11 digest references.
- Runtime corpus behavior; no build or compile was run (STREAM A constraint).

SLICE COMPLETE
