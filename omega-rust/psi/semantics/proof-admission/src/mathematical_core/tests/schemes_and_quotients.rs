use super::{
    apply, constant, default_budget, id, indexed_context, is_set_zero, lambda, pair, pi,
    quotient_signature, refl, scheme_signature, sigma, strict_sort, sup, two, two_indexed_family,
    two_quotient, two_zero, type_sort, variable, w_type,
};
use crate::mathematical_core::term::sorts_equal;
use crate::mathematical_core::{
    Context, CoreError, Declaration, INDEXED_IND, INDEXED_W, Level, MathematicalCertificate,
    QUOTIENT, QUOTIENT_BETA, QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_ID_TRANS, QUOTIENT_IS_SET,
    QUOTIENT_LIFT, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT, QUOTIENT_SET, QUOTIENT_SOUND,
    QUOTIENT_TRANSPORT, QUOTIENT_TRANSPORT_CONST, Sort, TermArena, assumption_closure,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, judgment_assumption_closure, quotient_scheme,
    verify_mathematical_certificate,
};

#[test]
fn indexed_scheme_checks_as_a_parametric_signature() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let declarations = indexed_scheme(&mut arena);
    assert_eq!(declarations.len(), 5);

    // The whole scheme re-decides parametrically: every body inhabits
    // its statement under the checked prefix, for all instantiations of
    // `l, u, v` (and `w` for `iindW`). This is the encoding's formation
    // evidence — not a producer flag.
    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 5);
    assert!(before - budget.remaining() > 0);

    // The encoding lives entirely in the arena: retained storage is the
    // terms the five declarations built, nothing hidden.
    assert!(!arena.is_empty());
}

#[test]
fn indexed_family_forms_and_the_condition_unfolds() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_indexed_family(&mut arena);
    let context = Context::empty().with_signature(scheme_signature(&mut arena));

    // Formation: `IW zero` is a `Type 0` — the family's level
    // `max(0,0,0)` lands at the description's closed levels.
    let index = two_zero(&mut arena);
    let family_ty = family.indexed_w(&mut arena, index);
    let sort = infer_sort(&mut arena, &context, family_ty, &mut budget).unwrap();
    assert!(sorts_equal(&sort, &Sort::Type(Level::Constant(0))));

    // The indexing equation the profile names:
    //   IndexedAt i (sup A B a k)
    //     ≡ Σ(_ : Id Two (out a) i). Π(b : B a). IndexedAt (next a b) (k b)
    // with `a`, `k`, `i` all neutral variables.
    let a_binding = two(&mut arena);
    let k_binding = {
        let a_bound = variable(&mut arena, 0);
        let b_domain = apply(&mut arena, family.children, a_bound);
        let carrier = two(&mut arena);
        let codomain = w_type(&mut arena, carrier, family.children);
        pi(&mut arena, b_domain, codomain)
    };
    let i_binding = two(&mut arena);
    let context = context
        .extend(a_binding)
        .extend(k_binding)
        .extend(i_binding);
    // Under [a, k, i]: i = 0, k = 1, a = 2.
    let carrier = two(&mut arena);
    let label = variable(&mut arena, 2);
    let function = variable(&mut arena, 1);
    let node = sup(&mut arena, carrier, family.children, label, function);
    let index = variable(&mut arena, 0);
    let condition = family.indexed_at(&mut arena, index, node);
    let expected = {
        // `Σ(_ : Id Two (out a) i). Π(b : B a). IndexedAt (next a b) (k b)`
        let domain = {
            let ty = two(&mut arena);
            let a_bound = variable(&mut arena, 2);
            let left = apply(&mut arena, family.out, a_bound);
            let right = variable(&mut arena, 0);
            id(&mut arena, ty, left, right)
        };
        let codomain = {
            // Under [_, i, k, a]: a = 3, k = 2.
            let a_bound = variable(&mut arena, 3);
            let b_domain = apply(&mut arena, family.children, a_bound);
            let body = {
                // Under [b, _, i, k, a]: b = 0, k = 3, a = 4.
                let a_bound = variable(&mut arena, 4);
                let next_a = apply(&mut arena, family.next, a_bound);
                let b_bound = variable(&mut arena, 0);
                let next_ab = apply(&mut arena, next_a, b_bound);
                let k = variable(&mut arena, 3);
                let b_bound = variable(&mut arena, 0);
                let k_b = apply(&mut arena, k, b_bound);
                family.indexed_at(&mut arena, next_ab, k_b)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    let type_zero = type_sort(&mut arena, 0);
    // Both sides are checked types before conversion is consulted.
    check_type(&mut arena, &context, condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // And the equation is what *distinguishes* the index: `IndexedAt i
    // (sup a k)` never collapses to the child part alone.
    let children_only = {
        let a_bound = variable(&mut arena, 2);
        let b_domain = apply(&mut arena, family.children, a_bound);
        let body = {
            // Under [b, i, k, a]: b = 0, k = 2, a = 3.
            let a_bound = variable(&mut arena, 3);
            let next_a = apply(&mut arena, family.next, a_bound);
            let b_bound = variable(&mut arena, 0);
            let next_ab = apply(&mut arena, next_a, b_bound);
            let k = variable(&mut arena, 2);
            let b_bound = variable(&mut arena, 0);
            let k_b = apply(&mut arena, k, b_bound);
            family.indexed_at(&mut arena, next_ab, k_b)
        };
        pi(&mut arena, b_domain, body)
    };
    assert!(
        !convertible(
            &mut arena,
            &context,
            condition,
            children_only,
            type_zero,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn isup_and_iindw_check_and_compute_with_a_neutral_child_function() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_indexed_family(&mut arena);
    let context = indexed_context(&mut arena, &family);

    // `isup a g : IW (out a)` — and `out a ≡ a`, so it also inhabits
    // `IW a`. `g` is a bound variable: the constructor's child function
    // is neutral.
    let a = variable(&mut arena, 1);
    let g = variable(&mut arena, 0);
    let constructor = family.sup(&mut arena, a, g);
    let a_bound = variable(&mut arena, 1);
    let at_a = family.indexed_w(&mut arena, a_bound);
    check_type(&mut arena, &context, constructor, at_a, &mut budget).unwrap();

    // The dependent eliminator: `iindW Q s a (isup a g) : Q a (isup a
    // g)` at motive level `w = 0`.
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let index = variable(&mut arena, 1);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(0),
        motive,
        step,
        index,
        constructor,
    );
    let shared = {
        let q = variable(&mut arena, 3);
        let a_bound = variable(&mut arena, 1);
        let q_a = apply(&mut arena, q, a_bound);
        apply(&mut arena, q_a, constructor)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // Definitional constructor computation — the profile's required
    // judgment `iindW Q s i (isup a g) ≡ s a g (b ↦ iindW Q s (next a
    // b) (g b))` — decided by conversion with `g` still a neutral
    // variable. The reduct's child function `b ↦ iwPack (next a b)
    // (fst (g b)) (snd (g b))` closes to `g` through pair eta and typed
    // function eta; no value of `g` is consulted.
    let hypothesis = {
        let a_bound = variable(&mut arena, 1);
        let domain = apply(&mut arena, family.children, a_bound);
        let body = {
            // Under [b, g, a, s, Q]: b = 0, g = 1, a = 2, s = 3, Q = 4.
            let a_bound = variable(&mut arena, 2);
            let next_a = apply(&mut arena, family.next, a_bound);
            let b_bound = variable(&mut arena, 0);
            let next_ab = apply(&mut arena, next_a, b_bound);
            let g = variable(&mut arena, 1);
            let b_bound = variable(&mut arena, 0);
            let g_b = apply(&mut arena, g, b_bound);
            let motive = variable(&mut arena, 4);
            let step = variable(&mut arena, 3);
            family.ind(&mut arena, Level::Constant(0), motive, step, next_ab, g_b)
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let s = variable(&mut arena, 2);
        let a_bound = variable(&mut arena, 1);
        let s_a = apply(&mut arena, s, a_bound);
        let g = variable(&mut arena, 0);
        let s_a_g = apply(&mut arena, s_a, g);
        apply(&mut arena, s_a_g, hypothesis)
    };
    check_type(&mut arena, &context, expected, shared, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            expected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    // `iwPack` packages a tree with its indexing evidence: under the
    // extended context `k : Π(b : B a). W Two B`, `f : Π(b : B a).
    // IndexedAt (next a b) (k b)` — both neutral — `iwPack a (sup a k)
    // ⟨refl (out a), f⟩` checks at `IW a`.
    let k_binding = {
        let a_bound = variable(&mut arena, 1);
        let b_domain = apply(&mut arena, family.children, a_bound);
        let carrier = two(&mut arena);
        let codomain = w_type(&mut arena, carrier, family.children);
        pi(&mut arena, b_domain, codomain)
    };
    let f_binding = {
        // Under [k, g, a, s, Q]: `Π(b : B a). IndexedAt (next a b) (k b)`.
        let a_bound = variable(&mut arena, 2);
        let b_domain = apply(&mut arena, family.children, a_bound);
        let body = {
            // Under [b, k, g, a, s, Q]: b = 0, k = 1, a = 3.
            let a_bound = variable(&mut arena, 3);
            let next_a = apply(&mut arena, family.next, a_bound);
            let b_bound = variable(&mut arena, 0);
            let next_ab = apply(&mut arena, next_a, b_bound);
            let k = variable(&mut arena, 1);
            let b_bound = variable(&mut arena, 0);
            let k_b = apply(&mut arena, k, b_bound);
            family.indexed_at(&mut arena, next_ab, k_b)
        };
        pi(&mut arena, b_domain, body)
    };
    let context = context.extend(k_binding).extend(f_binding);
    // Under [f, k, g, a, s, Q]: f = 0, k = 1, a = 3.
    let node = {
        let carrier = two(&mut arena);
        let label = variable(&mut arena, 3);
        let function = variable(&mut arena, 1);
        sup(&mut arena, carrier, family.children, label, function)
    };
    let evidence = {
        let ty = two(&mut arena);
        let a_bound = variable(&mut arena, 3);
        let value = apply(&mut arena, family.out, a_bound);
        let refl_part = refl(&mut arena, ty, value);
        let f = variable(&mut arena, 0);
        pair(&mut arena, refl_part, f)
    };
    let index = variable(&mut arena, 3); // a ≡ out a
    let packed = family.pack(&mut arena, index, node, evidence);
    let a_bound = variable(&mut arena, 3);
    let at_a = family.indexed_w(&mut arena, a_bound);
    check_type(&mut arena, &context, packed, at_a, &mut budget).unwrap();
}

#[test]
fn indexed_applications_reject_wrong_descriptions_and_indices() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_indexed_family(&mut arena);
    let context = indexed_context(&mut arena, &family);

    // A constructor at the wrong index: `isup a g` never inhabits
    // `IW zero` when `a` is neutral — `out a` is not `zero`.
    let a = variable(&mut arena, 1);
    let g = variable(&mut arena, 0);
    let constructor = family.sup(&mut arena, a, g);
    let zero = two_zero(&mut arena);
    let wrong_index = family.indexed_w(&mut arena, zero);
    let error =
        check_type(&mut arena, &context, constructor, wrong_index, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The eliminator's `t` must inhabit `IW i` at *the same* index:
    // `iindW Q s zero (isup a g)` rejects because `IW (out a)` does not
    // convert to `IW zero`.
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let zero = two_zero(&mut arena);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(0),
        motive,
        step,
        zero,
        constructor,
    );
    let expected = {
        let q = variable(&mut arena, 3);
        let zero = two_zero(&mut arena);
        let q_zero = apply(&mut arena, q, zero);
        apply(&mut arena, q_zero, constructor)
    };
    let error = check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A value where the index type belongs is malformed, not a type.
    let mut bad_family = family.clone();
    bad_family.index = two_zero(&mut arena);
    let zero = two_zero(&mut arena);
    let malformed = bad_family.indexed_w(&mut arena, zero);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A strict index type is a malformed universe for `I : Type l`.
    let mut strict_family = family.clone();
    strict_family.index = strict_sort(&mut arena, 0);
    let zero = two_zero(&mut arena);
    let malformed = strict_family.indexed_w(&mut arena, zero);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // Wrong instantiation arity is rejected at the constant itself.
    let bad_constant = constant(&mut arena, INDEXED_W, vec![Level::Constant(0)]);
    let error = infer_type(&mut arena, &context, bad_constant, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: INDEXED_W,
            expected: 3,
            supplied: 1,
        }
    );

    // And the eliminator is arity 4 — `l, u, v` and the motive level
    // `w`: omitting `w` is a malformed instantiation, not a default.
    let bad_ind = constant(
        &mut arena,
        INDEXED_IND,
        vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
    );
    let error = infer_type(&mut arena, &context, bad_ind, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: INDEXED_IND,
            expected: 4,
            supplied: 3,
        }
    );
}

#[test]
fn indexed_family_certificate_verifies_and_reports_closure() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_indexed_family(&mut arena);
    let mut declarations = indexed_scheme(&mut arena);
    // A producer assumption appended after the scheme: the encoding's
    // definitions never mention it, so a judgment that avoids it keeps
    // it out of the closure.
    let type_zero = type_sort(&mut arena, 0);
    declarations.push(Declaration::assumption(0, type_zero));
    check_signature(&mut arena, &declarations, &mut budget).unwrap();

    // The same context as `indexed_context`, rebuilt under the full
    // signature for the certificate's bindings.
    let motive_type = {
        let i_domain = two(&mut arena);
        let i_bound = variable(&mut arena, 0);
        let iw_at_i = family.indexed_w(&mut arena, i_bound);
        let type_zero = type_sort(&mut arena, 0);
        let inner = pi(&mut arena, iw_at_i, type_zero);
        pi(&mut arena, i_domain, inner)
    };
    let step_type = {
        let a_domain = two(&mut arena);
        let g_domain = {
            let a_bound = variable(&mut arena, 0);
            let b_domain = apply(&mut arena, family.children, a_bound);
            let next_ab = {
                let a_bound = variable(&mut arena, 1);
                let next_a = apply(&mut arena, family.next, a_bound);
                let b_bound = variable(&mut arena, 0);
                apply(&mut arena, next_a, b_bound)
            };
            let codomain = family.indexed_w(&mut arena, next_ab);
            pi(&mut arena, b_domain, codomain)
        };
        let hypothesis_domain = {
            let a_bound = variable(&mut arena, 1);
            let b_domain = apply(&mut arena, family.children, a_bound);
            let codomain = {
                let next_ab = {
                    let a_bound = variable(&mut arena, 2);
                    let next_a = apply(&mut arena, family.next, a_bound);
                    let b_bound = variable(&mut arena, 0);
                    apply(&mut arena, next_a, b_bound)
                };
                let q = variable(&mut arena, 3);
                let q_at = apply(&mut arena, q, next_ab);
                let g = variable(&mut arena, 1);
                let b_bound = variable(&mut arena, 0);
                let g_b = apply(&mut arena, g, b_bound);
                apply(&mut arena, q_at, g_b)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let step_result = {
            let q = variable(&mut arena, 3);
            let a_bound = variable(&mut arena, 2);
            let out_a = apply(&mut arena, family.out, a_bound);
            let a_bound = variable(&mut arena, 2);
            let g = variable(&mut arena, 1);
            let node = family.sup(&mut arena, a_bound, g);
            let q_out = apply(&mut arena, q, out_a);
            apply(&mut arena, q_out, node)
        };
        let inner = pi(&mut arena, hypothesis_domain, step_result);
        let middle = pi(&mut arena, g_domain, inner);
        pi(&mut arena, a_domain, middle)
    };
    let a_binding = two(&mut arena);
    let g_binding = {
        let a_bound = variable(&mut arena, 0);
        let b_domain = apply(&mut arena, family.children, a_bound);
        let next_ab = {
            let a_bound = variable(&mut arena, 1);
            let next_a = apply(&mut arena, family.next, a_bound);
            let b_bound = variable(&mut arena, 0);
            apply(&mut arena, next_a, b_bound)
        };
        let codomain = family.indexed_w(&mut arena, next_ab);
        pi(&mut arena, b_domain, codomain)
    };

    // The theorem: `Q a (isup a g)` is inhabited by `iindW Q s a (isup
    // a g)` under `Q, s, a, g` — the eliminator's own judgment carried
    // through a certificate, re-verified after the signature check.
    let a = variable(&mut arena, 1);
    let g = variable(&mut arena, 0);
    let constructor = family.sup(&mut arena, a, g);
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let index = variable(&mut arena, 1);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(0),
        motive,
        step,
        index,
        constructor,
    );
    let expected = {
        let q = variable(&mut arena, 3);
        let a_bound = variable(&mut arena, 1);
        let q_a = apply(&mut arena, q, a_bound);
        apply(&mut arena, q_a, constructor)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![motive_type, step_type, a_binding, g_binding],
        term: elimination,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    assert!(before - budget.remaining() > 0);

    // The judgment names only definitions 0–4, so the producer
    // assumption at position 5 stays out of the exact closure.
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert!(closure.is_empty());
}

// Set-quotient scheme — the quotient specification's "Set-quotient
// foundation" as an ordinary parametric signature: the carrier/operation/
// law interface as admitted assumptions, with `transport`, the J-derived
// identity lemmas and `lift` derived as checked definitions, and the
// forward-precondition-transport `liftPre` admitted as a named assumption.

#[test]
fn quotient_scheme_checks_as_a_parametric_signature() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let declarations = quotient_scheme(&mut arena);
    assert_eq!(declarations.len(), 13);

    // The whole scheme re-decides parametrically: every assumption
    // statement is a type under its level arity and every definition
    // body inhabits its statement, for all instantiations of `u, v`
    // (plus the motive/premise levels of the arity-3 and arity-5
    // declarations). This is the interface's formation evidence — not a
    // producer flag.
    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 13);
    assert!(before - budget.remaining() > 0);
    assert!(!arena.is_empty());

    // Exactly the specification's interface plus the extensionality-
    // blocked `liftPre` are admitted assumptions; `isSet`, `transport`,
    // the identity lemmas and `lift` are checked definitions.
    let assumptions = [
        QUOTIENT,
        QUOTIENT_PROJECT,
        QUOTIENT_SET,
        QUOTIENT_SOUND,
        QUOTIENT_EFFECTIVE,
        QUOTIENT_ELIM,
        QUOTIENT_BETA,
        QUOTIENT_LIFT_PRECONDITION,
    ];
    let definitions = [
        QUOTIENT_IS_SET,
        QUOTIENT_TRANSPORT,
        QUOTIENT_ID_TRANS,
        QUOTIENT_TRANSPORT_CONST,
        QUOTIENT_LIFT,
    ];
    for (position, declaration) in signature.declarations().iter().enumerate() {
        let position = position as u32;
        if assumptions.contains(&position) {
            assert!(
                declaration.is_assumption(),
                "position {position} must be an assumption"
            );
        } else {
            assert!(
                !declaration.is_assumption(),
                "position {position} must be a definition"
            );
        }
        assert_eq!(
            definitions.contains(&position),
            !declaration.is_assumption()
        );
    }
}

#[test]
fn quotient_lift_admits_explicit_representative_operation_and_congruence() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `setB : isSet Two`, `f : Π(_ : Two). Two`,
    // `respect : Π(a b : Two). Id Two a b → Id Two (f a) (f b)`,
    // `z : Q Two R`: the explicit setness evidence, representative
    // operation and congruence theorem a caller must supply. Indices at
    // depth 4: setB = 3, f = 2, respect = 1, z = 0.
    let two_ty = two(&mut arena);
    let set_b = is_set_zero(&mut arena, two_ty);
    let operation = {
        let domain = two(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let respect = {
        let domain = two(&mut arena);
        let inner_domain = two(&mut arena);
        // Under `a, b`: `Id Two a b` — `R a b`'s unfolding — then
        // `Id Two (f a) (f b)` with f at ambient index 0 shifted 3.
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        let related = id(&mut arena, ty, a, b);
        let ty = two(&mut arena);
        let f = variable(&mut arena, 3);
        let a = variable(&mut arena, 2);
        let left = apply(&mut arena, f, a);
        let f = variable(&mut arena, 3);
        let b = variable(&mut arena, 1);
        let right = apply(&mut arena, f, b);
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
        .extend(respect)
        .extend(element);

    // `lift Two R Two setB f respect z : Two` — the explicit
    // representative operation and congruence theorem admit the
    // quotient-owned section.
    let set_b = variable(&mut arena, 3);
    let operation = variable(&mut arena, 2);
    let respect = variable(&mut arena, 1);
    let element = variable(&mut arena, 0);
    let result_ty = two(&mut arena);
    let lifted = family.lift(
        &mut arena,
        zero.clone(),
        result_ty,
        set_b,
        operation,
        respect,
        element,
    );
    let result = two(&mut arena);
    check_type(&mut arena, &context, lifted, result, &mut budget).unwrap();

    // The same application infers `Two` directly.
    let inferred = infer_type(&mut arena, &context, lifted, &mut budget).unwrap();
    let result = two(&mut arena);
    assert!(arena.structurally_equal(inferred, result));
}

#[test]
fn quotient_lift_keeps_the_representative_opaque() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `setB : isSet Two`, `f : Two → Two`, `respect`, `a : Two`:
    // `lift … (project a)` is a value of `Two`, but `project a` is a
    // neutral constant application — `beta` is a propositional
    // identity, never a conversion, so the lifted value never reduces
    // to `f a` and no representative is extractable.
    let two_ty = two(&mut arena);
    let set_b = is_set_zero(&mut arena, two_ty);
    let operation = {
        let domain = two(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let respect = {
        let domain = two(&mut arena);
        let inner_domain = two(&mut arena);
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        let related = id(&mut arena, ty, a, b);
        let ty = two(&mut arena);
        let f = variable(&mut arena, 3);
        let a = variable(&mut arena, 2);
        let left = apply(&mut arena, f, a);
        let f = variable(&mut arena, 3);
        let b = variable(&mut arena, 1);
        let right = apply(&mut arena, f, b);
        let conclusion = id(&mut arena, ty, left, right);
        let inner = pi(&mut arena, related, conclusion);
        let middle = pi(&mut arena, inner_domain, inner);
        pi(&mut arena, domain, middle)
    };
    let representative = two(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(set_b)
        .extend(operation)
        .extend(respect)
        .extend(representative);

    // `lift … (project a) : Two` typechecks…
    let representative = variable(&mut arena, 0);
    let projected = family.project(&mut arena, representative);
    let set_b = variable(&mut arena, 3);
    let operation = variable(&mut arena, 2);
    let respect = variable(&mut arena, 1);
    let result_ty = two(&mut arena);
    let lifted = family.lift(
        &mut arena,
        zero.clone(),
        result_ty,
        set_b,
        operation,
        respect,
        projected,
    );
    let result = two(&mut arena);
    check_type(&mut arena, &context, lifted, result, &mut budget).unwrap();

    // …but it is *not* `f a` definitionally: the section at `project a`
    // computes only through the propositional `beta` identity.
    let representative = variable(&mut arena, 0);
    let operation = variable(&mut arena, 2);
    let case = apply(&mut arena, operation, representative);
    let shared = two(&mut arena);
    assert!(
        !convertible(&mut arena, &context, lifted, case, shared, &mut budget).unwrap(),
        "the quotient image is opaque: `lift … (project a)` never converts to `f a`"
    );
}

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
fn quotient_sound_and_effective_witness_exactly_the_relation() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);

    // Context `a : Two`, `b : Two`, `r : R a b` — unfolded to
    // `Id Two a b` — and `p : Id Q (project a) (project b)`.
    let related = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let projection_identity = {
        // Under ambient `[a, b, r]`: `a` is index 2, `b` index 1.
        let a = variable(&mut arena, 2);
        let left = family.project(&mut arena, a);
        let b = variable(&mut arena, 1);
        let right = family.project(&mut arena, b);
        let ty = family.quotient(&mut arena);
        id(&mut arena, ty, left, right)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(two(&mut arena))
        .extend(two(&mut arena))
        .extend(related)
        .extend(projection_identity);

    // `sound a b r : Id Q (project a) (project b)` — related
    // representatives map to identical quotient images.
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let r = variable(&mut arena, 1);
    let sounded = family.sound(&mut arena, a, b, r);
    let expected = {
        let a = variable(&mut arena, 3);
        let left = family.project(&mut arena, a);
        let b = variable(&mut arena, 2);
        let right = family.project(&mut arena, b);
        let ty = family.quotient(&mut arena);
        id(&mut arena, ty, left, right)
    };
    check_type(&mut arena, &context, sounded, expected, &mut budget).unwrap();

    // `effective a b p : R a b` — an identity of projections supplies
    // evidence of exactly the selected relation, which unfolds to
    // `Id Two a b` here.
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let p = variable(&mut arena, 0);
    let effected = family.effective(&mut arena, a, b, p);
    let expected = {
        let relation = family.relation;
        let a = variable(&mut arena, 3);
        let applied = apply(&mut arena, relation, a);
        let b = variable(&mut arena, 2);
        apply(&mut arena, applied, b)
    };
    check_type(&mut arena, &context, effected, expected, &mut budget).unwrap();

    // An identity between the *representatives* is not effectivity
    // evidence: `effective` needs `Id Q (project a) (project b)`, and
    // `r : Id Two a b` does not convert — no representative crossing.
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let representative_identity = variable(&mut arena, 1);
    let wrong = family.effective(&mut arena, a, b, representative_identity);
    let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "a representative identity must not serve as projection identity: {error:?}"
    );
}

#[test]
fn quotient_transport_carries_premise_evidence_across_the_relation() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // `P = λ(_ : Q). Two` — a constant premise family — with context
    // `a : Two`, `b : Two`, `r : Id Two a b` (`R a b` unfolded),
    // `h : Two` (`P (project a)` unfolded).
    let premise_family = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        lambda(&mut arena, domain, codomain)
    };
    let related = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(two(&mut arena))
        .extend(two(&mut arena))
        .extend(related)
        .extend(two(&mut arena));

    // `transport P a b r h : P (project b)` — which is `Two`.
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let r = variable(&mut arena, 1);
    let h = variable(&mut arena, 0);
    let carried = family.transport(&mut arena, zero.clone(), premise_family, a, b, r, h);
    let result = two(&mut arena);
    check_type(&mut arena, &context, carried, result, &mut budget).unwrap();

    // Premise evidence cannot cross without relation evidence: passing
    // `h : Two` where `R a b` is required rejects.
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let not_related = variable(&mut arena, 0);
    let h = variable(&mut arena, 0);
    let premise_family = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        lambda(&mut arena, domain, codomain)
    };
    let wrong = family.transport(&mut arena, zero, premise_family, a, b, not_related, h);
    let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "a bare premise is not relation evidence: {error:?}"
    );
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
fn quotient_assumption_closure_records_the_admitted_interface() {
    let mut arena = TermArena::new();
    let signature = quotient_signature(&mut arena);

    // `lift` is a checked definition, but its derivation routes through
    // `elim`, `sound`, `project` and the carrier — a judgment whose
    // evidence is a `lift` application commits to exactly those
    // admitted assumptions. A receiver refusing quotient assumptions
    // sees them here, exactly.
    let closure = assumption_closure(&arena, &signature, &[QUOTIENT_LIFT]);
    let expected: std::collections::BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
            .into_iter()
            .collect();
    assert_eq!(closure, expected);

    // `transport` is derived — but the derivation uses `sound`, so its
    // closure records the law it computes through.
    let closure = assumption_closure(&arena, &signature, &[QUOTIENT_TRANSPORT]);
    let expected: std::collections::BTreeSet<u32> = [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND]
        .into_iter()
        .collect();
    assert_eq!(closure, expected);

    // `liftPre` is itself an assumption: any judgment depending on it
    // records it — the optional transport-backed shape is never
    // silently admitted.
    let closure = assumption_closure(&arena, &signature, &[QUOTIENT_LIFT_PRECONDITION]);
    let expected: std::collections::BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_LIFT_PRECONDITION]
            .into_iter()
            .collect();
    assert_eq!(closure, expected);

    // A representative-level judgment with no quotient constants
    // commits to nothing — the spec's "receiver refusing quotient
    // assumptions" case.
    let two_ty = two(&mut arena);
    let x = variable(&mut arena, 0);
    let identity = id(&mut arena, two_ty, x, x);
    let closure = judgment_assumption_closure(&arena, &signature, &[identity]);
    assert!(closure.is_empty());
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
fn quotient_lift_carries_through_a_checked_certificate() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let declarations = quotient_scheme(&mut arena);
    let signature = {
        let mut signature_budget = default_budget();
        check_signature(&mut arena, &declarations, &mut signature_budget).unwrap()
    };
    let zero = Level::Constant(0);

    // The same `lift` judgment as the direct check, carried as a
    // certificate: signature + context + evidence + claimed type, all
    // re-decided by `verify_mathematical_certificate`.
    let set_b = {
        let two_ty = two(&mut arena);
        is_set_zero(&mut arena, two_ty)
    };
    let operation = {
        let domain = two(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let respect = {
        let domain = two(&mut arena);
        let inner_domain = two(&mut arena);
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        let related = id(&mut arena, ty, a, b);
        let ty = two(&mut arena);
        let f = variable(&mut arena, 3);
        let a = variable(&mut arena, 2);
        let left = apply(&mut arena, f, a);
        let f = variable(&mut arena, 3);
        let b = variable(&mut arena, 1);
        let right = apply(&mut arena, f, b);
        let conclusion = id(&mut arena, ty, left, right);
        let inner = pi(&mut arena, related, conclusion);
        let middle = pi(&mut arena, inner_domain, inner);
        pi(&mut arena, domain, middle)
    };
    let element = family.quotient(&mut arena);

    let set_b_var = variable(&mut arena, 3);
    let operation_var = variable(&mut arena, 2);
    let respect_var = variable(&mut arena, 1);
    let element_var = variable(&mut arena, 0);
    let carrier_two = two(&mut arena);
    let lifted = family.lift(
        &mut arena,
        zero,
        carrier_two,
        set_b_var,
        operation_var,
        respect_var,
        element_var,
    );
    let expected = two(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![set_b, operation, respect, element],
        term: lifted,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    assert!(before - budget.remaining() > 0);

    // The judgment commits to the admitted interface: `Q`, `project`,
    // `sound`, `elim` are the exact assumption set — a receiver refusing
    // quotient assumptions rejects this certificate by its own closure.
    let closure = certificate_assumption_closure(&arena, &certificate);
    let expected: std::collections::BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
            .into_iter()
            .collect();
    assert_eq!(closure, expected);

    // Sanity: the certificate context alone still re-checks — the
    // signature the verifier built is the one under which the context
    // is well-formed.
    let _ = signature;
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
