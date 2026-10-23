//! Saved decisions are historical values, not promises about current storage.
//! Rejoin their exact local definition and selected Boolean polarity, collect
//! complete effects through this use, then retain only repeatable observations
//! whose read places survived. Flow facts and transition requirements consume
//! this same judgment; generated names and adjacency grant no authority.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

/// Current observations of a saved predicate. The root is included when all
/// its reads survive; independently preserved conjuncts remain useful when a
/// disjoint effect prevents repeating the complete original computation. Extra
/// writes belong to operands evaluated after dispatch and before the consumer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn saved_guard_observations<'program>(
    program: &dyn validation::CheckingInput,
    machine: &'program Machine,
    state: &State,
    statement_index: usize,
    guard: ExpressionHandle,
    value: bool,
    later_writes: &[String],
    frames: Option<&validation::CallFrameResolver<'program>>,
) -> Vec<(ExpressionHandle, bool)> {
    let Some(frames) = frames else {
        return Vec::new();
    };
    let Some(mut dispatch_writes) = frames
        .expression_write_frame(machine, guard)
        .into_complete_paths()
    else {
        return Vec::new();
    };
    dispatch_writes.extend_from_slice(later_writes);
    let mut dispatch = Vec::new();
    guard_observations(program, machine, state, guard, value, &mut dispatch);
    let mut prepared = Vec::new();
    let mut visited = Vec::new();
    let mut observation_position = 0;
    while let Some(&(subject, polarity)) = dispatch.get(observation_position) {
        observation_position += 1;
        if visited.contains(&(subject, polarity)) {
            continue;
        }
        visited.push((subject, polarity));
        let Some(symbol) =
            validation::expression_traversal::single_segment_name_symbol(program, subject)
        else {
            continue;
        };
        if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Local {
            continue;
        }
        let Some((predicate, writes)) =
            reaching_capture(program, machine, state, statement_index, symbol, frames)
        else {
            continue;
        };
        let frontier = dispatch_writes.len();
        dispatch_writes.extend(writes);
        // The saved Boolean itself must still be the value whose initializer
        // supplied the predicate, independently of that predicate's operands.
        if reads_survive(
            program,
            machine,
            state,
            statement_index,
            subject,
            &dispatch_writes,
            frames,
        ) {
            let mut observations = vec![(predicate, polarity)];
            guard_observations(
                program,
                machine,
                state,
                predicate,
                polarity,
                &mut observations,
            );
            for observation in observations {
                if !prepared.contains(&observation)
                    && expression_is_stable_predicate(program, observation.0)
                    && reads_survive(
                        program,
                        machine,
                        state,
                        statement_index,
                        observation.0,
                        &dispatch_writes,
                        frames,
                    )
                {
                    prepared.push(observation);
                    // Ordinary authored Boolean locals and parser captures
                    // transport the same decision. Follow preserved aliases
                    // through this worklist, without a second recovery path.
                    if program
                        .expression_table
                        .name_path_node(observation.0)
                        .is_some()
                    {
                        dispatch.push(observation);
                    }
                }
            }
        }
        dispatch_writes.truncate(frontier);
    }
    prepared
}

fn reaching_capture<'program>(
    program: &dyn validation::CheckingInput,
    machine: &'program Machine,
    state: &State,
    statement_index: usize,
    symbol: SymbolHandle,
    frames: &validation::CallFrameResolver<'program>,
) -> Option<(ExpressionHandle, Vec<String>)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut writes = Vec::new();
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order.position(statement_index)?;
    for statement in order.indices()[..end]
        .iter()
        .rev()
        .filter_map(|position| statements.get(*position))
    {
        if let Some(local) = statement.as_local_data()
            && local.symbol == symbol
        {
            if !local.initial_value.is_valid() {
                return None;
            }
            writes.extend(
                frames
                    .expression_write_frame(machine, local.initial_value)
                    .into_complete_paths()?,
            );
            return Some((local.initial_value, writes));
        }
        match statement {
            StatementNode::Transition(transition) => {
                // Only a fallthrough arm reaches the later lexical statement.
                // Its guard executed, but its unselected target did not.
                if transition.exit != TransitionExit::Ordinary
                    || transition.continuation.is_valid()
                    || matches!(transition.guard, TransitionGuardNode::Always)
                {
                    return None;
                }
                if let TransitionGuardNode::When(guard) = transition.guard {
                    writes.extend(
                        frames
                            .expression_write_frame(machine, guard)
                            .into_complete_paths()?,
                    );
                }
            }
            StatementNode::Assignment(_) => {
                writes.extend(
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths()?,
                );
                writes.extend(
                    frames
                        .statement_value_write_frame(machine, statement)
                        .into_complete_paths()?,
                );
            }
            StatementNode::Call(call) => {
                writes.extend(
                    frames
                        .may_write_frame(machine, call)
                        .into_complete_paths()?,
                );
                writes.extend(
                    frames
                        .statement_value_write_frame(machine, statement)
                        .into_complete_paths()?,
                );
            }
            StatementNode::LocalData(_) | StatementNode::Expression(_) => {
                writes.extend(
                    frames
                        .statement_value_write_frame(machine, statement)
                        .into_complete_paths()?,
                );
            }
            // Assembly and binding/proof directives do not have a complete
            // executable preservation judgment here. Absence is not purity.
            _ => return None,
        }
    }
    None
}

fn reads_survive(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    writes: &[String],
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    if writes.is_empty() {
        return true;
    }
    let mut nodes = Vec::new();
    validation::expression_traversal::collect_expression_tree(program, expression, &mut nodes);
    nodes.into_iter().all(|node| {
        // Canonical rvalues have expression roots too. They are identities,
        // not storage reads: inspect only place-shaped nodes here, while the
        // complete walk independently visits every computed operand.
        match program.expression_table.expression(node) {
            ExpressionNode::Name(path)
                if validation::exact_case_reference_owner(program, node).is_some()
                    || (path.symbol.is_valid()
                        && program.symbols.get(path.symbol).kind == SymbolKind::Const) =>
            {
                return true;
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Borrow(_) => {}
            ExpressionNode::Atomic(_) => return false,
            _ => return true,
        }
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            node,
        );
        let preserved = |root, segments: &[facts::PlaceSegment]| {
            validation::frame_coordinate_path(program, root, segments).is_some_and(|path| {
                writes
                    .iter()
                    .all(|written| !validation::frame_paths_overlap(written, &path))
            })
        };
        if let Some(place) = place {
            let Some(root) = place.root.as_symbol() else {
                return false;
            };
            if !preserved(root, &place.segments) {
                return false;
            }
        } else {
            // Literal/type/classifier nodes do not read mutable storage.
            // A missing coordinate for an actual read is not preservation.
            match program.expression_table.expression(node) {
                ExpressionNode::Name(path)
                    if matches!(
                        program.symbols.get(path.symbol).kind,
                        SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                    ) =>
                {
                    return false;
                }
                ExpressionNode::Member(_)
                    if validation::collection_length_receiver(
                        program,
                        machine,
                        Some(state),
                        node,
                    )
                    .is_none() =>
                {
                    return false;
                }
                ExpressionNode::Indexed(_)
                | ExpressionNode::Borrow(_)
                | ExpressionNode::Atomic(_) => return false,
                _ => {}
            }
        }
        // Alias-normalized writes and local diagnostic labels are different
        // vocabularies. Resolve every reference-valued read through the frame
        // owner's complete prefix read footprints, including shared and field loans.
        // A coarse window footprint can invalidate an observation, but cannot
        // establish exact referent identity or predicate equality.
        let reference = validation::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            node,
        );
        if !reference
            .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
        {
            return true;
        }
        let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
            return false;
        };
        let Some(reads) =
            frames.reference_expression_read_paths_before_statement(machine, statement, node)
        else {
            return false;
        };
        reads.iter().all(|read| {
            writes
                .iter()
                .all(|written| !validation::frame_paths_overlap(written, read))
        })
    })
}

/// Signed observations guaranteed by the selected Boolean outcome, in
/// evaluation order. A true conjunction or false disjunction observes both
/// children. Other short-circuit outcomes establish no individual operand.
pub(super) fn guard_observations(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    value: bool,
    observations: &mut Vec<(ExpressionHandle, bool)>,
) {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && validation::has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) =>
        {
            guard_observations(program, machine, state, unary.operand, !value, observations);
            return;
        }
        ExpressionNode::Binary(binary)
            if (binary.operator == BinaryOperator::And && value)
                || (binary.operator == BinaryOperator::Or && !value) =>
        {
            guard_observations(program, machine, state, binary.left, value, observations);
            guard_observations(program, machine, state, binary.right, value, observations);
            return;
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            let observed = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(expected)) => Some((binary.left, *expected)),
                (ExpressionNode::Boolean(expected), _) => Some((binary.right, *expected)),
                _ => None,
            };
            if let Some((operand, expected)) = observed {
                let equal = value == (binary.operator == BinaryOperator::Equal);
                guard_observations(
                    program,
                    machine,
                    state,
                    operand,
                    expected == equal,
                    observations,
                );
                return;
            }
        }
        _ => {}
    }
    observations.push((expression, value));
}

pub(super) fn expression_is_stable_predicate(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| expression_is_stable_predicate(program, field.value)),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .all(|value| expression_is_stable_predicate(program, *value)),
        ExpressionNode::Member(member) => expression_is_stable_predicate(program, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            expression_is_stable_predicate(program, indexed.collection)
                && expression_is_stable_predicate(program, indexed.index)
        }
        ExpressionNode::Unary(unary) => expression_is_stable_predicate(program, unary.operand),
        ExpressionNode::Binary(binary) => {
            expression_is_stable_predicate(program, binary.left)
                && expression_is_stable_predicate(program, binary.right)
        }
        ExpressionNode::Borrow(borrow) => expression_is_stable_predicate(program, borrow.target),
        // A selected primitive observation is repeatable while its operand
        // storage is preserved. Observation contexts retain those operand
        // places, so ordinary alias-aware invalidation owns both polarities.
        ExpressionNode::Call(_) => {
            typed_trees::byte_predicates::byte_predicate_call(program, expression)
                .is_some_and(|(_, operand)| expression_is_stable_predicate(program, operand))
        }
        _ => false,
    }
}
