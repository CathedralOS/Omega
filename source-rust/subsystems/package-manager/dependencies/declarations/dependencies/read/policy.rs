// Syntax policy guards rejecting package-authored toolchain vocabulary and unprojected dependency calls or literals.
// `reject_unprojected_dependency_syntax` walks all statement containers (machine states, trait requirement default bodies, conformance members, and capability state signatures) as well as all expressions so registration and description-helper calls, `Source::`, and `PackageSelection::` occurrences outside projected direct rows are rejected.

use super::error::DependencyProjectionError;
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME, constructor_parts};
use crate::declarations::editable_build::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_HELPER_OWNER, is_dependency_call,
    package_authored_type_name,
};
use build_declarations::BUILD_TYPE_NAME;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::Item;
use syntax_trees::statement::{StatementHandle, StatementNode};

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
                        | DEPENDENCY_HELPER_OWNER
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
            && is_dependency_call(
                Some(owner.as_str(syntax_trees)),
                machine_leaf_name(machine.name.as_str(syntax_trees)),
            )
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
    accepted_statements: &[StatementHandle],
    accepted_sources: &[ExpressionHandle],
    accepted_calls: &[Vec<ExpressionHandle>],
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

    for (_, statements) in syntax_trees.items.statement_bodies() {
        reject_dependency_statements(
            syntax_trees,
            syntax_trees.items.statements(statements),
            accepted_statements,
        )?;
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
                if is_dependency_call(
                    expression_owner(syntax_trees, call.receiver),
                    call.target.as_str(syntax_trees),
                ) =>
            {
                let arguments = syntax_trees.expressions.expression_handles(call.arguments);
                if !accepted_calls
                    .iter()
                    .any(|accepted| accepted.as_slice() == arguments)
                {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn reject_dependency_statements(
    syntax_trees: &SyntaxTrees,
    statements: &[StatementHandle],
    accepted_statements: &[StatementHandle],
) -> Result<(), DependencyProjectionError> {
    for statement_handle in statements {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        let owner = match syntax_trees
            .statements
            .identifier_path_members(call.receiver)
        {
            [owner] => Some(owner.as_str(syntax_trees)),
            _ => None,
        };
        if is_dependency_call(owner, call.target.as_str(syntax_trees))
            && !accepted_statements.contains(statement_handle)
        {
            return Err(DependencyProjectionError::UnsupportedDependencyShape);
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
