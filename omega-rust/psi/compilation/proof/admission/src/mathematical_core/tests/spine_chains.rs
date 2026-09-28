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
