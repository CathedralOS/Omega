//! Dependency rows that package commands can edit in place.

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

mod dependency_import;
mod dependency_rows;
pub(crate) use dependency_import::{DependencyImport, helper as imported_dependency_helper};
mod direct_build_call;
mod entry;
#[cfg(test)]
mod tests;

pub(crate) use dependency_rows::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DependencyRowError, FROM_SOURCE_HELPER_NAME,
    WITH_ALIAS_HELPER_NAME, project_dependency_rows,
};
pub(crate) use entry::{
    BuildEntrySyntaxProjection, package_authored_type_name, project_build_entry_syntax,
};

pub(crate) const BUILD_MACHINE_NAME: &str = "build";

pub(crate) const DEPENDENCY_REGISTRATION_NAME: &str = "dependency";

pub(crate) const REGISTRATION_TYPE_NAME: &str = "DependencyRegistration";

pub(crate) fn machine_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

// The single-segment static owner of a call expression (`Build` or
// `dependency`), or none for a value receiver or an unqualified call.
pub(crate) fn expression_owner(
    syntax_trees: &SyntaxTrees,
    receiver: ExpressionHandle,
) -> Option<&str> {
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
