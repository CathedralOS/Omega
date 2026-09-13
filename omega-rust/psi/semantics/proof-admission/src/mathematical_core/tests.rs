//! Behavioral tests for the mathematical core. Each test names the judgment
//! or control it pins down, per the board's discriminating-control contract.

use super::*;

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level(level))))
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level(level))))
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair })
}

fn default_budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

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

/// Builds `λ(A : Type 0). λ(x : A). x` and its Π type; returns the identity
/// term and the expected type for reuse by the storage test.
fn polymorphic_identity(arena: &mut TermArena) -> (TermHandle, TermHandle) {
    let type_zero = type_sort(arena, 0);
    let bound = variable(arena, 0);
    let inner = lambda(arena, bound, bound);
    let identity = lambda(arena, type_zero, inner);
    // Π(A : Type 0). Π(x : A). A
    let codomain_domain = variable(arena, 0);
    let codomain_body = variable(arena, 1);
    let codomain = pi(arena, codomain_domain, codomain_body);
    let expected_type = pi(arena, type_zero, codomain);
    (identity, expected_type)
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
    let normalized = weak_head_normalize(&mut arena, redex, &mut budget).unwrap();

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
    let error = weak_head_normalize(&mut arena, omega, &mut budget).unwrap_err();
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
    assert_eq!(arena.len(), 51);
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
    let normalized = weak_head_normalize(&mut arena, projected_first, &mut budget).unwrap();
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
    let normalized = weak_head_normalize(&mut arena, projected_second, &mut budget).unwrap();
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
    let normalized = weak_head_normalize(&mut arena, projected, &mut budget).unwrap();
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
