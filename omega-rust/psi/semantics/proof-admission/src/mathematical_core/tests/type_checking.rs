use super::{
    apply, branching_family, case_two, default_budget, fst, id, lambda, pair, pi,
    polymorphic_identity, refl, sigma, snd, strict_sort, two, two_one, two_zero, type_sort,
    universe_motive, variable,
};
use crate::mathematical_core::{
    Budget, Context, CoreError, Level, Signature, Sort, TermArena, check_type, convertible,
    infer_sort, infer_type, substitute, weak_head_normalize,
};

#[test]
fn sorts_inhabit_the_next_relevant_universe() {
    let mut arena = TermArena::new();
    let context = Context::empty();
    let mut budget = default_budget();

    let type_three = type_sort(&mut arena, 3);
    let expected = type_sort(&mut arena, 4);
    let inferred = infer_type(&mut arena, &context, type_three, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));

    let strict_two = strict_sort(&mut arena, 2);
    let expected = type_sort(&mut arena, 3);
    let inferred = infer_type(&mut arena, &context, strict_two, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));

    // There is no self-typing universe: `Type 0 : Type 0` rejects.
    let type_zero = type_sort(&mut arena, 0);
    let error = check_type(&mut arena, &context, type_zero, type_zero, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn dependent_function_type_takes_the_level_maximum() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();
    let type_zero = type_sort(&mut arena, 0);

    // Π(A : Type 0). A : Type 1, because `Type 0` itself lives at Type 1.
    let body = variable(&mut arena, 0);
    let function_type = pi(&mut arena, type_zero, body);
    let inferred = infer_type(&mut arena, &empty, function_type, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // Π(A : Type 0). Π(x : A). A : Type 1.
    let inner_domain = variable(&mut arena, 0);
    let inner_codomain = variable(&mut arena, 1);
    let inner_pi = pi(&mut arena, inner_domain, inner_codomain);
    let outer_pi = pi(&mut arena, type_zero, inner_pi);
    let inferred = infer_type(&mut arena, &empty, outer_pi, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));

    // Context A : Type 0, P : Strict 0. In depth 2, A is index 1 and P is
    // index 0; under the Π binder, P is index 1.
    let strict_zero = strict_sort(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(strict_zero);
    let domain = variable(&mut arena, 1);
    let codomain = variable(&mut arena, 1);
    let strict_function = pi(&mut arena, domain, codomain);
    let inferred = infer_type(&mut arena, &context, strict_function, &mut budget).unwrap();
    let expected = strict_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // Π(A : Type 2). Strict 0 : Type 3 — `Strict 0 : Type 1`, max(3, 1) = 3.
    let type_two = type_sort(&mut arena, 2);
    let mixed = pi(&mut arena, type_two, strict_zero);
    let inferred = infer_type(&mut arena, &empty, mixed, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 3);
    assert!(arena.structurally_equal(inferred, expected));
}

#[test]
fn polymorphic_identity_checks_and_applies() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();
    let (identity, identity_type) = polymorphic_identity(&mut arena);

    check_type(&mut arena, &empty, identity, identity_type, &mut budget).unwrap();

    // Context N : Type 0, n : N. In depth 2, N is index 1 and n is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let n_type = variable(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(n_type);
    let type_argument = variable(&mut arena, 1);
    let witness = variable(&mut arena, 0);
    let partially_applied = apply(&mut arena, identity, type_argument);
    let applied = apply(&mut arena, partially_applied, witness);
    let inferred = infer_type(&mut arena, &context, applied, &mut budget).unwrap();
    let expected = variable(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // Applying the identity to `n`, a non-type, rejects at the domain.
    let bad_argument = apply(&mut arena, identity, witness);
    let error = infer_type(&mut arena, &context, bad_argument, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::ArgumentTypeMismatch { .. }));

    // `n` is not a function.
    let not_a_function = apply(&mut arena, witness, witness);
    let error = infer_type(&mut arena, &context, not_a_function, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAFunction { .. }));

    // A Π domain must be a type; `n` is not.
    let malformed = pi(&mut arena, witness, type_zero);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));
}

#[test]
fn substitution_shifts_free_variables_under_binders() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A. In depth 2, A is index 1 and a is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let a_type = variable(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(a_type);

    // (λ(x : A). λ(y : A). x) a → λ(y : A). a, with the free `a` shifted.
    let inner_domain = variable(&mut arena, 2);
    let inner_body = variable(&mut arena, 1);
    let inner = lambda(&mut arena, inner_domain, inner_body);
    let outer_domain = variable(&mut arena, 1);
    let outer = lambda(&mut arena, outer_domain, inner);
    let argument = variable(&mut arena, 0);
    let redex = apply(&mut arena, outer, argument);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), redex, &mut budget).unwrap();

    // The correct answer: domain A (index 1), body the shifted a (index 1).
    let expected_domain = variable(&mut arena, 1);
    let expected_body = variable(&mut arena, 1);
    let expected = lambda(&mut arena, expected_domain, expected_body);
    assert!(arena.structurally_equal(normalized, expected));

    // The captured wrong answer λ(y : A). y must NOT convert at Π(y : A). A.
    let captured_body = variable(&mut arena, 0);
    let captured_domain = variable(&mut arena, 1);
    let captured = lambda(&mut arena, captured_domain, captured_body);
    let shared_domain = variable(&mut arena, 1);
    let shared_codomain = variable(&mut arena, 2);
    let shared_type = pi(&mut arena, shared_domain, shared_codomain);
    assert!(
        convertible(
            &mut arena,
            &context,
            normalized,
            expected,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
    assert!(
        !convertible(
            &mut arena,
            &context,
            captured,
            expected,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
}

#[test]
fn strict_proofs_convert_and_relevant_witnesses_stay_distinct() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, p : P, q : P, A : Type 0, x : A, y : A.
    // In depth 6: P is 5, p is 4, q is 3, A is 2, x is 1, y is 0.
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let q_binding = variable(&mut arena, 1);
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let y_binding = variable(&mut arena, 1);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(p_binding)
        .extend(q_binding)
        .extend(type_zero)
        .extend(x_binding)
        .extend(y_binding);

    let proposition = variable(&mut arena, 5);
    let first_proof = variable(&mut arena, 4);
    let second_proof = variable(&mut arena, 3);
    let relevant_type = variable(&mut arena, 2);
    let first_witness = variable(&mut arena, 1);
    let second_witness = variable(&mut arena, 0);

    // Two strict proofs of the same proposition convert.
    assert!(
        convertible(
            &mut arena,
            &context,
            first_proof,
            second_proof,
            proposition,
            &mut budget,
        )
        .unwrap()
    );

    // Distinct relevant witnesses do not collapse.
    assert!(
        !convertible(
            &mut arena,
            &context,
            first_witness,
            second_witness,
            relevant_type,
            &mut budget,
        )
        .unwrap()
    );

    // Two distinct functions into a strict proposition also convert: the
    // shared type Π(z : A). P has sort Strict 0, so irrelevance applies.
    let first_domain = variable(&mut arena, 2);
    let first_body = variable(&mut arena, 5);
    let first_function = lambda(&mut arena, first_domain, first_body);
    let second_domain = variable(&mut arena, 2);
    let second_body = variable(&mut arena, 4);
    let second_function = lambda(&mut arena, second_domain, second_body);
    let function_domain = variable(&mut arena, 2);
    let function_codomain = variable(&mut arena, 6);
    let strict_function_type = pi(&mut arena, function_domain, function_codomain);
    assert!(
        convertible(
            &mut arena,
            &context,
            first_function,
            second_function,
            strict_function_type,
            &mut budget,
        )
        .unwrap()
    );
}

#[test]
fn unbound_variable_rejects() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let type_zero = type_sort(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(type_zero);
    let unbound = variable(&mut arena, 3);
    let error = infer_type(&mut arena, &context, unbound, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundVariable {
            index: 3,
            context_depth: 2,
        }
    );
}

#[test]
fn conversion_refuses_at_the_step_ceiling() {
    let mut arena = TermArena::new();

    // Ω = (λ(x : T). x x)(λ(x : T). x x) with a fake domain T = Type 0. It is
    // ill-typed, so drive `weak_head_normalize` directly: it must refuse at
    // the ceiling instead of looping.
    let fake_domain = type_sort(&mut arena, 0);
    let self_argument = variable(&mut arena, 0);
    let self_function = variable(&mut arena, 0);
    let self_apply = apply(&mut arena, self_function, self_argument);
    let delta = lambda(&mut arena, fake_domain, self_apply);
    let omega = apply(&mut arena, delta, delta);

    let mut budget = Budget::new(8);
    let error = weak_head_normalize(&mut arena, &Signature::new(), omega, &mut budget).unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);
    assert_eq!(budget.remaining(), 0);
}

#[test]
fn checking_the_polymorphic_identity_retains_bounded_storage() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();
    let (identity, identity_type) = polymorphic_identity(&mut arena);
    check_type(&mut arena, &empty, identity, identity_type, &mut budget).unwrap();

    let type_zero = type_sort(&mut arena, 0);
    let n_type = variable(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(n_type);
    let type_argument = variable(&mut arena, 1);
    let witness = variable(&mut arena, 0);
    let partially_applied = apply(&mut arena, identity, type_argument);
    let applied = apply(&mut arena, partially_applied, witness);
    let expected_type = variable(&mut arena, 1);
    check_type(&mut arena, &context, applied, expected_type, &mut budget).unwrap();

    // A substitution that does not reach a bound variable returns the same
    // handle and retains storage: nothing is allocated for unchanged terms.
    let closed = pi(&mut arena, type_zero, type_zero);
    let argument = variable(&mut arena, 0);
    assert_eq!(substitute(&mut arena, closed, argument), closed);

    // Retained-storage receipt: the whole construction and every check above
    // live in a small bounded arena.
    assert_eq!(arena.len(), 44);
}

#[test]
fn dependent_pair_type_tracks_component_sorts() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();
    let type_zero = type_sort(&mut arena, 0);

    // Σ(A : Type 0). A : Type 1, mirroring dependent function formation.
    let body = variable(&mut arena, 0);
    let pair_type = sigma(&mut arena, type_zero, body);
    let inferred = infer_type(&mut arena, &empty, pair_type, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // Context A : Type 0, P : Strict 0, Q : Strict 0. In depth 3, A is
    // index 2, P is index 1 and Q is index 0.
    let strict_zero = strict_sort(&mut arena, 0);
    let context = Context::empty()
        .extend(type_zero)
        .extend(strict_zero)
        .extend(strict_zero);

    // Σ(x : A). Q : Type 0 — the first component is relevant data, so the
    // pair is not a subsingleton even though the codomain is a proposition.
    let domain = variable(&mut arena, 2);
    let codomain = variable(&mut arena, 1);
    let mixed = sigma(&mut arena, domain, codomain);
    let inferred = infer_type(&mut arena, &context, mixed, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // Σ(p : P). Q : Strict 0 — a conjunction of propositions is a
    // proposition. Only an all-strict pair collapses.
    let strict_domain = variable(&mut arena, 1);
    let strict_codomain = variable(&mut arena, 1);
    let conjunction = sigma(&mut arena, strict_domain, strict_codomain);
    let inferred = infer_type(&mut arena, &context, conjunction, &mut budget).unwrap();
    let expected = strict_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));
}

#[test]
fn dependent_pairs_introduce_and_project() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, P : Π(x : A). Strict 0, a : A, proof : P a.
    // In depth 4: A is index 3, P is index 2, a is index 1, proof is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let a_in_prefix = variable(&mut arena, 0);
    let predicate_type = pi(&mut arena, a_in_prefix, strict_zero);
    let a_type = variable(&mut arena, 1);
    let p_in_prefix = variable(&mut arena, 1);
    let a_in_prefix = variable(&mut arena, 0);
    let proof_type = apply(&mut arena, p_in_prefix, a_in_prefix);
    let context = Context::empty()
        .extend(type_zero)
        .extend(predicate_type)
        .extend(a_type)
        .extend(proof_type);

    // Σ(x : A). P x — under the binder, P is index 3 and x is index 0.
    let sigma_domain = variable(&mut arena, 3);
    let p_under = variable(&mut arena, 3);
    let x_under = variable(&mut arena, 0);
    let px_under = apply(&mut arena, p_under, x_under);
    let dependent_sigma = sigma(&mut arena, sigma_domain, px_under);

    // (a, proof) checks at Σ(x : A). P x only through the componentwise
    // path: the inferred non-dependent Σ(x : A). P a never converts.
    let a_term = variable(&mut arena, 1);
    let proof_term = variable(&mut arena, 0);
    let witness = pair(&mut arena, a_term, proof_term);
    check_type(&mut arena, &context, witness, dependent_sigma, &mut budget).unwrap();

    // fst (a, proof) : A and computes to a.
    let projected_first = fst(&mut arena, witness);
    let inferred = infer_type(&mut arena, &context, projected_first, &mut budget).unwrap();
    let a_in_full = variable(&mut arena, 3);
    assert!(arena.structurally_equal(inferred, a_in_full));
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), projected_first, &mut budget).unwrap();
    let a_term = variable(&mut arena, 1);
    assert!(arena.structurally_equal(normalized, a_term));

    // snd (a, proof) : P (fst (a, proof)) — the dependent result type
    // retains the projected first component and converts to P a.
    let projected_second = snd(&mut arena, witness);
    let inferred = infer_type(&mut arena, &context, projected_second, &mut budget).unwrap();
    let p_full = variable(&mut arena, 2);
    let a_full = variable(&mut arena, 1);
    let expected_second_type = apply(&mut arena, p_full, a_full);
    assert!(
        convertible(
            &mut arena,
            &context,
            inferred,
            expected_second_type,
            strict_zero,
            &mut budget,
        )
        .unwrap()
    );
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), projected_second, &mut budget).unwrap();
    let proof_term = variable(&mut arena, 0);
    assert!(arena.structurally_equal(normalized, proof_term));

    // A wrong first component and a non-proof second component reject.
    let wrong_second = pair(&mut arena, a_term, a_term);
    let error = check_type(
        &mut arena,
        &context,
        wrong_second,
        dependent_sigma,
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn projections_reduce_through_function_redexes() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A. In depth 2, A is index 1 and a is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let a_type = variable(&mut arena, 0);
    let _context = Context::empty().extend(type_zero).extend(a_type);

    // fst ((λ(x : A). (x, x)) a) → fst (a, a) → a.
    let x_domain = variable(&mut arena, 1);
    let x = variable(&mut arena, 0);
    let duplicate = pair(&mut arena, x, x);
    let duplicator = lambda(&mut arena, x_domain, duplicate);
    let a_term = variable(&mut arena, 0);
    let redex_pair = apply(&mut arena, duplicator, a_term);
    let projected = fst(&mut arena, redex_pair);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), projected, &mut budget).unwrap();
    let expected = variable(&mut arena, 0);
    assert!(arena.structurally_equal(normalized, expected));
}

#[test]
fn pair_eta_converts_a_neutral_with_its_projections() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, w : Σ(x : A). A. In depth 2, A is index 1.
    let type_zero = type_sort(&mut arena, 0);
    let a_domain = variable(&mut arena, 0);
    let a_codomain = variable(&mut arena, 1);
    let sigma_type = sigma(&mut arena, a_domain, a_codomain);
    let context = Context::empty().extend(type_zero).extend(sigma_type);

    // w ≡ (fst w, snd w) and (fst w, snd w) ≡ w at Σ(x : A). A. Under the
    // Σ binder (depth 3), A is index 2.
    let shared_domain = variable(&mut arena, 1);
    let shared_codomain = variable(&mut arena, 2);
    let shared_type = sigma(&mut arena, shared_domain, shared_codomain);
    let w = variable(&mut arena, 0);
    let w_first = fst(&mut arena, w);
    let w_second = snd(&mut arena, w);
    let reconstructed = pair(&mut arena, w_first, w_second);
    assert!(
        convertible(
            &mut arena,
            &context,
            w,
            reconstructed,
            shared_type,
            &mut budget
        )
        .unwrap()
    );
    assert!(
        convertible(
            &mut arena,
            &context,
            reconstructed,
            w,
            shared_type,
            &mut budget
        )
        .unwrap()
    );

    // A different neutral pair does not collapse into w.
    let domain_again = variable(&mut arena, 1);
    let codomain_again = variable(&mut arena, 2);
    let sigma_again = sigma(&mut arena, domain_again, codomain_again);
    let context = context.extend(sigma_again);
    let w_other = variable(&mut arena, 0);
    let w_inner = variable(&mut arena, 1);
    let shared_domain = variable(&mut arena, 2);
    let shared_codomain = variable(&mut arena, 3);
    let shared_type = sigma(&mut arena, shared_domain, shared_codomain);
    assert!(
        !convertible(
            &mut arena,
            &context,
            w_inner,
            w_other,
            shared_type,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn mixed_pairs_collapse_only_their_proof_components() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, P : Strict 0, a : A, b : A, p : P, q : P.
    // In depth 6: A is 5, P is 4, a is 3, b is 2, p is 1, q is 0.
    let type_zero = type_sort(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    // Each binding is well-scoped in its own prefix: A is index 1 in
    // [Type_0, Strict_0], index 2 once `a` is bound; P is index 3 then 4.
    let a_type_two = variable(&mut arena, 1);
    let a_type_three = variable(&mut arena, 2);
    let p_type_four = variable(&mut arena, 3);
    let p_type_five = variable(&mut arena, 4);
    let context = Context::empty()
        .extend(type_zero)
        .extend(strict_zero)
        .extend(a_type_two)
        .extend(a_type_three)
        .extend(p_type_four)
        .extend(p_type_five);

    // Σ(x : A). P is relevant (first component is data), so two packages
    // sharing only their relevant first component are not convertible;
    // once the data agrees, the proof components collapse by irrelevance.
    // Under the Σ binder (depth 7), P is index 5.
    let domain = variable(&mut arena, 5);
    let codomain = variable(&mut arena, 5);
    let shared_type = sigma(&mut arena, domain, codomain);

    let a_term = variable(&mut arena, 3);
    let p_term = variable(&mut arena, 1);
    let first_package = pair(&mut arena, a_term, p_term);
    let a_again = variable(&mut arena, 3);
    let q_term = variable(&mut arena, 0);
    let same_data = pair(&mut arena, a_again, q_term);
    assert!(
        convertible(
            &mut arena,
            &context,
            first_package,
            same_data,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );

    let b_term = variable(&mut arena, 2);
    let q_again = variable(&mut arena, 0);
    let different_data = pair(&mut arena, b_term, q_again);
    assert!(
        !convertible(
            &mut arena,
            &context,
            first_package,
            different_data,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
}

#[test]
fn projections_and_pairs_reject_ill_typed_uses() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A. In depth 2, A is index 1 and a is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let a_type = variable(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(a_type);

    // fst a is not a projection: a is not a pair.
    let a_term = variable(&mut arena, 0);
    let bad_projection = fst(&mut arena, a_term);
    let error = infer_type(&mut arena, &context, bad_projection, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAPair { .. }));

    // (a, a) cannot check at A: it is a pair type, not the domain itself.
    let a_first = variable(&mut arena, 0);
    let a_second = variable(&mut arena, 0);
    let misplaced = pair(&mut arena, a_first, a_second);
    let a_expected = variable(&mut arena, 1);
    let error = check_type(&mut arena, &context, misplaced, a_expected, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn dependent_pairs_flow_through_call_arguments() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, P : Π(x : A). Strict 0, a : A, proof : P a,
    // f : Π(w : Σ(x : A). P x). A.
    // In depth 5: A is 4, P is 3, a is 2, proof is 1, f is 0.
    let type_zero = type_sort(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let a_in_prefix = variable(&mut arena, 0);
    let predicate_type = pi(&mut arena, a_in_prefix, strict_zero);
    let a_type = variable(&mut arena, 1);
    let p_in_prefix = variable(&mut arena, 1);
    let a_in_prefix = variable(&mut arena, 0);
    let proof_type = apply(&mut arena, p_in_prefix, a_in_prefix);

    // f's type in its own prefix [Type_0, Π(x:A).Strict_0, A, P a]:
    // Π(w : Σ(x : A). P x). A. A is index 3 in the length-4 prefix; under
    // the Σ binder (depth 5) P is index 3, and under the Π binder A is 4.
    let a_in_sigma_domain = variable(&mut arena, 3);
    let p_in_sigma = variable(&mut arena, 3);
    let x_in_sigma = variable(&mut arena, 0);
    let px_in_sigma = apply(&mut arena, p_in_sigma, x_in_sigma);
    let sigma_domain_type = sigma(&mut arena, a_in_sigma_domain, px_in_sigma);
    let a_result = variable(&mut arena, 4);
    let function_type = pi(&mut arena, sigma_domain_type, a_result);

    let context = Context::empty()
        .extend(type_zero)
        .extend(predicate_type)
        .extend(a_type)
        .extend(proof_type)
        .extend(function_type);

    // f (a, proof) : A — the argument is a dependent pair checked against
    // the Σ domain componentwise, which inference alone cannot establish.
    let f_term = variable(&mut arena, 0);
    let a_term = variable(&mut arena, 2);
    let proof_term = variable(&mut arena, 1);
    let argument = pair(&mut arena, a_term, proof_term);
    let call = apply(&mut arena, f_term, argument);
    let inferred = infer_type(&mut arena, &context, call, &mut budget).unwrap();
    let expected = variable(&mut arena, 4);
    assert!(arena.structurally_equal(inferred, expected));
}

#[test]
fn function_eta_converts_a_neutral_with_its_wrapper() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, f : Π(x : A). A. In depth 2, A is index 1 and f
    // is index 0. f's binding is well-scoped in its length-1 prefix:
    // domain A is index 0, codomain A under the binder is index 1.
    let type_zero = type_sort(&mut arena, 0);
    let domain_in_prefix = variable(&mut arena, 0);
    let codomain_in_prefix = variable(&mut arena, 1);
    let function_type = pi(&mut arena, domain_in_prefix, codomain_in_prefix);
    let context = Context::empty().extend(type_zero).extend(function_type);

    // Shared type Π(x : A). A in depth 2: domain A is index 1, codomain A
    // under the binder is index 2.
    let shared_domain = variable(&mut arena, 1);
    let shared_codomain = variable(&mut arena, 2);
    let shared_type = pi(&mut arena, shared_domain, shared_codomain);

    // λ(x : A). f x — under the binder f is index 1 and x is index 0.
    let lambda_domain = variable(&mut arena, 1);
    let f_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, f_under, bound);
    let wrapped = lambda(&mut arena, lambda_domain, body);
    let f = variable(&mut arena, 0);

    // x ↦ f x ≡ f and f ≡ x ↦ f x at the checked function type.
    assert!(convertible(&mut arena, &context, wrapped, f, shared_type, &mut budget,).unwrap());
    assert!(convertible(&mut arena, &context, f, wrapped, shared_type, &mut budget,).unwrap());

    // Add g : Π(x : A). A, a distinct neutral. In depth 3, A is index 2,
    // f is index 1 and g is index 0; under the binder f is index 2.
    let g_domain = variable(&mut arena, 1);
    let g_codomain = variable(&mut arena, 2);
    let g_type = pi(&mut arena, g_domain, g_codomain);
    let context = context.extend(g_type);
    let shared_domain = variable(&mut arena, 2);
    let shared_codomain = variable(&mut arena, 3);
    let shared_type = pi(&mut arena, shared_domain, shared_codomain);

    // Eta grants no pointwise collapse: x ↦ f x does not convert to g.
    let lambda_domain = variable(&mut arena, 2);
    let f_under = variable(&mut arena, 2);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, f_under, bound);
    let wrapped_f = lambda(&mut arena, lambda_domain, body);
    let g = variable(&mut arena, 0);
    assert!(!convertible(&mut arena, &context, wrapped_f, g, shared_type, &mut budget,).unwrap());

    // The same construction around g itself does convert.
    let lambda_domain = variable(&mut arena, 2);
    let g_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, g_under, bound);
    let wrapped_g = lambda(&mut arena, lambda_domain, body);
    assert!(convertible(&mut arena, &context, wrapped_g, g, shared_type, &mut budget,).unwrap());

    // A wrapper whose body drops the application is a different function:
    // x ↦ x is not f merely because both are functions.
    let lambda_domain = variable(&mut arena, 2);
    let bound = variable(&mut arena, 0);
    let identity = lambda(&mut arena, lambda_domain, bound);
    let f = variable(&mut arena, 1);
    assert!(!convertible(&mut arena, &context, identity, f, shared_type, &mut budget,).unwrap());
}

#[test]
fn function_eta_applies_across_admitted_sort_combinations() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, f : Π(p : P). A. In depth 3, P is
    // index 2, A is index 1 and f is index 0. A strict domain with a
    // relevant codomain is an admitted Π formation; the Π itself is
    // relevant, so eta — not irrelevance — decides this conversion.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let p_in_prefix = variable(&mut arena, 1);
    let a_under = variable(&mut arena, 1);
    let function_type = pi(&mut arena, p_in_prefix, a_under);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(function_type);

    // Shared type Π(p : P). A in depth 3: domain P is index 2, codomain A
    // under the binder is index 2.
    let shared_domain = variable(&mut arena, 2);
    let shared_codomain = variable(&mut arena, 2);
    let shared_type = pi(&mut arena, shared_domain, shared_codomain);
    let sort = infer_sort(&mut arena, &context, shared_type, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));

    // λ(p : P). f p — under the binder f is index 1 and p is index 0.
    let lambda_domain = variable(&mut arena, 2);
    let f_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, f_under, bound);
    let wrapped = lambda(&mut arena, lambda_domain, body);
    let f = variable(&mut arena, 0);
    assert!(convertible(&mut arena, &context, wrapped, f, shared_type, &mut budget,).unwrap());
    assert!(convertible(&mut arena, &context, f, wrapped, shared_type, &mut budget,).unwrap());

    // A dependent codomain keeps the same rule. Context
    // B : Type 0, Q : Π(y : B). Type 0, h : Π(y : B). Q y. In depth 3,
    // B is index 2, Q is index 1 and h is index 0.
    let b_in_prefix = variable(&mut arena, 0);
    let type_zero_body = type_sort(&mut arena, 0);
    let predicate_type = pi(&mut arena, b_in_prefix, type_zero_body);
    let b_domain = variable(&mut arena, 1);
    let q_under = variable(&mut arena, 1);
    let y_under = variable(&mut arena, 0);
    let qy_under = apply(&mut arena, q_under, y_under);
    let dependent_function_type = pi(&mut arena, b_domain, qy_under);
    let context = Context::empty()
        .extend(type_zero)
        .extend(predicate_type)
        .extend(dependent_function_type);

    // Shared type Π(y : B). Q y in depth 3: domain B is index 2; under the
    // binder Q is index 2 and y is index 0. Its sort is relevant Type 0:
    // the dependent codomain Q y inhabits Type 0, so the maximum is 0.
    let shared_domain = variable(&mut arena, 2);
    let q_shared = variable(&mut arena, 2);
    let y_shared = variable(&mut arena, 0);
    let qy_shared = apply(&mut arena, q_shared, y_shared);
    let shared_type = pi(&mut arena, shared_domain, qy_shared);
    let sort = infer_sort(&mut arena, &context, shared_type, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));

    // λ(y : B). h y ≡ h at the dependent Π.
    let lambda_domain = variable(&mut arena, 2);
    let h_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, h_under, bound);
    let wrapped = lambda(&mut arena, lambda_domain, body);
    let h = variable(&mut arena, 0);
    assert!(convertible(&mut arena, &context, wrapped, h, shared_type, &mut budget,).unwrap());
}

#[test]
fn function_eta_composes_inside_dependent_type_conversion() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, R : Π(h : Π(x : A). A). Type 0, f : Π(x : A). A.
    // In depth 3, A is index 2, R is index 1 and f is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let inner_domain = variable(&mut arena, 0);
    let inner_codomain = variable(&mut arena, 1);
    let inner_pi = pi(&mut arena, inner_domain, inner_codomain);
    let result_sort = type_sort(&mut arena, 0);
    let family_type = pi(&mut arena, inner_pi, result_sort);
    let f_domain = variable(&mut arena, 1);
    let f_codomain = variable(&mut arena, 2);
    let function_type = pi(&mut arena, f_domain, f_codomain);
    let context = Context::empty()
        .extend(type_zero)
        .extend(family_type)
        .extend(function_type);

    // R f ≡ R (x ↦ f x) as types at Type 0: the argument conversion runs
    // at R's Π domain, where the eta rule sees the function type.
    let r = variable(&mut arena, 1);
    let f = variable(&mut arena, 0);
    let direct = apply(&mut arena, r, f);
    let lambda_domain = variable(&mut arena, 2);
    let f_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, f_under, bound);
    let wrapped = lambda(&mut arena, lambda_domain, body);
    let r_again = variable(&mut arena, 1);
    let eta_expanded = apply(&mut arena, r_again, wrapped);
    let shared_type = type_sort(&mut arena, 0);
    assert!(
        convertible(
            &mut arena,
            &context,
            direct,
            eta_expanded,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );

    // A different function argument still distinguishes the types.
    let g_domain = variable(&mut arena, 2);
    let g_codomain = variable(&mut arena, 3);
    let g_type = pi(&mut arena, g_domain, g_codomain);
    let context = context.extend(g_type);
    let r = variable(&mut arena, 2);
    let f = variable(&mut arena, 1);
    let left = apply(&mut arena, r, f);
    let r_again = variable(&mut arena, 2);
    let g = variable(&mut arena, 0);
    let right = apply(&mut arena, r_again, g);
    assert!(!convertible(&mut arena, &context, left, right, shared_type, &mut budget,).unwrap());
}

#[test]
fn function_eta_never_deletes_an_untyped_wrapper() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, f : Π(x : A). A. In depth 2, A is index 1 and f
    // is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let domain_in_prefix = variable(&mut arena, 0);
    let codomain_in_prefix = variable(&mut arena, 1);
    let function_type = pi(&mut arena, domain_in_prefix, codomain_in_prefix);
    let context = Context::empty().extend(type_zero).extend(function_type);

    // The same wrapper that converted at Π(x : A). A does not collapse at
    // a non-function shared type: the rule is a conversion judgment at a
    // checked Π, never a syntactic `x ↦ f x` deletion.
    let lambda_domain = variable(&mut arena, 1);
    let f_under = variable(&mut arena, 1);
    let bound = variable(&mut arena, 0);
    let body = apply(&mut arena, f_under, bound);
    let wrapped = lambda(&mut arena, lambda_domain, body);
    let f = variable(&mut arena, 0);
    let neutral_type = variable(&mut arena, 1);
    assert!(!convertible(&mut arena, &context, wrapped, f, neutral_type, &mut budget,).unwrap());

    // Nor does an eta-expanded comparison succeed in the other direction.
    assert!(!convertible(&mut arena, &context, f, wrapped, neutral_type, &mut budget,).unwrap());
}

#[test]
fn two_forms_and_introduces() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();

    // Formation: `Two : Type 0`, a ground relevant type that does not
    // inhabit its own universe.
    let two_type = two(&mut arena);
    let inferred = infer_type(&mut arena, &empty, two_type, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // Introduction: `zero : Two` and `one : Two`.
    let zero = two_zero(&mut arena);
    let one = two_one(&mut arena);
    let zero_type = infer_type(&mut arena, &empty, zero, &mut budget).unwrap();
    let one_type = infer_type(&mut arena, &empty, one, &mut budget).unwrap();
    assert!(arena.structurally_equal(zero_type, two_type));
    assert!(arena.structurally_equal(one_type, two_type));

    // The constructors are definitionally distinct relevant data.
    assert!(convertible(&mut arena, &empty, zero, zero, two_type, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &empty, zero, one, two_type, &mut budget).unwrap());
}

#[test]
fn dependent_two_elimination_checks_at_branch_specific_types() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context t : Two. The family C above gives `C zero ≡ Π(_:Two).Two`
    // and `C one ≡ Two`; a `λ(_:Two).zero` checks only at the first and a
    // bare `zero` only at the second.
    let two_type = two(&mut arena);
    let context = Context::empty().extend(two_type);
    let family = branching_family(&mut arena);
    let function_body = two_zero(&mut arena);
    let function_domain = two(&mut arena);
    let function_witness = lambda(&mut arena, function_domain, function_body);
    let one_witness = two_zero(&mut arena);
    let scrutinee = variable(&mut arena, 0);
    let elimination = case_two(&mut arena, family, function_witness, one_witness, scrutinee);

    // `caseTwo(C, λ(_:Two).zero, zero, t) : C t`.
    let expected = apply(&mut arena, family, scrutinee);
    check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

    // Swapped branches reject: each is checked at its own constructor's
    // landing, not at a shared supertype.
    let wrong_zero = two_zero(&mut arena);
    let wrong_one_domain = two(&mut arena);
    let wrong_one_body = two_zero(&mut arena);
    let wrong_one = lambda(&mut arena, wrong_one_domain, wrong_one_body);
    let swapped = case_two(&mut arena, family, wrong_zero, one_witness, scrutinee);
    let error = infer_type(&mut arena, &context, swapped, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
    let swapped = case_two(&mut arena, family, function_witness, wrong_one, scrutinee);
    let error = infer_type(&mut arena, &context, swapped, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn lambda_bodies_check_componentwise_against_a_dependent_pi() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();

    // `λ(_ : Two). ⟨zero, refl Two zero⟩` checked against
    // `Π(_ : Two). Σ(t : Two). caseTwo(M, Id Two zero zero, Two, t)`.
    // The pair's inferred type is the non-dependent
    // `Σ(_ : Two). Id Two zero zero`, which no conversion equates with
    // the tagged family — but componentwise the payload `refl` checks at
    // `caseTwo(M, …, zero) ≡ Id Two zero zero`. This is the shape a
    // disjunction introduction reaches inside a case-analysis branch:
    // the branch `λ` must be *checked*, not merely inferred.
    let two_type = two(&mut arena);
    let zero = two_zero(&mut arena);
    let identity = id(&mut arena, two_type, zero, zero);
    let proof = refl(&mut arena, two_type, zero);
    let payload = pair(&mut arena, zero, proof);
    let witness = lambda(&mut arena, two_type, payload);

    let motive = universe_motive(&mut arena);
    let bound = variable(&mut arena, 0);
    let one_landing = two(&mut arena);
    let family = case_two(&mut arena, motive, identity, one_landing, bound);
    let codomain = sigma(&mut arena, two_type, family);
    let expected = pi(&mut arena, two_type, codomain);

    check_type(&mut arena, &empty, witness, expected, &mut budget).unwrap();

    // The annotation is still checked: a binder naming the wrong domain
    // rejects instead of smuggling the body under it.
    let strict_zero = strict_sort(&mut arena, 0);
    let lying = lambda(&mut arena, strict_zero, payload);
    let error = check_type(&mut arena, &empty, lying, expected, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn case_two_computes_on_each_constructor_and_stays_stuck_on_neutrals() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context t : Two, u : Two. In depth 2, t is index 1 and u is index 0.
    let two_type = two(&mut arena);
    let context = Context::empty().extend(two_type).extend(two_type);
    let family = branching_family(&mut arena);
    let function_body = two_zero(&mut arena);
    let function_domain = two(&mut arena);
    let function_witness = lambda(&mut arena, function_domain, function_body);
    let one_witness = two_zero(&mut arena);

    // `caseTwo(C, d0, d1, zero) → d0` and `… one → d1`: each computation
    // consumes one step and lands on the supplied branch.
    let zero = two_zero(&mut arena);
    let on_zero = case_two(&mut arena, family, function_witness, one_witness, zero);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), on_zero, &mut budget).unwrap();
    assert_eq!(normalized, function_witness);
    let one = two_one(&mut arena);
    let on_one = case_two(&mut arena, family, function_witness, one_witness, one);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), on_one, &mut budget).unwrap();
    assert_eq!(normalized, one_witness);

    // The computed result's type follows the same reduction: the type of
    // `caseTwo C d0 d1 zero` is `C zero`, which normalizes to
    // `Π(_:Two).Two`, and `C one` normalizes to `Two`.
    let result_type = infer_type(&mut arena, &context, on_zero, &mut budget).unwrap();
    let normalized_type =
        weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget).unwrap();
    let function_domain = two(&mut arena);
    let function_codomain = two(&mut arena);
    let expected_type = pi(&mut arena, function_domain, function_codomain);
    assert!(arena.structurally_equal(normalized_type, expected_type));
    let result_type = infer_type(&mut arena, &context, on_one, &mut budget).unwrap();
    let normalized_type =
        weak_head_normalize(&mut arena, &Signature::new(), result_type, &mut budget).unwrap();
    let expected_type = two(&mut arena);
    assert!(arena.structurally_equal(normalized_type, expected_type));

    // A neutral scrutinee keeps the elimination stuck; normalization
    // returns the same node rather than guessing a branch.
    let t = variable(&mut arena, 1);
    let stuck = case_two(&mut arena, family, function_witness, one_witness, t);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
    assert_eq!(normalized, stuck);

    // Stuck eliminations convert componentwise: identical motive,
    // branches and scrutinee convert; a different scrutinee does not.
    // The constant family `λ(_:Two).Two` keeps both sides at `Two` so the
    // shared type is honest.
    let constant_domain = two(&mut arena);
    let constant_body = two(&mut arena);
    let constant = lambda(&mut arena, constant_domain, constant_body);
    let t = variable(&mut arena, 1);
    let u = variable(&mut arena, 0);
    let left = case_two(&mut arena, constant, zero, one, t);
    let left_again = case_two(&mut arena, constant, zero, one, t);
    let right = case_two(&mut arena, constant, zero, one, u);
    assert!(
        convertible(
            &mut arena,
            &context,
            left,
            left_again,
            two_type,
            &mut budget
        )
        .unwrap()
    );
    assert!(!convertible(&mut arena, &context, left, right, two_type, &mut budget).unwrap());

    // Constructor computation is a budgeted step: an exhausted budget
    // refuses instead of reporting a judgment.
    let mut empty_budget = Budget::new(0);
    let error =
        weak_head_normalize(&mut arena, &Signature::new(), on_zero, &mut empty_budget).unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);
}

#[test]
fn case_two_rejects_motive_and_scrutinee_violations() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, a : A, t : Two. In depth 4, P is
    // index 3, A is index 2, a is index 1 and t is index 0.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_type = variable(&mut arena, 1);
    let two_type = two(&mut arena);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(a_type)
        .extend(two_type);
    let t = variable(&mut arena, 0);
    let a = variable(&mut arena, 1);

    // A strict motive `λ(_:Two). P` lands in `Strict 0`: the eliminator
    // targets relevant `Type`, and boxing owns strict targets.
    let strict_body = variable(&mut arena, 4);
    let strict_motive_domain = two(&mut arena);
    let strict_motive = lambda(&mut arena, strict_motive_domain, strict_body);
    let elimination = case_two(&mut arena, strict_motive, a, a, t);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictCaseMotiveCodomain { .. }));

    // A motive into a non-universe `λ(_:Two). a` leaves `C t` without a
    // type to check against.
    let term_body = variable(&mut arena, 2);
    let term_motive_domain = two(&mut arena);
    let term_motive = lambda(&mut arena, term_motive_domain, term_body);
    let elimination = case_two(&mut arena, term_motive, a, a, t);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::CaseMotiveCodomainNotAUniverse { .. }
    ));

    // A motive over a different domain `λ(x:A). Type 0` is not a
    // `Two`-family.
    let wrong_domain = variable(&mut arena, 2);
    let wrong_body = type_sort(&mut arena, 0);
    let wrong_motive = lambda(&mut arena, wrong_domain, wrong_body);
    let elimination = case_two(&mut arena, wrong_motive, a, a, t);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A motive that is not a function at all cannot name a family.
    let not_a_function = two_zero(&mut arena);
    let elimination = case_two(&mut arena, not_a_function, a, a, t);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAFunction { .. }));

    // The scrutinee must be a `Two`: `a : A` is not.
    let motive = universe_motive(&mut arena);
    let branch_type = type_sort(&mut arena, 0);
    let elimination = case_two(&mut arena, motive, branch_type, branch_type, a);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn pointwise_two_agreement_grants_no_function_equality() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context D : Π(_:Two). Type 0, f : Π(t:Two). D t. In depth 2, D is
    // index 1 and f is index 0; under f's own binder D is index 1.
    let two_type = two(&mut arena);
    let result_sort = type_sort(&mut arena, 0);
    let family_type = pi(&mut arena, two_type, result_sort);
    let applied_family = {
        let family = variable(&mut arena, 1);
        let bound = variable(&mut arena, 0);
        apply(&mut arena, family, bound)
    };
    let function_domain = two(&mut arena);
    let function_type = pi(&mut arena, function_domain, applied_family);
    let context = Context::empty().extend(family_type).extend(function_type);

    // The shared type `Π(t:Two). D t` in depth 2: under the binder D is
    // index 2.
    let shared_domain = two(&mut arena);
    let shared_codomain = {
        let family = variable(&mut arena, 2);
        let bound = variable(&mut arena, 0);
        apply(&mut arena, family, bound)
    };
    let shared_type = pi(&mut arena, shared_domain, shared_codomain);

    // `x ↦ caseTwo(D, f zero, f one, x)` agrees with `f` on every
    // constructor yet is not `f`: there is no `Two` eta law, and function
    // eta only compares the wrapper's body to `f x`, where a stuck
    // elimination never matches a plain application.
    let wrapped = {
        let family = variable(&mut arena, 2);
        let zero = two_zero(&mut arena);
        let f_at_zero = {
            let f = variable(&mut arena, 1);
            apply(&mut arena, f, zero)
        };
        let one = two_one(&mut arena);
        let f_at_one = {
            let f = variable(&mut arena, 1);
            apply(&mut arena, f, one)
        };
        let bound = variable(&mut arena, 0);
        let body = case_two(&mut arena, family, f_at_zero, f_at_one, bound);
        let domain = two(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let f = variable(&mut arena, 0);
    assert!(!convertible(&mut arena, &context, wrapped, f, shared_type, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &context, f, wrapped, shared_type, &mut budget).unwrap());

    // The wrapper still typechecks: the elimination is well-formed at
    // `Π(t:Two). D t`, it just does not collapse to `f`.
    check_type(&mut arena, &context, wrapped, shared_type, &mut budget).unwrap();

    // Control: ordinary function eta over the `Two` domain still holds.
    let eta_wrapped = {
        let f_under = variable(&mut arena, 1);
        let bound = variable(&mut arena, 0);
        let body = apply(&mut arena, f_under, bound);
        let domain = two(&mut arena);
        lambda(&mut arena, domain, body)
    };
    assert!(
        convertible(
            &mut arena,
            &context,
            eta_wrapped,
            f,
            shared_type,
            &mut budget
        )
        .unwrap()
    );
}
