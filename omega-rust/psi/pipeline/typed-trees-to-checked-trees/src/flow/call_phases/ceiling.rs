//! The declared-signature ceiling of a call whose write frame is unknown.
//!
//! An opaque, unresolved, generic or dispatched callee, or an actual whose
//! storage origin the frame resolver cannot summarize, retains "the
//! signature/authority ceiling", never an empty write set and never a guess
//! narrower than the declaration (wiki/spec/language/dependent_values.md,
//! "Mutation frames"). That ceiling is the storage reachable through the
//! call's exclusive borrows: each `&mut`/`&write` actual's exact storage and
//! the receiver of a `&mut self` callee. A by-value argument whose type
//! carries no exclusive reference hands the callee its own copy and reaches
//! no caller storage; one that may carry a reference, or an exclusive actual
//! with no representable storage origin, leaves the ceiling unrepresentable,
//! and the caller keeps retiring every live fact as before.

use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use checked_trees::BorrowCallFact;
use typed_trees::types::TypeReferenceNode;

pub(super) fn signature_ceiling_places(
    program: &typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<Vec<CanonicalPlace>> {
    // A builtin function (`min`, `max`, `sqrt`, the float classifiers, the
    // asm intrinsics) takes scalar values and owns no caller storage; its
    // ceiling is empty even though it declares no state parameters.
    if program
        .symbols
        .builtin_function_for_symbol(borrow_call.target_symbol)
        .is_some()
    {
        return Some(Vec::new());
    }
    let site = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let parameters =
        crate::semantic_calls::call_target_parameters(program, borrow_call.target_symbol)?;
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &site);
    if parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count()
        != arguments.len()
    {
        return None;
    }
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(ctx.call_frames, program, &mut owned_frames);
    let mut places = Vec::new();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let actual = if parameter.is_self {
            if !is_exclusive_reference(program, parameter.type_reference) {
                continue;
            }
            crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &site,
            )?
        } else {
            let argument = arguments[argument_index];
            argument_index += 1;
            if !is_exclusive_reference(program, parameter.type_reference) {
                // Declared reference-free storage cannot reach the caller; an
                // unbound generic, opaque or reference-bearing value can.
                if resolver.is_none_or(|resolver| {
                    resolver.local_requires_write_origin(parameter.type_reference)
                }) {
                    return None;
                }
                continue;
            }
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                borrow_call.statement_index,
                argument,
            )?
        };
        for storage in crate::flow::rebase_local_write_places(
            program,
            state.symbol,
            borrow_call.statement_index,
            actual,
            ctx.call_frames,
        )? {
            if !places.contains(&storage) {
                places.push(storage);
            }
        }
    }
    crate::flow::close_storage_places_over_aliases_with_resolver(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        places,
        ctx.call_frames,
    )
}

fn is_exclusive_reference(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
            _ => return false,
        }
    }
    false
}
