//! Rejoin the complete authored body before emitting its ordered effects.

use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, Multiplicity, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use checked_trees::statement::StatementNode;
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult};

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
                    | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
            ) {
                statements.len().saturating_sub(1)
            } else {
                statements.len()
            }
        });
    let prefix = state.bindings.len();
    let marker_count = super::cases::validate_markers(checked, machine, source, state, end)?;
    // A case dispatch over a computed subject evaluates its one call at the
    // first arm's statement, ahead of the dispatch; `cases` rejoins it.
    let case_subject_call = match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::ClosedSum {
            subject:
                checked_trees::CheckedUnitStructuralArgumentPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        },
                    ..
                },
            ..
        } => super::cases::computed_subject(state, *binding_ordinal)?,
        _ => None,
    }
    .filter(|call| call.coordinate.statement_index as usize == end);
    let returned_scalar = returned_scalar_binding(state);
    let tail_value = usize::from(
        matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. }
        ) && state.operations.last().is_some_and(|operation| {
            // Every structural producer occupies its authored occurrence.
            // Result and source replay below establish whether that value is
            // the completion; producer families do not change effect count.
            let result = operation.structural_result_binding();
            result.is_some_and(|result| {
                result.statement_index as usize == statements.len().saturating_sub(1)
            })
        }),
    )
        // A scalar final expression's own producer occupies the tail statement:
        // a scalar local or an ordinary scalar call. A terminal machine call
        // at a closing transition counts as `terminal_call` below instead.
        + usize::from(returned_scalar.is_some_and(|binding| {
            binding.statement_index as usize + 1 == statements.len()
                && matches!(statements.last(), Some(StatementNode::Expression(_)))
                && state.operations.last().is_some_and(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                        result == binding
                    }
                    CheckedUnitEffectOperationPlan::Call(call) => {
                        !call.is_boundary() && call.scalar_result() == Some(binding)
                    }
                    _ => false,
                })
        }));
    // An unconditional terminal machine call runs as the state's last
    // operation at its own transition; the `ReturnUnit` terminator completes
    // after it, or `ReturnScalar` returns the scalar result the call bound at
    // that transition. A guarded arm's call is never here: it runs in its arm.
    let terminal_call = (matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
    ) || returned_scalar
        .is_some_and(|binding| binding.statement_index as usize == end))
        && end < statements.len()
        && crate::call_sites::linear_terminal_invocation(
            checked,
            state.state,
            u32::try_from(end)
                .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?,
        )?;
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
    // An atomic writing carrier's result placeholder (`let prior: T = 0;`
    // ahead of `place = Atomic { .. }`) is the same kind of carrier: it
    // declares the prior's binding, which the carrier's one atomic event
    // defines at the next statement.
    // An exclusive or shared loan local (`let seen: &mut u8 = &mut self.byte;`,
    // `let b: &u32 = &self.size;`) is the same kind of carrier: it plans no
    // operation, so its statement is a marker like the record pattern's.
    // reseated, so it names one place for its scope and holds no value of its
    // own: each write through it is planned as the ordinary store of that
    // place, and the binding itself plans nothing.
    let is_marker_at = |index: usize| {
        let marker_kind = statements.get(index).and_then(|statement| {
            if is_record_pattern_marker(statement) {
                Some("record")
            } else if !closed_sum && is_arm_pattern_marker(statement) {
                Some("arm")
            } else if !super::super::super::statement_custody::statement_owes_operation(
                checked,
                machine,
                state.state,
                index,
                statement,
            ) {
                Some("custody")
            } else {
                None
            }
        });
        marker_kind.is_some() || crate::emission::atomic_sources::result_placeholder(checked, statements, index)
            // A runtime-indexed dispatch's shared subject plans no operation:
            // each arm observes the borrowed element itself.
            || (crate::expression_preparation::source_custody::case_sources::is_erased_dispatch_subject(
                checked, statements, index,
            ) && !state.operations.iter().any(|operation| {
                // The same planned-result test the loan markers use: a
                // subslice-shaped subject forwarded into a guarded arm
                // establishes its own view, so its statement produced a real
                // operation and cannot recount as the dispatch's no-op
                // subject.
                operation
                    .structural_result_binding()
                    .is_some_and(|result| result.statement_index as usize == index)
            }))
    };
    let record_pattern_markers = (prefix..end).filter(|index| is_marker_at(*index)).count();
    if prefix > end
        || state.operations.len() + marker_count + record_pattern_markers
            != end - prefix
                + tail_value
                + continued
                + usize::from(case_subject_call.is_some())
                + usize::from(terminal_call)
    {
        return unsupported("Unit graph dropped or added a body effect");
    }
    crate::emission::primitive_store::validate_element_rosters(
        checked,
        state.state,
        &state.operations,
    )?;
    let mut cursor = prefix;
    for (index, operation) in state.operations.iter().enumerate() {
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. },
                ..
            }
        ) {
            // Expiry runs before entry and consumes no authored statement.
            continue;
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } = operation {
            for call in calls {
                if call
                    .operation()
                    .call()
                    .filter(|planned| !planned.is_boundary())
                    .and_then(CheckedUnitCallPlan::structural_result)
                    .is_none()
                {
                    return unsupported("Unit graph structural call result lost its operation");
                }
            }
        }
        let ordinal = if continuations[index] {
            // The continued statement is the one the cursor consumed last.
            // A reference release names the upcoming statement instead (its
            // expiry boundary), so it takes the last consumed index directly.
            match authored_statement(operation) {
                Some(_)
                    if matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                    ) && cursor > 0 =>
                {
                    cursor - 1
                }
                Some(statement) if cursor > 0 && statement as usize == cursor - 1 => {
                    statement as usize
                }
                _ => return unsupported("Unit graph continuation left its authored statement"),
            }
        } else {
            while cursor < statements.len() && is_marker_at(cursor) {
                cursor += 1;
            }
            let ordinal = cursor;
            cursor += 1;
            ordinal
        };
        let authored = statements.get(ordinal).ok_or(LoweringError::Unsupported(
            "Unit graph effect lost its authored statement",
        ))?;
        // A store replays only at the assignment its plan names.
        if matches!(authored, StatementNode::Assignment(_))
            && let Some(reordered) = reordered_store(operation, ordinal)
        {
            return unsupported(reordered);
        }
        match (operation, authored) {
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    operand_source: Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. }),
                    ..
                },
                _,
            ) if result.statement_index as usize == ordinal => {
            }
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    value,
                    discard_result_on_return: true,
                    ..
                },
                StatementNode::LocalData(_)
                | StatementNode::Expression(_)
                | StatementNode::Call(_),
            ) if result.statement_index as usize == ordinal
                && returning_view_discard(checked, machine, state, index, result, *value) =>
            {
            }
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    value,
                    discard_result_on_return: false,
                    ..
                },
                StatementNode::LocalData(_)
                | StatementNode::Expression(_)
                | StatementNode::Call(_)
                // A value-returning transition tail establishes its result on
                // the transition's own statement — the value arm's source.
                | StatementNode::Transition(_),
            ) if result.statement_index as usize == ordinal
                && !returning_view_discard(checked, machine, state, index, result, *value) =>
            {
            }
            // A mutable primitive local's establishment consumes its authored
            // `let` by the same ordinal and typing agreement; the storage
            // roster validator re-joins its `StorageInitializer` binding and
            // mutable custody separately.
            (
                CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                    statement_index,
                    primitive_type,
                    ..
                },
                StatementNode::LocalData(local),
            ) if local.is_mutable
                && *statement_index as usize == ordinal
                && checked.primitive_type_reference(local.type_reference)
                    == Some(*primitive_type) =>
            {}
            (
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
                StatementNode::LocalData(local),
            ) if !local.is_mutable
                && result.statement_index as usize == ordinal
                && checked.primitive_type_reference(local.type_reference)
                    == Some(result.primitive_type) =>
            {
                // Source rows key the local by its authored ordinal, not the
                // dense binding ordinal its plan assigns.
                let role = CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal:
                        crate::expression_preparation::source_custody::source_local_ordinal(
                            checked,
                            state.state,
                            result.statement_index,
                        )?,
                };
                match value {
                    checked_trees::CheckedCallScalarArgument::Pure(value) => {
                        let (_, retained) = checked
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
                    }
                }
            }
            // A scalar-result state's final expression: the Return-role value
            // the ordinary completion evaluates at the tail statement.
            (
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
                StatementNode::Expression(expression),
            ) if returned_scalar == Some(result)
                && result.statement_index as usize == ordinal
                && ordinal + 1 == statements.len() =>
            {
                validate_returned_value(checked, machine, state, result, value, *expression)?;
            }
            // An ordinary scalar call owns the returned final expression.
            (CheckedUnitEffectOperationPlan::Call(call), StatementNode::Expression(_))
                if let Some(result) = call.scalar_result()
                    && !call.is_boundary()
                    && returned_scalar == Some(result)
                    && call.coordinate.statement_index as usize == ordinal
                    && call.coordinate.call_ordinal == 0
                    && result.statement_index == call.coordinate.statement_index
                    && ordinal + 1 == statements.len() =>
            {
            }
            // A computed case subject and its prepared operand calls occupy
            // the first arm's statement. Only their canonical evaluated
            // occurrences belong here; copied receivers read saved storage.
            (CheckedUnitEffectOperationPlan::Call(call), StatementNode::Transition(_))
                if case_subject_call.is_some()
                    && ordinal == end
                    && matches!(call.source_site().flatten(),
                        Some(checked_trees::NominalMachineUseSite::Expression(expression))
                            if validation::saved_dispatch_call(
                                &checked.typed, source, ordinal, expression,
                            ) == Some((ordinal, expression))) =>
            {
            }
            // A kept structural result, ordinary or boundary. A boundary
            // call's claim receipts rejoin checked consumption the same way
            // its Unit and scalar siblings' do.
            (
                CheckedUnitEffectOperationPlan::Call(call),
                StatementNode::LocalData(_) | StatementNode::Expression(_) | StatementNode::Call(_),
            ) if kept_structural_call_at(call, ordinal) => {
                validate_boundary_call_consumption(checked, machine, source, state, call)?;
            }
            // `place = call(..)` over a structural field of exclusive borrowed
            // storage: the call, the displaced value's move-out, the store of
            // the call result into the opened hole and the displaced value's
            // cleanup all belong to the one assignment. Each rejoins the
            // complete statement roster, so no member stands alone. Boundary
            // and ordinary calls produce the same replacement window; the
            // call owner separately checks their distinct authority contracts.
            (CheckedUnitEffectOperationPlan::Call(call), StatementNode::Assignment(assignment))
                if kept_structural_call_at(call, ordinal) =>
            {
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
            }
            (
                CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. },
                StatementNode::Assignment(_),
            ) => {}
            // A result or a borrowed argument owner dies at the exact call's
            // continuation, regardless of the statement's surface spelling.
            // Operand custody independently rejoins this immediate cleanup.
            (
                CheckedUnitEffectOperationPlan::ContinuationCleanup { .. },
                _,
            ) => {}
            (
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(_),
                StatementNode::Assignment(_),
            ) => {
            }
            (
                CheckedUnitEffectOperationPlan::ElementViewWrite(_),
                StatementNode::Assignment(_),
            ) => {
            }
            (
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_),
                StatementNode::Assignment(_),
            ) => {
            }
            (
                CheckedUnitEffectOperationPlan::ByteSequenceStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                // A record literal made only of view members has no scalar
                // store to own its roster; its first member store does.
                let view_members = record_view_members(state, store.statement_index);
                let has_scalar_members = state.operations.iter().any(|operation| {
                    matches!(operation,
                        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(other)
                            if other.statement_index == store.statement_index)
                });
                if !has_scalar_members
                    && view_members
                        .first()
                        .is_some_and(|first| std::ptr::eq(*first, store))
                    && matches!(
                        checked.expression_table.expression(assignment.value),
                        checked_trees::expression::ExpressionNode::StructLiteral(_)
                    )
                {
                }
            }
            (
                CheckedUnitEffectOperationPlan::RepresentationViewStore(_),
                StatementNode::Assignment(_),
            ) => {
            }
            // One atomic event occupies its carrier statement: the assignment
            // for a writing event, the local for a load. Emission rejoins the
            // event, place, orderings, operands and prior binding to that
            // carrier (`emission::atomic_sources`) before it emits.
            (
                CheckedUnitEffectOperationPlan::AtomicAccess(access),
                StatementNode::Assignment(_) | StatementNode::LocalData(_),
            ) if access.statement_index as usize == ordinal => {}
            (
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    destination,
                    path,
                    ..
                },
                StatementNode::Assignment(_),
            ) => {
                let position = match destination {
                    checked_trees::CheckedStoreRoot::Parameter { position } => position,
                    // A whole store to a primitive local this state already
                    // established rejoins its authored assignment exactly as
                    // an ordinary body's does.
                    checked_trees::CheckedStoreRoot::Local { symbol }
                        if path.is_empty()
                            && state.operations[..index].iter().any(|earlier| {
                                matches!(earlier,
                                    CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                                        symbol: established, ..
                                    } if established == symbol)
                            }) =>
                    {
                        crate::emission::primitive_store::validate_symbol_assignment(
                            checked,
                            state.state,
                            *statement_index,
                            path,
                        )?;
                        continue;
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol } if !path.is_empty() => {
                        // Projected primitive stores use the same established
                        // structural locals as ordinary bodies. Rejoin the
                        // earlier declaration and exact assignment here;
                        // emission still requires its live owned storage. A
                        // borrowed-view local writes through its loan: its
                        // authored target resolves through the same place, so
                        // the store's path continues the initializer's path
                        // rather than the local's own symbol.
                        let mut prefix = crate::emission::structural_scalar_store_source::local_store_destination(
                            checked,
                            machine,
                            source,
                            state.state,
                            *statement_index,
                            *symbol,
                        )?;
                        prefix.extend(path.iter().cloned());
                        crate::emission::primitive_store::validate_symbol_assignment(
                            checked,
                            state.state,
                            *statement_index,
                            &prefix,
                        )?;
                        continue;
                    }
                    checked_trees::CheckedStoreRoot::Local { .. } => {
                        return unsupported("Unit graph primitive store has no established local");
                    }
                };
                let destination = state
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.position == *position)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph primitive store parameter is absent",
                    ))?;
                crate::emission::primitive_store::validate_assignment(
                    checked,
                    state.state,
                    *statement_index,
                    destination,
                    path,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
                StatementNode::Assignment(_),
            ) => {
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
                let _ = record_view_members(state, store.statement_index);
                if statement_stores
                    .first()
                    .is_some_and(|first| std::ptr::eq(*first, store))
                {
                }
            }
            (
                CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_),
                StatementNode::Assignment(_),
            ) => {
            }
            // A Unit call retains its exact occurrence and ordinary argument
            // custody. Boundary receipts and ordinary claim transfers are
            // independently checked by their shared call consumers.
            (
                CheckedUnitEffectOperationPlan::Call(call),
                StatementNode::Call(_) | StatementNode::Expression(_),
            ) if *call.result() == CheckedUnitCallResult::Unit
                && call.coordinate.statement_index as usize == ordinal
                && call.coordinate.call_ordinal == 0 =>
            {
                validate_boundary_call_consumption(checked, machine, source, state, call)?;
            }
            // The unconditional terminal machine call at the state's closing
            // transition, counted above; its Unit completion follows it, or
            // the state returns the scalar result it binds there.
            (CheckedUnitEffectOperationPlan::Call(call), StatementNode::Transition(_))
                if terminal_call
                    && ordinal == end
                    && match call.result() {
                        CheckedUnitCallResult::Unit => returned_scalar.is_none(),
                        CheckedUnitCallResult::Scalar(result) => {
                            !call.is_boundary() && returned_scalar == Some(result)
                        }
                        CheckedUnitCallResult::Structural { .. } => false,
                    }
                    && call.coordinate.statement_index as usize == ordinal
                    && call.coordinate.call_ordinal == 0 =>
            {
                validate_boundary_call_consumption(checked, machine, source, state, call)?;
            }
            // A scalar call bound by an immutable `let`: the local's primitive
            // is the call's result. A boundary call replays like an ordinary
            // scalar call and consumes its boundary like a Unit boundary call.
            (CheckedUnitEffectOperationPlan::Call(call), StatementNode::LocalData(local))
                if let Some(result) = call.scalar_result()
                    && !local.is_mutable
                    && call.coordinate.statement_index as usize == ordinal
                    && call.coordinate.call_ordinal == 0
                    && result.statement_index == call.coordinate.statement_index
                    && checked.primitive_type_reference(local.type_reference)
                        == Some(result.primitive_type) =>
            {
                validate_boundary_call_consumption(checked, machine, source, state, call)?;
            }
            // A scalar call whose result an assignment stores into a field
            // replays like the `let`-bound arm: the store's checked value is
            // the call's scalar result (its primitive matched the declared
            // result type at plan construction).
            (
                CheckedUnitEffectOperationPlan::Call(call),
                StatementNode::Assignment(_) | StatementNode::Call(_),
            ) if call.scalar_result().is_some_and(|result| {
                result.statement_index == call.coordinate.statement_index
            }) && call.coordinate.statement_index as usize == ordinal
                && call.coordinate.call_ordinal == 0 =>
            {
                validate_boundary_call_consumption(checked, machine, source, state, call)?;
            }
            (
                CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. },
                StatementNode::LocalData(_) | StatementNode::Expression(_) | StatementNode::Transition(_),
            ) if result.statement_index as usize == ordinal => {
                super::super::super::byte_locals::validate(checked, state.state, operation)?;
            }
            (
                CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. },
                StatementNode::LocalData(_) | StatementNode::Expression(_) | StatementNode::Transition(_),
            ) if result.statement_index as usize == ordinal => {
                super::super::super::view_ranges::binding_site(checked, state.state, operation)?;
            }
            // A deferred structural local's establishment consumes its own
            // declaration: the authored `let` is exactly the statement the
            // storage begins at, and member stores rejoin that home later.
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralLocal { result, .. },
                StatementNode::LocalData(_),
            ) if result.statement_index as usize == ordinal => {}
            // The shared replay rejoins either the declaration or the exact
            // call/formal that owns this array's elements. A call operand is
            // part of that call's statement, not an additional body effect.
            (
                CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source,
                    result,
                    elements,
                },
                _,
            ) if result.statement_index as usize == ordinal => {
                super::super::super::scalar_arrays::validate(
                    checked,
                    machine,
                    state.state,
                    *source,
                    result,
                    elements,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::EstablishReference { result, source },
                StatementNode::LocalData(_),
            ) if result.statement_index as usize == ordinal => {
                super::super::super::reference_results::validate_local_establishment(checked, state.state, result)?;
            }
            // A returned reference occupies its completion statement, regardless
            // of borrow access or expression-versus-transition spelling. This
            // walk checks order and the exact produced binding; `returns` then
            // independently rejoins the authored source, access, path and type.
            // Repeating borrow-shape admission here made ordinary mutable
            // expression tails fail before that semantic replay could run.
            (
                CheckedUnitEffectOperationPlan::EstablishReference { result, .. },
                StatementNode::Expression(_) | StatementNode::Transition(_),
            ) if result.statement_index as usize == ordinal
                && matches!(
                    &state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned }
                        if returned.source
                            == checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            }
                ) => {}
            // A reference release rejoins either the statement at its expiry
            // boundary or, when the boundary's own operations have not yet
            // been consumed, the statement the walk last attributed. The
            // stamped boundary is authoritative: a loan whose last use sits
            // inside the terminator itself weakens one step past the final
            // authored statement, so any boundary at or past the last
            // attributed ordinal rejoins.
            (
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index,
                    binding_ordinal,
                    loan,
                    ..
                },
                _,
            ) if *statement_index as usize >= ordinal => {
                // The release also pairs with the unique earlier structural
                // call carrying this loan, at exactly the boundary the borrow
                // checker recorded — the ordering walk's `>=` rejoin is the
                // position check, not the custody proof.
                super::super::super::reference_results::validate_composed_release(
                    checked,
                    machine,
                    state,
                    index,
                    *binding_ordinal,
                    *loan,
                    *statement_index,
                )?;
            }
            _ => return unsupported("Unit graph reordered a source effect"),
        }
    }
    Ok(end)
}

/// Replay the one checked loan release that makes an affine slice-view
/// carrier disposable on every return arm. The returned reference is
/// established separately inside the arm; this prefix carrier has no later
/// reborrow rooted at that view which could own the same release.
fn returning_view_discard(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    operation_index: usize,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    value: checked_trees::CheckedStructuralValueHandle,
) -> bool {
    let Some(view_source) = checked
        .facts
        .values
        .structural_values
        .nodes
        .is_valid(value)
        .then(|| &checked.facts.values.structural_values.nodes.get(value).kind)
        .and_then(|kind| match kind {
            checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                Some(source)
            }
            _ => None,
        })
    else {
        return false;
    };
    let Ok((_, source_state)) = crate::declarations::state_declaration(checked, state.state) else {
        return false;
    };
    let parameter_symbol = |parameter_index: u32| {
        let parameter = state.structural_parameters.get(parameter_index as usize)?;
        let authored = checked
            .typed
            .state_parameters(source_state)
            .get(parameter.position as usize)?;
        Some(authored.symbol)
    };
    let same_root = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| match (
        &argument.source,
        &view_source.source,
    ) {
        (
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        ) => parameter_symbol(*parameter_index) == Some(*symbol),
        (
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        ) => Some(*symbol) == parameter_symbol(*parameter_index),
        _ => argument.source == view_source.source,
    };

    // Enum dispatch and ordered guards have the same physical cleanup only
    // when every selected exit returns. A successor may carry this view into
    // another state, so it cannot borrow an all-return disposal permission.
    let all_returning = match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::Guarded { .. } => true,
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
            !cases.is_empty()
                && cases.iter().all(|case| {
                    let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
                        checked_trees::CheckedConditionalReturnArm::Structural(
                            CheckedUnitEffectOperationPlan::EstablishReference { source, .. },
                        ),
                    ) = &case.outcome
                    else {
                        return false;
                    };
                    same_root(source)
                        && matches!(
                            (view_source.access, source.access),
                            (
                                checked_trees::CheckedStructuralAccess::MutableBorrow,
                                checked_trees::CheckedStructuralAccess::MutableBorrow
                                    | checked_trees::CheckedStructuralAccess::SharedBorrow,
                            ) | (
                                checked_trees::CheckedStructuralAccess::SharedBorrow,
                                checked_trees::CheckedStructuralAccess::SharedBorrow,
                            )
                        )
                        && source.path.starts_with(&view_source.path)
                        && (source.path.len() > view_source.path.len()
                            || source.type_identity == view_source.type_identity)
                })
        }
        _ => false,
    };
    result.multiplicity == Multiplicity::Affine
        && all_returning
        && !state.operations[operation_index + 1..]
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishReference { source, .. }
                        if same_root(source) && source.path.starts_with(&view_source.path)
                )
            })
        && checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.kind == language_semantics::PermissionEventKind::Consume
                    && event.machine_symbol == machine
                    && event.state_symbol == state.state
                    && matches!(
                        event.claim_identity,
                        language_semantics::PermissionClaimIdentity::Established {
                            machine_symbol,
                            state_symbol,
                            source: language_semantics::PermissionEventSource::Statement {
                                statement_index,
                            },
                            ..
                        } if machine_symbol == machine
                            && state_symbol == state.state
                            && statement_index == result.statement_index as usize
                    )
            })
            .count()
            == 1
}

/// The rejection for a store the walk attributed to another statement than
/// the one its plan names.
fn reordered_store(
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
) -> Option<&'static str> {
    if operation.store_statement_index()? as usize == ordinal {
        return None;
    }
    Some(match operation {
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(_) => {
            "Unit graph reordered a byte-view write"
        }
        CheckedUnitEffectOperationPlan::ElementViewWrite(_) => {
            "Unit graph reordered an element-view write"
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_) => {
            "Unit graph reordered an indexed byte store"
        }
        CheckedUnitEffectOperationPlan::ByteSequenceStore(_) => {
            "Unit graph reordered a byte-field store"
        }
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. } => {
            "Unit graph reordered a primitive store"
        }
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_) => {
            "Unit graph reordered a field store"
        }
        CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
        | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_) => {
            "Unit graph reordered a case field store"
        }
        _ => return None,
    })
}

/// Whether a call keeps a structural result bound at this statement.
/// Nested operands keep their own call ordinal; source custody checks that
/// occurrence instead of requiring the statement's outermost call.
fn kept_structural_call_at(call: &CheckedUnitCallPlan, ordinal: usize) -> bool {
    call.discard_result_on_return() == Some(false)
        && call.coordinate.statement_index as usize == ordinal
        && call
            .structural_result()
            .is_some_and(|result| result.statement_index as usize == ordinal)
}

/// A boundary call consumes its boundary at its own statement; an ordinary
/// call has no boundary to consume.
fn validate_boundary_call_consumption(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    call: &CheckedUnitCallPlan,
) -> Result<(), LoweringError> {
    if !call.is_boundary() {
        return Ok(());
    }
    super::claims::validate_boundary_consumption(
        checked,
        machine,
        source,
        state,
        call.coordinate,
        call.target.state,
        &call.structural_arguments,
        call.claim_transfers(),
    )
}

fn returned_scalar_binding(
    state: &CheckedComposedUnitControlStatePlan,
) -> Option<&checked_trees::CheckedUnitScalarResultBindingPlan> {
    match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
            completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
        } => Some(binding),
        _ => None,
    }
}

/// Rejoin a returned final expression with the value checking recorded for
/// it under the `Return` role: the pure expression bound to exactly this
/// statement, or the unique computation root this machine owns there.
fn validate_returned_value(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    result: &checked_trees::CheckedUnitScalarResultBindingPlan,
    value: &checked_trees::CheckedCallScalarArgument,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<(), LoweringError> {
    let role = CheckedScalarExpressionRole::Return;
    match value {
        checked_trees::CheckedCallScalarArgument::Pure(value) => {
            let (binding, retained) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state.state, result.statement_index, role)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph scalar return has no source binding",
                ))?;
            if retained != value || binding.expression != expression {
                return unsupported("Unit graph scalar return value changed");
            }
            Ok(())
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
                "Unit graph scalar return has no computation root",
            ))?;
            if roots.next().is_some() || root.machine != machine || root.root != *handle {
                return unsupported("Unit graph scalar return computation changed");
            }
            Ok(())
        }
    }
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
        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => Some(store.statement_index),
        CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write.statement_index),
        CheckedUnitEffectOperationPlan::ElementViewWrite(write) => Some(write.statement_index),
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index, ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::AtomicAccess(access) => Some(access.statement_index),
        _ => match operation.call() {
            Some(call) => Some(call.coordinate.statement_index),
            None => continued_statement(operation),
        },
    }
}

/// The statement an operation may continue rather than consume: a call's
/// continuation cleanup, a store of its own statement's call result,
/// the member stores of a whole-record replacement, the element stores of a
/// whole array-literal replacement, and the move-out/store pair of a
/// displaced borrowed field.
fn continued_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
            if store.value.completed_call_binding_ordinal().is_some() =>
        {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
            ..
        } => Some(coordinate.statement_index),
        // A view field's descriptor store is one member store of a record
        // literal's roster, in whichever authored position its member takes.
        CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
            if matches!(
                store.destination,
                checked_trees::CheckedByteSequenceStoreDestination::ViewField { .. }
            ) =>
        {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index, ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store)
            if matches!(
                store.value,
                checked_trees::CheckedStoreValue::ScalarResult { .. }
            ) =>
        {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)
            if matches!(
                write.value,
                checked_trees::CheckedStoreValue::ScalarResult { .. }
            ) =>
        {
            Some(write.statement_index)
        }
        CheckedUnitEffectOperationPlan::ElementViewWrite(write)
            if matches!(
                write.value,
                checked_trees::CheckedStoreValue::ScalarResult { .. }
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
        // A call-result loan's release rejoins the statement where the loan's
        // last use already ran: the loan is custody, not a fresh statement.
        CheckedUnitEffectOperationPlan::ReleaseReference {
            statement_index, ..
        } => Some(*statement_index),
        _ => None,
    }
}

/// Which operations continue the authored statement the most recent
/// statement-consuming operation began. One authored statement may plan
/// several effects, and one rule covers all of them: an operation whose
/// `continued_statement` is that current statement rejoins it; every other
/// operation consumes the next authored statement. Only a plain field or
/// primitive store can also begin a statement (the first member of a record
/// replacement, the first element of an array-literal replacement);
/// every other continuing kind exists only after its producer, so one that
/// continues nothing lost that producer.
fn statement_continuations(
    operations: &[CheckedUnitEffectOperationPlan],
) -> Result<Vec<bool>, LoweringError> {
    let mut current = None;
    // Call operands execute before their consumer, whether constructed inline
    // or returned by a nested call. All retain their exact authored statement
    // and call coordinates; operand custody separately verifies their ordering.
    let mut began_with_argument = false;
    // Whether the current statement began with a structural establishment of
    // its own value: a whole array-literal replacement establishes every
    // element before its first window pair, so each later element continues
    // the statement the first began. The statement's replacement roster then
    // rejoins the complete element order.
    let mut began_with_establishment = false;
    operations
        .iter()
        .map(|operation| {
            if matches!(
                operation,
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. },
                    ..
                }
            ) {
                // Stage 04 schedules expiry before this entry. Keeping the
                // current statement unchanged makes cleanup transparent to coverage.
                return Ok(true);
            }
            let continued = continued_statement(operation);
            if continued.is_some() && continued == current {
                return Ok(true);
            }
            if began_with_establishment
                && authored_statement(operation) == current
                && own_establishment(operation)
            {
                return Ok(true);
            }
            // A reference release is emitted at the head of its expiry
            // boundary's statement, ahead of that statement's own
            // operations, so it rejoins the last statement consumed — marker
            // statements in between leave a gap the stamped boundary keeps.
            if matches!(
                operation,
                CheckedUnitEffectOperationPlan::ReleaseReference { .. }
            ) && current
                .is_some_and(|consumed| continued.is_some_and(|boundary| boundary > consumed))
            {
                return Ok(true);
            }
            if began_with_argument
                && authored_statement(operation) == current
                && (argument_construction(operation) || operation.call().is_some())
            {
                return Ok(true);
            }
            let may_begin = continued.is_none()
                || matches!(operation,
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)
                    if !matches!(
                        store.value,
                        checked_trees::CheckedStoreValue::ScalarResult { .. }
                    ))
                || matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                )
                || matches!(operation,
                CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
                    if matches!(
                        store.destination,
                        checked_trees::CheckedByteSequenceStoreDestination::ViewField { .. }
                    ) && store.value.completed_call_binding_ordinal().is_none())
                // A whole-element store into a displaced element is only the
                // move-out/store window pair: whichever of them leads is the
                // statement's first effect, not a continuation.
                || matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                        | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                );
            if !may_begin {
                return unsupported("Unit graph continuation lost its producing statement");
            }
            current = authored_statement(operation);
            began_with_argument = argument_construction(operation)
                || operation
                    .call()
                    .is_some_and(|call| call.coordinate.call_ordinal != 0);
            began_with_establishment = own_establishment(operation);
            Ok(false)
        })
        .collect()
}

/// A structural establishment of its statement's own value rather than of
/// one of the statement's call arguments.
fn own_establishment(operation: &CheckedUnitEffectOperationPlan) -> bool {
    matches!(
        operation,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            operand_source: None,
            ..
        }
    )
}

/// A structural construction authored as one of its statement's call
/// arguments rather than bound to a local or returned.
fn argument_construction(operation: &CheckedUnitEffectOperationPlan) -> bool {
    super::super::super::argument_schedule::argument_construction(operation).is_some()
}

/// The descriptor stores one record literal assignment planned for its shared
/// view members, in plan order.
fn record_view_members(
    state: &CheckedComposedUnitControlStatePlan,
    statement_index: u32,
) -> Vec<&checked_trees::CheckedByteSequenceStorePlan> {
    state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store)
                if store.statement_index == statement_index
                    && matches!(
                        store.destination,
                        checked_trees::CheckedByteSequenceStoreDestination::ViewField { .. }
                    ) =>
            {
                Some(store)
            }
            _ => None,
        })
        .collect()
}
