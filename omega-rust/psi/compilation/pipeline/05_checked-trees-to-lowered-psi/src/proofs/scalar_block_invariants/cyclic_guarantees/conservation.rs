//! Propose conservation of invocation-scoped mathematical contract terms.
//!
//! A guarantee can describe work remaining as well as its final result. Its
//! mathematical subterms supply candidate relations between the current header
//! telescope and immutable invocation formals. These equalities grant no facts:
//! the ordinary invariant roster must prove every initial and preserving edge,
//! and discard proposals that are not conserved. No rank supplies a premise.

use std::collections::BTreeMap;

use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarType, ValueId};
use terminal_psi::{TerminalMachine, ValueDeclaration};

pub(super) fn candidates(
    machine: &TerminalMachine,
    bound: &BTreeMap<ValueId, ValueDeclaration>,
    remaining: &mut usize,
) -> Vec<Proposition> {
    let mut candidates = Vec::new();
    let mut propositions = machine
        .contract
        .ensures
        .iter()
        .map(|clause| &clause.proposition)
        .collect::<Vec<_>>();
    while let Some(proposition) = propositions.pop() {
        if !spend(remaining) {
            break;
        }
        match proposition {
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                let mut terms = vec![right, left];
                while let Some(term) = terms.pop() {
                    if !spend(remaining) {
                        return candidates;
                    }
                    if let Some(current) = bind(term, machine, bound, remaining, 0)
                        && current != *term
                    {
                        let original = term.clone();
                        let predicate = if current < original {
                            Proposition::IntegerMathEqual(current, original)
                        } else {
                            Proposition::IntegerMathEqual(original, current)
                        };
                        if !candidates.contains(&predicate) {
                            candidates.push(predicate);
                        }
                    }
                    match term {
                        IntegerMathTerm::Add(left, right)
                        | IntegerMathTerm::Subtract(left, right)
                        | IntegerMathTerm::Multiply(left, right) => {
                            terms.extend([right.as_ref(), left.as_ref()]);
                        }
                        IntegerMathTerm::ShiftLeft { value, count } => {
                            terms.extend([count.as_ref(), value.as_ref()]);
                        }
                        IntegerMathTerm::IntegerLiteral(_) | IntegerMathTerm::MathValue { .. } => {}
                    }
                }
            }
            Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
                propositions.extend(members);
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                // Discover terms, never import the surrounding conditional as
                // a fact. Every resulting equality has its own arrival proofs.
                propositions.extend([premise.as_ref(), conclusion.as_ref()]);
            }
            _ => {}
        }
    }
    candidates
}

/// Build a simultaneous substitution; replacement identifiers are not visited
/// again. Reject results, missing formals, and mismatched finite carriers.
fn bind(
    term: &IntegerMathTerm,
    machine: &TerminalMachine,
    bound: &BTreeMap<ValueId, ValueDeclaration>,
    remaining: &mut usize,
    depth: usize,
) -> Option<IntegerMathTerm> {
    if depth >= 64 || !spend(remaining) {
        return None;
    }
    let mut child = |term| bind(term, machine, bound, remaining, depth + 1).map(Box::new);
    Some(match term {
        IntegerMathTerm::IntegerLiteral(value) => IntegerMathTerm::IntegerLiteral(*value),
        IntegerMathTerm::MathValue { source_type, value } => {
            let formal = machine
                .parameters
                .iter()
                .find(|formal| formal.id == *value)?;
            let current = bound.get(value)?;
            if formal.scalar_type != ScalarType::Integer(*source_type)
                || current.scalar_type != formal.scalar_type
            {
                return None;
            }
            IntegerMathTerm::MathValue {
                source_type: *source_type,
                value: current.id,
            }
        }
        IntegerMathTerm::Add(left, right) => IntegerMathTerm::Add(child(left)?, child(right)?),
        IntegerMathTerm::Subtract(left, right) => {
            IntegerMathTerm::Subtract(child(left)?, child(right)?)
        }
        IntegerMathTerm::Multiply(left, right) => {
            IntegerMathTerm::Multiply(child(left)?, child(right)?)
        }
        IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
            value: child(value)?,
            count: child(count)?,
        },
    })
}

fn spend(remaining: &mut usize) -> bool {
    let Some(next) = remaining.checked_sub(1) else {
        return false;
    };
    *remaining = next;
    true
}
