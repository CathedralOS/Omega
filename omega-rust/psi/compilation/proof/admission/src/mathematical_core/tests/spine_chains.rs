//! Deep single-child chains — binder telescopes, application spines,
//! tuple witnesses and projection chains — run on collected worklists
//! instead of the call stack. Every test runs on the ordinary test
//! thread: a regression to one native frame per link fails here rather
//! than being hidden by the verification stack's reservation.
//!
//! `DEEP` sits far past the ~260-link certificate chains the C2L census
//! measured. Dependent application, pair and projection tests use `SHALLOW`
//! to bound their repeated substitution work. Separate remapping tests cover
//! `DEEP` without replacing the complete dependent judgments here.

use super::{apply, default_budget, lambda, pair, pi, sigma, snd, two, two_zero, variable};
use crate::mathematical_core::{
    Context, CoreError, Term, TermArena, TermHandle, check_type, infer_type,
};

const DEEP: usize = 131_072;
const SHALLOW: usize = 4_096;

/// `Π(_ : Two). … Π(_ : Two). Two` — a closed telescope `n` binders deep.
fn two_telescope(arena: &mut TermArena, n: usize) -> TermHandle {
    let mut tail = two(arena);
    for _ in 0..n {
        let domain = two(arena);
        tail = pi(arena, domain, tail);
    }
    tail
}

#[test]
fn long_lambda_spine_infers_off_the_call_stack() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    // `λ(_ : Two). … λ(_ : Two). zero : Π(_ : Two). … Π(_ : Two). Two`.
    let mut term = two_zero(&mut arena);
    for _ in 0..DEEP {
        let domain = two(&mut arena);
        term = lambda(&mut arena, domain, term);
    }
    let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();

    // Walk the inferred telescope iteratively — a recursive comparison
    // would burn the very stack this test exists to prove unnecessary.
    let mut cursor = inferred;
    for _ in 0..DEEP {
        match arena.get(cursor) {
            Term::Pi { domain, codomain } => {
                assert!(matches!(arena.get(domain), Term::Two));
                cursor = codomain;
            }
            other => panic!("expected a Pi link, found {other:?}"),
        }
    }
    assert!(matches!(arena.get(cursor), Term::Two));
}

#[test]
fn long_pair_witness_checks_against_a_sigma_chain() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    // `pair(zero, pair(zero, … zero)) : Σ(_ : Two). Σ(_ : Two). … Two` —
    // the right-nested conjunction shape the per-condition certificates
    // carry.
    let mut witness = two_zero(&mut arena);
    for _ in 0..SHALLOW {
        let first = two_zero(&mut arena);
        witness = pair(&mut arena, first, witness);
    }
    let mut expected = two(&mut arena);
    for _ in 0..SHALLOW {
        let domain = two(&mut arena);
        expected = sigma(&mut arena, domain, expected);
    }
    check_type(&mut arena, &context, witness, expected, &mut budget).unwrap();

    // The inferred type of the same witness is the right-nested `Σ`
    // chain; walk it iteratively for the same reason as the lambda case.
    let inferred = infer_type(&mut arena, &context, witness, &mut budget).unwrap();
    let mut cursor = inferred;
    for _ in 0..SHALLOW {
        match arena.get(cursor) {
            Term::Sigma { domain, codomain } => {
                assert!(matches!(arena.get(domain), Term::Two));
                cursor = codomain;
            }
            other => panic!("expected a Sigma link, found {other:?}"),
        }
    }
    assert!(matches!(arena.get(cursor), Term::Two));
}

#[test]
fn left_nested_pair_witness_checks_on_the_default_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let zero = two_zero(&mut arena);
        let mut witness = zero;
        let mut expected = two;
        for _ in 0..SHALLOW {
            witness = pair(&mut arena, witness, zero);
            expected = sigma(&mut arena, expected, two);
        }
        check_type(
            &mut arena,
            &Context::empty(),
            witness,
            expected,
            &mut default_budget(),
        )
        .unwrap();
        let inferred = infer_type(
            &mut arena,
            &Context::empty(),
            witness,
            &mut default_budget(),
        )
        .unwrap();
        assert!(arena.structurally_equal(inferred, expected));
        assert!(matches!(
            check_type(
                &mut arena,
                &Context::empty(),
                witness,
                two,
                &mut default_budget(),
            ),
            Err(CoreError::TypeMismatch { expected, .. }) if expected == two
        ));
    })
    .join()
    .expect("both pair components use the default stack");
}

#[test]
fn pair_checking_restores_binders_and_substitutes_only_checked_components() {
    let mut arena = TermArena::new();
    let universe = super::type_sort(&mut arena, 0);
    let a_in_prefix = variable(&mut arena, 0);
    let context = Context::empty().extend(universe).extend(a_in_prefix);
    let two = two(&mut arena);
    let zero = two_zero(&mut arena);
    let bound = variable(&mut arena, 0);
    let identity = lambda(&mut arena, two, bound);
    let identity_type = pi(&mut arena, two, two);
    let ambient_a = variable(&mut arena, 0);
    let a_under_pair = variable(&mut arena, 2);
    let expected = sigma(&mut arena, identity_type, a_under_pair);
    let witness = pair(&mut arena, identity, ambient_a);
    check_type(
        &mut arena,
        &context,
        witness,
        expected,
        &mut default_budget(),
    )
    .unwrap();

    // The second component is checked under the ambient context, with the
    // *first value* substituted into its dependent codomain.
    let family = variable(&mut arena, 0);
    let dependent = sigma(&mut arena, universe, family);
    let witness = pair(&mut arena, two, zero);
    check_type(
        &mut arena,
        &Context::empty(),
        witness,
        dependent,
        &mut default_budget(),
    )
    .unwrap();
    let invalid = pair(&mut arena, two, two);
    assert!(matches!(
        check_type(
            &mut arena,
            &Context::empty(),
            invalid,
            dependent,
            &mut default_budget(),
        ),
        Err(CoreError::TypeMismatch { expected, .. }) if expected == two
    ));

    // A binder introduced while checking a first component cannot supply a
    // previously unbound variable to its sibling.
    let unbound = variable(&mut arena, 0);
    let expected = sigma(&mut arena, identity_type, two);
    let invalid = pair(&mut arena, identity, unbound);
    assert_eq!(
        check_type(
            &mut arena,
            &Context::empty(),
            invalid,
            expected,
            &mut default_budget(),
        ),
        Err(CoreError::UnboundVariable {
            index: 0,
            context_depth: 0
        })
    );
}

#[test]
fn pair_judgments_refuse_the_first_component_before_visiting_the_second() {
    let mut arena = TermArena::new();
    let dummy = arena.insert(Term::Dummy);
    let unbound = variable(&mut arena, 0);
    let witness = pair(&mut arena, dummy, unbound);
    let two = two(&mut arena);
    let expected = sigma(&mut arena, two, two);
    assert_eq!(
        infer_type(
            &mut arena,
            &Context::empty(),
            witness,
            &mut default_budget()
        ),
        Err(CoreError::DummyTerm)
    );
    assert_eq!(
        check_type(
            &mut arena,
            &Context::empty(),
            witness,
            expected,
            &mut default_budget(),
        ),
        Err(CoreError::DummyTerm)
    );
}

#[test]
fn long_application_spine_consumes_arguments_in_order() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `f : Π(_ : Two). … Π(_ : Two). Two` applied to `zero` `SHALLOW`
    // times lands at `Two`. Each argument performs its own dependent
    // codomain substitution, so this case bounds total work at `SHALLOW`;
    // the independent remapping tests exercise much deeper term graphs.
    let function_type = two_telescope(&mut arena, SHALLOW);
    let context = Context::empty().extend(function_type);
    let mut term = variable(&mut arena, 0);
    for _ in 0..SHALLOW {
        let argument = two_zero(&mut arena);
        term = apply(&mut arena, term, argument);
    }
    let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Two));

    // Applying the `n`-th argument where the telescope has ended reports
    // the partially applied spine as the non-function — the payload keeps
    // the exact `function` subterm the judgment rejected.
    let mut over_applied = term;
    for _ in 0..2 {
        let argument = two_zero(&mut arena);
        over_applied = apply(&mut arena, over_applied, argument);
    }
    let error = infer_type(&mut arena, &context, over_applied, &mut budget).unwrap_err();
    match error {
        CoreError::NotAFunction {
            function,
            actual_type,
        } => {
            assert_eq!(function, term);
            assert!(matches!(arena.get(actual_type), Term::Two));
        }
        other => panic!("expected NotAFunction, found {other:?}"),
    }
}

#[test]
fn long_snd_chain_projects_off_the_call_stack() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `w : Σ(_ : Two). Σ(_ : Two). … Two`; `snd^SHALLOW w : Two`. The
    // dependent result substitutes the reconstructed `fst` at every link,
    // so this too bounds the total repeated substitution work at `SHALLOW`.
    let mut w_type = two(&mut arena);
    for _ in 0..SHALLOW {
        let domain = two(&mut arena);
        w_type = sigma(&mut arena, domain, w_type);
    }
    let context = Context::empty().extend(w_type);
    let mut term = variable(&mut arena, 0);
    for _ in 0..SHALLOW {
        term = snd(&mut arena, term);
    }
    let inferred = infer_type(&mut arena, &context, term, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Two));
}

#[test]
fn a_chain_rejects_at_its_end() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    // `λ(_ : Two).^SHALLOW v` with `v` one binder past the telescope:
    // the walk must reach the tail before reporting the unbound index.
    let out_of_scope = u32::try_from(SHALLOW).unwrap();
    let mut term = variable(&mut arena, out_of_scope);
    for _ in 0..SHALLOW {
        let domain = two(&mut arena);
        term = lambda(&mut arena, domain, term);
    }
    let error = infer_type(&mut arena, &context, term, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundVariable {
            index: out_of_scope,
            context_depth: SHALLOW,
        }
    );
}

#[test]
fn deep_binder_domains_form_on_the_default_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let mut term = two;
        for _ in 0..DEEP {
            term = pi(&mut arena, term, two);
        }
        let inferred = infer_type(&mut arena, &Context::empty(), term, &mut default_budget())
            .expect("the complete left-nested binder tree is a valid relevant type");
        assert_eq!(
            arena.get(inferred),
            Term::Sort(crate::mathematical_core::Sort::Type(
                crate::mathematical_core::Level::Constant(0)
            ))
        );
    })
    .join()
    .expect("binder-domain formation uses the default stack");
}

#[test]
fn binder_formation_restores_the_exact_prefix_between_children() {
    let mut arena = TermArena::new();
    let universe = super::type_sort(&mut arena, 0);
    let a = variable(&mut arena, 0);
    let a_under_x = variable(&mut arena, 1);
    let inner = pi(&mut arena, a, a_under_x);
    let term = pi(&mut arena, universe, inner);
    let inferred = infer_type(&mut arena, &Context::empty(), term, &mut default_budget()).unwrap();
    assert_eq!(
        arena.get(inferred),
        Term::Sort(crate::mathematical_core::Sort::Type(
            crate::mathematical_core::Level::Constant(1)
        ))
    );
    // Layer formation uses the same exact binder prefix, including when it
    // forms a dependent domain in another binder.
    let proposition = arena.insert(Term::Squash { ty: a });
    let boxed = arena.insert(Term::Box { ty: proposition });
    let inner = pi(&mut arena, boxed, a_under_x);
    let composed = pi(&mut arena, universe, inner);
    let inferred = infer_type(
        &mut arena,
        &Context::empty(),
        composed,
        &mut default_budget(),
    )
    .unwrap();
    assert_eq!(
        arena.get(inferred),
        Term::Sort(crate::mathematical_core::Sort::Type(
            crate::mathematical_core::Level::Constant(1)
        ))
    );
    // The domain of the outer binder is checked before that binder exists.
    // A completed sibling may not donate its binding to this same handle.
    let invalid = pi(&mut arena, a, universe);
    assert_eq!(
        infer_type(
            &mut arena,
            &Context::empty(),
            invalid,
            &mut default_budget()
        ),
        Err(CoreError::UnboundVariable {
            index: 0,
            context_depth: 0
        })
    );
    let two = super::two(&mut arena);
    let invalid_sort = pi(&mut arena, two, a);
    assert!(
        matches!(
            infer_type(
                &mut arena,
                &Context::empty(),
                invalid_sort,
                &mut default_budget()
            ),
            Err(CoreError::NotASort { .. })
        ),
        "a bound value does not become a type"
    );
}

#[test]
fn deep_relevant_and_strict_formations_preserve_layer_refusals() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let mut term = two;
        for _ in 0..DEEP / 2 {
            let proposition = arena.insert(Term::Squash { ty: term });
            term = arena.insert(Term::Box { ty: proposition });
        }
        let inferred =
            infer_type(&mut arena, &Context::empty(), term, &mut default_budget()).unwrap();
        assert_eq!(
            arena.get(inferred),
            Term::Sort(crate::mathematical_core::Sort::Type(
                crate::mathematical_core::Level::Constant(0)
            ))
        );
        let invalid = arena.insert(Term::Box { ty: two });
        let mut nested_invalid = invalid;
        for _ in 0..DEEP / 2 {
            let proposition = arena.insert(Term::Squash { ty: nested_invalid });
            nested_invalid = arena.insert(Term::Box { ty: proposition });
        }
        assert_eq!(
            infer_type(
                &mut arena,
                &Context::empty(),
                nested_invalid,
                &mut default_budget()
            ),
            Err(CoreError::NonStrictBoxDomain { domain: two })
        );
        let empty = arena.insert(Term::Empty);
        let invalid = arena.insert(Term::Squash { ty: empty });
        assert_eq!(
            infer_type(
                &mut arena,
                &Context::empty(),
                invalid,
                &mut default_budget()
            ),
            Err(CoreError::StrictSquashDomain { domain: empty })
        );
    })
    .join()
    .expect("relevant and strict formation uses the default stack");
}
