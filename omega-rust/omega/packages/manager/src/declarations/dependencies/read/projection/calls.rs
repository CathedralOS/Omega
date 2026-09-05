use super::{DEPEND_AS_MACHINE_NAME, DEPEND_MACHINE_NAME};
use crate::declarations::dependencies::read::error::DependencyProjectionError;
use crate::declarations::dependencies::read::model::DependencySourceRequest;
use crate::declarations::dependencies::read::source_literal::{
    project_alias_literal, project_source_literal,
};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::item::{StateHandle, StateParameterHandle};
use syntax_trees::statement::{StatementHandle, StatementNode};

pub(super) struct ProjectedDirectDependencies {
    pub requests: Vec<DependencySourceRequest>,
    pub accepted_statements: Vec<StatementHandle>,
    pub accepted_sources: Vec<ExpressionHandle>,
    pub accepted_aliases: Vec<ExpressionHandle>,
}

pub(super) fn project_direct_dependencies(
    syntax_trees: &SyntaxTrees,
    build_entry: StateHandle,
    builder_parameter: StateParameterHandle,
) -> Result<ProjectedDirectDependencies, DependencyProjectionError> {
    let mut projected = ProjectedDirectDependencies {
        requests: Vec::new(),
        accepted_statements: Vec::new(),
        accepted_sources: Vec::new(),
        accepted_aliases: Vec::new(),
    };
    let builder_name = syntax_trees
        .items
        .state_parameter(builder_parameter)
        .name
        .as_str();
    let entry = syntax_trees.items.state(build_entry);
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
        if call.receiver_starts_at_self {
            return Err(DependencyProjectionError::WrongDependencyReceiver);
        }
        let [receiver] = syntax_trees
            .statements
            .identifier_path_members(call.receiver)
        else {
            return Err(DependencyProjectionError::WrongDependencyReceiver);
        };
        if receiver.as_str() != builder_name {
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
                projected.accepted_aliases.push(*alias_handle);
                (Some(alias), *source_handle)
            }
            _ => unreachable!("dependency operation filtered above"),
        };
        projected.requests.push(project_source_literal(
            syntax_trees,
            source_handle,
            explicit_alias,
        )?);
        projected.accepted_statements.push(*statement_handle);
        projected.accepted_sources.push(source_handle);
    }
    Ok(projected)
}
