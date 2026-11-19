// Direct dependency call projection from shared `build_declarations` row grammar into `DependencySourceRequest` values.

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
    pub requests: Vec<(DependencyPurpose, DependencySourceRequest)>, // Authored-order rows tagged with the scope their call authorizes.
    pub accepted_statements: Vec<StatementHandle>,
    pub accepted_sources: Vec<ExpressionHandle>,
    pub accepted_calls: Vec<Vec<ExpressionHandle>>, // Argument rosters of projected vocabulary calls.
}

pub(super) fn project_direct_dependencies(
    syntax_trees: &SyntaxTrees,
    build: &shared::BuildEntrySyntaxProjection,
) -> Result<ProjectedDirectDependencies, DependencyProjectionError> {
    let mut projected = ProjectedDirectDependencies {
        requests: Vec::new(),
        accepted_statements: Vec::new(),
        accepted_sources: Vec::new(),
        accepted_calls: Vec::new(),
    };
    for row in
        shared::project_dependency_rows(syntax_trees, build).map_err(map_dependency_row_error)?
    {
        let explicit_alias = row
            .alias()
            .map(|alias_handle| project_alias_literal(syntax_trees, alias_handle))
            .transpose()?;
        projected.requests.push((
            row.purpose(),
            project_source_literal(syntax_trees, row.source(), explicit_alias)?,
        ));
        projected
            .accepted_statements
            .extend_from_slice(row.statements());
        projected
            .accepted_calls
            .extend_from_slice(row.accepted_call_arguments());
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
