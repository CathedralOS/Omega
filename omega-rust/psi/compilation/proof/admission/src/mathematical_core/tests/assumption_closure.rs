//! Assumption policy must traverse the complete stored graph on a default stack.

use super::{constant, default_budget, pi, two, type_sort};
use crate::mathematical_core::{
    Declaration, MathematicalCertificate, Term, TermArena, certificate_assumption_closure,
    check_signature, judgment_assumption_closure, verification_workers_started,
    verify_mathematical_certificate,
};
use std::collections::BTreeSet;

#[test]
fn certificate_assumption_closure_stays_on_callers_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let universe = type_sort(&mut arena, 0);
        let abstract_type = constant(&mut arena, 0, Vec::new());
        let signature = vec![
            Declaration::assumption(0, universe),
            Declaration::assumption(0, universe), // Unused: never a policy root.
            Declaration::definition(0, universe, abstract_type),
        ];
        let mut term = constant(&mut arena, 2, Vec::new());
        let domain = two(&mut arena);
        for _ in 0..131_072 {
            term = pi(&mut arena, domain, term);
        }
        let certificate =
            MathematicalCertificate::with_signature(signature, Vec::new(), term, universe);
        // This is a valid judgment, not a malformed graph whose policy result
        // could substitute for typing. General kernel verification stays intact.
        verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())
            .expect("deep closed telescope has its declared universe");
        let workers = verification_workers_started();
        assert_eq!(
            certificate_assumption_closure(&arena, &certificate),
            BTreeSet::from([0]),
            "the definition's body contributes its axiom; unused declarations do not"
        );
        assert_eq!(
            verification_workers_started(),
            workers,
            "assumption traversal must stay on the caller's default stack"
        );
    })
    .join()
    .expect("default-stack assumption traversal");
}

#[test]
fn shared_term_graph_retains_exact_signature_dependencies() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let universe = type_sort(&mut arena, 0);
        let first = constant(&mut arena, 0, Vec::new());
        let second = constant(&mut arena, 1, Vec::new());
        let alias = constant(&mut arena, 2, Vec::new());
        let mut graph = alias;
        // Repeated paths describe a finite DAG, not an exponentially expanded
        // tree. Both fields remain policy dependencies through the shared node.
        for _ in 0..131_072 {
            graph = arena.insert(Term::Pi {
                domain: graph,
                codomain: graph,
            });
        }
        for (body, expected) in [(first, 0), (second, 1)] {
            let declarations = [
                Declaration::assumption(0, universe),
                Declaration::assumption(0, universe),
                Declaration::definition(0, universe, body),
            ];
            let signature = check_signature(&mut arena, &declarations, &mut default_budget())
                .expect("each independent signature is well formed");
            assert_eq!(
                judgment_assumption_closure(&arena, &signature, &[graph, alias, graph]),
                BTreeSet::from([expected]),
                "equal term handles cannot reuse another signature's assumption closure"
            );
        }
    })
    .join()
    .expect("default-stack shared graph traversal");
}
