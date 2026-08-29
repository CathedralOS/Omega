use super::error::DependencyProjectionError;
use super::model::{BuildDependencyProjection, DependencySourceRequest};
use super::policy::{reject_authored_toolchain_vocabulary, reject_unprojected_dependency_syntax};
use super::source_literal::{project_alias_literal, project_source_literal};
use crate::manifest::roles::{BuildDeclarationError, convert_shared_declaration};
use omega_build_declarations as shared;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::statement::StatementNode;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

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
    let entry_handle = build_entry.build_entry();
    let entry = syntax_trees.items.state(entry_handle);
    let builder_parameter = syntax_trees
        .items
        .state_parameter(build_entry.builder_parameter());
    let builder_name = builder_parameter.name.as_str();
    let mut requests = Vec::<DependencySourceRequest>::new();
    let mut accepted_statements = Vec::new();
    let mut accepted_sources = Vec::new();
    let mut accepted_aliases = Vec::new();
    for statement_handle in syntax_trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle) else {
            continue;
        };
        if !matches!(
            call.target.as_str(),
            DEPEND_MACHINE_NAME | DEPEND_AS_MACHINE_NAME
        ) {
            continue;
        }
        if call.receiver_starts_at_self
            || !matches!(
                syntax_trees.statements.identifier_path_members(call.receiver),
                [receiver] if receiver.as_str() == builder_name
            )
        {
            return Err(DependencyProjectionError::WrongDependencyReceiver);
        }
        if !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement != Default::default()
            || call.discards_result
        {
            return Err(DependencyProjectionError::WrongDependencyArguments);
        }
        let arguments = syntax_trees.statements.expression_handles(call.arguments);
        let (explicit_alias, source_handle) = match call.target.as_str() {
            DEPEND_MACHINE_NAME => {
                let [source_handle] = arguments else {
                    return Err(DependencyProjectionError::WrongDependencyArguments);
                };
                (None, *source_handle)
            }
            DEPEND_AS_MACHINE_NAME => {
                let [alias_handle, source_handle] = arguments else {
                    return Err(DependencyProjectionError::WrongDependencyArguments);
                };
                let alias = project_alias_literal(syntax_trees, *alias_handle)?;
                accepted_aliases.push(*alias_handle);
                (Some(alias), *source_handle)
            }
            _ => unreachable!("dependency operation filtered above"),
        };
        requests.push(project_source_literal(
            syntax_trees,
            source_handle,
            explicit_alias,
        )?);
        accepted_statements.push(*statement_handle);
        accepted_sources.push(source_handle);
    }

    reject_unprojected_dependency_syntax(
        syntax_trees,
        &accepted_statements,
        &accepted_sources,
        &accepted_aliases,
    )?;
    let role_projection = shared::project_build_declaration_in_entry(syntax_trees, build_entry)
        .map_err(map_build_declaration_error)?;
    Ok(BuildDependencyProjection::new(
        convert_shared_declaration(role_projection.into_declaration()),
        requests,
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
