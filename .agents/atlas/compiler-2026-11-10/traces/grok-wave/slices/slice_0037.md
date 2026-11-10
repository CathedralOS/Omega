slice_0037

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0277 | `omega-rust/psi/compilation/proof/src/checker.rs:5427-5473` | REACHABLE | Cross-machine field lookup can mis-label unsignedness; Uncovered assignment path accepts when inferred interval fits target without re-checking the value. |
| A0278 | `omega-rust/psi/compilation/proof/src/checker.rs:6522` | CLOSED | Proof pass intentionally skips these tags; validation already ran, and value-bearing constrained checks use other obligation kinds. |
| A0279 | `omega-rust/psi/compilation/proof/src/lib.rs:845` | REACHABLE | `_ => false` skips `ArrayLiteral`/`StructLiteral` (and others), so calls nested there are treated as call-free for dehoist and related guards. |
| A0280 | `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs:10775-10779` | CLOSED | Recursive elaboration placeholder while the family is on `indexed_family_stack`; full family still must pass `check_signature` before insert. |
| A0281 | `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs:4953-5195` | CLOSED | `continue` skips retaining a theorem; the ensure expression stays in normal contract entailment instead of being auto-marked proved. |
| A0282 | `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs:2211-2215` | UNDECIDED | Dropped rows are omitted from `identity_values`; I did not trace whether downstream admission can pass without them. |
| A0283 | `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs:5418-5518` | UNDECIDED | After 128 passes `Ok(())` leaves pending aliases unrecorded; I did not trace validation paths that require those aliases. |
| A0284 | `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs:1109` | REACHABLE | `PrimitiveType::Selector` falls through `_` to `ScalarCarrier::Integer`, unlike explicit non-integer handling elsewhere in validation. |

### A0277

1. Read in full: `operand_declared_primitive`, `operand_is_unsigned`, `witness_operand` closure in `guarded_integer_range_for_assignment_with_context`, `guard_narrowed_operand_range`, and `check_bounded_assignment` (`omega-rust/psi/compilation/proof/src/checker.rs:332-418`, `5190-5473`).
2. Search: `rg '\boperand_declared_primitive\b' /agent/repos/omega/omega-rust` → **2** hits (definition + use in `operand_is_unsigned`, both `checker.rs`); `rg '\boperand_is_unsigned\b' …` → **3** hits (`checker.rs` at `5221`, `5335`, `5427`); `rg '\bguarded_integer_range_for_assignment\b' …` → **6** hits (all `checker.rs`, including `check_bounded_assignment` at `374`).
3. Callers read: `check_bounded_assignment` (`332-418`); caller `check_proof_plan_inner` (`6506-6520`); caller `check_proof_plan_with_decisions` (`6468-6472`); pipeline `prepare_typed_program` (`omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checking.rs:422`). Did not read `proved_assignment_integer_range_with_context` callers beyond grep.
4. Input to accepting branch: `CertificateVerdict::Uncovered` on a `BoundedAssignmentObligation`; `operand_declared_primitive` walks **all** `program.machines()` (`5459-5473`) and returns the first matching `self.<member>` field type, ignoring `obligation.machine_symbol`; if that makes `operand_is_unsigned` true, witness narrowing sets `minimum` to `0` (`5220-5224`, `5334-5337`). `interval_fits` only tests whether the inferred interval lies inside the declared target bounds (`377-380`) and does **not** require the assignment value to lie in the inferred interval when `interval_fits` is true (`381-390` skipped).
5. **Verdict: REACHABLE.** Smallest rejection scenario (plain words): two machines share attached data with the same field name but different signedness; in the later machine an assignment needs certificate fallback, the wrong machine’s unsigned field makes `operand_is_unsigned` true, the inferred interval fits the declared signed target type on paper, and a negative assigned value is accepted because `occurrence_integer_expression_fits_range` is not consulted.
6. Unsure: whether `occurrence_integer_expression_fits_range` (`validation/src/arithmetic_domains.rs:10555`) would still fail such programs in practice without running one.

### A0278

1. Read in full: `check_proof_plan_inner` (`6475-6549`), `ProofPlanMeasurements::record_obligation` (`5606-5618`), and obligation collection for `BoundedValue` / `GuardedTransition` (`obligations.rs:54-99`, `156-167`).
2. Search: `rg '\bcheck_proof_plan_with_decisions\b' omega-rust` → **4** hits (`checker.rs`, `lib.rs`, `checking.rs`); `rg 'ProofObligation::BoundedValue' omega-rust` → production uses in `obligations.rs`, no-op in `checker.rs:6522`, fact export in `published.rs:4323`.
3. Callers read: `prepare_typed_program` (`checking.rs:399-454`) runs `validation::prepare_specialized_program` **before** proof (`413-419`), then `check_proof_plan_with_decisions` (`422`).
4. Input to accepting branch: any `ProofObligation::BoundedValue` or `GuardedTransition` in the plan; `decided_here` is false (`6489-6492`), literal arm is empty (`6522`), no diagnostic.
5. **Verdict: CLOSED.** `BoundedValue` records constrained **slots** (owned data, parameters, return types) at `obligations.rs:54-99`; actual values are pursued via `BoundedInitializer`, `BoundedAssignment`, `BoundedCallArgument`, etc., which `check_proof_plan_inner` does discharge. `GuardedTransition` is a recorded fact (`published.rs:4343-4349`); guard expressions are validated in `validation` (e.g. `program_validation.rs:3214`, transition argument collection at `obligations.rs:570-586`). The no-op does not bypass `prepare_typed_program`’s validation error path (`430-437`).
6. Unsure: none material for this entry.

### A0279

1. Read in full: `expression_contains_call_node` (`lib.rs:817-847`), `dehoisted_initializer` (`695-713`), `dehoisted_operand` (`667-685`).
2. Search: `rg '\bexpression_contains_call_node\b' omega-rust` → **18** hits across `lib.rs` (11), `checker.rs` (4), `obligations.rs` (3); production call sites include `dehoisted_initializer` (`706`), `assignment_is_overwritten_before_consumption` (`457-496`), transition `arguments_are_call_free` (`573`).
3. Callers read: `dehoisted_operand` → binary operand narrowing (`obligations.rs:429-430`, `checker.rs:5355`); `assignment_is_overwritten_before_consumption` (`checker.rs:456-458`).
4. Input to accepting branch: any expression whose outermost kind is not explicitly walked; `ExpressionNode` includes `ArrayLiteral`, `StructLiteral`, `Atomic`, `SavedSubject`, `Name`, `Range`, etc. (`typed-trees/src/values.rs:4199-4220`), all hit `_ => false` at `lib.rs:845` without subtree scan.
5. **Verdict: REACHABLE.** Smallest rejection scenario: a state local’s initializer is an array or struct literal containing a call; `expression_contains_call_node` returns false, `dehoisted_initializer` treats the initializer as safe and dehoists through it, and downstream proof or overwrite logic that assumes “call-free” incorrectly proceeds.
6. Unsure: whether typed validation already rejects call-bearing literals in initializers before proof runs.

### A0280

1. Read in full: `indexed_application` (`10770-10799`), `source_family` stack push/pop (`10849-10853`), and caller context at `2530` (grep only, not full enclosing function).
2. Search: `rg '\bindexed_application\b' omega-rust/psi/compilation/proof` → **2** hits (definition + one call site in `mathematical_declarations.rs`).
3. Callers: `source_family` pushes symbol before `build_source_family` (`10849-10857`); during build, internal indexed references hit the stack guard (`10775-10778`).
4. Input: applying a family symbol to indices while that symbol is already on `indexed_family_stack` during its own construction.
5. **Verdict: CLOSED.** Returns `Sort(Type 0)` as an elaboration placeholder, not as a finalized typing judgment for complete programs; `source_family` still requires `proof_admission::check_signature` on the generated description (`10859-10864`) before `indexed_families.insert`.
6. Unsure: none.

### A0281

1. Read in full: `retain_indexed_contract_theorems` (`4942-5306`), especially shape-miss `continue` paths (`4953-5194`, `5213-5214`).
2. Search: `rg 'retain_indexed_contract_theorems' omega-rust` → **2** hits (`definition`, `check()` at `12390`).
3. Consumer read: `indexed_contract_is_proved` (`float_and_mathematical_contracts.rs:2311-2327`); `contract_entailment.rs:90361-90371` retains ensures expressions **unless** `indexed_contract_is_proved` is true.
4. Input: machines whose telescope/mapping/contracts do not match the expected shape; elaboration `continue`s without error.
5. **Verdict: CLOSED.** Skipping retention does not remove the ensure from checking: unproved expressions stay in the ensures bucket (`90369-90370`) and must still pass ordinary entailment; auto-removal only happens when a theorem was actually retained and matches (`90362-90367`).
6. Unsure: whether any program path treats “no indexed theorem” as vacuous proof without reading further entailment branches.

### A0282

1. Read in full: `prepare_identity_values` (`2185-2269`), focusing on `continue` at `2211-2214`.
2. Search: `rg 'prepare_identity_values' omega-rust` → **2** hits (`definition`, call in `check()` at `12488-12493`).
3. Caller read: `check()` loop over applications (`12434-12504`); abnormal rows are skipped, not errored.
4. Input: identity emission rows whose normal form is not a 3-argument `Id` constant application.
5. **Verdict: UNDECIDED.** I read the drop site and that `identity_values` is stored on `PreparedApplication`, but I did not read consumers of `PreparedApplication.identity_values` or admission paths that require every emission row.
6. Unsure: whether a malformed row’s absence lets a bad application pass kernel checking or checked-tree publication.

### A0283

1. Read in full: `retain_source_evidence_aliases` (`5402-5519`).
2. Search: `rg 'retain_source_evidence_aliases' omega-rust` → **2** hits (`definition`, `check()` at `12506`).
3. Caller read: `check()` always calls it and propagates only `Err` (`12506-12508`); 128-iteration cap (`5418-5517`) breaks on no progress without error.
4. Input: erased immutable locals whose evidence alias chain does not resolve within 128 passes; they remain absent from `prepared.source_evidence`.
5. **Verdict: UNDECIDED.** I read that the function returns `Ok(())` with pending work left, but I did not trace programs that depend on those aliases through `bind_source_evidence_types` (`5310-5397`) or validation `is_source_evidence_carrier`.
6. Unsure: whether missing alias rows weaken proofs or cause silent acceptance vs later hard errors.

### A0284

1. Read in full: `scalar_carrier_of_type_reference` (`1099-1129`); entry line `1109` matches digest.
2. Search: internal uses via grep (`scalar_carrier_of_type_reference` in `mathematical_declarations.rs`); cross-read `validation/src/float_and_mathematical_contracts.rs:774` where `PrimitiveType::Selector` returns `None` for a related predicate.
3. Input: any primitive not explicitly matched, including `PrimitiveType::Selector` (exists in typed trees; see `declarations.rs:2498` grep hit).
4. Accepting branch: `_ => Some(ScalarCarrier::Integer)` at `1109`.
5. **Verdict: REACHABLE.** Smallest rejection scenario: a mathematical source subject or evidence path involving a `Selector`-typed reference is classified as integer scalar for elaboration, while other validation layers treat `Selector` as non-integer-bounded (`values_scalar.rs:9783` pattern), potentially admitting integer-style math where selector values should not be discharged that way.
6. Unsure: whether authored mathematical declarations can surface `Selector` through `primitive_type_reference` in reachable programs.

## What I did not read

- Full bodies of `build_source_family`, `elaborate_type_reference` call site at `mathematical_declarations.rs:2530`, and most `validation::prepare_specialized_program` internals.
- All consumers of `PreparedApplication.identity_values` and full alias-dependent validation beyond `bind_source_evidence_types` header.
- Test-only modules in `checker.rs` beyond grep-listed call sites.
- `proved_assignment_integer_range_with_context` production callers outside `checker.rs`.

SLICE COMPLETE
