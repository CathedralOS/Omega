use super::{
    apply, concrete_step, default_budget, id, id_elim, ind_w, lambda, pair, pi, refl, sigma,
    strict_sort, sup, symmetry_motive, two, two_one, two_zero, type_sort, variable, w_context,
    w_type,
};
use crate::mathematical_core::{
    Budget, Context, CoreError, Signature, TermArena, check_type, convertible, infer_type,
    weak_head_normalize,
};

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
