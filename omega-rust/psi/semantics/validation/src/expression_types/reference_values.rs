//! Declared member value types, without manufacturing a loan or storage origin.

use super::*;
use language_semantics::ReferenceAccess;

pub(super) fn member_matches_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
    required: TypeReferenceHandle,
) -> bool {
    let Some(actual) = declared_value_type(program, expression) else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access: required_access,
        referee: required_referee,
        ..
    } = program.type_reference_table.type_reference(required)
    else {
        return false;
    };
    if let TypeReferenceNode::Reference {
        access: actual_access,
        referee: actual_referee,
        ..
    } = program.type_reference_table.type_reference(actual)
    {
        return program.normalized_type_identity(actual)
            == program.normalized_type_identity(required)
            || (*actual_access == ReferenceAccess::Mutable
                && *required_access == ReferenceAccess::Shared
                && program.normalized_type_identity(*actual_referee)
                    == program.normalized_type_identity(*required_referee));
    }
    // The existing implicit shared borrow of an owned field still needs that
    // field's actual type. Member syntax alone cannot match an arbitrary referee.
    *required_access == ReferenceAccess::Shared
        && program.normalized_type_identity(actual)
            == program.normalized_type_identity(*required_referee)
}

fn declared_value_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let [spelling] = program.expression_table.name_path_members(name.members) else {
                return None;
            };
            (name.symbol.is_valid()
                && name.symbol == name.head_symbol
                && program.symbols.name(name.symbol) == spelling.as_str())
            .then(|| named_value_type_reference(program, name))?
        }
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call),
        ExpressionNode::Member(member) => {
            if let ExpressionNode::Name(root) = program.expression_table.expression(member.receiver)
                && let Some(machine) = program.machines().iter().find(|machine| {
                    root.head_symbol.is_valid() && machine.symbol == root.head_symbol
                })
            {
                return crate::places::exact_self_field(program, machine, expression)
                    .map(|field| field.type_reference);
            }
            let receiver = declared_value_type(program, member.receiver)?;
            let receiver = crate::places::unwrapped_type_reference(program, receiver)?;
            let symbol = match program.type_reference_table.type_reference(receiver) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if arguments.is_empty() => *base_symbol,
                _ => return None,
            };
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| symbol.is_valid() && definition.symbol == symbol);
            let definition = definitions.next()?;
            if definitions.next().is_some() || !program.data_type_parameters(definition).is_empty()
            {
                return None;
            }
            crate::places::exact_data_member_field(
                program,
                definition,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            )
            .map(|field| field.type_reference)
        }
        _ => None,
    }
}
