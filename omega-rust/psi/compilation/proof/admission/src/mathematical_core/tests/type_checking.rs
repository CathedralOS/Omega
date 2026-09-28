use super::{
    apply, branching_family, case_two, default_budget, fst, lambda, pair, pi, sigma, snd,
    strict_sort, two, two_one, two_zero, type_sort, universe_motive, variable,
};
use crate::mathematical_core::{
    Budget, Context, CoreError, Signature, TermArena, check_type, convertible, infer_type,
    weak_head_normalize,
};

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
