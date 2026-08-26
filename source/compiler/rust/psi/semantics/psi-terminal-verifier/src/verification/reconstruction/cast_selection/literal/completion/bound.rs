//! Independent direct landed-literal root-bound replay.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
    source_endpoint: ScalarTerm,
    endpoint: usize,
) -> Option<Proposition> {
    let closed = if endpoint == 1 {
        closed_integer_less_or_equal(&source_endpoint, landed_literal)
    } else {
        closed_integer_less_or_equal(landed_literal, &source_endpoint)
    };
    if !closed {
        return None;
    }
    Some(if endpoint == 1 {
        Proposition::LessOrEqual(source_endpoint, root.clone())
    } else {
        Proposition::LessOrEqual(root.clone(), source_endpoint)
    })
}
