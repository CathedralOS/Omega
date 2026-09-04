use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::TypeReferenceNode;

/// The literal upper bound the incoming edges place on `offset` at this
/// state's entry: the PER-EDGE MEET (M2 gap 4a) -- EVERY incoming edge
/// machine-wide must prove a bound, and the entry bound is their MAX (the
/// weakest all satisfy). Per-edge routes, in order:
/// - a CONSTANT argument bounds at its own value;
/// - the edge's GUARDED (true) arm, whose guard conjunct `arg <= K` /
///   `arg < K` names (by display spelling) the very expression passed at
///   the param's position -- guard check and argument capture happen in
///   the same transition step, so the bound holds at entry;
/// - R4 witness (the own_machine shape): a BOUNDARY call EARLIER in the
///   source state whose `ensures <param> <= K` bounds the `&mut` argument
///   place spelled identically to the transition argument, with NO
///   intervening write to that place and NO later call (a later callee
///   holding `&mut self` could rewrite the field) between the witness and
///   the transition.
///
/// One unprovable edge kills the meet. Symbolic bounds (`offset +
/// desc_size < map_size`) remain -- gap 4b.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundSide {
    Upper,
    Lower,
}

pub(super) fn incoming_guard_offset_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    offset: ExpressionHandle,
) -> Option<i64> {
    incoming_offset_bound(
        program,
        machine,
        state,
        offset,
        SYMBOLIC_BOUND_DEPTH,
        BoundSide::Upper,
    )
}

fn incoming_offset_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    offset: ExpressionHandle,
    depth: u8,
    side: BoundSide,
) -> Option<i64> {
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    // The offset must be a bare PARAM of this state; the guard bounds the
    // ARGUMENT at the call site, which becomes the param at entry.
    let ExpressionNode::Name(path) = program.expression_table.expression(offset) else {
        return None;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    // Position among NON-SELF parameters: call-site argument lists exclude
    // the receiver.
    let param_position = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == param_name.as_str())?;

    let mut meet: Option<i64> = None;
    let mut incoming_edges = 0usize;
    for source in program.machine_states(machine) {
        let source_statements = program.statement_table.statements(source.statement_nodes);
        for (statement_index, statement) in source_statements.iter().enumerate() {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target_handle)
                else {
                    continue;
                };
                let target_name = program
                    .statement_table
                    .name_path_members(path.members)
                    .last()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                if target_name != state.name.as_str() {
                    continue;
                }
                incoming_edges += 1;
                let argument = program
                    .statement_table
                    .expression_handles(*arguments)
                    .get(param_position)
                    .copied()?;
                // A constant argument bounds at its own value (both sides).
                if let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(argument)
                {
                    let value = literal.value_i64().filter(|value| *value >= 0)?;
                    meet = Some(meet.map_or(value, |existing: i64| match side {
                        BoundSide::Upper => existing.max(value),
                        BoundSide::Lower => existing.min(value),
                    }));
                    continue;
                }
                let argument_label = program.expression_table.display_name(argument);
                // Gap 4b: a SELF-FORWARDING edge (the state passes this very
                // param back to itself unchanged) preserves whatever holds at
                // entry -- it contributes nothing to the meet and must not
                // kill it.
                if source.symbol == state.symbol && argument_label == param_name.as_str() {
                    continue;
                }
                // Only the GUARDED (true) arm establishes the guard's bound;
                // the R4 ensures witness precedes the whole transition, so it
                // holds on EITHER arm (and on an Always edge).
                let guard_bound = match transition.guard {
                    TransitionGuardNode::When(guard) if target_handle == transition.target => {
                        match side {
                            BoundSide::Upper => guard_upper_bound_for(
                                program,
                                machine,
                                source,
                                guard,
                                &argument_label,
                                depth,
                            ),
                            BoundSide::Lower => guard_lower_bound_for(
                                program,
                                machine,
                                source,
                                guard,
                                &argument_label,
                                depth,
                            ),
                        }
                    }
                    _ => None,
                };
                let edge_bound = guard_bound.or_else(|| {
                    boundary_ensures_argument_bound(
                        program,
                        machine,
                        source,
                        source_statements,
                        statement_index,
                        &argument_label,
                        side,
                    )
                })?;
                meet = Some(meet.map_or(edge_bound, |existing: i64| match side {
                    BoundSide::Upper => existing.max(edge_bound),
                    BoundSide::Lower => existing.min(edge_bound),
                }));
            }
        }
    }
    // No incoming edge at all (the entry state, or dead states) proves
    // nothing.
    if incoming_edges == 0 {
        return None;
    }
    meet
}

/// The R4 witness route: scan the statements BEFORE the transition for the
/// LAST boundary call whose `ensures <param> <= K`/`< K` bounds a `&mut`
/// argument place spelled `argument_label`; refuse if anything after that
/// witness could rewrite the place (an assignment to it, or ANY other call
/// -- callees hold `&mut self`).
fn boundary_ensures_argument_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    statements: &[psi_typed_trees::statement::StatementNode],
    transition_index: usize,
    argument_label: &str,
    side: BoundSide,
) -> Option<i64> {
    use psi_typed_trees::statement::StatementNode;
    let call_frames = crate::calls::CallFrameResolver::new(program);
    let mut witness: Option<i64> = None;
    for statement in &statements[..transition_index] {
        match statement {
            StatementNode::Call(call) => {
                // A resolved call whose may-write frame reaches this place
                // invalidates an earlier witness; a boundary call may also
                // mint a new one. Disjoint resolved calls preserve the
                // witness, while unknown calls remain fail-closed.
                let minted = boundary_call_ensures_bound(
                    program,
                    machine,
                    source,
                    call,
                    argument_label,
                    side,
                );
                if minted.is_some() {
                    witness = minted;
                } else {
                    let written = call_frames
                        .as_ref()
                        .and_then(|frames| frames.may_write_paths(machine, call));
                    if !written.is_some_and(|paths| {
                        paths
                            .iter()
                            .all(|path| !crate::calls::frame_paths_overlap(path, argument_label))
                    }) {
                        witness = None;
                    }
                }
            }
            StatementNode::Assignment(assignment)
                if program.expression_table.display_name(assignment.target) == argument_label =>
            {
                witness = None;
            }
            _ => {}
        }
    }
    witness
}

/// `call`'s `ensures <param> <= K`/`< K` INCLUSIVE bound for the `&mut`
/// argument place spelled `argument_label`, resolved through the receiver
/// field's declared boundary trait. None for non-boundary callees, other
/// spellings, or params without a literal upper bound.
fn boundary_call_ensures_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
    argument_label: &str,
    side: BoundSide,
) -> Option<i64> {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::signature::SignatureContractKind;
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .last()?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == receiver.as_str() =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })?;
    let TypeReferenceNode::Named {
        name: trait_name, ..
    } = program.type_reference_table.type_reference(field_type)
    else {
        return None;
    };
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == trait_name.as_str())?;
    let signature = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name == call.target)?;
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Which non-self param position holds our place as a `&mut` argument?
    let position = arguments.iter().position(|argument| {
        matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Borrow(inner)
                if program.expression_table.display_name(inner.target) == argument_label
        )
    })?;
    let parameter = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .nth(position)?;
    let mut bound: Option<i64> = None;
    for contract in program
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            // Resolve bounds in the callee signature's scope before mapping
            // the selected parameter back to the caller argument. Equality
            // between out-parameters may therefore carry a literal witness
            // (`size == limit && limit <= 8`) without confusing either name
            // with caller scope.
            let fact_bound = match side {
                BoundSide::Upper => guard_upper_bound_for(
                    program,
                    machine,
                    source,
                    *expression,
                    parameter.name.as_str(),
                    SYMBOLIC_BOUND_DEPTH,
                ),
                BoundSide::Lower => guard_lower_bound_for(
                    program,
                    machine,
                    source,
                    *expression,
                    parameter.name.as_str(),
                    SYMBOLIC_BOUND_DEPTH,
                ),
            };
            if let Some(fact_bound) = fact_bound {
                bound = Some(bound.map_or(fact_bound, |existing: i64| match side {
                    BoundSide::Upper => existing.min(fact_bound),
                    BoundSide::Lower => existing.max(fact_bound),
                }));
            }
        }
    }
    bound
}

/// `label <= K` / `label < K` within an `&&` conjunction (through the
/// `== true` desugar), by display spelling.
/// Recursion cap for symbolic bound resolution: the M2 chain needs depth 2
/// (offset bound -> map_size bound); anything deeper stays unproven.
const SYMBOLIC_BOUND_DEPTH: u8 = 2;

fn guard_upper_bound_for(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    guard: ExpressionHandle,
    label: &str,
    depth: u8,
) -> Option<i64> {
    use psi_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            guard_upper_bound_for(program, machine, source, binary.left, label, depth)
                .or_else(|| {
                    guard_upper_bound_for(program, machine, source, binary.right, label, depth)
                })
                .or_else(|| {
                    let peer = (depth > 0)
                        .then(|| equality_peer_for(program, guard, label))
                        .flatten()?;
                    let peer_label = program.expression_table.display_name(peer);
                    if peer_label == label {
                        return None;
                    }
                    symbolic_expression_bound(
                        program,
                        machine,
                        source,
                        peer,
                        depth - 1,
                        BoundSide::Upper,
                    )
                    .or_else(|| {
                        guard_upper_bound_for(
                            program,
                            machine,
                            source,
                            guard,
                            &peer_label,
                            depth - 1,
                        )
                    })
                })
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_upper_bound_for(program, machine, source, binary.left, label, depth)
        }
        BinaryOperator::Equal if depth > 0 => {
            let peer = equality_peer_for(program, guard, label)?;
            symbolic_expression_bound(program, machine, source, peer, depth - 1, BoundSide::Upper)
        }
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            // The comparison's inclusive RHS bound: a literal, or (gap 4b)
            // a symbolic NAME whose own inclusive bound resolves through
            // the per-edge meet in the SOURCE state's scope.
            let rhs_inclusive = match program.expression_table.expression(binary.right) {
                ExpressionNode::Integer(literal) => literal.value_i64()?,
                ExpressionNode::Name(_) if depth > 0 => {
                    symbolic_param_upper_bound(program, machine, source, binary.right, depth - 1)?
                }
                _ => return None,
            };
            let bound = if binary.operator == BinaryOperator::Less {
                rhs_inclusive.checked_sub(1)?
            } else {
                rhs_inclusive
            };
            // Direct match: the compared expression IS the labeled one.
            if program.expression_table.display_name(binary.left) == label {
                return Some(bound);
            }
            // Gap 4b composition: `X + Y <OP> RHS` bounds X at RHS_bound -
            // lower(Y) -- sound because Y >= lower(Y) forces X down by at
            // least that much. Both operand orders.
            if depth > 0
                && let ExpressionNode::Binary(addition) =
                    program.expression_table.expression(binary.left)
                && addition.operator == BinaryOperator::Add
            {
                for (x, y) in [
                    (addition.left, addition.right),
                    (addition.right, addition.left),
                ] {
                    if program.expression_table.display_name(x) == label
                        && let Some(y_floor) =
                            symbolic_param_lower_bound(program, machine, source, y, depth - 1)
                        && y_floor >= 0
                    {
                        return bound.checked_sub(y_floor);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// A NAME's inclusive UPPER bound in `source`'s scope: its declared range,
/// or (as a param) the per-edge meet -- the gap-4b symbolic resolution.
fn symbolic_param_upper_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    name: ExpressionHandle,
    depth: u8,
) -> Option<i64> {
    let declared = crate::places::declared_place_type_raw(program, machine, Some(source), name)
        .and_then(|raw| {
            let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
            interval.high()
        });
    declared
        .or_else(|| incoming_offset_bound(program, machine, source, name, depth, BoundSide::Upper))
}

/// A NAME's inclusive LOWER bound in `source`'s scope (declared range or
/// the per-edge meet's lower twin) -- the `desc_size >= sizeof` witness leg.
fn symbolic_param_lower_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    name: ExpressionHandle,
    depth: u8,
) -> Option<i64> {
    let declared = crate::places::declared_place_type_raw(program, machine, Some(source), name)
        .and_then(|raw| {
            let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
            interval.low()
        });
    declared
        .or_else(|| incoming_offset_bound(program, machine, source, name, depth, BoundSide::Lower))
}

/// `label >= K` / `> K` within the same guard walk -- the lower twin.
fn guard_lower_bound_for(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    guard: ExpressionHandle,
    label: &str,
    depth: u8,
) -> Option<i64> {
    use psi_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            guard_lower_bound_for(program, machine, source, binary.left, label, depth)
                .or_else(|| {
                    guard_lower_bound_for(program, machine, source, binary.right, label, depth)
                })
                .or_else(|| {
                    let peer = (depth > 0)
                        .then(|| equality_peer_for(program, guard, label))
                        .flatten()?;
                    let peer_label = program.expression_table.display_name(peer);
                    if peer_label == label {
                        return None;
                    }
                    symbolic_expression_bound(
                        program,
                        machine,
                        source,
                        peer,
                        depth - 1,
                        BoundSide::Lower,
                    )
                    .or_else(|| {
                        guard_lower_bound_for(
                            program,
                            machine,
                            source,
                            guard,
                            &peer_label,
                            depth - 1,
                        )
                    })
                })
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_lower_bound_for(program, machine, source, binary.left, label, depth)
        }
        BinaryOperator::Equal if depth > 0 => {
            let peer = equality_peer_for(program, guard, label)?;
            symbolic_expression_bound(program, machine, source, peer, depth - 1, BoundSide::Lower)
        }
        BinaryOperator::GreaterOrEqual | BinaryOperator::Greater => {
            if program.expression_table.display_name(binary.left) != label {
                return None;
            }
            let ExpressionNode::Integer(literal) =
                program.expression_table.expression(binary.right)
            else {
                return None;
            };
            let k = literal.value_i64()?;
            if binary.operator == BinaryOperator::Greater {
                k.checked_add(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}

/// The expression equated to `label` in a conjunction, if any. This returns
/// only the peer expression; the caller decides which independent bound walk
/// to apply. The recursion cap on that walk makes equality cycles incomplete
/// rather than unsound.
fn equality_peer_for(
    program: &TypedTrees,
    expression: ExpressionHandle,
    label: &str,
) -> Option<ExpressionHandle> {
    use psi_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => equality_peer_for(program, binary.left, label)
            .or_else(|| equality_peer_for(program, binary.right, label)),
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            equality_peer_for(program, binary.left, label)
        }
        BinaryOperator::Equal => {
            let left = program.expression_table.display_name(binary.left);
            let right = program.expression_table.display_name(binary.right);
            if left == label && right != label {
                Some(binary.right)
            } else if right == label && left != label {
                Some(binary.left)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn symbolic_expression_bound(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    source: &psi_typed_trees::state::State,
    expression: ExpressionHandle,
    depth: u8,
    side: BoundSide,
) -> Option<i64> {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        return literal.value_i64();
    }
    match side {
        BoundSide::Upper => symbolic_param_upper_bound(program, machine, source, expression, depth),
        BoundSide::Lower => symbolic_param_lower_bound(program, machine, source, expression, depth),
    }
}
