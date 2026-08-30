use psi_symbols::{SymbolHandle, SymbolKind};

use crate::TypedTrees;
use crate::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{
    OperatorDefinition, declared_domain_constraints_match, normalized_operand_parameters,
    type_reference_matches_with_policy,
};

/// Derive one complete closed type-only application for an operator use from
/// the same operand unification used by spelling resolution. This is D29's
/// first checked rung: const, lifetime, machine, and proposition binders
/// deliberately return `None` until their category-specific identities exist.
pub fn closed_operator_type_application_for_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    if !operator.lifetime_parameters.is_empty() {
        return None;
    }
    let type_parameters = program.operator_type_parameters(operator);
    if type_parameters
        .iter()
        .any(|parameter| !matches!(parameter.kind, crate::data::TypeParameterKind::Type))
    {
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
                    false,
                ) && declared_domain_constraints_match(program, actual, expected.type_reference)
            })
        });
    if !matches {
        return None;
    }
    let application = type_parameters
        .iter()
        .map(|parameter| {
            bindings.iter().find_map(|(symbol, argument)| {
                (*symbol == parameter.symbol).then_some((parameter.symbol, *argument))
            })
        })
        .collect::<Option<Vec<_>>>()?;
    application
        .iter()
        .all(|(_, argument)| closed_boundary_application_type(program, *argument))
        .then_some(application)
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
