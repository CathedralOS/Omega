# Record: unit 042, gap 021 (area 07-psi-semantics)

File: `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/bounded_denotation.rs`, lines 15101-17334 (2,234 lines).
Crate: proof-admission (`proofs/bounded`). This range is the tail of the file's `#[cfg(test)]` module plus a
separate `#[cfg(test)] pub(crate) mod test_utilities`. It contains no production code.

## 1. Test cases, lines 15101-17144 (inside the existing tests module)

**Purpose.** Unit tests for `denote_bounded_certificate` / `verify_bounded_certificate`, which turn a bounded
proof (`ProofNode`/`ProofRule`) into mathematical-kernel terms. Each test builds a proof, denotes it, checks the
shape of the kernel term (`Term::IdElim`, `Pair`, `CaseTwo`, `Refl`, and so on), then re-checks it with
`verify_mathematical_certificate`. Negative cases confirm the bounded checker refuses forged or malformed proofs.
Read in full.

**Shape.** The range begins inside the test that ends at 15139 (integer-order weakening, assertions on the
signature of 6 declarations and the assumption closure `{0..5}`, 15101-15125). Then, free-standing `#[test]`
functions: `integer_math_transitivity_becomes_identity_eliminations` (15142), `conversion_and_value_transport_use_existing_evidence_and_identity_elimination` (15205),
`order_substitution_becomes_a_named_law_citation` (15260), `a_decided_closed_equality_is_refl_in_the_kernel` (15330),
`a_closed_order_uses_only_the_fixed_numeral_law_closure` (15359), `disjunction_rules_become_tagged_sum_constructions` (15411),
`semantic_axioms_bind_after_the_assumption_roster` (15529), `forged_and_malformed_certificates_reject` (15576),
`the_equal_integer_math_crossing_denotes_identically` (15648), `the_closure_names_every_judgment_premise` (15681).
Nested `mod connectives` (15708-16832) with helper `predicate_denotation_elaboration` (15712) and 12 tests (closed-false relations to
`EmptyElim` 15752; Falsehood via J discriminant 15773; closed-true goals 15787; connective composition 15812; open Boolean expansion by `CaseTwo` 15854;
permuted/deduplicated/flattened/Truth-dropping conjunctions 15914-15978; permuted and Falsehood-dropping disjunctions 15981-16050; contravariant implication domain 16053;
wide 16,384-member connective 16074; failure and declaration order 16149; deep 4,096-node elaboration 16179; binder restoration 16233;
ambient evidence through case binders 16304; repeated disjunction tags 16398; deep closed inhabitants 16442;
tag preference in closed inhabitants 16519; free-variable scan 16594; nested 8,192-node logical cache 16664; cache keeps exact syntax 16730; cache arity and first-failure effects 16785).
Nested `mod content_algebra` (16833-17112): helpers `content_algebra` (16838, kind `CountedQuantity`, parameter "Byte"), `content_root` (16845), `content_context` (16849), `content_term` (16857),
`conservation` (16871), `assert_content_carrier_vocabulary` (16883); tests for J-composition transitivity in four endpoint orientations (16912),
reflexive conservation as `refl` (16979), rejection with `EqualityAlgebraMismatch`/`EqualityMiddleMismatch`/`EqualityConclusionMismatch` (17006), unchanged content transport (17057).
Two more tests outside the modules: `closed_primitive_order_does_not_assume_its_conclusion` (17115), `false_closed_order_is_rejected_before_derivation` (17132).
Test count in 15101-end: 40 `#[test]` (command: `awk 'NR>=15101 && /#\[test\]/{c++}'`, equals the 7 greps at lines 16784-17131 plus the earlier ones). The file as a whole has 122 (crate map).

**Inputs and outputs.** Reads internal items `Denotation` (struct at 3821), `Elaboration` (struct at 7010), `Denotation::tagged_sum` (5464), `closed_inhabitant` (6569),
`occurs_free` (6745), plus `semantic_vocabulary::{Proposition, ScalarTerm, ContentTerm...}`, `mathematical_kernel::{Term, Budget, infer_sort, verify_mathematical_certificate}`.
Helpers `budget()`, `value()`, `literal()`, `atom()`, `unsigned64_type()`, `boolean_value()` and the `v!` macro are defined before line 15101 (not in this range; `v!` is re-defined at 17158 in `test_utilities`).
Produces only assertions; no state is written outside local values.

**How it decides.** Assertions key on term constructor shape (`matches!(denoted.arena.get(..), Term::IdElim{..})`, 15165) and on signature length and exact assumption-index sets
(e.g. `BTreeSet::from([0, 1, 2, 4, 6, 8, 10, 12, 13])` at 15374), so the tests pin the numbering of the fixed numeral vocabulary. Error cases key on `ProofError` variants (`UnknownAssumption(1)` 15591,
`AssumptionConclusionMismatch(0)` 15599, `CertificateConclusionMismatch` 15613) and on the `&'static str` payload `RuleConclusionMismatch("conjunction arity")` (16141, 16165), so a spelling change of that string would break them.
Tamper tests: replace the proof term with `Term::Variable(9)` and expect `CoreError::UnboundVariable` (15636-15643); clear a signature body or change an operand and expect kernel rejection (15378-15393).

**Cases by shape.** Fixed lists in the tests: closed-false relations (15753-15759: five specific inequalities); disjunction arities 2 and 3 with indices (15445-15449); eight tag/branch combinations for
closed inhabitants (16530-16537); depth values `[(8192,false),(8192,true),(64,true)]` (16446). Comments state "dependent families keep their fallback" (16577): a `Pi`/`Sigma` whose codomain depends on the bound variable gets no closed inhabitant.

**Same job elsewhere.** The assert-and-forge helpers in `test_utilities` (17302-17333) do the same job as `forged_and_malformed_certificates_reject` (15576): refuse or tamper a denotation. Other test families in
`integer_rules.rs:3133` use the helpers (see section 2). Suspected, not read: `proof.rs` has its own `assumption_proof` used here via `crate::proof::assumption_proof` (17149).

**Unused or one-caller code.** None in this section; test functions only.

**Stated limits.** "closed inhabitants ... false or dependent branches are not closed by this producer" (16561); "genuinely dependent families keep their fallback" (16577); "overflowing body indices stay ineligible" (16651).

**Fail-open risks.** None seen. Tests assert refusals; no production decision is in this range. The deep-stack tests run on spawned threads (16075, 16180, 16443, 16665) with comments that they stay on the default stack; this is a test harness choice (stated at 16145, 16229, 16515, 16726).
Odd formatting: the body of `wide_connective_denotation_uses_default_stack_and_preserves_order` (16074-16146) is indented less than the surrounding code (read; not rustfmt-style indentation).
Debug `eprintln!` calls remain in four tests (16092, 16099, 16206, 16467 and others).

## 2. `test_utilities` module, lines 17146-17334

**Purpose.** Shared test helpers for this file and sibling test code. `#[cfg(test)] pub(crate) mod test_utilities` (17146-17147).

**Shape.**
- `macro_rules! v` (17158): six-argument wrapper for `verify_bounded_certificate`.
- `denote_via_assumption_premise` (17164, pub(crate)): proves goal by `PredicateDenotation` over premise assumption 0, calls `expect(message)`.
- `mathematical` (17180, pub(crate)): converts a `ScalarTerm::Value` of integer type or `ScalarTerm::Integer` into `IntegerMathTerm`; other terms hit `unreachable!()` (17191).
- `open_operation_preserves_skipped_numeric_resource_refusals` (17194, pub(crate)): given a binary `IntegerMathTerm` constructor, builds an open term over a `1 << 65_536` literal; asserts the open goal verifies (17214) but the closed form
  `large == large` is refused (17218-17224), so the 65,536-bit result limit still refuses when the term is closed.
- `direct_fixture` (17227, pub(super)), `verify_then_elaborate` (17264), `elaborate_checked` (17282): verify then run `Elaboration::node`, returning the `Denotation`/`Elaboration` for inspection.
- `direct_affine_proof` (17242, pub(crate)): builds an `IntegerAffineBound` proof with an empty-witness `affine_witness`.
- `assert_refused` (17302, pub(crate)): asserts `verify_bounded_certificate(...)` is `Err`.
- `verify_and_assert_forged` (17315, pub(crate)): verifies, then calls `crate::forge_term_and_assert_rejected(&mut denoted)` (17331).

**Inputs and outputs.** Takes contexts, propositions and proofs, uses `Budget::default()`; returns denotations or panics on failure.

**How it decides.** `assert_refused` keys only on `is_err()` (17309-17312), any error variant counts as refusal, so a test using it would not distinguish the reason. `mathematical` keys on the term variant and `ScalarType::Integer` (17182-17190).

**Same job elsewhere / unused code.** Grep (count mode) for `test_utilities::` under `proofs/bounded`: 14 occurrences in 2 files: 13 in `bounded_denotation.rs`, 1 in `integer_rules.rs` (line 3133, imports `assert_refused`, `verify_and_assert_forged`).
Per-helper grep for the other pub(crate) helpers was not run; `direct_fixture`, `verify_then_elaborate` and `elaborate_checked` are `pub(super)` and so are reachable only from the sibling tests module in this file.
`crate::forge_term_and_assert_rejected` is defined outside this range and was not read.

**Stated limits.** None in comments. **Fail-open risks.** None seen (test code only); the `unreachable!()` at 17191 panics, it does not accept.

## Coverage
- Read in full: `bounded_denotation.rs` 15101-17334 (2,234 lines).
- Skimmed: none. Not read: none within the unit.
- Definitions of `Denotation`, `Elaboration`, `budget()`, `value()` etc. outside the range were located by grep only (line numbers at 3821, 7010, 5464, 6569, 6745), not read.

## Five things a maintainer most needs to know about this unit
1. The whole range is test code (40 tests plus a helper module); nothing here is shipped, so it records expected behaviour of denotation rather than implementing any.
2. Tests pin exact kernel-signature sizes and assumption-index sets (e.g. 14 declarations and indices `[0,1,2,4,6,8,10,12,13]` for a closed `<`, 15371-15375), so adding or reordering vocabulary declarations breaks them.
3. Several error assertions match the string payload `"conjunction arity"` (16141, 16165), including for the disjunction case at 16141 where `expected` is chosen, but 16165 expects "conjunction arity" even when `disjunctive` is true.
4. Four tests spawn threads to prove deep (4,096 to 16,384 nodes) denotation fits the default stack; they print progress with `eprintln!`.
5. `assert_refused` accepts any error; the specific-error tests are the ones that use `matches!` on `BoundedDenotationError`.

## What you are unsure about
- Whether `forge_term_and_assert_rejected` really rejects every forged term; it lies outside the range.
- Whether the sibling tests at 15101 start mid-test: I inferred the enclosing test's start from the closing brace at 15139; its name is before the range.
- Item 3 above: the 16165 expectation holds for the `disjunctive == true` loop iteration because `Denotation::disjunction` apparently reports the same string for the malformed member; I did not read `disjunction` to confirm.
