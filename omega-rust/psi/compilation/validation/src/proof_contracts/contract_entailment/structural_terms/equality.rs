//! Equality authority for structural substitution and reflexivity.
//!
//! The selected builtin comparison must compare complete values with identity
//! semantics. IEEE equality, including an IEEE leaf inside a compound value,
//! cannot supply that authority: signed zeros compare equal and NaN does not.
//! A declaring telescope supplies operand types before any actual substitution.

use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::StructuralTerm;

pub(in crate::proof_contracts::contract_entailment) fn has_identity_equality_meaning(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    let spelling = match binary.operator {
        BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
        BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
        _ => return false,
    };
    let types = [binary.left, binary.right].map(|operand| {
        crate::parameter_expression_result_type_reference(program, scope, parameters, operand)
    });
    // Select with complete declaration types before interpreting a builtin
    // scalar read. A reference overload still sees its descriptor type;
    // readable scalar contents do not authorize aggregate descriptor identity.
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program, scope, expression, spelling, &types,
    ) {
        return false;
    }
    let Some(types) = crate::value_custody::expression_types::builtin_scalar_operand_references(
        program, binary, types, None,
    ) else {
        return false;
    };
    types.iter().all(|reference| {
        let reference = *reference;
        if let Some((primitive, _)) =
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
        {
            !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
        } else {
            matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if matches!(program.symbols.builtin_type_atom(*symbol),
                    Some(symbols::BuiltinTypeAtom::Int | symbols::BuiltinTypeAtom::UInt)))
        }
    })
}

/// Complete builtin scalar carriers whose selected comparison can imply Id.
/// This supplies neither operand observations nor comparison truth.
pub(in crate::proof_contracts::contract_entailment) fn builtin_identity_operand_references(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<[TypeReferenceHandle; 2]> {
    let binary = program.expression_table.binary_node(expression)?;
    let spelling = match binary.operator {
        BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
        BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
        _ => return None,
    };
    let types = [binary.left, binary.right].map(|operand| {
        crate::parameter_expression_result_type_reference(program, scope, parameters, operand)
    });
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program, scope, expression, spelling, &types,
    ) {
        return None;
    }
    let references = crate::value_custody::expression_types::builtin_scalar_operand_references(
        program, binary, types, None,
    )?;
    // Only specified, same-carrier owned primitive comparisons imply Id.
    // Aggregate/custom comparisons require separate checked bridge evidence;
    // reading equal referent contents never identifies their descriptors.
    if references.iter().any(|reference| {
        crate::reference_shell(program, *reference).is_some()
            || !program
                .primitive_type_reference(*reference)
                .is_some_and(|primitive| {
                    primitive.is_fixed_integer()
                        || matches!(primitive, PrimitiveType::Bool | PrimitiveType::Addr)
                })
    }) || program.normalized_type_identity(references[0])
        != program.normalized_type_identity(references[1])
    {
        return None;
    }
    Some(references)
}
