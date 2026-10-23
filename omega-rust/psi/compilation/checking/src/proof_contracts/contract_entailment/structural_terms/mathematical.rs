//! Rebind checked calculus applications to the caller's observed values.
//!
//! The mathematical producer has already checked and reduced the application.
//! This consumer reads that term, never a source definition body. Its callback
//! owns observation and substitution; kernel Int alone supplies no permission
//! to reread storage or to replace a selected source operation's meaning.

use super::StructuralTerm;
use crate::mathematical_applications::{PreparedConstantMeaning, PreparedMathematicalView};
use proof_admission::{Term, TermHandle};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

pub(in crate::proof_contracts::contract_entailment) fn application_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    mut observe: impl FnMut(ExpressionHandle) -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    let view = program.prepared_mathematics()?;
    let application = view.application(expression)?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .map(|capture| {
            let value = observe(capture.expression)?;
            let value = if program
                .primitive_type_reference(capture.source_type)
                .is_some_and(|primitive| {
                    primitive.is_fixed_integer()
                        || primitive == typed_trees::types::PrimitiveType::Addr
                }) {
                StructuralTerm::IntegerEmbedding {
                    reference: capture.source_type,
                    value: Box::new(value),
                }
            } else {
                value
            };
            Some(Some(value))
        })
        .collect::<Option<Vec<_>>>()?;
    read_term(program, view, application.term, &captures, 0)
}

fn read_term(
    program: &dyn crate::CheckingInput,
    view: PreparedMathematicalView<'_>,
    term: TermHandle,
    captures: &[Option<StructuralTerm>],
    depth: usize,
) -> Option<StructuralTerm> {
    if depth >= 128 {
        return None;
    }
    let read = |term| read_term(program, view, term, captures, depth + 1);
    match view.arena.get(term) {
        Term::Variable(index) => captures
            .get(captures.len().checked_sub(index as usize + 1)?)?
            .clone(),
        Term::Squash { ty } => read(ty),
        Term::Id { ty, left, right } => {
            let Term::Constant { declaration, .. } = view.arena.get(ty) else {
                return None;
            };
            if !matches!(
                view.constant(declaration),
                Some(
                    PreparedConstantMeaning::IntegerCarrier
                        | PreparedConstantMeaning::AddressCarrier
                )
            ) {
                return None;
            }
            Some(binary(BinaryOperator::Equal, read(left)?, read(right)?))
        }
        Term::Sigma { domain, codomain } if !matches!(view.arena.get(domain), Term::Two) => {
            let mut extended = captures.to_vec();
            extended.push(None);
            Some(binary(
                BinaryOperator::And,
                read(domain)?,
                read_term(program, view, codomain, &extended, depth + 1)?,
            ))
        }
        _ => {
            let (declaration, arguments) =
                crate::mathematical_applications::application_spine(view, term)?;
            match (view.constant(declaration)?, arguments.as_slice()) {
                (PreparedConstantMeaning::Integer(value), []) => {
                    Some(StructuralTerm::Integer(value.clone()))
                }
                (PreparedConstantMeaning::AddressBound(symbol), [])
                    if program.symbols.builtin_type_member(*symbol)
                        == Some(symbols::BuiltinTypeMember::AddressBound) =>
                {
                    Some(StructuralTerm::BoundValue(*symbol))
                }
                (
                    PreparedConstantMeaning::IntegerOperation(operator)
                    | PreparedConstantMeaning::IntegerRelation(operator),
                    [left, right],
                ) => Some(binary(*operator, read(*left)?, read(*right)?)),
                // Captures already retain the source carrier. The checked
                // embedding preserves that value; it does not land it again.
                (PreparedConstantMeaning::IntegerEmbedding { .. }, [argument]) => read(*argument),
                (PreparedConstantMeaning::Negation, [proposition]) => {
                    let operand = read(*proposition)?;
                    // MathematicalBinary comparisons here come only from
                    // authenticated integer relations/identity, never IEEE or
                    // a selected custom comparison. Retain their exact
                    // complement instead of hiding it in Boolean equality.
                    if let StructuralTerm::MathematicalBinary {
                        operator,
                        left,
                        right,
                    } = &operand
                        && let Some(operator) = operator.complement()
                    {
                        return Some(StructuralTerm::MathematicalBinary {
                            operator,
                            left: left.clone(),
                            right: right.clone(),
                        });
                    }
                    Some(binary(
                        BinaryOperator::Equal,
                        operand,
                        StructuralTerm::Constructor {
                            data: "bool".to_owned(),
                            case: "false".to_owned(),
                            fields: Vec::new(),
                        },
                    ))
                }
                // Opaque and source-selected operations have no builtin
                // arithmetic law merely because their kernel result is Int.
                _ => None,
            }
        }
    }
}

fn binary(operator: BinaryOperator, left: StructuralTerm, right: StructuralTerm) -> StructuralTerm {
    StructuralTerm::MathematicalBinary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}
