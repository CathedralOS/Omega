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

/// Preserve the scalar lane's conservative overlap admission after replaying
/// the exact authored observation roster shared with all call result families.
pub(crate) fn rejoin(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    call: &BorrowCallFact,
    arguments: &[ExpressionHandle],
) -> Result<Vec<ArgumentAccessExtent>, LoweringError> {
    let positions = replay(checked, machine, state, statement, call, arguments)?;
    let rows = checked
        .facts
        .borrow
        .argument_accesses
        .span(call.accesses)
        .ok_or(LoweringError::Unsupported(
            "computed shared borrow has a stale observation roster",
        ))?;
    for (position, row) in rows.iter().enumerate() {
        for (later_position, later) in rows.iter().enumerate().skip(position + 1) {
            if row.root_symbol == later.root_symbol
                && (row.kind.is_exclusive() || later.kind.is_exclusive())
                && row.end_before_access.is_none_or(|end| end > later_position)
            {
                return unsupported("computed shared borrow aliases an exclusive observation");
            }
        }
    }
    Ok(positions)
}

/// Authenticate every ordered observation and its lifetime against authored
/// places. This does not grant loans or replace path-sensitive conflict checks.
pub(crate) fn replay(
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
        // The borrow roster records reads of captured places, not the type of
        // a newly produced range descriptor. Its dynamic Index projects the
        // collection element; a closed FixedRange has no scalar place type.
        // Reconstruct that exact distinction before checking the recorded end.
        // This is only the read row's lifetime, not the descriptor's backing
        // loan. Explicit/forwarded loans bypass copy expiry below; view source
        // custody and Terminal independently retain their backing lifetime.
        if let ExpressionNode::Indexed(indexed) = table.expression(expression)
            && let ExpressionNode::Range(range) = table.expression(indexed.index)
        {
            let fixed_start = !range.start.is_valid()
                || table
                    .constant_integer_value(range.start)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some();
            let fixed_end = range.end.is_valid()
                && table
                    .constant_integer_value(range.end)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some();
            if fixed_start && fixed_end {
                return false;
            }
            return validation::expression_result_type_reference(
                &checked.typed,
                source_machine,
                source_state,
                indexed.collection,
            )
            .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
            .and_then(
                |reference| match checked.type_reference_table.type_reference(reference) {
                    checked_trees::types::TypeReferenceNode::Slice { element_type }
                    | checked_trees::types::TypeReferenceNode::FixedArray {
                        element_type, ..
                    } => Some(*element_type),
                    _ => None,
                },
            )
            .is_some_and(|element| {
                validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    element,
                ) && checked.type_multiplicity(element)
                    == language_semantics::Multiplicity::Unrestricted
            });
        }
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
    // Only the exact terminal occurrence observes payloads already extracted
    // by dispatch. Nested calls on the same statement keep their own schedule.
    let dispatch_extraction = if matches!(
        checked
            .statement_table
            .statements(source_state.statement_nodes)
            .get(statement as usize),
        Some(checked_trees::statement::StatementNode::Transition(_))
    )
        && crate::emission::call_source_custody::authored::transition_target_at_call(
            checked,
            state,
            checked_trees::CheckedUnitCallCoordinate {
                statement_index: statement,
                call_ordinal: call.call_ordinal as u32,
            },
        )
        .is_ok()
    {
        validation::DispatchExtraction::of_transition_arm(
            &checked.typed,
            source_state,
            statement as usize,
        )
    } else {
        validation::DispatchExtraction::default()
    };
    let mut positions = Vec::with_capacity(arguments.len());
    let selected_access = |argument, formal| {
        // A range computes a new descriptor. It is observed in source order,
        // unlike forwarding an already established reference carrier, whose
        // place selectors are evaluated before its retained loan.
        if matches!(table.expression(argument), ExpressionNode::Indexed(indexed)
            if matches!(table.expression(indexed.index), ExpressionNode::Range(_)))
        {
            return None;
        }
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
    let signature = crate::emission::call_source_custody::authored::target_signature(
        checked,
        machine,
        call.target_symbol,
    )?;
    let parameters = explicit_parameters(signature.parameters, arguments.len())?;
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
                // Place evaluation already replayed a temporary's producer and
                // its operands. A computed result has no named capture row;
                // result custody independently owns the temporary's live loan.
                (
                    has_named_place_root(checked, target)?.then_some(target),
                    kind,
                )
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
                    ExpressionNode::Name(_)
                        if validation::exact_case_reference_owner(&checked.typed, expression)
                            .is_some() =>
                    {
                        (None, BorrowAccessKind::Read)
                    }
                    ExpressionNode::Name(_) => (Some(expression), BorrowAccessKind::Read),
                    ExpressionNode::Member(member)
                        if !has_named_place_root(checked, expression)? =>
                    {
                        // A copied payload reads the subject the dispatch
                        // already evaluated. Match the checking access walk:
                        // computed subjects contribute no second receiver read.
                        if !(member.case_variant.is_some()
                            && dispatch_extraction.reads_extracted_copy(
                                &checked.typed,
                                expression,
                                plain_copy,
                            ))
                        {
                            children.push(member.receiver);
                        }
                        (None, BorrowAccessKind::Read)
                    }
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
                        if typed_trees::byte_predicates::byte_predicate_call(
                            &checked.typed,
                            expression,
                        )
                        .is_some()
                        {
                            if !call.carries_only_positional_arguments()
                                || !call.selects_only_nominal_route()
                            {
                                return unsupported(
                                    "byte observation has no positional operand custody",
                                );
                            }
                            let (_, operand) =
                                super::super::byte_predicates::operands(checked, expression)?;
                            // Primitive observations read their operand; they
                            // have no machine signature or formal capable of
                            // lending mutable access. Ordinary resolved calls
                            // below retain their exact formal-directed loans.
                            children.push(operand);
                        } else {
                            // A resolved static qualifier names the callee's owner;
                            // it is not storage observed while evaluating the call.
                            // Unknown selections keep the receiver on the replay path.
                            let no_runtime_receiver =
                                checked.call_has_no_runtime_receiver_for_target(call);
                            if call.receiver.is_valid() && !no_runtime_receiver {
                                receiver = Some(call.receiver);
                            }
                            let arguments = table.expression_handles(call.arguments);
                            if arguments.len() != call.arguments.count() as usize {
                                return unsupported(
                                    "computed shared borrow has stale nested argument observations",
                                );
                            }
                            let signature =
                                crate::emission::call_source_custody::authored::target_signature(
                                    checked,
                                    machine,
                                    call.target_symbol,
                                )?;
                            let parameters =
                                explicit_parameters(signature.parameters, arguments.len())?;
                            for (argument, parameter) in arguments.iter().zip(parameters) {
                                let access = selected_access(*argument, parameter.type_reference);
                                argument_children.push((*argument, access));
                            }
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
                    ExpressionNode::Indexed(indexed)
                        if !has_named_place_root(checked, expression)? =>
                    {
                        children.extend([indexed.collection, indexed.index]);
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
                    crate::emission::call_source_custody::projected_receivers::observation_place(
                        checked,
                        machine,
                        state,
                        statement as usize,
                        named,
                    )?
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
                // Saved dispatch payloads finish before target access 0; other
                // scalar copies finish here. Loans retain their call lifetime.
                if direct {
                    operand = Some(position);
                }
                observation_ends.push(
                    (!observing_loan
                        && !matches!(table.expression(expression), ExpressionNode::Borrow(_))
                        && plain_copy(expression))
                    .then(|| {
                        if dispatch_extraction.reads_extracted_copy(
                            &checked.typed,
                            expression,
                            plain_copy,
                        ) {
                            0
                        } else {
                            position + 1
                        }
                    }),
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
    }
    Ok(positions)
}

/// Distinguish stored projections from a computed value's member/index reads.
/// This only chooses which evaluation to replay; the existing canonical-place
/// resolver still authenticates every named storage root and full projection.
fn has_named_place_root(
    checked: &CheckedTrees,
    mut expression: ExpressionHandle,
) -> Result<bool, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("call observation has a stale or cyclic place source");
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Name(_) => {
                return Ok(
                    validation::exact_case_reference_owner(&checked.typed, expression).is_none(),
                );
            }
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => expression = cast.value,
            _ => return Ok(false),
        }
    }
}

/// The source may spell self explicitly or supply it as a receiver. Keep the
/// original explicit order, including erased formals, for observation replay.
fn explicit_parameters(
    parameters: &[checked_trees::signature::StateParameter],
    argument_count: usize,
) -> Result<Vec<&checked_trees::signature::StateParameter>, LoweringError> {
    let nonself_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = argument_count > nonself_count;
    let explicit = parameters
        .iter()
        .filter(|parameter| explicit_self || !parameter.is_self)
        .collect::<Vec<_>>();
    if explicit.len() != argument_count {
        return unsupported("observation arguments disagree with the selected signature");
    }
    Ok(explicit)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_observation_access_replays_exact_subject_and_resolved_permissions() {
        for (declaration, expected) in [
            ("", BorrowAccessKind::Read),
            (
                "machine valid_utf8(bytes: &mut [u8; 4]) -> bool { bytes[0] = 0; false }",
                BorrowAccessKind::Mutable,
            ),
        ] {
            let checked = crate::front_end::checked_program(&format!(
                "{declaration}
                 boundary trait Output {{ machine flag(value: bool) reaches Output; }}
                 machine inspect(bytes: &mut [u8; 4], other: &[u8; 4]) reaches Output {{
                     Output::flag(valid_utf8(bytes));
                 }}"
            ));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "inspect")
                .expect("inspect");
            let state = &checked.machine_states(machine)[0];
            let other = checked
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name.as_str() == "other")
                .expect("other")
                .symbol;
            let borrow_state = checked
                .facts
                .borrow
                .states
                .iter()
                .map(|(_, row)| row)
                .find(|row| row.state_symbol == state.symbol)
                .expect("borrow state");
            let call = checked
                .facts
                .borrow
                .calls
                .span(borrow_state.calls)
                .expect("calls")
                .iter()
                .find(|call| call.call_ordinal == 0)
                .expect("outer call")
                .clone();
            let checked_trees::statement::StatementNode::Call(authored) =
                &checked.statement_table.statements(state.statement_nodes)[call.statement_index]
            else {
                panic!("outer statement")
            };
            let arguments = checked
                .statement_table
                .expression_handles(authored.arguments)
                .to_vec();
            let machine = machine.symbol;
            let state = state.symbol;
            replay(
                &checked,
                machine,
                state,
                call.statement_index as u32,
                &call,
                &arguments,
            )
            .expect("primitive reads and resolved mutable calls replay independently");
            let [row] = checked
                .facts
                .borrow
                .argument_accesses
                .span(call.accesses)
                .expect("accesses")
            else {
                panic!("one byte subject observation")
            };
            assert_eq!(row.kind, expected);
            for change_root in [false, true] {
                let mut changed = checked.clone();
                let [row] = changed
                    .facts
                    .borrow
                    .argument_accesses
                    .span_mut(call.accesses)
                    .expect("accesses")
                else {
                    panic!("one observation")
                };
                if change_root {
                    row.root_symbol = other;
                } else {
                    row.kind = if expected == BorrowAccessKind::Read {
                        BorrowAccessKind::Mutable
                    } else {
                        BorrowAccessKind::Read
                    };
                }
                assert!(
                    replay(
                        &changed,
                        machine,
                        state,
                        call.statement_index as u32,
                        &call,
                        &arguments
                    )
                    .is_err(),
                    "subject and permission are independently authenticated"
                );
            }
        }
    }
}
