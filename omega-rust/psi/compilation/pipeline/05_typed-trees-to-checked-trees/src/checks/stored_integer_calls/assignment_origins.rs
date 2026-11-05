//! Physical destinations of one performed integer assignment.
//!
//! The captured-address query owns operand order. Owned storage can retain its
//! declaration geometry without an alias inventory, but reference-backed
//! destinations must still supply their actual origins.

use crate::published::places::place_at;
use typed_trees::{machine::Machine, state::State, statement::StatementNode};
use validation::{CallFrameResolver, LocalWriteSource};

pub(super) fn sources(
    program: &dyn validation::CheckingInput,
    frames: &CallFrameResolver<'_>,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    statement: &StatementNode,
) -> Result<Vec<LocalWriteSource>, &'static str> {
    let assignment = statement
        .as_assignment()
        .ok_or("write query is not an assignment")?;
    frames
        .assignment_storage_origins(machine, statement)
        .ok()
        .map(|storage| storage.destinations)
        .or_else(|| owned_destination(program, state, statement_index, assignment.target))
        .ok_or("performed write has unknown destination origins")
}

/// Owned storage has its declaration's geometry even when the alias
/// inventory has no entry for it. This cannot turn a reference endpoint
/// into ordinary storage: the whole owner must contain no loans.
fn owned_destination(
    program: &dyn validation::CheckingInput,
    state: &State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<LocalWriteSource>> {
    let place = place_at(program, state.symbol, statement_index, expression)?;
    let root = place.root.as_symbol()?;
    let reference = program
        .state_parameter_by_symbol(state, root)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, root)
                .map(|(_, local)| local.type_reference)
        })?;
    if crate::checks::type_reference_is_reference(program, reference)
        || !validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::CleanupOwned,
        )
    {
        return None;
    }
    Some(vec![LocalWriteSource {
        invocation_output: None,
        invocation_result: None,
        is_image_storage: false,
        root,
        segments: place.segments,
        representation: None,
        recasts: Vec::new(),
        collection_coarse: false,
        path: String::new(),
    }])
}
