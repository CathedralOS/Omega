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

/// A call may replace a pointer and then write through its replacement. A
/// fitting pending store proves nothing about those intermediate writes.
/// Report their missing origins only when the failed call actually receives
/// mutable access that can reach the descriptor for narrowed physical storage.
pub(super) fn operand_write_refusal<'program>(
    program: &'program dyn validation::CheckingInput,
    frames: &CallFrameResolver<'program>,
    machine: &'program Machine,
    state: &'program State,
    statement_index: usize,
    statement: &StatementNode,
    destinations: &[LocalWriteSource],
) -> Option<&'static str> {
    let assignment = statement.as_assignment()?;
    let reference =
        validation::declared_place_type_raw(program, machine, Some(state), assignment.target)?;
    validation::reference_shell(program, reference)?;
    let slot = place_at(program, state.symbol, statement_index, assignment.target)?;
    let root = slot.root.as_symbol()?;
    if !destinations.iter().any(|source| {
        validation::stored_integer_source_reach(program, state, source)
            .is_ok_and(|reach| reach.requires_fit)
    }) {
        return None;
    }
    let failures = frames.stored_reference_call_failures(machine, statement)?;
    for site in
        validation::collect_call_sites_in_statement(program, machine, state, statement_index)
    {
        let target = site.target_symbol(program);
        let validation::CallSite::Expression { expression, .. } = site else {
            continue;
        };
        if !failures
            .iter()
            .any(|failure| failure.target == target && failure.expression == expression)
            || validation::call_target_has_checked_body_contract(program, target)
        {
            continue;
        }
        let Some(parameters) = site.parameters(program, machine.symbol) else {
            continue;
        };
        let origins = match site {
            validation::CallSite::Statement(call) => frames.call_write_origins(machine, call),
            validation::CallSite::Expression { expression, .. } => {
                frames.expression_call_write_origins(machine, expression)
            }
            validation::CallSite::TransitionNamed { .. } => None,
        };
        let Some(origins) = origins else {
            continue;
        };
        for (position, parameter) in parameters.iter().enumerate() {
            if !validation::reference_shell(program, parameter.type_reference)
                .is_some_and(|shell| shell.access.is_exclusive())
            {
                continue;
            }
            let Some(sources) = super::actual_origins(parameters, position, &origins) else {
                continue;
            };
            if sources
                .iter()
                .any(|source| source.root == root && slot.segments.starts_with(&source.segments))
            {
                return Some(
                    "performed write has unknown destination origins inside an operand call that can replace the captured reference",
                );
            }
        }
    }
    None
}
