//! Endpoint conservation is an equality relation over exact completed coordinates.
//! The rank owner has already selected one arrival witness. This independent
//! relation never supplies another rank assignment or a value-identity premise.
//! Its domain comes from authored endpoint reads and completed actuals, not a
//! unique leaf of a carrier type. Every local and cross-member arrival constrains
//! the same relation; a missing literal/unsupported producer cannot erase an edge.

use super::super::{
    Engine, ExpressionHandle, ExpressionNode, Machine, Polynomial, RankSubjectCoordinate, State,
    arrival_correspondence, comparison_proven, coordinate::CoordinateKind, exact_integer_parameter,
    field_coordinates, fields, integer_bindings, lengths, meanings, parameter_comparisons,
    saved_arguments,
};
use super::{BinaryOperator, RankingRangeCallMember};
use facts::NormalizedWriteFrame;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

pub(crate) struct RankingRangeCallEdge<'program> {
    pub source: usize,
    pub destination: usize,
    pub site_state: &'program State,
    pub statement_ordinal: usize,
    pub arguments: &'program [ExpressionHandle],
    pub guards: Vec<(ExpressionHandle, bool)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValueCoordinate {
    state: usize,
    location: RankSubjectCoordinate,
}

struct StateFrame<'program> {
    machine: &'program Machine,
    state: &'program State,
}

struct Arrival<'program> {
    source: usize,
    destination: usize,
    statement: usize,
    arguments: &'program [ExpressionHandle],
    guards: Vec<(ExpressionHandle, bool)>,
    // A continuation executes its preceding callee first. Until the selected
    // return frame is supplied here it cannot conserve a live prefix value.
    after_call: bool,
}

pub(crate) fn mixed_call_endpoints_are_pinned<'program>(
    program: &'program dyn crate::CheckingInput,
    members: &[RankingRangeCallMember<'program>],
    calls: &[RankingRangeCallEdge<'program>],
    frames: Option<&crate::CallFrameResolver<'program>>,
) -> bool {
    let prove = || -> Option<()> {
        let mut states = Vec::new();
        let mut entries = Vec::new();
        let mut arrivals = Vec::new();
        let relevance = crate::rank_relevance(
            program,
            &members
                .iter()
                .map(|member| member.machine.symbol)
                .collect::<Vec<_>>(),
        )?;
        for member in members {
            let body = program.machine_states(member.machine);
            if body.is_empty() {
                return None;
            }
            let start = states.len();
            entries.push(start);
            states.extend(body.iter().map(|state| StateFrame {
                machine: member.machine,
                state,
            }));
            for arrival in
                crate::machine_calls::call_cycles::runtime_ranking::internal_arrival_sites(
                    program,
                    member.machine,
                )
            {
                // A rankless acyclic suffix cannot return to this component.
                // Its ordinary actual formation remains the source owner's
                // obligation; it need not retain a recurrent endpoint value.
                if !relevance.requires_rank(body[arrival.destination].symbol) {
                    continue;
                }
                let source = &body[arrival.source];
                let statement = program
                    .statement_table
                    .statements(source.statement_nodes)
                    .get(arrival.statement_index)?
                    .as_transition()?;
                let mut guards = Vec::new();
                for preceding in program
                    .statement_table
                    .statements(source.statement_nodes)
                    .iter()
                    .take(arrival.statement_index)
                {
                    if let Some(transition) = preceding.as_transition()
                        && let TransitionGuardNode::When(guard) = transition.guard
                    {
                        guards.push((guard, false));
                    }
                }
                if let TransitionGuardNode::When(guard) = statement.guard {
                    guards.push((guard, true));
                }
                arrivals.push(Arrival {
                    source: start + arrival.source,
                    destination: start + arrival.destination,
                    statement: arrival.statement_index,
                    arguments: arrival.arguments,
                    guards,
                    after_call: statement.continuation == arrival.target,
                });
            }
        }
        for call in calls {
            let caller = members.get(call.source)?;
            let body = program.machine_states(caller.machine);
            let ordinal = typed_trees::state::state_position(body, call.site_state.symbol)?;
            arrivals.push(Arrival {
                source: entries[call.source] + ordinal,
                destination: *entries.get(call.destination)?,
                statement: call.statement_ordinal,
                arguments: call.arguments,
                guards: call.guards.clone(),
                after_call: false,
            });
        }
        let mut endpoints = Vec::new();
        for (position, member) in members.iter().enumerate() {
            if !member.range.is_valid() {
                continue;
            }
            let range = program.expression_table.range_node(member.range)?;
            for expression in [range.start, range.end] {
                endpoint_reads(
                    program,
                    &states[entries[position]],
                    entries[position],
                    expression,
                    &mut endpoints,
                    0,
                )?;
            }
        }
        for origin in endpoints {
            conserve(program, &states, &arrivals, &origin, frames)?;
        }
        Some(())
    };
    prove().is_some()
}

fn endpoint_reads(
    program: &dyn crate::CheckingInput,
    frame: &StateFrame<'_>,
    state: usize,
    expression: ExpressionHandle,
    reads: &mut Vec<ValueCoordinate>,
    depth: usize,
) -> Option<()> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    match node {
        ExpressionNode::Integer(_) => {}
        ExpressionNode::Atomic(atomic) => {
            endpoint_reads(program, frame, state, atomic.value, reads, depth + 1)?;
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let collection = crate::value_custody::places::collection_length_receiver(
                program,
                frame.machine,
                Some(frame.state),
                expression,
            );
            // A fixed extent is declared by the carrier, not read from its
            // contents. Replacing elements or the entire array cannot change
            // this endpoint; runtime slice extents still need conservation.
            if collection.is_some_and(|receiver| {
                crate::value_custody::places::declared_collection_extent(
                    program,
                    frame.machine,
                    Some(frame.state),
                    receiver,
                )
                .is_some()
            }) {
                return Some(());
            }
            let receiver = collection.unwrap_or(expression);
            let place = crate::canonical_place_from_expression_in_state(
                program,
                frame.state.symbol,
                0,
                receiver,
            )?;
            let location = RankSubjectCoordinate {
                parameter: place.root.as_symbol()?,
                path: place.segments,
            };
            coordinate_kind(program, frame.state, &location)?;
            let value = ValueCoordinate { state, location };
            if !reads.contains(&value) {
                reads.push(value);
            }
        }
        ExpressionNode::Binary(binary) if super::super::dependency_binary(binary.operator) => {
            endpoint_reads(program, frame, state, binary.left, reads, depth + 1)?;
            endpoint_reads(program, frame, state, binary.right, reads, depth + 1)?;
        }
        _ => return None,
    }
    Some(())
}

#[derive(Clone, Copy)]
enum ValueKind {
    Integer,
    Length,
}

fn coordinate_kind(
    program: &dyn crate::CheckingInput,
    state: &State,
    coordinate: &RankSubjectCoordinate,
) -> Option<ValueKind> {
    let reference = super::super::ranking_coordinate_type(program, state, coordinate)?;
    if exact_integer_parameter(program, reference).is_some() {
        Some(ValueKind::Integer)
    } else if lengths::is_slice(program, reference) {
        Some(ValueKind::Length)
    } else {
        None
    }
}

fn conserve<'program>(
    program: &'program dyn crate::CheckingInput,
    states: &[StateFrame<'program>],
    arrivals: &[Arrival<'_>],
    origin: &ValueCoordinate,
    frames: Option<&crate::CallFrameResolver<'program>>,
) -> Option<()> {
    let mut domain = vec![origin.clone()];
    // Completed operands seed exact observed leaves, including captured copies.
    // A whole forwarded record is expanded only by a demanded suffix below.
    let mut completions = Vec::new();
    for arrival in arrivals {
        let source = &states[arrival.source];
        let destination = &states[arrival.destination];
        let parameters = program.state_parameters(destination.state);
        if parameters.len() != arrival.arguments.len() {
            return None;
        }
        let mut row = Vec::new();
        for (formal, actual) in parameters.iter().zip(arrival.arguments) {
            let values = arrival_correspondence::completed_operand_coordinates(
                program,
                source.machine,
                source.state,
                arrival.statement,
                *actual,
            )
            .unwrap_or_default();
            for value in &values {
                include_coordinate(
                    program,
                    states,
                    origin,
                    &mut domain,
                    ValueCoordinate {
                        state: arrival.source,
                        location: value.source.clone(),
                    },
                );
                include_coordinate(
                    program,
                    states,
                    origin,
                    &mut domain,
                    ValueCoordinate {
                        state: arrival.destination,
                        location: RankSubjectCoordinate {
                            parameter: formal.symbol,
                            path: value.relative_path.clone(),
                        },
                    },
                );
            }
            row.push(values);
        }
        completions.push(row);
    }
    // Demand-directed closure follows exact producer prefixes. An ancestry
    // check declines genuinely recurrent path growth, not acyclic alternatives.
    let mut pending = domain
        .iter()
        .cloned()
        .map(|value| (value.clone(), vec![value]))
        .collect::<Vec<_>>();
    while let Some((destination, ancestry)) = pending.pop() {
        for (arrival, completions) in arrivals.iter().zip(&completions) {
            if arrival.destination != destination.state {
                continue;
            }
            let parameters = program.state_parameters(states[arrival.destination].state);
            let position = parameters
                .iter()
                .position(|formal| formal.symbol == destination.location.parameter)?;
            for completion in &completions[position] {
                let Some(suffix) = destination
                    .location
                    .path
                    .strip_prefix(completion.relative_path.as_slice())
                else {
                    continue;
                };
                if !suffix.is_empty()
                    && completion.derivation
                        != checked_trees::CheckedRankValueDerivation::CapturedCoordinate
                {
                    continue;
                }
                let mut location = completion.source.clone();
                location.path.extend_from_slice(suffix);
                let source = ValueCoordinate {
                    state: arrival.source,
                    location,
                };
                if source.state == origin.state && source != *origin {
                    continue;
                }
                if coordinate_kind(program, states[source.state].state, &source.location).is_none()
                {
                    continue;
                }
                if ancestry.iter().any(|ancestor| {
                    ancestor.state == source.state
                        && ancestor.location.parameter == source.location.parameter
                        && source.location.path.len() > ancestor.location.path.len()
                        && source.location.path.starts_with(&ancestor.location.path)
                }) {
                    return None;
                }
                if !domain.contains(&source) {
                    domain.push(source.clone());
                    let mut ancestry = ancestry.clone();
                    ancestry.push(source.clone());
                    pending.push((source, ancestry));
                }
            }
        }
    }
    // Prove the equalities independently of this candidate relation. No role
    // boolean or lineage row enters an arithmetic hypothesis.
    let evidence = arrivals
        .iter()
        .map(|arrival| equality_edges(program, states, arrival, &domain, frames))
        .collect::<Option<Vec<_>>>()?;
    let mut retained = vec![true; domain.len()];
    loop {
        let mut changed = false;
        for (arrival, equalities) in arrivals.iter().zip(&evidence) {
            for (destination, value) in domain.iter().enumerate() {
                if !retained[destination] || value.state != arrival.destination {
                    continue;
                }
                if !equalities[destination]
                    .iter()
                    .any(|source| retained[*source])
                {
                    retained[destination] = false;
                    changed = true;
                }
            }
        }
        // A cycle-only conservation relation is not rooted at the authored
        // endpoint. Keep only productive equality transport from that origin.
        let mut reached = vec![false; domain.len()];
        reached[0] = retained[0];
        loop {
            let mut progress = false;
            for equalities in &evidence {
                for (destination, sources) in equalities.iter().enumerate() {
                    if retained[destination]
                        && !reached[destination]
                        && sources.iter().any(|source| reached[*source])
                    {
                        reached[destination] = true;
                        progress = true;
                    }
                }
            }
            if !progress {
                break;
            }
        }
        for (retained, reached) in retained.iter_mut().zip(reached) {
            if *retained && !reached {
                *retained = false;
                changed = true;
            }
        }
        if !retained[0]
            || arrivals.iter().any(|arrival| {
                !domain
                    .iter()
                    .zip(&retained)
                    .any(|(value, retained)| *retained && value.state == arrival.destination)
            })
        {
            return None;
        }
        if !changed {
            return Some(());
        }
    }
}

fn include_coordinate(
    program: &dyn crate::CheckingInput,
    states: &[StateFrame<'_>],
    origin: &ValueCoordinate,
    domain: &mut Vec<ValueCoordinate>,
    value: ValueCoordinate,
) {
    if (value.state != origin.state || value == *origin)
        && coordinate_kind(program, states[value.state].state, &value.location).is_some()
        && !domain.contains(&value)
    {
        domain.push(value);
    }
}

fn equality_edges<'program>(
    program: &'program dyn crate::CheckingInput,
    states: &[StateFrame<'program>],
    arrival: &Arrival<'_>,
    domain: &[ValueCoordinate],
    frames: Option<&crate::CallFrameResolver<'program>>,
) -> Option<Vec<Vec<usize>>> {
    let source = &states[arrival.source];
    let destination = &states[arrival.destination];
    let mut equalities = vec![Vec::new(); domain.len()];
    if arrival.after_call {
        return Some(equalities);
    }
    let bindings = integer_bindings(program, source.state)?;
    let mut engine = Engine::strict(program, source.machine, &bindings);
    if !engine.strict_symbol_bindings_are_valid() {
        return None;
    }
    let expressions = arrival
        .arguments
        .iter()
        .copied()
        .chain(arrival.guards.iter().map(|(guard, _)| *guard))
        .collect::<Vec<_>>();
    let mut fields = field_coordinates::FieldCoordinates::empty();
    fields.install(
        program,
        source.state,
        source.state,
        None,
        &mut engine,
        &expressions,
    )?;
    let lengths = lengths::bindings(program, source.machine, source.state, None);
    lengths::install(
        program,
        source.machine,
        source.state,
        source.state,
        &lengths,
        &mut engine,
        &expressions,
    )?;
    let mut slices = lengths::SliceCoordinates::empty();
    slices.install(
        program,
        source.machine,
        source.state,
        source.state,
        None,
        &mut engine,
        &expressions,
    )?;
    saved_arguments::install(program, source.machine, source.state, &lengths, &mut engine);
    let mut comparisons = parameter_comparisons(
        program,
        source.machine,
        source.state,
        &mut engine,
        &bindings,
    )?;
    comparisons.extend(fields.comparisons(program));
    comparisons.extend(slices.comparisons());
    comparisons.extend(super::super::length_bound_comparisons(&lengths));
    // Guard reads denote live values. When a prefix writes, decline its guard
    // hypotheses here rather than replaying them against arrival snapshots.
    let writes = prefix_frames(program, source, arrival.statement, frames)?;
    let empty_prefix = writes
        .iter()
        .all(|frame| frame.complete_paths().is_some_and(|paths| paths.is_empty()));
    if empty_prefix {
        for &(guard, holds) in &arrival.guards {
            meanings::admitted_carrier(program, source.machine, source.state, guard, 0)?;
            super::super::collect_guard(&mut engine, guard, holds, &mut comparisons, 0)?;
        }
        if source.state.symbol == program.machine_states(source.machine).first()?.symbol {
            comparisons.extend(super::super::entry_comparisons(
                program,
                source.machine,
                source.state,
                &mut engine,
                &bindings,
            )?);
        }
    }
    if !engine.install_hypotheses(comparisons) {
        return None;
    }
    let source_values = domain
        .iter()
        .map(|value| {
            if value.state != arrival.source {
                return None;
            }
            let observation = match coordinate_kind(program, source.state, &value.location)? {
                ValueKind::Integer => super::super::RankPremiseObservation::Value,
                ValueKind::Length => super::super::RankPremiseObservation::Length,
            };
            let protected = super::super::rank_protected_path(
                program,
                value.location.parameter,
                &value.location.path,
                observation,
            )?;
            if !writes.iter().all(|frame| {
                super::super::frame_preserves_rank_premises(
                    frame.clone(),
                    std::slice::from_ref(&protected),
                )
            }) {
                return None;
            }
            coordinate_value(
                program,
                source.state,
                &value.location,
                &bindings,
                &lengths,
                &mut engine,
            )
        })
        .collect::<Vec<_>>();
    let parameters = program.state_parameters(destination.state);
    for (position, value) in domain.iter().enumerate() {
        if value.state != arrival.destination {
            continue;
        }
        let ordinal = parameters
            .iter()
            .position(|formal| formal.symbol == value.location.parameter)?;
        let actual = *arrival.arguments.get(ordinal)?;
        meanings::admitted_carrier(program, source.machine, source.state, actual, 0)?;
        let carried = actual_value(
            program,
            source,
            destination.state,
            &value.location,
            actual,
            &lengths,
            &mut engine,
        );
        let Some(carried) = carried else {
            continue;
        };
        for (source_position, source_value) in source_values.iter().enumerate() {
            if source_value.as_ref().is_some_and(|source_value| {
                comparison_proven(&engine, BinaryOperator::Equal, &carried, source_value)
            }) {
                equalities[position].push(source_position);
            }
        }
    }
    Some(equalities)
}

fn coordinate_value(
    program: &dyn crate::CheckingInput,
    state: &State,
    coordinate: &RankSubjectCoordinate,
    bindings: &[super::super::super::StrictArithmeticSymbolBinding],
    lengths: &[(symbols::SymbolHandle, String)],
    engine: &mut Engine<'_>,
) -> Option<Polynomial> {
    match coordinate_kind(program, state, coordinate)? {
        ValueKind::Integer if coordinate.path.is_empty() => {
            let binding = bindings
                .iter()
                .find(|binding| binding.symbol == coordinate.parameter)?;
            match &binding.value {
                super::super::super::StrictArithmeticBindingValue::Atom { identity, .. } => {
                    Some(Polynomial::atom(identity.clone()))
                }
                super::super::super::StrictArithmeticBindingValue::Integer(value) => {
                    Some(Polynomial::constant(value.clone()))
                }
            }
        }
        ValueKind::Length if coordinate.path.is_empty() => lengths
            .iter()
            .find(|(symbol, _)| *symbol == coordinate.parameter)
            .map(|(_, identity)| Polynomial::atom(identity.clone())),
        ValueKind::Integer => {
            let field = fields::FieldCoordinate::at_coordinate(
                program,
                state,
                coordinate,
                CoordinateKind::Integer,
            )?;
            engine
                .install_hypotheses(field.field_comparisons(program))
                .then(|| field.value())
        }
        ValueKind::Length => Some(
            lengths::SliceCoordinate::at_coordinate(
                program,
                state,
                coordinate,
                CoordinateKind::Length,
            )?
            .value(),
        ),
    }
}

fn actual_value(
    program: &dyn crate::CheckingInput,
    source: &StateFrame<'_>,
    destination: &State,
    coordinate: &RankSubjectCoordinate,
    actual: ExpressionHandle,
    lengths: &[(symbols::SymbolHandle, String)],
    engine: &mut Engine<'_>,
) -> Option<Polynomial> {
    match coordinate_kind(program, destination, coordinate)? {
        ValueKind::Integer if coordinate.path.is_empty() => {
            meanings::install_nonpolynomial_terms(
                program,
                source.machine,
                source.state,
                engine,
                actual,
                0,
            )?;
            engine.normalize(actual)
        }
        ValueKind::Length if coordinate.path.is_empty() => lengths::actual(
            program,
            source.machine,
            source.state,
            actual,
            lengths,
            engine,
        ),
        ValueKind::Integer => {
            let field = fields::FieldCoordinate::at_coordinate(
                program,
                destination,
                coordinate,
                CoordinateKind::Integer,
            )?;
            field.field_actual(program, source.machine, source.state, engine, actual)
        }
        ValueKind::Length => {
            let slice = lengths::SliceCoordinate::at_coordinate(
                program,
                destination,
                coordinate,
                CoordinateKind::Length,
            )?;
            slice.arrived_length(
                program,
                source.machine,
                source.state,
                engine,
                actual,
                slice.borrowed,
                lengths,
            )
        }
    }
}

fn prefix_frames<'program>(
    program: &'program dyn crate::CheckingInput,
    source: &StateFrame<'program>,
    through: usize,
    frames: Option<&crate::CallFrameResolver<'program>>,
) -> Option<Vec<NormalizedWriteFrame>> {
    let frames = frames?;
    let mut writes = Vec::new();
    for statement in program
        .statement_table
        .statements(source.state.statement_nodes)
        .iter()
        .take(through + 1)
    {
        // Operand effects retain their own complete storage coordinates.
        let values = frames.statement_value_write_frame(source.machine, statement);
        values.complete_paths()?;
        writes.push(values);
        let own = match statement {
            StatementNode::Assignment(_) => {
                Some(frames.assignment_write_frame(source.machine, statement))
            }
            StatementNode::Call(call) => Some(frames.may_write_frame(source.machine, call)),
            _ => None,
        };
        if let Some(frame) = own {
            frame.complete_paths()?;
            writes.push(frame);
        }
    }
    Some(writes)
}
