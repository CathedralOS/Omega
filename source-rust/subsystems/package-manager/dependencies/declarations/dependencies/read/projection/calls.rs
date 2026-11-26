// Direct dependency call projection from shared `build_declarations` row grammar into `DependencySourceRequest` values.

use crate::declarations::dependencies::read::error::DependencyProjectionError;
use crate::declarations::dependencies::read::source_literal::{
    project_alias_literal, project_source_literal,
};
use crate::declarations::editable_build;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;

pub(super) struct ProjectedDirectDependencies {
    pub rows: Vec<super::EditableDependency>,
    pub accepted_calls: Vec<ExpressionHandle>,
}

pub(super) fn project_direct_dependencies(
    syntax_trees: &SyntaxTrees,
    build: &editable_build::BuildEntrySyntaxProjection,
) -> Result<ProjectedDirectDependencies, DependencyProjectionError> {
    let mut projected = ProjectedDirectDependencies {
        rows: Vec::new(),
        accepted_calls: Vec::new(),
    };
    for row in
        editable_build::project_dependency_rows(syntax_trees, build).map_err(map_dependency_row_error)?
    {
        let explicit_alias = row
            .alias()
            .map(|alias_handle| project_alias_literal(syntax_trees, alias_handle))
            .transpose()?;
        projected.rows.push(super::EditableDependency {
            request: project_source_literal(syntax_trees, row.source(), explicit_alias)?,
            source: row.source(),
        });
        projected
            .accepted_calls
            .extend_from_slice(row.accepted_calls());
    }
    Ok(projected)
}

fn map_dependency_row_error(error: editable_build::DependencyRowError) -> DependencyProjectionError {
    match error {
        editable_build::DependencyRowError::WrongReceiver => {
            DependencyProjectionError::WrongDependencyReceiver
        }
        editable_build::DependencyRowError::WrongArguments => {
            DependencyProjectionError::WrongDependencyArguments
        }
    }
}
