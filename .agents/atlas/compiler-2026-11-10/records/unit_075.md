# Unit 75 notes

Files covered (all paths under `omega-rust/psi/compilation/proof/`):
1. admission/src/mathematical_typing.rs (3675 lines)
2. admission/src/predicate_denotation.rs (1350)
3. admission/src/proof.rs (4137)
4. admission/tests/common/mod.rs (521)
5. admission/tests/contradiction_elimination.rs (175)
6. admission/tests/equality_symmetry.rs (32)
7. admission/tests/indexed_derivation.rs (990)
8. admission/tests/indexed_levels.rs (514)
9. admission/tests/indexed_mutual.rs (544)
10. admission/tests/indexed_nested.rs (418)
11. admission/tests/indexed_relations.rs (863)
12. admission/tests/indexed_vector.rs (299)
13. admission/tests/integer_order_weakening.rs (60)
14. admission/tests/nonconstructive_existence.rs (605)
15. admission/tests/predicate_conversion.rs (74)
16. admission/tests/predicate_denotation.rs (496)
17. admission/tests/source_checked_authority.rs (31)
18. admission/tests/suite.rs (20)
19. admission/tests/theorem_certificate_admission.rs (265)
20. admission/tests/value_equality_transport.rs (454)
21. src/boundary.rs (213)

(All line numbers below are for the named file; "typing" = mathematical_typing.rs, "pred" = predicate_denotation.rs, "proof" = proof.rs, all under admission/src.)

---

## 1. admission/src/mathematical_typing.rs

**Purpose.** The type checker of the mathematical core: infers the type (and sort) of a term of the dependent calculus (`Pi`, `Sigma`, `Two`, `Id`, `W`, `Squash`, `Box`, `Empty`, constants from a `Signature`), checks a term against an expected type, and decides typed conversion between two terms at a shared type. Header (1-9) says strict proof irrelevance is "gated solely on the shared type's `Strict` sort". The recursion is turned into an explicit work stack so deep terms do not overflow the host stack (read: `drive`, 2284-2657; `comparison::Work` 630-776).

**Shape.**
- `Context` (typing:38) = `bindings: Vec<TermHandle>` (de Bruijn stack), `level_arity: u32`, `signature: Signature`. Methods: `empty` 45, `with_level_arity` 53, `with_signature` 69, `extend` 77 (clones the whole binding vector), `lookup` 87 (shifts stored type by `index+1`), `len`, `is_empty`.
- `CoreError` (105-208): about 30 variants (DummyTerm, UnboundVariable, NotASort/NotAFunction/NotAPair/NotAnIdentity/NotAW/NotASquash/NotABox, StrictCaseMotiveCodomain, StrictIdentityDomain, StrictWCarrier, StrictSquashDomain, NonStrictBoxDomain, SquashTargetNotStrict, UnknownDeclaration, DeclarationArityMismatch, ArgumentTypeMismatch, TypeMismatch, StepCeiling, ...).
- Public entry points: `infer_sort` 210, `infer_type` 237, `check_type` 3660, `pub(crate) convert_retaining` 343, `argument_error_pair_pattern!` macro 3670 (matches `TypeMismatch | ArgumentTypeMismatch`).
- Helpers: `inferred_sort` 220 (weak-head-normalises the inferred type, must be `Term::Sort`), `infer_leaf_type` 270 (Dummy/Variable/Sort/Two/TwoZero/TwoOne/Empty/Constant), `check_level` 355, `w_step_type` 385 and `box_body_type` 425 (build the induction step type for `IndW` and the body type for `BoxElim`).
- `mod judgments` (441-3658): the driver. `Operation{Convert, Infer, Check}` (2202), `Pending` (1988-2200, about 60 continuation frames), `JudgmentValue{Inferred, Checked, Converted}` (2211), `drive` (2284). `mod comparison` (499-1941): conversion work items `Work` (630), `Step` (551), `Query{terms, shared_type, retained}` (521), `enter_heads` (828) and `Work::resume` (1363).

**Inputs/outputs.** Reads `TermArena` (mutated: terms are inserted), `Context`, `Budget` (from `mathematical_core::conversion`), `weak_head_normalize` / `weak_head_normalize_retaining` (conversion.rs, not read), `shift`/`substitute`/`instantiate_levels` (substitution.rs, not read), `Signature` declarations (`signature.get(index)`, 317). Returns `TermHandle` types, `Sort`, `()` for check, or `bool` for convert. Only side effect is arena growth and budget spend.

**How it decides.**
- Variables: by de Bruijn index in `context.bindings` (87-94). Constants: by declaration index into the signature, plus level-arity equality (323) and level scoping (`check_level`, 355).
- Conversion (`enter_heads`, 839-1359): structural match on the pair of head constructors; Sort/Two/Variable/Constant compared by value (840-862); binders compared by comparing domains then codomains under an extended context; Lambda vs non-lambda and Pair vs non-pair use eta (1284-1357). Anything unmatched returns `Complete(false)` (1358).
- Proof irrelevance: `SharedSort` (1371-1398): infers the sort of the shared type; if `sort.is_strict()` result is `true` immediately (1375-1378). Otherwise a structural-equality shortcut on raw terms, then weak-head normalise and compare heads.
- `retained: &dyn Fn(u32) -> bool` (508, 521) is a predicate over declaration index passed to `weak_head_normalize_retaining` (785); the default `expand_all` (510) returns `false` for every index. `retained` is keyed on declaration identity (u32 index).
- Sort rules: `Pi` result sort follows the codomain's strictness (3051-3054); `Sigma` is Strict only if both parts Strict (3055-3058); `Squash` needs a `Type` domain and gives `Strict` (3066-3069); `Box` needs a Strict domain and gives `Type` (3070-3073); `Id` domain must not be Strict (3484); `W` carrier must be `Type` (3417); motive codomains for `CaseTwo`, `IdElim`, `IndW` must be `Type` sorts (2833-2882) whereas `BoxElim` motive only needs `Term::Sort(_)` (2899, accepts Strict or Type); `SquashElim` target must be Strict (3395).
- Pair-argument special case: an `Apply` whose argument node is syntactically a `Term::Pair` is checked against the domain instead of inferred (3119-3125).

**Cases by shape.** Conversion handles a fixed list of pairs of heads (839-1357) and returns `false` for any other pair (1358). Level `check_level` handles Constant/Parameter/Successor/Maximum (355-383). `infer_type` routes 21 constructor kinds to `judgments::infer` and leaves the rest (Dummy, Variable, Sort, Two, TwoZero, TwoOne, Empty, Constant) to `infer_leaf_type` (245-266); the leaf function has `unreachable!("constructor inference owns these nodes")` for the constructor arms (308). Six `unreachable!` / `.expect` sites in the file (grep count).

**Same job elsewhere.**
- Conversion/typing driver is the single implementation; `mathematical_core.rs:1311` calls `typing::convert_retaining` (grep: 1 caller outside the definition) and `mathematical_core.rs` has 177 uses of `infer_sort|infer_type|check_type` (grep count, not read). `mathematical_declarations.rs` in the proof crate has 67 (grepped, not read).
- The IW/indexed-family tests (`tests/common/mod.rs:200-252`, `indexed_derivation.rs`) re-derive by hand the same step type that `w_step_type` (typing:385) builds for plain W. Suspected: indexed-family version lives in mathematical_core.rs (not read).
- `w_step_type` and `box_body_type` are each used only in typing.rs (grep: 8 occurrences across 1 file, definition + uses inside `comparison` and `judgments`).

**Unused / one-caller.** `convert_retaining`: 1 caller outside its definition (mathematical_core.rs:1311). `Context::with_level_arity`: 32 occurrences across typing/mathematical_core/mathematical_declarations (grep), so used. `Context::is_empty`: not grepped specifically.

**Stated limits.** Header: decides conversion "gated solely on the shared type's `Strict` sort" (7-8). `StepCeiling` error variant (207) is the budget refusal. No TODO/unsupported comments seen.

**Fail-open risks.**
- `SharedSort` returns `true` for any two terms when the shared type's sort is Strict (typing:1375-1378). Kind: "stated" (header 7-8) and by design. What gets through: two arbitrary terms compared at a Strict-sorted type. `convert` infers only the sort of `shared_type` (drive 2295-2301); it never checks that `left` and `right` are inhabitants of `shared_type`. So a caller that supplies wrongly typed terms with a strict shared type gets `true`. Callers inside typing always supply the shared type from the terms' own inferred type (e.g. 3158-3160, 3290-3297, 3580-3585 `shared_type = Sort(expected_sort)`); external caller `mathematical_core.rs:1311` not traced. Mark "open" for the external caller.
- `CheckExpected` (3534-3587) when `term` is a Pair/Lambda but the expected type does not normalise to Sigma/Pi falls through to infer-and-convert (3580-3586), i.e. falls back, does not accept.
- Default reject elsewhere: all unmatched conversion pairs give `false`. None else seen.

---

## 2. admission/src/predicate_denotation.rs

**Purpose.** Normalises a boolean scalar predicate (`Proposition`) into a canonical proposition form and, optionally, substitutes cited value equalities (`v = expr`) into it, under hard node/depth budgets. The normalised goal/requirements/axioms are what a certificate is then checked against. Header 1-8.

**Shape.**
- `mod budget` (19-160): `Budget{remaining}` starts at 4096 (31); `step(depth)` errors at `depth >= 64` (35) and counts 1; `proposition` (49), `content` (100), `mathematical` (117), `scalar` (132) walk a term and charge nodes; path-bearing leaves charge `path().len()` (77-85, 107, 145, 150).
- `mod value_equalities` (161-349): `ValueEqualities{definitions: Vec<(&ScalarTerm,&ScalarTerm)>}` (166); `is_definition_subject` (170) accepts `Value`, `IntegerField`, `BooleanField`, `IeeeFloatField`, `ViewExtent`, `StructuralRead`, `ByteSequencePredicate`; `from_semantic_axioms` (184), `proposition` (202), `mathematical` (252), `scalar` (303, with `active: Vec<usize>` cycle guard, 317).
- Public: `check_predicate_evaluation_size` 352, `check_predicate_denotations` 438, `check_predicate_denotations_with_value_equalities` 453, `check_value_equality_denotation` 469, `CheckedPredicateDenotations` 392 (with `check_certificate` 414), `PredicateDenotationError` 787 (ResourceLimitExceeded, CyclicValueEquality, InvalidValueEquality, Malformed, Proof).
- Private: `normalize` 516, `closed_integer_relation` 557, `closed_ieee_float_relation` 586, `boolean_equality` 620, `boolean` 672, `equality` 745 (orders the two sides with `<=`), `connective` 753 (flatten, sort, dedup), `comparison_with_representable_literal` 799.
- Tests: `mod value_equality_tests` 845-1350.

**Inputs/outputs.** Reads `semantic_vocabulary::{Proposition, ScalarTerm, IntegerMathTerm, PropositionContext}`, `numerics::float_semantics`, `crate::kernel::compare_integer_literals` (572), `crate::proof::integer_math_normalization::lift_exact_integer_term` (265, 279), `crate::check_certificate` (427). Produces new `Proposition` values; no mutation of inputs.

**How it decides.**
- Value equalities: an axiom `Equal(left,right)` is a definition only if `left` is one of the seven subject kinds (192) and `left != right`; the first equation for a given left term wins, later ones for the same left are silently dropped (195). Lookup is by term equality (`*subject == original`, 315).
- Cycle: a definition already on the `active` stack gives `CyclicValueEquality` (317-319).
- Closed relations: integer literal pairs compared only for `IntegerCarrier::Fixed` (569); IEEE literals compared with `FloatSemantics` only when both formats equal the stated format (602).
- Boolean equality with a literal operand becomes a polarity on the other side (635-640); two boolean atoms stay an `Equal` when positive (641); otherwise expands into a disjunction of conjunctions (644-658). `boolean` has a catch-all `_ => equality(term, Boolean(positive))` (741).
- Integer comparisons between different carriers: if one side is a literal exactly representable in the other's type it is re-typed (799-843, `exact_cast_value_to`, kind encoded as 0/1/2); else `mathematical_comparison` (682) lifts to the mathematical-integer form.

**Cases by shape.** `is_definition_subject` fixed list of seven term kinds (170-181). `comparison_with_representable_literal` handles only `IntegerEqual/LessThan/LessOrEqual` with an `Integer` literal on exactly one side (800-835). `normalize` `other => Ok(other.clone())` (553) passes every proposition kind not matched through unchanged.

**Same job elsewhere.** `check_predicate_evaluation_size` (352) is the shared size gate; the same 64-depth/4096-node limit is used by `integer_math_normalization::lift_*` in proof.rs via this function (proof 908, 930, 988). `ordered_comparison_operands!` macro patterns repeat (211, 559).

**Unused / one-caller.** grep over `omega-rust/psi` (excluding admission tests): `check_predicate_evaluation_size` called from terminal-installation/evaluation_and_synchronous_calls.rs:82, terminal-interpreter/boundary_effects.rs:68 and 1794, descriptors.rs:3325, proof.rs:908/930/988. `check_predicate_denotations_with_value_equalities`: descriptors.rs:3552, 3890, 6669, scalars.rs:3121/3169 (tests?) plus this file. `check_predicate_denotations`: proof.rs:313/315 only outside tests. `check_value_equality_denotation`: proof.rs:340/342 only outside tests.

**Stated limits.** Hard limits 4096 nodes / depth 64 (31, 35), tests at 1062-1100 pin them. `from_semantic_axioms` comment-free.

**Fail-open risks.**
- `from_semantic_axioms` drops non-matching or duplicate-left equations silently (189-195): fewer definitions, so it can only reject more. Kind: "traced" (the equations used for the original and for the conclusion in `value_equality_transport_relation`, proof.rs:334-350, are the same set, so a dropped second definition applies to both sides equally).
- `normalize` passes unknown propositions unchanged (553). That is not acceptance; the caller still requires a certificate. None accepting seen.
- `check_value_equality_denotation` accepts only equations whose left is a definition subject (485) and errors on empty list (491).

---

## 3. admission/src/proof.rs

**Purpose.** Checks an explicit `ProofNode` certificate tree against a goal, a list of assumptions and a list of semantic axioms, one rule at a time, then asks the mathematical core to re-decide it. Also records which assumptions/axioms/rules the certificate actually used (`CertificateAcceptance`). Header 1-10.

**Shape.**
- Types: `AcceptedProofRule` 32 (28 variants), `AcceptedPremise` 64, `CertificateAcceptance` 70 (`rules`, `assumptions`, `semantic_axioms`, `mathematical_core`), `MathematicalCoreDecision{Judged(receipt), Refused(&'static str)}` 78, `MathematicalJudgmentReceipt` 84, `AcceptanceBuilder` 92, `PremiseRoster` 121, `RuleScope` 2338, `ProofError` 2425 (about 36 variants).
- Entry functions: `check_certificate` 144, `accept_certificate` 154, `accept_certificate_with_machine_parameters` 171 (validate goal/assumptions/axioms against the context; run `check_node`; require `proof.conclusion == goal` (206); call `re_decide_in_mathematical_core` 220).
- Traversal: `mod traversal` (2092-2320): explicit stack of `Action{Enter, Post, Children, Cases, Branches, PushAssumption, PopAssumption}`; post-order; implication introduction and disjunction elimination push/pop a local assumption (2214-2223, 2152-2161). `check_node_locally` 2345 dispatches on 28 `ProofRule` variants via the `dispatched_rule!` macro (2327).
- Rule modules: `add_order` 245, `equality_rules` 288 (symmetry 295, `predicate_denotation_relation` 308, `value_equality_transport_relation` 334, `equality_transitivity_relation` 369), `integer_bound_rules` 454 (`affine_bound_relation` 475, `exact_definition_bound_relation` 580, `cast_bound_relation` 713, `correlated_forbidden_roots_relation` 812), `integer_math_normalization` 896 (`lift_fixed_integer_relation` 901, `lower_integer_math_relation` 923, `propositions_match_under_integer_math_normalization` 966, `lift_exact_integer_*` 985-1038), `integer_order_rules` 1152, `order_discreteness` 1452, `propositional_rules` 1598, `strict_order_transitivity` 1762, `subtract_antitone` 1996, `subtract_order` 2049. Helpers `scalar_order` 2484, `mathematical_order` 2492.
- Test modules from 1040 (inside `integer_math_normalization`), 1514, 1818, 2502 (`disjunction_elimination`), 2641 (`order_substitution_tests`), 2785 (`tests`, to 4137).

**Inputs/outputs.** Reads `PropositionContext`, assumptions, semantic axioms, `BTreeSet<ValueId>` of machine parameters, `crate::decide_primitive` (primitive judgment, 1613), the integer-affine / cast / correlated-root witness checkers (from `crate`), `predicate_denotation` checkers, `mathematical_core::verify_bounded_certificate_with_machine_parameters` (228). Returns `CertificateAcceptance` or `ProofError`.

**How it decides.**
- Assumption/semantic-axiom citation: by index into the supplied slice, then proposition equality modulo integer-math normalisation (`propositions_match_under_integer_math_normalization`, 1629, 1646); the same function is applied at 380, 971-972.
- Citations recorded by index and exact proposition (`PremiseRoster::record` 127). Assumption indices at or above the ambient count are local (discharged) and are not recorded (100-104).
- Local assumptions from implication introduction/disjunction cases: pushed on `assumptions` for the child and popped (2160-2166).
- Conjunction/disjunction/implication rules compare conclusions by full `Proposition` equality (1667, 1685, 1703, 1720, 1737, 1754, 1757).
- Integer order rules compare terms by equality and by `scalar_type()`: carrier must be `IntegerCarrier::Fixed` and not `is_address()` for add/subtract order (271-272, 2032-2033, 2075-2076), antisymmetry (1420), discreteness `adjacent` (1502). Literal zero detected via `integer_value()` match (276-279, 2080-2083).
- Endpoint substitution: endpoint number 0/1 else `UnknownIntegerOrderEndpoint` (1322-1335); equality matched in either orientation (1337-1338).
- Mathematical re-decision: `Err(Unsupported(family))` becomes `Ok(Refused(family))` (238-240); any other error becomes `ProofError::MathematicalCore` (241).

**Cases by shape.** `AcceptedProofRule`/`ProofRule` fixed list of 28 rules (32-61); `check_node_locally` match is exhaustive (2359-2421). Equality transitivity handles three proposition families only: `Equal`, `IntegerMathEqual`, `ContentConservation` (374-438), `_ => RulePremiseMismatch` (438). `less_or_equal_transitivity` handles `LessOrEqual` and `IntegerMathLessOrEqual` only (1239-1264). `order_weakening` accepts only `Equal`/`LessThan` premise (1209). `exact_definition_bound_relation` handles only `ExactIntegerAdd`/`ExactIntegerSubtract` and the `Value`-on-one-side orientation (553-569, 600-612). `lift_exact_integer_term` handles `ExactIntegerAdd/Subtract/Multiply`, `Value`, `Integer` (999-1027). Correlated roots: two branches `dividend`/`divisor` (837).

**Same job elsewhere.**
- `bounded_denotation.rs` (not read) re-runs the same relation functions: grep shows `bounded_denotation.rs` lines 7113-7114, 7976, 8004, 8036, 8066, 8083, 8108, 8139, 8151, 8175, 8212, 8242, 8264, 8281, 8315, 8411-8444, 8476, 8513 call into `equality_rules`, `integer_order_rules`, `integer_bound_rules`, `add_order`, `subtract_order`, `subtract_antitone`, `order_discreteness`, `strict_order_transitivity`, `ExactDefinitionOperation` (grep). So these `*_relation` functions are shared between the bounded checker (this file) and the bounded denotation.
- The `scalar_order`/`mathematical_order` split (2484, 2492) and `check_endpoints` generic (1311) duplicate the strict/non-strict handling of scalar vs math orders; `strict_order_transitivity::endpoints` (1801) has the same shape.
- `add_order::check` (250) and `subtract_order::check` (2054) are near mirror images.
- `affine_bound_relation` and `cast_bound_relation` repeat the same "Truth root => look up bounds, else convert" structure (486-497, 724-735).

**Unused / one-caller.** Grep over `omega-rust`: `accept_certificate|accept_certificate_with_machine_parameters|check_certificate` outside admission: descriptors.rs 10, scalars.rs 1. `lift_exact_integer_relation|lift_exact_integer_order|lift_exact_integer_term|propositions_match_under_integer_math_normalization`: 35 occurrences across 7 files (proof.rs 25, bounded_denotation.rs 3, kernel.rs 2, predicate_denotation.rs 2, descriptors.rs 1, integer_rules.rs 1, lib.rs 1). `check_integer_order_discreteness` etc. are `pub(super)` single-dispatch.

**Stated limits.** Header says the common mathematical core must agree "on every covered certificate" (8-9): certificates outside coverage are returned `Refused`.

**Fail-open risks.**
- `re_decide_in_mathematical_core` (238-240): when the core reports `Unsupported(family)`, the certificate is still accepted on the bounded rules alone with `Refused(family)`. Kind: "stated" (header "covered certificate") and "traced" for consumers: outside tests the only reader of `MathematicalCoreDecision` is `proof/src/checker.rs:5680-5693`, which counts `Refused` as `kernel_refusals += 1` and does not reject (grep + read). A wrong certificate in an uncovered family depends on the bounded rules alone.
- `check_assumption` uses `assumptions.get(*index)` on the combined (ambient + local) list (1643); local indices are valid inside branches (traced, test at 2603-2638).
- `schedule_children` for `Primitive`, `SemanticAxiom`, `Assumption`, `IntegerCorrelatedForbiddenRoots` has no children (2178-2181): leaf rules whose legitimacy depends on `check_node_locally`. Traced.
- `check_integer_cited_bound` with `IntegerAffineBound` and `root_bound == Truth`: skips `check_integer_affine_bound_conversion` and instead checks the conclusion against `integer_affine_truth_bounds` (486-493). Stated by test `TruthRootWithoutTotalImage` (3455-3457).
- `lift_*` functions return `None` when size check fails (908, 930, 988) and callers then treat the relation as "no normalisation" (e.g. 485, 1364), falling back to exact match: fail-closed.

---

## 4. tests/common/mod.rs

**Purpose.** Constructor DSL used by the indexed-family tests: `variable`, `pi`, `lambda`, `apply`, `pair`, `fst`, `snd`, `sigma`, `type_sort`, `strict_sort`, `two`, `case_two`, `id`, `nat`, `refl`, `squash*`, `boxed`, `box_elim` (27-191). Also `step_type` (200) (the `iindW` step type, hand-built), `carrier` 256, `strict_branching` 271, `tag_selected_children` 291, `elem_nat_payload` 310, `nat_succ_declarations` 326, `strict_children_error` 340, `next_indexed_w` 366, `assert_type_mismatch` 379, `assert_check_type_mismatch` 390, and the shared length-indexed vector description `vector_branching`, `vector_out_index`, `vector_next_index`, `vector_family` (416-521). Constants NAT=5, NAT_ZERO=6, NAT_SUCC=7, ELEM=8 (14-17), keyed on declaration position.
**Same job elsewhere.** `value` (193) builds a Boolean `ScalarTerm` (used by predicate_conversion.rs and value_equality_transport.rs); each of those files also defines local `context()`.

## 5. tests/contradiction_elimination.rs

Six tests on scoped contradiction elimination: from `3 <= counter` and `counter < 3`, strict-order transitivity gives `3 < 3`, predicate denotation converts it to `Falsehood AND goal`, conjunction elimination yields goal (69-92). Negatives: consistent premises (108), missing premise (117), citation by wrong position (125, expects `AssumptionConclusionMismatch`), branch assumption discharged by implication (137-166, cited list is `[0]` only), goal naming an undeclared value (169). Keyed on indices and proposition equality. Header cites `wiki/spec/proofs/classicality.md#contradiction-elimination`.

## 6. tests/equality_symmetry.rs

One test (23-32): `EqualitySymmetry` over a `Truth` premise child (replaced at 29-30) is rejected. 32 lines.

## 7. tests/indexed_derivation.rs

Builds an indexed family of implicational derivations (`Deriv <Gamma, phi>`) on the derived `indexed_scheme` with index `I = Sigma(Ctx).Frm` and three rules (assm, impI, impE), then (a) proves by `iindW` with a dependent motive that the conclusion identity holds, checks the result converts to `refl Frm psi`, and pins measured spend `4_877` budget steps (590-729, assertion at 728); (b) rejects wrong judgments/malformed descriptions: wrong conclusion, wrong context, swapped premises, assm at unextended context, a `next` that requires the wrong minor-premise judgment (rejected when used), an ill-typed `next`, a Strict-branching family, and a bad motive universe (731-990). Assumptions `Ctx`, `Frm`, `consF`, `impF` are declarations 5-8 (90-95). Defect cases: a defective-but-well-typed `next` still forms the type (860-869) but a derivation under it rejects (908-916).

## 8. tests/indexed_levels.rs

Universe-polymorphic length-indexed vector: `vecOf` and `consVec` are level-arity-1 definitions (250-331, declaration 8, `VEC_OF`); tests instantiate at `u=0` (`Two`) and `u=1` (`Type 0`) in one signature and show the two instantiations do not convert (347-430); reject wrong universe on element (443-449), wrong level count (452-461, `DeclarationArityMismatch`), unbound level parameter in closed context (465-471), description at wrong levels (475-496), `iindW` with 3 level args (500-513, expected 4). Test asserts `declarations.len() == 10` (352).

## 9. tests/indexed_mutual.rs

Mutual `Tree`/`Forest` as one indexed family indexed by `Two` (`zero`=Tree, `one`=Forest); one test (317-544) rejecting wrong sorts, a child of wrong sort, uniform-tree child function, swapped `caseTwo` branches, a `next` that flips sorts (flipped family), Strict-branching, and bad motive universe. Declaration `ELEM=5` (60).

## 10. tests/indexed_nested.rs

Level-indexed rose tree whose `rnode` payload carries `Vec k` (the vector family from common). One test (293-418): wrong level (`Rose k` vs `Rose (succ k)`), malformed payload (element where vector needed), index = value, strict index sort, Strict-branching, and a `next` returning the position (`bad_next`). Budget `1 << 22` (65-67). Comment: `B rnode = Nat` gives omega-branching "kernel has no `Fin` former" (39-41).

## 11. tests/indexed_relations.rs

Strict layer: `Squash (IW ...)` is existence without witness; leaf family (`I=A=Two`, dead children `Id Two one one`) (77-105). Test 1 (113-277): squash and `unsq` computation, refusal cells (relevant target `SquashTargetNotStrict`, un-squashed scrutinee `NotASquash`, squashed witness not a derivation `TypeMismatch`, double squash `StrictSquashDomain`, boxing relevant `NonStrictBoxDomain`), certificate with empty assumption closure (262-276). Test 2 (279-545): `iindW` reaches strict target via `Box (Squash ...)` and `unbox`; Strict-codomain motive refused. Test 3 (652-863): a description cannot name its own family; self-reference fails `UnknownDeclaration{declaration:9, signature_len:9}` (683-689), forward reference `UnknownDeclaration{11,10}` (741-750); control: a completed family embedded negatively checks (811-862), signature len 12 (813).

## 12. tests/indexed_vector.rs

Length-indexed vector: one test (133-299) rejecting cons at wrong index (`same_length`, `zero`), nil at `succN n`, `iindW` at mismatched index, a `next` grabbing `fst p : Elem` where Nat needed (captured), Strict-branching, `w=1` motive universe. Contexts `vector_context` 89, `eliminator_context` 106.

## 13. tests/integer_order_weakening.rs

One test (39-60): `IntegerOrderWeakening` from `Equal` between two Booleans, or between u16 and u8 values, is rejected. Keyed on `scalar_type()` equality (proof.rs:1213-1216).

## 14. tests/nonconstructive_existence.rs

Kernel legs of nonconstructive-existence control (header 1-33): declarations `U32`=0, `choice`=1 (assumption, level arity 1), `subset`=2, `chosen`=3, `transfer`=4 (a proved squash-composition at level arity 3) (52-61, 329-339). Test 1 (365-486): `transfer` closure is empty (`judgment_assumption_closure`, 376-381); `transfer ... (sq w) step` type checks at the strict `Squash` target and weak-head normalises to the neutral spine `step (fst w) (unbox ...)` (440-471). Test 2 (488-605): `unsq` into relevant `Subset P` gives `SquashTargetNotStrict` (508-511), `fst` on squash gives `NotAPair` (519-522); the derivable `Squash (Subset P) -> Squash U32` checks with closure `{U32}` only (600-604).

## 15. tests/predicate_conversion.rs

Two tests (23-74): `PredicateDenotation` conversion rejects wrong polarity, wrong operand, wrong integer carrier, and an unproved child (24-55); and cannot bypass the depth limit (65-deep `BooleanNot`, 58-74).

## 16. tests/predicate_denotation.rs

Tests of `check_predicate_denotations` (no value equalities): data identity reflexive vs distinct subjects (16-60); closed IEEE order preserves NaN/signed zero/format (63-163, "unknown IEEE values are not reflexive" 161); nonliteral Boolean equality never asserts either operand (188-214); malformed operand types and context changes (217-251); conjunction conversion preserves falsehood and exact premise order (254-275); bounds on depth, total premises (2500+2500) and Boolean expansion (14 nested equalities) (278-304); exhaustive check of 624 small Boolean expressions x polarity x orientation x 4 assignments against a test evaluator (389-434, `assert_eq!(expressions.len(), 624)` 410); IEEE and mathematical-integer atoms preserved without new laws (437-496).

## 17. tests/source_checked_authority.rs

One test (8-31): `verify_obligation` with `EvidenceRoute::SourceChecked` on `Truth` or `Falsehood` gives `EvidenceError::SourceCheckedOutsideOperation`. Comment says "after the unused exported admission shortcut is retired" (12). 31 lines.

## 18. tests/suite.rs

Single test binary: `mod common;` plus 15 topic modules (5-19). Comment: "A new `tests/<topic>.rs` joins by one `mod` line". All 15 non-common topics in my unit are listed (contradiction_elimination, equality_symmetry, indexed_* x5, integer_order_weakening, nonconstructive_existence, predicate_conversion, predicate_denotation, source_checked_authority, theorem_certificate_admission, value_equality_transport).

## 19. tests/theorem_certificate_admission.rs

Header (1-22): theorem `equal_symmetric(a: u64, b: u64) requires a == b ensures b == a` shipped as `EqualitySymmetry{Assumption 0}`; `verify_obligation` admits and the core judges. Tests: judged receipt `{declarations 3, assumption_closure 3, context_depth 1, arena_slots 34}` (115-129); wrong goal `CertificateConclusionMismatch` (133-140); wrong proof `AssumptionConclusionMismatch(0)` (143-155); kernel rejects forged goal orientation, forged refl proof, and forged closure (dropped declaration gives `UnknownDeclaration{2,2}`) (158-252); `a == b` cited where lifted `IntegerMathEqual` is the goal is judged (255-265). Pins `arena_slots: 34`.

## 20. tests/value_equality_transport.rs

Tests on `ValueEqualityTransport`: byte-content equations need the exact observation and evidence (25-58); ambient equations cannot replace missing or unproved children (5 mutations, 100-129); guarded (implication) or reversed equations are not definitions, but the symmetric form accepted via `EqualitySymmetry` (132-171); cyclic and expanding equation lists fail (174-193); borrowed equations bounded (infinite `repeat(&equation)` terminates with `ResourceLimitExceeded`, 196-216); `mod float_fields` (218-454): store/read equations through the same exact field (285-322), conditional/wrong identity/wrong format equations cannot rewrite a field (325-353), cycles (356-378), bit-identity vs IEEE equality for NaN payloads and signed zero (381-453).

---

## 21. src/boundary.rs

**Purpose.** Descriptors for boundary proof obligations: what kind of boundary (`Host`, `CorePrimitive`, `TargetIntrinsic`, 11-16), whether it must `Establish` or `Preserve` a fact (29-33), and a set that can list unmet preservations and render diagnostic strings (83-134). Header 1-6.
**Shape.** `BoundaryKind` 11, `BoundaryObligationMode` 29, `BoundaryProofObligation{kind, mode, fact: Identifier, operation: Identifier}` 45 (`establishes` 53, `preserves` 62, `describe` 71), `BoundaryObligationSet{obligations}` 83 (`push`, `established`, `preserved`, `is_empty`, `unmet_preservations` 112, `missing_fact_diagnostics` 120). Four unit tests 137-212.
**Inputs/outputs.** Uses `typed_trees::name::Identifier` only. Pure data; no reads of other tables.
**How it decides.** `unmet_preservations` keeps a `Preserve` obligation if its `fact` identifier is not in the `available` slice (`available.contains(&obligation.fact)`, 117): keyed on identifier equality.
**Unused / one-caller.** `grep -c` of `BoundaryProofObligation|BoundaryObligationSet|BoundaryKind|BoundaryObligationMode` across `omega-rust`: 42 occurrences in 1 file (boundary.rs itself). The module is `pub mod boundary;` (proof/src/lib.rs:33) and lib.rs:28 describes it as "reusable proof vocabulary and boundary contracts". So these types have no users outside their own file in `omega-rust` (grep). (Other trees such as `source/` not searched.)
**Stated limits.** none. **Fail-open risks.** none seen (the set reports nothing for `Establish`-only boundaries by design; test 203-211).

---

## Coverage

- Read in full: all 21 files, 15,736 lines in total (3675 + 1350 + 4137 + 521 + 175 + 32 + 990 + 514 + 544 + 418 + 863 + 299 + 60 + 605 + 74 + 496 + 31 + 20 + 265 + 454 + 213).
- Skimmed: none. Not read: none.
- Also read for tracing: proof/src/checker.rs:2345-2385 and 5675-5700 (small windows).

## Five things a maintainer most needs to know about this unit

1. `mathematical_typing.rs` is one explicit-stack driver (`drive`, typing:2284) with about 60 continuation frames and a parallel conversion work enum (`comparison::Work`, 630); typing/inference/check/conversion are all one machine, and the only external consumers are `mathematical_core.rs` (177 type-operation uses, 1 `convert_retaining` use) and `mathematical_declarations.rs` (67 uses).
2. Conversion returns `true` for any two terms whenever the shared type's sort is Strict (typing:1375-1378), and `convert` never checks that the two terms inhabit the shared type; callers carry that obligation.
3. `accept_certificate` can succeed with `MathematicalCoreDecision::Refused(family)` (proof:238-240); the only non-test reader found, `proof/src/checker.rs:5692`, just counts refusals.
4. The per-rule relation functions in proof.rs (`equality_rules::*_relation`, `integer_order_rules::*`, `integer_bound_rules::*_relation`, `add_order`, `subtract_order`, `subtract_antitone`, `strict_order_transitivity`, `order_discreteness`) are `pub(crate)` and are reused by `bounded_denotation.rs` (grep lines 7113-8513), so a change there changes both checkers.
5. Budgets are hard-coded: predicate denotation 4096 nodes and depth 64 (pred:31, 35); several tests pin exact step/slot counts (`4_877` in indexed_derivation.rs:728, `arena_slots: 34` in theorem_certificate_admission.rs:127, `declarations.len() == 10` in indexed_levels.rs:352, `expressions.len() == 624` in tests/predicate_denotation.rs:410).

## What you are unsure about

- Whether `mathematical_core.rs:1311` (the one external `convert_retaining` caller) always passes a shared type that the two terms inhabit (not read).
- Whether `bounded_denotation.rs` really mirrors every rule decision in proof.rs or only re-calls the relation functions (grep shows the calls, I did not read the surrounding code).
- Whether any consumer in `descriptors.rs` rejects a `Refused` core decision (grep for `MathematicalCoreDecision` outside admission/tests/proof.rs found only checker.rs; descriptors.rs uses `accept_certificate` ten times but I did not read those sites).
- `boundary.rs` users were searched only under `omega-rust`; `source/` (Omega-written compiler) was not searched.
- `Context::is_empty` and several `pub` helpers in typing.rs (`level_arity`, `signature`) were not individually grepped for call counts.
