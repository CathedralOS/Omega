//! The rest of the set-quotient derive-list: `idSym`/`idCancel`/
//! `propIsSet`/`boxProp` are checked lemma definitions, and `indProp`,
//! `coverage`, `unique` are the specification's remaining derived items
//! — proposition-valued induction through boxing, squashed coverage,
//! and pointwise uniqueness of sections. Each is exercised at the
//! `Two`/equality quotient with ordinary typing re-deciding every
//! application.

use crate::mathematical_core::tests::{
    apply, boxed, default_budget, fst, id, is_set_zero, lambda, pi, quotient_signature, refl,
    sigma, squash, squash_elim, squash_intro, two, two_quotient, variable,
};
use crate::mathematical_core::{
    Context, CoreError, Level, MathematicalCertificate, QUOTIENT, QUOTIENT_BOX_PROP,
    QUOTIENT_COVERAGE, QUOTIENT_ELIM, QUOTIENT_ID_CANCEL, QUOTIENT_ID_SYM, QUOTIENT_IND_PROP,
    QUOTIENT_PROJECT, QUOTIENT_PROP_IS_SET, QUOTIENT_SOUND, QUOTIENT_UNIQUE, TermArena,
    assumption_closure, certificate_assumption_closure, check_signature, check_type, convertible,
    infer_type, quotient_scheme, verify_mathematical_certificate,
};

#[test]
fn quotient_identity_lemmas_check_at_two() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `a : Two`, `b : Two`, `p : Id Two a b` — the plain
    // identity-lemma inputs. Ambient indices: p = 0, b = 1, a = 2;
    // inside `p`'s entry the prefix is `[a, b]`, so a = 1, b = 0.
    let identity_ab = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(two(&mut arena))
        .extend(two(&mut arena))
        .extend(identity_ab);

    // `idSym Two a b p : Id Two b a` — symmetry is a checked constant.
    let ty = two(&mut arena);
    let a = variable(&mut arena, 2);
    let b = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);
    let symmetrized = family.id_sym(&mut arena, zero.clone(), ty, a, b, p);
    let expected = {
        let ty = two(&mut arena);
        let b = variable(&mut arena, 1);
        let a = variable(&mut arena, 2);
        id(&mut arena, ty, b, a)
    };
    check_type(&mut arena, &context, symmetrized, expected, &mut budget).unwrap();

    // `idCancel Two a b p : Id (Id Two b b) (trans (sym p) p) (refl b)`
    // — the inverse law, spelled through the same spines the derived
    // declaration uses.
    let sym_p = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 2);
        let b = variable(&mut arena, 1);
        let p = variable(&mut arena, 0);
        family.id_sym(&mut arena, zero.clone(), ty, a, b, p)
    };
    let trans_sym_p = {
        let ty = two(&mut arena);
        let b = variable(&mut arena, 1);
        let a = variable(&mut arena, 2);
        let b2 = variable(&mut arena, 1);
        let p = variable(&mut arena, 0);
        family.id_trans(&mut arena, zero.clone(), ty, b, a, b2, sym_p, p)
    };
    let ty = two(&mut arena);
    let a = variable(&mut arena, 2);
    let b = variable(&mut arena, 1);
    let p = variable(&mut arena, 0);
    let cancelled = family.id_cancel(&mut arena, zero.clone(), ty, a, b, p);
    let expected = {
        let id_bb = {
            let ty = two(&mut arena);
            let b = variable(&mut arena, 1);
            id(&mut arena, ty, b, b)
        };
        let ty = two(&mut arena);
        let b = variable(&mut arena, 1);
        let refl_b = refl(&mut arena, ty, b);
        id(&mut arena, id_bb, trans_sym_p, refl_b)
    };
    check_type(&mut arena, &context, cancelled, expected, &mut budget).unwrap();
}

#[test]
fn quotient_prop_is_set_and_box_prop_close_propositions() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `f : Π(x y : Two). Id Two x y`, `a : Two`, `b : Two`,
    // `p : Id Two a b`, `q : Id Two a b` — a mere-proposition witness
    // and two parallel proofs. Ambient indices: q = 0, p = 1, b = 2,
    // a = 3, f = 4; `p`'s entry sees `[f, a, b]` (a = 1, b = 0) and
    // `q`'s entry sees `[f, a, b, p]` (a = 2, b = 1).
    let is_prop_two = {
        let x_domain = two(&mut arena);
        let codomain = {
            let y_domain = two(&mut arena);
            let ty = two(&mut arena);
            let x = variable(&mut arena, 1);
            let y = variable(&mut arena, 0);
            let body = id(&mut arena, ty, x, y);
            pi(&mut arena, y_domain, body)
        };
        pi(&mut arena, x_domain, codomain)
    };
    let identity_ab = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let identity_ab2 = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 2);
        let b = variable(&mut arena, 1);
        id(&mut arena, ty, a, b)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(is_prop_two)
        .extend(two(&mut arena))
        .extend(two(&mut arena))
        .extend(identity_ab)
        .extend(identity_ab2);

    // `propIsSet Two f a b p q : Id (Id Two a b) p q` — propositional
    // proof irrelevance, derived through `J`.
    let ty = two(&mut arena);
    let f = variable(&mut arena, 4);
    let a = variable(&mut arena, 3);
    let b = variable(&mut arena, 2);
    let p = variable(&mut arena, 1);
    let q = variable(&mut arena, 0);
    let set_witness = family.prop_is_set(&mut arena, zero.clone(), ty, f);
    let set_witness = apply(&mut arena, set_witness, a);
    let set_witness = apply(&mut arena, set_witness, b);
    let set_witness = apply(&mut arena, set_witness, p);
    let set_witness = apply(&mut arena, set_witness, q);
    let expected = {
        let id_ab = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 3);
            let b = variable(&mut arena, 2);
            id(&mut arena, ty, a, b)
        };
        let p = variable(&mut arena, 1);
        let q = variable(&mut arena, 0);
        id(&mut arena, id_ab, p, q)
    };
    check_type(&mut arena, &context, set_witness, expected, &mut budget).unwrap();
}

#[test]
fn quotient_box_prop_makes_boxed_strict_types_propositional() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `x : Box (Squash Two)`, `y : Box (Squash Two)` — indices
    // y = 0, x = 1.
    let box_squash_two = {
        let carrier = two(&mut arena);
        let inner = squash(&mut arena, carrier);
        boxed(&mut arena, inner)
    };
    let box_squash_two2 = {
        let carrier = two(&mut arena);
        let inner = squash(&mut arena, carrier);
        boxed(&mut arena, inner)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(box_squash_two)
        .extend(box_squash_two2);

    // `boxProp (Squash Two) x y : Id (Box (Squash Two)) x y` — any two
    // boxed proofs of a strict proposition are propositionally equal.
    let carrier = two(&mut arena);
    let strict_ty = squash(&mut arena, carrier);
    let x = variable(&mut arena, 1);
    let y = variable(&mut arena, 0);
    let prop = family.box_prop(&mut arena, zero.clone(), strict_ty, x, y);
    let expected = {
        let ty = {
            let carrier = two(&mut arena);
            let inner = squash(&mut arena, carrier);
            boxed(&mut arena, inner)
        };
        let x = variable(&mut arena, 1);
        let y = variable(&mut arena, 0);
        id(&mut arena, ty, x, y)
    };
    check_type(&mut arena, &context, prop, expected, &mut budget).unwrap();

    // But the equality is propositional, not definitional: `x` and `y`
    // remain distinct neutrals — the reference core grants no
    // definitional irrelevance on relevant boxed data.
    let x = variable(&mut arena, 1);
    let y = variable(&mut arena, 0);
    let shared = {
        let carrier = two(&mut arena);
        let inner = squash(&mut arena, carrier);
        boxed(&mut arena, inner)
    };
    assert!(
        !convertible(&mut arena, &context, x, y, shared, &mut budget).unwrap(),
        "boxed inhabitants are propositionally, not definitionally, equal"
    );
}

#[test]
fn quotient_ind_prop_reaches_strict_targets_through_boxing() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // `P = λ(_ : Q). Squash Two` — a constant strict motive — with
    // `d = λ(a : Two). sq a` and `z : Q`. `indProp Two R P d z :
    // Squash Two`: the strict target is reached through the boxed
    // eliminator, the route the specification prescribes.
    let strict_motive = {
        let domain = family.quotient(&mut arena);
        let inner = two(&mut arena);
        let body = squash(&mut arena, inner);
        lambda(&mut arena, domain, body)
    };
    let case = {
        let domain = two(&mut arena);
        let bound = variable(&mut arena, 0);
        let inner = two(&mut arena);
        let body = squash_intro(&mut arena, inner, bound);
        lambda(&mut arena, domain, body)
    };
    let element = family.quotient(&mut arena);
    let context = Context::empty().with_signature(signature).extend(element);

    let z = variable(&mut arena, 0);
    let induced = family.ind_prop(&mut arena, zero.clone(), strict_motive, case, z);
    let inner = two(&mut arena);
    let expected = squash(&mut arena, inner);
    check_type(&mut arena, &context, induced, expected, &mut budget).unwrap();

    // A relevant motive is not proposition-valued: `λ(_ : Q). Two` lands
    // `Type`, and `indProp` rejects it — strict motives only.
    let relevant_motive = {
        let domain = family.quotient(&mut arena);
        let body = two(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let case = {
        let domain = two(&mut arena);
        let body = variable(&mut arena, 0);
        lambda(&mut arena, domain, body)
    };
    let z = variable(&mut arena, 0);
    let wrong = family.ind_prop(&mut arena, zero, relevant_motive, case, z);
    let error = infer_type(&mut arena, &context, wrong, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "a Type-valued motive is not proposition-valued: {error:?}"
    );
}

#[test]
fn quotient_coverage_is_squashed_existence_without_extraction() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);

    // `coverage z : Squash (Σ(a : Two). Id Q (project a) z)` — under
    // the `a` binder the ambient `z` shifts to index 1.
    let covered_ty = {
        let a_domain = two(&mut arena);
        let codomain = {
            let a = variable(&mut arena, 0);
            let projected = family.project(&mut arena, a);
            let z = variable(&mut arena, 1);
            let ty = family.quotient(&mut arena);
            id(&mut arena, ty, projected, z)
        };
        sigma(&mut arena, a_domain, codomain)
    };
    let element = family.quotient(&mut arena);
    let context = Context::empty().with_signature(signature).extend(element);

    let z = variable(&mut arena, 0);
    let covered = family.coverage(&mut arena, z);
    let expected = squash(&mut arena, covered_ty);
    check_type(&mut arena, &context, covered, expected, &mut budget).unwrap();

    // The squash eliminates into a strict target — the witness is
    // usable propositionally: `unsq (Squash Two) (λw. sq (fst w))
    // (coverage z) : Squash Two`.
    let unsquashed = {
        let carrier = two(&mut arena);
        let proposition = squash(&mut arena, carrier);
        let function = {
            let witness_domain = covered_ty;
            let w = variable(&mut arena, 0);
            let first = fst(&mut arena, w);
            let inner = two(&mut arena);
            let body = squash_intro(&mut arena, inner, first);
            lambda(&mut arena, witness_domain, body)
        };
        let z = variable(&mut arena, 0);
        let covered = family.coverage(&mut arena, z);
        squash_elim(&mut arena, proposition, function, covered)
    };
    let carrier = two(&mut arena);
    let expected = squash(&mut arena, carrier);
    check_type(&mut arena, &context, unsquashed, expected, &mut budget).unwrap();

    // But no representative is extractable: `fst (coverage z)` treats
    // the squash as a pair and rejects — coverage supplies no
    // representative-extraction function.
    let z = variable(&mut arena, 0);
    let covered = family.coverage(&mut arena, z);
    let extract = fst(&mut arena, covered);
    let error = infer_type(&mut arena, &context, extract, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::NotAPair { .. }),
        "squashed existence never opens to a representative: {error:?}"
    );
}

#[test]
fn quotient_unique_collapses_sections_agreeing_on_projections() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);
    let zero = Level::Constant(0);

    // Context `setB : isSet Two`, `s1 : Π(z : Q). Two`,
    // `s2 : Π(z : Q). Two`, `h : Π(a : Two). Id Two (s1 (project a)) (s2
    // (project a))`, `z : Q` — ambient indices z = 0, h = 1, s2 = 2,
    // s1 = 3, setB = 4; `h`'s entry sees `[setB, s1, s2]`, so under its
    // `a` binder s1 = 2, s2 = 1.
    let two_ty = two(&mut arena);
    let set_b = is_set_zero(&mut arena, two_ty);
    let section = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let section2 = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        pi(&mut arena, domain, codomain)
    };
    let agreement = {
        let a_domain = two(&mut arena);
        let codomain = {
            // Inside `h`'s entry the prefix is `[setB, s1, s2]`; under
            // `a`, `s1` is index 2 and `s2` is index 1.
            let a = variable(&mut arena, 0);
            let projected = family.project(&mut arena, a);
            let s1 = variable(&mut arena, 2);
            let left = apply(&mut arena, s1, projected);
            let a = variable(&mut arena, 0);
            let projected = family.project(&mut arena, a);
            let s2 = variable(&mut arena, 1);
            let right = apply(&mut arena, s2, projected);
            let ty = two(&mut arena);
            id(&mut arena, ty, left, right)
        };
        pi(&mut arena, a_domain, codomain)
    };
    let element = family.quotient(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(set_b)
        .extend(section)
        .extend(section2)
        .extend(agreement)
        .extend(element);

    // `P = λ(_ : Q). Two`, `sets = λ(_ : Q). setB` — the constant
    // family and its setness evidence.
    let constant_family = {
        let domain = family.quotient(&mut arena);
        let codomain = two(&mut arena);
        lambda(&mut arena, domain, codomain)
    };
    let sets = {
        let domain = family.quotient(&mut arena);
        let set_b = variable(&mut arena, 5);
        lambda(&mut arena, domain, set_b)
    };

    // `unique P sets s1 s2 h z : Id Two (s1 z) (s2 z)` — pointwise
    // agreement on projections collapses to pointwise equality of the
    // sections.
    let s1 = variable(&mut arena, 3);
    let s2 = variable(&mut arena, 2);
    let h = variable(&mut arena, 1);
    let z = variable(&mut arena, 0);
    let unique = family.unique(
        &mut arena,
        zero.clone(),
        constant_family,
        sets,
        s1,
        s2,
        h,
        z,
    );
    let expected = {
        let ty = two(&mut arena);
        let s1 = variable(&mut arena, 3);
        let z = variable(&mut arena, 0);
        let left = apply(&mut arena, s1, z);
        let s2 = variable(&mut arena, 2);
        let z = variable(&mut arena, 0);
        let right = apply(&mut arena, s2, z);
        id(&mut arena, ty, left, right)
    };
    check_type(&mut arena, &context, unique, expected, &mut budget).unwrap();

    // Pointwise, not functional: the same derivation is not evidence
    // of `Id (Π(z : Q). Two) s1 s2` — function equality needs the
    // extensionality this calculus deliberately lacks.
    let s1 = variable(&mut arena, 3);
    let s2 = variable(&mut arena, 2);
    let h = variable(&mut arena, 1);
    let z = variable(&mut arena, 0);
    let unique = family.unique(&mut arena, zero, constant_family, sets, s1, s2, h, z);
    let function_identity = {
        let ty = section;
        let s1 = variable(&mut arena, 3);
        let s2 = variable(&mut arena, 2);
        id(&mut arena, ty, s1, s2)
    };
    let error =
        check_type(&mut arena, &context, unique, function_identity, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::TypeMismatch { .. }),
        "pointwise uniqueness is not function extensionality: {error:?}"
    );
}

#[test]
fn quotient_derived_items_close_over_exactly_the_checked_interface() {
    let mut arena = TermArena::new();
    let signature = quotient_signature(&mut arena);

    // `indProp`, `coverage` and `unique` are derived through `elim`,
    // `transport` (which routes through `sound`), `project` and the
    // carrier `Q` — a judgment citing them commits to exactly that
    // admitted fragment: `setQ`, `effective`, `beta` and `liftPre` are
    // not in scope.
    let expected: std::collections::BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
            .into_iter()
            .collect();
    for declaration in [QUOTIENT_IND_PROP, QUOTIENT_COVERAGE, QUOTIENT_UNIQUE] {
        let closure = assumption_closure(&arena, &signature, &[declaration]);
        assert_eq!(
            closure, expected,
            "declaration {declaration} must close over exactly the checked interface"
        );
    }

    // The identity/boxing lemmas are pure derivations — no admitted
    // assumption enters their closure at all.
    for declaration in [
        QUOTIENT_ID_SYM,
        QUOTIENT_ID_CANCEL,
        QUOTIENT_PROP_IS_SET,
        QUOTIENT_BOX_PROP,
    ] {
        let closure = assumption_closure(&arena, &signature, &[declaration]);
        assert!(
            closure.is_empty(),
            "declaration {declaration} is a closed derivation"
        );
    }
}

#[test]
fn quotient_coverage_carries_through_a_checked_certificate() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let declarations = quotient_scheme(&mut arena);
    check_signature(&mut arena, &declarations, &mut default_budget()).unwrap();

    // The `coverage z : Squash (Σ(a : Two). Id Q (project a) z)`
    // judgment as a certificate: signature + context + evidence +
    // claimed type, re-decided by `verify_mathematical_certificate`.
    let element = family.quotient(&mut arena);
    let z = variable(&mut arena, 0);
    let covered = family.coverage(&mut arena, z);
    let expected = {
        let a_domain = two(&mut arena);
        let codomain = {
            let a = variable(&mut arena, 0);
            let projected = family.project(&mut arena, a);
            let z = variable(&mut arena, 1);
            let ty = family.quotient(&mut arena);
            id(&mut arena, ty, projected, z)
        };
        let packed = sigma(&mut arena, a_domain, codomain);
        squash(&mut arena, packed)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element],
        term: covered,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    assert!(before - budget.remaining() > 0);

    // The judgment commits to exactly the checked interface — the same
    // `Q`, `project`, `sound`, `elim` fragment the other derived items
    // carry; the strict-layer machinery inside `boxProp` adds nothing.
    let closure = certificate_assumption_closure(&arena, &certificate);
    let expected: std::collections::BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
            .into_iter()
            .collect();
    assert_eq!(closure, expected);
}
