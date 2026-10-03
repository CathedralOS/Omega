//! Reference-result classification for binding replacement. Literal projections
//! use their declared fields or every possible array element; purity is not
//! evidence that the resulting value is owned rather than a reference.
//! Collection metadata is a scalar observation, not a storage projection:
//! its receiver's reference carrier does not become the result's carrier.

use super::{
    data_definition_for_type, data_field_or_payload_type, declared_place_type_raw,
    unwrapped_type_reference,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
enum Projection<'expression> {
    Field(&'expression str),
    Element,
}

pub(crate) fn expression_result_is_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<bool> {
    let mut pending = vec![(expression, Vec::new())];
    let mut saw_result = false;
    let mut saw_reference = false;
    while let Some((expression, mut projections)) = pending.pop() {
        if projections.is_empty()
            && super::collection_length_receiver(program, machine, Some(state), expression)
                .is_some()
        {
            // Use the shared builtin-meaning query: a nominal field named
            // `len` may itself contain a reference and must keep its type.
            saw_result = true;
            continue;
        }
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Borrow(_)
        ) && let Some(reference) =
            declared_place_type_raw(program, machine, Some(state), expression)
        {
            saw_reference |= projected_type_is_reference(program, reference, &projections)?;
            saw_result = true;
            continue;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                projections.push(Projection::Field(member.member.as_str()));
                pending.push((member.receiver, projections));
            }
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                projections.push(Projection::Element);
                pending.push((indexed.collection, projections));
            }
            ExpressionNode::StructLiteral(literal) if !projections.is_empty() => {
                let Projection::Field(field) = projections.pop()? else {
                    return None;
                };
                let mut definitions = program.data_definitions().iter().filter(|definition| {
                    if literal.type_symbol.is_valid() {
                        definition.symbol == literal.type_symbol
                    } else {
                        definition.name == literal.type_name
                    }
                });
                let definition = definitions.next()?;
                if definitions.next().is_some() {
                    return None;
                }
                let reference = crate::value_custody::struct_literals::construction_field_type(
                    program,
                    definition,
                    literal.case_name.as_ref().map(|case| case.as_str()),
                    field,
                )?;
                saw_reference |= projected_type_is_reference(program, reference, &projections)?;
                saw_result = true;
            }
            ExpressionNode::ArrayLiteral(elements) if !projections.is_empty() => {
                let Projection::Element = projections.pop()? else {
                    return None;
                };
                let elements = program.expression_table.expression_handles(*elements);
                if elements.is_empty() {
                    return None;
                }
                pending.extend(
                    elements
                        .iter()
                        .map(|element| (*element, projections.clone())),
                );
            }
            ExpressionNode::Borrow(borrow) if !projections.is_empty() => {
                pending.push((borrow.target, projections));
            }
            ExpressionNode::Borrow(_) => {
                saw_reference = true;
                saw_result = true;
            }
            ExpressionNode::Cast(cast) => {
                saw_reference |=
                    projected_type_is_reference(program, cast.target_type, &projections)?;
                saw_result = true;
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::ArrayLiteral(_)
                if projections.is_empty() =>
            {
                saw_result = true;
            }
            _ => return None,
        }
    }
    saw_result.then_some(saw_reference)
}

fn projected_type_is_reference(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    projections: &[Projection<'_>],
) -> Option<bool> {
    for projection in projections.iter().rev() {
        reference = unwrapped_type_reference(program, reference)?;
        reference = match projection {
            Projection::Field(field) => data_field_or_payload_type(
                program,
                data_definition_for_type(program, reference)?,
                field,
            )?,
            Projection::Element => match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => *element_type,
                _ => return None,
            },
        };
    }
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference.is_valid().then(|| {
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::expression_result_is_reference;
    use typed_trees::statement::StatementNode;

    #[test]
    fn collection_metadata_does_not_erase_nominal_reference_fields() {
        let program = crate::front_end::typed_program(
            "data Holder { len: &u64; items: &[u32]; }
             machine inspect(items: &[u32], fixed: &[u32; 3], holder: &Holder) {
                 let slice_length: u64 = items.len;
                 let array_length: u64 = fixed.len;
                 let field_length: u64 = holder.items.len;
                 let nominal_reference: &u64 = holder.len;
             }",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let classifications = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                Some(expression_result_is_reference(
                    &program,
                    machine,
                    state,
                    local.initial_value,
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            classifications,
            [Some(false), Some(false), Some(false), Some(true)]
        );
    }
}
