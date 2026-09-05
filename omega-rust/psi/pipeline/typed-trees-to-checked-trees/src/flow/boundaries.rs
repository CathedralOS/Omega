use super::*;

pub(super) fn append_call_boundary_edges(
    program: &typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    borrow_call: &BorrowCallFact,
) -> HandleSpan<FlowBoundaryEdgeFact> {
    let Some((target_machine, target_state)) =
        target_machine_and_state(program, borrow_call.target_symbol)
    else {
        return HandleSpan::empty();
    };

    let mut span = HandleSpan::empty();
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(target_machine) {
        append_boundary_edges_for_trait(
            program,
            ctx,
            borrow_call,
            target_state,
            conformance.symbol,
            &mut visited_traits,
            &mut span,
        );
    }

    span
}

fn append_boundary_edges_for_trait(
    program: &typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    borrow_call: &BorrowCallFact,
    target_state: &typed_trees::state::State,
    trait_symbol: SymbolHandle,
    visited_traits: &mut Vec<SymbolHandle>,
    span: &mut HandleSpan<FlowBoundaryEdgeFact>,
) {
    if !trait_symbol.is_valid() || visited_traits.contains(&trait_symbol) {
        return;
    }

    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == trait_symbol)
    else {
        return;
    };

    visited_traits.push(trait_symbol);

    if trait_definition.is_boundary {
        for signature in program
            .trait_machine_signatures(trait_definition)
            .iter()
            .filter(|signature| signature.name == target_state.name)
        {
            ctx.boundaries.edges.append_to_span(
                span,
                FlowBoundaryEdgeFact {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    receiver_symbol: borrow_call.receiver_symbol,
                    target_symbol: borrow_call.target_symbol,
                    boundary_trait_symbol: trait_definition.symbol,
                    boundary_signature_symbol: signature.symbol,
                },
            );
        }
    }

    for requirement in program.trait_requirements(trait_definition) {
        append_boundary_edges_for_trait(
            program,
            ctx,
            borrow_call,
            target_state,
            requirement.symbol,
            visited_traits,
            span,
        );
    }

    visited_traits.pop();
}

fn target_machine_and_state(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_state_symbol)
            .map(|state| (machine, state))
    })
}
