// Syntax policy guards rejecting package-authored toolchain vocabulary and unprojected dependency calls or literals.
// Every authored call has an expression handle, including calls in nested statements.

use super::error::DependencyProjectionError;
use super::source_literal::{PACKAGE_SELECTION_TYPE_NAME, SOURCE_TYPE_NAME, constructor_parts};
use crate::declarations::editable_build::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_REGISTRATION_NAME, FROM_SOURCE_HELPER_NAME,
    REGISTRATION_TYPE_NAME, WITH_ALIAS_HELPER_NAME, expression_owner, imported_dependency_helper,
    machine_leaf_name, package_authored_type_name,
};
use package_compilation::declarations::{BUILD_MODULE_NAME, BUILD_TYPE_NAME};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::item::Item;

pub(crate) fn reject_authored_toolchain_vocabulary(
    syntax_trees: &SyntaxTrees,
) -> Result<(), DependencyProjectionError> {
    for item in syntax_trees.root_items() {
        if let Some(declaration) = package_authored_type_name(syntax_trees, item) {
            let name = machine_leaf_name(declaration);
            if [
                BUILD_MODULE_NAME,
                BUILD_TYPE_NAME,
                SOURCE_TYPE_NAME,
                PACKAGE_SELECTION_TYPE_NAME,
                DEPENDENCY_DESCRIPTION_TYPE_NAME,
                REGISTRATION_TYPE_NAME,
            ]
            .contains(&name)
            {
                return Err(DependencyProjectionError::AuthoredToolchainVocabulary {
                    name: name.to_owned(),
                });
            }
        }
        if let Item::Machine(machine) = item
            && let Some(owner) = machine.attached_data.as_ref()
            && owner.as_str(syntax_trees) == BUILD_MODULE_NAME
            && machine_leaf_name(machine.name.as_str(syntax_trees)) == DEPENDENCY_REGISTRATION_NAME
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
                if ((expression_owner(syntax_trees, call.receiver) == Some(BUILD_MODULE_NAME)
                    && call.target.as_str(syntax_trees) == DEPENDENCY_REGISTRATION_NAME)
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
