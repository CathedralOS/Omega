// Syntax policy guards rejecting package-authored toolchain vocabulary and unprojected dependency calls or literals.
// Every authored call has an expression handle, including calls in nested statements.

use super::error::DependencyProjectionError;
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME, constructor_parts};
use crate::declarations::editable_build::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, FROM_SOURCE_HELPER_NAME, WITH_ALIAS_HELPER_NAME,
    imported_dependency_helper, package_authored_type_name,
};
use build_declarations::{BUILD_TYPE_NAME, DependencyOperation};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::Item;

pub(super) fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(syntax_trees, item) {
            Some(name)
                if matches!(
                    machine_leaf_name(name),
                    BUILD_TYPE_NAME
                        | SOURCE_TYPE_NAME
                        | PACKAGE_SELECTION_TYPE_NAME
                        | DEPENDENCY_DESCRIPTION_TYPE_NAME
                        | REGISTRATION_TYPE_NAME
                ) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: machine_leaf_name(name).to_owned(),
                });
            }
            _ => {}
        }
        if let Item::Machine(machine) = item
            && let Some(owner) = machine.attached_data.as_ref()
            && owner.as_str(syntax_trees) == BUILD_TYPE_NAME
            && DependencyOperation::classify(machine_leaf_name(machine.name.as_str(syntax_trees)))
                .is_some()
        {
            return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                name: format!(
                    "{}::{}",
                    owner.as_str(syntax_trees),
                    machine.name.as_str(syntax_trees)
                ),
            });
        }
    }
    Ok(())
}

const REGISTRATION_TYPE_NAME: &str = "DependencyRegistration";

fn machine_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

pub(super) fn reject_unprojected_dependency_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_sources: &[ExpressionHandle],
    accepted_calls: &[ExpressionHandle],
) -> Result<(), DependencyProjectionError> {
    let mut accepted_selections = Vec::new();
    for source_handle in accepted_sources {
        let ExpressionNode::StructLiteral(literal) =
            syntax_trees.expressions.expression(*source_handle)
        else {
            continue;
        };
        for field in syntax_trees.expressions.struct_fields(literal.fields) {
            if field.name.as_str(syntax_trees) == "selection" {
                accepted_selections.push(field.value);
            }
        }
    }


    for (expression_handle, expression) in syntax_trees.expressions.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if constructor_parts(syntax_trees, literal).0 == SOURCE_TYPE_NAME =>
            {
                if !accepted_sources.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::StructLiteral(literal)
                if constructor_parts(syntax_trees, literal).0 == PACKAGE_SELECTION_TYPE_NAME =>
            {
                if !accepted_selections.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::Call(call)
                if ((expression_owner(syntax_trees, call.receiver) == Some(BUILD_TYPE_NAME)
                    && DependencyOperation::classify(call.target.as_str(syntax_trees))
                        .is_some())
                    || (!call.receiver.is_valid()
                        && matches!(
                            imported_dependency_helper(
                                syntax_trees,
                                call.target.as_str(syntax_trees)
                            ),
                            Some(FROM_SOURCE_HELPER_NAME | WITH_ALIAS_HELPER_NAME)
                        )))
                    && !accepted_calls.contains(&expression_handle) =>
            {
                return Err(DependencyProjectionError::UnsupportedDependencyShape);
            }
            _ => {}
        }
    }
    Ok(())
}

// The single-segment static owner of a call expression (`Build` or
// `dependency`), or none for a value receiver or an unqualified call.
fn expression_owner(syntax_trees: &SyntaxTrees, receiver: ExpressionHandle) -> Option<&str> {
    if !receiver.is_valid() {
        return None;
    }
    let ExpressionNode::Name {
        members: path,
        application: path_application,
    } = syntax_trees.expressions.expression(receiver)
    else {
        return None;
    };
    if path_application.is_some() {
        return None;
    }
    match syntax_trees
        .expressions
        .identifier_path_members(path.to_owned())
    {
        [owner] => Some(owner.as_str(syntax_trees)),
        _ => None,
    }
}
