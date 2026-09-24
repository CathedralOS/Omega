//! Rejoin the complete authored body before emitting its ordered effects.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, terminal_scalar_type, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<usize, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let end = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .unwrap_or_else(|| {
            if matches!(
                state.terminator,
                CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. }
                    | CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. }
            ) {
                statements.len().saturating_sub(1)
            } else {
                statements.len()
            }
        });
    let prefix = state.bindings.len();
    let marker_count = super::cases::validate_markers(checked, machine, source, state, end)?;
    let tail_value = usize::from(matches!(state.terminator, CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. })
        && state.operations.last().is_some_and(|operation| matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } | CheckedUnitEffectOperationPlan::StructuralCall { result, .. } | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } if result.statement_index as usize == statements.len().saturating_sub(1))));
    // Some operations continue the authored statement an earlier operation
    // began instead of consuming a new one; see `statement_continuations`.
    let continuations = statement_continuations(&state.operations)?;
    let continued = continuations.iter().filter(|continues| **continues).count();
    // A record pattern's marker statement (`let __destructure#x#y = place;`)
    // declares no storage and plans no operation; only its per-field locals
    // do. A whole-record replacement plans one field store per member, all at
    // its one assignment statement.
    // An arm pattern's marker (`__arm_destructure#V=..`) is the same kind of
    // compile-time carrier; a closed-sum terminator already rejoins and
    // counts its markers in `validate_markers`, every other terminator
    // counts them here.
    let closed_sum = matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
    );
    let is_marker = |statement: &StatementNode| {
        is_record_pattern_marker(statement) || (!closed_sum && is_arm_pattern_marker(statement))
    };
    let record_pattern_markers = statements
        .get(prefix..end)
        .unwrap_or_default()
        .iter()
        .filter(|statement| is_marker(statement))
        .count();
    if prefix > end
        || state.operations.len() + marker_count + record_pattern_markers
            != end - prefix + tail_value + continued
    {
        return unsupported("Unit graph dropped or added a body effect");
    }
    let mut next_structural_binding = 0_u32;
    // Mutable storage writes occupy source statements, not immutable result
    // ordinals. Continue the same namespace used by prefix scalar replay.
    let mut next_scalar_binding = u32::try_from(
        state
            .bindings
            .iter()
            .filter(|binding| {
                binding.destination == checked_trees::CheckedScalarBindingDestination::Immutable
            })
            .count(),
    )
    .map_err(|_| LoweringError::Unsupported("Unit graph scalar binding count overflow"))?;
    let mut cursor = prefix;
    for (index, operation) in state.operations.iter().enumerate() {
        let mut check_structural_binding =
            |result: &checked_trees::CheckedUnitStructuralResultBindingPlan|
             -> Result<(), LoweringError> {
                if result.binding_ordinal != next_structural_binding {
                    return unsupported("Unit graph structural binding namespace drifted");
                }
                next_structural_binding = next_structural_binding.checked_add(1).ok_or(
                    LoweringError::Unsupported("Unit graph structural binding count overflow"),
                )?;
                Ok(())
            };
        match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result, calls, ..
            } => {
                // A nested call's result binds in authored evaluation order
                // before the construction that consumes it.
                for call in calls {
                    let CheckedUnitEffectOperationPlan::StructuralCall {
                        result: call_result,
                        ..
                    } = call.operation()
                    else {
                        return unsupported(
                            "Unit graph structural call result lost its operation",
                        );
                    };
                    check_structural_binding(call_result)?;
                }
                check_structural_binding(result)?;
            }
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            // The displaced field value is a fresh structural binding in the
            // same namespace as the call result that replaces it.
            | CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => {
                check_structural_binding(result)?;
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                if result.binding_ordinal != next_scalar_binding {
                    return unsupported("Unit graph scalar binding namespace drifted");
                }
                next_scalar_binding =
                    next_scalar_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph scalar binding count overflow",
                        ))?;
            }
            CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate, result, ..
            } => {
                if result.binding_ordinal != next_scalar_binding {
                    return unsupported("Unit graph scalar binding namespace drifted");
                }
                // A discarded call result occupies its ordinal unclaimed; only
                // a retained result advances the namespace.
                if !crate::emission::call_source_custody::initializers::discards_result(
                    checked,
                    state.state,
                    *coordinate,
                )? {
                    next_scalar_binding =
                        next_scalar_binding
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph scalar binding count overflow",
                            ))?;
                }
            }
            _ => {}
        }
        let ordinal = if continuations[index] {
            // The continued statement is the one the cursor consumed last.
            match authored_statement(operation) {
                Some(statement) if cursor > 0 && statement as usize == cursor - 1 => {
                    statement as usize
                }
                _ => return unsupported("Unit graph continuation left its authored statement"),
            }
        } else {
            while statements.get(cursor).is_some_and(is_marker) {
                cursor += 1;
            }
            let ordinal = cursor;
            cursor += 1;
            ordinal
        };
        match (
            operation,
            statements.get(ordinal).ok_or(LoweringError::Unsupported(
                "Unit graph effect lost its authored statement",
            ))?,
        ) {
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return: false,
                    ..
                },
                StatementNode::LocalData(_) | StatementNode::Expression(_) | StatementNode::Call(_),
            ) if result.statement_index as usize == ordinal => {
                crate::expression_preparation::source_custody::structural::validate(
                    checked,
                    machine,
                    state.state,
                    operation,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
                StatementNode::LocalData(local),
            ) if !local.is_mutable
                && result.statement_index as usize == ordinal
                && checked.primitive_type_reference(local.type_reference)
                    == Some(result.primitive_type) =>
            {
                let role = CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: result.binding_ordinal,
                };
                match value {
                    checked_trees::CheckedCallScalarArgument::Pure(value) => {
                        let (binding, retained) = checked
                            .facts
                            .values
                            .scalar_expressions
                            .bound_expression_at(state.state, result.statement_index, role)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph scalar local has no source binding",
                            ))?;
                        if retained != value {
                            return unsupported("Unit graph scalar local value changed");
                        }
                        crate::expression_preparation::source_custody::validate_pure(
                            checked,
                            binding,
                            terminal_scalar_type(result.primitive_type)?,
                        )?;
                    }
                    checked_trees::CheckedCallScalarArgument::Computation(handle) => {
                        let mut roots = checked
                            .facts
                            .values
                            .scalar_computations
                            .roots
                            .iter()
                            .map(|(_, root)| root)
                            .filter(|root| {
                                root.state == state.state
                                    && root.statement_ordinal == result.statement_index
                                    && root.role == role
                            });
                        let root = roots.next().ok_or(LoweringError::Unsupported(
                            "Unit graph scalar local has no computation root",
                        ))?;
                        if roots.next().is_some() || root.machine != machine || root.root != *handle
                        {
                            return unsupported("Unit graph scalar local computation changed");
                        }
                        crate::expression_preparation::source_custody::validate_computation_calls(
                            checked,
                            machine,
                            state.state,
                            result.statement_index,
                            *handle,
                            local.initial_value,
                        )?;
                    }
                }
            }
            (
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return,
                    ..
                },
                StatementNode::LocalData(_) | StatementNode::Expression(_),
            ) if coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && !discard_result_on_return
                && result.statement_index as usize == ordinal =>
            {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    result,
                    completion_receipts,
                    discard_result_on_return,
                    ..
                },
                StatementNode::LocalData(_) | StatementNode::Call(_) | StatementNode::Expression(_),
            ) if completion_receipts.is_empty()
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && !discard_result_on_return
                && result.statement_index as usize == ordinal =>
            {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            // `place = call(..)` over a structural field of exclusive borrowed
            // storage: the call, the displaced value's move-out, the store of
            // the call result into the opened hole and the displaced value's
            // cleanup all belong to the one assignment. Each rejoins the
            // complete statement roster, so no member stands alone.
            (
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return: false,
                    ..
                },
                StatementNode::Assignment(assignment),
            ) if coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && result.statement_index as usize == ordinal =>
            {
                validate_displaced_field_replacement(
                    checked, machine, source, state, ordinal, assignment,
                )?;
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            // `place = <construction>` over the same kind of field: the
            // establishment replaces the call as the replacing value's
            // producer, and the rest of the roster is the same window pair.
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return: false,
                    ..
                },
                StatementNode::Assignment(assignment),
            ) if result.statement_index as usize == ordinal => {
                validate_displaced_field_replacement(
                    checked, machine, source, state, ordinal, assignment,
                )?;
                crate::expression_preparation::source_custody::structural::validate(
                    checked,
                    machine,
                    state.state,
                    operation,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. },
                StatementNode::Assignment(assignment),
            ) => validate_displaced_field_replacement(
                checked, machine, source, state, ordinal, assignment,
            )?,
            // A discarded call result dies on the same authored statement as
            // its producer; the producer's own custody rejoin already demanded
            // this exact immediate cleanup.
            (
                CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. },
                StatementNode::Call(_),
            ) => {}
            (
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(write),
                StatementNode::Assignment(assignment),
            ) => {
                if write.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a byte-view write");
                }
                crate::emission::byte_sequence_write::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    write,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered an indexed byte store");
                }
                crate::emission::structural_byte_sequence_index_store::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    store,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a byte-field store");
                }
                crate::emission::structural_byte_sequence_store::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    store,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
                    statement_index,
                    destination,
                    path,
                    index,
                    value,
                },
                StatementNode::Assignment(_),
            ) => {
                if *statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered an indexed primitive store");
                }
                let checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } =
                    destination
                else {
                    return unsupported(
                        "Unit graph indexed primitive store has no retained parameter destination",
                    );
                };
                let destination = state
                    .structural_parameters
                    .get(*parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph indexed primitive store parameter is absent",
                    ))?;
                crate::emission::primitive_store::validate_indexed_assignment(
                    checked,
                    machine,
                    state.state,
                    *statement_index,
                    destination,
                    path,
                    index,
                    value,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    destination,
                    path,
                    value,
                },
                StatementNode::Assignment(_),
            ) => {
                if *statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a primitive store");
                }
                let checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } =
                    destination
                else {
                    return unsupported(
                        "Unit graph primitive store has no retained parameter destination",
                    );
                };
                let destination = state
                    .structural_parameters
                    .get(*parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph primitive store parameter is absent",
                    ))?;
                crate::emission::primitive_store::validate_assignment(
                    checked,
                    state.state,
                    *statement_index,
                    destination,
                    path,
                    value,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a field store");
                }
                // The statement's first store validates the whole roster the
                // assignment planned: one field, or every member of a record
                // literal; the rest were covered by that check.
                let statement_stores = state
                    .operations
                    .iter()
                    .filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(other)
                            if other.statement_index == store.statement_index =>
                        {
                            Some(other)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if statement_stores
                    .first()
                    .is_some_and(|first| std::ptr::eq(*first, store))
                {
                    crate::emission::structural_scalar_store_source::validate_assignment_stores(
                        checked,
                        machine,
                        state.state,
                        store.statement_index,
                        assignment,
                        &statement_stores,
                    )?;
                }
            }
            (
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    target_state,
                    structural_arguments,
                    completion_receipts,
                    ..
                },
                StatementNode::Call(_) | StatementNode::Expression(_),
            ) if coordinate.statement_index as usize == ordinal && coordinate.call_ordinal == 0 => {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
                super::claims::validate_boundary_consumption(
                    checked,
                    machine,
                    source,
                    state,
                    *coordinate,
                    *target_state,
                    structural_arguments,
                    completion_receipts,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    claim_transfers,
                    ..
                },
                StatementNode::Call(_) | StatementNode::Expression(_),
            ) if claim_transfers.is_empty()
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0 =>
            {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::ScalarCall {
                    coordinate, result, ..
                },
                StatementNode::LocalData(local),
            ) if !local.is_mutable
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && result.statement_index == coordinate.statement_index
                && checked.primitive_type_reference(local.type_reference)
                    == Some(result.primitive_type) =>
            {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::ScalarCall {
                    coordinate, result, ..
                },
                StatementNode::Assignment(_) | StatementNode::Call(_),
            ) if coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && result.statement_index == coordinate.statement_index =>
            {
                crate::emission::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            _ => return unsupported("Unit graph reordered a source effect"),
        }
    }
    Ok(end)
}

fn is_record_pattern_marker(statement: &StatementNode) -> bool {
    matches!(statement, StatementNode::LocalData(local)
        if local.name.as_str().starts_with("__destructure#"))
}

fn is_arm_pattern_marker(statement: &StatementNode) -> bool {
    matches!(statement, StatementNode::LocalData(local)
        if local.name.as_str().starts_with("__arm_destructure#"))
}

/// The authored statement an effect operation belongs to, when it names one.
fn authored_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } => {
            Some(coordinate.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write.statement_index),
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
            statement_index, ..
        } => Some(*statement_index),
        _ => continued_statement(operation),
    }
}

/// The statement an operation may continue rather than consume: a call's
/// continuation cleanup, a store of its own statement's scalar call result,
/// the member stores of a whole-record replacement, and the move-out/store
/// pair of a displaced borrowed field.
fn continued_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate, .. } => {
            Some(coordinate.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store)
            if matches!(
                store.value,
                checked_trees::CheckedByteSequenceStoreValue::ScalarResult { .. }
            ) =>
        {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)
            if matches!(
                write.value,
                checked_trees::CheckedByteSequenceStoreValue::ScalarResult { .. }
            ) =>
        {
            Some(write.statement_index)
        }
        CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::StoreStructuralField {
            statement_index, ..
        } => Some(*statement_index),
        _ => None,
    }
}

/// Which operations continue the authored statement the most recent
/// statement-consuming operation began. One authored statement may plan
/// several effects, and one rule covers all of them: an operation whose
/// `continued_statement` is that current statement rejoins it; every other
/// operation consumes the next authored statement. Only a plain field store
/// can also begin a statement (the first member of a record replacement);
/// every other continuing kind exists only after its producer, so one that
/// continues nothing lost that producer.
fn statement_continuations(
    operations: &[CheckedUnitEffectOperationPlan],
) -> Result<Vec<bool>, LoweringError> {
    let mut current = None;
    // Whether the current statement began with a construction nested in its
    // own call's arguments: `f(Event::Insert { cents: 50 })` plans the
    // construction first and the call that consumes it second, both at the
    // one call statement. The call, and any further argument construction for
    // it, continue that statement instead of consuming the next.
    let mut began_with_argument = false;
    operations
        .iter()
        .map(|operation| {
            let continued = continued_statement(operation);
            if continued.is_some() && continued == current {
                return Ok(true);
            }
            if began_with_argument
                && authored_statement(operation) == current
                && (argument_construction(operation) || call_operation(operation))
            {
                return Ok(true);
            }
            let may_begin = continued.is_none()
                || matches!(operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)
                    if !matches!(
                        store.value,
                        checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { .. }
                    ));
            if !may_begin {
                return unsupported("Unit graph continuation lost its producing statement");
            }
            current = authored_statement(operation);
            began_with_argument = argument_construction(operation);
            Ok(false)
        })
        .collect()
}

/// A structural construction authored as one of its statement's call
/// arguments rather than bound to a local or returned.
fn argument_construction(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            operand_source: Some(
                checked_trees::CheckedArrayConstructionSource::CallArgument { .. }
            ),
            ..
        }
    )
}

/// An operation that performs its statement's call.
fn call_operation(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::CallUnit { .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
    )
}

/// Rejoin `place = value` where `value` is a whole owned structural value and
/// `place` is a structural field beneath an exclusive borrowed parameter.
/// The checked sequencer plans it as one statement roster, in this order:
/// the value's producer -- the statement's call, or the establishment of its
/// authored construction -- binding the replacing value; `MoveStructuralField`
/// binding the displaced old value and opening the window at `place`;
/// `StoreStructuralField` closing that exact window with the whole produced
/// value; and, for an affine field, the statement's continuation discarding
/// the displaced value.
/// Emission reconstructs the window through `BorrowedWindowLedger`; this
/// check ties every member to the authored assignment so none can be
/// substituted, reordered, or detached from its place.
fn validate_displaced_field_replacement(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
    assignment: &checked_trees::statement::TableAssignment,
) -> Result<(), LoweringError> {
    let roster = state
        .operations
        .iter()
        .filter(|operation| {
            authored_statement(operation).map(|index| index as usize) == Some(ordinal)
        })
        .collect::<Vec<_>>();
    let (producer, moved, place, destination, value, cleanup) = match roster.as_slice() {
        [
            producer,
            CheckedUnitEffectOperationPlan::MoveStructuralField {
                result: moved,
                source: place,
            },
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                destination, value, ..
            },
            cleanup @ ..,
        ] => (*producer, moved, place, destination, value, cleanup),
        _ => {
            return unsupported("Unit graph field replacement is not one producer and window pair");
        }
    };
    // The producer is the value the authored right-hand side denotes: the
    // statement's own call, or the establishment of its rooted construction.
    let produced = match (
        producer,
        checked.expression_table.expression(assignment.value),
    ) {
        (
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return: false,
                ..
            },
            checked_trees::expression::ExpressionNode::Call(_),
        ) if coordinate.call_ordinal == 0 => result,
        (
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                value: established,
                discard_result_on_return: false,
                ..
            },
            _,
        ) if checked
            .facts
            .values
            .structural_values
            .root_for_expression(state.state, result.statement_index, assignment.value)
            .is_some_and(|root| root.root == *established) =>
        {
            result
        }
        _ => return unsupported("Unit graph field replacement has no authored producer"),
    };
    let identity = &produced.type_identity;
    if place != destination
        || place.access != checked_trees::CheckedStructuralAccess::Owned
        || place.path.is_empty()
        || &place.type_identity != identity
        || &moved.type_identity != identity
        || moved.multiplicity != produced.multiplicity
        || moved.statement_index != produced.statement_index
    {
        return unsupported("Unit graph field replacement window names another place or type");
    }
    if value.source
        != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: produced.binding_ordinal,
        })
        || !value.path.is_empty()
        || value.access != checked_trees::CheckedStructuralAccess::Owned
        || &value.type_identity != identity
    {
        return unsupported("Unit graph field replacement stores another value than its call");
    }
    // The displaced value is disposed exactly as the checked plan retires it:
    // an affine value dies whole on the call continuation; a copy value has
    // no disposal obligation and no continuation.
    let displaced = checked_trees::CheckedUnitPartialAffineDiscardPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: moved.binding_ordinal,
        },
        path: Vec::new(),
        type_identity: identity.clone(),
    };
    let disposal = match (moved.multiplicity, cleanup) {
        (language_semantics::Multiplicity::Unrestricted, []) => true,
        (
            language_semantics::Multiplicity::Affine,
            [
                CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards,
                },
            ],
        ) => {
            coordinate.statement_index == produced.statement_index
                && coordinate.call_ordinal == 0
                && affine_discards.as_slice() == std::slice::from_ref(&displaced)
        }
        _ => false,
    };
    if !disposal {
        return unsupported("Unit graph displaced field value lost its planned disposal");
    }
    let target = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state.state,
        Some(ordinal),
        assignment.target,
    )?;
    let root = place
        .source_parameter_index()
        .and_then(|position| checked.state_parameters(source).get(position as usize))
        .ok_or(LoweringError::Unsupported(
            "Unit graph field replacement has no authored parameter root",
        ))?;
    if target.root != root.symbol || target.path != place.path {
        return unsupported("Unit graph field replacement drifted from its authored place");
    }
    Ok(())
}
