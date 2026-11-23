# Record: unit_043 gap_024 (area 07-psi-semantics, crate proof-admission, `proofs/bounded/`)

Second pass over ranges the first reader of unit 043 skipped. Root `B/` = `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/`.
Every range in `units/gap_024.tsv` was read in full. All of it is test code (proof.rs 4123-5117 sits inside
`#[cfg(test)] mod tests` that opens at `proof.rs:3627-3628`, grepped). Nothing here is production code, so items 4, 6-9 are
mostly about what the tests pin, not run-time decisions. Production behaviour is described only as far as a test asserts it.

## B/proof.rs lines 4123-5117 (995 lines, read in full; tail of `mod tests`)

1. Purpose: unit tests of the certificate checker (`check_certificate` `proof.rs:163`, `accept_certificate` `:173`,
   `accept_certificate_with_machine_parameters` `:190`, grepped) for integer-order, affine-bound, cast-bound, logical and
   equality rules, plus three deep-nesting stack tests. The range begins mid-test (the end of a transitivity test at 4123-4134).
2. Shape (test fns): `integer_order_substitution_transports_either_endpoint_in_either_equality_orientation` (:4137),
   `..._rejects_every_shape_and_endpoint_mismatch` (:4209), `integer_affine_bound_checks_root_proof_normalization_and_exact_custody` (:4329),
   `integer_cast_bound_checks_complete_definition_word_and_exact_custody` (:4434), `implication_certificate_is_checked_structurally` (:4545),
   `excluded_middle_is_expressible_and_a_cited_premise_stays_recorded` (:4570), `accepted_trust_closure_binds_the_exact_cited_premise` (:4599),
   `nested_scopes_do_not_export_discharged_same_index_premises` (:4625), `semantic_equalities_compose_only_through_the_same_middle_term` (:4646),
   `canonical_content_equalities_compose_through_either_orientation` (:4676), fixture `correlated_division_fixture` (:4733) with two tests (:4809, :4844),
   helpers `integer_context` (:4876), `values_context` (:4884), `integer_value` (:4893), `signed_literal` (:4900), `recorded_premise` (:4904),
   and three deep tests (:4912, :4953, :5028).
3. Inputs/outputs: build `ProofNode { conclusion, rule }` trees and call the checker; assert on `AcceptedProofRule` lists, `accepted.semantic_axioms`/
   `assumptions` (recorded premises with index), `MathematicalCoreDecision::Judged(receipt)`, or the exact `ProofError` variant.
   Helpers `assumption_proof`/`semantic_axiom_proof` are `#[cfg(test)] pub(crate)` at `proof.rs:3311,3319` (grepped).
4. How it decides (as pinned by tests):
   - IntegerOrderSubstitution is keyed on the `endpoint` index (0 or 1) and on the equality's two sides in either orientation (:4143-4205). Errors pinned:
     `UnknownIntegerOrderEndpoint(2)` (:4246), `IntegerOrderUnchangedEndpointMismatch` (:4261), `IntegerOrderSubstitutionMismatch` (:4276),
     `RulePremiseMismatch("integer order substitution relation"/"... equality")` (:4289,4299), `RuleConclusionMismatch` (:4309), `UnknownSemanticAxiom(1)` (:4324).
   - IntegerAffineBound: root bound is an assumption proof; witness lists `definition_axioms` and `literal_axioms` by semantic-axiom index; recorded axioms are
     exactly those cited (indices 0 and 1, :4373-4379). Out-of-range index gives `IntegerAffineWitnessError::UnknownSemanticAxiom(2)` (:4388);
     wrong conclusion gives `ConclusionMismatch` (:4401); a `Truth` root gives `TruthRootWithoutTotalImage` (:4427).
   - IntegerCastBound: one cast or a contiguous cast word (i16->u16->i8) maps the root bound; axioms cited are recorded (:4470-4513);
     `UnknownSemanticAxiom(1)` (:4522), `ConclusionTargetMismatch` for a reversed `<=` (:4538).
   - Assumption rule: keyed on the exact scoped premise at that index. A different premise at the same index gives `AssumptionConclusionMismatch(0)` (:4620);
     no premise gives `UnknownAssumption(0)` (:4584). Discharged same-index premises in sibling `ImplicationIntroduction` scopes are not exported (:4625-4643).
   - EqualityTransitivity requires the same middle term (:4646-4673); also holds for `ContentConservation` in either canonical orientation (:4676-4731).
   - IntegerCorrelatedForbiddenRoots: accepted only when the root is in the machine-parameter set; with set `{999}` the error is
     `RootNotSignedNative` (:4846-4858); a proof whose conclusion is swapped to `Truth` gives `ConclusionMismatch` (:4860-4873).
     Accepted run records semantic axioms 0,1,2 and assumptions 0,1 (:4821-4840).
5. Cases by shape: the correlated fixture is one shape only, signed i8, dividend `root * -2`, divisor `root*2 + 1`, requirement `-1 <= root <= 0`,
   `definition_axiom_count: 3`, `lower_bound_axiom: 3`, `upper_bound_axiom: 4` (:4733-4806). The cast test covers 1-step and 2-step words only.
6. Same job elsewhere: stack-depth admission tests resemble each other three times in this range (:4912, :4953, :5028), each wrapping `std::thread::spawn` and
   asserting `bounded_verification_entries()` increments by `(1, valid as usize)` (:4946, 5021, 5107). `bounded_verification_entries` has 16 matches in
   2 files (Grep count over `source-rust`: `bounded_observations.rs` 1, `proof.rs` 15).
7. Unused/one-caller: only tests and test helpers here; helper fns (`integer_context`, `values_context`, `integer_value`, `signed_literal`, `recorded_premise`)
   are private to the test module and defined at :4876-4909, used in the range.
8. Stated limits: none seen as TODO text. Test comment :4505-4510 style notes belong to indexed tests below.
9. Fail-open risks: none seen (test code). One test pins that a `Truth` leaf in a certificate leaf position is rejected by conclusion check (:4860).
   Three deep tests assert only the cases listed: depth 8192 logical (:4916), 8192 scalar `BooleanNot` chain and depth 64 (:4990), 128/8192 `ExactIntegerAdd` chain (:5052).
   In the scalar test the kernel's own term-sharing budget is measured and `consumed-1` must fail with `CoreError::StepCeiling` (:4982-4989).
   Receipt fields pinned: logical `(context_depth 1, declarations 0, assumption_closure 0)` (:4940-4942); open-integer `(declarations 4, assumption_closure 4, context_depth 1)` (:5091-5098),
   `arena_slots > depth` (:5099).
   Test formatting note: the first deep test's body is not indented under `spawn` (:4914-4949), and it prints with `eprintln!` (:4931,4947).

## B/tests/exact_multiply_result_bound.rs (688 lines, read in full)

1. Purpose: proves, through the public `verify_obligation` route (`:224`), that a bound on the result of an exact integer multiply comes from three existing rules:
   `IntegerAffineBound` (corner products), `ValueEqualityTransport`, then `IntegerOrderSubstitution` with a reflexive equality (header `:1-14`).
2. Shape: helpers `integer` `:38`, `value` `:42`, `math_value` `:49`, `intervals` `:78`, `multiply` `:91`, `result_equation` `:97`, `operand_bounds` `:117`,
   `product_term_bound` `:135`, `onto_value` `:166`, `result_bound` `:202`, `verdict` `:218`, `assert_proved` `:246`, `assert_refused` `:260`, `overflow_refusal` `:274`.
   Six tests: `every_sign_combination_gets_the_tight_bound_and_nothing_tighter` `:284`, `a_missing_operand_bound_is_refused_or_read_as_the_carrier_bound` `:340`,
   `bounds_about_another_value_are_refused` `:399`, `an_operand_needs_one_bound_on_each_side` `:480`, `an_inverted_interval_proves_only_what_its_contradictory_hypotheses_entail` `:505`,
   `a_corner_product_beyond_the_kernels_integers_is_refused` `:540`, `a_bound_beyond_the_carrier_leaves_the_overflow_obligation_unproved` `:617`.
3. Inputs/outputs: builds certificates from `ProofNode`s; `verdict` wraps them in `CertificateEnvelope` with `ProofSystemMarker::CURRENT` and `ObligationClass::Derivable`
   (:224-237) and returns `acceptance.mathematical_core` or an `EvidenceError`.
4. How it decides (pinned): result bound keyed on four operand-bound parts in a fixed slot order (goal side first, then other) (:117-123); `Truth` in a slot means the
   carrier endpoint (:363-367). Test oracle enumerates the whole integer box rather than corners (:285-297). A bound on another value ID, a result equation for other operands,
   or an add equation is refused (:407-476).
5. Cases by shape: product of exactly four parts (three parts refused, :349-361); three sign shapes `(-7,-2) (-3,4) (2,5)` (:290); endpoints are a sign and a 128-bit magnitude,
   corner product is a checked multiply, last fit is `2^128 - 2^64` (:541-564); overflow gives `DirectMultiplyBoundOverflow` (:277-279,585-587).
6. Same job elsewhere: the multiply-image rule is `map_direct_multiply_bound` (header :8), in `integer_affine.rs` (not read in this pass; suspected). Not grepped.
7. Unused code: none (tests).
8. Stated limits: `:505-510` "The kernel does not look for that" (inverted interval); `:617-620` overflow obligation `left * right <= 255` stays unproved.
9. Fail-open risks: pinned, not new. An inverted interval (`5 <= left <= 2`) is accepted for the corner extreme 5*10 (`:515-521`); the test comment says it is an implication
   from unsatisfiable hypotheses, so no false fact about a reachable state follows. Classification: stated (`:505-510`). Missing lower bound (slots 1 and 3) still proves 50 (`:379-383`), stated at `:363-367`.

## B/tests/indexed_derivation.rs (991 lines, read in full)

1. Purpose: demonstration that the derived indexed-family scheme (`indexed_scheme`) can express an implicational natural-deduction family `Deriv <Gamma, phi>` whose index is a
   judgment (context, conclusion), and that the kernel checks it (header `:1-48`).
2. Shape: constants `CTX 5`, `FRM 6`, `CONS 7`, `IMP 8` (:90-95); description builders `carrier` `:156`, `branching` `:200`, `out_index` `:241`, `next_index_with` `:322`,
   `next_index` `:417`, `derivation_family` `:429`, `derivation_declarations` `:442`, `derivation_signature` `:465`, `constructor_context` `:485`, `eliminator_context` `:555`.
   Two tests: `induction_with_a_dependent_motive_proves_the_conclusion_identity` `:590`, `derivations_reject_wrong_judgments_and_malformed_descriptions` `:733`.
3. Inputs/outputs: uses `proof_admission` kernel API (`check_type`, `convertible`, `check_signature`, `IndexedFamily::indexed_w/sup/ind`) and `crate::common::*` helpers (`budget`, `measure_budget` at `common/mod.rs:19,23`).
4. How it decides (pinned): wrong conclusion, wrong context, swapped premises, assm at unextended context all give `CoreError::TypeMismatch` (:786-837); a producer `next` that requires `<Gamma, psi>` instead of `<Gamma, phi>`
   still forms (`:863-870`) but derivations fail with `argument_error_pair_pattern!()` (:917); an ill-typed `next` is a type mismatch (:935); a strict-sorted children function gives
   `StrictCaseMotiveCodomain` or `TypeMismatch` (:958-961); eliminator at motive level 1 against `Type 0` mismatches (:981-990).
5. Cases by shape: one fixed three-rule logic (assm, impI, impE); `Ctx`, `Frm`, `consF`, `impF` are assumptions (header :43-47).
6. Same job elsewhere: same scheme and test skeleton as `indexed_mutual.rs`, `indexed_nested.rs` and `indexed_levels.rs` (this pass); helper `project` `:73` and `tag_selected_children` (from `common`) repeated.
7. Unused code: none (tests).
8. Stated limits: `:60-63` comments the step budget (measured ~3.7k and ~25.1k steps); the exact pinned number is `2_958` (:729). The stale comment text at 60-63 mentions different figures from the pinned
   value (test-local note; the pinned figure is for the first test only, inferred).
9. Fail-open risks: none seen (negative tests assert errors).

## B/tests/indexed_levels.rs (513 lines, read in full)

1. Purpose: checks level (universe) instantiation of the scheme declarations `IndexedAt`, `IW`, `iwPack`, `isup`, `iindW` through a level-polymorphic length-indexed vector, `vecOf`/`consVec`, at level arity 1 (header :1-37).
2. Shape: `VEC_OF = 8` (:60), `vec_at` `:63`, `payload` `:91`, `carrier` `:111`, `branching` `:124`, `out_index` `:138`, `next_index` `:178`, `family_at` `:229`, `vec_of_declaration` `:250`,
   `cons_vec_declaration` `:276`, `level_declarations` `:335`, `level_signature` `:342`. Tests: `the_polymorphic_family_declaration_checks_once_and_instantiates_at_each_level` `:348`,
   `level_instantiation_rejects_the_wrong_universe` `:432`.
3. Inputs/outputs: `check_signature` over 10 declarations (:352-356); `infer_sort`, `infer_type`, `convertible`.
4. How it decides (pinned): `vecOf[0] Two n` has sort `Type 0`; `vecOf[1] (Type 0) n` sort `Type 1` (:372,378); decl wrapper converts to the direct `IndexedFamily` spine (:388-415). Errors:
   `ArgumentTypeMismatch` for wrong universe (:448), `DeclarationArityMismatch {expected 1, supplied 0}` (:453-459) and `{declaration 4, expected 4, supplied 3}` (:505-511), `UnboundLevelParameter {index 0, arity 0}` (:467-470).
5. Cases by shape: element levels 0 and 1 only (:370-376); nil tag stores `E` as a dummy "without cumulativity" (:26-29).
6. Same job elsewhere: see indexed_derivation item 6. Local `type_sort` (:49) shadows a helper likely also in `common` (suspected; `common/mod.rs` was read in unit 043 but not re-grepped).
7. Unused code: none (tests).
8. Stated limits: none beyond the cumulativity note at :26-29 ("without cumulativity a fixed `Two` nil payload could never check").
9. Fail-open risks: none seen.

## B/tests/indexed_mutual.rs (544 lines, read in full)

1. Purpose: mutual `Tree`/`Forest` recursion as one indexed family with index `Two` (zero = Tree, one = Forest); negative tests that sort mismatches and malformed descriptions are rejected (header :1-46).
2. Shape: `ELEM = 5` (:60), `uniform_payload` `:78`, `carrier` `:85`, `branching` `:95`, `out_index` `:116`, `branching_at` `:126`, `next_index` `:144`, `mutual_family` `:212`,
   `mutual_declarations` `:225`, `tree_at`/`forest_at` `:237,243`, `node_label` `:250`, `fcons_label` `:259`, `indexed_motive` `:268`, `constructor_context` `:291`, `eliminator_prefix` `:305`.
   One test: `mutual_constructions_reject_wrong_sorts_and_malformed_descriptions` `:318`.
3. Inputs/outputs: as indexed_derivation.
4. How it decides (pinned): node never lands at Forest (:339), forest constructor never at Tree (:360); child of node must be a Forest (:377); `fcons` child function returning Tree everywhere fails (:390),
   swapped branches fail (:410); a flipped-sorts `next` yields `argument_error_pair_pattern!()` (:496); strict branching `StrictCaseMotiveCodomain|TypeMismatch` (:509-512); level-1 eliminator mismatch (:543).
5. Cases by shape: payload uniform `Sigma(e : Elem). Two` because a dependent `Payload tag` cannot feed the second `caseTwo` (comment :28-32, :74-77).
6. Same job elsewhere: same skeleton as the other three indexed tests; `branching_at` (:126) is similar to `indexed_derivation.rs:222` (`branching_at`), different payload.
7. Unused code: none (tests).
8. Stated limits: none beyond the uniform-payload note (:28-32).
9. Fail-open risks: none seen.

## B/tests/indexed_nested.rs (418 lines, read in full)

1. Purpose: a level-indexed rose tree whose `rnode` payload carries the previously demonstrated vector family (`Vec k`), showing strictly positive nesting through the payload (header :1-52).
2. Shape: `family_budget()` = `Budget::new(1 << 22)` (:65-67), `vec_at` `:87` (uses `common::vector_family`, `common/mod.rs:512`), `payload` `:97`, `branching` `:117`, `out_index` `:132`,
   `next_index` `:172`, `nested_family` `:223`, `nested_declarations` `:237`, `nested_context` `:264`. One test: `nested_applications_reject_wrong_indices_and_malformed_descriptions` `:294`.
3. Inputs/outputs: as above; shares assumptions `Nat`, `zeroN`, `succN`, `Elem` via `nat_succ_declarations` (`common/mod.rs:326`).
4. How it decides (pinned): node at level `succN k` rejected (`TypeMismatch`, :312); payload `<k, e : Elem>` instead of `Vec k` mismatches (:329); index `= zeroN` not a type (:337); strict index sort mismatches (:344);
   strict children error (:357-360); a `next` returning the position instead of `succN k` makes `isup` reject (:417).
5. Cases by shape: unbounded (omega) branching with `Nat` positions because "the kernel has no `Fin` former" (:39-44).
6. Same job elsewhere: see item 6 of indexed_derivation.
7. Unused code: none (tests).
8. Stated limits: `:39-44` "the kernel has no `Fin` former to bound positions by `k`"; `:60-64` budget comment says "measured spend is pinned per test" but this test pins no step count (read).
9. Fail-open risks: none seen.

## Coverage

Read in full (all ranges in `units/gap_024.tsv`):
- `B/proof.rs` 4123-5117 (995)
- `B/tests/exact_multiply_result_bound.rs` 1-688 (688)
- `B/tests/indexed_derivation.rs` 1-991 (991)
- `B/tests/indexed_levels.rs` 1-513 (513)
- `B/tests/indexed_mutual.rs` 1-544 (544)
- `B/tests/indexed_nested.rs` 1-418 (418)
Total in full: 4,149 lines. Skimmed: none. Not read: none.
Greps run: line numbers of `mod tests`/helpers in proof.rs; `bounded_verification_entries` count (16, 2 files); `fn (measure_budget|budget|strict_children_error|vector_family|nat_succ_declarations)` in `tests/common/mod.rs`.

## Five things a maintainer most needs to know about this range

1. Everything here is test code; the proof.rs range is the tail of `mod tests` (opens `proof.rs:3627-3628`), so no production logic was added by this pass.
2. The exact-multiply result bound has no dedicated rule: it is `IntegerAffineBound` + `ValueEqualityTransport` + `IntegerOrderSubstitution` composed (`exact_multiply_result_bound.rs:1-14`), and the test oracle enumerates the full box, not corners (:285-297).
3. An inverted interval is accepted for the corner extreme, and a `Truth` slot is read as the carrier endpoint; both are pinned and explained at `exact_multiply_result_bound.rs:505-510` and `:363-367`.
4. A bound beyond the carrier (400 in `u8`) is proved only about the unbounded product; the overflow obligation (`<= 255`) stays unproved (`:617-688`); corner products that overflow 128-bit magnitudes get `DirectMultiplyBoundOverflow` (`:277-279`).
5. Deep-nesting tests (`proof.rs:4912-5116`) assert on `bounded_verification_entries()` deltas and on default-stack threads at depth 8192; they depend on the exact receipt numbers (`declarations 4`, `assumption_closure 4`, `:5091-5098`) and on measured step counts (`:4982-4989`).

## What I am unsure about

- Whether `tests/common/mod.rs` already defines `type_sort`, `two`, `nat` etc. that `indexed_levels.rs:49` redefines (I did not re-grep common for `type_sort`; `indexed_derivation.rs:444` calls `type_sort(arena, 0)` without a local def, so a common helper likely exists).
- The step-budget comment in `indexed_derivation.rs:60-63` mentions "~3.7 and ~25.1 thousand steps" while the first test pins 2,958; I did not check whether the 25.1k test still exists elsewhere.
- Where `map_direct_multiply_bound` lives (named only in a test header, `exact_multiply_result_bound.rs:8`); not grepped.
