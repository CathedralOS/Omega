use super::{
    apply, constant, default_budget, instantiated_identity_type, lambda, offset, pair, pi,
    polymorphic_identity_declaration, sigma, sort_level, type_sort, variable,
};
use crate::mathematical_core::term::sorts_equal;
use crate::mathematical_core::{
    Budget, Context, CoreError, Declaration, Level, Sort, Term, TermArena, assumption_closure,
    check_signature, check_type, convertible, infer_sort, infer_type, judgment_assumption_closure,
    shift, substitute, weak_head_normalize,
};

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

// Derived indexed families — the W-based profile's `IndexedAt`/`IW`/
// `isup`/`iindW` encoding, re-decided as ordinary declarations rather
// than a second primitive inductive checker.
