//! Replay ordered observations using the same authored places as call operands.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckedTrees};

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

enum ObservationStep {
    Enter(ExpressionHandle, bool, Option<BorrowAccessKind>),
    Loan {
        expression: ExpressionHandle,
        target: ExpressionHandle,
        kind: BorrowAccessKind,
        direct: bool,
    },
    Exit {
        expression: ExpressionHandle,
        first_access: usize,
    },
    RetainReceiver {
        first_access: usize,
    },
}

/// Argument evaluation may read selectors before retaining its operand loan.
#[derive(Clone, Copy)]
pub(crate) struct ArgumentAccessExtent {
    pub(crate) start: usize,
    pub(crate) end: usize,
    operand: Option<usize>,
}

impl ArgumentAccessExtent {
    pub(crate) fn operand_position(self) -> Result<usize, LoweringError> {
        self.operand.ok_or(LoweringError::Unsupported(
            "structural actual has no retained operand observation",
        ))
    }
}

/// Keep each actual's complete observation extent separate from its operand row.
pub(crate) fn rejoin(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    call: &BorrowCallFact,
    arguments: &[ExpressionHandle],
) -> Result<Vec<ArgumentAccessExtent>, LoweringError> {
    let borrow = &checked.facts.borrow;
    let rows = borrow
        .argument_accesses
        .span(call.accesses)
        .ok_or(LoweringError::Unsupported(
            "computed shared borrow has a stale observation roster",
        ))?;
    let table = &checked.expression_table;
    let (source_machine, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let plain_copy = |expression| {
        validation::expression_result_type_reference(
            &checked.typed,
            source_machine,
            source_state,
            expression,
        )
        .is_some_and(|reference| {
            validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, reference)
                && checked.type_multiplicity(reference)
                    == language_semantics::Multiplicity::Unrestricted
        })
    };
    let mut positions = Vec::with_capacity(arguments.len());
    let selected_access = |argument, formal| {
        validation::expression_result_type_reference(
            &checked.typed,
            source_machine,
            source_state,
            argument,
        )
        .and_then(|reference| validation::reference_shell(&checked.typed, reference))
        .and_then(|_| validation::reference_shell(&checked.typed, formal))
        .map(|reference| match reference.access {
            language_core::ReferenceAccess::Shared => BorrowAccessKind::Read,
            language_core::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
            language_core::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
        })
    };
    let mut observation_ends: Vec<Option<usize>> = Vec::new();
    let mut position = 0;
    let owner =
        checked
            .machine_holding_state(call.target_symbol)
            .ok_or(LoweringError::Unsupported(
                "observation target has no authored owner",
            ))?;
    let target = checked
        .machine_states(owner)
        .iter()
        .find(|state| state.symbol == call.target_symbol)
        .ok_or(LoweringError::Unsupported(
            "observation target has no authored state",
        ))?;
    let parameters = checked
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return unsupported("observation arguments disagree with the selected signature");
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let start = position;
        let mut operand = None;
        let forwarded_access = selected_access(*argument, parameter.type_reference);
        let mut pending = vec![ObservationStep::Enter(*argument, true, forwarded_access)];
        let mut active = Vec::new();
        while let Some(step) = pending.pop() {
            let (expression, direct, loan, forwarded_access) = match step {
                ObservationStep::Enter(expression, direct, access) => {
                    (expression, direct, None, access)
                }
                ObservationStep::Loan {
                    expression,
                    target,
                    kind,
                    direct,
                } => (expression, direct, Some((target, kind)), None),
                ObservationStep::Exit {
                    expression,
                    first_access,
                } => {
                    if matches!(table.expression(expression), ExpressionNode::Call(_))
                        && plain_copy(expression)
                    {
                        for end in &mut observation_ends[first_access..] {
                            end.get_or_insert(position);
                        }
                    }
                    active.pop();
                    continue;
                }
                ObservationStep::RetainReceiver { first_access } => {
                    observation_ends[first_access..].fill(None);
                    continue;
                }
            };
            let observing_loan = loan.is_some();
            if !table.expression_is_valid(expression)
                || (loan.is_none() && active.contains(&expression))
            {
                return unsupported(
                    "computed shared borrow has a stale or cyclic source observation",
                );
            }
            let first_access = position;
            if loan.is_none() {
                active.push(expression);
                pending.push(ObservationStep::Exit {
                    expression,
                    first_access,
                });
                let requested = match table.expression(expression) {
                    ExpressionNode::Borrow(borrow) => Some((
                        borrow.target,
                        match borrow.access {
                            language_core::ReferenceAccess::Shared => BorrowAccessKind::Read,
                            language_core::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                            language_core::ReferenceAccess::WriteOnly => {
                                BorrowAccessKind::WriteOnly
                            }
                        },
                    )),
                    ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Cast(_) => forwarded_access.map(|kind| (expression, kind)),
                    _ => None,
                };
                if let Some((target, kind)) = requested {
                    pending.push(ObservationStep::Loan {
                        expression,
                        target,
                        kind,
                        direct,
                    });
                    pending.extend(
                        place_evaluation_reads(checked, target)?
                            .into_iter()
                            .rev()
                            .map(|expression| ObservationStep::Enter(expression, false, None)),
                    );
                    continue;
                }
            }
            let mut children = Vec::new();
            let mut argument_children = Vec::new();
            let mut receiver = None;
            let (named, kind) = if let Some((target, kind)) = loan {
                (Some(target), kind)
            } else {
                match table.expression(expression) {
                    ExpressionNode::Borrow(borrow) => (
                        Some(borrow.target),
                        match borrow.access {
                            language_core::ReferenceAccess::Shared => BorrowAccessKind::Read,
                            language_core::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                            language_core::ReferenceAccess::WriteOnly => {
                                BorrowAccessKind::WriteOnly
                            }
                        },
                    ),
                    ExpressionNode::Name(_) | ExpressionNode::StructLiteral(_)
                        if validation::scalar_case_constructor(checked, expression).is_some() =>
                    {
                        let constructor = validation::scalar_case_constructor(checked, expression)
                            .ok_or(LoweringError::Unsupported(
                                "computed case lost its source constructor",
                            ))?;
                        children.extend(
                            constructor
                                .fields
                                .into_iter()
                                .map(|(_, expression, _)| expression),
                        );
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Name(_) => (Some(expression), BorrowAccessKind::Read),
                    ExpressionNode::Member(_) => (Some(expression), BorrowAccessKind::Read),
                    ExpressionNode::Atomic(atomic) => {
                        children.push(atomic.value);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Binary(binary) => {
                        children.extend([binary.left, binary.right]);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Match(dispatch) => {
                        children.push(dispatch.subject);
                        let arms = table.match_arms(dispatch.arms);
                        if arms.len() != dispatch.arms.len() {
                            return unsupported(
                                "computed shared borrow has stale dispatch observations",
                            );
                        }
                        for arm in arms {
                            if let checked_trees::expression::MatchPattern::Value(pattern) =
                                arm.pattern
                            {
                                children.push(pattern);
                            }
                            children.push(arm.value);
                        }
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Unary(unary) => {
                        children.push(unary.operand);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Cast(cast) => {
                        children.push(cast.value);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Call(call) => {
                        // A resolved static qualifier names the callee's owner;
                        // it is not storage observed while evaluating the call.
                        // Unknown selections keep the receiver on the replay path.
                        let no_runtime_receiver = checked
                            .machine_holding_state(call.target_symbol)
                            .and_then(|owner| {
                                checked
                                    .machine_states(owner)
                                    .iter()
                                    .find(|state| state.symbol == call.target_symbol)
                                    .map(|state| {
                                        checked.call_has_no_runtime_receiver(call, owner, state)
                                    })
                            })
                            .unwrap_or(false);
                        if call.receiver.is_valid() && !no_runtime_receiver {
                            receiver = Some(call.receiver);
                        }
                        let arguments = table.expression_handles(call.arguments);
                        if arguments.len() != call.arguments.count() as usize {
                            return unsupported(
                                "computed shared borrow has stale nested argument observations",
                            );
                        }
                        let owner = checked.machine_holding_state(call.target_symbol).ok_or(
                            LoweringError::Unsupported(
                                "nested observation target has no authored owner",
                            ),
                        )?;
                        let target = checked
                            .machine_states(owner)
                            .iter()
                            .find(|state| state.symbol == call.target_symbol)
                            .ok_or(LoweringError::Unsupported(
                                "nested observation target has no authored state",
                            ))?;
                        let parameters = checked
                            .state_parameters(target)
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .collect::<Vec<_>>();
                        if parameters.len() != arguments.len() {
                            return unsupported(
                                "nested observations disagree with the selected signature",
                            );
                        }
                        for (argument, parameter) in arguments.iter().zip(parameters) {
                            let access = selected_access(*argument, parameter.type_reference);
                            argument_children.push((*argument, access));
                        }
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::ArrayLiteral(elements) => {
                        let values = table.expression_handles(*elements);
                        if values.len() != elements.count() as usize {
                            return unsupported("computed array has stale scalar observations");
                        }
                        children.extend_from_slice(values);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        let fields = table.struct_fields(literal.fields);
                        if fields.len() != literal.fields.count() as usize {
                            return unsupported(
                                "computed construction has stale field observations",
                            );
                        }
                        children.extend(fields.iter().map(|field| field.value));
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Indexed(indexed)
                        if is_array_value_projection(checked, indexed.collection) =>
                    {
                        // Admitted array-value projections have only closed literal
                        // leaves, so the collection contributes no observations.
                        // Array source replay independently validates every sibling;
                        // only the selector contributes reads here.
                        children.push(indexed.index);
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Indexed(indexed) => {
                        // The captured place is one observation, followed by this
                        // selector's reads (or range endpoints). Its full path
                        // already includes nested collection projections, so do
                        // not invent extra reads by walking that collection again.
                        children.push(indexed.index);
                        (Some(expression), BorrowAccessKind::Read)
                    }
                    ExpressionNode::Range(range) => {
                        if range.start.is_valid() {
                            children.push(range.start);
                        }
                        if range.end.is_valid() {
                            children.push(range.end);
                        }
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::String(_)
                    | ExpressionNode::ZeroValue(_) => (None, BorrowAccessKind::Read),
                }
            };
            if let Some(named) = named {
                if !table.expression_is_valid(named) {
                    return unsupported("computed shared borrow has a stale named observation");
                }
                // An attached self field can be captured at the field's root,
                // unlike a projection from an ordinary parameter. Reuse the
                // source rejoin instead of guessing capture identity from the
                // spelling of the final Name/Member chain.
                let captured = if matches!(table.expression(named), ExpressionNode::Indexed(indexed)
                    if matches!(table.expression(indexed.index), ExpressionNode::Range(_)))
                {
                    super::view_subslices::captured_range(
                        checked, machine, state, statement, named,
                    )?
                } else {
                    crate::emission::call_source_custody::projected_receivers::source(
                        checked,
                        machine,
                        state,
                        statement as usize,
                        named,
                    )?
                    .captured_place()
                    .clone()
                };
                let row = rows.get(position).ok_or(LoweringError::Unsupported(
                    "computed shared borrow omits an authored observation",
                ))?;
                if !captured.root_symbol.is_valid()
                    || row.root_symbol != captured.root_symbol
                    || row.kind != kind
                    || borrow
                        .access_segments
                        .span(row.segments)
                        .is_none_or(|path| path != captured.segments)
                {
                    return unsupported(
                        "computed shared borrow substituted or reordered an observation",
                    );
                }
                // A scalar copy finishes at this observation. Borrow formation
                // instead remains live until its enclosing call completes.
                if direct {
                    operand = Some(position);
                }
                observation_ends.push(
                    (!observing_loan
                        && !matches!(table.expression(expression), ExpressionNode::Borrow(_))
                        && plain_copy(expression))
                    .then_some(position + 1),
                );
                position += 1;
            }
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|expression| ObservationStep::Enter(expression, false, None)),
            );
            pending.extend(
                argument_children
                    .into_iter()
                    .rev()
                    .map(|(expression, access)| ObservationStep::Enter(expression, false, access)),
            );
            if let Some(receiver) = receiver {
                pending.push(ObservationStep::RetainReceiver { first_access });
                pending.push(ObservationStep::Enter(receiver, false, None));
            }
        }
        positions.push(ArgumentAccessExtent {
            start,
            end: position,
            operand,
        });
    }
    if position != rows.len() {
        return unsupported("computed shared borrow has extra observation rows");
    }
    for (position, row) in rows.iter().enumerate() {
        if row.end_before_access != observation_ends[position] {
            return unsupported("computed borrow changed its authored observation lifetime");
        }
        for (later_position, later) in rows.iter().enumerate().skip(position + 1) {
            if row.root_symbol == later.root_symbol
                && (row.kind.is_exclusive() || later.kind.is_exclusive())
                && observation_ends[position].is_none_or(|end| end > later_position)
            {
                return unsupported("computed shared borrow aliases an exclusive observation");
            }
        }
    }
    Ok(positions)
}

/// Place formation evaluates selectors, not the referent's contents.
fn place_evaluation_reads(
    checked: &CheckedTrees,
    target: ExpressionHandle,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let table = &checked.expression_table;
    let mut pending = vec![(target, false, false)];
    let mut active = Vec::new();
    let mut reads = Vec::new();
    while let Some((expression, exiting, read)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if !table.expression_is_valid(expression) || active.contains(&expression) {
            return unsupported("borrow place has stale or cyclic evaluation");
        }
        if read {
            reads.push(expression);
            continue;
        }
        active.push(expression);
        pending.push((expression, true, false));
        match table.expression(expression) {
            ExpressionNode::Name(_) => {}
            ExpressionNode::Member(member) => pending.push((member.receiver, false, false)),
            ExpressionNode::Indexed(indexed) => {
                // Collect source selectors in collection-first order.
                pending.push((indexed.index, false, true));
                pending.push((indexed.collection, false, false));
            }
            ExpressionNode::Borrow(borrow) => pending.push((borrow.target, false, false)),
            ExpressionNode::Cast(cast) => pending.push((cast.value, false, false)),
            _ => reads.push(expression),
        }
    }
    Ok(reads)
}

fn is_array_value_projection(checked: &CheckedTrees, mut expression: ExpressionHandle) -> bool {
    let table = &checked.expression_table;
    let mut visited = Vec::new();
    while table.expression_is_valid(expression) && !visited.contains(&expression) {
        visited.push(expression);
        match table.expression(expression) {
            ExpressionNode::ArrayLiteral(_) => return true,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            _ => return false,
        }
    }
    false
}
