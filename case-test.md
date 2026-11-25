# case-test

## still_open

True

## size

large

## depends_on

[]

## evidence

Worktree worker-6 at 898d56a20796. The binary target/debug/omega.exe was built from 1f08b50ac2da. None of the commits between the two touch case tests. I read the HEAD source. Probe programs and logs are under C:/Development/Omega/.agents/worktree/worker-6/build/diag-case-content/. Each was run as `omega --check --target windows_x86_64 <path> < /dev/null`.

- base: the #5606 fixture (`requires outcome in Outcome::Good`, `ensures result in Outcome::Good`). Exit 0.
- or_entry and alt_pass (`requires (outcome in Outcome::Good) || flag`): exit 1, "not supported yet: a case test in an entry clause in machine `relay`" (from stage 06, places.rs:7163).
- eq_operand and op_pass (`requires flag == (outcome in Outcome::Good)`): exit 1, "not supported yet: contract record formal named whole in a clause while reading the signature of `relay`" (from stage 05, read/clause_terms.rs:693). A second error follows: "a call of a machine whose signature is refused above".
- or_ensures and res_pass (`pick(flag) -> Outcome ensures (result in Outcome::Good) || flag`, called from build_entry): exit 1, "the case the data requires clause tests is not established by structure here in machine `build_entry` (no proof found from the facts available here)" (from places.rs:8579, `Held::case`). The wording is misleading: the clause is a callee `ensures`, not a data `requires`.

The three sample families each have 18 placements, and every placement shares the same first stop:
- representation_agreement/field: "`u` is spelled like a builtin type but is not one" at main.omg:19:45 (`let same_result<u: core\Level, A: core\Type<u>>`). In a scratch copy with `u` renamed to `v`, the next stop is "named operation call has contradictory selection custody" at main.omg:35:21 (`ensures Encode::try_from(value) in Result::Ok;` in trait PairAgreement), from stage 03 typing_queries.rs:1489.
- resource_representation/field: "pattern binding `frame` conflicts with an existing name in the visible value scope" at main.omg:131:31 (`Result::Ok { value as frame }` inside `compose_and_observe(frame: ..)`). The sample breaks state_contracts.md: "A runtime parameter or local cannot reuse the name of a visible value parameter or local". In a scratch copy with the binding renamed, the next stops are generics and lifetime refusals: "static implementation family needs its general binder or region proof" (WriteFirst, :26), "trait with lifetime parameters" (BufferedEncoding, :18), "generic data with lifetimes, bounds, cleanup...", and "a static implementation argument must name an implementation without its own application" / "a closed static type operand has no retained data view" (:118, :127).
- buffered_representation_agreement/field: six refusals, none about case tests: "static implementation family needs its general binder or region proof" (Writer), "trait with lifetime parameters" (Encoding), "type record application whose reference regions are not retained" (through_field), "a static implementation argument must name an implementation without its own application" (read_frame), "Identity operand has no term at its written carrier" (observe::consume), and a cascade.

Code read:
- Stage 05 read/contracts.rs:687-707 already forms `Proposition::StructuralCaseMembership` inside a disjunction, through `disjunctive_clause`. Stage 05 does not refuse the alternative form.
- psi/semantics/src/substitution/clauses.rs:129-151 and 452-459: `ClauseBindings::case` returns `bool`, and `case_predicate` turns a case test into `Truth` or `Falsehood`. Its doc says "A case test is not a value: where the clause is used the case is known by structure or not at all". `connective` (828-867) returns the error of any undecided part.
- Stage 06 answers "which case" only by structure. At entry, places.rs:4328-4337 uses `entry_cases` and gives `Reached::Unknown` otherwise. At a call result, places.rs:3388-3404 uses `returned_cases`. Both tables are built by `stated_cases` (places.rs:421-468) from conjuncts only. Scalar leaves at the same two points get `Supply::Boundary { value }` instead: places.rs:4314-4321 from `initial_contents`, and places.rs:3344-3368 from `returned_contents`. Those values are named by `ValueOrigin::StorageContent` (lowered-program tables/values.rs:67-75), which stage 05 creates in lower/places_contents.rs:398-420.
- Dispatch edges already push facts through `payload_equations` / `DispatchBinding` (lib.rs:1356-1383, places.rs:4855-4895). Branch conditions are pushed at lib.rs:1328-1354.
- TASKS.md:310 and wiki/drafts/designs/proof_kernel_core_audit.md:1390-1422 describe this item. OWNER_QUESTIONS.md has nothing on it.

## spec

The deciding rules, at most one sentence each:
- wiki/spec/language/data_and_literals.md, "Case constraints": "The data clause uses ordinary well-formed contract propositions, including Boolean combinations". Also: "A case implicitly declares its same-named domain. `Type::Case` therefore denotes case membership in domain position."
- wiki/spec/language/dependent_values.md:152: "Contracts may state `requires value in Domain` and corresponding `ensures`."
- wiki/spec/language/domains.md, "Refinement and tests": "`x in A | B` states union membership", so membership is an ordinary proposition that combines with others.
- wiki/spec/terminal-psi/fact_validity.md, "Entry facts and crash coverage": "Entry hypotheses bind exact invocation formals or structural entry paths". This permits naming the entry case of a formal's path as the entry snapshot.
- wiki/spec/language/expressions.md (quoted in values.rs BranchCondition): "`||` may use its false-path facts". This guards a payload projection in `(result in Err) || P(result.value)`.
- For the resource_representation sample, wiki/spec/language/state_contracts.md:111: "A runtime parameter or local cannot reuse the name of a visible value parameter or local".

No owner question governs this item.

## cause

The checker has no name for "which case a sum holds" as a value. A case test is always decided to a constant: `ClauseBindings::case -> bool` (psi/semantics/src/substitution/clauses.rs:129-151). Stage 06 can decide it only from structure (a stored case literal, a dispatch edge, a membership-test edge) or from a clause that states the case outright as a conjunct (`stated_cases`, places.rs:421-468). Outside those, the answer is `Reached::Unknown` (places.rs:4334 at entry, places.rs:3401 at a call result). Every binding then refuses:
- `AtEntry::case`, places.rs:7141-7165: "a case test in an entry clause".
- `AtCall::case`, places.rs:7930-7945: "...not established by structure at this call".
- `Held::case`, places.rs:8569-8581: "the case the data requires clause tests is not established by structure here".

`connective` (clauses.rs:828-867) passes the undecided part's error up, so `(x in C) || flag` cannot be assumed at entry or after a call. A scalar leaf at the same two points is named by a `ValueOrigin::StorageContent` value and becomes an ordinary term. The sum's case has no such content kind.

The Boolean-operand form also fails earlier. The `flag == (x in C)` comparison in read_clause (read/contracts.rs:708-778) reads each side with `clause_term(.., ClauseUse::Scalar)`. clause_term's Binary arm (read/clause_terms.rs:808-877) then reads `x` as a scalar and refuses the record formal: "record formal named whole in a clause" (read/clause_terms.rs:693). No reader turns a case test in operand position into anything.

## code_locations

['source-rust/subsystems/compiler/psi/semantics/src/substitution/clauses.rs:129-151 (ClauseBindings::case -> bool, case_predicate)', 'source-rust/subsystems/compiler/psi/semantics/src/substitution/clauses.rs:452-459,613-615,828-867 (substitute_clause doc, case arm, connective)', "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:7141-7165 (AtEntry::case, 'a case test in an entry clause')", 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:7930-7945 (AtCall::case)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:8315-8340,8569-8581 (Held::case_at, Held::case)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:421-468 (stated_cases), 470-517 (initial_contents)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:4307-4337 (entry supply: Contents -> Boundary, Case -> entry_cases/Unknown)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:3344-3404 (call-result supply: Contents -> returned_contents Boundary, Case -> returned_cases/Unknown)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:3548-3610 (case_on_edge), 4445-4505 (read_equation), 4855-4895 (payload_equations)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs:1328-1383 (branch and dispatch edge facts)', 'source-rust/subsystems/compiler/source-to-psi/representations/lowered-program/tables/values.rs:51-76 (ValueOrigin::StorageContent)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/lower/places_contents.rs:330-423 (naming boundary leaves)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/contracts.rs:682-778 (case test and comparison in read_clause)', 'source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/clause_terms.rs:690-694,808-877 (record formal named whole; Binary operand arm)', "source-rust/subsystems/compiler/foundation/declaration-identity/src/builtin.rs:50-58 (bare `u`/`i` counted as builtin spelling; representation_agreement's first stop)", "source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs:1484-1492 (representation_agreement's second stop)", 'samples/cli/facts/resource_representation/*/main.omg:131 (sample reuses `frame`, which the fresh-name rule forbids)', 'TASKS.md:310', 'wiki/drafts/designs/proof_kernel_core_audit.md:1390-1422']

## repair_plan

One mechanism fixes every program with this cause: name the case a sum place holds at a storage boundary, as scalar leaves already are, and let a case test that structure cannot decide become a proposition over that name.

1. Lowered program (source-to-psi/representations/lowered-program/tables/values.rs:51-76). Add `ValueOrigin::StorageCase { place: ExactPlaceId, case: StructuralCaseId, after: Option<OperationId> }`. It is a Boolean logical name for "this sum place holds this case" at entry (`after: None`) or at the normal return of an exact call (`after: Some(call)`). Like `StorageContent`, it is not an executable operand.

2. Stage 05 (lower/places_contents.rs:330-423). Where the stage names `StorageContent` leaves, also name one `StorageCase` value for every case of each sum place a clause or operation demands. Use the same readability and boundary rules: `named_entry` for Parameter and readable BlockParameter roots, and one value per unique call writing `Content::Overwritten`. Keep place order so stage 06's sorted check still holds. Extend that check (`initial_contents`, places.rs:470-517) to fill `entry_cases` and `returned_cases` tables of values, alongside `entry` and `calls`.

3. Stage 06, reaching (places.rs). For `Asked::Case`, answer from these values where structure gives nothing:
   - At entry (4328-4337) and at a call result (3388-3404), return a new `Supply::CaseBoundary { values }`, or `Reached::NamedCase`, instead of `Reached::Unknown`.
   - A store, a dispatch edge or a membership edge still answers structurally first, so later writes keep their meaning.
   - Rekey `entry_cases`/`returned_cases` (`stated_cases`, 421-468) as facts over the named values instead of a separate `Lineage` table. A stated conjunct `x in C` is then the entry fact `v_C == true`, which payload-read permission reads through the same named value. This is one mechanism, not two (rule 6).

4. Substitution (psi/semantics/src/substitution/clauses.rs:129-151). Replace `case(..) -> bool` with `case(..) -> CaseAnswer { Holds(bool), Named(ValueId) }`. The default `case_predicate` maps `Named(v)` to `Proposition::Equal(ScalarTerm::value(v, Boolean), Boolean(true))`. Update every implementation, all of which #5606 listed:
   - places.rs: `AtEntry` (7141), `AtCall` (7930), `Held` (8569), and the others at 6980, 8253, 8684 and 8747;
   - entry_inputs.rs: 950 and 1073, plus entry_inputs/recursive.rs:209;
   - formation.rs:912, laws.rs:660, lib.rs:2169 and 2247, places/cleanup.rs:387, places/contents.rs:555, result_summaries.rs:277;
   - substitution/clauses.rs:417 and 959.

   An implementation that has no boundary name keeps refusing. Never answer true by default.

5. Facts that tie names to control flow (lib.rs, beside `payload_equations` at 1356-1383 and the branch block at 1328-1354):
   - On each `Terminator::StructuralCase` edge whose sum reaches a boundary name, push `v_selected == true` and `v_other == false` for that place's other named cases (`Justification::DispatchBinding { edge }`).
   - For a `StructuralCaseMembership` operation, push `result == v_case` through `read_equation` (places.rs:4445) under `OperationSemantics`. The existing BranchCondition fact on the conditional then carries it.
   - At each boundary, push "exactly one of the place's case values holds" (a disjunction and pairwise exclusions; sums are small), justified as `FormalType` at entry and as the call's guarantee after a call.

6. Stage 05, Boolean operand (read/contracts.rs, in `read_clause` before the comparison path at 708). When `==` or `!=` has an operand that is a case test (Binary `CaseMembership`, or `== S::C` with `named_case`), read both sides as propositions with `self.clause`. Form the biconditional `Conjunction[Implication(L,R), Implication(R,L)]`, or for `!=` the same with `R` negated as `Implication(R, Falsehood)`. This keeps the published Psi clause vocabulary unchanged, with no new ScalarTerm. In clause_terms.rs's Binary arm (808), refuse a case-test operand by name ("case test as a scalar value outside a Boolean comparison") instead of falling through to "record formal named whole in a clause".

7. Delete the superseded refusals ("a case test in an entry clause" and the two "not established by structure" texts) where the named path replaces them. Reword `Held`'s text so it names a contract clause, not only a data requires. Update:
   - TASKS.md:310: delete the case-test clause from the Step 5 bullet;
   - the audit "Step 5 as built" paragraph (proof_kernel_core_audit.md:1412-1422);
   - the stage 05 README paragraph #5606 added;
   - the comment at substitution/clauses.rs:452-459.

The three sample families will still not compile after this; separate items stop them earlier:
- (a) representation_agreement, all 18 placements: `BuiltinTypeAtom::is_builtin_type_spelling` (foundation/declaration-identity/src/builtin.rs:51-53) treats a bare `u` or `i` as an integer family, because `"".bytes().all(is_ascii_digit)` is vacuously true. Stage 02 therefore never binds the level parameter `u`, and stage 03 type_reference.rs:4179 refuses it. Fix: require at least one digit. Next stop: stage 03 "named operation call has contradictory selection custody" (typing_queries.rs:1489) on `Encode::try_from(value) in Result::Ok`, where `Encode` is bound through the trait alias `ValueConversion = TryFrom<..>`.
- (b) resource_representation, all 18 placements: the sample breaks the fresh-name rule at main.omg:131 (`value as frame` while formal `frame` is visible). The sample is wrong per state_contracts.md and needs a content edit (no file count change). Then it hits generic and lifetime refusals: static implementation family binder/region proof, trait with lifetime parameters, generic data with lifetimes, static implementation argument with its own application.
- (c) buffered_representation_agreement: the same generic and lifetime refusals, plus "type record application whose reference regions are not retained" and "Identity operand has no term at its written carrier" (Step 5's other kept refusal).

None of (a)-(c) depends on this item, and this item depends on none of them.

## twin_programs

Pair A, a case test beside an alternative (scratch: build/diag-case-content/alt_pass, alt_fail). Positive:
```omega
data Outcome { case Good; case Bad; }
machine take(outcome: Outcome) requires outcome in Outcome::Good { }
machine relay(outcome: Outcome, flag: bool) requires (outcome in Outcome::Good) || flag requires !flag { take(outcome); }
machine build_entry() { relay(Outcome::Good, false); }
```
The negative twin differs only in the call argument: `relay(Outcome::Bad, false);`. It must be refused with exactly one CallRequirement, at build_entry's call of `relay`, detail "the goal is false". Today both programs are refused with "not supported yet: a case test in an entry clause in machine `relay`".

Pair B, a case test as a Boolean operand (scratch: op_pass, op_fail). Positive:
```omega
data Outcome { case Good; case Bad; }
machine take(outcome: Outcome) requires outcome in Outcome::Good { }
machine relay(outcome: Outcome, flag: bool) requires flag == (outcome in Outcome::Good) requires flag { take(outcome); }
machine build_entry() { relay(Outcome::Good, true); }
```
The negative twin is `relay(Outcome::Bad, true);`: one CallRequirement at build_entry, detail "the goal is false". Today both programs are refused with "not supported yet: contract record formal named whole in a clause while reading the signature of `relay`".

Each positive also needs the new entry name: `take(outcome)` inside `relay` is provable only from the named entry case and the other requires.

## tests

Add the pairs as fixtures beside stage 06's clause tests, in tests/fixtures/clause_terms/{pass,fail}/ (case_alternative, case_alternative_false, case_operand, case_operand_false). Add tests in src/tests/clause_terms.rs in the style of #5606, derived from the passing programs by one edit each:
- In alt_pass, delete `requires !flag`. The call to `take` must be left unproved as a CallRequirement, and must not be refused as unsupported.
- In res_pass, the caller uses a callee's `ensures (result in Outcome::Good) || flag` with `pick(false)`, then `take(picked)`: accepted. With `pick(true)`: CallRequirement unproved.
- Body dispatch: `relay` requires `(outcome in Outcome::Good) || flag` and does `transition outcome { Outcome::Good -> take(outcome), Outcome::Bad -> .. }`. Accepted. This tests the dispatch-edge fact over the named value.
- Write over the formal (`let mutable held = outcome; held = Outcome::Bad; take(held)` under `requires (outcome in Good) || flag` and `requires !flag`): "the goal is false". This shows the name belongs to the entry snapshot, not the current storage.
- Replace `a_case_test_beside_an_alternative_states_nothing_and_is_refused`; it pins the refusal this change removes.

Stage 05 unit test: `flag == (x in S::C)` reads as the biconditional proposition, and `!=` as the negated form.

Run:
- `mbx test -p prepared-typed-trees-to-lowered-program -p lowered-program-to-checked-program --lib < /dev/null`;
- `mbx test -p <psi semantics crate>` for the substitution change, with `cargo xtask check-boundary`;
- `mbx clippy` on those three crates with `--all-targets -- -D warnings`;
- `mbx test -p omega --test kernel_judged_laws`;
- one end-to-end sample check: `mbx xtask sample-gate --filter samples/cli/facts/source_indexed`, the storage-analysis area #5606 used.

Search all `ClauseBindings` implementors, including under psi/semantics, since the trait signature changes.

## risks

- Soundness depends on the name being the boundary snapshot only. Any write to the sum place or to a place above it must end the named answer, so that reaching answers structurally again. This is the same rule as `StorageContent`, and the write-over-formal derived test checks it.
- Naming per-case Booleans needs the "exactly one case holds" fact at each boundary. Leaving it out loses completeness; it does not break soundness. Never derive it anywhere except at the boundary.
- A payload projection under an undecided named case, such as `P(result.value.value)` in `(result in Err) || P(..)`, must stay guarded by the case test in the proposition. The implementer must check that the payload leaf is substituted only inside a disjunct or implication the case test guards, and that a payload read in a body still needs established case knowledge (structural, or the named value proved true).
- The trait signature change reaches every `ClauseBindings` implementor, including the psi/semantics ones #5606 did not audit.
- Block-parameter arrivals and join inference (joins.rs:220, 1305) also handle `StructuralCaseMembership`. A block formal's case may need the same naming later. This change does not do that, and join inference must keep refusing rather than guess.
- No sample's outcome changes from this item alone. All three families stop earlier: on the bare-`u` spelling bug and a stage 03 selection-custody refusal, on the sample's own name reuse, and on generics and lifetime refusals.

