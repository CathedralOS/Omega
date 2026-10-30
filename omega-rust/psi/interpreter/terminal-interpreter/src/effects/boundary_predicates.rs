//! One closed predicate evaluator for boundary input obligations and reported
//! crash guards. Callers provide only evaluated literals in the exact declared
//! scalar telescope; the evaluator never reads ambient caller value IDs.

use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use terminal_psi::BoundaryMachineDeclaration;

pub(super) fn scalar_literals(arguments: &[crate::values::TerminalScalarValue]) -> Vec<ScalarTerm> {
    arguments
        .iter()
        .map(|argument| match *argument {
            crate::values::TerminalScalarValue::Boolean(value) => ScalarTerm::Boolean(value),
            crate::values::TerminalScalarValue::Integer { scalar_type, value } => {
                ScalarTerm::Integer { scalar_type, value }
            }
            crate::values::TerminalScalarValue::IeeeFloat(value) => {
                ScalarTerm::IeeeFloatLiteral(value)
            }
        })
        .collect()
}

pub(crate) fn validate_input_requirements(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[crate::values::TerminalScalarValue],
) -> Result<(), crate::errors::TerminalInterpretError> {
    use crate::errors::{TerminalInterpretError, malformed};
    if boundary.scalar_requires.is_empty() {
        return Ok(());
    }
    let arguments = scalar_literals(arguments);
    let parameters = boundary
        .scalar_contract_parameters()
        .ok_or_else(malformed)?;
    if arguments.len() != parameters.len()
        || arguments
            .iter()
            .zip(&parameters)
            .any(|(actual, parameter)| {
                actual.scalar_type() != parameter.scalar_type || actual.validate().is_err()
            })
    {
        return Err(malformed());
    }
    proof_admission::check_predicate_evaluation_size(boundary.scalar_requires.iter()).map_err(
        |_| {
            TerminalInterpretError::UnsupportedSemanticVariant(
                "boundary input requirement evaluation exceeds the resource limit",
            )
        },
    )?;
    let context = PropositionContext::from_value_types(
        parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
    )
    .map_err(|_| malformed())?;
    let substitutions = parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, argument)| (parameter.id, argument.clone()))
        .collect();
    let mut integers = ClosedIntegerEvaluator::default();
    for requirement in &boundary.scalar_requires {
        if requirement.validate().is_err() || context.validate(requirement).is_err() {
            return Err(malformed());
        }
        let requirement = terminal_semantics::substitution::substitute_proposition_values(
            requirement,
            &substitutions,
        );
        match closed_guard(&requirement, &mut integers) {
            Ok(Some(true)) => {}
            Ok(Some(false)) => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement is false for this invocation",
                ));
            }
            Ok(None) => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement has no supported closed evaluation",
                ));
            }
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded) => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "boundary input requirement evaluation exceeds the resource limit",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) =
                (left, right)
            else {
                return Ok(None);
            };
            use semantic_vocabulary::{
                IeeeFloatComparisonKind as Kind, IeeeFloatComparisonOperation as Operation,
            };
            let (operation, negate) = match kind {
                Kind::Equal => (Operation::Equal, false),
                Kind::NotEqual => (Operation::NotEqual, false),
                Kind::LessThan => (Operation::Less, false),
                Kind::LessOrEqual => (Operation::LessOrEqual, false),
                Kind::NotLessThan => (Operation::Less, true),
                Kind::NotLessOrEqual => (Operation::LessOrEqual, true),
            };
            Some(crate::scalar_operations::ieee_float_compare(operation, *left, *right) != negate)
        }
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
            .boolean_value()
            .zip(right.boolean_value())
            .map(|(left, right)| left == right),
        semantic_vocabulary::ordered_comparison_operands!(left, right) => {
            let Some(((left_type, left), (right_type, right))) =
                left.integer_value().zip(right.integer_value())
            else {
                return Ok(None);
            };
            if left_type != right_type {
                return Ok(None);
            }
            left_type
                .compare(left, right)
                .map(|ordering| match proposition {
                    Proposition::Equal(..) => ordering.is_eq(),
                    Proposition::LessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                })
        }
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => integers
            .compare(left, right)?
            .map(|ordering| match proposition {
                Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let conjunction = matches!(proposition, Proposition::Conjunction(_));
            let mut value = conjunction;
            for child in children {
                let Some(child) = closed_guard(child, integers)? else {
                    return Ok(None);
                };
                value = if conjunction {
                    value && child
                } else {
                    value || child
                };
            }
            Some(value)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match closed_guard(premise, integers)? {
            Some(false) => Some(true),
            Some(true) => closed_guard(conclusion, integers)?,
            None => None,
        },
        _ => None,
    })
}
