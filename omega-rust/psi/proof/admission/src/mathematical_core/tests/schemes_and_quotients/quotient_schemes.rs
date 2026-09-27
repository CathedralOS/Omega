use crate::mathematical_core::tests::{
    apply, constant, default_budget, id, is_set_zero, lambda, pi, quotient_signature, two,
    two_quotient, type_sort, variable,
};
use crate::mathematical_core::{
    Context, CoreError, Declaration, Level, QUOTIENT_ELIM, QUOTIENT_IS_SET, QUOTIENT_LIFT,
    QUOTIENT_SOUND, TermArena, check_signature, check_type, infer_type, quotient_scheme,
};

#[test]
fn quotient_beta_is_propositional_evidence_not_conversion() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `P : Π(_ : Q). Type 0`, `sets : Π(z : Q). isSet (P z)`,
    // `d : Π(a : Two). P (project a)`, `c : <coherence>`, `a : Two` —
    // the full eliminator obligation roster as hypotheses.
    let family_type = {
        let domain = family.quotient(&mut arena);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let sets_type = {
        let domain = family.quotient(&mut arena);
        let bound = variable(&mut arena, 0);
        let family_ref = variable(&mut arena, 1);
        let applied = apply(&mut arena, family_ref, bound);
        // `isSet[0] (P z)` under the `z` binder — `P` is ambient index
        // 0, so index 1 under `z`.
        let codomain = {
            let is_set = constant(&mut arena, QUOTIENT_IS_SET, vec![Level::Constant(0)]);
            apply(&mut arena, is_set, applied)
        };
        pi(&mut arena, domain, codomain)
    };
    let case_type = {
        let domain = two(&mut arena);
        // Under `a`: `P (project a)` — ambient `[P, sets]` puts `P` at
        // index 1, so under the `a` binder it is index 2.
        let bound = variable(&mut arena, 0);
        let projected = family.project(&mut arena, bound);
        let family_ref = variable(&mut arena, 2);
        let codomain = apply(&mut arena, family_ref, projected);
        pi(&mut arena, domain, codomain)
    };
    let coherence_type = {
        // `Π(a b : Two). Π(r : Id Two a b). Id (P (project b))
        //  (transport P a b r (d a)) (d b)` — ambient `[P, sets, d]`
        // has `d` at index 0 and `P` at index 2; under `a, b, r` add 3.
        let a_domain = two(&mut arena);
        let b_domain = two(&mut arena);
        let r_domain = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 1);
            let b = variable(&mut arena, 0);
            id(&mut arena, ty, a, b)
        };
        let codomain = {
            let b = variable(&mut arena, 1);
            let projected_b = family.project(&mut arena, b);
            let p = variable(&mut arena, 5);
            let p_at_b = apply(&mut arena, p, projected_b);
            let a = variable(&mut arena, 2);
            let b = variable(&mut arena, 1);
            let r = variable(&mut arena, 0);
            let d = variable(&mut arena, 3);
            let a2 = variable(&mut arena, 2);
            let case_at_a = apply(&mut arena, d, a2);
            let p = variable(&mut arena, 5);
            let transported = family.transport(&mut arena, zero.clone(), p, a, b, r, case_at_a);
            let d = variable(&mut arena, 3);
            let b = variable(&mut arena, 1);
            let case_at_b = apply(&mut arena, d, b);
            id(&mut arena, p_at_b, transported, case_at_b)
        };
        let inner = pi(&mut arena, r_domain, codomain);
        let middle = pi(&mut arena, b_domain, inner);
        pi(&mut arena, a_domain, middle)
    };
    let representative_binding = two(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(family_type)
        .extend(sets_type)
        .extend(case_type)
        .extend(coherence_type)
        .extend(representative_binding);

    // `beta P sets d c a : Id (P (project a)) (elim P sets d c
    //  (project a)) (d a)` — the point law typechecks as a relevant
    // identity.
    let p = variable(&mut arena, 4);
    let sets = variable(&mut arena, 3);
    let d = variable(&mut arena, 2);
    let c = variable(&mut arena, 1);
    let a = variable(&mut arena, 0);
    let point = family.beta(&mut arena, zero.clone(), p, sets, d, c, a);
    let expected = {
        let a = variable(&mut arena, 0);
        let projected_a = family.project(&mut arena, a);
        let p = variable(&mut arena, 4);
        let left_ty = apply(&mut arena, p, projected_a);
        let p = variable(&mut arena, 4);
        let sets = variable(&mut arena, 3);
        let d = variable(&mut arena, 2);
        let c = variable(&mut arena, 1);
        let a = variable(&mut arena, 0);
        let projected = family.project(&mut arena, a);
        let section = family.elim(&mut arena, zero, p, sets, d, c, projected);
        let d = variable(&mut arena, 2);
        let a = variable(&mut arena, 0);
        let case = apply(&mut arena, d, a);
        id(&mut arena, left_ty, section, case)
    };
    check_type(&mut arena, &context, point, expected, &mut budget).unwrap();
}

#[test]
fn quotient_lift_rejects_a_malformed_congruence() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // `respect` must relate `f a` to `f b` *in that order*: a flipped
    // congruence `Id Two (f b) (f a)` is a different theorem and the
    // application rejects.
    let two_ty = two(&mut arena);
    let set_b = is_set_zero(&mut arena, two_ty);
    let operation = {
        let domain = two(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let flipped = {
        let domain = two(&mut arena);
        let inner_domain = two(&mut arena);
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        let related = id(&mut arena, ty, a, b);
        let ty = two(&mut arena);
        let f = variable(&mut arena, 3);
        let b = variable(&mut arena, 1);
        let left = apply(&mut arena, f, b);
        let f = variable(&mut arena, 3);
        let a = variable(&mut arena, 2);
        let right = apply(&mut arena, f, a);
        let conclusion = id(&mut arena, ty, left, right);
        let inner = pi(&mut arena, related, conclusion);
        let middle = pi(&mut arena, inner_domain, inner);
        pi(&mut arena, domain, middle)
    };
    let element = family.quotient(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(set_b)
        .extend(operation)
        .extend(flipped)
        .extend(element);

    let set_b = variable(&mut arena, 3);
    let operation = variable(&mut arena, 2);
    let flipped = variable(&mut arena, 1);
    let element = variable(&mut arena, 0);
    let result_ty = two(&mut arena);
    let wrong = family.lift(
        &mut arena,
        zero.clone(),
        result_ty,
        set_b,
        operation,
        flipped,
        element,
    );
    let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "a flipped congruence is a different theorem: {error:?}"
    );

    // And the congruence theorem is not optional: applying the
    // section's `z` where `respect` belongs rejects — the theorem
    // argument is explicit, never searched or elided.
    let lift = constant(&mut arena, QUOTIENT_LIFT, vec![zero.clone(); 3]);
    let applied = apply(&mut arena, lift, family.carrier);
    let applied = apply(&mut arena, applied, family.relation);
    let result_ty = two(&mut arena);
    let applied = apply(&mut arena, applied, result_ty);
    let set_b = variable(&mut arena, 3);
    let applied = apply(&mut arena, applied, set_b);
    let operation = variable(&mut arena, 2);
    let applied = apply(&mut arena, applied, operation);
    let element = variable(&mut arena, 0);
    let missing_theorem = apply(&mut arena, applied, element);
    let error = infer_type(&mut arena, &context, missing_theorem, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "the section value is not the congruence theorem: {error:?}"
    );
}

#[test]
fn quotient_lift_precondition_admits_the_transport_backed_shape() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);
    let one = Level::Constant(1);

    // `Pub = λ(_ : Q). Type 0` and `Pre = λ(_ : Two). Type 0` — premise
    // families whose evidence is a type. Context: `setB : isSet Two`,
    // `t : Π(a : Two). Π(_ : Type 0). Type 0`, `f : Π(a : Two). Π(_ :
    // Type 0). Two`, `respect : Π(a b : Two). Id Two a b → Π(_ _ : Type
    // 0). Id Two (f a (t a ha)) (f b (t b hb))`, `z : Q`, `h : Type 0`
    // (`Pub z` unfolded).
    let two_ty = two(&mut arena);
    let set_b = is_set_zero(&mut arena, two_ty);
    let public = {
        let domain = family.quotient(&mut arena);
        let codomain = type_sort(&mut arena, 0);
        lambda(&mut arena, domain, codomain)
    };
    let representative_premise = {
        let domain = two(&mut arena);
        let codomain = type_sort(&mut arena, 0);
        lambda(&mut arena, domain, codomain)
    };
    let transport_type = {
        let domain = two(&mut arena);
        let premise = type_sort(&mut arena, 0);
        let conclusion = type_sort(&mut arena, 0);
        let inner = pi(&mut arena, premise, conclusion);
        pi(&mut arena, domain, inner)
    };
    let operation_type = {
        let domain = two(&mut arena);
        let premise = type_sort(&mut arena, 0);
        let conclusion = two(&mut arena);
        let inner = pi(&mut arena, premise, conclusion);
        pi(&mut arena, domain, inner)
    };
    let respect_type = {
        // Under ambient `[setB, t, f]` and binders `a, b, r, ha, hb`:
        // indices are hb = 0, ha = 1, r = 2, b = 3, a = 4, f = 5,
        // t = 6.
        let a_domain = two(&mut arena);
        let b_domain = two(&mut arena);
        let r_domain = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 1);
            let b = variable(&mut arena, 0);
            id(&mut arena, ty, a, b)
        };
        let ha_domain = type_sort(&mut arena, 0);
        let hb_domain = type_sort(&mut arena, 0);
        let codomain = {
            let f = variable(&mut arena, 5);
            let a = variable(&mut arena, 4);
            let left = apply(&mut arena, f, a);
            let t = variable(&mut arena, 6);
            let a = variable(&mut arena, 4);
            let transported = apply(&mut arena, t, a);
            let ha = variable(&mut arena, 1);
            let transported = apply(&mut arena, transported, ha);
            let left = apply(&mut arena, left, transported);
            let f = variable(&mut arena, 5);
            let b = variable(&mut arena, 3);
            let right = apply(&mut arena, f, b);
            let t = variable(&mut arena, 6);
            let b = variable(&mut arena, 3);
            let transported = apply(&mut arena, t, b);
            let hb = variable(&mut arena, 0);
            let transported = apply(&mut arena, transported, hb);
            let right = apply(&mut arena, right, transported);
            let ty = two(&mut arena);
            id(&mut arena, ty, left, right)
        };
        let inner = pi(&mut arena, hb_domain, codomain);
        let inner = pi(&mut arena, ha_domain, inner);
        let inner = pi(&mut arena, r_domain, inner);
        let middle = pi(&mut arena, b_domain, inner);
        pi(&mut arena, a_domain, middle)
    };
    let element = family.quotient(&mut arena);
    let premise = type_sort(&mut arena, 0);
    let context = Context::empty()
        .with_signature(signature)
        .extend(set_b)
        .extend(transport_type)
        .extend(operation_type)
        .extend(respect_type)
        .extend(element)
        .extend(premise);

    // `liftPre Two R Two setB Pub Pre t f respect z h : B` — the
    // forward-transport operation admits with its complete roster.
    let result_ty = two(&mut arena);
    let set_b_var = variable(&mut arena, 5);
    let transport_var = variable(&mut arena, 4);
    let operation_var = variable(&mut arena, 3);
    let respect_var = variable(&mut arena, 2);
    let element_var = variable(&mut arena, 1);
    let premise_var = variable(&mut arena, 0);
    let lifted = family.lift_precondition(
        &mut arena,
        zero.clone(),
        one.clone(),
        one,
        result_ty,
        set_b_var,
        public,
        representative_premise,
        transport_var,
        operation_var,
        respect_var,
        element_var,
        premise_var,
    );
    let result = two(&mut arena);
    check_type(&mut arena, &context, lifted, result, &mut budget).unwrap();
}

#[test]
fn quotient_scheme_rejects_a_wrong_derived_body() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let mut declarations = quotient_scheme(&mut arena);

    // Tamper with `sound`: replace the admitted assumption with a
    // "definition" whose body returns the relation witness — `r : R a b`
    // is not the `Id Q (project a) (project b)` the statement claims.
    // `check_signature` re-decides every body against its own statement,
    // so a producer's claimed law is never trusted.
    let sound_arity = declarations[QUOTIENT_SOUND as usize].level_arity;
    let sound_ty = declarations[QUOTIENT_SOUND as usize].ty;
    let wrong_body = {
        // λ(A : Type u). λ(R : A → A → Type v). λ(a : A). λ(b : A).
        //   λ(r : R a b). r
        let a_ty = type_sort(&mut arena, 0);
        let rel_ty = {
            // `A → A → Type v` under the `A` binder: the outer domain's
            // `A` is index 0; under that Π's binder, `A` is index 1.
            let a = variable(&mut arena, 0);
            let codomain = type_sort(&mut arena, 1);
            let a_inner = variable(&mut arena, 1);
            let inner = pi(&mut arena, a_inner, codomain);
            pi(&mut arena, a, inner)
        };
        let r_var = variable(&mut arena, 0);
        let r_dom = {
            // `R a b` under `a, b`: `R` is index 2, `a` index 1, `b` 0.
            let relation = variable(&mut arena, 2);
            let a = variable(&mut arena, 1);
            let applied = apply(&mut arena, relation, a);
            let b = variable(&mut arena, 0);
            apply(&mut arena, applied, b)
        };
        let inner = lambda(&mut arena, r_dom, r_var);
        // `b`'s domain is `A` under `A, R, a`: index 2.
        let b_ty = variable(&mut arena, 2);
        let inner = lambda(&mut arena, b_ty, inner);
        // `a`'s domain is `A` under `A, R`: index 1.
        let a_ty2 = variable(&mut arena, 1);
        let inner = lambda(&mut arena, a_ty2, inner);
        let inner = lambda(&mut arena, rel_ty, inner);
        lambda(&mut arena, a_ty, inner)
    };
    declarations[QUOTIENT_SOUND as usize] =
        Declaration::definition(sound_arity, sound_ty, wrong_body);
    let error = check_signature(&mut arena, &declarations, &mut budget).unwrap_err();
    assert!(
        matches!(
            error,
            CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
        ),
        "a wrong body never passes as the law: {error:?}"
    );
}

#[test]
fn quotient_elim_rejects_missing_set_evidence() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // `elim` cannot run without its `sets` obligation: passing `h :
    // Two` where `Π(z : Q). isSet (P z)` is required rejects — no
    // set-valued elimination evidence is ever inferred or elided.
    let context = Context::empty()
        .with_signature(signature)
        .extend(two(&mut arena));

    let premise_family = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        lambda(&mut arena, domain, codomain)
    };
    let elim = constant(&mut arena, QUOTIENT_ELIM, vec![zero; 3]);
    let applied = apply(&mut arena, elim, family.carrier);
    let applied = apply(&mut arena, applied, family.relation);
    let applied = apply(&mut arena, applied, premise_family);
    // `Two` where the `sets` obligation belongs.
    let not_sets = variable(&mut arena, 0);
    let missing = apply(&mut arena, applied, not_sets);
    let error = infer_type(&mut arena, &context, missing, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "elimination without the setness obligation rejects: {error:?}"
    );
}
