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
            identity_carrier(program, reference, &mut Vec::new(), 0, false)
        }
    })
}

/// The exact complete data carriers of a formed Id question. This supplies
/// neither operand observations nor equality truth; consumers still owe both.
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

fn identity_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
    depth: usize,
    symbolic_data: bool,
) -> bool {
    if !reference.is_valid() || depth >= 64 {
        return false;
    }
    if let Some(primitive) = program.primitive_type_reference(reference) {
        return !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64);
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            identity_carrier(program, *base_type, active, depth + 1, symbolic_data)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            identity_carrier(program, *element_type, active, depth + 1, symbolic_data)
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
            // field has identity equality. Executable comparisons keep their
            // closed-field rule; Prop Id can retain a symbolic subject without
            // assigning an IEEE or executable equality algorithm to that subject.
            if active.contains(&reference) {
                return true;
            }
            if symbolic_data {
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                    && program
                        .data_type_parameter_by_symbol(*symbol)
                        .is_some_and(|parameter| {
                            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                        })
                {
                    return true;
                }
                if let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = program.type_reference_table.type_reference(reference)
                {
                    let Some(definition) = program.unique_data_definition_by_symbol(*base_symbol)
                    else {
                        return false;
                    };
                    if !program
                        .data_type_parameters(definition)
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*arguments),
                        )
                        .filter(|(parameter, _)| {
                            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                        })
                        .all(|(_, argument)| {
                            identity_carrier(program, *argument, active, depth + 1, true)
                        })
                    {
                        return false;
                    }
                }
            }
            let Some(data) =
                crate::value_custody::places::data_definition_for_type(program, reference)
            else {
                return false;
            };
            active.push(reference);
            let identity = program
                .data_member_fields(program.data_members(data))
                .all(|field| {
                    identity_carrier(
                        program,
                        field.type_reference,
                        active,
                        depth + 1,
                        symbolic_data,
                    )
                });
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
