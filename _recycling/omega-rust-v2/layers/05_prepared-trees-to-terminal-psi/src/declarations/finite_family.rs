//! Recover only explicitly listed tuples from the lowered requirement predicate.
//! An inequality, an ordinary formal, or an intersection of independent choices
//! keeps its ordinary predicate and does not request a specialization roster.
use crate::{ContractClause, PredicateOperator, PredicateStep, ScalarLiteral};

enum Term {
    Index(usize),
    Literal(ScalarLiteral),
    Bindings(Vec<(usize, ScalarLiteral)>),
    Alternatives(Vec<Vec<(usize, ScalarLiteral)>>),
}

pub(super) fn declared_family(
    arity: usize,
    clauses: &[ContractClause],
) -> Option<Vec<Vec<ScalarLiteral>>> {
    if arity == 0 {
        return None;
    }
    let mut rosters = clauses
        .iter()
        .filter_map(|clause| complete_tuples(arity, &clause.predicate));
    let mut tuples = rosters.next()?;
    for bounds in rosters {
        tuples.retain(|tuple| bounds.binary_search(tuple).is_ok());
    }
    Some(tuples)
}

fn complete_tuples(
    arity: usize,
    predicate: &[PredicateStep],
) -> Option<Vec<Vec<ScalarLiteral>>> {
    let mut stack = Vec::new();
    for step in predicate {
        let term = match step {
            PredicateStep::ValueIndex(index) => Term::Index(*index),
            PredicateStep::Integer(n) => Term::Literal(ScalarLiteral::Integer(n.clone())),
            PredicateStep::Boolean(b) => Term::Literal(ScalarLiteral::Boolean(*b)),
            PredicateStep::Apply(op) => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                match (op, left, right) {
                    (PredicateOperator::Equal, Term::Index(i), Term::Literal(v)) | (PredicateOperator::Equal, Term::Literal(v), Term::Index(i)) => {
                        Term::Bindings(vec![(i, v)])
                    }
                    (PredicateOperator::And, Term::Bindings(mut a), Term::Bindings(b)) => {
                        a.extend(b);
                        Term::Bindings(a)
                    }
                    (PredicateOperator::Or, a, b) => {
                        let mut rows = alternatives(a)?;
                        rows.extend(alternatives(b)?);
                        Term::Alternatives(rows)
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        stack.push(term);
    }
    let rows = alternatives(stack.pop()?)?;
    if !stack.is_empty() {
        return None;
    }
    let mut tuples = Vec::with_capacity(rows.len());
    for mut row in rows {
        row.sort_unstable_by_key(|(index, _)| *index);
        row.dedup();
        if row.len() != arity || !row.iter().enumerate().all(|(i, (slot, _))| i == *slot) {
            return None;
        }
        tuples.push(row.into_iter().map(|(_, value)| value).collect());
    }
    tuples.sort_unstable();
    tuples.dedup();
    Some(tuples)
}
fn alternatives(term: Term) -> Option<Vec<Vec<(usize, ScalarLiteral)>>> {
    match term {
        Term::Bindings(one) => Some(vec![one]),
        Term::Alternatives(rows) => Some(rows),
        _ => None,
    }
}
