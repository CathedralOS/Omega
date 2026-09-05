use super::error::DependencyProjectionError;
use super::model::BuildDependencyProjection;
use super::policy::{reject_authored_toolchain_vocabulary, reject_unprojected_dependency_syntax};
use crate::declarations::roles::convert_shared_declaration;
use build_declarations as shared;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_syntax_trees;

mod calls;
mod declaration;

use declaration::map_build_declaration_error;

pub(super) const DEPEND_MACHINE_NAME: &str = "depend";
pub(super) const DEPEND_AS_MACHINE_NAME: &str = "depend_as";
pub(super) const DEPEND_WHEN_MACHINE_NAME: &str = "depend_when";
pub(super) const DEPEND_AS_WHEN_MACHINE_NAME: &str = "depend_as_when";

pub(super) fn extract_build_projection_from_source(
    source: &str,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| DependencyProjectionError::Lex {
            message: error.message,
        })?;
    let syntax_trees =
        parse_syntax_trees(&tokens).map_err(|error| DependencyProjectionError::Parse {
            message: error.message,
        })?;
    extract_build_projection_from_syntax_trees(&syntax_trees)
}

/// Apply the package manager's complete static dependency projection policy to
/// one already-decoded declaration without retaining source-layer requests.
pub(crate) fn validate_static_dependency_source(
    source: &str,
) -> Result<(), DependencyProjectionError> {
    extract_build_projection_from_source(source).map(drop)
}

fn extract_build_projection_from_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;
    let build_entry = match shared::project_build_entry_syntax(syntax_trees) {
        Ok(projection) => projection,
        Err(error @ shared::BuildDeclarationError::MissingBuildDeclaration) => {
            reject_unprojected_dependency_syntax(syntax_trees, &[], &[], &[])?;
            return Err(map_build_declaration_error(error));
        }
        Err(error) => return Err(map_build_declaration_error(error)),
    };
    let dependencies = calls::project_direct_dependencies(
        syntax_trees,
        build_entry.build_entry(),
        build_entry.builder_parameter(),
    )?;
    reject_unprojected_dependency_syntax(
        syntax_trees,
        &dependencies.accepted_statements,
        &dependencies.accepted_sources,
        &dependencies.accepted_aliases,
    )?;
    let role_projection = shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    Ok(BuildDependencyProjection::new(
        convert_shared_declaration(role_projection.into_declaration()),
        dependencies.requests.into(),
    ))
}
