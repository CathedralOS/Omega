use crate::mathematical_core::term::sorts_equal;
use crate::mathematical_core::tests::{
    apply, constant, default_budget, id, indexed_context, lambda, pair, pi, refl, scheme_signature,
    sigma, strict_sort, sup, two, two_indexed_family, two_zero, type_sort, variable, w_type,
};
use crate::mathematical_core::{
    Context, CoreError, Declaration, INDEXED_IND, INDEXED_W, Level, MathematicalCertificate, Sort,
    TermArena, certificate_assumption_closure, check_signature, check_type, convertible,
    indexed_scheme, infer_sort, infer_type, verify_mathematical_certificate,
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
