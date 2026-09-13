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
