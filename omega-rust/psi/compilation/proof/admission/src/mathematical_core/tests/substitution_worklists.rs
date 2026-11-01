//! Binder-sensitive arena walks must use the platform default stack.

use super::{default_budget, pi, two, type_sort, variable};
use crate::mathematical_core::substitution::{instantiate_levels, shift, substitute};
use crate::mathematical_core::{Context, Level, Sort, Term, TermArena, check_type};

#[test]
fn deep_closed_telescope_remapping_stays_on_default_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let domain = two(&mut arena);
        let mut term = domain;
        for _ in 0..131_072 {
            term = pi(&mut arena, domain, term);
        }
        let expected = type_sort(&mut arena, 0);
        check_type(
            &mut arena,
            &Context::empty(),
            term,
            expected,
            &mut default_budget(),
        )
        .expect("closed telescope is a valid type before remapping");
        assert_eq!(shift(&mut arena, term, 0, 1), term);
        assert_eq!(substitute(&mut arena, term, domain), term);
        assert_eq!(instantiate_levels(&mut arena, term, &[]), Ok(term));
    })
    .join()
    .expect("term remapping uses a default-stack handle worklist");
}

#[test]
fn shared_variables_keep_their_exact_binder_scope() {
    let mut arena = TermArena::new();
    let zero = variable(&mut arena, 0);
    let two = two(&mut arena);
    let lambda = arena.insert(Term::Lambda {
        domain: two,
        body: zero,
    });
    let term = arena.insert(Term::Pair {
        first: zero,
        second: lambda,
    });
    let shifted = shift(&mut arena, term, 0, 1);
    let Term::Pair { first, second } = arena.get(shifted) else {
        panic!("pair")
    };
    assert!(matches!(arena.get(first), Term::Variable(1)));
    assert_eq!(
        second, lambda,
        "bound zero is unchanged despite sharing its handle with the free zero"
    );
    let replacement = variable(&mut arena, 4);
    let substituted = substitute(&mut arena, term, replacement);
    let Term::Pair { first, second } = arena.get(substituted) else {
        panic!("pair")
    };
    assert_eq!(first, replacement);
    assert_eq!(second, lambda);
    let free_under_binder = variable(&mut arena, 1);
    let body = arena.insert(Term::Lambda {
        domain: two,
        body: free_under_binder,
    });
    let result = substitute(&mut arena, body, replacement);
    let Term::Lambda { body, .. } = arena.get(result) else {
        panic!("lambda")
    };
    assert!(
        matches!(arena.get(body), Term::Variable(5)),
        "argument stays free under the new binder"
    );
    let higher = variable(&mut arena, 2);
    let decremented = substitute(&mut arena, higher, replacement);
    assert!(matches!(arena.get(decremented), Term::Variable(1)));
}

#[test]
fn repeated_subterms_are_remapped_once_per_scope_and_query() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let leaf = variable(&mut arena, 0);
        let mut term = leaf;
        for _ in 0..131_072 {
            term = arena.insert(Term::Pair {
                first: term,
                second: term,
            });
        }
        let before = arena.len();
        let mut result = shift(&mut arena, term, 0, 1);
        assert_eq!(
            arena.len() - before,
            131_073,
            "one remapped node per shared input, not one per path"
        );
        for _ in 0..131_072 {
            let Term::Pair { first, second } = arena.get(result) else {
                panic!("pair")
            };
            assert_eq!(first, second);
            result = first;
        }
        assert!(matches!(arena.get(result), Term::Variable(1)));
        let result = substitute(&mut arena, leaf, result);
        assert!(
            matches!(arena.get(result), Term::Variable(1)),
            "query-local mappings cannot reuse another operation's result"
        );
    })
    .join()
    .expect("shared term remapping");
}

#[test]
fn universe_instantiation_preserves_scope_and_refuses_missing_parameters() {
    let mut arena = TermArena::new();
    let level = Level::Maximum(
        Box::new(Level::Parameter(0)),
        Box::new(Level::Successor(Box::new(Level::Parameter(1)))),
    );
    let sort = arena.insert(Term::Sort(Sort::Type(level.clone())));
    let constant = arena.insert(Term::Constant {
        declaration: 17,
        levels: vec![level],
    });
    let term = arena.insert(Term::Pi {
        domain: sort,
        codomain: constant,
    });
    assert_eq!(
        instantiate_levels(&mut arena, term, &[Level::Constant(2)]),
        Err(1)
    );
    let result =
        instantiate_levels(&mut arena, term, &[Level::Constant(2), Level::Constant(4)]).unwrap();
    let Term::Pi { domain, codomain } = arena.get(result) else {
        panic!("pi")
    };
    let expected = Level::Maximum(
        Box::new(Level::Constant(2)),
        Box::new(Level::Successor(Box::new(Level::Constant(4)))),
    );
    assert_eq!(arena.get(domain), Term::Sort(Sort::Type(expected.clone())));
    assert_eq!(
        arena.get(codomain),
        Term::Constant {
            declaration: 17,
            levels: vec![expected]
        }
    );
}
