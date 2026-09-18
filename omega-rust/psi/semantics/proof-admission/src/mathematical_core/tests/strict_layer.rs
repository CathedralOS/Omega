//! The reference core's strict layer: `sEmpty` with ex falso into either
//! sort, `Squash` as existence without witness extraction, and `Box` as
//! the converse embedding that keeps irrelevance local. The kernel owns
//! no strict unit former — `sUnit := Π(_ : sEmpty). sEmpty` is derived —
//! and no eta law for squash or box.

use super::{
    apply, box_elim, box_intro, boxed, constant, default_budget, empty, empty_elim, lambda, pair,
    pi, sigma, sort_level, squash, squash_elim, squash_intro, strict_sort, two, two_zero,
    type_sort, variable,
};
use crate::mathematical_core::{
    Budget, Context, CoreError, Declaration, Level, Signature, TermArena, assumption_closure,
    check_signature, check_type, convertible, infer_sort, infer_type, weak_head_normalize,
};

#[test]
fn strict_empty_forms_at_level_zero_and_eliminates_into_either_sort() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `sEmpty : Strict 0` — one fixed level, like `Two : Type 0`: it
    // carries no data, so a single level suffices for ex falso into
    // every universe.
    let strict_empty = empty(&mut arena);
    let inferred = infer_type(&mut arena, &Context::empty(), strict_empty, &mut budget).unwrap();
    let expected = strict_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // Context P : Strict 0, A : Type 0, e : sEmpty — indices e = 0,
    // A = 1, P = 2.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let empty_binding = empty(&mut arena);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(empty_binding);

    // `sEmpty_rect A e : A` — ex falso reaches a relevant type.
    let target = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 0);
    let elimination = empty_elim(&mut arena, target, scrutinee);
    let inferred = infer_type(&mut arena, &context, elimination, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, target));
    check_type(&mut arena, &context, elimination, target, &mut budget).unwrap();

    // `sEmpty_rect P e : P` — and a strict proposition alike, since no
    // closed `e` exists for the result to depend on. The eliminator
    // never computes: `sEmpty` has no constructors, so a well-typed
    // scrutinee is always neutral and the elimination stays stuck.
    let strict_target = variable(&mut arena, 2);
    let scrutinee = variable(&mut arena, 0);
    let elimination = empty_elim(&mut arena, strict_target, scrutinee);
    check_type(
        &mut arena,
        &context,
        elimination,
        strict_target,
        &mut budget,
    )
    .unwrap();
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
    assert!(arena.structurally_equal(normalized, elimination));
}

#[test]
fn empty_elimination_rejects_non_type_targets_and_foreign_scrutinees() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, e : sEmpty, x : A — x = 0,
    // e = 1, A = 2, P = 3.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let empty_binding = empty(&mut arena);
    let x_binding = variable(&mut arena, 1);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(empty_binding)
        .extend(x_binding);

    // A target that is not a type at all rejects: `e` is a proof, not
    // a sort.
    let scrutinee = variable(&mut arena, 1);
    let bad = empty_elim(&mut arena, scrutinee, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));

    // A scrutinee of a relevant type is not an `sEmpty` proof: `x : A`
    // cannot fuel ex falso.
    let target = variable(&mut arena, 2);
    let wrong = variable(&mut arena, 0);
    let bad = empty_elim(&mut arena, target, wrong);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The dual control: `e : sEmpty` *does* fuel `Two` — elimination
    // into relevant types is exactly what the rule licenses.
    let two_target = two(&mut arena);
    let scrutinee = variable(&mut arena, 1);
    let elimination = empty_elim(&mut arena, two_target, scrutinee);
    let inferred = infer_type(&mut arena, &context, elimination, &mut budget).unwrap();
    let expected = two(&mut arena);
    assert!(arena.structurally_equal(inferred, expected));

    // But it cannot land at a different type than its own target:
    // `sEmpty_rect A e` is not a `P`.
    let target = variable(&mut arena, 2);
    let scrutinee = variable(&mut arena, 1);
    let elimination = empty_elim(&mut arena, target, scrutinee);
    let wrong_claim = variable(&mut arena, 3);
    let error =
        check_type(&mut arena, &context, elimination, wrong_claim, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn stuck_empty_eliminations_collapse_under_scrutinee_irrelevance() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, B : Type 0, e : sEmpty, f : sEmpty —
    // f = 0, e = 1, B = 2, A = 3.
    let type_zero = type_sort(&mut arena, 0);
    let other_type = type_sort(&mut arena, 0);
    let e_binding = empty(&mut arena);
    let f_binding = empty(&mut arena);
    let context = Context::empty()
        .extend(type_zero)
        .extend(other_type)
        .extend(e_binding)
        .extend(f_binding);

    // `sEmpty_rect A e` and `sEmpty_rect A f` at `A`: the scrutinee
    // comparison runs at `sEmpty`, where strict collapse equates the
    // distinct neutrals — two ex falso proofs are interchangeable.
    let target = variable(&mut arena, 3);
    let e = variable(&mut arena, 1);
    let left = empty_elim(&mut arena, target, e);
    let target = variable(&mut arena, 3);
    let f = variable(&mut arena, 0);
    let right = empty_elim(&mut arena, target, f);
    let shared = variable(&mut arena, 3);
    assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

    // The *targets* still decide: `sEmpty_rect A e` never converts to
    // `sEmpty_rect B e` at `A`.
    let other_target = variable(&mut arena, 2);
    let e = variable(&mut arena, 1);
    let other = empty_elim(&mut arena, other_target, e);
    let shared = variable(&mut arena, 3);
    assert!(!convertible(&mut arena, &context, left, other, shared, &mut budget).unwrap());
}

#[test]
fn squash_forms_at_the_carrier_level_and_introduces_checked_witnesses() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, a : A, B : Type 1 — B = 0,
    // a = 1, A = 2, P = 3.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let type_one = type_sort(&mut arena, 1);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(a_binding)
        .extend(type_one);

    // `Squash A : Strict 0` and `Squash B : Strict 1` — the proposition
    // follows the carrier's level.
    let carrier = variable(&mut arena, 2);
    let squashed = squash(&mut arena, carrier);
    let inferred = infer_type(&mut arena, &context, squashed, &mut budget).unwrap();
    let expected = strict_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    let carrier = variable(&mut arena, 0);
    let squashed = squash(&mut arena, carrier);
    let inferred = infer_type(&mut arena, &context, squashed, &mut budget).unwrap();
    let expected = strict_sort(&mut arena, 1);
    assert!(arena.structurally_equal(inferred, expected));

    // `sq_A a : Squash A` — the annotation re-runs formation and the
    // value checks at `A`.
    let carrier = variable(&mut arena, 2);
    let value = variable(&mut arena, 1);
    let witness = squash_intro(&mut arena, carrier, value);
    let carrier = variable(&mut arena, 2);
    let expected = squash(&mut arena, carrier);
    check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();
    let inferred = infer_type(&mut arena, &context, witness, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));

    // A dependent-pair witness still checks componentwise: squashing
    // `Σ(_ : A). Two` admits `(a, zero)`.
    let domain = variable(&mut arena, 2);
    let codomain = two(&mut arena);
    let carrier = sigma(&mut arena, domain, codomain);
    let first = variable(&mut arena, 1);
    let second = two_zero(&mut arena);
    let value = pair(&mut arena, first, second);
    let witness = squash_intro(&mut arena, carrier, value);
    let expected = squash(&mut arena, carrier);
    check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();
}

#[test]
fn squash_rejects_strict_carriers_and_mismatched_annotations() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, a : A — a = 0, A = 1, P = 2.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(a_binding);

    // Squashing a strict proposition rejects: `P` is already a
    // proposition, there is nothing left to forget.
    let carrier = variable(&mut arena, 2);
    let bad = squash(&mut arena, carrier);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictSquashDomain { .. }));

    // The introduction annotation re-runs the same formation rule.
    let carrier = variable(&mut arena, 2);
    let value = variable(&mut arena, 0);
    let bad = squash_intro(&mut arena, carrier, value);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::StrictSquashDomain { .. }));

    // A non-type carrier rejects at formation.
    let carrier = variable(&mut arena, 0);
    let bad = squash(&mut arena, carrier);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));

    // A value at a different type rejects: `a : A` does not squash at
    // `Squash (Type 0)`.
    let carrier = type_sort(&mut arena, 0);
    let value = variable(&mut arena, 0);
    let bad = squash_intro(&mut arena, carrier, value);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // And `sq_A a` is not a `Squash (Type 0)` either — squash types
    // compare componentwise at their payloads.
    let carrier = variable(&mut arena, 1);
    let value = variable(&mut arena, 0);
    let witness = squash_intro(&mut arena, carrier, value);
    let carrier = type_sort(&mut arena, 0);
    let wrong = squash(&mut arena, carrier);
    let error = check_type(&mut arena, &context, witness, wrong, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn squashed_proofs_are_irrelevant_but_never_extract_to_relevant_data() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A, b : A, s : Squash A, t : Squash A,
    // P : Strict 0, f : Π(_ : A). P, g : Π(_ : A). P — g = 0, f = 1,
    // P = 2, t = 3, s = 4, b = 5, a = 6, A = 7.
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let b_binding = variable(&mut arena, 1);
    let s_binding = {
        let carrier = variable(&mut arena, 2);
        squash(&mut arena, carrier)
    };
    let t_binding = {
        let carrier = variable(&mut arena, 3);
        squash(&mut arena, carrier)
    };
    let strict_zero = strict_sort(&mut arena, 0);
    let f_binding = {
        // Π(_ : A). P under prefix [A, a, b, s, t, P]: A = 5, P = 0;
        // under the binder P = 1.
        let domain = variable(&mut arena, 5);
        let codomain = variable(&mut arena, 1);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        // Under prefix [A, a, b, s, t, P, f]: A = 6, P = 1; under the
        // binder P = 2.
        let domain = variable(&mut arena, 6);
        let codomain = variable(&mut arena, 2);
        pi(&mut arena, domain, codomain)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(a_binding)
        .extend(b_binding)
        .extend(s_binding)
        .extend(t_binding)
        .extend(strict_zero)
        .extend(f_binding)
        .extend(g_binding);

    // `sq_A a ≡ sq_A b : Squash A`, `s ≡ t` and even `s ≡ sq_A a`:
    // at a strict shared type every two proofs collapse — including a
    // neutral against a canonical form. This is the definitional proof
    // irrelevance the foundation names, now with real squash proofs.
    let carrier = variable(&mut arena, 7);
    let shared = squash(&mut arena, carrier);
    let carrier = variable(&mut arena, 7);
    let value = variable(&mut arena, 6);
    let left = squash_intro(&mut arena, carrier, value);
    let carrier = variable(&mut arena, 7);
    let value = variable(&mut arena, 5);
    let right = squash_intro(&mut arena, carrier, value);
    assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());
    let neutral_left = variable(&mut arena, 4);
    let neutral_right = variable(&mut arena, 3);
    assert!(
        convertible(
            &mut arena,
            &context,
            neutral_left,
            neutral_right,
            shared,
            &mut budget,
        )
        .unwrap()
    );
    assert!(
        convertible(
            &mut arena,
            &context,
            neutral_left,
            right,
            shared,
            &mut budget,
        )
        .unwrap()
    );

    // The collapse is gated on the *shared type's* sort: at the
    // relevant `A`, the distinct neutrals `a` and `b` stay apart.
    let a = variable(&mut arena, 6);
    let b = variable(&mut arena, 5);
    let relevant_shared = variable(&mut arena, 7);
    assert!(!convertible(&mut arena, &context, a, b, relevant_shared, &mut budget).unwrap());

    // `unsq P f s : P` — squashed existence proves strict propositions.
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 4);
    let elimination = squash_elim(&mut arena, proposition, function, scrutinee);
    let expected = variable(&mut arena, 2);
    check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

    // Two different eliminations of the same squashed proof convert:
    // `unsq P f s ≡ unsq P g s : P` — both are proofs of a strict
    // proposition, and the whole comparison collapses.
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 4);
    let left = squash_elim(&mut arena, proposition, function, scrutinee);
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 0);
    let scrutinee = variable(&mut arena, 4);
    let right = squash_elim(&mut arena, proposition, function, scrutinee);
    let shared = variable(&mut arena, 2);
    assert!(convertible(&mut arena, &context, left, right, shared, &mut budget).unwrap());

    // But squashed existence never extracts a witness into relevant
    // data: `unsq` targeting `Two : Type 0` rejects — a strict
    // proposition target is required, so no `Squash A → A` projection
    // exists.
    let proposition = two(&mut arena);
    let function = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 4);
    let bad = squash_elim(&mut arena, proposition, function, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::SquashTargetNotStrict { .. }));

    // A scrutinee that is not squashed at all rejects as well: `a : A`
    // supplies no `Squash` carrier to eliminate over.
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 6);
    let bad = squash_elim(&mut arena, proposition, function, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASquash { .. }));

    // And a function at the wrong type — `a : A` is no `Π(_ : A). P` —
    // leaves the elimination without a way to spend the witness.
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 6);
    let scrutinee = variable(&mut arena, 4);
    let bad = squash_elim(&mut arena, proposition, function, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn squash_elimination_computes_on_canonical_witnesses() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, a : A, P : Strict 0, f : Π(_ : A). P —
    // f = 0, P = 1, a = 2, A = 3.
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let f_binding = {
        // Π(_ : A). P under prefix [A, a, P]: A = 2, P = 0; under the
        // binder P = 1.
        let domain = variable(&mut arena, 2);
        let codomain = variable(&mut arena, 1);
        pi(&mut arena, domain, codomain)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(a_binding)
        .extend(strict_zero)
        .extend(f_binding);

    // `unsq P f (sq_A a)` is well-typed at `P` and computes `f a` in
    // one budgeted step: a squashed witness hands the unwrapped value
    // to the function.
    let carrier = variable(&mut arena, 3);
    let value = variable(&mut arena, 2);
    let witness = squash_intro(&mut arena, carrier, value);
    let proposition = variable(&mut arena, 1);
    let function = variable(&mut arena, 0);
    let elimination = squash_elim(&mut arena, proposition, function, witness);
    let expected = variable(&mut arena, 1);
    check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
    let function = variable(&mut arena, 0);
    let value = variable(&mut arena, 2);
    let expected = apply(&mut arena, function, value);
    assert!(arena.structurally_equal(normalized, expected));

    // The step is budgeted: an empty budget refuses instead of
    // deciding.
    let mut empty_budget = Budget::new(0);
    let error = weak_head_normalize(
        &mut arena,
        &Signature::new(),
        elimination,
        &mut empty_budget,
    )
    .unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);

    // A neutral scrutinee keeps the elimination stuck — there is no
    // squash eta manufacturing a canonical witness. Extend the context
    // with `s : Squash A` — s = 0, f = 1, P = 2 — and the stuck
    // elimination is still a well-typed proof of `P`.
    let carrier = variable(&mut arena, 3);
    let s_binding = squash(&mut arena, carrier);
    let context = context.extend(s_binding);
    let proposition = variable(&mut arena, 2);
    let function = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 0);
    let stuck = squash_elim(&mut arena, proposition, function, scrutinee);
    let expected = variable(&mut arena, 2);
    check_type(&mut arena, &context, stuck, expected, &mut budget).unwrap();
    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), stuck, &mut budget).unwrap();
    assert!(arena.structurally_equal(normalized, stuck));
}

#[test]
fn the_dependent_squash_eliminator_is_derived_through_irrelevance() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context A : Type 0, C : Π(_ : Squash A). Strict 0,
    // g : Π(x : A). C (sq_A x), s : Squash A — s = 0, g = 1, C = 2,
    // A = 3.
    let type_zero = type_sort(&mut arena, 0);
    let c_binding = {
        // Π(_ : Squash A). Strict 0 under prefix [A]: A = 0.
        let carrier = variable(&mut arena, 0);
        let domain = squash(&mut arena, carrier);
        let codomain = strict_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        // Π(x : A). C (sq_A x) under prefix [A, C]: A = 1, C = 0; under
        // the x binder C = 1, A = 2, x = 0.
        let domain = variable(&mut arena, 1);
        let codomain = {
            let carrier = variable(&mut arena, 2);
            let bound = variable(&mut arena, 0);
            let witness = squash_intro(&mut arena, carrier, bound);
            let family = variable(&mut arena, 1);
            apply(&mut arena, family, witness)
        };
        pi(&mut arena, domain, codomain)
    };
    let s_binding = {
        let carrier = variable(&mut arena, 2);
        squash(&mut arena, carrier)
    };
    let context = Context::empty()
        .extend(type_zero)
        .extend(c_binding)
        .extend(g_binding)
        .extend(s_binding);

    // The paper's eliminator is non-dependent, so `unsqDep C g s : C s`
    // is built as `(unsq D h s) s` where `D := Π(t : Squash A). C t` is
    // a strict proposition (its codomain is strict) and
    // `h := λ(x : A). λ(t : Squash A). g x`. The body `g x : C (sq_A x)`
    // checks at `C t` because `t ≡ sq_A x` holds definitionally at the
    // strict `Squash A` — irrelevance is what makes the dependent form
    // derivable.
    let big_d = {
        // Π(t : Squash A). C t at ambient [A, C, g, s]: A = 3, C = 2;
        // under the t binder C = 3, t = 0.
        let carrier = variable(&mut arena, 3);
        let domain = squash(&mut arena, carrier);
        let family = variable(&mut arena, 3);
        let bound = variable(&mut arena, 0);
        let codomain = apply(&mut arena, family, bound);
        pi(&mut arena, domain, codomain)
    };
    let h = {
        // λ(x : A). λ(t : Squash A). g x — A = 3 ambient, Squash A at
        // index 4 under x; under both binders g = 3, x = 1.
        let inner = {
            let carrier = variable(&mut arena, 4);
            let domain = squash(&mut arena, carrier);
            let function = variable(&mut arena, 3);
            let bound = variable(&mut arena, 1);
            let body = apply(&mut arena, function, bound);
            lambda(&mut arena, domain, body)
        };
        let domain = variable(&mut arena, 3);
        lambda(&mut arena, domain, inner)
    };
    let scrutinee = variable(&mut arena, 0);
    let elimination = squash_elim(&mut arena, big_d, h, scrutinee);
    let argument = variable(&mut arena, 0);
    let dependent = apply(&mut arena, elimination, argument);

    // `unsqDep s : C s` — the dependent landing type the eliminator was
    // derived for.
    let family = variable(&mut arena, 2);
    let bound = variable(&mut arena, 0);
    let expected = apply(&mut arena, family, bound);
    let inferred = infer_type(&mut arena, &context, dependent, &mut budget).unwrap();
    assert!(arena.structurally_equal(inferred, expected));
    check_type(&mut arena, &context, dependent, expected, &mut budget).unwrap();
}

#[test]
fn boxing_wraps_a_strict_proposition_as_relevant_data() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, p : P, q : P, A : Type 0, b : Box P,
    // b' : Box P — b' = 0, b = 1, A = 2, q = 3, p = 4, P = 5.
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let q_binding = variable(&mut arena, 1);
    let type_zero = type_sort(&mut arena, 0);
    let b_binding = {
        let payload = variable(&mut arena, 3);
        boxed(&mut arena, payload)
    };
    let other_binding = {
        let payload = variable(&mut arena, 4);
        boxed(&mut arena, payload)
    };
    let context = Context::empty()
        .extend(strict_zero)
        .extend(p_binding)
        .extend(q_binding)
        .extend(type_zero)
        .extend(b_binding)
        .extend(other_binding);

    // `Box P : Type 0` — the converse embedding: a strict proposition
    // wrapped as relevant data at the same level.
    let payload = variable(&mut arena, 5);
    let boxed_p = boxed(&mut arena, payload);
    let inferred = infer_type(&mut arena, &context, boxed_p, &mut budget).unwrap();
    let expected = type_sort(&mut arena, 0);
    assert!(arena.structurally_equal(inferred, expected));

    // `box_P p : Box P` — the annotation re-runs formation and the
    // value checks at `P`.
    let payload = variable(&mut arena, 5);
    let value = variable(&mut arena, 4);
    let witness = box_intro(&mut arena, payload, value);
    let payload = variable(&mut arena, 5);
    let expected = boxed(&mut arena, payload);
    check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();

    // `box_P p ≡ box_P q : Box P` — canonical boxes are always equal:
    // the payload comparison runs at `P`, where strictness collapses
    // any two proofs.
    let payload = variable(&mut arena, 5);
    let value = variable(&mut arena, 4);
    let left = box_intro(&mut arena, payload, value);
    let payload = variable(&mut arena, 5);
    let value = variable(&mut arena, 3);
    let right = box_intro(&mut arena, payload, value);
    assert!(convertible(&mut arena, &context, left, right, expected, &mut budget).unwrap());

    // But a *neutral* box is not forced to be a `box`: `b ≢ box_P p`
    // and `b ≢ b'` at the relevant `Box P` — irrelevance does not
    // escape the box.
    let neutral = variable(&mut arena, 1);
    let other_neutral = variable(&mut arena, 0);
    assert!(!convertible(&mut arena, &context, neutral, right, expected, &mut budget).unwrap());
    assert!(
        !convertible(
            &mut arena,
            &context,
            neutral,
            other_neutral,
            expected,
            &mut budget,
        )
        .unwrap()
    );
    assert!(
        convertible(
            &mut arena,
            &context,
            neutral,
            neutral,
            expected,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn box_formation_and_introduction_reject_relevant_domains() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, A : Type 0, a : A — a = 0, A = 1, P = 2.
    let strict_zero = strict_sort(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let context = Context::empty()
        .extend(strict_zero)
        .extend(type_zero)
        .extend(a_binding);

    // `Box A` for a relevant `A` rejects: boxing wraps a *strict*
    // proposition as data; a relevant `A` was never squashed.
    let payload = variable(&mut arena, 1);
    let bad = boxed(&mut arena, payload);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NonStrictBoxDomain { .. }));

    // The introduction annotation re-runs the same formation rule.
    let payload = variable(&mut arena, 1);
    let value = variable(&mut arena, 0);
    let bad = box_intro(&mut arena, payload, value);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NonStrictBoxDomain { .. }));

    // A non-type payload rejects at formation.
    let payload = variable(&mut arena, 0);
    let bad = boxed(&mut arena, payload);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotASort { .. }));

    // A value at the wrong strict proposition rejects: `a : A` is no
    // `P`-proof.
    let payload = variable(&mut arena, 2);
    let value = variable(&mut arena, 0);
    let bad = box_intro(&mut arena, payload, value);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn unbox_computes_and_its_motive_may_land_at_either_sort() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, p : P, A : Type 0,
    // C : Π(_ : Box P). Type 0, g : Π(a : P). C (box_P a),
    // b : Box P — b = 0, g = 1, C = 2, A = 3, p = 4, P = 5.
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let type_zero = type_sort(&mut arena, 0);
    let c_binding = {
        // Π(_ : Box P). Type 0 under prefix [P, p, A]: P = 2.
        let payload = variable(&mut arena, 2);
        let domain = boxed(&mut arena, payload);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        // Π(a : P). C (box_P a) under prefix [P, p, A, C]: P = 3,
        // C = 0; under the a binder C = 1, P = 4, a = 0.
        let domain = variable(&mut arena, 3);
        let codomain = {
            let payload = variable(&mut arena, 4);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 1);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let b_binding = {
        let payload = variable(&mut arena, 4);
        boxed(&mut arena, payload)
    };
    let context = Context::empty()
        .extend(strict_zero)
        .extend(p_binding)
        .extend(type_zero)
        .extend(c_binding)
        .extend(g_binding)
        .extend(b_binding);

    // `unbox C g (box_P p)` checks at `C (box_P p)` and computes `g p`
    // in one budgeted step.
    let payload = variable(&mut arena, 5);
    let value = variable(&mut arena, 4);
    let scrutinee = box_intro(&mut arena, payload, value);
    let motive = variable(&mut arena, 2);
    let body = variable(&mut arena, 1);
    let elimination = box_elim(&mut arena, motive, body, scrutinee);
    let motive = variable(&mut arena, 2);
    let payload = variable(&mut arena, 5);
    let value = variable(&mut arena, 4);
    let argument = box_intro(&mut arena, payload, value);
    let expected = apply(&mut arena, motive, argument);
    check_type(&mut arena, &context, elimination, expected, &mut budget).unwrap();

    let normalized =
        weak_head_normalize(&mut arena, &Signature::new(), elimination, &mut budget).unwrap();
    let function = variable(&mut arena, 1);
    let value = variable(&mut arena, 4);
    let expected = apply(&mut arena, function, value);
    assert!(arena.structurally_equal(normalized, expected));

    // The step is budgeted: an empty budget refuses.
    let mut empty_budget = Budget::new(0);
    let error = weak_head_normalize(
        &mut arena,
        &Signature::new(),
        elimination,
        &mut empty_budget,
    )
    .unwrap_err();
    assert_eq!(error, CoreError::StepCeiling);

    // The strict-motive instance `unbox (λ_ : Box P. P) (λa : P. a) b`
    // is the plain projection `Box P → P`: unboxing lands at a strict
    // proposition, so opening a box never manufactures relevant data.
    let projection_motive = {
        // λ(_ : Box P). P — P = 5 ambient, 6 under the binder.
        let payload = variable(&mut arena, 5);
        let domain = boxed(&mut arena, payload);
        let body = variable(&mut arena, 6);
        lambda(&mut arena, domain, body)
    };
    let domain = variable(&mut arena, 5);
    let bound = variable(&mut arena, 0);
    let projection_body = lambda(&mut arena, domain, bound);
    let scrutinee = variable(&mut arena, 0);
    let projection = box_elim(&mut arena, projection_motive, projection_body, scrutinee);
    let expected = variable(&mut arena, 5);
    check_type(&mut arena, &context, projection, expected, &mut budget).unwrap();

    // A scrutinee that is not boxed rejects: `p : P` supplies no `Box`
    // to open.
    let motive = variable(&mut arena, 2);
    let body = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 4);
    let bad = box_elim(&mut arena, motive, body, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::NotABox { .. }));

    // A motive whose codomain is not a universe — `λ(_ : Box P). p`
    // lands at `P` — leaves `P x` without a type to check against.
    // Under the binder `p` sits at index 5.
    let bad_motive = {
        let payload = variable(&mut arena, 5);
        let domain = boxed(&mut arena, payload);
        let body = variable(&mut arena, 5);
        lambda(&mut arena, domain, body)
    };
    let body = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 0);
    let bad = box_elim(&mut arena, bad_motive, body, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::BoxMotiveCodomainNotAUniverse { .. }
    ));

    // A motive over a different domain — `λ(_ : Two). Type 0` — is not
    // a `Box P`-family.
    let domain = two(&mut arena);
    let body = type_sort(&mut arena, 0);
    let wrong_domain = lambda(&mut arena, domain, body);
    let body = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 0);
    let bad = box_elim(&mut arena, wrong_domain, body, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A body that is not a function of the payload — `p : P` is no
    // `Π(a : P). C (box_P a)` — rejects.
    let motive = variable(&mut arena, 2);
    let body = variable(&mut arena, 4);
    let scrutinee = variable(&mut arena, 0);
    let bad = box_elim(&mut arena, motive, body, scrutinee);
    let error = infer_type(&mut arena, &context, bad, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));
}

#[test]
fn stuck_unbox_eliminations_compare_componentwise() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // Context P : Strict 0, C : Π(_ : Box P). Type 0,
    // D : Π(_ : Box P). Type 0, g : Π(a : P). C (box_P a),
    // h : Π(a : P). C (box_P a), gD : Π(a : P). D (box_P a),
    // b : Box P, b' : Box P — b' = 0, b = 1, gD = 2, h = 3, g = 4,
    // D = 5, C = 6, P = 7.
    let strict_zero = strict_sort(&mut arena, 0);
    let c_binding = {
        let payload = variable(&mut arena, 0);
        let domain = boxed(&mut arena, payload);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let d_binding = {
        // Under prefix [P, C]: P = 1.
        let payload = variable(&mut arena, 1);
        let domain = boxed(&mut arena, payload);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        // Under prefix [P, C, D]: P = 2, C = 1; under the a binder
        // C = 2, P = 3, a = 0.
        let domain = variable(&mut arena, 2);
        let codomain = {
            let payload = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 2);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let h_binding = {
        // Under prefix [P, C, D, g]: P = 3, C = 2; under a: C = 3,
        // P = 4.
        let domain = variable(&mut arena, 3);
        let codomain = {
            let payload = variable(&mut arena, 4);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 3);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let g_d_binding = {
        // Under prefix [P, C, D, g, h]: P = 4, D = 2; under a: D = 3,
        // P = 5.
        let domain = variable(&mut arena, 4);
        let codomain = {
            let payload = variable(&mut arena, 5);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 3);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let b_binding = {
        let payload = variable(&mut arena, 5);
        boxed(&mut arena, payload)
    };
    let other_binding = {
        let payload = variable(&mut arena, 6);
        boxed(&mut arena, payload)
    };
    let context = Context::empty()
        .extend(strict_zero)
        .extend(c_binding)
        .extend(d_binding)
        .extend(g_binding)
        .extend(h_binding)
        .extend(g_d_binding)
        .extend(b_binding)
        .extend(other_binding);

    // `unbox C g b` vs itself at `C b` converts.
    let motive = variable(&mut arena, 6);
    let body = variable(&mut arena, 4);
    let scrutinee = variable(&mut arena, 1);
    let left = box_elim(&mut arena, motive, body, scrutinee);
    let motive = variable(&mut arena, 6);
    let body = variable(&mut arena, 4);
    let scrutinee = variable(&mut arena, 1);
    let again = box_elim(&mut arena, motive, body, scrutinee);
    let family = variable(&mut arena, 6);
    let argument = variable(&mut arena, 1);
    let shared = apply(&mut arena, family, argument);
    assert!(convertible(&mut arena, &context, left, again, shared, &mut budget).unwrap());

    // A different neutral body does not: the bodies compare at
    // `Π(a : P). C (box_P a)`, a real relevant function type — `P` is
    // strict but the codomain is not, so `g ≢ h`.
    let motive = variable(&mut arena, 6);
    let body = variable(&mut arena, 3);
    let scrutinee = variable(&mut arena, 1);
    let other_body = box_elim(&mut arena, motive, body, scrutinee);
    assert!(!convertible(&mut arena, &context, left, other_body, shared, &mut budget).unwrap());

    // A different neutral scrutinee does not either: `Box P` is
    // relevant, so `b ≢ b'` — boxed proofs stay distinct outside the
    // payload.
    let motive = variable(&mut arena, 6);
    let body = variable(&mut arena, 4);
    let scrutinee = variable(&mut arena, 0);
    let other_scrutinee = box_elim(&mut arena, motive, body, scrutinee);
    assert!(
        !convertible(
            &mut arena,
            &context,
            left,
            other_scrutinee,
            shared,
            &mut budget,
        )
        .unwrap()
    );

    // And a different motive does not: `unbox D gD b` eliminates over
    // another family entirely.
    let motive = variable(&mut arena, 5);
    let body = variable(&mut arena, 2);
    let scrutinee = variable(&mut arena, 1);
    let other_motive = box_elim(&mut arena, motive, body, scrutinee);
    assert!(
        !convertible(
            &mut arena,
            &context,
            left,
            other_motive,
            shared,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn the_strict_unit_type_is_derived_not_primitive() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `sUnit := Π(_ : sEmpty). sEmpty` — a strict proposition, since
    // the codomain is strict.
    let domain = empty(&mut arena);
    let codomain = empty(&mut arena);
    let strict_unit = pi(&mut arena, domain, codomain);
    let inferred = infer_sort(&mut arena, &Context::empty(), strict_unit, &mut budget).unwrap();
    assert!(matches!(
        inferred,
        crate::mathematical_core::Sort::Strict(_)
    ));

    // `tt := λ(x : sEmpty). x` inhabits it.
    let domain = empty(&mut arena);
    let bound = variable(&mut arena, 0);
    let tt = lambda(&mut arena, domain, bound);
    check_type(&mut arena, &Context::empty(), tt, strict_unit, &mut budget).unwrap();

    // Every inhabitant is equal: `tt` and `λ(x : sEmpty). sEmpty_rect
    // sEmpty x` convert at `sUnit` — strict collapse, not a unit eta
    // law.
    let other = {
        let target = empty(&mut arena);
        let bound = variable(&mut arena, 0);
        let body = empty_elim(&mut arena, target, bound);
        let domain = empty(&mut arena);
        lambda(&mut arena, domain, body)
    };
    check_type(
        &mut arena,
        &Context::empty(),
        other,
        strict_unit,
        &mut budget,
    )
    .unwrap();
    assert!(
        convertible(
            &mut arena,
            &Context::empty(),
            tt,
            other,
            strict_unit,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn strict_formers_check_parametrically_inside_declarations() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `squashId : Π(A : Type u). Π(x : A). Squash A := λA. λx. sq_A x`
    // — one level parameter, an ordinary checked definition. Under the
    // A binder A = 0; under both binders A = 1, x = 0.
    let ty = {
        let type_u = sort_level(&mut arena, Level::Parameter(0));
        let inner = {
            let domain = variable(&mut arena, 0);
            let carrier = variable(&mut arena, 1);
            let codomain = squash(&mut arena, carrier);
            pi(&mut arena, domain, codomain)
        };
        pi(&mut arena, type_u, inner)
    };
    let body = {
        let type_u = sort_level(&mut arena, Level::Parameter(0));
        let inner = {
            let domain = variable(&mut arena, 0);
            let carrier = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let witness = squash_intro(&mut arena, carrier, bound);
            lambda(&mut arena, domain, witness)
        };
        lambda(&mut arena, type_u, inner)
    };
    let declaration = Declaration::definition(1, ty, body);
    let signature = check_signature(&mut arena, &[declaration], &mut budget).unwrap();

    // A definition carries no assumptions: its closure is empty.
    assert!(assumption_closure(&arena, &signature, &[0]).is_empty());

    // `squashId[0] : Π(A : Type 0). Π(x : A). Squash A` — instantiated
    // through `Constant` with exact level arguments.
    let context = Context::empty().with_signature(signature);
    let reference = constant(&mut arena, 0, vec![Level::Constant(0)]);
    let inferred = infer_type(&mut arena, &context, reference, &mut budget).unwrap();
    let expected = {
        let type_zero = type_sort(&mut arena, 0);
        let inner = {
            let domain = variable(&mut arena, 0);
            let carrier = variable(&mut arena, 1);
            let codomain = squash(&mut arena, carrier);
            pi(&mut arena, domain, codomain)
        };
        pi(&mut arena, type_zero, inner)
    };
    assert!(arena.structurally_equal(inferred, expected));

    // An assumption of squashed shape lands in the closure exactly:
    // `sqAxiom : Π(A : Type u). Squash A` is recorded, never unfolded.
    let axiom_ty = {
        let type_u = sort_level(&mut arena, Level::Parameter(0));
        let carrier = variable(&mut arena, 0);
        let codomain = squash(&mut arena, carrier);
        pi(&mut arena, type_u, codomain)
    };
    let axiom = Declaration::assumption(1, axiom_ty);
    let signature = check_signature(&mut arena, &[axiom], &mut budget).unwrap();
    let closure = assumption_closure(&arena, &signature, &[0]);
    assert_eq!(closure, [0].into_iter().collect());
}
