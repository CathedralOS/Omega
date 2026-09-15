use super::error::DependencyProjectionError;
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME, constructor_parts};
use build_declarations::{DependencyOperation, is_dependency_call_name};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::Item;
use syntax_trees::statement::{StatementHandle, StatementNode};

const BUILD_TYPE_NAME: &str = "Build";

pub(super) fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        match package_authored_type_name(item) {
            Some(name)
                if matches!(
                    machine_leaf_name(name),
                    BUILD_TYPE_NAME | SOURCE_TYPE_NAME | PACKAGE_SELECTION_TYPE_NAME
                ) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: machine_leaf_name(name).to_owned(),
                });
            }
            _ => {}
        }
        match item {
            Item::Machine(machine)
                if machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str() == BUILD_TYPE_NAME)
                    && is_dependency_call_name(machine_leaf_name(machine.name.as_str())) =>
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: format!("Build::{}", machine.name.as_str()),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn package_authored_type_name(item: &Item) -> Option<&str> {
    match item {
        Item::Data(data) => Some(data.name.as_str()),
        Item::Domain(domain) => Some(domain.name.as_str()),
        Item::Trait(definition) => Some(definition.name.as_str()),
        _ => None,
    }
}

fn machine_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

pub(super) fn reject_unprojected_dependency_syntax(
    syntax_trees: &SyntaxTrees,
    accepted_statements: &[StatementHandle],
    accepted_sources: &[ExpressionHandle],
    accepted_aliases: &[ExpressionHandle],
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        for state_handle in syntax_trees.items.state_handles(machine.states) {
            let state = syntax_trees.items.state(*state_handle);
            for statement_handle in syntax_trees.items.statements(state.statements) {
                let StatementNode::Call(call) =
                    syntax_trees.statements.statement(*statement_handle)
                else {
                    continue;
                };
                if is_dependency_call_name(call.target.as_str())
                    && !accepted_statements.contains(statement_handle)
                {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
        }
    }

    for (expression_handle, expression) in syntax_trees.expressions.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if constructor_parts(literal).0 == SOURCE_TYPE_NAME =>
            {
                if !accepted_sources.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::Call(call) if is_dependency_call_name(call.target.as_str()) => {
                match (
                    DependencyOperation::classify(call.target.as_str()),
                    syntax_trees.expressions.expression_handles(call.arguments),
                ) {
                    (Some(operation), [source])
                        if !operation.takes_alias() && accepted_sources.contains(source) => {}
                    (Some(operation), [alias, source])
                        if operation.takes_alias()
                            && accepted_aliases.contains(alias)
                            && accepted_sources.contains(source) => {}
                    _ => return Err(DependencyProjectionError::UnsupportedDependencyShape),
                }
            }
            _ => {}
        }
    }
    Ok(())
}
