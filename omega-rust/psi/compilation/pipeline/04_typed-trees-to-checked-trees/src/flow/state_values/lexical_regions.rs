//! Captured entry values of an extracted region in the same source activation.
//! Ordinary machine bodies remain checked only under their declared premises.

use super::{ArgumentInputs, StateValues, fields};
use arena::HandleSpan;
use checked_trees::{BorrowCallFact, FlowOperandFact, FlowSemanticContextRef};
use facts::{FactPlan, ScalarValue};

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn record<'plans>(
    program: &'plans dyn validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut crate::flow::FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
    captured: &[FlowOperandFact],
    invocation: HandleSpan<FlowSemanticContextRef>,
) {
    let Some((owner_index, state_index)) = build.state_location(program, call.target_symbol) else {
        return;
    };
    let owner = &program.machines()[owner_index];
    let destination = &program.machine_states(owner)[state_index];
    let Some(origin) = owner.lexical_region_origin else {
        return;
    };
    if super::entry_state(program, owner).is_none_or(|entry| entry.symbol != destination.symbol) {
        return;
    }
    let site = crate::flow::calls::memoized_find_call_site(
        program,
        build,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    );
    let parameters = program.state_parameters(destination);
    let authenticated = !owner.is_public
        && !owner.target_family_interface
        && owner.supplies_checked_body()
        && origin.machine == machine.symbol
        && origin.state == state.symbol
        && matches!(site, Some(validation::CallSite::Statement(source))
            if source.target_symbol == destination.symbol && source.arguments == origin.arguments)
        && !call.has_receiver
        && captured.len() == parameters.len()
        && captured.iter().map(|value| value.expression).eq(program
            .statement_table
            .expression_handles(origin.arguments)
            .iter()
            .copied())
        && captured.iter().all(|value| {
            value.state_symbol == state.symbol && value.statement_index == call.statement_index
        });
    let values = parameters
        .iter()
        .zip(0..)
        .map(|(parameter, ordinal)| {
            let observed = authenticated
                .then(|| captured.get(ordinal))
                .flatten()
                .and_then(|capture| {
                    let completed =
                        super::completed_argument_facts(program, build, capture, invocation)?;
                    // Every retained alternative must agree at this read cut.
                    // A missing candidate is unknown, not a narrower predecessor.
                    let mut values = completed.places.iter().map(|place| {
                        crate::values::scalar_value_at_place(
                            program,
                            semantic,
                            completed
                                .contexts
                                .iter()
                                .map(|context| semantic.contexts.get(*context)),
                            &place.projected,
                        )
                        .or_else(|| super::literal(program, capture.expression))
                    });
                    let first = values.next()??;
                    values
                        .all(|value| value.as_ref() == Some(&first))
                        .then_some(first)
                })
                .unwrap_or(ScalarValue::Unknown);
            (parameter.symbol, observed)
        })
        .collect();
    let source =
        fields::BoundsSource::invocation(state.symbol, call.statement_index, call.call_ordinal);
    // Bad or unavailable custody is an unknown incoming edge, never a missing
    // predecessor that could preserve another activation's narrower value.
    super::join(
        program,
        build,
        owner,
        destination,
        source,
        StateValues {
            state: destination.symbol,
            values,
            bounds: Vec::new(),
            fields: Vec::new(),
            qualifications: Vec::new(),
            predicates: Vec::new(),
            entry_paths: crate::facts::boolean_predicates::EntryPaths::unknown(),
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        },
    );
}
