use super::{DEPEND_AS_MACHINE_NAME, DEPEND_MACHINE_NAME};
use crate::project::dependencies::read::error::DependencyProjectionError;
use crate::project::dependencies::read::model::DependencySourceRequest;
use crate::project::dependencies::read::source_literal::{
    project_alias_literal, project_source_literal,
};
use psi_source::SourceSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::ExpressionHandle;
use psi_syntax_trees::item::StateHandle;
use psi_syntax_trees::statement::{StatementHandle, StatementNode};

pub(super) struct DependencyOccurrence {
    pub state: StateHandle,
    pub receiver: String,
    pub request: DependencySourceRequest,
    pub source_span: SourceSpan,
}

pub(super) struct ProjectedDependencyOccurrences {
    pub occurrences: Vec<DependencyOccurrence>,
    pub accepted_statements: Vec<StatementHandle>,
    pub accepted_sources: Vec<ExpressionHandle>,
    pub accepted_aliases: Vec<ExpressionHandle>,
}

pub(super) fn project_dependency_occurrences(
    syntax_trees: &SyntaxTrees,
    states: psi_arena::HandleSpan<StateHandle>,
) -> Result<ProjectedDependencyOccurrences, DependencyProjectionError> {
    let mut projected = ProjectedDependencyOccurrences {
        occurrences: Vec::new(),
        accepted_statements: Vec::new(),
        accepted_sources: Vec::new(),
        accepted_aliases: Vec::new(),
    };
    for state_handle in syntax_trees.items.state_handles(states) {
        let state = syntax_trees.items.state(*state_handle);
        for statement_handle in syntax_trees.items.statements(state.statements) {
            let StatementNode::Call(call) = syntax_trees.statements.statement(*statement_handle)
            else {
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
            projected.occurrences.push(DependencyOccurrence {
                state: *state_handle,
                receiver: receiver.as_str().to_owned(),
                request: project_source_literal(syntax_trees, source_handle, explicit_alias)?,
                source_span: call.target.source_span(),
            });
            projected.accepted_statements.push(*statement_handle);
            projected.accepted_sources.push(source_handle);
        }
    }
    Ok(projected)
}
