use psi_language_semantics::const_value::CanonicalConstIdentity;
use psi_symbols::{SymbolHandle, SymbolKind};

use crate::TypedTrees;
use crate::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{
    OperatorConstBinding, OperatorDefinition, declared_domain_constraints_match,
    normalized_operand_parameters, type_reference_matches_with_policy,
};

/// One declaration-ordered, closed operator telescope argument inferred from
/// the exact operand tuple. Const identity deliberately excludes display text;
/// the independently retained declared carrier is rechecked by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedOperatorApplicationArgument {
    Type {
        binder_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
    Const {
        binder_symbol: SymbolHandle,
        declared_carrier: TypeReferenceHandle,
        value: CanonicalConstIdentity,
    },
}

/// Derive one complete closed type/const application for an operator use from
/// the same operand unification used by spelling resolution. Lifetime,
/// machine, and proposition binders remain fail-closed until their exact
/// category-specific identities exist.
pub fn closed_operator_application_for_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<Vec<ClosedOperatorApplicationArgument>> {
    if !operator.lifetime_parameters.is_empty() {
        return None;
    }
    let type_parameters = program.operator_type_parameters(operator);
    if type_parameters.iter().any(|parameter| {
        !matches!(
            parameter.kind,
            crate::data::TypeParameterKind::Type | crate::data::TypeParameterKind::Const { .. }
        )
    }) {
        return None;
    }
    if type_parameters.is_empty() {
        return Some(Vec::new());
    }
    let parameters = program.operator_parameters(operator);
    if parameters.len() != operand_types.len() {
        return None;
    }
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    let matches = operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .all(|(actual, expected)| {
            actual.is_none_or(|actual| {
                type_reference_matches_with_policy(
                    program,
                    actual,
                    expected.type_reference,
                    None,
                    type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                    false,
                ) && declared_domain_constraints_match(program, actual, expected.type_reference)
            })
        });
    if !matches {
        return None;
    }
    let application = type_parameters
        .iter()
        .map(|parameter| match parameter.kind {
            crate::data::TypeParameterKind::Type => bindings
                .iter()
                .find_map(|(symbol, argument)| {
                    (*symbol == parameter.symbol).then_some(
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol: parameter.symbol,
                            type_reference: *argument,
                        },
                    )
                })
                .filter(|argument| match argument {
                    ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                        closed_boundary_application_type(program, *type_reference)
                    }
                    ClosedOperatorApplicationArgument::Const { .. } => false,
                }),
            crate::data::TypeParameterKind::Const { type_reference } => const_bindings
                .iter()
                .find_map(|OperatorConstBinding { symbol, value }| {
                    (*symbol == parameter.symbol).then_some(
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol: parameter.symbol,
                            declared_carrier: type_reference,
                            value: value.clone(),
                        },
                    )
                })
                .filter(|_| closed_boundary_application_type(program, type_reference)),
            crate::data::TypeParameterKind::Machine { .. }
            | crate::data::TypeParameterKind::Proposition { .. } => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(application)
}

fn closed_boundary_application_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => lifetime.is_none() && closed_boundary_application_type(program, *referee),
        // Constraint expressions and declared-domain arguments need their own
        // exact closedness replay. The first D29 cohort does not erase them.
        TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. } => false,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => closed_boundary_application_type(program, *element_type),
        TypeReferenceNode::FixedArray { .. } => false,
        TypeReferenceNode::Slice { element_type } => {
            closed_boundary_application_type(program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            lifetime_arguments.is_empty()
                && closed_boundary_application_nominal(program, *base_symbol, base_name.as_str())
                && program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .all(|argument| closed_boundary_application_type(program, *argument))
        }
        TypeReferenceNode::Named { symbol, name } => {
            closed_boundary_application_nominal(program, *symbol, name.as_str())
        }
        TypeReferenceNode::Unit => true,
    }
}

fn closed_boundary_application_nominal(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> bool {
    if PrimitiveType::from_name(name).is_some()
        || psi_language_semantics::const_value::CanonicalConstValue::from_atom(name).is_some()
        || name.parse::<i128>().is_ok()
    {
        return true;
    }
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinType | SymbolKind::Data
        )
}
