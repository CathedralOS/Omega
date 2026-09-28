use super::{
    constant, default_budget, lambda, pi, polymorphic_identity_declaration, sort_level, type_sort,
    variable,
};
use crate::mathematical_core::{
    Budget, Context, CoreError, Declaration, Level, Term, TermArena, check_signature, check_type,
    convertible, infer_type, weak_head_normalize,
};

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

// ── Declarations, constants, and the assumption closure ────────────────

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

// Derived indexed families — the W-based profile's `IndexedAt`/`IW`/
// `isup`/`iindW` encoding, re-decided as ordinary declarations rather
// than a second primitive inductive checker.
