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
    // An unreached private activation supplies no incoming edge. Emitting
    // an unknown backedge before its first actual call would erase the facts
    // that call establishes and force an ungrounded loop-candidate premise.
    if !super::reachable(build, program, machine, state.symbol) {
        return;
    }
    let Some((owner_index, state_index)) = build.state_location(program, call.target_symbol) else {
        return;
    };
    let owner = &program.machines()[owner_index];
    let destination = &program.machine_states(owner)[state_index];
    if owner.lexical_region_origin.is_none()
        && !super::lexically_nested_machine(program, owner.symbol)
    {
        return;
    }
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
    let authenticated = validation::private_invocation_entry_is_closed(program, owner)
        && !owner.is_public
        && !owner.target_family_interface
        && owner.supplies_checked_body()
        && site.is_some_and(|site| {
            site.target_symbol(program) == destination.symbol
                && match owner.lexical_region_origin {
                    // Only the generated call's argument roster exactly captures
                    // the extracted region's bindings.
                    Some(origin) => {
                        origin.machine == machine.symbol
                            && origin.state == state.symbol
                            && validation::call_site_argument_expressions(program, &site)
                                .iter()
                                .copied()
                                .eq(program
                                    .statement_table
                                    .expression_handles(origin.arguments)
                                    .iter()
                                    .copied())
                    }
                    // An authored nested machine authenticates each invocation
                    // by lexical containment: the call's own recorded arguments
                    // are the capture roster under its operand evidence epoch.
                    None => {
                        super::lexical_scope_covers_call(program, machine.symbol, owner.symbol)
                            && validation::call_site_argument_expressions(program, &site)
                                .iter()
                                .copied()
                                .eq(captured.iter().map(|value| value.expression))
                    }
                }
        })
        && !call.has_receiver
        && captured.len() == parameters.len()
        && captured.iter().all(|value| {
            value.state_symbol == state.symbol && value.statement_index == call.statement_index
        });
    // Prepare each completed operand once. Scalars, bounds, qualifications
    // and field observations share that exact value/referent epoch; none may
    // reconstruct an earlier argument from the final invocation environment.
    let mut values = Vec::new();
    let mut bounds = Vec::new();
    let mut fields = Vec::new();
    let mut qualifications = Vec::new();
    for (ordinal, parameter) in parameters.iter().enumerate() {
        let capture = captured.get(ordinal);
        let completed = authenticated
            .then(|| capture)
            .flatten()
            .and_then(|capture| {
                super::completed_argument_facts(program, build, capture, invocation)
            });
        let observed = capture
            .zip(completed.as_ref())
            .and_then(|(capture, completed)| {
                // Every retained alternative must agree; absence is unknown.
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
        let interval = match &observed {
            ScalarValue::Integer(value) => Some(facts::IntegerRange {
                minimum: value.clone(),
                maximum: value.clone(),
            }),
            _ => completed.as_ref().and_then(|completed| {
                let mut joined: Option<facts::IntegerRange> = None;
                for place in &completed.places {
                    let incoming = crate::values::integer_bounds_at_place(
                        program,
                        semantic,
                        completed
                            .contexts
                            .iter()
                            .map(|context| semantic.contexts.get(*context)),
                        &place.projected,
                    )?;
                    joined = Some(match joined {
                        None => incoming,
                        Some(previous) => facts::IntegerRange {
                            minimum: previous.minimum.min(incoming.minimum),
                            maximum: previous.maximum.max(incoming.maximum),
                        },
                    });
                }
                joined
            }),
        };
        values.push((parameter.symbol, observed));
        bounds.push(super::ParameterBounds::new(parameter.symbol, interval));
        if let Some((capture, completed)) = capture.zip(completed.as_ref()) {
            fields.extend(super::fields::capture_completed_argument(
                program,
                semantic,
                completed,
                parameter.symbol,
            ));
            qualifications.extend(super::qualifications::capture_completed_argument(
                program,
                semantic,
                build,
                capture,
                completed,
                parameter.symbol,
            ));
        }
    }
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
            field_sources: Vec::new(),
            values,
            bounds,
            fields,
            qualifications,
            predicates: Vec::new(),
            entry_paths: crate::facts::boolean_predicates::EntryPaths::unknown(),
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        },
    );
}
