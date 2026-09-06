//! Unit entry requirements over exact scalar and structural parameters.

use super::*;
use crate::contract_predicates::PredicateTerms;
use crate::crash_routes::{
    checked_boolean_scalar_term, checked_scalar_term, lower_structural_member_term,
};

#[cfg(test)]
mod tests;

pub(super) fn lower_structural_runtime_requirement(
    expression: &CheckedBooleanExpression,
    scalar_parameters: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Proposition, LoweringError> {
    // Validate the complete entry namespace before Boolean simplification.
    validate_namespace(expression)?;
    crate::contract_predicates::proposition(
        expression,
        &RuntimeRequirementTerms {
            scalar_parameters,
            parameters,
            structural_types,
        },
    )
}

fn validate_namespace(expression: &CheckedBooleanExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. } => Ok(()),
        CheckedBooleanExpression::Not(operand) => validate_namespace(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            validate_namespace(left)?;
            validate_namespace(right)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            validate_integer_namespace(left)?;
            validate_integer_namespace(right)
        }
        _ => unsupported("runtime requirement is outside the immutable entry parameter namespace"),
    }
}

fn validate_integer_namespace(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::StructuralParameterField { primitive_type, .. } => {
            if *primitive_type == PrimitiveType::Addr {
                return unsupported("runtime requirement cannot use an address carrier");
            }
            integer_scalar_type(*primitive_type).map(|_| ())
        }
        CheckedScalarExpression::IntegerLiteral { .. } => Ok(()),
        _ => unsupported(
            "runtime requirements admit only fixed integer parameters, members, and literals",
        ),
    }
}

struct RuntimeRequirementTerms<'parameters> {
    scalar_parameters: &'parameters [ValueDeclaration],
    parameters: &'parameters [StructuralParameterDeclaration],
    structural_types: &'parameters [StructuralTypeDeclaration],
}

impl PredicateTerms for RuntimeRequirementTerms<'_> {
    fn integer(&self, expression: &CheckedScalarExpression) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedScalarExpression::Parameter { primitive_type, .. } => {
                integer_scalar_type(*primitive_type)?;
                checked_scalar_term(expression, self.scalar_parameters)
            }
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                integer_scalar_type(*primitive_type)?,
                self.parameters,
                self.structural_types,
            ),
            CheckedScalarExpression::IntegerLiteral { literal } => {
                let scalar_type = integer_landing_scalar_type(literal)?;
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return unsupported("runtime requirement literal is not an integer");
                };
                ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            _ => unsupported("runtime requirement integer operand is outside the entry namespace"),
        }
    }

    fn boolean(&self, expression: &CheckedBooleanExpression) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedBooleanExpression::Constant(_) | CheckedBooleanExpression::Parameter { .. } => {
                checked_boolean_scalar_term(expression, self.scalar_parameters)
            }
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                ScalarType::Boolean,
                self.parameters,
                self.structural_types,
            ),
            _ => unsupported("runtime requirement Boolean operand is outside the entry namespace"),
        }
    }
}

/// Rebind the complete supported predicate without changing callee slot or term order.
/// Structural roots remain owned by the subsequent exact place substitution.
pub(super) fn substitute_runtime_requirement_scalar_values(
    proposition: &mut Proposition,
    substitutions: &BTreeMap<ValueId, ValueDeclaration>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &ScalarTerm,
        substitutions: &BTreeMap<ValueId, ValueDeclaration>,
    ) -> Result<ScalarTerm, LoweringError> {
        match term {
            ScalarTerm::Value { id, scalar_type } => {
                match scalar_type {
                    ScalarType::Boolean => {}
                    ScalarType::Integer(integer_type) if !integer_type.is_address() => {}
                    _ => {
                        return unsupported("runtime requirement value has an unsupported carrier");
                    }
                }
                let actual = substitutions.get(id).ok_or(LoweringError::Unsupported(
                    "runtime requirement scalar formal has no exact actual argument",
                ))?;
                if actual.scalar_type != *scalar_type {
                    return unsupported(
                        "runtime requirement scalar argument changes its formal type",
                    );
                }
                Ok(ScalarTerm::value(actual.id, actual.scalar_type))
            }
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. } => Ok(term.clone()),
            _ => unsupported(
                "runtime requirement scalar substitution encountered an unsupported term",
            ),
        }
    }

    match proposition {
        Proposition::Truth | Proposition::Falsehood => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            let rebound_left = substitute_term(left, substitutions)?;
            let rebound_right = substitute_term(right, substitutions)?;
            *left = rebound_left;
            *right = rebound_right;
            Ok(())
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            for child in children {
                substitute_runtime_requirement_scalar_values(child, substitutions)?;
            }
            Ok(())
        }
        _ => unsupported(
            "runtime requirement scalar substitution encountered an unsupported proposition",
        ),
    }
}
