//! Source-ordered oriented equalities for affine certificate production.

use std::cell::RefCell;
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::integer_evidence::{Citation, cited_facts};
use super::super::scope_identity::{self, ScopeDerived};

fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}

pub(super) fn ordered<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    cited_facts(assumptions, semantic_axioms)
        .filter_map(|(citation, equality)| match equality {
            Proposition::Equal(left, right) => Some((citation, equality, left, right)),
            _ => None,
        })
        .flat_map(|(citation, equality, left, right)| {
            [
                (citation, equality, left, right),
                (citation, equality, right, left),
            ]
        })
}

/// `ordered` restricted to value roots landed on an in-type integer literal.
/// Deciding a landing evaluates the literal side, and producers walk the
/// bindings at many goals under one scope, so the selection is derived once
/// per scope as `(citation, reversed)` orientations and resolved against the
/// caller's slices in the same order.
pub(super) fn exact_value_bindings<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    let bindings = exact_value_binding_orientations(assumptions, semantic_axioms);
    (0..bindings.len()).map(move |index| {
        let (citation, reversed) = bindings[index];
        oriented(citation, reversed, assumptions, semantic_axioms)
    })
}

/// The `(citation, reversed)` orientations `exact_value_bindings` resolves.
pub(super) fn exact_value_binding_orientations(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Rc<Vec<(Citation, bool)>> {
    thread_local! {
        static BINDINGS: RefCell<ScopeDerived<Vec<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    scope_identity::derived(&BINDINGS, assumptions, semantic_axioms, || {
        cited_facts(assumptions, semantic_axioms)
            .filter_map(|(citation, equality)| match equality {
                Proposition::Equal(left, right) => Some((citation, left, right)),
                _ => None,
            })
            .flat_map(|(citation, left, right)| {
                [
                    (citation, false, left, right),
                    (citation, true, right, left),
                ]
            })
            .filter(|(_, _, root, literal)| exact_value_binding(root, literal))
            .map(|(citation, reversed, _, _)| (citation, reversed))
            .collect()
    })
}

/// The cited equality at `citation`, oriented root-first.
pub(super) fn oriented<'a>(
    citation: Citation,
    reversed: bool,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm) {
    let equality = citation.fact(assumptions, semantic_axioms);
    let Proposition::Equal(left, right) = equality else {
        unreachable!("oriented citations name cited equalities")
    };
    if reversed {
        (citation, equality, right, left)
    } else {
        (citation, equality, left, right)
    }
}

pub(super) fn value_aliases<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(assumptions, semantic_axioms).filter(|(_, _, root, alias)| {
        root != alias
            && matches!(root, ScalarTerm::Value { .. })
            && matches!(alias, ScalarTerm::Value { .. })
    })
}
