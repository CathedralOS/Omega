//! Exact source bindings and final custody of structural call results.
//! Checking selects current named homes independently of immutable producer
//! tokens. Consume that decision instead of replaying local initializers;
//! actual result tokens still require production order and unique consumption.

mod inventory;
mod producers;
pub(crate) use inventory::with_expression_consumers as result_use_operations;

use super::super::admission::CallerView;
use super::super::parameters::expression_producer;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, ExpressionNode,
    LoweringError, Multiplicity, StatementNode, unsupported,
};
use checked_trees::{
    CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitStructuralResultBindingPlan,
};
use producers::{
    loaned_record_projection, producer, projected_moves_cover_result, shared_byte_view_referent,
    shared_view_binding, validate_shared_temporary_continuation, whole_local_array_loan,
    whole_local_bounded_owner_loan,
};

/// An evaluated slice operand establishes a descriptor, not an owned referent.
/// Its access stays on the authored reference shell while its storage identity
/// names the slice. Other constructions retain the ordinary owned-value
/// contract; this does not admit arbitrary reference temporaries.
fn operand_contract(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Result<
    (
        validation::StructuralResultContract,
        Option<checked_trees::CheckedStructuralAccess>,
    ),
    LoweringError,
> {
    if let Some(view) = validation::slice_view_shell(&checked.typed, reference, &[]) {
        let access = match view.access {
            language_core::ReferenceAccess::Shared => {
                checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            language_core::ReferenceAccess::Mutable => {
                checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            language_core::ReferenceAccess::WriteOnly => {
                return unsupported("slice operand has unsupported write-only access");
            }
        };
        return Ok((
            validation::StructuralResultContract {
                carrier: view.slice,
                qualifications: validation::view_result_carrier_qualifications(
                    &checked.typed,
                    reference,
                )
                .map_err(LoweringError::Unsupported)?,
            },
            Some(access),
        ));
    }
    validation::structural_result_contract(&checked.typed, reference)
        .map(|contract| (contract, None))
        .map_err(LoweringError::Unsupported)
}

/// The ordinary call with a Unit result an operation performs, if any.
fn ordinary_unit_call(operation: &CheckedUnitEffectOperationPlan) -> Option<&CheckedUnitCallPlan> {
    operation
        .call()
        .filter(|call| call.is_module() && *call.result() == CheckedUnitCallResult::Unit)
}

/// Whether a call's result can retain no loan of its operands:
/// nothing, a scalar, or a structural result that mints no reference loan
/// or returned claim and whose callee declares borrow-free contents. Nominal
/// cleanup and represented predicates travel with the owned result without
/// retaining a loan. The declared return type is read from the callee itself
/// rather than from the checked plan's result shape, so a loan lent into such
/// a call provably ends with it.
fn loan_free_call(checked: &CheckedTrees, operation: &CheckedUnitEffectOperationPlan) -> bool {
    let Some(call) = operation.call() else {
        return false;
    };
    let binding = match call.result() {
        CheckedUnitCallResult::Unit | CheckedUnitCallResult::Scalar(_) => return true,
        CheckedUnitCallResult::Structural { binding, .. } => binding,
    };
    // A Unit/scalar boundary result cannot retain a loan any more than an
    // ordinary result can. Structural foreign retention still requires its
    // dedicated custody protocol, not the ordinary callee inspection below.
    if call.is_boundary() {
        return false;
    }
    call.structural_custody().is_some_and(|custody| {
        custody.reference_loans.is_empty() && custody.result_claims.is_empty()
    }) && crate::declarations::state_declaration(checked, call.target.state).is_ok_and(
        |(_, callee_state)| {
            let returned = callee_state.return_type;
            checked.typed.normalized_type_identity(returned).as_str() == binding.type_identity
                // Whether producing an owner prolongs an operand loan is a
                // contents question, not permission to discard or copy it.
                // CleanupOwned rejects represented byte predicates even though
                // those predicates contain no reference. The shared BorrowFree
                // judgment retains qualifications without inventing membership.
                && validation::has_contents(
                    &checked.typed,
                    returned,
                    validation::ContentsRequirement::BorrowFree,
                )
        },
    )
}

/// The structural result binding of a call, ordinary or boundary.
fn call_structural_result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<&CheckedUnitStructuralResultBindingPlan> {
    operation.call()?.structural_result()
}

/// Rejoin a construction authored as a call argument to the call that reads
/// it: `f(Event::Insert { cents: 50 })` establishes the value at the call's
/// statement and supplies the formal it was written for. The
/// construction's own source custody — the authored argument expression at
/// that formal position — is checked where the construction rejoins its
/// statement. This ties the two operations together, so the value cannot move
/// into another formal, another call, a projected parameter, or two consumers.
/// Sharing keeps the completed owner through its exact normal continuation;
/// the borrowed formal's unrestricted multiplicity is not the owner's.
fn validate_argument_construction_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    argument_index: usize,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    source_position: u32,
) -> Result<(), LoweringError> {
    let (machine, state, operations) = (caller.machine, caller.state, caller.operations);
    let Some(call) = operation.call() else {
        return unsupported("argument construction requires an ordinary or boundary call");
    };
    let coordinate = call.coordinate;
    let argument =
        call.structural_arguments
            .get(argument_index)
            .ok_or(LoweringError::Unsupported(
                "argument construction has no argument slot",
            ))?;
    let binding_ordinal =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "argument construction has no binding ordinal",
            ))?;
    let mut consumers = operations
        .iter()
        .enumerate()
        .filter(|(_, candidate)| *candidate == operation);
    let (consumer_index, _) = consumers.next().ok_or(LoweringError::Unsupported(
        "argument construction consumer is absent",
    ))?;
    if consumers.next().is_some() {
        return unsupported("argument construction consumer is ambiguous");
    }
    let source = producer(operations, binding_ordinal)?;
    let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
        parameter_position,
        ..
    }) = source.construction_source
    else {
        return unsupported("argument operand is not its call's own construction");
    };
    // Every established operand carries its declared value contract. The
    // producer establishes those predicates; this consuming edge only joins
    // the exact retained root and formal roster, independent of evaluator kind.
    let declared_qualifications = match &operations[source.operation_index] {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. }
            if checked
                .facts
                .values
                .structural_values
                .nodes
                .is_valid(*value) =>
        {
            let values = &checked.facts.values.structural_values;
            let expression = values.nodes.get(*value).expression;
            values
                .root_for_expression(state, source.result.statement_index, expression)
                .filter(|root| root.machine == machine && root.root == *value)
                .and_then(|root| {
                    operand_contract(checked, root.type_reference)
                        .ok()
                        .map(|(contract, _)| contract)
                })
                .is_some_and(|contract| contract.qualifications == parameter.qualifications)
        }
        _ => parameter.qualifications.is_empty(),
    };
    if source.operation_index >= consumer_index
        || !source.precedes_consumer(coordinate)
        || source.discard
        || parameter_position != source_position
        || !argument.path.is_empty()
        || !matches!(
            argument.access,
            checked_trees::CheckedStructuralAccess::Owned
                | checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::MutableBorrow
        )
        || parameter.access != argument.access
        || parameter.fused_service_erasure.is_some()
        || !declared_qualifications
        || parameter.multiplicity
            != if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                Multiplicity::Unrestricted
            } else {
                source.result.multiplicity
            }
        || argument.type_identity != source.result.type_identity
        || parameter.type_identity != source.result.type_identity
    {
        return unsupported("argument construction moves into another formal or custody");
    }
    if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
        validate_shared_temporary_continuation(
            checked,
            operations,
            consumer_index,
            &source,
            argument,
        )?;
    }
    let consumer_uses = operations
        .iter()
        .flat_map(CheckedUnitEffectOperationPlan::call_structural_arguments)
        .filter(|candidate| {
            candidate.source_structural_result_binding_ordinal() == Some(binding_ordinal)
        })
        .count();
    if consumer_uses != 1 {
        return unsupported("argument construction is read by more than its own call");
    }
    Ok(())
}

pub(crate) fn validate_usage(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operations: &[CheckedUnitEffectOperationPlan],
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let producer = producer(operations, result.binding_ordinal)?;
    if producer.result != result {
        return unsupported("Unit structural result binding disagrees with its producer");
    }
    let mut consumed = false;
    let mut disposed = false;
    let mut projected_paths = Vec::<&[checked_trees::CheckedUnitStructuralPathSegment]>::new();
    // A `let` establishes its local's value as this result. Until the local
    // is rebound, a store or rebind naming the local moves that same value.
    let mut result_local = {
        let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
        match checked
            .statement_table
            .statements(state.statement_nodes)
            .get(result.statement_index as usize)
        {
            Some(StatementNode::LocalData(local)) => Some(
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
            ),
            _ => None,
        }
    };
    // Every projected move under a still return-dropped carrier must carry
    // its own proven leaf loan; a single unloaned projection revives the
    // ordinary residual-continuation requirement.
    let mut projected_residual_loaned = true;
    for (operation_index, operation) in
        operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .with_value_calls()
                    .map(move |operation| (index, operation))
            })
    {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            path,
            defer_to_exit: false,
            ..
        } = operation
            && *binding_ordinal == result.binding_ordinal
        {
            // Checking has selected this exact lifetime boundary. A leaf
            // release removes only its custody, leaving scalar siblings and
            // the owner's scheduled residual cleanup available.
            if path.is_empty() {
                disposed = true;
            } else {
                projected_paths.push(path);
            }
            continue;
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: selected, ..
        } = operation
            && let Some((_, receipt)) = checked
                .facts
                .flow
                .ownership
                .owned_selection_at(caller.state, selected.statement_index)
        {
            // A selection carries both the chosen source and its complement.
            // This replaces each candidate's old unconditional death, without
            // claiming that every arm moves that candidate into the result.
            let sources = checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "selected result has stale source custody",
                ))?;
            // Only statement-established sources name a result binding:
            // parameter sources carry their authored position in
            // `statement_ordinal`, which is not a statement index.
            let result_local_symbol = checked
                .statement_table
                .statements(
                    crate::declarations::state_declaration(checked, caller.state)?
                        .1
                        .statement_nodes,
                )
                .get(result.statement_index as usize)
                .and_then(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.symbol),
                    _ => None,
                });
            if let Some(source_ordinal) = sources.iter().position(|source| {
                (matches!(
                    source.provenance,
                    language_semantics::PermissionProvenance::Established {
                        source: language_semantics::PermissionEventSource::Statement {
                            statement_index,
                        },
                        ..
                    } if statement_index == result.statement_index as usize
                ) || (source.origin_selection.is_valid()
                    && source.statement_ordinal as usize == result.statement_index as usize))
                    && Some(source.symbol) == result_local_symbol
            }) {
                let source_handle = arena::Handle::from_parts(
                    receipt.sources.start().arena_index() + source_ordinal as u32,
                    receipt.sources.start().generation(),
                );
                // A whole-local move carries the source's own type into the
                // result. A source moved only through projected children keeps
                // its root type here; the projected leaf identity was replayed
                // against the recorded transfer path by source custody above.
                let whole = checked
                    .facts
                    .flow
                    .ownership
                    .selection_transfers
                    .span(receipt.transfers)
                    .is_some_and(|transfers| {
                        transfers.iter().any(|transfer| {
                            transfer.source == source_handle
                                && checked
                                    .facts
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(transfer.path)
                                    .is_empty()
                        })
                    });
                if consumed
                    || disposed
                    || !projected_paths.is_empty()
                    || operation_index <= producer.operation_index
                    || selected.statement_index <= result.statement_index
                    || result.multiplicity != Multiplicity::Affine
                    || (whole && selected.type_identity != result.type_identity)
                {
                    return unsupported("selected result reuses an unavailable structural source");
                }
                consumed = true;
            }
        }
        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            value,
            result: destination,
            ..
        } = operation
            && checked
                .facts
                .flow
                .ownership
                .owned_selection_at(caller.state, destination.statement_index)
                .is_none()
        {
            let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
            if let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(result.statement_index as usize)
            {
                let values = &checked.facts.values.structural_values;
                let mut pending = vec![*value];
                let mut visited = Vec::new();
                while let Some(value) = pending.pop() {
                    if !values.nodes.is_valid(value) || visited.contains(&value) {
                        return unsupported(
                            "record consumption has stale or repeated value custody",
                        );
                    }
                    visited.push(value);
                    let node = values.nodes.get(value);
                    match &node.kind {
                        checked_trees::CheckedStructuralValueKind::Place(argument)
                            if argument.source == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local.symbol }) => {
                            if !matches!(checked.expression_table.expression(node.expression), ExpressionNode::Name(name) if name.symbol == local.symbol && name.head_symbol == local.symbol && name.members.count() == 1)
                                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                                || !argument.path.is_empty() || argument.type_identity != result.type_identity
                                || consumed || disposed || !projected_paths.is_empty()
                                || operation_index <= producer.operation_index
                                || destination.statement_index <= result.statement_index {
                                return unsupported("record child lost its exact prior owned local");
                            }
                            consumed = result.multiplicity != Multiplicity::Unrestricted;
                        }
                        checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                        | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                            for field in values.record_fields.span(*fields).ok_or(LoweringError::Unsupported("record consumption field roster missing"))? {
                                if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) = field.value { pending.push(value); }
                            }
                        }
                        checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                            pending.extend(elements.iter().rev().copied());
                        }
                        checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => pending.extend(values.dispatch_arms.span(*arms).ok_or(LoweringError::Unsupported("record consumption arm roster missing"))?.iter().map(|arm| arm.value)),
                        _ => {}
                    }
                }
            }
        }
        if let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary,
            cleanup_actions,
        } = operation
        {
            if matches!(
                boundary,
                checked_trees::CheckedUnitCleanupBoundary::StatementEntry { .. }
                    | checked_trees::CheckedUnitCleanupBoundary::Replacement { .. }
            ) {
                let source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    };
                if !cleanup_actions
                    .iter()
                    .any(|discard| *discard.source() == source)
                {
                    continue;
                }
                disposed = true;
                continue;
            }
            let checked_trees::CheckedUnitCleanupBoundary::Call(coordinate) = boundary else {
                return unsupported("cleanup has no supported semantic boundary");
            };
            let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            };
            let immediately_discarded = operation_index.checked_sub(1)
                == Some(producer.operation_index)
                // A returning consumer can retire its argument temporaries
                // without retiring the distinct value it just returned.
                && cleanup_actions.iter().any(|discard| *discard.source() == source)
                && operations[producer.operation_index]
                    .call()
                    .is_some_and(|call| {
                        call.structural_result().is_some()
                            && call.coordinate == *coordinate
                            && call.coordinate.call_ordinal == 0
                    });
            if immediately_discarded {
                if consumed
                    || disposed
                    || producer.discard
                    || !projected_paths.is_empty()
                    || !matches!(cleanup_actions.as_slice(), [discard]
                        if *discard.source() == source && discard.path().is_empty()
                            && discard.type_identity() == result.type_identity)
                {
                    return unsupported(
                        "discarded boundary result lost its immediate whole cleanup",
                    );
                }
                disposed = true;
                continue;
            }
            // Empty complements still end the temporary's lifetime. Its exact
            // consumer identifies that owner even when no residual row exists.
            let owns_continuation = operation_index
                .checked_sub(1)
                .and_then(|previous| operations.get(previous))
                .and_then(CheckedUnitEffectOperationPlan::call)
                .filter(|call| call.is_module() && call.coordinate == *coordinate)
                .is_some_and(|call| {
                    call.structural_arguments
                        .iter()
                        .any(|argument| argument.source == source)
                });
            if !owns_continuation {
                continue;
            }
            if consumed
                || disposed
                || producer.discard
                || !producer.is_argument_temporary()
                || producer.coordinate.statement_index != coordinate.statement_index
                || operation_index <= producer.operation_index
            {
                return unsupported("call continuation does not own this result remainder");
            }
            disposed = true;
            continue;
        }
        // A field replacement's store moves the whole produced value into the
        // hole its move-out opened: the value's one consuming use. A copy
        // value is copied there and keeps owing nothing.
        if let CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. }
        | CheckedUnitEffectOperationPlan::RebindStructuralLocal { value, .. } = operation
            && (value.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                || (operation_index > producer.operation_index
                    && result_local.as_ref() == Some(&value.source)))
        {
            if consumed
                || disposed
                || operation_index <= producer.operation_index
                || !value.path.is_empty()
                || value.access != checked_trees::CheckedStructuralAccess::Owned
                || value.type_identity != result.type_identity
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            consumed = result.multiplicity != Multiplicity::Unrestricted;
            continue;
        }
        if let CheckedUnitEffectOperationPlan::RebindStructuralLocal { destination, .. } = operation
            && result_local.as_ref() == Some(&destination.source)
            && operation_index > producer.operation_index
        {
            result_local = None;
        }
        let Some(call) = operation.call() else {
            continue;
        };
        let coordinate = call.coordinate;
        for argument in &call.structural_arguments {
            if argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal) {
                continue;
            }
            let source_order = producer.precedes_consumer(coordinate);
            if consumed || disposed || operation_index <= producer.operation_index || !source_order
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            // Either call family, ordinary or boundary, may borrow the result.
            if matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                    | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            ) && !producer.is_argument_temporary()
            {
                if projected_paths.iter().any(|earlier| {
                    earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                }) {
                    return unsupported("local receiver borrows an already moved field");
                }
                // Exact declaration/path/access and the captured loan are
                // independently rejoined by validate_consumer. A loan keeps
                // this whole result's original ownership and cleanup debt.
                continue;
            }
            if result.multiplicity == Multiplicity::Unrestricted {
                // Copies retain their source after either call family. Exact
                // boundary type eligibility is rejoined by validate_consumer;
                // this walk checks the produced value's use and lifetime.
                if !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                ) || !argument.path.is_empty()
                    || argument.type_identity != result.type_identity
                {
                    return unsupported(
                        "unrestricted result requires a whole owned or shared call operand",
                    );
                }
                if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    validate_shared_temporary_continuation(
                        checked,
                        operations,
                        operation_index,
                        &producer,
                        argument,
                    )?;
                }
                continue;
            }
            if !argument.path.is_empty() {
                let loaned = loaned_record_projection(operation, argument);
                if !(ordinary_unit_call(operation).is_some() || loaned)
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || result.multiplicity != Multiplicity::Affine
                    || projected_paths.iter().any(|earlier| {
                        earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                    })
                {
                    return unsupported(
                        "Unit result projection overlaps or lacks owned call custody",
                    );
                }
                projected_paths.push(&argument.path);
                projected_residual_loaned &= loaned;
                continue;
            }
            // A linear result moves whole like an affine one: the consuming edge
            // carries the claim and checking already proved who retires it, so
            // only the multiplicity differs.
            if !projected_paths.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.type_identity != result.type_identity
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Linear
                )
            {
                return unsupported(
                    "Unit structural result use is not a whole owned move or shared borrow",
                );
            }
            if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && producer.is_argument_temporary()
            {
                validate_shared_temporary_continuation(
                    checked,
                    operations,
                    operation_index,
                    &producer,
                    argument,
                )?;
            }
            consumed = argument.access == checked_trees::CheckedStructuralAccess::Owned;
        }
    }
    let mut returned_carrier = false;
    if let Some(returned) = &caller.structural_result
        && returned.source
            == (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            })
    {
        if consumed
            || disposed
            || !projected_paths.is_empty()
            || returned.type_identity != result.type_identity
            || returned.multiplicity != result.multiplicity
        {
            return unsupported("structural completion reuses or changes its owned result");
        }
        returned_carrier = true;
        consumed = result.multiplicity != Multiplicity::Unrestricted;
    }
    // Unrestricted values carry no disposal debt. Argument transport is still
    // checked above; completing with a result is rejoined to source separately.
    if result.multiplicity == Multiplicity::Unrestricted {
        return if producer.discard || consumed || disposed || !projected_paths.is_empty() {
            unsupported("unrestricted result acquired affine transfer or disposal custody")
        } else {
            Ok(())
        };
    }
    // A reference result transfers its live carrier through any owned move,
    // including completion, or retires the producing call's exact loan.
    // The receiving carrier owns the transferred loan; checking follows the
    // retained value graph to prevent release by its former holder. Treating
    // only completion as transfer would dispose a moved constructor child.
    // Retirement is an explicit ReleaseReference, or state exit when the last
    // use is the final statement.
    // Edge retirement still owns the carrier place and requires its discard;
    // an explicit release or whole transfer must not also discard it.
    if let Some(custody) = operations[producer.operation_index]
        .call()
        .and_then(|call| call.structural_custody())
        && !custody.reference_loans.is_empty()
    {
        if returned_carrier || consumed {
            // A whole move forwards the exact carrier and its outstanding loan;
            // it does not dispose the referent or retire the source loan.
            if producer.discard || disposed || !projected_paths.is_empty() {
                return unsupported("transferred reference also acquired disposal custody");
            }
            return Ok(());
        }
        if !projected_paths.is_empty() || (disposed && producer.discard) {
            return unsupported("reference result acquired owned cleanup custody");
        }
        if !disposed {
            // Edge retirement: without a release operation the loan ends with
            // the carrier place's own return discard, which the checker must
            // therefore have kept on the result.
            if !producer.discard {
                return unsupported("reference result omits its carrier disposal");
            }
        }
        return Ok(());
    }

    // Affine custody belongs to the result carrier, not necessarily to its
    // backing storage. A mutable view descriptor retires under these same
    // exact transfer/disposal rules; its source loan is checked separately.
    // Shared descriptors already followed the unrestricted path above. A
    // view-shaped type alone can neither waive nor forbid carrier cleanup.
    if (projected_paths.is_empty() && producer.discard == (consumed || disposed))
        || (!projected_paths.is_empty() && producer.discard && !projected_residual_loaned)
        || (producer.coordinate.call_ordinal != 0
            && !consumed
            && projected_paths.is_empty()
            && !disposed)
        || (producer.coordinate.call_ordinal != 0
            && !projected_paths.is_empty()
            && !disposed
            // A loaned projection may carry the whole carrier: when every
            // structural child is moved out, no residual remains for a
            // continuation to own, and the terminal frontier retires the
            // empty shell at the consuming call.
            && !(projected_residual_loaned
                && projected_moves_cover_result(
                    &checked.facts.flow.terminal_unit_effects.structural_types,
                    &result.type_identity,
                    &projected_paths,
                ))
            && !matches!(
                operations,
                [_, _, CheckedUnitEffectOperationPlan::Complete { .. }]
            ))
    {
        return unsupported(
            "Unit structural result cleanup disagrees with its final consuming use",
        );
    }
    Ok(())
}

pub(crate) fn validate_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    target_entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let Some(call) = operation.call() else {
        return unsupported("structural result use requires an ordinary or boundary call");
    };
    // Retained loans are admitted by the call's recorded custody, as other
    // structural calls are, not by recognizing the caller's storage
    // spelling. The loan's release is checked by validate_usage.
    let loan_is_accounted = loan_free_call(checked, operation)
        || (call.is_module()
            && call
                .structural_custody()
                .is_some_and(|custody| !custody.reference_loans.is_empty()));
    // A structural call keeps its transfers in its custody record; a boundary
    // call's are its completion receipts.
    let (coordinate, structural_arguments, claim_transfers) = (
        call.coordinate,
        &call.structural_arguments,
        call.claim_transfers(),
    );
    if structural_arguments.len() != target_parameters.len() {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let operation_index = caller
        .operations
        .iter()
        .position(|candidate| candidate == operation)
        .ok_or(LoweringError::Unsupported(
            "structural result consumer has no operation position",
        ))?;
    let authored = crate::call_sites::locate(checked, caller.state, coordinate)?;
    super::super::argument_schedule::build_operations(
        checked,
        caller.machine,
        caller.state,
        caller.operations,
    )?;
    let (_, state) = crate::declarations::state_declaration(checked, caller.state)?;
    let signature =
        crate::declarations::target_signature(checked, caller.machine, authored.source_target)?;
    super::super::call_parameters::structural_parameters(checked, call)?;
    let source_positions =
        super::super::call_parameters::structural_positions(checked, call, &signature)?;
    // Claims use dense runtime slots; constructions retain authored positions.
    // The declaration projection joins them without manufacturing a receiver slot.
    for (index, ((argument, parameter), source_position)) in structural_arguments
        .iter()
        .zip(target_parameters)
        .zip(source_positions)
        .enumerate()
    {
        let parameter_index = index;
        let boundary_array_argument = authored.boundary
            && signature
                .parameters
                .get(source_position as usize)
                .is_some_and(|source| {
                    crate::content_classes::closed_primitive_array(checked, source.type_reference)
                });
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == source_position).then_some(*expression)
            });
        let binding_ordinal = argument.source_structural_result_binding_ordinal();
        if let Some(expression) = expression
            && super::super::reference_results::validate_consumer(
                checked, caller, coordinate, argument, parameter, expression,
            )?
        {
            // A non-retaining boundary borrow consumes the same live loan as
            // an ordinary call. Its concrete pointee ABI is checked during
            // realization; this consumer still cannot donate claim custody.
            if target_entry_claims
                .iter()
                .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported(
                    "reference result consumer acquired unrelated boundary or claim custody",
                );
            }
            continue;
        }
        if let Some(source_index) = argument.source_parameter_index() {
            let source = caller
                .structural_parameters
                .get(source_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural argument source parameter is absent",
                ))?;
            let source_parameter = checked
                .state_parameters(state)
                .get(source.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural argument source position is absent",
                ))?;
            // Whole owned array transport must rejoin the exact source and
            // target payload. A loan's endpoint type and access intentionally
            // differ from its enclosing array: the ordinary borrow and typed
            // projection checks own that custody instead. The authored-position
            // partition above identifies the caller's source in either case.
            if argument.access == checked_trees::CheckedStructuralAccess::Owned
                && crate::content_classes::closed_primitive_array(checked, source_parameter.type_reference)
                && (!argument.path.is_empty()
                    || source.access != argument.access
                    || parameter.access != argument.access
                    || source.multiplicity != Multiplicity::Unrestricted
                    || parameter.multiplicity != Multiplicity::Unrestricted
                    || source.type_identity != argument.type_identity
                    || parameter.type_identity != argument.type_identity
                    || !source.qualifications.is_empty()
                    || !parameter.qualifications.is_empty()
                    || source.fused_service_erasure.is_some()
                    || parameter.fused_service_erasure.is_some()
                    || !expression.is_some_and(|expression| matches!(
                        checked.expression_table.expression(expression), ExpressionNode::Name(name)
                            if name.symbol == source_parameter.symbol
                                && name.head_symbol == source_parameter.symbol
                                && checked.expression_table.name_path_members(name.members).len() == 1))
                    || target_entry_claims.iter().any(|claim| claim.parameter_index as usize == parameter_index)
                    || claim_transfers.iter().any(|transfer| transfer.argument_index as usize == index))
            {
                return unsupported("primitive array parameter requires exact whole owned transport");
            }
        }
        if binding_ordinal.is_none()
            && expression
                .is_some_and(|expression| expression_producer(checked, expression).is_some())
        {
            return unsupported(
                "nested structural argument has no expression-owned producer binding",
            );
        }
        // Source admission already bound this operand to its current storage
        // and exact access. Replaying every initializer here mistakes a
        // rebound home for its retired result and rejects ordinary sequencing.
        // Emission resolves the checked home; result usage separately retains
        // production order and one-time consumption for actual result tokens.
        let Some(binding_ordinal) = binding_ordinal else {
            continue;
        };
        let producer = producer(caller.operations, binding_ordinal)?;
        let result = producer.result;
        let source_order = producer.precedes_consumer(coordinate);
        let shared_view_argument = shared_view_binding(checked, caller.state, result)?
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow;
        // A whole linear result moved into this call transfers its producer's
        // live claim: the call's completion receipts publish that claim at
        // this argument rather than leaving the operand claim-free. Target
        // qualifications stay admissible — checking already joined the
        // bound local's domains to the formal's exact requirement.
        let linear_result_move = result.multiplicity == Multiplicity::Linear
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && argument.type_identity == result.type_identity
            && parameter.access == checked_trees::CheckedStructuralAccess::Owned
            && parameter.multiplicity == Multiplicity::Linear
            && parameter.type_identity == argument.type_identity
            && parameter.fused_service_erasure.is_none()
            && claim_transfers
                .iter()
                .any(|transfer| transfer.argument_index as usize == index);
        // A construction authored as this call's own argument moves whole
        // into the formal it was written for, and into no other consumer.
        let argument_construction = matches!(
            producer.construction_source,
            Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
        );
        if argument_construction {
            validate_argument_construction_consumer(
                checked,
                caller,
                operation,
                index,
                parameter,
                source_position,
            )?;
        }
        if (producer.discard
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
            && !loaned_record_projection(operation, argument))
            || producer.operation_index >= operation_index
            || !source_order
            || call_structural_result(operation)
                .is_some_and(|consumer| result.binding_ordinal >= consumer.binding_ordinal)
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
        // A slice view evaluated as this call's own argument lends its
        // backing for the call, at a boundary as at an ordinary call: the
        // descriptor crosses under the formal's exact access and slice type
        // and transfers no claim.
        if argument_construction
            && argument.path.is_empty()
            && result.multiplicity == Multiplicity::Unrestricted
            && matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            )
            && argument.access == parameter.access
            && argument.type_identity == result.type_identity
            && parameter.type_identity == argument.type_identity
            && parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.fused_service_erasure.is_none()
            && signature
                .parameters
                .get(source_position as usize)
                .is_some_and(|formal| {
                    validation::slice_view_shell(&checked.typed, formal.type_reference, &[])
                        .is_some()
                })
            && target_entry_claims
                .iter()
                .all(|claim| claim.parameter_index as usize != parameter_index)
            && claim_transfers
                .iter()
                .all(|transfer| transfer.argument_index as usize != index)
        {
            continue;
        }
        // Whole local arrays lend their backing without transferring ownership.
        // The source/ordinal join above identifies the declaration; this check
        // independently rejoins its loan and fixed-array/slice presentation.
        if whole_local_array_loan(
            checked,
            caller,
            result,
            argument,
            signature
                .parameters
                .get(source_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "local array loan has no authored formal",
                ))?
                .type_reference,
        )? {
            if argument.access != parameter.access
                || parameter.type_identity != argument.type_identity
                || parameter.multiplicity != Multiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported("local array loan changes the target custody");
            }
            continue;
        }
        // A whole bounded owner lends its own carrier or a shared live-byte
        // view. Boundary joins stay confined to shared byte formals with Unit
        // or scalar results; source capture and claim checks remain required.
        if whole_local_bounded_owner_loan(
            checked,
            caller,
            result,
            argument,
            parameter,
            signature
                .parameters
                .get(source_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "bounded owner loan has no authored formal",
                ))?
                .type_reference,
        )? {
            let boundary_shared_view = authored.boundary
                && parameter.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && signature
                    .parameters
                    .get(source_position as usize)
                    .is_some_and(|formal| {
                        shared_byte_view_referent(checked, formal.type_reference).is_some()
                    })
                && operation.call().is_some_and(|call| {
                    matches!(
                        call.result(),
                        CheckedUnitCallResult::Unit | CheckedUnitCallResult::Scalar(_)
                    )
                });
            if (authored.boundary && !boundary_shared_view)
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
                || (!boundary_shared_view && !loan_free_call(checked, operation))
            {
                return unsupported("bounded owner loan changes the target custody");
            }
            continue;
        }
        // Lending a whole local or a projected subtree does not consume its
        // owner. The authored path was rejoined above; a loan either ends at
        // this call or survives in independently reconstructed result custody.
        // Stored-reference crossings retain that same call custody. The path
        // replay preserves their descriptors; Terminal independently checks
        // each live permission instead of recognizing a returned-record form.
        let lent_access = match argument.access {
            checked_trees::CheckedStructuralAccess::MutableBorrow
            | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            | checked_trees::CheckedStructuralAccess::SharedBorrow => true,
            checked_trees::CheckedStructuralAccess::Owned => false,
        };
        let borrowed_storage = matches!(
            result.multiplicity,
            Multiplicity::Affine | Multiplicity::Unrestricted
        ) && loan_is_accounted
            && argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                        | checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::Referent
                )
            })
            && lent_access
            && argument.access == parameter.access
            && parameter.type_identity == argument.type_identity
            && parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.fused_service_erasure.is_none()
            && target_entry_claims
                .iter()
                .all(|claim| claim.parameter_index as usize != parameter_index)
            && claim_transfers
                .iter()
                .all(|transfer| transfer.argument_index as usize != index);
        if !linear_result_move
            && !borrowed_storage
            && (!matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) || (result.multiplicity == Multiplicity::Unrestricted
                && (call.is_boundary() && !boundary_array_argument && !shared_view_argument
                    || !argument.path.is_empty()
                    || !matches!(
                        argument.access,
                        checked_trees::CheckedStructuralAccess::Owned
                            | checked_trees::CheckedStructuralAccess::SharedBorrow
                    )))
                || (argument.path.is_empty() && argument.type_identity != result.type_identity)
                || parameter.type_identity != argument.type_identity
                || (!argument.path.is_empty()
                    && (!(ordinary_unit_call(operation).is_some()
                        || loaned_record_projection(operation, argument))
                        || argument.access != checked_trees::CheckedStructuralAccess::Owned))
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.access != parameter.access
                || parameter.multiplicity
                    != if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                        Multiplicity::Unrestricted
                    } else {
                        result.multiplicity
                    }
                // Qualification obligations belong to the emitted call's
                // semantic check, independently of this source custody join.
                // In particular, a subslice's primitive byte predicates are
                // derived from its exact producer without result annotations.
                || parameter.fused_service_erasure.is_some()
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == parameter_index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index))
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
    }
    Ok(())
}
