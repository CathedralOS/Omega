/*
Editable dependency projection from parsed `build.omg` syntax trees.

`extract_scoped_from_source` projects every unconditional direct
dependency row in authored order, tagged with the scope it authorizes, so
edit planning can correlate row positions to one flat list across both
purposes. Only the executed Build determines which product it produces.
*/

use super::error::DependencyProjectionError;
use super::policy::reject_unprojected_dependency_syntax;
use super::{DependencyPurpose, DependencySourceRequest};
use crate::declarations::editable_build;
use syntax_trees::SyntaxTrees;

mod calls;

pub(crate) struct EditableSource<'a> {
    pub parsed: source_syntax::ParsedText<'a>,
    pub entry: editable_build::BuildEntrySyntaxProjection,
    pub requests: Vec<(DependencyPurpose, DependencySourceRequest)>,
}

pub(crate) fn extract_scoped_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    let parsed = parse_build(source)?;
    let entry = project_entry(&parsed.syntax)?;
    project_editable_dependencies(&parsed.syntax, &entry)
}

pub(crate) fn project_source_for_edit(
    source: &str,
) -> Result<EditableSource<'_>, DependencyProjectionError> {
    let parsed = parse_build(source)?;
    let entry = project_entry(&parsed.syntax)?;
    let requests = project_editable_dependencies(&parsed.syntax, &entry)?;
    Ok(EditableSource {
        parsed,
        entry,
        requests,
    })
}

fn parse_build(source: &str) -> Result<source_syntax::ParsedText<'_>, DependencyProjectionError> {
    source_syntax::parse_text(source).map_err(|error| match error {
        source_syntax::SourceParseError::Lex { message, .. } => {
            DependencyProjectionError::Lex { message }
        }
        source_syntax::SourceParseError::Parse { message, .. } => {
            DependencyProjectionError::Parse { message }
        }
    })
}

fn project_entry(
    syntax_trees: &SyntaxTrees,
) -> Result<editable_build::BuildEntrySyntaxProjection, DependencyProjectionError> {
    match editable_build::project_build_entry_syntax(syntax_trees) {
        Ok(projection) => Ok(projection),
        Err(error @ DependencyProjectionError::MissingBuildMachine) => {
            reject_unprojected_dependency_syntax(syntax_trees, &[], &[])?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

fn project_editable_dependencies(
    syntax_trees: &SyntaxTrees,
    build_entry: &editable_build::BuildEntrySyntaxProjection,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    let dependencies = calls::project_direct_dependencies(syntax_trees, build_entry)?;
    reject_unprojected_dependency_syntax(
        syntax_trees,
        &dependencies.accepted_sources,
        &dependencies.accepted_calls,
    )?;
    Ok(dependencies.requests)
}
