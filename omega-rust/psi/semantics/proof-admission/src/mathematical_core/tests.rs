//! Behavioral tests for the mathematical core. Each test names the judgment
//! or control it pins down, per the board's discriminating-control contract.

use super::term::sorts_equal;
use super::*;

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
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

fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

fn two_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoZero)
}

fn two_one(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoOne)
}

fn case_two(
    arena: &mut TermArena,
    motive: TermHandle,
    zero_branch: TermHandle,
    one_branch: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::CaseTwo {
        motive,
        zero_branch,
        one_branch,
        scrutinee,
    })
}

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

fn id_elim(
    arena: &mut TermArena,
    motive: TermHandle,
    base: TermHandle,
    endpoint: TermHandle,
    proof: TermHandle,
) -> TermHandle {
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint,
        proof,
    })
}

fn w_type(arena: &mut TermArena, carrier: TermHandle, children: TermHandle) -> TermHandle {
    arena.insert(Term::W { carrier, children })
}

fn sup(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    label: TermHandle,
    function: TermHandle,
) -> TermHandle {
    arena.insert(Term::Sup {
        carrier,
        children,
        label,
        function,
    })
}

fn ind_w(
    arena: &mut TermArena,
    motive: TermHandle,
    step: TermHandle,
    tree: TermHandle,
) -> TermHandle {
    arena.insert(Term::IndW { motive, step, tree })
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

/// `M := λ(_ : Two). Type 0` — the motive the large-elimination family
/// uses to return a universe per branch.
fn universe_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let body = type_sort(arena, 0);
    lambda(arena, domain, body)
}

/// `C := λ(t : Two). caseTwo(M, Π(_ : Two). Two, Two, t)` — a closed
/// family `Π(_ : Two). Type 0` landing at a function type on `zero` and
/// at `Two` on `one`, so the two branches of an outer elimination are
/// checked at definitionally different types.
fn branching_family(arena: &mut TermArena) -> TermHandle {
    let motive = universe_motive(arena);
    let domain = two(arena);
    let codomain = two(arena);
    let function_type = pi(arena, domain, codomain);
    let one_landing = two(arena);
    let bound = variable(arena, 0);
    let body = case_two(arena, motive, function_type, one_landing, bound);
    let domain = two(arena);
    lambda(arena, domain, body)
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

#[test]
fn identity_forms_at_the_carrier_level_and_refl_introduces() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A, b : A. In depth 3, A is index 2, a is
    // index 1 and b is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let b_binding = variable(&mut arena, 1);
    let context = Context::empty()
        .extend(type_zero)
        .extend(a_binding)
        .extend(b_binding);

    // Formation: `Id A a b : Type 0` — the identity type lives at the
    // carrier's level, and the carrier must be a relevant type.
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let b = variable(&mut arena, 0);
    let identity = id(&mut arena, carrier, a, b);
    let inferred = infer_type(&mut arena, &context, identity, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // `Id (Type 0) A B : Type 1` — the carrier `Type 0` itself lives at
    // `Type 1`, and the identity type follows it.
    let type_zero = type_sort(&mut arena, 0);
    let other_binding = type_sort(&mut arena, 0);
    let type_context = Context::empty().extend(type_zero).extend(other_binding);
    let type_carrier = type_sort(&mut arena, 0);
    let a_type = variable(&mut arena, 1);
    let b_type = variable(&mut arena, 0);
    let over_types = id(&mut arena, type_carrier, a_type, b_type);
    let inferred = infer_type(&mut arena, &type_context, over_types, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // Introduction: `refl A a : Id A a a`, and it checks there.
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let reflexive = refl(&mut arena, carrier, a);
    let inferred = infer_type(&mut arena, &context, reflexive, &mut budget).unwrap();
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let expected = id(&mut arena, carrier, a, a);
    assert!(arena.structurally_equal(inferred, expected));
    check_type(&mut arena, &context, reflexive, expected, &mut budget).unwrap();

    // But `refl A a` is not a proof of `Id A a b`: the endpoints must
    // convert to the reflexive value.
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let b = variable(&mut arena, 0);
    let off_endpoint = id(&mut arena, carrier, a, b);
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let reflexive = refl(&mut arena, carrier, a);
    let error = check_type(&mut arena, &context, reflexive, off_endpoint, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A strict carrier rejects: `Id P p p` over a proposition has no
    // proof-relevant distinction to carry. Context P : Strict 0, p : P.
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let strict_context = Context::empty().extend(strict_zero).extend(p_binding);
    let strict_carrier = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);
    let strict_identity = id(&mut arena, strict_carrier, p, p);
    let error = infer_type(&mut arena, &strict_context, strict_identity, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictIdentityDomain { .. }));
    let strict_carrier = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);
    let strict_refl = refl(&mut arena, strict_carrier, p);
    let error = infer_type(&mut arena, &strict_context, strict_refl, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictIdentityDomain { .. }));

    // A carrier that is not a type at all rejects at formation.
    let a = variable(&mut arena, 1);
    let b = variable(&mut arena, 0);
    let not_a_type = id(&mut arena, a, b, b);
    let error = infer_type(&mut arena, &context, not_a_type, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));

    // An endpoint at the wrong type rejects: `p : P` is not an `A`.
    // Context P : Strict 0, A : Type 0, a : A, p : P. In depth 4, A is
    // index 2, a is index 1 and p is index 0.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 1);
    let p_binding = variable(&mut arena, 2);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(a_binding)
        .extend(p_binding);
    let carrier = variable(&mut arena, 2);
    let a = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);
    let bad_endpoint = id(&mut arena, carrier, a, p);
    let error = infer_type(&mut arena, &context, bad_endpoint, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn refl_carries_a_dependent_pair_endpoint() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, P : Π(x : A). Strict 0, a : A, pr : P a. In
    // depth 4, A is index 3, P is index 2, a is index 1 and pr is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let a_domain = variable(&mut arena, 0);
    let predicate_type = pi(&mut arena, a_domain, strict_zero);
    let a_binding = variable(&mut arena, 1);
    let p_prefix = variable(&mut arena, 1);
    let a_prefix = variable(&mut arena, 0);
    let proof_binding = apply(&mut arena, p_prefix, a_prefix);
    let context = Context::empty()
        .extend(type_zero)
        .extend(predicate_type)
        .extend(a_binding)
        .extend(proof_binding);

    // `refl (Σ(x : A). P x) (a, pr)` — the annotated carrier lets the
    // dependent pair check componentwise where inferring the pair could
    // only produce a non-dependent `Σ`.
    let sigma_domain = variable(&mut arena, 3);
    let p_under = variable(&mut arena, 3);
    let bound = variable(&mut arena, 0);
    let codomain = apply(&mut arena, p_under, bound);
    let carrier = sigma(&mut arena, sigma_domain, codomain);
    let a = variable(&mut arena, 1);
    let proof = variable(&mut arena, 0);
    let endpoint = pair(&mut arena, a, proof);
    let reflexive = refl(&mut arena, carrier, endpoint);
    let inferred = infer_type(&mut arena, &context, reflexive, &mut budget).unwrap();

    let sigma_domain = variable(&mut arena, 3);
    let p_under = variable(&mut arena, 3);
    let bound = variable(&mut arena, 0);
    let codomain = apply(&mut arena, p_under, bound);
    let carrier = sigma(&mut arena, sigma_domain, codomain);
    let a = variable(&mut arena, 1);
    let proof = variable(&mut arena, 0);
    let endpoint = pair(&mut arena, a, proof);
    let expected = id(&mut arena, carrier, endpoint, endpoint);
    assert!(arena.structurally_equal(inferred, expected));
}

/// `C := λ(y : A). λ(_ : Id A x y). Id A y x` — the symmetry motive,
/// written at `depth` context entries where `a` is the carrier's index,
/// `x` the fixed endpoint's, and the bound `y` names index 0 under the
/// motive's own binder.
fn symmetry_motive(arena: &mut TermArena, a: u32, x: u32) -> TermHandle {
    let domain = variable(arena, a);
    let inner = {
        // Under the `y` binder the carrier is at `a + 1` and the fixed
        // endpoint at `x + 1`.
        let ty = variable(arena, a + 1);
        let fixed = variable(arena, x + 1);
        let bound = variable(arena, 0);
        let proof_domain = id(arena, ty, fixed, bound);
        // Under both binders: `Id A y x` — carrier `a + 2`, bound `y`
        // index 1, fixed `x + 2`.
        let ty = variable(arena, a + 2);
        let bound_y = variable(arena, 1);
        let fixed = variable(arena, x + 2);
        let body = id(arena, ty, bound_y, fixed);
        lambda(arena, proof_domain, body)
    };
    lambda(arena, domain, inner)
}

#[test]
fn identity_elimination_proves_symmetry() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, x : A, y : A, p : Id A x y. In depth 4, A is
    // index 3, x is index 2, y is index 1 and p is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let y_binding = variable(&mut arena, 1);
    let proof_binding = {
        let ty = variable(&mut arena, 2);
        let x = variable(&mut arena, 1);
        let y = variable(&mut arena, 0);
        id(&mut arena, ty, x, y)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(x_binding)
        .extend(y_binding)
        .extend(proof_binding);

    // `J(C, refl A x, y, p) : C y p`, and `C y p` computes to
    // `Id A y x` — elimination transports the endpoints.
    let motive = symmetry_motive(&mut arena, 3, 2);
    let base = {
        let ty = variable(&mut arena, 3);
        let x = variable(&mut arena, 2);
        refl(&mut arena, ty, x)
    };
    let endpoint = variable(&mut arena, 1);
    let proof = variable(&mut arena, 0);
    let elimination = id_elim(&mut arena, motive, base, endpoint, proof);

    let ty = variable(&mut arena, 3);
    let y = variable(&mut arena, 1);
    let x = variable(&mut arena, 2);
    let flipped = id(&mut arena, ty, y, x);
    check_type(&mut arena, &context, elimination, flipped, &mut budget).unwrap();

    // The claimed type `Id A x y` is the *unflipped* judgment: the same
    // term cannot prove it.
    let ty = variable(&mut arena, 3);
    let x = variable(&mut arena, 2);
    let y = variable(&mut arena, 1);
    let unflipped = id(&mut arena, ty, x, y);
    let error = check_type(&mut arena, &context, elimination, unflipped, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn identity_elimination_computes_on_refl_and_respects_the_ceiling() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, x : A. In depth 2, A is index 1 and x is
    // index 0.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let context = Context::empty().extend(type_zero).extend(x_binding);

    // `J(C, refl A x, x, refl A x)` with the symmetry motive computes to
    // `refl A x` in one budgeted step: symmetry of a reflexive identity
    // is reflexive.
    let motive = symmetry_motive(&mut arena, 1, 0);
    let base = {
        let ty = variable(&mut arena, 1);
        let x = variable(&mut arena, 0);
        refl(&mut arena, ty, x)
    };
    let endpoint = variable(&mut arena, 0);
    let proof = {
        let ty = variable(&mut arena, 1);
        let x = variable(&mut arena, 0);
        refl(&mut arena, ty, x)
    };
    let elimination = id_elim(&mut arena, motive, base, endpoint, proof);

    // The elimination is well-typed — `C x (refl A x) ≡ Id A x x` — and
    // its inferred type reduces to `Id A x x`.
    let ty = variable(&mut arena, 1);
    let x = variable(&mut arena, 0);
    let expected = id(&mut arena, ty, x, x);
    check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
    assert_eq!(normalized, base);

    // Constructor computation is a budgeted step: an empty budget
    // refuses instead of reporting a judgment.
    let mut empty_budget = Budget::new(0);
    let error = weak_head_normalize(
        &mut arena,
        &Signature::new(),
        elimination,
        &mut empty_budget,
    )
    .unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);

    // The computation makes the elimination convertible to a
    // differently-stored copy of its base at the same type.
    let result_type = {
        let motive = symmetry_motive(&mut arena, 1, 0);
        let at_fixed_argument = variable(&mut arena, 0);
        let at_fixed = apply(&mut arena, motive, at_fixed_argument);
        let ty = variable(&mut arena, 1);
        let x = variable(&mut arena, 0);
        let refl_proof = refl(&mut arena, ty, x);
        apply(&mut arena, at_fixed, refl_proof)
    };
    let same_refl = {
        let ty = variable(&mut arena, 1);
        let x = variable(&mut arena, 0);
        refl(&mut arena, ty, x)
    };
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            same_refl,
            result_type,
            &mut budget,
        )
        .unwrap()
    );
}

#[test]
fn stuck_identity_eliminations_compare_componentwise() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, x : A, y : A, p : Id A x y, q : Id A x y.
    // In depth 5, A is index 4, x is index 3, y is index 2, p is index 1
    // and q is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let y_binding = variable(&mut arena, 1);
    let p_binding = {
        let ty = variable(&mut arena, 2);
        let x = variable(&mut arena, 1);
        let y = variable(&mut arena, 0);
        id(&mut arena, ty, x, y)
    };
    let q_binding = {
        let ty = variable(&mut arena, 3);
        let x = variable(&mut arena, 2);
        let y = variable(&mut arena, 1);
        id(&mut arena, ty, x, y)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(x_binding)
        .extend(y_binding)
        .extend(p_binding)
        .extend(q_binding);

    // A constant motive `λ(y : A). λ(_ : Id A x y). Type 0` keeps both
    // sides at `Type 0`, so the shared type is honest.
    let motive = {
        let domain = variable(&mut arena, 4);
        let inner_domain = {
            let ty = variable(&mut arena, 5);
            let x = variable(&mut arena, 4);
            let bound = variable(&mut arena, 0);
            id(&mut arena, ty, x, bound)
        };
        let body = type_sort(&mut arena, 0);
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let two_base = two(&mut arena);
    let function_base = {
        let domain = two(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };

    // `J(C, d, y, p)` vs itself converts; the same elimination over the
    // other proof does not — identity proofs are relevant data.
    let endpoint = variable(&mut arena, 2);
    let p = variable(&mut arena, 1);
    let left = id_elim(&mut arena, motive, two_base, endpoint, p);
    let endpoint = variable(&mut arena, 2);
    let p = variable(&mut arena, 1);
    let left_again = id_elim(&mut arena, motive, two_base, endpoint, p);
    let endpoint = variable(&mut arena, 2);
    let q = variable(&mut arena, 0);
    let other_proof = id_elim(&mut arena, motive, two_base, endpoint, q);
    let endpoint = variable(&mut arena, 2);
    let p = variable(&mut arena, 1);
    let other_base = id_elim(&mut arena, motive, function_base, endpoint, p);
    let shared_type = type_sort(&mut arena, 0);

    assert!(
        convertible(
            &mut arena,
            &context,
            left,
            left_again,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
    assert!(
        !convertible(
            &mut arena,
            &context,
            left,
            other_proof,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
    assert!(
        !convertible(
            &mut arena,
            &context,
            left,
            other_base,
            shared_type,
            &mut budget,
        )
        .unwrap()
    );
}

#[test]
fn identity_elimination_rejects_malformed_motives_and_endpoints() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, x : A, y : A, p : Id A x y.
    // In depth 5, P is index 4, A is index 3, x is index 2, y is index 1
    // and p is index 0.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let y_binding = variable(&mut arena, 1);
    let p_binding = {
        let ty = variable(&mut arena, 2);
        let x = variable(&mut arena, 1);
        let y = variable(&mut arena, 0);
        id(&mut arena, ty, x, y)
    };
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(x_binding)
        .extend(y_binding)
        .extend(p_binding);
    let y = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);

    // A strict motive codomain `λ(y:A). λ(_:Id A x y). P` lands in
    // `Strict 0`: the eliminator targets relevant `Type`, and boxing
    // owns strict targets. Under both binders P is index 6.
    let strict_motive = {
        let domain = variable(&mut arena, 3);
        let inner_domain = {
            let ty = variable(&mut arena, 4);
            let x = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            id(&mut arena, ty, x, bound)
        };
        let body = variable(&mut arena, 6);
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let base = {
        let ty = variable(&mut arena, 3);
        let x = variable(&mut arena, 2);
        refl(&mut arena, ty, x)
    };
    let elimination = id_elim(&mut arena, strict_motive, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::StrictIdentityMotiveCodomain { .. }
    ));

    // A motive into a non-universe `λ(y:A). λ(_:Id A x y). y` leaves
    // `C y p` without a type to check against. Under both binders the
    // bound `y` is index 1.
    let term_motive = {
        let domain = variable(&mut arena, 3);
        let inner_domain = {
            let ty = variable(&mut arena, 4);
            let x = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            id(&mut arena, ty, x, bound)
        };
        let body = variable(&mut arena, 1);
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let elimination = id_elim(&mut arena, term_motive, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::IdentityMotiveCodomainNotAUniverse { .. }
    ));

    // A one-argument motive `λ(y:A). Type 0` never reaches the proof:
    // its codomain is not a `Π`.
    let short_motive = {
        let domain = variable(&mut arena, 3);
        let body = type_sort(&mut arena, 0);
        lambda(&mut arena, domain, body)
    };
    let elimination = id_elim(&mut arena, short_motive, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::IdentityMotiveCodomainNotAFunction { .. }
    ));

    // A motive that is not a function at all cannot name a family.
    let not_a_function = two_zero(&mut arena);
    let elimination = id_elim(&mut arena, not_a_function, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAFunction { .. }));

    // A motive over a different domain `λ(y:Two). λ(_:Id A x y). Type 0`
    // is not an `A`-family.
    let wrong_domain_motive = {
        let domain = two(&mut arena);
        let inner_domain = {
            let ty = variable(&mut arena, 4);
            let x = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            id(&mut arena, ty, x, bound)
        };
        let body = type_sort(&mut arena, 0);
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let elimination = id_elim(&mut arena, wrong_domain_motive, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A motive whose second domain is the carrier `A` rather than
    // `Id A x y` does not eliminate an identity.
    let wrong_inner_motive = {
        let domain = variable(&mut arena, 3);
        let inner_domain = variable(&mut arena, 4);
        let body = type_sort(&mut arena, 0);
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let elimination = id_elim(&mut arena, wrong_inner_motive, base, y, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The supplied endpoint must be the proof's recorded endpoint:
    // `J(C, d, x, p)` for `p : Id A x y` relocates the target and
    // rejects.
    let motive = symmetry_motive(&mut arena, 3, 2);
    let x = variable(&mut arena, 2);
    let elimination = id_elim(&mut arena, motive, base, x, p);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The scrutinee must be an identity proof: `x : A` is not one.
    let motive = symmetry_motive(&mut arena, 3, 2);
    let x = variable(&mut arena, 2);
    let elimination = id_elim(&mut arena, motive, base, y, x);
    let error = infer_type(&mut arena, &context, elimination, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAnIdentity { .. }));
}

#[test]
fn identity_proofs_stay_relevant_without_uip() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, x : A, p : Id A x x, q : Id A x x. In depth
    // 4, A is index 3, x is index 2, p is index 1 and q is index 0.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let p_binding = {
        let ty = variable(&mut arena, 1);
        let x = variable(&mut arena, 0);
        id(&mut arena, ty, x, x)
    };
    let q_binding = {
        let ty = variable(&mut arena, 2);
        let x = variable(&mut arena, 1);
        id(&mut arena, ty, x, x)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(x_binding)
        .extend(p_binding)
        .extend(q_binding);
    let ty = variable(&mut arena, 3);
    let x = variable(&mut arena, 2);
    let shared_type = id(&mut arena, ty, x, x);

    // `refl A x` checks at `Id A x x` — the positive control.
    let ty = variable(&mut arena, 3);
    let x = variable(&mut arena, 2);
    let reflexive = refl(&mut arena, ty, x);
    check_type(&mut arena, &context, reflexive, shared_type, &mut budget).unwrap();

    // But a `refl` never converts to a neutral proof of the same
    // identity, and two neutral proofs of the same identity stay
    // distinct: there is no K/UIP collapse on the relevant layer.
    let p = variable(&mut arena, 1);
    let q = variable(&mut arena, 0);
    assert!(!convertible(&mut arena, &context, reflexive, p, shared_type, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &context, p, reflexive, shared_type, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &context, p, q, shared_type, &mut budget).unwrap());

    // Different subjects do not collapse either: in context
    // A : Type 0, x : A, y : A (depth 3), `Id A x y` and `Id A y x` are
    // distinct types.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let y_binding = variable(&mut arena, 1);
    let context = Context::empty()
        .extend(type_zero)
        .extend(x_binding)
        .extend(y_binding);
    let ty = variable(&mut arena, 2);
    let x = variable(&mut arena, 1);
    let y = variable(&mut arena, 0);
    let forward = id(&mut arena, ty, x, y);
    let ty = variable(&mut arena, 2);
    let y = variable(&mut arena, 0);
    let x = variable(&mut arena, 1);
    let backward = id(&mut arena, ty, y, x);
    let shared_sort = type_sort(&mut arena, 0);
    assert!(
        !convertible(
            &mut arena,
            &context,
            forward,
            backward,
            shared_sort,
            &mut budget,
        )
        .unwrap()
    );
}

/// The `indW` step type `Π(a' : A). Π(k' : Π(b : B a'). W A B).
/// Π(_ : Π(b : B a'). P (k' b)). P (sup A B a' k')` over the prefix
/// `A : Type 0, B : Π(_:A). Type 0, a : A, k : Π(b:B a). W A B,
/// P : Π(_:W A B). Type 0` — written by hand so the kernel's own
/// step-type construction is cross-checked against an independent
/// encoding rather than against itself.
fn w_step_binding(arena: &mut TermArena) -> TermHandle {
    let a_domain = variable(arena, 4);
    // k' : Π(b : B a'). W A B — under `a'` B is index 4 and under
    // `a', b` the ambient A and B are indices 6 and 5.
    let b_at = variable(arena, 4);
    let a_bound = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_bound);
    let carrier = variable(arena, 6);
    let children = variable(arena, 5);
    let w_at_two = w_type(arena, carrier, children);
    let function_type = pi(arena, child_positions, w_at_two);
    // ih : Π(b : B a'). P (k' b) — the domain at depth 2 has B = 5,
    // a' = 1; the codomain at depth 3 has P = 3, k' = 1, b = 0.
    let b_at = variable(arena, 5);
    let a_bound = variable(arena, 1);
    let hypothesis_domain = apply(arena, b_at, a_bound);
    let p_at = variable(arena, 3);
    let k_bound = variable(arena, 1);
    let b_bound = variable(arena, 0);
    let child = apply(arena, k_bound, b_bound);
    let hypothesis_codomain = apply(arena, p_at, child);
    let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
    // P (sup A B a' k') at depth 3: P = 3, a' = 2, k' = 1, and the
    // ambient A and B sit at 7 and 6.
    let p_at = variable(arena, 3);
    let carrier = variable(arena, 7);
    let children = variable(arena, 6);
    let label = variable(arena, 2);
    let function = variable(arena, 1);
    let node = sup(arena, carrier, children, label, function);
    let result = apply(arena, p_at, node);
    let inner = pi(arena, hypothesis, result);
    let middle = pi(arena, function_type, inner);
    pi(arena, a_domain, middle)
}

/// `A : Type 0, B : Π(_:A). Type 0, a : A, k : Π(b : B a). W A B,
/// P : Π(_ : W A B). Type 0, s : <step type>, t : W A B, u : W A B`.
/// At depth 0: u = 0, t = 1, s = 2, P = 3, k = 4, a = 5, B = 6, A = 7.
fn w_context(arena: &mut TermArena) -> Context {
    let type_zero = type_sort(arena, 0);
    let a_in_prefix = variable(arena, 0);
    let b_binding = pi(arena, a_in_prefix, type_zero);
    let a_binding = variable(arena, 1);
    // k : Π(b : B a). W A B over prefix [A, B, a].
    let b_at = variable(arena, 1);
    let a_at = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_at);
    let carrier = variable(arena, 3);
    let children = variable(arena, 2);
    let w_under_b = w_type(arena, carrier, children);
    let k_binding = pi(arena, child_positions, w_under_b);
    // P : Π(_ : W A B). Type 0 over prefix [A, B, a, k].
    let carrier = variable(arena, 3);
    let children = variable(arena, 2);
    let w = w_type(arena, carrier, children);
    let p_binding = pi(arena, w, type_zero);
    let s_binding = w_step_binding(arena);
    // t over prefix [A, B, a, k, P, s]; u over one more binding.
    let carrier = variable(arena, 5);
    let children = variable(arena, 4);
    let t_binding = w_type(arena, carrier, children);
    let carrier = variable(arena, 6);
    let children = variable(arena, 5);
    let u_binding = w_type(arena, carrier, children);
    Context::empty()
        .extend(type_zero)
        .extend(b_binding)
        .extend(a_binding)
        .extend(k_binding)
        .extend(p_binding)
        .extend(s_binding)
        .extend(t_binding)
        .extend(u_binding)
}

/// `λ(a' : A). λ(k' : Π(b : B a'). W A B). λ(ih : Π(b : B a'). Two).
/// zero` — a concrete induction step for the constant motive
/// `λ(_ : W A B). Two`, checked against the kernel's built step type
/// through the motive's own reductions. Written in the 8-binding
/// `w_context` (A = 7, B = 6).
fn concrete_step(arena: &mut TermArena) -> TermHandle {
    let a_domain = variable(arena, 7);
    // Π(b : B a'). W A B under a': B = 7, a' = 0; codomain at depth 2:
    // A = 9, B = 8.
    let b_at = variable(arena, 7);
    let a_bound = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_bound);
    let carrier = variable(arena, 9);
    let children = variable(arena, 8);
    let w_at_two = w_type(arena, carrier, children);
    let function_type = pi(arena, child_positions, w_at_two);
    // Π(b : B a'). Two at depth 2: B = 8, a' = 1.
    let b_at = variable(arena, 8);
    let a_bound = variable(arena, 1);
    let hypothesis_domain = apply(arena, b_at, a_bound);
    let hypothesis_codomain = two(arena);
    let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
    let body = two_zero(arena);
    let inner = lambda(arena, hypothesis, body);
    let middle = lambda(arena, function_type, inner);
    lambda(arena, a_domain, middle)
}

#[test]
fn w_forms_at_the_maximum_component_level() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let empty = Context::empty();

    // `W (Type 0) (λ(_ : Type 0). Two) : Type 1` — the carrier's own
    // universe `u = 1` dominates the family's `v = 0`.
    let carrier = type_sort(&mut arena, 0);
    let domain = type_sort(&mut arena, 0);
    let body = two(&mut arena);
    let children = lambda(&mut arena, domain, body);
    let w = w_type(&mut arena, carrier, children);
    let inferred = infer_type(&mut arena, &empty, w, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // `W Two (λ(_ : Two). Type 0) : Type 1` — a family into a higher
    // universe lifts the formation: `max(0, 1) = 1`.
    let carrier = two(&mut arena);
    let domain = two(&mut arena);
    let body = type_sort(&mut arena, 0);
    let children = lambda(&mut arena, domain, body);
    let w = w_type(&mut arena, carrier, children);
    let inferred = infer_type(&mut arena, &empty, w, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // `W Two (λ(_ : Two). Two) : Type 0` stays at the floor.
    let carrier = two(&mut arena);
    let domain = two(&mut arena);
    let body = two(&mut arena);
    let children = lambda(&mut arena, domain, body);
    let w = w_type(&mut arena, carrier, children);
    let inferred = infer_type(&mut arena, &empty, w, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // In the context, `W A B : Type 0` — the opaque family `B` is as
    // dependent as formation allows.
    let context = w_context(&mut arena);
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let w = w_type(&mut arena, carrier, children);
    let inferred = infer_type(&mut arena, &context, w, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));
}

#[test]
fn w_formation_rejects_strict_and_malformed_families() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context S : Strict 0, A : Type 0, A2 : Type 0, a : A. At depth 4:
    // a = 0, A2 = 1, A = 2, S = 3.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let type_zero_again = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 1);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(type_zero_again)
        .extend(a_binding);

    // A strict carrier has no relevant data for a well-founded tree.
    let s = variable(&mut arena, 3);
    let any_children = two(&mut arena);
    let w = w_type(&mut arena, s, any_children);
    let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictWCarrier { .. }));

    // A branching family that is not a function gives child positions
    // no domain.
    let a = variable(&mut arena, 0);
    let carrier = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, a);
    let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAFunction { .. }));

    // A family over a different carrier is not `B : A → Type v`:
    // `λ(_ : A2). Two` has domain `A2`, not `A`.
    let a2 = variable(&mut arena, 1);
    let body = two(&mut arena);
    let wrong_family = lambda(&mut arena, a2, body);
    let carrier = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, wrong_family);
    let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A family into `Strict` makes child positions propositions —
    // boxing owns those, not `W`.
    let a = variable(&mut arena, 2);
    let s_body = variable(&mut arena, 4);
    let strict_family = lambda(&mut arena, a, s_body);
    let carrier = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, strict_family);
    let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictWChildrenCodomain { .. }));

    // A family into a non-universe `λ(_ : A). a` leaves `B a` without
    // a type for child positions.
    let a = variable(&mut arena, 2);
    let a_body = variable(&mut arena, 1);
    let term_family = lambda(&mut arena, a, a_body);
    let carrier = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, term_family);
    let error = infer_type(&mut arena, &context, w, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::WChildrenCodomainNotAUniverse { .. }
    ));
}

#[test]
fn sup_checks_its_annotation_and_components() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);

    // `sup A B a k : W A B` — the positive control.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let w = w_type(&mut arena, carrier, children);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node = sup(&mut arena, carrier, children, label, function);
    check_type(&mut arena, &context, node, w, &mut budget).unwrap();
    let inferred = infer_type(&mut arena, &context, node, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, w));

    // The child function's expected domain `B a` is computed through
    // reduction: an eta-expanded annotation `λ(x : A). B x` still
    // checks `k` at `Π(b : B a). W A B`, and the resulting `W` type is
    // the same one by typed function eta.
    let x = variable(&mut arena, 0);
    let b_under = variable(&mut arena, 7);
    let applied = apply(&mut arena, b_under, x);
    let a_domain = variable(&mut arena, 7);
    let eta_family = lambda(&mut arena, a_domain, applied);
    let carrier = variable(&mut arena, 7);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node = sup(&mut arena, carrier, eta_family, label, function);
    check_type(&mut arena, &context, node, w, &mut budget).unwrap();

    // The annotation is checked, never trusted: a strict carrier
    // rejects inside `sup` exactly as in formation.
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let strict_zero = strict_sort(&mut arena, 0);
    let context = Context::empty().extend(strict_zero);
    let s = variable(&mut arena, 0);
    let node = sup(&mut arena, s, s, s, s);
    let error = infer_type(&mut arena, &context, node, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictWCarrier { .. }));

    // A label that is not an `A` rejects; so does a child function
    // that is not a `Π(b : B a). W A B`.
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let wrong_label_term = variable(&mut arena, 1);
    let function = variable(&mut arena, 4);
    let wrong_label = sup(&mut arena, carrier, children, wrong_label_term, function);
    let error = infer_type(&mut arena, &context, wrong_label, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let label = variable(&mut arena, 5);
    let wrong_function_term = variable(&mut arena, 5);
    let wrong_function = sup(&mut arena, carrier, children, label, wrong_function_term);
    let error = infer_type(&mut arena, &context, wrong_function, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn w_induction_checks_the_step_against_its_dependent_type() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);

    // `indW(P, s, t) : P t` with the step supplied as a context
    // variable — the hand-written `w_step_binding` and the kernel's
    // `w_step_type` must agree for this to check.
    let p = variable(&mut arena, 3);
    let s = variable(&mut arena, 2);
    let t = variable(&mut arena, 1);
    let induction = ind_w(&mut arena, p, s, t);
    let p = variable(&mut arena, 3);
    let expected = apply(&mut arena, p, t);
    check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();
    let inferred = infer_type(&mut arena, &context, induction, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));

    // The same elimination over `sup A B a k` lands at `P (sup …)`.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node = sup(&mut arena, carrier, children, label, function);
    let p = variable(&mut arena, 3);
    let induction = ind_w(&mut arena, p, s, node);
    let p = variable(&mut arena, 3);
    let expected = apply(&mut arena, p, node);
    check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();

    // A concrete lambda step for the constant motive `λ(_ : W A B).
    // Two` checks through the motive's reductions: the step's
    // `Π(b : B a'). Two` hypothesis and `Two` result convert to the
    // built `Π(b : B a'). C (k' b)` and `C (sup …)`.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let w = w_type(&mut arena, carrier, children);
    let body = two(&mut arena);
    let constant_motive = lambda(&mut arena, w, body);
    let step = concrete_step(&mut arena);
    let induction = ind_w(&mut arena, constant_motive, step, t);
    let expected = two(&mut arena);
    check_type(&mut arena, &context, induction, expected, &mut budget).unwrap();

    // The tree must inhabit a `W` type — `a : A` does not.
    let a = variable(&mut arena, 5);
    let p = variable(&mut arena, 3);
    let induction = ind_w(&mut arena, p, s, a);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAW { .. }));

    // A motive that is not a function cannot name a family.
    let not_a_function = two_zero(&mut arena);
    let induction = ind_w(&mut arena, not_a_function, s, t);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotAFunction { .. }));

    // A motive over `A` is not a `W A B` family.
    let wrong_domain = variable(&mut arena, 7);
    let body = two(&mut arena);
    let wrong_motive = lambda(&mut arena, wrong_domain, body);
    let induction = ind_w(&mut arena, wrong_motive, s, t);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A step of the wrong shape — `λ(a' : A). a'` is `A → A`, not the
    // dependent step type — rejects at `check_type`.
    let a_domain = variable(&mut arena, 7);
    let a_bound = variable(&mut arena, 0);
    let wrong_step = lambda(&mut arena, a_domain, a_bound);
    let p = variable(&mut arena, 3);
    let induction = ind_w(&mut arena, p, wrong_step, t);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn w_induction_rejects_strict_and_non_universe_motives() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context S : Strict 0, A : Type 0, B : Π(_:A). Type 0, a : A,
    // t : W A B. At depth 5: t = 0, a = 1, B = 2, A = 3, S = 4.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_in_prefix = variable(&mut arena, 0);
    let b_binding = pi(&mut arena, a_in_prefix, type_zero);
    let a_binding = variable(&mut arena, 1);
    let carrier = variable(&mut arena, 2);
    let children = variable(&mut arena, 1);
    let t_binding = w_type(&mut arena, carrier, children);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(b_binding)
        .extend(a_binding)
        .extend(t_binding);

    // A motive `λ(_ : W A B). S` into `Strict 0` is not an admitted
    // elimination target: boxing owns strict motives.
    let carrier = variable(&mut arena, 3);
    let children = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, children);
    let s_body = variable(&mut arena, 5);
    let strict_motive = lambda(&mut arena, w, s_body);
    let t = variable(&mut arena, 0);
    let step_placeholder = variable(&mut arena, 0);
    let induction = ind_w(&mut arena, strict_motive, step_placeholder, t);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::StrictInductionMotiveCodomain { .. }
    ));

    // A motive `λ(_ : W A B). a` into the non-universe `A` leaves
    // `P t` without a type to check against.
    let carrier = variable(&mut arena, 3);
    let children = variable(&mut arena, 2);
    let w = w_type(&mut arena, carrier, children);
    let a_body = variable(&mut arena, 2);
    let term_motive = lambda(&mut arena, w, a_body);
    let induction = ind_w(&mut arena, term_motive, step_placeholder, t);
    let error = infer_type(&mut arena, &context, induction, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::InductionMotiveCodomainNotAUniverse { .. }
    ));
}

#[test]
fn w_induction_computes_on_sup_with_a_neutral_child_function() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);

    // `indW(P, s, sup A B a k)` with `k` an arbitrary variable — the
    // supplied child function is neutral, not an expanded lambda.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node = sup(&mut arena, carrier, children, label, function);
    let p = variable(&mut arena, 3);
    let s = variable(&mut arena, 2);
    let induction = ind_w(&mut arena, p, s, node);

    // One budgeted step unfolds to `s a k (λ(b : B a). indW(P, s,
    // k b))`: the induction hypothesis is rebuilt from the `sup`'s
    // checked `B` annotation even though `W A B` itself is neutral.
    let before = budget.remaining();
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), induction, &mut budget).unwrap();
    assert_eq!(before - budget.remaining(), 1);
    let ih_domain = {
        let b = variable(&mut arena, 6);
        let a = variable(&mut arena, 5);
        apply(&mut arena, b, a)
    };
    let ih_body = {
        let p = variable(&mut arena, 4);
        let s = variable(&mut arena, 3);
        let k = variable(&mut arena, 5);
        let bound = variable(&mut arena, 0);
        let child = apply(&mut arena, k, bound);
        ind_w(&mut arena, p, s, child)
    };
    let ih = lambda(&mut arena, ih_domain, ih_body);
    let expected = {
        let s = variable(&mut arena, 2);
        let a = variable(&mut arena, 5);
        let k = variable(&mut arena, 4);
        let applied = apply(&mut arena, s, a);
        let applied = apply(&mut arena, applied, k);
        apply(&mut arena, applied, ih)
    };
    assert!(arena.structurally_equal(normalized, expected));

    // The result type follows the same reduction: `P (sup …)` is the
    // inferred type, and it stays stuck while `P` is neutral.
    let inferred = infer_type(&mut arena, &context, induction, &mut budget).unwrap();
    let p = variable(&mut arena, 3);
    let expected_type = apply(&mut arena, p, node);
    assert!(arena.structurally_equal(inferred, expected_type));

    // Constructor computation is a budgeted step: an exhausted budget
    // refuses instead of reporting a judgment.
    let mut empty_budget = Budget::new(0);
    let error = weak_head_normalize(&mut arena, &Signature::new(), induction, &mut empty_budget)
        .unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);

    // A neutral tree keeps the elimination stuck — `indW(P, s, t)` is
    // its own normal form.
    let t = variable(&mut arena, 1);
    let p = variable(&mut arena, 3);
    let s = variable(&mut arena, 2);
    let stuck = ind_w(&mut arena, p, s, t);
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
    assert_eq!(normalized, stuck);
}

#[test]
fn stuck_w_inductions_compare_componentwise() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);

    // Under the constant motive `λ(_ : W A B). Two` every induction
    // lands at `Two`, so the shared type is honest for both sides.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let w = w_type(&mut arena, carrier, children);
    let body = two(&mut arena);
    let motive = lambda(&mut arena, w, body);
    let step = concrete_step(&mut arena);
    let t = variable(&mut arena, 1);
    let u = variable(&mut arena, 0);
    let shared = two(&mut arena);

    let left = ind_w(&mut arena, motive, step, t);
    let left_again = ind_w(&mut arena, motive, step, t);
    let right = ind_w(&mut arena, motive, step, u);
    assert!(convertible(&mut arena, &context, left, left_again, shared, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

    // A different step rejects as well: `λ(a':A).λ(k':…).λ(ih:…). one`
    // is not the supplied step.
    let a_domain = variable(&mut arena, 7);
    let b_at = variable(&mut arena, 7);
    let a_bound = variable(&mut arena, 0);
    let child_positions = apply(&mut arena, b_at, a_bound);
    let carrier = variable(&mut arena, 9);
    let children = variable(&mut arena, 8);
    let w_at_two = w_type(&mut arena, carrier, children);
    let function_type = pi(&mut arena, child_positions, w_at_two);
    let b_at = variable(&mut arena, 8);
    let a_bound = variable(&mut arena, 1);
    let hypothesis_domain = apply(&mut arena, b_at, a_bound);
    let hypothesis_codomain = two(&mut arena);
    let hypothesis = pi(&mut arena, hypothesis_domain, hypothesis_codomain);
    let body = two_one(&mut arena);
    let inner = lambda(&mut arena, hypothesis, body);
    let middle = lambda(&mut arena, function_type, inner);
    let other_step = lambda(&mut arena, a_domain, middle);
    let other = ind_w(&mut arena, motive, other_step, t);
    assert!(!convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap());
}

#[test]
fn w_types_convert_componentwise_and_sup_has_no_eta() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = w_context(&mut arena);
    let shared_sort = type_sort(&mut arena, 0);

    // `W A B` converts to itself; an eta-expanded family `λ(x : A).
    // B x` names the same `W` type by typed function eta.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let w = w_type(&mut arena, carrier, children);
    let x = variable(&mut arena, 0);
    let b_under = variable(&mut arena, 7);
    let applied = apply(&mut arena, b_under, x);
    let a_domain = variable(&mut arena, 7);
    let eta_family = lambda(&mut arena, a_domain, applied);
    let carrier = variable(&mut arena, 7);
    let w_eta = w_type(&mut arena, carrier, eta_family);
    assert!(convertible(&mut arena, &context, w, w_eta, shared_sort, &mut budget).unwrap());

    // A different family gives a different `W`: `λ(_ : A). Two` never
    // agrees with the opaque `B`.
    let a_domain = variable(&mut arena, 7);
    let body = two(&mut arena);
    let constant_family = lambda(&mut arena, a_domain, body);
    let carrier = variable(&mut arena, 7);
    let w_constant = w_type(&mut arena, carrier, constant_family);
    assert!(
        !convertible(
            &mut arena,
            &context,
            w,
            w_constant,
            shared_sort,
            &mut budget
        )
        .unwrap()
    );

    // A family at a different level — `λ(_ : A). Type 0` is
    // `Π(_ : A). Type 1` — can never convert to `B : Π(_ : A). Type 0`:
    // without cumulativity the two `W` types share no universe, so the
    // judgment is false rather than undecided.
    let a_domain = variable(&mut arena, 7);
    let body = type_sort(&mut arena, 0);
    let higher_family = lambda(&mut arena, a_domain, body);
    let carrier = variable(&mut arena, 7);
    let w_higher = w_type(&mut arena, carrier, higher_family);
    let higher_sort = type_sort(&mut arena, 1);
    assert!(!convertible(&mut arena, &context, w, w_higher, higher_sort, &mut budget).unwrap());

    // Two constructor trees compare at the shared `W A B`: the labels
    // at `A`, the child functions at `Π(b : B a). W A B`. Identical
    // children convert; a different label does not — and there is no
    // W eta, so `sup A B a k` never converts to the neutral `t`.
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node = sup(&mut arena, carrier, children, label, function);
    let carrier = variable(&mut arena, 7);
    let children = variable(&mut arena, 6);
    let label = variable(&mut arena, 5);
    let function = variable(&mut arena, 4);
    let node_again = sup(&mut arena, carrier, children, label, function);
    assert!(convertible(&mut arena, &context, node, node_again, w, &mut budget).unwrap());
    let t = variable(&mut arena, 1);
    assert!(!convertible(&mut arena, &context, node, t, w, &mut budget).unwrap());
    assert!(!convertible(&mut arena, &context, t, node, w, &mut budget).unwrap());
}

/// `Type` sorts holding arbitrary level expressions, for the
/// level-parameter tests below.
fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(level)))
}

/// `level + count` written with the successor constructor.
fn offset(mut level: Level, count: u32) -> Level {
    for _ in 0..count {
        level = level.successor().unwrap();
    }
    level
}

#[test]
fn level_parameters_must_stay_inside_the_judgment_arity() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `Type u` under one universe parameter inhabits `Type (u+1)`; the
    // parameter is a positional index into the judgment's level scope.
    let context = Context::with_level_arity(1);
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let inferred = infer_type(&mut arena, &context, type_u, &mut budget).unwrap();
    let expected = sort_level(&mut arena, Level::Parameter(0).successor().unwrap());
    assert!(arena.structurally_equal(inferred, expected));

    // Under no parameters `u` is a malformed universe, and a parameter
    // one past the arity end is equally malformed.
    let closed = Context::empty();
    let error = infer_type(&mut arena, &closed, type_u, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 0, arity: 0 }
    );
    let out_of_scope = sort_level(&mut arena, Level::Parameter(1));
    let error = infer_type(&mut arena, &context, out_of_scope, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 1, arity: 1 }
    );

    // A malformed level nested inside a larger type still rejects — the
    // scope rule is structural, not a top-level formality.
    let domain = sort_level(&mut arena, Level::Constant(0).maximum(Level::Parameter(3)));
    let bound = variable(&mut arena, 0);
    let nested = pi(&mut arena, domain, bound);
    let error = infer_type(&mut arena, &context, nested, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 3, arity: 1 }
    );
}

#[test]
fn a_universe_polymorphic_identity_checks_parametrically() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `λ(A : Type u). λ(x : A). x : Π(A : Type u). Π(x : A). A` under one
    // universe parameter — the universe-polymorphic dependent function the
    // board names, decided for every instantiation of `u` at once.
    let context = Context::with_level_arity(1);
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let bound = variable(&mut arena, 0);
    let inner = lambda(&mut arena, bound, bound);
    let identity = lambda(&mut arena, type_u, inner);
    let codomain_domain = variable(&mut arena, 0);
    let codomain_body = variable(&mut arena, 1);
    let codomain = pi(&mut arena, codomain_domain, codomain_body);
    let expected = pi(&mut arena, type_u, codomain);
    check_type(&mut arena, &context, identity, expected, &mut budget).unwrap();

    // Formation computed `max(u+1, u)` for the Π's sort; the level
    // algebra — not syntactic luck — collapses that to `u+1`.
    let inferred = infer_type(&mut arena, &context, expected, &mut budget).unwrap();
    let claimed_sort = sort_level(&mut arena, Level::Parameter(0).successor().unwrap());
    let relevant = type_sort(&mut arena, 0);
    assert!(
        convertible(
            &mut arena,
            &context,
            inferred,
            claimed_sort,
            relevant,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn a_universe_polymorphic_dependent_pair_checks_componentwise() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `λ(A : Type u). λ(B : Type v). λ(a : A). λ(b : B). (a, b)` at
    // `Π(A : Type u). Π(B : Type v). Π(a : A). Π(b : B). Σ(_ : A). B`
    // under two parameters: the second component is checked at the
    // codomain instantiated by the first, so the pair's dependent
    // structure survives polymorphic levels.
    let context = Context::with_level_arity(2);
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let type_v = sort_level(&mut arena, Level::Parameter(1));
    // The pair body at depth 4: a is index 1, b is index 0.
    let a = variable(&mut arena, 1);
    let b = variable(&mut arena, 0);
    let body = pair(&mut arena, a, b);
    // λ(b : B) at depth 3: B is index 1 (a is 0, B is 1, A is 2).
    let b_domain = variable(&mut arena, 1);
    let inner = lambda(&mut arena, b_domain, body);
    // λ(a : A) at depth 2: A is index 1.
    let a_domain = variable(&mut arena, 1);
    let inner = lambda(&mut arena, a_domain, inner);
    let inner = lambda(&mut arena, type_v, inner);
    let witness = lambda(&mut arena, type_u, inner);
    // Σ(_ : A). B: domain A at depth 4 is index 3; codomain B under the
    // Σ binder at depth 5 is index 3.
    let sigma_domain = variable(&mut arena, 3);
    let sigma_codomain = variable(&mut arena, 3);
    let sigma = sigma(&mut arena, sigma_domain, sigma_codomain);
    // Π(b : B) at depth 3: B is index 1.
    let b_domain = variable(&mut arena, 1);
    let over_b = pi(&mut arena, b_domain, sigma);
    let a_domain = variable(&mut arena, 1);
    let over_a = pi(&mut arena, a_domain, over_b);
    let over_v = pi(&mut arena, type_v, over_a);
    let expected = pi(&mut arena, type_u, over_v);
    check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();
}

#[test]
fn the_level_algebra_decides_sort_conversion() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::with_level_arity(2);
    // Comparing two sorts as terms asks only for a relevant shared type —
    // the (Sort, Sort) arm never inspects it further.
    let relevant = type_sort(&mut arena, 0);
    let u = Level::Parameter(0);
    let v = Level::Parameter(1);
    for (left, right) in [
        // `max` commutes and is associative and idempotent.
        (u.clone().maximum(v.clone()), v.clone().maximum(u.clone())),
        (u.clone().maximum(u.clone()), u.clone()),
        (
            u.clone().maximum(v.clone().maximum(Level::Constant(2))),
            Level::Constant(2).maximum(v.clone().maximum(u.clone())),
        ),
        // `max(u+1, u)` is `u+1`: the smaller same-parameter offset absorbs.
        (
            u.clone().successor().unwrap().maximum(u.clone()),
            u.clone().successor().unwrap(),
        ),
        // `succ` distributes over `max`.
        (
            u.clone().maximum(v.clone()).successor().unwrap(),
            u.clone()
                .successor()
                .unwrap()
                .maximum(v.clone().successor().unwrap()),
        ),
        // A constant floor covered by a variable offset is redundant:
        // `max(3, u+5)` is `u+5` at every instantiation, and `max(0, u)`
        // is `u` because 0 is the bottom level.
        (
            Level::Constant(3).maximum(offset(u.clone(), 5)),
            offset(u.clone(), 5),
        ),
        (Level::Constant(0).maximum(u.clone()), u.clone()),
    ] {
        let left = sort_level(&mut arena, left);
        let right = sort_level(&mut arena, right);
        assert!(
            convertible(&mut arena, &context, left, right, relevant, &mut budget).unwrap(),
            "levels must convert"
        );
    }

    // Distinct parameters never convert — the kernel decides equality, it
    // never solves for a unifier — and neither do `u`/`u+1`, a `max` and
    // one of its sides, or a constant and a parameter.
    for (left, right) in [
        (u.clone(), v.clone()),
        (u.clone(), u.clone().successor().unwrap()),
        (u.clone().maximum(v.clone()), u.clone()),
        (Level::Constant(3), u.clone()),
    ] {
        let left = sort_level(&mut arena, left);
        let right = sort_level(&mut arena, right);
        assert!(
            !convertible(&mut arena, &context, left, right, relevant, &mut budget).unwrap(),
            "levels must not convert"
        );
    }
}

#[test]
fn strict_universes_carry_parameters_without_layer_mixing() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::with_level_arity(1);
    let u = Level::Parameter(0);

    // `Strict u : Type (u+1)` — a strict sort inhabits the relevant
    // universe one level up, and the parameter stays in scope.
    let strict_u = arena.insert(Term::Sort(Sort::Strict(u.clone())));
    let inferred = infer_type(&mut arena, &context, strict_u, &mut budget).unwrap();
    let expected = sort_level(&mut arena, u.clone().successor().unwrap());
    assert!(arena.structurally_equal(inferred, expected));

    // `Π(A : Strict u). A : Strict (u+1)`: formation computes
    // `max(u+1, u)` at the codomain's layer, and the level algebra
    // collapses it to `u+1`.
    let bound = variable(&mut arena, 0);
    let strict_pi = pi(&mut arena, strict_u, bound);
    let sort = infer_sort(&mut arena, &context, strict_pi, &mut budget).unwrap();
    assert!(sorts_equal(
        &sort,
        &Sort::Strict(u.clone().successor().unwrap())
    ));

    // The layers never identify: `Type u` never converts to `Strict u`,
    // at the same level or any other.
    let relevant = type_sort(&mut arena, 0);
    let type_u = sort_level(&mut arena, u.clone());
    assert!(
        !convertible(
            &mut arena,
            &context,
            type_u,
            strict_u,
            relevant,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn level_parameters_ignore_term_binders() {
    let mut arena = TermArena::new();
    // Levels live in the judgment's level scope, not the term's de Bruijn
    // spine: shifting and substituting never touch a `Parameter`, so a
    // universe-polymorphic type travels under binders unchanged.
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    assert_eq!(shift(&mut arena, type_u, 0, 4), type_u);
    let argument = variable(&mut arena, 0);
    assert_eq!(substitute(&mut arena, type_u, argument), type_u);
}

// ── Declarations, constants, and the assumption closure ────────────────

fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

/// `polyId : Π(A : Type u). Π(x : A). A := λ(A : Type u). λ(x : A). x` —
/// the canonical universe-polymorphic definition, one level parameter.
fn polymorphic_identity_declaration(arena: &mut TermArena) -> Declaration {
    let type_u = sort_level(arena, Level::Parameter(0));
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    let ty = pi(arena, type_u, inner_pi);
    let type_u = sort_level(arena, Level::Parameter(0));
    let bound_a = variable(arena, 0);
    let inner_x = variable(arena, 0);
    let inner = lambda(arena, bound_a, inner_x);
    let body = lambda(arena, type_u, inner);
    Declaration::definition(1, ty, body)
}

/// The `polyId` statement instantiated at `u`: `Π(A : Type u). Π(x : A). A`.
fn instantiated_identity_type(arena: &mut TermArena, u: Level) -> TermHandle {
    let type_u = sort_level(arena, u);
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    pi(arena, type_u, inner_pi)
}

#[test]
fn a_universe_polymorphic_declaration_instantiates_at_a_constant() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let declaration = polymorphic_identity_declaration(&mut arena);
    let signature = check_signature(&mut arena, &[declaration], &mut budget).unwrap();
    let context = Context::empty().with_signature(signature);

    // `polyId([3]) : Π(A : Type 3). Π(x : A). A` — the closed judgment
    // supplies one closed level argument and receives the instantiated
    // statement.
    let applied = constant(&mut arena, 0, vec![Level::Constant(3)]);
    let expected = instantiated_identity_type(&mut arena, Level::Constant(3));
    let inferred = infer_type(&mut arena, &context, applied, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));
    check_type(&mut arena, &context, applied, expected, &mut budget).unwrap();

    // Instantiation preserves the dependent spine: the codomain still
    // names its `A` binder, so `polyId([0])` applied to a `Type 0`
    // argument and an inhabitant yields the argument's own type.
    let type_zero = type_sort(&mut arena, 0);
    let two = arena.insert(Term::Two);
    let two_zero = arena.insert(Term::TwoZero);
    let at_zero = constant(&mut arena, 0, vec![Level::Constant(0)]);
    let applied = apply(&mut arena, at_zero, two);
    let specialized = apply(&mut arena, applied, two_zero);
    let inferred = infer_type(&mut arena, &context, specialized, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, two));
    let _ = type_zero;
}

#[test]
fn a_second_declaration_references_only_the_checked_prefix() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `d0 : Type 0 := Two`; `d1 : Type 0 := d0` — the second declaration
    // resolves through the first because it is checked under the prefix.
    let type_zero = type_sort(&mut arena, 0);
    let two = arena.insert(Term::Two);
    let first = Declaration::definition(0, type_zero, two);
    let type_zero = type_sort(&mut arena, 0);
    let second = Declaration::definition(0, type_zero, constant(&mut arena, 0, Vec::new()));
    let signature = check_signature(&mut arena, &[first, second], &mut budget).unwrap();
    assert_eq!(signature.len(), 2);

    // Both definitions unfold: `d1` δ-steps to `d0`, which δ-steps to
    // `Two` — the index discipline is what keeps unfolding well-founded.
    let context = Context::empty().with_signature(signature);
    let last = constant(&mut arena, 1, Vec::new());
    let normalized =
        weak_head_normalize(&mut arena, context.signature(), last, &mut budget).unwrap();
    let two = arena.insert(Term::Two);
    assert!(arena.structurally_equal(normalized, two));
}

#[test]
fn check_signature_rejects_self_and_forward_references() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // A declaration's statement mentioning its own position is a
    // self-reference: under the empty prefix nothing resolves.
    let type_zero = type_sort(&mut arena, 0);
    let self_index = constant(&mut arena, 0, Vec::new());
    let self_reference = Declaration::assumption(0, pi(&mut arena, type_zero, self_index));
    let error = check_signature(&mut arena, &[self_reference], &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnknownDeclaration {
            declaration: 0,
            signature_len: 0,
        }
    );

    // A forward reference fails identically: the first declaration names
    // the second, but only the empty prefix is ambient when it checks.
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let type_zero = type_sort(&mut arena, 0);
    let forward_index = constant(&mut arena, 1, Vec::new());
    let forward = Declaration::assumption(0, pi(&mut arena, type_zero, forward_index));
    let type_zero = type_sort(&mut arena, 0);
    let later = Declaration::assumption(0, type_zero);
    let error = check_signature(&mut arena, &[forward, later], &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnknownDeclaration {
            declaration: 1,
            signature_len: 0,
        }
    );
}

#[test]
fn constant_instantiation_controls_are_checked() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let declaration = polymorphic_identity_declaration(&mut arena);
    let signature = check_signature(&mut arena, &[declaration], &mut budget).unwrap();

    // An index past the signature never resolves.
    let missing = constant(&mut arena, 7, vec![Level::Constant(0)]);
    let context = Context::empty().with_signature(signature.clone());
    let error = infer_type(&mut arena, &context, missing, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnknownDeclaration {
            declaration: 7,
            signature_len: 1,
        }
    );

    // The instantiation must supply exactly the declaration's arity.
    let under = constant(&mut arena, 0, Vec::new());
    let error = infer_type(&mut arena, &context, under, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: 0,
            expected: 1,
            supplied: 0,
        }
    );
    let over = constant(&mut arena, 0, vec![Level::Constant(0), Level::Constant(0)]);
    let error = infer_type(&mut arena, &context, over, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: 0,
            expected: 1,
            supplied: 2,
        }
    );

    // Every supplied level must be in the judgment's own scope: a closed
    // judgment cannot pass `Parameter(0)`.
    let escaped = constant(&mut arena, 0, vec![Level::Parameter(0)]);
    let error = infer_type(&mut arena, &context, escaped, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 0, arity: 0 }
    );

    // Under arity 1 the same argument is in scope and instantiates.
    let mut budget = default_budget();
    let context = Context::with_level_arity(1).with_signature(signature);
    let escaped = constant(&mut arena, 0, vec![Level::Parameter(0)]);
    let inferred = infer_type(&mut arena, &context, escaped, &mut budget).unwrap();
    let expected = instantiated_identity_type(&mut arena, Level::Parameter(0));
    assert!(arena.structurally_equal(inferred, expected));
}

#[test]
fn a_declaration_whose_statement_is_not_a_type_rejects() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `zero : Two` is an inhabitant, not a type — `infer_sort` refuses it.
    let two_zero = arena.insert(Term::TwoZero);
    let malformed = Declaration::assumption(0, two_zero);
    let error = check_signature(&mut arena, &[malformed], &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));
}

#[test]
fn a_declaration_body_must_inhabit_its_statement() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `d : Two := Type 0` — the body is a universe, not a boolean.
    let two = arena.insert(Term::Two);
    let type_zero = type_sort(&mut arena, 0);
    let malformed = Declaration::definition(0, two, type_zero);
    let error = check_signature(&mut arena, &[malformed], &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn a_declaration_statement_must_keep_its_own_level_scope() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // Arity 0 but the statement mentions `Parameter(0)` — the declaration
    // claims a level it never quantified over.
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let malformed = Declaration::assumption(0, type_u);
    let error = check_signature(&mut arena, &[malformed], &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 0, arity: 0 }
    );
}

#[test]
fn a_definition_constant_unfolds_and_an_assumption_stays_neutral() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let axiom = Declaration::assumption(1, type_u);
    let identity = polymorphic_identity_declaration(&mut arena);
    let signature = check_signature(&mut arena, &[axiom, identity], &mut budget).unwrap();
    let context = Context::with_level_arity(1).with_signature(signature);

    // The definition δ-unfolds to its instantiated body in one step.
    let defined = constant(&mut arena, 1, vec![Level::Parameter(0)]);
    let normalized =
        weak_head_normalize(&mut arena, context.signature(), defined, &mut budget).unwrap();
    let expected_body = {
        let type_u = sort_level(&mut arena, Level::Parameter(0));
        let bound_a = variable(&mut arena, 0);
        let inner_x = variable(&mut arena, 0);
        let inner = lambda(&mut arena, bound_a, inner_x);
        lambda(&mut arena, type_u, inner)
    };
    assert!(arena.structurally_equal(normalized, expected_body));

    // An unfolding is a budgeted step: an exhausted budget refuses it
    // rather than pretending the terms never convert.
    let mut empty_budget = Budget::new(0);
    let error = weak_head_normalize(&mut arena, context.signature(), defined, &mut empty_budget)
        .unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);

    // The assumption has no body: it is a neutral atom and stays stuck.
    let neutral = constant(&mut arena, 0, vec![Level::Parameter(0)]);
    let mut budget = default_budget();
    let normalized =
        weak_head_normalize(&mut arena, context.signature(), neutral, &mut budget).unwrap();
    assert_eq!(normalized, neutral);
}

#[test]
fn stuck_constants_convert_at_semantically_equal_instantiations() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `M : Type u` at arity 2 — the parameters it never uses still count
    // toward the instantiation length.
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let axiom = Declaration::assumption(2, type_u);
    let signature = check_signature(&mut arena, &[axiom], &mut budget).unwrap();
    let context = Context::with_level_arity(2).with_signature(signature);
    let u = Level::Parameter(0);
    let v = Level::Parameter(1);

    // `M(max(u, v))` and `M(max(v, u))` are the same assumption at
    // semantically equal instantiations — conversion decides it without
    // unfolding anything.
    let left = constant(&mut arena, 0, vec![u.clone().maximum(v.clone())]);
    let right = constant(&mut arena, 0, vec![v.clone().maximum(u.clone())]);
    let shared = sort_level(&mut arena, u.clone().maximum(v.clone()));
    assert!(
        convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap(),
        "the same assumption at equal levels must convert"
    );

    // Different instantiations are different assumptions.
    let other = constant(&mut arena, 0, vec![u.clone()]);
    assert!(
        !convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap(),
        "distinct level instantiations must not convert"
    );

    // And a different declaration is a different assumption entirely.
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let first = Declaration::assumption(1, type_u);
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    let second = Declaration::assumption(1, type_u);
    let signature = check_signature(&mut arena, &[first, second], &mut budget).unwrap();
    let context = Context::with_level_arity(1).with_signature(signature);
    let left = constant(&mut arena, 0, vec![Level::Parameter(0)]);
    let right = constant(&mut arena, 1, vec![Level::Parameter(0)]);
    let shared = sort_level(&mut arena, Level::Parameter(0));
    assert!(!convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());
}

#[test]
fn assumption_closure_reaches_through_statements_and_bodies() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `axiom : Type 0`; `uses : Type 0 := axiom`; `spare : Type 0` — a
    // judgment mentioning only `uses` still commits to `axiom`, while
    // the unused `spare` never enters the closure.
    let type_zero = type_sort(&mut arena, 0);
    let axiom = Declaration::assumption(0, type_zero);
    let type_zero = type_sort(&mut arena, 0);
    let axiom_constant = constant(&mut arena, 0, Vec::new());
    let uses = Declaration::definition(0, type_zero, axiom_constant);
    let type_zero = type_sort(&mut arena, 0);
    let spare = Declaration::assumption(0, type_zero);
    let signature = check_signature(&mut arena, &[axiom, uses, spare], &mut budget).unwrap();

    let closure = assumption_closure(&arena, &signature, &[1]);
    assert_eq!(closure, [0].into_iter().collect());

    // The same reachability from a judgment's terms: an evidence term
    // naming only `uses` reports `axiom` — and a statement-level
    // dependency counts even when conversion never unfolded it.
    let evidence = constant(&mut arena, 1, Vec::new());
    let closure = judgment_assumption_closure(&arena, &signature, &[evidence]);
    assert_eq!(closure, [0].into_iter().collect());

    // Declaring a dependency on an assumption itself records it.
    let direct = constant(&mut arena, 0, Vec::new());
    let closure = judgment_assumption_closure(&arena, &signature, &[direct]);
    assert_eq!(closure, [0].into_iter().collect());

    // Nothing depends on `spare`: no root reaches index 2.
    let closure = assumption_closure(&arena, &signature, &[0, 1]);
    assert_eq!(closure, [0].into_iter().collect());
}

#[test]
fn assumption_statements_carry_the_closure_too() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    // `axiom : Type 0`; `thm : axiom := zero`? — `axiom` is a type, so a
    // definition `thm : Type 0 → axiom`? Keep it simple: `wrap`'s
    // *statement* is a Π over `axiom` and its body never mentions it —
    // the closure still records the axiom because statements count.
    let type_zero = type_sort(&mut arena, 0);
    let axiom = Declaration::assumption(0, type_zero);
    // `wrap : Π(_ : axiom). Type 0` as an assumption whose statement
    // mentions the axiom. Under the signature, `axiom` is `Constant 0`.
    let axiom_ty = constant(&mut arena, 0, Vec::new());
    let type_zero = type_sort(&mut arena, 0);
    let wrap_ty = pi(&mut arena, axiom_ty, type_zero);
    let wrap = Declaration::assumption(0, wrap_ty);
    let signature = check_signature(&mut arena, &[axiom, wrap], &mut budget).unwrap();

    // The judgment's evidence is `wrap` alone; its statement's reference
    // to `axiom` is an assumption dependency of the whole judgment.
    let evidence = constant(&mut arena, 1, Vec::new());
    let closure = judgment_assumption_closure(&arena, &signature, &[evidence]);
    assert_eq!(closure, [0, 1].into_iter().collect());
}
