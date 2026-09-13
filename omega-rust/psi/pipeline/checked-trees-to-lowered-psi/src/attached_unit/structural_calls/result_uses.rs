//! Exact source bindings and final custody of structural call results.

use super::super::parameters::expression_producer;
use super::*;
use checked_trees::CheckedUnitStructuralResultBindingPlan;

/// Rejoin a whole named linear operand to its earlier successful call, not
/// merely another result with the same carrier type. Source claims retain the
/// input lineage across both calls; operation emission checks the same join in
/// the Terminal place/claim namespace.
pub(crate) fn validate_linear_result_consumer(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
    operation: &CheckedUnitEffectOperationPlan,
    argument_index: usize,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        structural_arguments,
        custody,
        result: consumer_result,
        ..
    } = operation
    else {
        return unsupported("linear result operand requires an ordinary structural call");
    };
    let argument = structural_arguments
        .get(argument_index)
        .ok_or(LoweringError::Unsupported(
            "linear result operand has no argument slot",
        ))?;
    let binding_ordinal =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "linear result operand has no binding ordinal",
            ))?;
    let mut consumers = operations
        .iter()
        .enumerate()
        .filter(|(_, candidate)| *candidate == operation);
    let (consumer_index, _) = consumers.next().ok_or(LoweringError::Unsupported(
        "linear result consumer is absent",
    ))?;
    if consumers.next().is_some() {
        return unsupported("linear result consumer is ambiguous");
    }
    let source = producer(operations, binding_ordinal)?;
    let producer_index = source.operation_index;
    let producer = &operations[producer_index];
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate: producer_coordinate,
        result,
        custody: produced,
        ..
    } = producer
    else {
        return unsupported("linear result operand has no ordinary producer");
    };
    if producer_index >= consumer_index
        || producer_coordinate.call_ordinal != 0
        || producer_coordinate.statement_index >= coordinate.statement_index
        || result.binding_ordinal >= consumer_result.binding_ordinal
        || source.discard
        || result.multiplicity != Multiplicity::Linear
        || parameter.multiplicity != Multiplicity::Linear
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || parameter.access != argument.access
        || parameter.is_self
        || parameter.fused_service_erasure.is_some()
        || result.type_identity != argument.type_identity
        || parameter.type_identity != result.type_identity
        || parameter.qualifications != produced.result_qualifications
    {
        return unsupported("linear result operand has stale or incompatible producer custody");
    }
    super::validate_custody(checked, machine, state, producer)?;
    super::validate_custody(checked, machine, state, operation)?;
    crate::call_source_custody::initializers::validate_structural(
        checked,
        machine,
        state,
        *producer_coordinate,
        result,
    )?;
    let (_, source_state) = crate::scalar_source_custody::authored_state(checked, state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("linear result operand has no authored local");
    };
    if local.is_mutable
        || validation::structural_result_qualifications(&checked.typed, local.type_reference)
            .map_err(LoweringError::Unsupported)?
            != produced.result_qualifications
    {
        return unsupported("linear result local changes its producing qualification row");
    }
    let authored =
        crate::call_source_custody::authored::locate_source(checked, state, *coordinate)?;
    let mut arguments = authored
        .structural_arguments
        .iter()
        .filter(|(position, _)| *position == parameter.position);
    let (_, expression) = arguments.next().ok_or(LoweringError::Unsupported(
        "linear result operand has no authored argument",
    ))?;
    if arguments.next().is_some()
        || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(name)
        if name.symbol == local.symbol && name.head_symbol == local.symbol
            && checked.expression_table.name_path_members(name.members).len() == 1)
    {
        return unsupported("linear result operand does not name its exact producing local");
    }
    let transfers = custody
        .claim_transfers
        .iter()
        .filter(|transfer| transfer.argument_index as usize == argument_index)
        .collect::<Vec<_>>();
    if transfers.len() != 1
        || produced.returned_claim_transfers.len() != 1
        || transfers[0].claim_identity != produced.returned_claim_transfers[0].caller_claim
    {
        return unsupported("linear result operand changes the returned input claim lineage");
    }
    for earlier in &operations[producer_index + 1..=consumer_index] {
        let arguments = match earlier {
            CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => continue,
        };
        if arguments
            .iter()
            .enumerate()
            .any(|(argument_position, candidate)| {
                candidate.source_structural_result_binding_ordinal() == Some(binding_ordinal)
                    && candidate.access == checked_trees::CheckedStructuralAccess::Owned
                    && (earlier != operation || argument_position != argument_index)
            })
        {
            return unsupported("linear result operand has already been consumed");
        }
    }
    Ok(())
}

struct Producer<'plan> {
    operation_index: usize,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &'plan CheckedUnitStructuralResultBindingPlan,
    discard: bool,
    construction_source: Option<checked_trees::CheckedArrayConstructionSource>,
}

impl Producer<'_> {
    fn precedes_consumer(&self, coordinate: checked_trees::CheckedUnitCallCoordinate) -> bool {
        if let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            ..
        }) = self.construction_source
        {
            return self.result.statement_index == coordinate.statement_index
                && call_ordinal == coordinate.call_ordinal;
        }
        if self.coordinate.call_ordinal == 0 {
            self.result.statement_index < coordinate.statement_index
        } else {
            self.result.statement_index == coordinate.statement_index
                && coordinate.call_ordinal < self.coordinate.call_ordinal
        }
    }
}

fn producer(
    operations: &[CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
) -> Result<Producer<'_>, LoweringError> {
    let mut matches = operations
        .iter()
        .enumerate()
        .filter_map(|(operation_index, operation)| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => Some(Producer {
                operation_index,
                coordinate: checked_trees::CheckedUnitCallCoordinate {
                    statement_index: result.statement_index,
                    call_ordinal: 0,
                },
                result,
                discard: *discard_result_on_return,
                construction_source: None,
            }),
            CheckedUnitEffectOperationPlan::EstablishScalarArray { source, result, .. }
                if result.binding_ordinal == binding_ordinal =>
            {
                Some(Producer {
                    operation_index,
                    coordinate: checked_trees::CheckedUnitCallCoordinate {
                        statement_index: result.statement_index,
                        call_ordinal: 0,
                    },
                    result,
                    discard: false,
                    construction_source: Some(*source),
                })
            }
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => Some(Producer {
                operation_index,
                coordinate: *coordinate,
                result,
                discard: *discard_result_on_return,
                construction_source: None,
            }),
            _ => None,
        });
    let result = matches.next().ok_or(LoweringError::Unsupported(
        "Unit structural result argument has no exact producer binding",
    ))?;
    if matches.next().is_some() {
        return unsupported("Unit structural result argument has ambiguous producer bindings");
    }
    Ok(result)
}

pub(crate) fn validate_usage(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let producer = producer(&caller.operations, result.binding_ordinal)?;
    if producer.result != result {
        return unsupported("Unit structural result binding disagrees with its producer");
    }
    let mut consumed = false;
    let mut disposed = false;
    let mut projected_paths = Vec::<&[checked_trees::CheckedUnitStructuralPathSegment]>::new();
    for (operation_index, operation) in
        caller
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .with_value_calls()
                    .map(move |operation| (index, operation))
            })
    {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal, ..
        } = operation
            && *binding_ordinal == result.binding_ordinal
        {
            super::super::reference_results::validate_releases(checked, caller)?;
            if consumed
                || disposed
                || operation_index <= producer.operation_index
                || producer.discard
            {
                return unsupported("reference carrier is released after losing custody");
            }
            disposed = true;
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
            super::super::structural_values::source_custody::validate(
                checked,
                caller.machine,
                caller.state,
                operation,
            )?;
            let sources = checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "selected result has stale source custody",
                ))?;
            if sources
                .iter()
                .any(|source| source.statement_ordinal == result.statement_index)
            {
                if consumed
                    || disposed
                    || !projected_paths.is_empty()
                    || operation_index <= producer.operation_index
                    || selected.statement_index <= result.statement_index
                    || result.multiplicity != Multiplicity::Affine
                    || selected.type_identity != result.type_identity
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
            let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
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
                        checked_trees::CheckedStructuralValueKind::Record { fields, .. } => {
                            for field in values.record_fields.span(*fields).ok_or(LoweringError::Unsupported("record consumption field roster missing"))? {
                                if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) = field.value { pending.push(value); }
                            }
                        }
                        checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => pending.extend(values.dispatch_arms.span(*arms).ok_or(LoweringError::Unsupported("record consumption arm roster missing"))?.iter().map(|arm| arm.value)),
                        _ => {}
                    }
                }
            }
        }
        if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        } = operation
        {
            let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            };
            let immediately_discarded = operation_index.checked_sub(1)
                == Some(producer.operation_index)
                && matches!(&caller.operations[producer.operation_index],
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate: call, .. }
                    | CheckedUnitEffectOperationPlan::StructuralCall { coordinate: call, .. }
                        if call == coordinate && call.call_ordinal == 0);
            if immediately_discarded {
                if consumed
                    || disposed
                    || producer.discard
                    || !projected_paths.is_empty()
                    || !matches!(affine_discards.as_slice(), [discard]
                        if discard.source == source && discard.path.is_empty()
                            && discard.type_identity == result.type_identity)
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
                .and_then(|previous| caller.operations.get(previous))
                .is_some_and(|operation| {
                    matches!(operation,
                    CheckedUnitEffectOperationPlan::CallUnit { structural_arguments, .. }
                        if structural_arguments.iter().any(|argument| argument.source == source))
                });
            if !owns_continuation {
                if affine_discards
                    .iter()
                    .any(|discard| discard.source == source)
                {
                    return unsupported("call continuation substituted its result owner");
                }
                continue;
            }
            if consumed
                || disposed
                || producer.discard
                || producer.coordinate.call_ordinal == 0
                || producer.coordinate.statement_index != coordinate.statement_index
                || operation_index <= producer.operation_index
                || if projected_paths.is_empty() {
                    !matches!(affine_discards.as_slice(), [discard]
                        if discard.source == source && discard.path.is_empty() && discard.type_identity == result.type_identity)
                } else {
                    affine_discards
                        .iter()
                        .any(|discard| discard.source != source || discard.path.is_empty())
                }
            {
                return unsupported("call continuation does not own this result remainder");
            }
            disposed = true;
            continue;
        }
        let (CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            structural_arguments,
            ..
        }) = operation
        else {
            continue;
        };
        for argument in structural_arguments {
            if argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal) {
                continue;
            }
            let source_order = producer.precedes_consumer(*coordinate);
            if consumed || disposed || operation_index <= producer.operation_index || !source_order
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            if matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            ) && matches!(
                operation,
                CheckedUnitEffectOperationPlan::CallUnit { .. }
                    | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::StructuralCall { .. }
            ) && producer.coordinate.call_ordinal == 0
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
                if !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::CallUnit { .. }
                        | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::StructuralCall { .. }
                ) || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.type_identity != result.type_identity
                {
                    return unsupported(
                        "unrestricted array result requires a whole owned ordinary argument",
                    );
                }
                continue;
            }
            if !argument.path.is_empty() {
                if !matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
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
                continue;
            }
            if !projected_paths.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.type_identity != result.type_identity
                || result.multiplicity != Multiplicity::Affine
            {
                return unsupported(
                    "Unit structural result use is not a whole affine move or shared borrow",
                );
            }
            if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && producer.coordinate.call_ordinal != 0
                && (producer.coordinate.call_ordinal != 1
                    || producer.discard
                    || coordinate.call_ordinal != 0
                    || !matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { scalar_arguments, structural_arguments, .. }
                        if scalar_arguments.is_empty() && structural_arguments.len() == 1)
                    || !matches!(caller.operations.get(operation_index + 1),
                        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate: cleanup, affine_discards })
                            if cleanup == coordinate && affine_discards.len() == 1
                                && affine_discards[0].source == argument.source))
            {
                return unsupported("anonymous shared result has no dying Unit call continuation");
            }
            consumed = argument.access == checked_trees::CheckedStructuralAccess::Owned;
        }
    }
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
    if (projected_paths.is_empty() && producer.discard == (consumed || disposed))
        || (!projected_paths.is_empty() && producer.discard)
        || (producer.coordinate.call_ordinal != 0
            && !consumed
            && projected_paths.is_empty()
            && !disposed)
        || (producer.coordinate.call_ordinal != 0
            && !projected_paths.is_empty()
            && !disposed
            && !matches!(
                caller.operations.as_slice(),
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
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    target_entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let (coordinate, structural_arguments, claim_transfers) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            claim_transfers,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            structural_arguments,
            claim_transfers,
            ..
        } => (coordinate, structural_arguments, claim_transfers.as_slice()),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        } => (coordinate, structural_arguments, &[][..]),
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        } => (
            coordinate,
            structural_arguments,
            completion_receipts.as_slice(),
        ),
        _ => {
            return unsupported("structural result use requires an ordinary or boundary call");
        }
    };
    if structural_arguments.len() != target_parameters.len() {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let operation_index = caller
        .operations
        .iter()
        .position(|candidate| {
            candidate
                .with_value_calls()
                .any(|candidate| candidate == operation)
        })
        .ok_or(LoweringError::Unsupported(
            "structural result consumer has no operation position",
        ))?;
    let authored =
        crate::call_source_custody::authored::locate_source(checked, caller.state, *coordinate)?;
    let authored_nested = authored
        .structural_arguments
        .iter()
        .any(|(_, expression)| expression_producer(checked, *expression).is_some());
    validate_nested_execution_order(checked, caller, coordinate.statement_index, authored_nested)?;
    let (source_machine, state) =
        crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    for (index, (argument, parameter)) in structural_arguments
        .iter()
        .zip(target_parameters)
        .enumerate()
    {
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == parameter.position).then_some(*expression)
            });
        let binding_ordinal = argument.source_structural_result_binding_ordinal();
        if let Some(expression) = expression
            && super::super::reference_results::validate_consumer(
                checked,
                caller,
                *coordinate,
                argument,
                parameter,
                expression,
            )?
        {
            if authored.boundary
                || parameter.is_self
                || target_entry_claims
                    .iter()
                    .any(|claim| claim.parameter_index as usize == index)
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
        if parameter.is_self
            && binding_ordinal.is_some()
            && matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            )
        {
            crate::call_source_custody::projected_receivers::validate(
                checked,
                caller,
                operation,
                target_parameters,
            )?;
            if target_entry_claims
                .iter()
                .any(|claim| claim.parameter_index as usize == index)
                || claim_transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            {
                return unsupported("plain local receiver acquired transferred claims");
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
            if validation::is_closed_primitive_array_type(&checked.typed, source_parameter.type_reference)
                && (authored.boundary
                    || !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
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
                    || source_parameter.is_self
                    || parameter.is_self
                    || !expression.is_some_and(|expression| matches!(
                        checked.expression_table.expression(expression), ExpressionNode::Name(name)
                            if name.symbol == source_parameter.symbol
                                && name.head_symbol == source_parameter.symbol
                                && checked.expression_table.name_path_members(name.members).len() == 1))
                    || target_entry_claims.iter().any(|claim| claim.parameter_index as usize == index)
                    || claim_transfers.iter().any(|transfer| transfer.argument_index as usize == index))
            {
                return unsupported("primitive array parameter requires exact whole owned ordinary transport");
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
        // Check both directions: an authored result cannot be replaced with a
        // same-typed parameter or construction-local plan.
        for candidate in &caller.operations {
            if let CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source:
                    source @ checked_trees::CheckedArrayConstructionSource::CallArgument {
                        call_ordinal,
                        parameter_position,
                    },
                result,
                ..
            } = candidate
            {
                // A literal has no local symbol. Its call occurrence and formal
                // position own the constructor even when it has no leaf values.
                let (source_expression, source_type) =
                    super::super::scalar_arrays::construction_expression(
                        checked,
                        caller.machine,
                        caller.state,
                        result.statement_index,
                        *source,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "array argument constructor lost its authored occurrence",
                    ))?;
                let names_result = result.statement_index == coordinate.statement_index
                    && *call_ordinal == coordinate.call_ordinal
                    && *parameter_position == parameter.position
                    && expression == Some(source_expression);
                if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                    return unsupported(
                        "array argument does not rejoin its exact constructor occurrence",
                    );
                }
                if names_result
                    && (authored.boundary
                        || argument.access != checked_trees::CheckedStructuralAccess::Owned
                        || !argument.path.is_empty()
                        || result.multiplicity != Multiplicity::Unrestricted
                        || !validation::is_closed_primitive_array_type(&checked.typed, source_type)
                        || checked
                            .typed
                            .normalized_type_identity(source_type)
                            .into_string()
                            != result.type_identity)
                {
                    return unsupported("array argument constructor lost whole owned custody");
                }
                continue;
            }
            let (producer_coordinate, source_site, result) = match candidate {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate: producer_coordinate,
                    source_site,
                    result,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate: producer_coordinate,
                    source_site,
                    result,
                    ..
                } => (*producer_coordinate, *source_site, result),
                CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => (
                    checked_trees::CheckedUnitCallCoordinate {
                        statement_index: result.statement_index,
                        call_ordinal: 0,
                    },
                    None,
                    result,
                ),
                _ => continue,
            };
            if producer_coordinate.call_ordinal == 0
                && matches!(
                    statements.get(result.statement_index as usize),
                    Some(StatementNode::Expression(_))
                )
            {
                if binding_ordinal == Some(result.binding_ordinal) {
                    return unsupported(
                        "terminal structural result cannot supply an earlier call operand",
                    );
                }
                continue;
            }
            if producer_coordinate.call_ordinal == 0
                && matches!(
                    statements.get(result.statement_index as usize),
                    Some(StatementNode::Call(_))
                )
            {
                crate::call_source_custody::initializers::validate_discarded_structural(
                    checked,
                    caller.machine,
                    caller.state,
                    producer_coordinate,
                    result,
                )?;
                if binding_ordinal == Some(result.binding_ordinal) {
                    return unsupported("discarded boundary result cannot supply a later operand");
                }
                continue;
            }
            let names_result = if producer_coordinate.call_ordinal == 0 {
                let Some(StatementNode::LocalData(local)) =
                    statements.get(result.statement_index as usize)
                else {
                    return unsupported("Unit structural result producer has no authored local");
                };
                if !local.symbol.is_valid()
                    || (result.multiplicity == Multiplicity::Unrestricted
                        && !(validation::is_closed_primitive_array_type(
                            &checked.typed,
                            local.type_reference,
                        ) || super::super::structural_values::plain_record(
                            checked,
                            local.type_reference,
                        )))
                    || checked
                        .typed
                        .normalized_type_identity(local.type_reference)
                        .into_string()
                        != result.type_identity
                {
                    return unsupported(
                        "Unit structural result producer disagrees with its authored local",
                    );
                }
                match expression.map(|expression| {
                    super::super::parameters::source_path(
                        checked,
                        source_machine,
                        local.type_reference,
                        expression,
                    )
                }) {
                    Some(Ok((root, path, access))) if root == local.symbol => {
                        if path != argument.path
                            || access.unwrap_or(checked_trees::CheckedStructuralAccess::Owned)
                                != argument.access
                        {
                            return unsupported(
                                "Unit result projection disagrees with its authored path",
                            );
                        }
                        true
                    }
                    _ => false,
                }
            } else {
                let source = crate::call_source_custody::authored::locate_source(
                    checked,
                    caller.state,
                    producer_coordinate,
                )?;
                if source.source_site != source_site {
                    return unsupported(
                        "nested structural producer has a different authored source",
                    );
                }
                let Some(checked_trees::NominalMachineUseSite::Expression(source_expression)) =
                    source.source_site
                else {
                    return unsupported("anonymous producer has no expression-owned source");
                };
                let matches = producer_coordinate.statement_index == coordinate.statement_index
                    && expression.and_then(|expression| expression_producer(checked, expression))
                        == Some(source_expression);
                if matches {
                    let signature = crate::call_source_custody::authored::target_signature(
                        checked,
                        source_machine.symbol,
                        source.source_target,
                    )?;
                    if result.multiplicity == Multiplicity::Unrestricted
                        && !validation::is_closed_primitive_array_type(
                            &checked.typed,
                            signature.return_type,
                        )
                    {
                        return unsupported(
                            "unrestricted structural operand has no primitive array producer",
                        );
                    }
                    let (root, path, access) = super::super::parameters::source_place_path(
                        checked,
                        source_machine,
                        signature.return_type,
                        expression.unwrap(),
                    )?;
                    if root != facts::PlaceRoot::Expression(source_expression)
                        || path != argument.path
                        || access.unwrap_or(checked_trees::CheckedStructuralAccess::Owned)
                            != argument.access
                    {
                        return unsupported(
                            "anonymous result projection disagrees with its authored source",
                        );
                    }
                    if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                        super::shared_temporary::validate(
                            checked,
                            caller,
                            producer_coordinate,
                            *coordinate,
                            source_expression,
                        )?;
                    }
                }
                matches
            };
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported(if producer_coordinate.call_ordinal == 0 {
                    "Unit structural result argument does not rejoin its exact authored local"
                } else {
                    "Unit structural result argument does not rejoin its exact authored source"
                });
            }
            if names_result
                && producer_coordinate.call_ordinal == 0
                && expression.is_none_or(|expression| {
                    named_result_operand(checked, expression).1 != argument.access
                })
            {
                return unsupported(
                    "Unit structural result access disagrees with its authored operand",
                );
            }
        }
        let Some(binding_ordinal) = binding_ordinal else {
            continue;
        };
        let producer = producer(&caller.operations, binding_ordinal)?;
        let result = producer.result;
        let source_order = producer.precedes_consumer(*coordinate);
        if (producer.discard && argument.access == checked_trees::CheckedStructuralAccess::Owned)
            || producer.operation_index >= operation_index
            || !source_order
            || matches!(operation, CheckedUnitEffectOperationPlan::StructuralCall { result: consumer, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result: consumer, .. }
                if result.binding_ordinal >= consumer.binding_ordinal)
            || !matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            )
            || (result.multiplicity == Multiplicity::Unrestricted
                && (!matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::CallUnit { .. }
                        | CheckedUnitEffectOperationPlan::ScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::StructuralCall { .. }
                ) || !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned))
            || (argument.path.is_empty() && argument.type_identity != result.type_identity)
            || parameter.type_identity != argument.type_identity
            || (!argument.path.is_empty()
                && (!matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
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
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || parameter.fused_service_erasure.is_some()
            || target_entry_claims
                .iter()
                .any(|claim| claim.parameter_index as usize == index)
            || claim_transfers
                .iter()
                .any(|transfer| transfer.argument_index as usize == index)
        {
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
    }
    Ok(())
}

fn named_result_operand(
    checked: &CheckedTrees,
    expression: checked_trees::expression::ExpressionHandle,
) -> (
    checked_trees::expression::ExpressionHandle,
    checked_trees::CheckedStructuralAccess,
) {
    if let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression)
        && borrow.access == language_core::ReferenceAccess::Shared
    {
        (
            borrow.target,
            checked_trees::CheckedStructuralAccess::SharedBorrow,
        )
    } else {
        (expression, checked_trees::CheckedStructuralAccess::Owned)
    }
}

fn validate_nested_execution_order(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    statement_index: u32,
    authored_nested: bool,
) -> Result<(), LoweringError> {
    if !authored_nested
        && !caller.operations.iter().any(|operation| {
            matches!(operation,
        CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
            if coordinate.statement_index == statement_index && coordinate.call_ordinal != 0)
        })
    {
        return Ok(());
    }
    let expected = crate::call_source_custody::authored::nested::authored_postorder(
        checked,
        caller.state,
        statement_index,
    )?;
    let mut actual = Vec::new();
    for operation in &caller.operations {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
                if coordinate.statement_index == statement_index =>
            {
                coordinate
            }
            _ => continue,
        };
        actual.push(coordinate.call_ordinal);
    }
    if !actual
        .iter()
        .copied()
        .eq(expected.iter().filter_map(|(ordinal, expression)| {
            if *ordinal != 0
                && let ExpressionNode::Call(call) = checked.expression_table.expression(*expression)
                && crate::scalar_source_custody::authored_state(checked, call.target_symbol)
                    .is_ok_and(|(_, target)| {
                        checked
                            .primitive_type_reference(target.return_type)
                            .is_some()
                    })
            {
                None
            } else {
                Some(*ordinal)
            }
        }))
    {
        return unsupported(
            "nested structural operations disagree with authored argument execution order",
        );
    }
    Ok(())
}
