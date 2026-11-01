//! Alias-formal occurrences substitute supplying expressions before resolution.
//! No operator runs here. The original alias application still owns argument
//! carrier/formation admission, and ordinary checking admits the copied RHS.

use super::{Diagnostic, SyntaxTrees, TypeAliasDefinition, TypeParameterKind, TypeReferenceHandle};
use crate::preparation::generic_data::{collect_expression_handles, visit_expression_type_inputs};
use arena::HandleSpan;
use std::collections::{HashMap, HashSet};
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::types::TypeReferenceNode;

pub(super) fn bind_const_expression(
    templates: &SyntaxTrees,
    root: ExpressionHandle,
    alias: &TypeAliasDefinition,
    arguments: &[TypeReferenceHandle],
    uses: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
    unresolved: &mut Vec<ExpressionHandle>,
) -> Vec<TypeReferenceHandle> {
    let mut expressions = HashSet::new();
    collect_expression_handles(templates, root, &mut expressions);
    let proofs = super::static_operands::proof_bindings(templates, &expressions);
    let mut expressions = expressions.into_iter().collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    let parameters = templates.items.type_parameters(alias.type_parameters);
    let mut children = Vec::new();
    for expression in expressions {
        visit_expression_type_inputs(templates, expression, &mut |reference| {
            children.push(reference)
        });
        match templates.expressions.expression(expression) {
            ExpressionNode::Name(path) => {
                let [name] = templates.expressions.identifier_path_members(*path) else {
                    unresolved.push(expression);
                    continue;
                };
                let shadowed = proofs.get(&expression).is_some_and(|bindings| {
                    bindings
                        .iter()
                        .any(|binding| binding.as_str() == name.as_str(templates))
                });
                let parameter = (!shadowed)
                    .then(|| {
                        parameters.iter().position(|parameter| {
                            matches!(&parameter.kind, TypeParameterKind::Const { .. })
                                && parameter.name.as_str(templates) == name.as_str(templates)
                        })
                    })
                    .flatten();
                if let Some(position) = parameter {
                    uses.push((expression, arguments[position]));
                } else {
                    unresolved.push(expression);
                }
            }
            // A declaration-global name or call needs its own retained scope;
            // a caller's same-spelled declaration is never a substitute.
            ExpressionNode::Call(_) => unresolved.push(expression),
            _ => {}
        }
    }
    children
}

pub(super) fn supplied_expressions(
    syntax: &mut SyntaxTrees,
    uses: &[(ExpressionHandle, TypeReferenceHandle)],
) -> Result<Vec<(ExpressionHandle, ExpressionHandle)>, Diagnostic> {
    let mut actuals: HashMap<TypeReferenceHandle, ExpressionHandle> = HashMap::new();
    let mut substitutions = Vec::with_capacity(uses.len());
    for &(source, argument) in uses {
        let actual = if let Some(actual) = actuals.get(&argument) {
            *actual
        } else {
            let owner = syntax
                .type_references
                .supplied_operand_owner(argument)
                .map_err(Diagnostic::error)?;
            if owner.is_valid() {
                return Err(Diagnostic::error(
                    "transparent alias RHS expression cannot yet preserve explicitly supplied declaration custody",
                ));
            }
            let expression = match syntax.type_references.type_reference(argument).clone() {
                TypeReferenceNode::ConstExpression(expression) => expression,
                TypeReferenceNode::Named(name) => {
                    let span = name.source_span();
                    let members = name
                        .as_str(syntax)
                        .split("::")
                        .map(|member| Identifier::new(member, span))
                        .collect::<Vec<_>>();
                    let mut path = HandleSpan::empty();
                    for member in members {
                        syntax
                            .expressions
                            .append_identifier_path_member_to_span(&mut path, member);
                    }
                    let expression = syntax.expressions.insert(ExpressionNode::Name(path));
                    syntax.expressions.set_source_span(expression, span);
                    expression
                }
                _ => {
                    return Err(Diagnostic::error(
                        "transparent alias Const expression requires a value argument",
                    ));
                }
            };
            actuals.insert(argument, expression);
            expression
        };
        substitutions.push((source, actual));
    }
    Ok(substitutions)
}
