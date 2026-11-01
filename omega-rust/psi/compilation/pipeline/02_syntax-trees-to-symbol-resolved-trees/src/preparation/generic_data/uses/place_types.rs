//! Declared destination types for contextual constructor formation. Follow an
//! actual lexical root through its declaration roster; no value is evaluated,
//! and no storage, tag, bounds or write authority is established here.
use super::super::{
    DataMember, ExpressionHandle, ExpressionNode, Identifier, SyntaxTrees, TypeReferenceHandle,
    TypeReferenceNode,
};
use super::{ConstructorFrontier, expected_instance};
use crate::preparation::{
    declaration_selection,
    generic_data::{Instantiation, selected_data_item},
};
use syntax_trees::statement::StatementNode;

enum Selector {
    Field(Identifier),
    Element,
}

pub(super) fn declared_destination(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    frontier: &ConstructorFrontier<'_>,
    instances: &[Instantiation],
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Option<TypeReferenceHandle> {
    let mut current = expression;
    let mut selectors = Vec::new();
    let mut reference = loop {
        match syntax.expressions.expression(current) {
            ExpressionNode::Member(member) => {
                selectors.push(Selector::Field(member.member.clone()));
                current = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    syntax.expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                selectors.push(Selector::Element);
                current = indexed.collection;
            }
            ExpressionNode::Name(path) => {
                let (head, tail) = syntax
                    .expressions
                    .identifier_path_members(*path)
                    .split_first()?;
                for field in tail.iter().rev() {
                    selectors.push(Selector::Field(field.clone()));
                }
                let local = frontier
                    .prior_statements
                    .iter()
                    .rev()
                    .find_map(|statement| {
                        let StatementNode::LocalData(local) =
                            syntax.statements.statement(*statement)
                        else {
                            return None;
                        };
                        (local.name.as_str(syntax) == head.as_str(syntax))
                            .then_some(local.type_reference)
                    });
                break local.or_else(|| {
                    syntax
                        .items
                        .state_parameters(frontier.parameters)
                        .iter()
                        .find_map(|parameter| {
                            let parameter = syntax.items.state_parameter(*parameter);
                            (parameter.name.as_str(syntax) == head.as_str(syntax))
                                .then_some(parameter.type_reference)
                        })
                })?;
            }
            _ => return None,
        }
    };
    for selector in selectors.into_iter().rev() {
        reference = referent(syntax, reference)?;
        reference = match selector {
            Selector::Element => match syntax.type_references.type_reference(reference) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => *element_type,
                _ => return None,
            },
            Selector::Field(name) => {
                let owner = if let Some(instance) =
                    expected_instance(syntax, instances, selection, reference)
                {
                    instance.declaration
                } else {
                    let name = syntax
                        .type_references
                        .type_reference(reference)
                        .as_named()?;
                    selected_data_item(syntax, selection, name)?
                };
                let data = syntax.root_item(owner).as_data()?;
                let mut fields = Vec::new();
                for member in syntax.items.data_members(data.members) {
                    match member {
                        DataMember::Field(field)
                            if field.name.as_str(syntax) == name.as_str(syntax) =>
                        {
                            fields.push(field.type_reference)
                        }
                        DataMember::Variant(case) => fields.extend(
                            syntax
                                .items
                                .data_payload_fields(case.payload)
                                .iter()
                                .filter(|field| field.name.as_str(syntax) == name.as_str(syntax))
                                .map(|field| field.type_reference),
                        ),
                        _ => {}
                    }
                }
                let first = *fields.first()?;
                if fields.iter().any(|field| *field != first) {
                    return None;
                }
                first
            }
        };
    }
    Some(reference)
}

fn referent(
    syntax: &SyntaxTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    for _ in 0..=syntax.type_references.type_reference_count() {
        if !reference.is_valid() {
            return None;
        }
        reference = match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return Some(reference),
        };
    }
    None
}
