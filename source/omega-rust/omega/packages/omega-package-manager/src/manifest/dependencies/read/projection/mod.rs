use super::error::DependencyProjectionError;
use super::model::BuildDependencyProjection;
use super::policy::{reject_authored_toolchain_vocabulary, reject_unprojected_dependency_syntax};
use crate::manifest::roles::{BuildDeclarationError, convert_shared_declaration};
use omega_build_declarations as shared;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::Item;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

mod calls;
mod graph;

pub(super) const DEPEND_MACHINE_NAME: &str = "depend";
pub(super) const DEPEND_AS_MACHINE_NAME: &str = "depend_as";

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
    extract_build_projection_from_syntax_trees(&syntax_trees, &tokens)
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
    tokens: &[psi_tokens::Token<'_>],
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
    let build_machine = syntax_trees
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine)
                if syntax_trees
                    .items
                    .state_handles(machine.states)
                    .contains(&build_entry.build_entry()) =>
            {
                Some(machine)
            }
            _ => None,
        })
        .expect("validated build entry must belong to its root machine");

    let calls = calls::project_dependency_occurrences(syntax_trees, build_machine.states)?;
    reject_unprojected_dependency_syntax(
        syntax_trees,
        &calls.accepted_statements,
        &calls.accepted_sources,
        &calls.accepted_aliases,
    )?;
    let dependencies = graph::project_state_graph(
        syntax_trees,
        tokens,
        build_machine.states,
        build_entry.build_entry(),
        build_entry.builder_parameter(),
        calls.occurrences,
    )?;
    let role_projection = shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    Ok(BuildDependencyProjection::new(
        convert_shared_declaration(role_projection.into_declaration()),
        dependencies,
    ))
}

fn map_build_declaration_error(error: BuildDeclarationError) -> DependencyProjectionError {
    match error {
        BuildDeclarationError::ScopedBuildMachine { scope } => {
            DependencyProjectionError::ScopedBuildMachine { scope }
        }
        BuildDeclarationError::DuplicateBuildMachines { count } => {
            DependencyProjectionError::DuplicateBuildMachines { count }
        }
        BuildDeclarationError::InvalidBuildMachine => {
            DependencyProjectionError::InvalidBuildMachine
        }
        BuildDeclarationError::MissingBuildEntry => DependencyProjectionError::MissingBuildEntry,
        BuildDeclarationError::InvalidBuildParameter => {
            DependencyProjectionError::InvalidBuildParameter
        }
        error => DependencyProjectionError::BuildDeclaration(Box::new(error)),
    }
}
