use super::error::DependencyProjectionError;
use super::policy::{reject_authored_toolchain_vocabulary, reject_unprojected_dependency_syntax};
use super::{
    BuildDependencyProjection, DependencyProjections, DependencyPurpose, DependencySourceRequest,
    ProjectedDependencies,
};
use crate::declarations::roles::convert_shared_declaration;
use build_declarations as shared;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_syntax_trees;

mod calls;
mod declaration;

use declaration::map_build_declaration_error;

pub(super) fn extract_build_projection_from_source(
    source: &str,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let syntax_trees = syntax_trees_for(source)?;
    Ok(project_build_and_dependencies(&syntax_trees)?.0)
}

/// Every unconditional direct dependency row in authored order, tagged with
/// the scope it authorizes. Edit planning correlates row positions to this
/// one flat list across both purposes; the split projection alone cannot
/// recover the authored interleaving.
pub(super) fn extract_scoped_requests_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    let syntax_trees = syntax_trees_for(source)?;
    Ok(project_build_and_dependencies(&syntax_trees)?.1)
}

/// Apply the package manager's complete static dependency projection policy to
/// one already-decoded declaration without retaining source-layer requests.
pub(crate) fn validate_static_dependency_source(
    source: &str,
) -> Result<(), DependencyProjectionError> {
    extract_build_projection_from_source(source).map(drop)
}

fn syntax_trees_for(source: &str) -> Result<SyntaxTrees, DependencyProjectionError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|error| DependencyProjectionError::Lex {
            message: error.message,
        })?;
    parse_syntax_trees(&tokens).map_err(|error| DependencyProjectionError::Parse {
        message: error.message,
    })
}

fn project_build_and_dependencies(
    syntax_trees: &SyntaxTrees,
) -> Result<
    (
        BuildDependencyProjection,
        Vec<(DependencyPurpose, DependencySourceRequest)>,
    ),
    DependencyProjectionError,
> {
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
        &dependencies.accepted_aliases,
    )?;
    let role_projection = shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    let mut product_requests = Vec::new();
    let mut build_requests = Vec::new();
    for (purpose, request) in &dependencies.requests {
        match purpose {
            DependencyPurpose::Product => product_requests.push(request.clone()),
            DependencyPurpose::Build => build_requests.push(request.clone()),
        }
    }
    Ok((
        BuildDependencyProjection::new(
            convert_shared_declaration(role_projection.into_declaration()),
            DependencyProjections::new(
                ProjectedDependencies::from(product_requests),
                ProjectedDependencies::from(build_requests),
            ),
        ),
        dependencies.requests,
    ))
}
