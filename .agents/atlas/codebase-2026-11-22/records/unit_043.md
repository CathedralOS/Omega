# Record: unit_043 (area 07-psi-semantics, crate proof-admission, `proofs/bounded/`)

Root `S/` = `source-rust/subsystems/compiler/psi/semantics/`; files below are under `S/proofs/bounded/`.
THIS RECORD IS PARTIAL. The unit is 22,013 lines; I ran out of room. See Coverage for exact unread ranges.
Nothing below describes an unread range.

## kernel.rs (702 lines, read in full)

1. Purpose: decides a "primitive judgment" (a one-step, no-search justification such as reflexive equality, a closed
   integer comparison, a carrier min/max bound) for a proposition. `kernel.rs:1-7` header.
2. Shape: `decide_primitive(context, proposition, judgment) -> Result<(), KernelError>` (`kernel.rs:18`);
   `KernelError` (`:259`: MalformedProposition, ClosedIntegerEvaluation, FloatInteger, JudgmentDoesNotEstablishGoal);
   `compare_integer_literals` (`:233`, pub(crate)); private `integer_carrier_bound` (`:136`),
   `mathematical_integer_carrier_bound` (`:168`), `..._exclusive` (`:207`), `closed_mathematical_relation` (`:123`),
   `compare_fixed_integer_relation` (`:242`). Re-exports `terminal_psi::PrimitiveJudgment` (`:16`).
3. Inputs/outputs: reads `Proposition`, `PropositionContext`, `closed_predicates`/`integer_rules::closed_integer`
   evaluators, `proof::integer_math_normalization::lift_exact_integer_relation`, `crate::closed_float_integer_relation`.
   Output: Ok or an error; no state.
4. How it decides: a `match (judgment, proposition)` (`:36-117`) keyed on the pair of judgment tag and proposition
   variant. Carrier bound is keyed on the value's own declared type (`ScalarType::Integer`, `carrier()==Fixed`) and
   exact literal equality with `minimum_value()/maximum_value()` (`:154-165`). A negation (`premise -> Falsehood`) is
   accepted only when the closed comparison evaluates to `Some(false)` (`:80-84`, comment "An unavailable evaluation is
   not evidence of falsity"). Size check runs before validation (`:30-32`).
5. Cases by shape: carrier bound only for literal-vs-`Value` (either side) at fixed carrier, or `MathValue` with a
   parameter-width type (`bits().is_none()`) against `carrier_maximum/minimum` terms (`:172-184`); exclusive form only
   for address types with right side equal to `max+1` (`:214-229`). Unmatched pair falls to `_ => false` (`:116`).
6. Same job elsewhere: closed integer comparison is also done in `predicate_denotation.rs:479-506`
   (`closed_integer_relation`, calls `compare_integer_literals` from here, `predicate_denotation.rs:494`).
7. Unused/one-caller: `decide_primitive(` has 57 matches in `S/` and `source-to-psi` across 17 files
   (Grep count over `source-rust`; 22 in kernel.rs itself, mostly tests; callers outside tests include
   `bounded.rs` 2, `bounded_denotation.rs` 1, `proof.rs` 4, `source-to-psi/.../prove.rs` 4, `prove/ranges.rs` 1,
   `prove/widths.rs` 1, `prove/ieee_float_order.rs` 2).
8. Stated limits: `:157` "A parameter width has no literal endpoint, so no literal is one."
9. Fail-open: none seen. Every unmatched pair is `false`; evaluation errors propagate (`?`), not `true`.

## predicate_denotation.rs (1,518 lines, read in full)

1. Purpose: rewrites a goal, its requirements and semantic axioms into one canonical Proposition (Boolean scalar terms
   turned into propositions, closed fixed-integer comparisons turned into Truth/Falsehood, equalities ordered), under a
   shared node/depth budget, then lets a certificate be checked against the rewritten forms. Optionally first
   substitutes cited value definitions (`x == expr`) into the propositions.
2. Shape: `check_predicate_denotations` (`:315`), `check_predicate_denotations_with_value_equalities` (`:330`),
   `check_value_equality_substitution` (`:349`, keeps Boolean structure), `check_value_equality_denotation` (`:364`),
   `check_predicate_evaluation_size` (`:233`), `CheckedPredicateDenotations` (`:269`, holds originals + converted, method
   `check_certificate` `:291` which re-validates originals then calls `proof::check_certificate`),
   private `mod value_equalities` (`:20`, `ValueEqualities` with `definitions: Vec<(&ScalarTerm,&ScalarTerm)>` `:25`),
   `normalize` (`:422`), `boolean_equality` (`:508`), `boolean` (`:560`), `connective` (`:641`),
   `comparison_with_representable_literal` (`:699`), `PredicateDenotationError` (`:675`).
3. Inputs/outputs: reads `PropositionContext`, `closed_predicates::PredicateBudget` (alias `Budget`), `crate::kernel`,
   `crate::proof::integer_math_normalization`. Returns new `Proposition` values; no mutation of inputs.
4. How it decides: a definition is any axiom `Equal(left,right)` whose left side is a "definition subject" (Value,
   Application, field reads, ViewExtent, StructuralRead, ByteSequencePredicate; `:29-41`) with `left != right`; the first
   definition per subject wins (`:55`, `.any(|subject| *subject == left)`). Substitution cycles are refused via an
   `active` position list (`:199-201`). Keys are term identity (structural `==`), not spelling. Budget exhaustion returns
   `ResourceLimitExceeded` (`:683-687`).
5. Cases by shape: `ValueEqualities::proposition` handles an explicit list of Proposition variants (`:71-130`);
   `is_definition_subject` is a fixed eight-variant list (`:29-41`). `check_value_equality_denotation` refuses an empty
   equation list (`:396-398`) and any equation not `Equal(subject,_)` with a definition subject (`:390-393`).
   `normalize` falls to `other => Ok(other.clone())` for unlisted variants (`:475`).
6. Same job elsewhere: Boolean-to-comparison lowering resembles `integer_math_normalization` in `proof.rs:923-1118`
   (read the first 1300 lines of proof.rs only). Value-equality users: 65 matches in 16 files by Grep count (names
   `check_value_equality_substitution|check_predicate_denotations_with_value_equalities|check_value_equality_denotation`),
   including source-to-psi stage 06 (`joins.rs` 4, `known.rs` 4, `prove.rs` 8), `S/src/scalars/scalar_field_requirements.rs` 3,
   `descriptor_results/payload_*.rs` 3, `proof.rs` 2, `bounded.rs` 2, `float_integer.rs` 1.
7. Unused/one-caller: all pub fns above have at least one non-test caller per the count; I did not split per function.
8. Stated limits: `:346-348` substitution "supplies no evidence for its equations" (the caller must prove them).
9. Fail-open: `normalize` `other => Ok(other.clone())` (`:475`) passes unlisted variants unchanged, i.e. no rewriting;
   it accepts nothing by itself because a certificate is still checked against the result (stated by design, `:215-227`
   in tests; traced only in the sense that `check_certificate` is always applied afterwards, `:291-312`). Otherwise none seen.
   Tests at `:746-1518` read; they assert refusal for missing/changed definitions (`:876-908`), definitions placed in
   requirements/conjunction/disjunction/implication (`:911-959`), cycles (`:988-1010`), budget (`:1013-1051`).

## tests/bitwise_sibling_premises.rs (166 lines, read), tests/common/mod.rs (521, read), tests/contradiction_elimination.rs (175, read), tests/equality_symmetry.rs (32, read)

- bitwise_sibling_premises: pairs a positive (`y == 3` cited) with a negative twin (`y <= 3` only) for `x & y` and
  `x | y` through `IntegerAffineBound` (`:104-166`). Comment `:3-10` records an earlier kernel bug (searched uncited axioms).
- common/mod.rs: shared term-builder DSL for the `indexed_*` integration tests (`variable`, `pi`, `lambda`, `sigma`,
  `case_two`, `vector_family` `:512`, `step_type` `:200`, etc.). `#![allow(dead_code)]` at `:5`. Constants `NAT=5, NAT_ZERO=6,
  NAT_SUCC=7, ELEM=8` (`:14-17`) are declaration indices.
- contradiction_elimination: six tests on the spec path `3<=counter`, `counter<bound` ⊢ `3<bound` ⊢ `Falsehood AND goal`
  ⊢ goal (`:69-92`), with negatives for consistent premises, missing premise, wrong position, escaped scope, malformed goal.
- equality_symmetry: one negative test (`:24-32`).

## integer_affine.rs lines 1-1400 of 7,192 (read)

1. Purpose: checks integer "affine forms" replayed from cited SSA definition axioms, and maps a bound on a root value
   to a bound on a target value (`integer_affine.rs:1-7`). Lines read are `mod bound_mapping` (`:9-...`).
2. Shape (read part): `relative_translation` submodule (`:20-370`, incl. 4 tests `:101-369`);
   `check_integer_affine_bound_conversion` (`:372`), `map_integer_affine_bound` (`:407`), `BoundRoute` (`:419`) chosen by
   `bound_route` (`:434`) from `form.endpoint_steps`, `walk_endpoint_steps` (`:472`, chain case),
   `integer_affine_wrapping_evidence` (`:729`), `push_wrapping_evidence` (`:762`), `map_correlated_{add,subtract,multiply}_bound`
   (`:818,855,891`), `map_direct_{add,subtract,multiply}_bound` (`:940,985,1052`), `DirectAddEndpoint` (`:1178`),
   `direct_operand_endpoint` (`:1214`), literal arithmetic helpers (`:1273-1347`).
3. Inputs/outputs: `CheckedIntegerAffineForm` + a root bound `Proposition` -> the mapped `Proposition` or
   `IntegerAffineBoundConversionError`.
4. How it decides: the route is keyed on the shape of the `endpoint_steps` slice and, when empty, on the variant of the
   target term (`:434-464`). Bound orientation is keyed on whether `bound_left == form.root()` (`:714-720`), term equality.
5. Cases by shape: `bound_route` single-step slice patterns only (`:436-453`); `[]` + target variant Add/Subtract/Multiply/
   ShiftLeft (`:454-461`); anything else is `Chain`. Direct multiply needs exactly four evidence members (`:1059`), direct
   add/subtract exactly two (`:947,992`), correlated multiply exactly two (`:901`).
6. Same job elsewhere: `check_integer_affine_bound_conversion` accepts the conclusion if it equals either the current
   mapping or a mapping with policy `RemainderUpperBound::TotalImage` (`:380-396`): two mappings, one called "legacy"
   in the local binding name (`:393`). The same affine-bound dispatch is called from `proof.rs:494-544` (`affine_bound_relation`).
7. Unused: not checked (range unread).
8. Stated limits: `:1195` "A carrier endpoint exists only at a closed width."; `:904-905` a comment that `i1` declines the rule.
9. Fail-open: `check_integer_affine_bound_conversion` `:387-396` accepts a conclusion equal to the `TotalImage` ("legacy")
   remainder mapping even if the current `DividendIntersection` mapping differs (traced within this file only: the
   TotalImage image bound is weaker for unsigned, `:536-548`, so a conclusion `x%m <= m-1` is accepted where the current
   policy would give `min(mapped, m-1)`; it is a weaker-but-sound bound for the unsigned case, no comment states why both
   are kept: open). `walk_endpoint_steps` `Remainder` case, unsigned upper endpoint: `mapped.min(image_maximum)` vs
   `image_maximum` (`:536-548`).

## proof.rs lines 1-1300 of 5,117 (read)

1. Purpose: walks a `ProofNode` certificate on an explicit stack, applies each rule's local check, records which
   premises/axioms were cited (`CertificateAcceptance`), then re-decides the certificate in the mathematical kernel
   (`proof.rs:1-10`).
2. Shape (read part): `AcceptedProofRule` enum (`:31-80`, 34 variants, several marked "provisional"), `AcceptedPremise`
   (`:83`), `CertificateAcceptance` (`:89`: rules, assumptions, semantic_axioms, `mathematical_core`), `MathematicalCoreDecision`
   (`:97`: `Judged(receipt)` or `Refused(&'static str)`), `AcceptanceBuilder` (`:111`), `PremiseRoster` (`:140`, uses
   `BTreeMap<usize, Vec<usize>>` `:142`), `check_certificate` (`:163`), `accept_certificate` (`:173`),
   `accept_certificate_with_machine_parameters` (`:190`), `re_decide_in_mathematical_core` (`:239`), submodules
   `add_order` (`:264`), `equality_rules` (`:307`), `integer_bound_rules` (`:473`), `integer_math_normalization` (`:923`,
   tests from `:1120`, read to `:1300`).
3. Inputs/outputs: context, goal, assumptions, semantic axioms, machine-parameter value set, proof -> `CertificateAcceptance`
   or `ProofError`.
4. How it decides: top level order: validate goal/assumptions/axioms against context (`:200-212`), `check_node`
   (`:217`), `proof.conclusion == goal` (`:225`), then kernel re-decision (`:228`). Assumptions are recorded only when
   `index < ambient_assumption_count` (`:119-123`), so discharged local assumptions are not recorded as ambient.
   Equality transitivity matches on the middle term by `==` (`:395,408`) and accepts the conclusion also under integer-math
   normalization (`:399`, `propositions_match_under_integer_math_normalization` `:993`).
5. Cases by shape: `equality_transitivity_relation` three pair shapes only: `Equal/Equal`, `IntegerMathEqual/IntegerMathEqual`,
   `ContentConservation/ContentConservation`; else `RulePremiseMismatch` (`:393-458`). `add_order::check` requires fixed non-address
   carrier and a zero literal (`:290-299`). `exact_definition_bound_relation` handles `Add`/`Subtract` only (`:547`).
   `lift_exact_integer_term` handles IntegerWiden, ExactIntegerAdd/Subtract/Multiply/ShiftLeft, Value, Integer; `_ => None`
   (`:1026-1107`).
6. Same job elsewhere: `affine_bound_relation` (`:494`) and `cast_bound_relation` (`:741`) share the "root_bound == Truth ->
   truth_bounds, else conversion check" shape (`:514-525` vs `:752-763`).
7. Unused: not checked per function. `accept_certificate(` etc. not counted.
8. Stated limits: `:56-58` provisional rule families; `:73-74` "provisional until derived in the mathematical core".
   `MathematicalCoreDecision::Refused(&'static str)` (`:99`) records that the mathematical kernel did not cover this certificate.
9. Fail-open: `re_decide_in_mathematical_core` `:257-259`: when the kernel reports `BoundedDenotationError::Unsupported(family)` the
   certificate is still accepted, with `MathematicalCoreDecision::Refused(family)` recorded; only other errors reject.
   Traced within this file: a certificate in an unsupported rule family is admitted on the local checks alone
   (stated by the enum doc comments for "provisional" rules, `:56-78`); whether callers read `mathematical_core` was not traced.
   `lower_integer_math_relation(..).unwrap_or_else(|| conclusion.clone())` (`:512-513`, `:750-751`) keeps the original
   conclusion when lowering is not possible (stated: none; traced: conclusion is then compared as-is, so no extra acceptance).

## Coverage

Read in full: kernel.rs 1-702 (702); predicate_denotation.rs 1-1518 (1,518); tests/bitwise_sibling_premises.rs 1-166 (166);
tests/common/mod.rs 1-521 (521); tests/contradiction_elimination.rs 1-175 (175); tests/equality_symmetry.rs 1-32 (32).
Total in full: 3,114 lines.
Read (partial ranges): integer_affine.rs 1-1400 (1,400); proof.rs 1-1300 (1,300). Total: 2,700 lines.
Skimmed: none.
NOT READ: integer_affine.rs 1401-7192 (5,792 lines); integer_rules.rs 1-3386 (3,386 lines); proof.rs 1301-5117 (3,817 lines);
tests/exact_multiply_result_bound.rs 1-688; tests/indexed_derivation.rs 1-991; tests/indexed_levels.rs 1-513;
tests/indexed_mutual.rs 1-544; tests/indexed_nested.rs 1-418 (3,154 lines). Total not read: 16,149 lines.

## Five things a maintainer most needs to know (about what was read)

1. A certificate is admitted in two passes: local rule checks, then re-decision by the mathematical kernel; if the kernel
   returns `Unsupported`, the certificate stays admitted with `Refused(family)` recorded (`proof.rs:239-262`).
2. `decide_primitive` accepts a negation only when a closed relation evaluates to `Some(false)`; an evaluation error or
   unavailable value is never read as false (`kernel.rs:80-84`).
3. Predicate-denotation value substitution uses only axioms of the form `Equal(subject, _)`; definitions placed in
   requirements or under connectives are not used (test `predicate_denotation.rs:911-959`), first definition per subject wins (`:55`).
4. `check_integer_affine_bound_conversion` accepts the conclusion of either of two remainder policies (`integer_affine.rs:380-396`).
5. The affine-bound mapper selects a rule by endpoint-step slice shape and falls back to a generic chain walk
   (`integer_affine.rs:434-464`); evidence member counts are fixed per route (2, 2, 4).

## What I am unsure about

- Everything in the unread ranges, in particular `integer_rules.rs` (closed_integer, integer_cast, forbidden_root, integer_shift)
  and the `check_node` traversal and rule dispatcher in `proof.rs` after line 1300 (where `dispatched_rule!`, `RuleScope`,
  `ProofError` and `propositional_rules` are defined; only their uses were seen).
- Whether the "legacy" TotalImage acceptance at `integer_affine.rs:387-396` is still exercised by any producer.
- Per-function caller counts for the `pub` items here; only two grouped counts were run.
