//! Unit entry requirements over exact scalar and structural parameters.
//!
//! These predicates describe invocation-time observations, including fields
//! reached through mutable parameters. They do not assert that every body store
//! preserves the entry facts: consumption and return obligations own invariant
//! restoration. Validate the entire entry namespace before simplifying Boolean
//! expressions, then use the shared contract converter for IEEE comparisons so
//! unordered negation and exact binary32/binary64 formats survive publication.
use super::{
    BTreeMap, CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
    Proposition, ScalarTerm, ScalarType, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, ValueDeclaration, ValueId, integer_landing_scalar_type, integer_scalar_type,
    integer_value, terminal_scalar_type, unsupported,
};
use crate::proofs::contract_predicates::PredicateTerms;
use crate::proofs::crash_routes::{
    checked_boolean_scalar_term, checked_scalar_term, checked_scalar_term_with_views,
    lower_structural_member_term,
};

mod source;
pub(crate) use source::validate_graph_parameter_ranges;
pub(crate) use source::{validate_runtime_memberships, validate_scalar_source};

#[cfg(test)]
mod tests;

pub(crate) fn lower_structural_runtime_requirement(
    expression: &CheckedBooleanExpression,
    scalar_parameters: &[ValueDeclaration],
    erased: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Proposition, LoweringError> {
    // Validate the complete entry namespace before Boolean simplification.
    validate_namespace(expression)?;
    crate::proofs::contract_predicates::proposition(
        expression,
        &RuntimeRequirementTerms {
            scalar_parameters,
            erased,
            parameters,
            structural_types,
            view_roster: parameters
                .iter()
                .map(|parameter| (parameter.position, parameter.clone()))
                .collect(),
            element_views: crate::expression_preparation::bindings::element_views(
                &parameters
                    .iter()
                    .map(|parameter| (parameter.position, parameter.clone()))
                    .collect::<Vec<_>>(),
                structural_types,
            ),
        },
    )
}

fn validate_namespace(expression: &CheckedBooleanExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. } => Ok(()),
        CheckedBooleanExpression::Not(operand) => validate_namespace(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            validate_namespace(left)?;
            validate_namespace(right)
        }
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            crate::proofs::contract_predicates::validate_math_namespace(
                left,
                &validate_integer_namespace,
            )?;
            crate::proofs::contract_predicates::validate_math_namespace(
                right,
                &validate_integer_namespace,
            )
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            validate_integer_namespace(left)?;
            validate_integer_namespace(right)
        }
        CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            validate_float_namespace(left)?;
            validate_float_namespace(right)
        }
        _ => unsupported("runtime requirement is outside the immutable entry parameter namespace"),
    }
}

fn validate_float_namespace(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::ErasedParameter { primitive_type, .. }
        | CheckedScalarExpression::StructuralParameterField { primitive_type, .. }
            if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) =>
        {
            Ok(())
        }
        CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
        // Closed format bits and entry leaves need no operation-safety premise.
        // Body reads and computations cannot borrow the clause being established.
        _ => unsupported("runtime IEEE requirement needs exact float entry parameters or literals"),
    }
}

fn validate_integer_namespace(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::Parameter { primitive_type, .. }
        | CheckedScalarExpression::ErasedParameter { primitive_type, .. }
        | CheckedScalarExpression::StructuralParameterField { primitive_type, .. } => {
            if *primitive_type == PrimitiveType::Addr {
                return unsupported("runtime requirement cannot use an address carrier");
            }
            integer_scalar_type(*primitive_type).map(|_| ())
        }
        CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
        // These operations are total on their fixed carriers. Partial
        // arithmetic cannot assume its own entry clause to justify itself.
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } if matches!(
            kind,
            checked_trees::CheckedIntegerBinaryKind::WrappingAdd
                | checked_trees::CheckedIntegerBinaryKind::WrappingSubtract
                | checked_trees::CheckedIntegerBinaryKind::WrappingMultiply
                | checked_trees::CheckedIntegerBinaryKind::SaturatingAdd
                | checked_trees::CheckedIntegerBinaryKind::SaturatingSubtract
                | checked_trees::CheckedIntegerBinaryKind::SaturatingMultiply
                | checked_trees::CheckedIntegerBinaryKind::BitwiseAnd
                | checked_trees::CheckedIntegerBinaryKind::BitwiseOr
                | checked_trees::CheckedIntegerBinaryKind::BitwiseXor
                | checked_trees::CheckedIntegerBinaryKind::WrappingShiftLeft
                | checked_trees::CheckedIntegerBinaryKind::WrappingShiftRight
        ) && *primitive_type != PrimitiveType::Addr =>
        {
            integer_scalar_type(*primitive_type)?;
            validate_integer_namespace(left)?;
            validate_integer_namespace(right)
        }
        // Other fixed-width arithmetic over invocation operands is the same
        // namespace the scalar-graph entry roster admits: `self.length - 1`
        // stays a contract term, and the term lowering below still fails
        // closed on kinds the checked requirement roster cannot produce.
        CheckedScalarExpression::IntegerBinary {
            primitive_type,
            left,
            right,
            ..
        } if *primitive_type != PrimitiveType::Addr => {
            integer_scalar_type(*primitive_type)?;
            validate_integer_namespace(left)?;
            validate_integer_namespace(right)
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        }
        | CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
            ..
        }
        | CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } if *primitive_type != PrimitiveType::Addr => {
            integer_scalar_type(*primitive_type)?;
            validate_integer_namespace(operand)
        }
        CheckedScalarExpression::Boolean(expression) => validate_namespace(expression),
        // Whole-view extents carry the caller-proven bound the boundary
        // argument's domain needs; nested member extents keep the historical
        // fail-closed lane.
        CheckedScalarExpression::StructuralParameterByteLength { path, .. } if path.is_empty() => {
            Ok(())
        }
        _ => unsupported(
            "runtime requirements admit only fixed integer parameters, members, and literals",
        ),
    }
}

struct RuntimeRequirementTerms<'parameters> {
    scalar_parameters: &'parameters [ValueDeclaration],
    erased: &'parameters [ValueDeclaration],
    parameters: &'parameters [StructuralParameterDeclaration],
    structural_types: &'parameters [StructuralTypeDeclaration],
    view_roster: Vec<(u32, StructuralParameterDeclaration)>,
    element_views: BTreeMap<StructuralTypeId, Option<ScalarType>>,
}

impl RuntimeRequirementTerms<'_> {
    fn scalar_leaf(
        &self,
        expression: &CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedScalarExpression::Parameter { primitive_type, .. }
            | CheckedScalarExpression::ErasedParameter { primitive_type, .. } => {
                if !matches!(
                    primitive_type,
                    typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64
                ) {
                    integer_scalar_type(*primitive_type)?;
                }
                checked_scalar_term(expression, self.scalar_parameters, self.erased)
            }
            CheckedScalarExpression::IeeeFloatLiteral { value } => {
                Ok(ScalarTerm::IeeeFloatLiteral(*value))
            }
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                terminal_scalar_type(*primitive_type)?,
                self.parameters,
                self.structural_types,
            ),
            CheckedScalarExpression::StructuralParameterByteLength { .. } => {
                checked_scalar_term_with_views(
                    expression,
                    self.scalar_parameters,
                    self.erased,
                    &self.view_roster,
                    &self.element_views,
                )
            }
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
}

impl PredicateTerms for RuntimeRequirementTerms<'_> {
    fn scalar(&self, expression: &CheckedScalarExpression) -> Result<ScalarTerm, LoweringError> {
        crate::proofs::integer_predicates::lower(expression, &[], &|leaf| self.scalar_leaf(leaf))
    }

    fn boolean(&self, expression: &CheckedBooleanExpression) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedBooleanExpression::Not(operand) => {
                ScalarTerm::boolean_not(self.boolean(operand)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Equal { left, right } => {
                ScalarTerm::boolean_equal(self.boolean(left)?, self.boolean(right)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::ErasedParameter { .. } => {
                checked_boolean_scalar_term(expression, self.scalar_parameters, self.erased)
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
pub(crate) fn substitute_runtime_requirement_scalar_values(
    proposition: &mut Proposition,
    substitutions: &BTreeMap<ValueId, ValueDeclaration>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &ScalarTerm,
        substitutions: &BTreeMap<ValueId, ValueDeclaration>,
    ) -> Result<ScalarTerm, LoweringError> {
        fn rebind(
            term: &mut ScalarTerm,
            substitutions: &BTreeMap<ValueId, ValueDeclaration>,
        ) -> Result<(), LoweringError> {
            match term {
                ScalarTerm::BooleanNot { operand }
                | ScalarTerm::IntegerBitwiseNot { operand, .. } => rebind(operand, substitutions),
                ScalarTerm::BooleanEqual { left, right }
                | ScalarTerm::IntegerEqual { left, right, .. }
                | ScalarTerm::IntegerLessThan { left, right, .. }
                | ScalarTerm::IntegerLessOrEqual { left, right, .. }
                | ScalarTerm::WrappingIntegerAdd { left, right, .. }
                | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
                | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
                | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
                | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
                | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
                | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
                | ScalarTerm::IntegerBitwiseOr { left, right, .. }
                | ScalarTerm::IntegerBitwiseXor { left, right, .. }
                | ScalarTerm::WrappingIntegerShiftLeft {
                    value: left,
                    count: right,
                    ..
                }
                | ScalarTerm::WrappingIntegerShiftRight {
                    value: left,
                    count: right,
                    ..
                } => {
                    rebind(left, substitutions)?;
                    rebind(right, substitutions)
                }
                ScalarTerm::Value { id, scalar_type } => {
                    match scalar_type {
                        ScalarType::Boolean | ScalarType::IeeeFloat(_) => {}
                        ScalarType::Integer(integer_type) if !integer_type.is_address() => {}
                        _ => {
                            return unsupported(
                                "runtime requirement value has an unsupported carrier",
                            );
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
                    *id = actual.id;
                    Ok(())
                }
                ScalarTerm::Boolean(_)
                | ScalarTerm::Integer { .. }
                | ScalarTerm::IeeeFloatLiteral(_)
                | ScalarTerm::BooleanField { .. }
                | ScalarTerm::IntegerField { .. }
                | ScalarTerm::IeeeFloatField { .. }
                | ScalarTerm::ViewExtent { .. }
                | ScalarTerm::ByteSequencePredicate { .. } => Ok(()),
                _ => unsupported(
                    "runtime requirement scalar substitution encountered an unsupported term",
                ),
            }
        }
        // Preserve the operation tree and its witnesses; only formal identities
        // change. Validate before and after so a forged carrier witness or an
        // invalid actual cannot hide inside a successfully rebound expression.
        term.validate()
            .map_err(LoweringError::InvalidCrashPredicate)?;
        let mut rebound = term.clone();
        rebind(&mut rebound, substitutions)?;
        rebound
            .validate()
            .map_err(LoweringError::InvalidCrashPredicate)?;
        Ok(rebound)
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
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            *proposition = crate::proofs::contract_predicates::ieee_float_proposition(
                *kind,
                substitute_term(left, substitutions)?,
                substitute_term(right, substitutions)?,
            )?;
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
