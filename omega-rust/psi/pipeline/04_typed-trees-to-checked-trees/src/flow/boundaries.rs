use crate::flow::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::{BorrowCallFact, FlowBoundaryEdgeFact};
use symbols::SymbolHandle;

pub(super) fn append_call_boundary_edges(
    program: &typed_trees::TypedTrees,
    build: &mut FlowBuildContext,
    borrow_call: &BorrowCallFact,
) -> HandleSpan<FlowBoundaryEdgeFact> {
    let Some((target_machine, target_state)) = build
        .state_location(program, borrow_call.target_symbol)
        .map(|(machine_index, state_index)| {
            let machine = &program.machines()[machine_index];
            (machine, &program.machine_states(machine)[state_index])
        })
    else {
        return HandleSpan::empty();
    };

    let mut span = HandleSpan::empty();
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(target_machine) {
        append_boundary_edges_for_trait(
            program,
            build,
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
    build: &mut FlowBuildContext,
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
            build.boundaries.edges.append_to_span(
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
            build,
            borrow_call,
            target_state,
            requirement.symbol,
            visited_traits,
            span,
        );
    }

    visited_traits.pop();
}
