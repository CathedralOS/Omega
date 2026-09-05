use super::static_arguments::capture as static_arguments;
use super::*;
use psi_typed_trees::expression::{ExpressionNode, TableStructLiteralField};
use psi_typed_trees::name::Identifier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    handle: ExpressionHandle,
    node: ExpressionNode,
    arguments: Vec<ExpressionHandle>,
    names: Vec<Identifier>,
    members: Vec<SymbolHandle>,
    fields: Vec<TableStructLiteralField>,
    type_arguments: Vec<TypeReferenceHandle>,
}

pub(super) fn capture(
    builder: &mut Builder<'_>,
    handle: ExpressionHandle,
) -> Result<(), Vec<Diagnostic>> {
    let program = builder.program;
    let table = &program.expression_table;
    if !table.expression_is_valid(handle) {
        return Err(rejected("a stale nonzero expression handle"));
    }
    let node = table.expression(handle);
    let mut arguments = Vec::new();
    let mut names = Vec::new();
    let mut members = Vec::new();
    let mut fields = Vec::new();
    let mut type_arguments = Vec::new();
    match node {
        ExpressionNode::ArrayLiteral(span) => {
            builder.charge(span.count() as usize)?;
            arguments.extend_from_slice(table.expression_handles(*span));
        }
        ExpressionNode::Atomic(value) => {
            arguments.extend([value.value, value.result]);
        }
        ExpressionNode::Binary(value) => {
            arguments.extend([value.left, value.right]);
        }
        ExpressionNode::Cast(value) => {
            arguments.push(value.value);
            builder.type_reference(value.target_type)?;
            builder.charge(
                (value.target_label.count() as usize)
                    .saturating_add(value.semantic_domain.count() as usize)
                    .saturating_add(value.semantic_domain_arguments.count() as usize),
            )?;
            names.extend_from_slice(table.name_path_members(value.target_label));
            names.extend_from_slice(table.name_path_members(value.semantic_domain));
            type_arguments.extend_from_slice(
                program
                    .type_reference_table
                    .type_reference_handles(value.semantic_domain_arguments),
            );
            builder.symbol(value.semantic_domain_symbol)?;
        }
        ExpressionNode::Call(value) => {
            builder.charge(value.arguments.count() as usize)?;
            arguments.push(value.receiver);
            arguments.extend_from_slice(table.expression_handles(value.arguments));
            builder.symbol(value.target_symbol)?;
            static_arguments(builder, &value.machine_arguments)?;
            if let Some(dispatch) = &value.static_requirement_dispatch {
                for symbol in [
                    dispatch.declaring_trait,
                    dispatch.requirement,
                    dispatch.realization_machine,
                    dispatch.realization_state,
                ] {
                    builder.symbol(symbol)?;
                }
            }
            if let Some(request) = &value.quotient_operation {
                static_arguments(
                    builder,
                    std::slice::from_ref(&request.representative_operation),
                )?;
                builder.charge(request.theorem_evidence.len())?;
                for theorem in &request.theorem_evidence {
                    static_arguments(builder, std::slice::from_ref(&theorem.application))?;
                }
            }
            if let Some(request) = &value.private_layout_operation {
                static_arguments(builder, std::slice::from_ref(&request.selected_slot))?;
            }
        }
        ExpressionNode::Indexed(value) => arguments.extend([value.collection, value.index]),
        ExpressionNode::Member(value) => {
            arguments.push(value.receiver);
            builder.symbol(value.member_symbol)?;
        }
        ExpressionNode::Borrow(value) => arguments.push(value.target),
        ExpressionNode::Name(value) => {
            builder.charge(
                (value.members.count() as usize)
                    .saturating_add(value.member_symbols.count() as usize),
            )?;
            names.extend_from_slice(table.name_path_members(value.members));
            members.extend_from_slice(table.name_path_member_symbols(value.member_symbols));
            builder.symbol(value.head_symbol)?;
            builder.symbol(value.symbol)?;
        }
        ExpressionNode::Range(value) => arguments.extend([value.start, value.end]),
        ExpressionNode::StructLiteral(value) => {
            builder.charge(value.fields.count() as usize)?;
            fields.extend_from_slice(table.struct_fields(value.fields));
            builder.symbol(value.type_symbol)?;
            if let Some(symbol) = value.case_symbol {
                builder.symbol(symbol)?;
            }
            for field in &fields {
                builder.symbol(field.field_symbol)?;
                arguments.push(field.value);
            }
        }
        ExpressionNode::Unary(value) => arguments.push(value.operand),
        ExpressionNode::ZeroValue(reference) => builder.type_reference(*reference)?,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
    for argument in &arguments {
        builder.expression(*argument)?;
    }
    for symbol in &members {
        builder.symbol(*symbol)?;
    }
    for reference in &type_arguments {
        builder.type_reference(*reference)?;
    }
    builder.result.expressions.push(Snapshot {
        handle,
        node: node.clone(),
        arguments,
        names,
        members,
        fields,
        type_arguments,
    });
    Ok(())
}
