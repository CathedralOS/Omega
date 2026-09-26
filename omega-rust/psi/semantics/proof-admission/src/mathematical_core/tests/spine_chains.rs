//! Deep single-child chains — binder telescopes, application spines,
//! tuple witnesses and projection chains — run on collected worklists
//! instead of the call stack. Every test runs on the ordinary test
//! thread: a regression to one native frame per link fails here rather
//! than being hidden by the verification stack's reservation.
//!
//! `DEEP` sits far past the ~260-link certificate chains the C2L census
//! measured; the checks that still route a deep subterm through the
//! recursive substitution helpers stay at `SHALLOW`, which is already
//! sixteen times that observed depth.

use super::{apply, default_budget, fst, lambda, pair, pi, sigma, snd, two, two_zero, variable};
use crate::mathematical_core::{
    Context, CoreError, Level, Sort, Term, TermArena, TermHandle, check_type, infer_sort,
    infer_type,
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
fn long_formation_telescope_infers_its_sort_off_the_call_stack() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    let telescope = two_telescope(&mut arena, DEEP);
    let sort = infer_sort(&mut arena, &context, telescope, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));

    // Alternating Π/Σ links run one shared chain rule; every component
    // is relevant `Two`, so the telescope stays `Type 0`.
    let mut tail = two(&mut arena);
    for link in 0..DEEP {
        let domain = two(&mut arena);
        tail = if link % 2 == 0 {
            pi(&mut arena, domain, tail)
        } else {
            sigma(&mut arena, domain, tail)
        };
    }
    let sort = infer_sort(&mut arena, &context, tail, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));
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
fn long_telescope_body_checks_off_the_call_stack() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    // `check_type`'s componentwise descent: a λ-telescope body against a
    // Π-telescope type iterates instead of recursing.
    let mut term = two_zero(&mut arena);
    for _ in 0..SHALLOW {
        let domain = two(&mut arena);
        term = lambda(&mut arena, domain, term);
    }
    let expected = two_telescope(&mut arena, SHALLOW);
    check_type(&mut arena, &context, term, expected, &mut budget).unwrap();
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
fn long_application_spine_consumes_arguments_in_order() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();

    // `f : Π(_ : Two). … Π(_ : Two). Two` applied to `zero` `SHALLOW`
    // times lands at `Two`. The per-link codomain substitution still
    // descends recursively through substitution.rs — outside the typing
    // judgment's own stack discipline — so this stays at `SHALLOW`.
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
    // dependent result substitutes the reconstructed `fst` each link —
    // recursion inside substitution.rs — so this too stays at `SHALLOW`.
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
fn a_single_link_still_runs_the_spine_rules() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let context = Context::empty();

    // Depth one degenerates into the plain judgments: one binder, one
    // application, one pair, one projection.
    let body = two_zero(&mut arena);
    let domain = two(&mut arena);
    let identity = lambda(&mut arena, domain, body);
    let pi_domain = two(&mut arena);
    let pi_codomain = two(&mut arena);
    let expected = pi(&mut arena, pi_domain, pi_codomain);
    check_type(&mut arena, &context, identity, expected, &mut budget).unwrap();

    let argument = two_zero(&mut arena);
    let applied = apply(&mut arena, identity, argument);
    let inferred = infer_type(&mut arena, &context, applied, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Two));

    let first = two_zero(&mut arena);
    let second = two_zero(&mut arena);
    let witness = pair(&mut arena, first, second);
    let inferred = infer_type(&mut arena, &context, witness, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Sigma { .. }));

    let projected = fst(&mut arena, witness);
    let inferred = infer_type(&mut arena, &context, projected, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Two));

    let projected = snd(&mut arena, witness);
    let inferred = infer_type(&mut arena, &context, projected, &mut budget).unwrap();
    assert!(matches!(arena.get(inferred), Term::Two));
}
