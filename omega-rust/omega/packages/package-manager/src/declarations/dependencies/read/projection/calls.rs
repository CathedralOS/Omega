use crate::declarations::dependencies::read::error::DependencyProjectionError;
use crate::declarations::dependencies::read::source_literal::{
    project_alias_literal, project_source_literal,
};
use crate::declarations::dependencies::read::{DependencyPurpose, DependencySourceRequest};
use build_declarations as shared;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::statement::StatementHandle;

pub(super) struct ProjectedDirectDependencies {
    /// Authored-order rows tagged with the scope their call authorizes.
    pub requests: Vec<(DependencyPurpose, DependencySourceRequest)>,
    pub accepted_statements: Vec<StatementHandle>,
    pub accepted_sources: Vec<ExpressionHandle>,
    pub accepted_aliases: Vec<ExpressionHandle>,
}

/// Interpret the shared dependency row grammar into source requests.
///
/// `build_declarations` owns which `builder.<operation>(...)` statements are
/// dependency rows, their authorized scope, and their argument shape. This
/// layer only interprets the retained literals into requests.
pub(super) fn project_direct_dependencies(
    syntax_trees: &SyntaxTrees,
    build: &shared::BuildEntrySyntaxProjection,
) -> Result<ProjectedDirectDependencies, DependencyProjectionError> {
    let mut projected = ProjectedDirectDependencies {
        requests: Vec::new(),
        accepted_statements: Vec::new(),
        accepted_sources: Vec::new(),
        accepted_aliases: Vec::new(),
    };
    for row in
        shared::project_dependency_rows(syntax_trees, build).map_err(map_dependency_row_error)?
    {
        let explicit_alias = row
            .alias()
            .map(|alias_handle| project_alias_literal(syntax_trees, alias_handle))
            .transpose()?;
        if let Some(alias_handle) = row.alias() {
            projected.accepted_aliases.push(alias_handle);
        }
        projected.requests.push((
            row.purpose(),
            project_source_literal(syntax_trees, row.source(), explicit_alias)?,
        ));
        projected.accepted_statements.push(row.statement());
        projected.accepted_sources.push(row.source());
    }
    Ok(projected)
}

fn map_dependency_row_error(error: shared::DependencyRowError) -> DependencyProjectionError {
    match error {
        shared::DependencyRowError::WrongReceiver => {
            DependencyProjectionError::WrongDependencyReceiver
        }
        shared::DependencyRowError::WrongArguments => {
            DependencyProjectionError::WrongDependencyArguments
        }
    }
}
