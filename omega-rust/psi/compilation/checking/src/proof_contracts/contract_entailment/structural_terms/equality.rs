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
    types.iter().all(|reference| {
        reference.is_some_and(|reference| identity_carrier(program, reference, &mut Vec::new(), 0))
    }) && typed_trees::operator::has_builtin_spelled_expression_meaning(
        program, scope, expression, spelling, &types,
    )
}

fn identity_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
    depth: usize,
) -> bool {
    if !reference.is_valid() || depth >= 64 {
        return false;
    }
    if let Some(primitive) = program.primitive_type_reference(reference) {
        return !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64);
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            identity_carrier(program, *base_type, active, depth + 1)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            identity_carrier(program, *element_type, active, depth + 1)
        }
        TypeReferenceNode::Named { symbol, .. }
            if matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(symbols::BuiltinTypeAtom::Int | symbols::BuiltinTypeAtom::UInt)
            ) =>
        {
            true
        }
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            // Recursive data remains structural provided every nonrecursive
            // field has identity equality. Open fields fail rather than
            // pretending an unconstrained type argument cannot become IEEE.
            if active.contains(&reference) {
                return true;
            }
            let Some(data) =
                crate::value_custody::places::data_definition_for_type(program, reference)
            else {
                return false;
            };
            active.push(reference);
            let identity = program
                .data_member_fields(program.data_members(data))
                .all(|field| identity_carrier(program, field.type_reference, active, depth + 1));
            active.pop();
            identity
        }
        TypeReferenceNode::Unit => true,
        // A referent/content comparison does not equate descriptor identity.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. } => false,
    }
}

/// `fact`'s operand pair when it is a binary `==` carrying identity meaning
/// under `scope`'s parameters. Case observations and every other fact shape
/// yield `None` so each citation walk applies its own exit.
pub(in crate::proof_contracts::contract_entailment) fn identity_equality_terms(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    parameters: &[StateParameter],
    fact: ExpressionHandle,
) -> Option<(StructuralTerm, StructuralTerm)> {
    if super::is_case_observation(program, fact) {
        return None;
    }
    let binary = program.expression_table.binary_node(fact)?;
    if binary.operator != BinaryOperator::Equal
        || !has_identity_equality_meaning(program, scope, parameters, fact)
    {
        return None;
    }
    super::structural_term_pair_for_parameters(program, scope, parameters, binary)
}
