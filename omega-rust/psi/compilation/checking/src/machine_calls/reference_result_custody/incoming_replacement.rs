//! Retirement of a still-current descriptor delivered inside an owned input.
//! Ingress permissions are not synthetic statement-zero loans. Join the exact
//! admitted leaf to this store, prove no preceding or RHS write replaced its
//! descriptor, and let ordinary carrier cleanup retire only that home/path.
//! Installed replacements keep the separate activation/weakening-group route.

use super::captured_returns::paths_overlap as overlaps;
use super::{CheckFacts, CheckedReferenceResultSourcePlan, SymbolHandle};
use checked_trees::{CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment};
use facts::{PlaceRoot, PlaceSegment};

pub(super) fn carrier(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: PlaceRoot,
    statement_index: u32,
    selection: &[CheckedUnitStructuralPathSegment],
) -> Option<CheckedReferenceResultSourcePlan> {
    let symbol = owner.as_symbol()?;
    let parameter = program.state_parameter_by_symbol(state, symbol)?;
    if !parameter.is_mutable
        || crate::reference_shell(program, parameter.type_reference).is_some()
        || !super::stored_carrier_ingress_is_complete(
            program,
            facts,
            machine,
            state.symbol,
            symbol,
            parameter.type_reference,
        )
    {
        return None;
    }
    let segments = super::loan_place_segments(program, parameter.type_reference, selection)?;
    // Every strict prefix must be owned storage. The selected endpoint is a
    // descriptor only when its receiving type agrees; a referent write must
    // never retire the pointer used to reach that referent.
    for length in 0..segments.len() {
        let prefix = crate::CanonicalPlace {
            root: owner,
            segments: segments[..length].to_vec(),
        };
        let reference = crate::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index as usize,
            &prefix,
        )?;
        if crate::reference_shell(program, reference).is_some() {
            return None;
        }
    }
    let selected =
        crate::project_type_reference_from_segments(program, parameter.type_reference, &segments)?;
    super::stored_loan_parts(program, selected)?;
    let assignment = program
        .statement_table
        .statement_at(state, statement_index as usize)?
        .as_assignment()?;
    let descriptor_identity = crate::stored_structural_type_identity(program, selected, &[], &[]);
    let receiving_identity =
        crate::stored_structural_type_identity(program, assignment.receiving_type, &[], &[]);
    if !descriptor_identity.is_valid()
        || !receiving_identity.is_valid()
        || descriptor_identity != receiving_identity
    {
        return None;
    }
    let flow = super::state_flow(facts, machine, state.symbol)?;
    let current = facts.flow.state_statement(flow, statement_index as usize)?;
    if !facts.semantic.places.is_valid(current.stored_destination) {
        return None;
    }
    let destination = facts.semantic.places.get(current.stored_destination);
    if destination.root != owner
        || facts.semantic.place_segments.span(destination.segments)? != segments
    {
        return None;
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    for statement in facts.flow.control.statements.span(flow.statements)? {
        if !order.precedes(statement.statement_index, statement_index as usize) {
            continue;
        }
        if statement.completed_unknown_write
            || overlaps_writes(facts, owner, &segments, statement.completed_writes)?
        {
            return None;
        }
        if facts.semantic.places.is_valid(statement.stored_destination) {
            let destination = facts.semantic.places.get(statement.stored_destination);
            if destination.root == owner
                && overlaps(
                    &segments,
                    facts.semantic.place_segments.span(destination.segments)?,
                )
            {
                return None;
            }
        }
    }
    // The completed RHS retains all writes evaluated before the final store.
    // Entry-only stability would resurrect an ingress descriptor replaced by
    // an earlier RHS operand. Missing captures remain unknown, never empty.
    if !facts.flow.control.operands.is_valid(current.stored_value) {
        return None;
    }
    let value = facts.flow.control.operands.get(current.stored_value);
    if value.state_symbol != state.symbol
        || value.statement_index != statement_index as usize
        || value.expression != assignment.value
        || value.entry_origin_unknown_write
        || overlaps_writes(facts, owner, &segments, value.entry_origin_writes)?
    {
        return None;
    }
    let mut leaves = super::record_carrier_sources(
        program,
        parameter.type_reference,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
    )?
    .into_iter()
    .filter(|leaf| leaf.path == selection);
    let leaf = leaves.next()?;
    if leaves.next().is_some() {
        return None;
    }
    Some(leaf)
}

fn overlaps_writes(
    facts: &CheckFacts,
    owner: PlaceRoot,
    selection: &[PlaceSegment],
    writes: arena::HandleSpan<checked_trees::FlowOperandReferent>,
) -> Option<bool> {
    for write in facts.flow.control.operand_origin_writes.span(writes)? {
        if write.root == owner
            && overlaps(
                selection,
                facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(write.segments)?,
            )
        {
            return Some(true);
        }
    }
    Some(false)
}
