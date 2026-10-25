//! Explicit state inputs derived from live edge contexts, not declarations.
use validation::CallSite;

use crate::flow::FlowBuildContext;

use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowCallFact, FlowConstraintRef, FlowOperandFact, FlowSemanticContextRef};
use facts::ScalarValue;
use facts::{FactPayload, FactPlace, FactPlan, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::statement::{TransitionExit, TransitionTargetNode};
use validation::CanonicalPlace;

mod arguments;
pub(super) mod lexical_regions;
mod parameter_bounds;
mod predicates;
use parameter_bounds::ParameterBounds;
pub(super) mod fields;
pub(in crate::flow) mod qualifications;
pub(super) use arguments::capture_argument;

/// A missing row is unreachable. A present unknown value must
/// participate in the incoming meet; it is never an omitted predecessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StateValues {
    state: SymbolHandle,
    values: Vec<(SymbolHandle, ScalarValue)>,
    /// The integer interval every edge proves for the parameter, `None` when
    /// this edge delivers none. The join keeps the UNION: the destination
    /// only knows the argument lies inside the widest delivered range.
    /// `put(35) | put(46)` joins `ch` to `[35, 46]`, which is how an
    /// all-ASCII glyph table keeps a `u8` byte store provably in-class.
    bounds: Vec<ParameterBounds>,
    fields: Vec<fields::FieldValue>,
    qualifications: Vec<qualifications::QualifiedInput>,
    predicates: Vec<crate::facts::boolean_predicates::BoundPredicate>,
    entry_paths: crate::facts::boolean_predicates::EntryPaths,
    /// Selected cases of internal state arrivals. Scalars join through `values`.
    argument_edges: ArgumentInputs,
    /// Cases every recorded edge agrees on, keyed by the destination parameter.
    cases: Vec<(SymbolHandle, SymbolHandle)>,
}

/// One internal state-arrival edge's selected-case observations.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ArgumentEdge {
    source: fields::BoundsSource,
    cases: Vec<(SymbolHandle, SymbolHandle)>,
}

/// Unreached inputs collect edges; a conservative fallback already represents
/// every possible predecessor. Unknown is absorbing, not an empty edge list
/// that a later visit may narrow to the predecessors encountered so far.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ArgumentInputs {
    Unknown,
    Edges(Vec<ArgumentEdge>),
}

/// The greatest predicate set this state's incoming edges have ever jointly
/// delivered for `root`/`segments` -- the co-inductive premise an element-store
/// reseed may assume for the carrier. Empty when no edge has established any.
///
/// `root` is the destination-side carrier identity: the machine symbol for an
/// attached `self` field, or the destination parameter's own symbol for a
/// `&mut`/shared argument's storage.
pub(super) fn field_predicate_ceiling<'a>(
    build: &'a FlowBuildContext<'_>,
    state: SymbolHandle,
    root: facts::PlaceRoot,
    segments: &[facts::PlaceSegment],
) -> &'a [validation::field_domains::ByteSequencePredicate] {
    let field = build
        .state_value_inputs
        .iter()
        .find(|row| row.state == state)
        .and_then(|row| {
            row.fields
                .iter()
                .find(|field| field.root() == root && field.segments() == segments)
        });
    field.map_or(&[], |field| field.predicate_ceiling())
}

pub(super) fn entry_state_index(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<usize> {
    validation::activation_target_state_index(program, machine, machine.symbol)
}

fn entry_state<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<&'program typed_trees::state::State> {
    program
        .machine_states(machine)
        .get(entry_state_index(program, machine)?)
}

fn reachable(
    build: &FlowBuildContext,
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: SymbolHandle,
) -> bool {
    entry_state(program, machine).is_some_and(|entry| entry.symbol == state)
        || build
            .state_value_inputs
            .iter()
            .any(|input| input.state == state)
}

fn join(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    destination: &typed_trees::state::State,
    source: fields::BoundsSource,
    incoming: StateValues,
) {
    let state = incoming.state;
    let mut changed = false;
    // The widening thresholds are program-pure: fetch the context's memoized
    // set before borrowing the row so the arena scan runs at most once per
    // flow build, not once per rejoin.
    let thresholds = build.integer_literal_thresholds_at(program);
    if let Some(previous) = build
        .state_value_inputs
        .iter_mut()
        .find(|row| row.state == incoming.state)
    {
        changed |= fields::meet(
            program,
            machine,
            destination,
            &mut previous.fields,
            &incoming.fields,
            source,
            thresholds.as_slice(),
        );
        changed |= qualifications::meet(&mut previous.qualifications, &incoming.qualifications);
        changed |= predicates::meet(&mut previous.predicates, &incoming.predicates);
        if previous.entry_paths.union(&incoming.entry_paths) {
            changed = true;
            // Each change adds an arrival clause or removes a stricter one
            // over the finite original-entry guard atoms; no syntax unrolling.
            build.new_state_field_input_height += 1;
        }
        for (parameter, value) in &mut previous.values {
            let next = incoming
                .values
                .iter()
                .find(|row| row.0 == *parameter)
                .map(|row| row.1.clone())
                .unwrap_or_default();
            if *value != next {
                changed |= *value != ScalarValue::Unknown;
                *value = ScalarValue::Unknown;
            }
        }
        for bound in &mut previous.bounds {
            let next = incoming
                .bounds
                .iter()
                .find(|row| row.symbol == bound.symbol)
                .and_then(|row| row.bounds.as_ref());
            changed |= bound.join(program, destination, next, thresholds.as_slice());
        }
    } else {
        changed = true;
        build.new_state_field_input_height += fields::height(&incoming.fields, thresholds.len());
        build.new_state_field_input_height += incoming.predicates.len();
        build.new_state_field_input_height = build
            .new_state_field_input_height
            .saturating_add(qualifications::height(program, &incoming.qualifications));
        build.new_state_field_input_height = incoming
            .bounds
            .iter()
            .fold(build.new_state_field_input_height, |height, bound| {
                height.saturating_add(bound.height(thresholds.len()))
            });
        let mut incoming = incoming;
        for field in &mut incoming.fields {
            field.seed_delivery(source);
        }
        build.state_value_inputs.push(incoming);
    }
    if changed && !build.dirty_state_value_inputs.contains(&state) {
        build.dirty_state_value_inputs.push(state);
    }
}

/// Unknown inputs for every state of the machines at roster `positions`.
pub(super) fn unknown_inputs(
    program: &dyn ::validation::CheckingInput,
    positions: &[usize],
) -> Vec<StateValues> {
    positions
        .iter()
        .map(|position| &program.machines()[*position])
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateValues {
                    state: state.symbol,
                    fields: Vec::new(),
                    qualifications: Vec::new(),
                    predicates: Vec::new(),
                    entry_paths: crate::facts::boolean_predicates::EntryPaths::unknown(),
                    values: program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                        .collect(),
                    bounds: Vec::new(),
                    argument_edges: ArgumentInputs::Unknown,
                    cases: Vec::new(),
                })
        })
        .collect()
}

pub(crate) fn literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
        ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_transition(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition: &typed_trees::statement::TableTransition,
    target: typed_trees::statement::TransitionTargetHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
    captured_operands: &[FlowOperandFact],
    argument_values: &[ScalarValue],
    argument_qualifications: Vec<qualifications::QualifiedInput>,
) {
    if transition.exit != TransitionExit::Ordinary
        || !reachable(build, program, machine, state.symbol)
    {
        return;
    }
    let (destination, arguments) = match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let Some(destination) = build
                .state_index_in_machine(program, machine.symbol, path.symbol)
                .and_then(|index| program.machine_states(machine).get(index))
            else {
                return;
            };
            (
                destination,
                program.statement_table.expression_handles(*arguments),
            )
        }
        _ => return,
    };
    if entry_state(program, machine).is_some_and(|entry| entry.symbol == destination.symbol) {
        return;
    }
    let mut ordinal = 0;
    let mut values = Vec::new();
    let mut bounds = Vec::new();
    for parameter in program.state_parameters(destination) {
        let captured = argument_values.get(ordinal);
        let argument = arguments.get(ordinal);
        ordinal += 1;
        let scalar = program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| {
                primitive.accepts_integer_literal()
                    || primitive == typed_trees::types::PrimitiveType::Bool
            });
        let value = if !scalar {
            ScalarValue::Unknown
        } else {
            captured.cloned().unwrap_or_default()
        };
        // The integer interval this edge delivers for the parameter: a
        // literal or captured value is a point range, and a place argument
        // forwards the bounds its live facts prove. No evidence, no bound --
        // an edge that proves nothing narrows the joined interval to nothing.
        let bound = match &value {
            ScalarValue::Integer(value) => Some(facts::IntegerRange {
                minimum: value.clone(),
                maximum: value.clone(),
            }),
            _ => argument
                .and_then(|argument| {
                    build.canonical_place_at(program, state.symbol, statement_index, *argument)
                })
                .and_then(|place| {
                    build.integer_bounds_at_place(program, semantic, contexts, &place)
                }),
        };
        values.push((parameter.symbol, value));
        bounds.push(ParameterBounds::new(parameter.symbol, bound));
    }
    let fields = fields::capture(
        program,
        semantic,
        build,
        machine,
        state,
        destination,
        statement_index,
        arguments,
        contexts,
    );
    let predicates = predicates::capture(
        program,
        semantic,
        build,
        state,
        destination,
        statement_index,
        arguments,
        contexts,
    );
    let path_contexts = crate::flow::active_context_handles(build, contexts);
    let entry_paths = crate::facts::boolean_predicates::EntryPaths::retained(
        semantic,
        &path_contexts,
        machine.symbol,
    );
    let mut qualifications = argument_qualifications;
    qualifications.extend(qualifications::capture_invocation_referents(
        program,
        semantic,
        build,
        machine,
        state,
        statement_index,
        contexts,
    ));
    join(
        program,
        build,
        machine,
        destination,
        fields::BoundsSource::transition(state.symbol, target),
        StateValues {
            state: destination.symbol,
            values,
            bounds,
            fields,
            qualifications,
            predicates,
            entry_paths,
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        },
    );
    let cases = capture_argument_cases(
        program,
        semantic,
        build,
        machine,
        state,
        destination,
        arguments,
        captured_operands,
    );
    deliver_argument_edge(
        program,
        build,
        destination,
        fields::BoundsSource::transition(state.symbol, target),
        cases,
    );
}

/// Ordinary invocations are not narrowed by the internal transition proof.
pub(super) fn record_invocation<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
) {
    if !reachable(build, program, machine, state.symbol) {
        return;
    }
    let Some((owner, destination)) =
        build
            .state_location(program, call.target_symbol)
            .map(|(machine_index, state_index)| {
                let owner = &program.machines()[machine_index];
                (owner, &program.machine_states(owner)[state_index])
            })
    else {
        return;
    };
    if entry_state(program, owner).is_some_and(|entry| entry.symbol == destination.symbol) {
        return;
    }
    if owner.symbol == machine.symbol
        && matches!(
            super::calls::memoized_find_call_site(
                program,
                build,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal
            ),
            Some(CallSite::TransitionNamed { .. })
        )
    {
        return;
    }
    join(
        program,
        build,
        owner,
        destination,
        fields::BoundsSource::invocation(state.symbol, call.statement_index, call.call_ordinal),
        StateValues {
            state: destination.symbol,
            fields: Vec::new(),
            qualifications: Vec::new(),
            predicates: Vec::new(),
            entry_paths: crate::facts::boolean_predicates::EntryPaths::unknown(),
            bounds: Vec::new(),
            values: program
                .state_parameters(destination)
                .iter()
                .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                .collect(),
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        },
    );
    // An ordinary invocation of an internal state contributes an unknown case
    // edge; another predecessor's selected tag cannot survive that arrival.
    deliver_argument_edge(
        program,
        build,
        destination,
        fields::BoundsSource::invocation(state.symbol, call.statement_index, call.call_ordinal),
        Vec::new(),
    );
}

/// Deliver what each operand completed, not what its source holds after the
/// other operands ran. Named jumps use the same capture law
/// as contract checking: detached values keep completion facts; views consult
/// their exact live referents at invocation. Missing captures remain unknown
/// incoming edges and never disappear from a destination's join.
fn capture_argument_cases(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
    captured: &[FlowOperandFact],
) -> Vec<(SymbolHandle, SymbolHandle)> {
    let parameters = program.state_parameters(destination);
    let mut cases = Vec::new();
    if parameters.len() != arguments.len()
        || !captured
            .iter()
            .map(|capture| capture.expression)
            .eq(arguments.iter().copied())
        || captured
            .iter()
            .any(|capture| capture.state_symbol != state.symbol)
    {
        return cases;
    }
    let invocation = captured
        .last()
        .map_or(HandleSpan::<FlowConstraintRef>::empty(), |capture| {
            capture.constraints
        });
    for (parameter, capture) in parameters.iter().zip(captured) {
        let contexts = super::operand_contexts(
            program,
            &build.contexts,
            &build.control,
            semantic,
            capture,
            parameter.type_reference,
            invocation,
        );
        let Some(places) = argument_places(
            program,
            build,
            machine,
            state,
            capture,
            parameter.type_reference,
        ) else {
            continue;
        };
        if let Some(variant) = places
            .iter()
            .find_map(|place| assigned_case_at(program, semantic, &contexts, place))
        {
            cases.push((parameter.symbol, variant));
        };
    }
    cases.sort_by_key(|(parameter, variant)| {
        (
            parameter.arena_index(),
            parameter.generation(),
            variant.arena_index(),
            variant.generation(),
        )
    });
    cases
}

/// Exact completed-value or live-referent namespaces shared by arrival
/// consumers. The caller selects the matching operand contexts independently.
fn argument_places(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    capture: &FlowOperandFact,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<Vec<CanonicalPlace>> {
    let view = super::operand_binds_live_view(program, reference);
    let mut places = Vec::new();
    if !view {
        places.push(CanonicalPlace {
            root: facts::PlaceRoot::Expression(capture.expression),
            segments: Vec::new(),
        });
    }
    let storage_place = if view {
        match build
            .control
            .operand_referents
            .span_or_empty(capture.referents)
        {
            [referent] => build
                .control
                .operand_referent_segments
                .span(referent.segments)
                .map(|segments| CanonicalPlace {
                    root: referent.root,
                    segments: segments.to_vec(),
                }),
            _ => None,
        }
    } else if validation::value_has_builtin_coordinates(program, machine, state, capture.expression)
    {
        build.canonical_place_at(
            program,
            capture.state_symbol,
            capture.statement_index,
            capture.expression,
        )
    } else {
        None
    };
    places.extend(storage_place);
    (!places.is_empty()).then_some(places)
}

fn assigned_case_at(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    place: &CanonicalPlace,
) -> Option<SymbolHandle> {
    let mut found = None;
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        let FactPayload::AssignedCase { variant } = fact.payload else {
            continue;
        };
        if !variant.is_valid() || program.symbols.get(variant).kind != symbols::SymbolKind::Variant
        {
            continue;
        }
        let Some(handle) = fact.place.as_place() else {
            continue;
        };
        let Some(fact_place) = crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(handle),
        ) else {
            continue;
        };
        if fact_place != *place {
            continue;
        }
        if found.is_some_and(|existing| existing != variant) {
            return None;
        }
        found = Some(variant);
    }
    found
}

fn deliver_argument_edge(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    destination: &typed_trees::state::State,
    source: fields::BoundsSource,
    cases: Vec<(SymbolHandle, SymbolHandle)>,
) {
    let index = ensure_input_row(program, build, destination);
    let row = &mut build.state_value_inputs[index];
    let ArgumentInputs::Edges(edges) = &mut row.argument_edges else {
        return;
    };
    let mut added = false;
    if let Some(edge) = edges.iter_mut().find(|edge| edge.source == source) {
        if edge.cases == cases {
            return;
        }
        edge.cases = cases;
    } else {
        edges.push(ArgumentEdge { source, cases });
        added = true;
    }
    let cases = joined_cases(edges);
    if added {
        build.new_state_field_input_height = build.new_state_field_input_height.saturating_add(1);
    }
    let row = &mut build.state_value_inputs[index];
    let changed = cases != row.cases;
    row.cases = cases;
    let state = destination.symbol;
    if changed && !build.dirty_state_value_inputs.contains(&state) {
        build.dirty_state_value_inputs.push(state);
    }
}

fn ensure_input_row(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    destination: &typed_trees::state::State,
) -> usize {
    if let Some(index) = build
        .state_value_inputs
        .iter()
        .position(|row| row.state == destination.symbol)
    {
        return index;
    }
    build.state_value_inputs.push(StateValues {
        state: destination.symbol,
        values: program
            .state_parameters(destination)
            .iter()
            .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
            .collect(),
        bounds: Vec::new(),
        fields: Vec::new(),
        qualifications: Vec::new(),
        predicates: Vec::new(),
        entry_paths: crate::facts::boolean_predicates::EntryPaths::unknown(),
        argument_edges: ArgumentInputs::Edges(Vec::new()),
        cases: Vec::new(),
    });
    build.state_value_inputs.len() - 1
}

fn joined_cases(edges: &[ArgumentEdge]) -> Vec<(SymbolHandle, SymbolHandle)> {
    let Some(first) = edges.first() else {
        return Vec::new();
    };
    first
        .cases
        .iter()
        .copied()
        .filter(|case| edges.iter().all(|edge| edge.cases.contains(case)))
        .collect()
}

pub(super) fn append_entry_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) {
    // A generic machine body is checked under its declared premises. Actual
    // call operands belong to caller substitution, never ambient entry proof.
    // Extracted lexical regions instead receive their authenticated activation
    // captures, just as the original source region did before extraction.
    if machine.lexical_region_origin.is_none()
        && entry_state(program, machine).is_some_and(|entry| entry.symbol == state.symbol)
    {
        return;
    }
    let Some(input) = build
        .state_value_inputs
        .iter()
        .find(|input| input.state == state.symbol)
    else {
        return;
    };
    let point = ProgramPoint::State {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
    };
    fields::append(program, semantic, &input.fields, machine, state, point);
    qualifications::append(semantic, &input.qualifications, point);
    predicates::append(semantic, &input.predicates, point);
    if let Some(entry) = validation::activation_target_state_index(program, machine, machine.symbol)
        .and_then(|index| program.machine_states(machine).get(index))
    {
        crate::facts::boolean_predicates::seed_entry_paths(
            semantic,
            machine.symbol,
            entry.symbol,
            point,
            &input.entry_paths,
        );
    }
    for (parameter, value) in &input.values {
        if *value == ScalarValue::Unknown {
            continue;
        }
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(*parameter);
        let value = semantic.scalar_values.append(value.clone());
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::AssignedScalarValue { value },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
    for parameter in &input.bounds {
        let Some(bounds) = &parameter.bounds else {
            continue;
        };
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(parameter.symbol);
        let bounds = semantic.integer_ranges.append(bounds.clone());
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::AssignedIntegerBounds { bounds },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
    for (parameter, variant) in &input.cases {
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(*parameter);
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::AssignedCase { variant: *variant },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
}
