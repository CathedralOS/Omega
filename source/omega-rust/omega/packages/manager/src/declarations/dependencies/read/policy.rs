use super::error::DependencyProjectionError;
use super::projection::{
    DEPEND_AS_MACHINE_NAME, DEPEND_AS_WHEN_MACHINE_NAME, DEPEND_MACHINE_NAME,
    DEPEND_WHEN_MACHINE_NAME,
};
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::item::Item;
use psi_syntax_trees::statement::{StatementHandle, StatementNode};

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
                    && matches!(
                        machine_leaf_name(machine.name.as_str()),
                        DEPEND_MACHINE_NAME
                            | DEPEND_AS_MACHINE_NAME
                            | DEPEND_WHEN_MACHINE_NAME
                            | DEPEND_AS_WHEN_MACHINE_NAME
                    ) =>
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
        Item::WireData(wire) => Some(wire.name.as_str()),
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
                if matches!(
                    call.target.as_str(),
                    DEPEND_MACHINE_NAME
                        | DEPEND_AS_MACHINE_NAME
                        | DEPEND_WHEN_MACHINE_NAME
                        | DEPEND_AS_WHEN_MACHINE_NAME
                ) && !accepted_statements.contains(statement_handle)
                {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
        }
    }

    for (expression_handle, expression) in syntax_trees.expressions.iter_expressions() {
        match expression {
            ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == SOURCE_TYPE_NAME =>
            {
                if !accepted_sources.contains(&expression_handle) {
                    return Err(DependencyProjectionError::UnsupportedDependencyShape);
                }
            }
            ExpressionNode::Call(call)
                if matches!(
                    call.target.as_str(),
                    DEPEND_MACHINE_NAME
                        | DEPEND_AS_MACHINE_NAME
                        | DEPEND_WHEN_MACHINE_NAME
                        | DEPEND_AS_WHEN_MACHINE_NAME
                ) =>
            {
                match (
                    call.target.as_str(),
                    syntax_trees.expressions.expression_handles(call.arguments),
                ) {
                    (DEPEND_MACHINE_NAME, [source]) if accepted_sources.contains(source) => {}
                    (DEPEND_AS_MACHINE_NAME, [alias, source])
                        if accepted_aliases.contains(alias)
                            && accepted_sources.contains(source) => {}
                    _ => return Err(DependencyProjectionError::UnsupportedDependencyShape),
                }
            }
            _ => {}
        }
    }
    Ok(())
}
