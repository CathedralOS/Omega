//! Release-store proofs bind the performed atomic event to its completed
//! destination and scalar operand. Generic type arguments retain the exact
//! atomic cell/value pair; a same-sized integer or another cell is not enough.
use super::{QueryArgument, QueryArguments};
use checked_trees::{CheckFacts, FlowOperandFact, FlowStateFact};
use typed_trees::types::PrimitiveType;
use validation::CanonicalPlace;

fn query_primitive(
    program: &dyn validation::CheckingInput,
    arguments: &QueryArguments<'_>,
) -> Option<PrimitiveType> {
    let frame = arguments.frames.len().checked_sub(1)?;
    let cell = arguments.binder_symbol(program, frame, 0)?;
    let value = arguments.binder_symbol(program, frame, 1)?;
    if !program.symbols.builtin_type_atom(cell)?.is_atomic()
        || program.symbols.builtin_type_atom(value)?.is_atomic()
    {
        return None;
    }
    let cell = program
        .type_reference_table
        .find_named_type_reference(cell)?;
    let value = program
        .type_reference_table
        .find_named_type_reference(value)?;
    let primitive = program.primitive_type_reference(cell)?;
    (program.primitive_type_reference(value) == Some(primitive)).then_some(primitive)
}

fn receiver_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    arguments: &QueryArguments<'_>,
    required: QueryArgument,
) -> Option<Vec<CanonicalPlace>> {
    let (expression, suffix) = arguments.source_projection(program, required, &[])?;
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement,
        expression,
    )?;
    if !suffix.is_empty() {
        let reference =
            validation::canonical_place_type_reference(program, state.symbol, statement, &place)?;
        place
            .segments
            .extend(crate::values::applied_projection_segments(
                program, reference, &suffix,
            )?);
    }
    crate::flow::stored_views::address_before_statement(
        program,
        &facts.semantic,
        &facts.flow.control,
        state,
        flow.statements,
        statement,
        place,
    )
}

fn captured_places(facts: &CheckFacts, operand: &FlowOperandFact) -> Option<Vec<CanonicalPlace>> {
    let rows = facts
        .flow
        .control
        .operand_referents
        .span(operand.referents)?;
    if rows.is_empty() {
        return None;
    }
    Some(
        rows.iter()
            .map(|row| CanonicalPlace {
                root: row.root,
                segments: facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span_or_empty(row.segments)
                    .to_vec(),
            })
            .collect(),
    )
}

pub(super) fn matches_store(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    event: &facts::Fact,
    arguments: &QueryArguments<'_>,
    required: &[QueryArgument],
) -> bool {
    let result = store_matches(
        program, facts, flow, state, statement, contexts, event, arguments, required,
    );
    result.unwrap_or(false)
}
fn store_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    event: &facts::Fact,
    arguments: &QueryArguments<'_>,
    required: &[QueryArgument],
) -> Option<bool> {
    let facts::ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = event.point
    else {
        return None;
    };
    if machine_symbol != flow.machine_symbol || state_symbol != state.symbol {
        return None;
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if !order.precedes(statement_index, statement)
        && !(statement
            == program
                .statement_table
                .statements(state.statement_nodes)
                .len()
            && order.position(statement_index).is_some())
    {
        return None;
    }
    let machine = program.machine_by_symbol(machine_symbol)?;
    let authored = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let carrier = crate::flow::atomic_effects::release_store(program, machine, state, authored)?;
    let row = facts.flow.state_statement(flow, statement_index)?;
    let destination = facts.flow.control.operands.get(row.store_target);
    let actual = captured_places(facts, destination)?;
    let [receiver, value] = required else {
        return None;
    };
    let requested = receiver_places(program, facts, flow, state, statement, arguments, *receiver)?;
    if actual.is_empty()
        || requested.is_empty()
        || actual.len() != requested.len()
        || actual.iter().any(|place| !requested.contains(place))
    {
        return Some(false);
    }
    let assignment = authored.as_assignment()?;
    let mut captures = facts
        .flow
        .control
        .atomic_executions
        .iter()
        .filter_map(|(_, row)| {
            (row.state_symbol == state.symbol
                && row.statement_index == statement_index
                && row.expression == assignment.value)
                .then_some(row)
        });
    let capture = captures.next()?;
    if captures.next().is_some() {
        return None;
    }
    let [operand] = facts.flow.control.operands.span(capture.operands)? else {
        return None;
    };
    let [expression] = carrier.operands.as_slice() else {
        return None;
    };
    if operand.expression != *expression {
        return None;
    }
    let primitive = query_primitive(program, arguments)?;
    let cell_type =
        validation::declared_place_type_raw(program, machine, Some(state), carrier.place)?;
    if program.primitive_type_reference(cell_type) != Some(primitive) {
        return Some(false);
    }
    if program.primitive_type_reference(operand.type_reference) != Some(primitive) {
        return Some(false);
    }
    Some(super::operands::operand_matches(
        program, facts, flow, state, statement, contexts, operand, arguments, *value, primitive,
    ))
}

pub(super) fn guarantee_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    call: &checked_trees::FlowCallFact,
    candidate: &typed_trees::proposition::PropositionApplication,
    arguments: &QueryArguments<'_>,
    required: &[QueryArgument],
) -> bool {
    let Some(contract) = validation::instruction_effect_contract(program, candidate) else {
        return false;
    };
    if contract.operation != facts::InstructionEffectOperation::AtomicReleaseStore {
        return false;
    }
    let Some(supplied) = QueryArguments::new(program, &contract) else {
        return false;
    };
    let Some(primitive) = query_primitive(program, arguments) else {
        return false;
    };
    if query_primitive(program, &supplied) != Some(primitive) {
        return false;
    }
    let Some(frame) = supplied.frames.last() else {
        return false;
    };
    let ([actual_receiver, actual_value], [receiver, value]) =
        (frame.arguments.as_slice(), required)
    else {
        return false;
    };
    let Some((expression, suffix)) = supplied.source_projection(program, *actual_receiver, &[])
    else {
        return false;
    };
    let input = super::super::call_guarantees::guarantee_input(
        program, facts, flow, call, expression, &suffix,
    );
    let Some((operand, path)) = input else {
        return false;
    };
    let Some(mut actual) = captured_places(facts, operand) else {
        return false;
    };
    for place in &mut actual {
        place.segments.extend_from_slice(&path);
    }
    let Some(requested) =
        receiver_places(program, facts, flow, state, statement, arguments, *receiver)
    else {
        return false;
    };
    if actual.is_empty()
        || actual.len() != requested.len()
        || actual.iter().any(|place| !requested.contains(place))
    {
        return false;
    }
    super::operands::guarantee_value_matches(
        program,
        facts,
        flow,
        state,
        statement,
        contexts,
        call,
        &supplied,
        *actual_value,
        arguments,
        *value,
        primitive,
    )
}
