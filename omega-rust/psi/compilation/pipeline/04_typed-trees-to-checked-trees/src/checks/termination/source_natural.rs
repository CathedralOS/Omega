//! Source-only natural coordinates for logical invocations. Selecting a
//! logical body is only operational eligibility: this owner independently
//! proves a natural floor and strict decrease at every recurrent call site.
//! No theorem conclusion, runtime parameter position or fabricated frame is
//! used. Internal state cycles cannot decrease an immutable machine binder.

use super::{
    graph,
    order::{OrderResolution, RankingOrder},
};
use crate::checks::current_scalar_values::CurrentScalarValues;
use checked_trees::{CheckedSourceNaturalRankEdge, CheckedSourceNaturalRankMember};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument},
    machine::Machine,
    types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
};
use validation::{CallSite, ScopedArithmeticValue};

pub(super) struct Component {
    pub members: Vec<CheckedSourceNaturalRankMember>,
    pub edges: Vec<CheckedSourceNaturalRankEdge>,
}
struct Coordinate {
    subject: ExpressionHandle,
    carrier: TypeReferenceHandle,
    primitive: PrimitiveType,
    binder_position: Option<usize>,
    closed: Option<BigInt>,
    realized_from: Option<SymbolHandle>,
}

/// None leaves the ordinary executable/structural ranking owner in charge.
/// An applicable source component returns an actual checked result, including
/// a precise failure; profile membership alone never supplies that result.
pub(super) fn admit(
    program: &dyn validation::CheckingInput,
    positions: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Result<Component, String>> {
    let machines = positions
        .iter()
        .map(|position| &program.machines()[*position])
        .collect::<Vec<_>>();
    if machines.is_empty()
        || !machines.iter().all(|machine| {
            validation::mathematical_applications::is_indexed_logical_machine(program, machine)
        })
    {
        return None;
    }
    let coordinates = machines
        .iter()
        .map(|machine| coordinate(program, machine, 0))
        .collect::<Option<Vec<_>>>()?;
    Some(admit_coordinates(program, &machines, &coordinates, frames))
}

fn admit_coordinates(
    program: &dyn validation::CheckingInput,
    machines: &[&Machine],
    coordinates: &[Coordinate],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<Component, String> {
    if !coordinates
        .iter()
        .all(|coordinate| coordinate.primitive == coordinates[0].primitive)
    {
        return Err("logical recursive members do not share one exact natural carrier".into());
    }
    let mut members = Vec::new();
    let mut edges = Vec::new();
    for (machine, rank) in machines.iter().zip(coordinates) {
        if internal_cycle(program, machine) {
            return Err(format!(
                "immutable source rank in `{}` cannot decrease around an internal state cycle",
                machine.name
            ));
        }
        members.push(CheckedSourceNaturalRankMember {
            machine: machine.symbol,
            body: machine.states,
            subject: rank.subject,
            carrier: rank.carrier,
            realized_from: rank.realized_from,
            realized_parameters: if let Some(original) = rank.realized_from {
                realized_forwarding(program, machine, original, rank.carrier)?
            } else {
                Vec::new()
            },
        });
        for state in program.machine_states(machine) {
            for ordinal in 0..program
                .statement_table
                .statements(state.statement_nodes)
                .len()
            {
                let calls =
                    validation::collect_call_sites_in_statement(program, machine, state, ordinal);
                for (call_ordinal, site) in calls.iter().enumerate() {
                    let (target, application, fallback, ordinary, reentry) = match site {
                        CallSite::Statement(call) => (
                            call.target_symbol,
                            call.authored_machine_application,
                            call.machine_arguments.as_ref(),
                            program.statement_table.expression_handles(call.arguments),
                            false,
                        ),
                        CallSite::Expression { call, .. } => (
                            validation::call_target_symbol(program, call),
                            call.authored_machine_application,
                            call.machine_arguments.as_ref(),
                            program.expression_table.expression_handles(call.arguments),
                            false,
                        ),
                        CallSite::TransitionNamed {
                            path, arguments, ..
                        } => {
                            if program
                                .machine_states(machine)
                                .first()
                                .is_some_and(|entry| entry.symbol == path.symbol)
                            {
                                // Re-entering this invocation preserves its
                                // immutable machine operands. It still owes
                                // strict descent (and normally rejects).
                                (
                                    machine.symbol,
                                    Default::default(),
                                    &[][..],
                                    program.statement_table.expression_handles(*arguments),
                                    true,
                                )
                            } else {
                                // Other own-state jumps are covered by the
                                // independently checked acyclic state graph.
                                continue;
                            }
                        }
                    };
                    let Some(callee_position) = machines.iter().position(|callee| {
                        callee.symbol == target
                            || program
                                .machine_states(callee)
                                .first()
                                .is_some_and(|entry| entry.symbol == target)
                    }) else {
                        if program.machine_holding_state(target).is_some_and(|owner| {
                            owner.symbol != machine.symbol
                                && machines.iter().any(|member| member.symbol == owner.symbol)
                        }) {
                            return Err(
                                "recursive source invocation enters an unbound non-entry state"
                                    .into(),
                            );
                        }
                        continue;
                    };
                    let callee = machines[callee_position];
                    let destination = &coordinates[callee_position];
                    let (source_callee, arguments) = if application.is_valid() {
                        let selected = program
                            .expression_table
                            .static_machine_application(application);
                        (
                            program
                                .machine_by_symbol(selected.registration_operation)
                                .or_else(|| {
                                    program.machine_holding_state(selected.registration_operation)
                                }),
                            program
                                .expression_table
                                .static_machine_application_arguments(application),
                        )
                    } else {
                        (Some(callee), fallback)
                    };
                    let source_callee = source_callee.ok_or_else(|| {
                        "recursive source invocation has no exact authored operation".to_string()
                    })?;
                    if !same_source_target(program, callee, source_callee.symbol) {
                        return Err("recursive invocation and selected specialization have different source owners".into());
                    }
                    let source_rank = coordinate(program, source_callee, 0).ok_or_else(|| {
                        "recursive source operation has no exact natural coordinate".to_string()
                    })?;
                    if source_rank.primitive != rank.primitive {
                        return Err(
                            "recursive static argument changes the source rank carrier".into()
                        );
                    }
                    if let Some(original) = destination.realized_from {
                        let entry = program
                            .machine_states(callee)
                            .first()
                            .ok_or_else(|| "recursive realized callee has no entry".to_string())?;
                        let (position, _) =
                            realized_parameter(program, entry, original, destination.carrier)
                                .ok_or_else(|| {
                                    "recursive realized callee lost its exact value formal"
                                        .to_string()
                                })?;
                        let source_position = source_rank.binder_position.ok_or_else(|| {
                            "recursive realized value has no authored source position".to_string()
                        })?;
                        let agrees = arguments
                            .get(source_position)
                            .zip(ordinary.get(position))
                            .is_some_and(|(source, actual)| {
                                source_actual_matches_argument(program, source, *actual)
                            });
                        if !agrees {
                            return Err("recursive ordinary Value operand does not match its exact authored source actual".into());
                        }
                    }
                    let target_edge = validation::transition_call_target(
                        program,
                        machine,
                        state,
                        ordinal,
                        call_ordinal,
                    )
                    .unwrap_or_default();
                    let values = CurrentScalarValues::at(
                        program,
                        machine,
                        state,
                        ordinal,
                        target_edge,
                        frames,
                    )
                    .ok_or_else(|| {
                        "recursive call has no stable caller prefix for its ranking".to_string()
                    })?;
                    let current = if let Some(original) = rank.realized_from {
                        let (_, parameter) =
                            realized_parameter(program, state, original, rank.carrier).ok_or_else(
                                || "source rank lost its exact state realization".to_string(),
                            )?;
                        values.current_symbol(parameter)
                    } else {
                        values
                            .current(rank.subject)
                            .map(ScopedArithmeticValue::Term)
                    }
                    .ok_or_else(|| {
                        format!(
                            "source ranking subject in `{}` has no admitted current value",
                            machine.name
                        )
                    })?;
                    let next = if reentry {
                        current.clone()
                    } else if let Some(position) = source_rank.binder_position {
                        static_value(
                            program,
                            &values,
                            arguments.get(position).ok_or_else(|| {
                                "recursive invocation lost its static rank operand".to_string()
                            })?,
                        )
                        .ok_or_else(|| {
                            "recursive static rank operand has no admitted value".to_string()
                        })?
                    } else {
                        ScopedArithmeticValue::Integer(source_rank.closed.clone().ok_or_else(
                            || "recursive source rank has no checked value".to_string(),
                        )?)
                    };
                    let zero = ScopedArithmeticValue::Integer(BigInt::zero());
                    let proves =
                        |operator, left: &ScopedArithmeticValue, right: &ScopedArithmeticValue| {
                            values.proves_values_relation(program, machine, operator, left, right)
                        };
                    if let Some(closed) = &destination.closed {
                        if !proves(
                            BinaryOperator::Equal,
                            &next,
                            &ScopedArithmeticValue::Integer(closed.clone()),
                        ) {
                            return Err("recursive static operand does not equal the selected closed ranking coordinate".into());
                        }
                    }
                    if !proves(BinaryOperator::GreaterOrEqual, &current, &zero)
                        || !proves(BinaryOperator::GreaterOrEqual, &next, &zero)
                        || !proves(BinaryOperator::Less, &next, &current)
                    {
                        return Err(format!(
                            "cannot prove independent strict source-rank descent for `{}` at state `{}` statement {} call {}",
                            machine.name, state.name, ordinal, call_ordinal
                        ));
                    }
                    edges.push(CheckedSourceNaturalRankEdge {
                        caller: machine.symbol,
                        callee: callee.symbol,
                        source_callee: source_callee.symbol,
                        state: state.symbol,
                        statement_ordinal: u32::try_from(ordinal)
                            .map_err(|_| "source statement ordinal exceeds storage")?,
                        call_ordinal: u32::try_from(call_ordinal)
                            .map_err(|_| "source call ordinal exceeds storage")?,
                        authored_application: application,
                        argument_position: (!reentry)
                            .then_some(source_rank.binder_position)
                            .flatten()
                            .map(u32::try_from)
                            .transpose()
                            .map_err(|_| "static rank position exceeds storage")?,
                    });
                }
            }
        }
    }
    if edges.is_empty()
        || machines
            .iter()
            .any(|machine| !edges.iter().any(|edge| edge.caller == machine.symbol))
    {
        return Err("source recursive component has an incomplete exact call inventory".into());
    }
    Ok(Component { members, edges })
}

/// Source state procedures can be invoked by statements or nested expressions
/// as well as transition targets. The immutable machine rank cannot discharge
/// a cycle hidden in any of those ordinary call positions.
fn internal_cycle(program: &dyn validation::CheckingInput, machine: &Machine) -> bool {
    let states = program.machine_states(machine);
    let mut adjacency = graph::machine_adjacency(program, machine);
    for (source, state) in states.iter().enumerate() {
        for ordinal in 0..program
            .statement_table
            .statements(state.statement_nodes)
            .len()
        {
            for call in
                validation::collect_call_sites_in_statement(program, machine, state, ordinal)
            {
                let target = match call {
                    CallSite::Statement(call) => call.target_symbol,
                    CallSite::Expression { call, .. } => {
                        validation::call_target_symbol(program, call)
                    }
                    CallSite::TransitionNamed { path, .. } => path.symbol,
                };
                if let Some(destination) =
                    validation::internal_state_index(program, machine, target)
                {
                    adjacency[source].push(destination);
                }
            }
        }
    }
    graph::strongly_connected_components(&adjacency)
        .iter()
        .any(|component| graph::component_is_cyclic(&adjacency, component))
}

fn same_source_target(
    program: &dyn validation::CheckingInput,
    selected: &Machine,
    source: SymbolHandle,
) -> bool {
    selected.symbol == source
        || program
            .typed_program()
            .executable_application_template(selected.symbol)
            .ok()
            .flatten()
            == Some(source)
}

fn coordinate(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    depth: usize,
) -> Option<Coordinate> {
    if depth > 16 {
        return None;
    }
    let entry = program.machine_states(machine).first()?;
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    if witness.rank_range.is_some() {
        return None;
    }
    let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
    let [subject] = subjects.as_slice() else {
        return None;
    };
    let arguments = typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
    if !matches!(
        RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments),
        OrderResolution::Resolved(RankingOrder::NatDescending)
    ) {
        return None;
    }
    match program.expression_table.expression(*subject) {
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.symbol == path.head_symbol => {
            let parameter = program
                .machine_type_parameters(machine)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == path.symbol);
            let Some((position, parameter)) = parameter else {
                return realized_coordinate(program, machine, *subject, path.symbol);
            };
            let carrier = match parameter.kind {
                typed_trees::data::TypeParameterKind::Value { type_reference }
                | typed_trees::data::TypeParameterKind::Const { type_reference } => type_reference,
                _ => return None,
            };
            let primitive = program.primitive_type_reference(carrier)?;
            matches!(
                primitive,
                typed_trees::unsigned_integer_primitive_pattern!()
            )
            .then_some(Coordinate {
                subject: *subject,
                carrier,
                primitive,
                binder_position: Some(position),
                closed: None,
                realized_from: None,
            })
        }
        ExpressionNode::Integer(literal) => {
            let template = program
                .typed_program()
                .executable_application_template(machine.symbol)
                .ok()
                .flatten()?;
            if template == machine.symbol {
                return None;
            }
            let source = coordinate(program, program.machine_by_symbol(template)?, depth + 1)?;
            Some(Coordinate {
                subject: *subject,
                carrier: source.carrier,
                primitive: source.primitive,
                binder_position: None,
                closed: Some(literal.value_bignum()?),
                realized_from: None,
            })
        }
        _ => None,
    }
}

fn source_actual_matches_argument(
    program: &dyn validation::CheckingInput,
    source: &StaticMachineArgument,
    actual: ExpressionHandle,
) -> bool {
    if source.type_reference.is_valid() {
        return matches!(program.type_reference_table.type_reference(source.type_reference),
            TypeReferenceNode::ConstExpression(expression) if *expression == actual);
    }
    source.symbol.is_valid()
        && source.application.is_none()
        && source.evidence_projection.is_none()
        && source.const_literal.is_none()
        && matches!(program.expression_table.expression(actual), ExpressionNode::Name(path)
            if path.symbol == source.symbol && path.head_symbol == source.symbol)
}

fn realized_coordinate(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    subject: ExpressionHandle,
    symbol: SymbolHandle,
) -> Option<Coordinate> {
    let entry = program.machine_states(machine).first()?;
    let parameter = program.state_parameter_by_symbol(entry, symbol)?;
    if parameter.is_mutable || parameter.is_const {
        return None;
    }
    let original = program.symbols.get(symbol).generated_from;
    let rows = program
        .machine_specializations
        .iter()
        .filter(|row| row.instance == machine.symbol)
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return None;
    };
    if program
        .typed_program()
        .executable_application_template(machine.symbol)
        != Ok(Some(row.template))
    {
        return None;
    }
    let template = program.machine_by_symbol(row.template)?;
    let (position, binder) = program
        .machine_type_parameters(template)
        .iter()
        .enumerate()
        .find(|(_, binder)| binder.symbol == original)?;
    let typed_trees::data::TypeParameterKind::Value { type_reference } = binder.kind else {
        return None;
    };
    let typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
        type_reference: runtime_carrier,
        ..
    } = program
        .typed_program()
        .specialization_binding(row, original)?
    else {
        return None;
    };
    let identity = program.normalized_type_identity(type_reference);
    if !identity.is_valid()
        || identity != program.normalized_type_identity(*runtime_carrier)
        || identity != program.normalized_type_identity(parameter.type_reference)
        || program.symbols.get(symbol).parent != entry.symbol
    {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    matches!(
        primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    )
    .then_some(Coordinate {
        subject,
        carrier: parameter.type_reference,
        primitive,
        // This is the original source telescope position, not a runtime slot.
        binder_position: Some(position),
        closed: None,
        realized_from: Some(original),
    })
}

fn realized_parameter(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    original: SymbolHandle,
    carrier: TypeReferenceHandle,
) -> Option<(usize, SymbolHandle)> {
    let identity = program.normalized_type_identity(carrier);
    if !identity.is_valid() {
        return None;
    }
    let parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            let declaration = program.symbols.get(parameter.symbol);
            !parameter.is_mutable
                && !parameter.is_const
                && declaration.parent == state.symbol
                && declaration.generated_from == original
                && identity == program.normalized_type_identity(parameter.type_reference)
        })
        .map(|(position, parameter)| (position, parameter.symbol))
        .collect::<Vec<_>>();
    let [parameter] = parameters.as_slice() else {
        return None;
    };
    Some(*parameter)
}

/// Authenticate the actual transport, not just similarly generated names.
/// Every subordinate state receives this immutable invocation coordinate
/// unchanged. Recursive machine invocations are judged separately by strict
/// descent on their retained authored application tuple.
fn realized_forwarding(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    original: SymbolHandle,
    carrier: TypeReferenceHandle,
) -> Result<Vec<(SymbolHandle, SymbolHandle)>, String> {
    let states = program.machine_states(machine);
    let mut mapping = Vec::new();
    for state in states {
        let (_, source) = realized_parameter(program, state, original, carrier)
            .ok_or_else(|| "source rank has no unique immutable state realization".to_string())?;
        mapping.push((state.symbol, source));
        for ordinal in 0..program
            .statement_table
            .statements(state.statement_nodes)
            .len()
        {
            for call in
                validation::collect_call_sites_in_statement(program, machine, state, ordinal)
            {
                let (target, arguments): (_, &[ExpressionHandle]) = match call {
                    CallSite::Statement(call) => (
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    ),
                    CallSite::Expression { call, .. } => (
                        validation::call_target_symbol(program, call),
                        program.expression_table.expression_handles(call.arguments),
                    ),
                    CallSite::TransitionNamed {
                        path, arguments, ..
                    } => (
                        path.symbol,
                        program.statement_table.expression_handles(arguments),
                    ),
                };
                let Some(destination) = validation::internal_state_index(program, machine, target)
                else {
                    continue;
                };
                let (position, _) =
                    realized_parameter(program, &states[destination], original, carrier)
                        .ok_or_else(|| {
                            "source rank destination lost its exact realization".to_string()
                        })?;
                let forwarded = arguments.get(position).is_some_and(|argument| {
                    matches!(
                        program.expression_table.expression(*argument), ExpressionNode::Name(path)
                            if path.symbol == source && path.head_symbol == source
                    )
                });
                if !forwarded {
                    return Err(
                        "internal state arrival does not preserve the exact realized source rank"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(mapping)
}

fn static_value(
    program: &dyn validation::CheckingInput,
    values: &CurrentScalarValues<'_>,
    argument: &StaticMachineArgument,
) -> Option<ScopedArithmeticValue> {
    if let Some(literal) = &argument.const_literal {
        return literal.value_bignum().map(ScopedArithmeticValue::Integer);
    }
    if argument.type_reference.is_valid()
        && let TypeReferenceNode::ConstExpression(expression) = program
            .type_reference_table
            .type_reference(argument.type_reference)
    {
        return values.current(*expression).map(ScopedArithmeticValue::Term);
    }
    if argument.symbol.is_valid() {
        return values.current_symbol(argument.symbol);
    }
    let spelling = typed_trees::typed_trees::static_const_argument_spelling(
        program.typed_program(),
        argument,
    )?;
    if let Some(value) = language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
    {
        let language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
            value.decode_encoding()?
        else {
            return None;
        };
        return Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)));
    }
    BigInt::from_decimal_str(&spelling).map(ScopedArithmeticValue::Integer)
}
