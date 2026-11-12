/*
Editable dependency projection from parsed `build.omg` syntax trees.

`extract_scoped_requests_from_source` projects every unconditional direct
dependency row in authored order, tagged with the scope it authorizes, so
edit planning can correlate row positions to one flat list across both
purposes. We validate the editable shape without treating its literal role as
source identity or a declaration from an interpreted activation.
*/

use super::error::DependencyProjectionError;
use super::policy::{reject_authored_toolchain_vocabulary, reject_unprojected_dependency_syntax};
use super::{DependencyPurpose, DependencySourceRequest};
use build_declarations as shared;
use syntax_trees::SyntaxTrees;

mod calls;
mod declaration;

use declaration::map_build_declaration_error;

pub(super) fn extract_scoped_requests_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    let syntax_trees = syntax_trees_for(source)?;
    project_editable_dependencies(&syntax_trees)
}

fn syntax_trees_for(source: &str) -> Result<SyntaxTrees, DependencyProjectionError> {
    psi_compiler::parse_text(source)
        .map(|parsed| parsed.syntax)
        .map_err(|error| match error {
            psi_compiler::SourceParseError::Lex { message } => {
                DependencyProjectionError::Lex { message }
            }
            psi_compiler::SourceParseError::Parse { message } => {
                DependencyProjectionError::Parse { message }
            }
        })
}

fn project_editable_dependencies(
    syntax_trees: &SyntaxTrees,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;
    let build_entry = match shared::project_build_entry_syntax(syntax_trees) {
        Ok(projection) => projection,
        Err(error @ shared::BuildDeclarationError::MissingBuildDeclaration) => {
            reject_unprojected_dependency_syntax(syntax_trees, &[], &[], &[])?;
            return Err(map_build_declaration_error(error));
        }
        Err(error) => return Err(map_build_declaration_error(error)),
    };
    let dependencies = calls::project_direct_dependencies(syntax_trees, &build_entry)?;
    reject_unprojected_dependency_syntax(
        syntax_trees,
        &dependencies.accepted_statements,
        &dependencies.accepted_sources,
        &dependencies.accepted_calls,
    )?;
    shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    Ok(dependencies.requests)
}
