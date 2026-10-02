//! Local receivers retain the exact producer's storage and declared access.
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, ReceiverSource, SymbolHandle,
    unsupported,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitCallCoordinate, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralParameterPlan, types::TypeReferenceNode,
};

/// An established shared-borrow local: a unique immutable declaration whose
/// carrier is a `&T` type. Its storage is the referent's established place
/// joined through its own result binding, never an owned record home -- so
/// `declaration` above intentionally does not admit it. A mutable binding
/// could reseat the referent and is not this carrier.
pub(super) fn shared_borrow<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<&'a checked_trees::statement::TableLocalData> {
    let (_, local) = checked
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement)?;
    (!local.is_mutable
        && local.initial_value.is_valid()
        && matches!(
            checked
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                ..
            }
        ))
    .then_some(local)
}

/// An immutable local whose value is a mutable reference returned by a call.
/// Its result binding carries permission, not owned record storage, so it must
/// never enter [`declaration`]'s owned-local route.
pub(super) fn returned_reference<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<(usize, &'a checked_trees::statement::TableLocalData)> {
    let (ordinal, local) = checked
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement)?;
    (!local.is_mutable
        && local.initial_value.is_valid()
        && matches!(
            checked.expression_table.expression(local.initial_value),
            checked_trees::expression::ExpressionNode::Call(_)
        )
        && validation::reference_result_custody::mutable_borrowed_parts(
            &checked.typed,
            local.type_reference,
        )
        .is_some())
    .then_some((ordinal, local))
}

pub(crate) fn declaration<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<(usize, &'a checked_trees::statement::TableLocalData)> {
    let (ordinal, local) = checked
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement)?;
    // An uninitialized `let mut` whose statement carries a checked
    // `ZeroedStructural` establishment is the same owned home an
    // initialized local is: its storage exists and begins zero.
    let zeroed_establishment = !local.initial_value.is_valid()
        && checked
            .facts
            .values
            .structural_values
            .root_at(state.symbol, ordinal as u32)
            .is_some_and(|root| {
                matches!(
                    checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(root.root)
                        .kind,
                    checked_trees::CheckedStructuralValueKind::ZeroedStructural {}
                )
            });
    // Primitive defaults use a scalar initializer, not a structural-value
    // root. Rejoin that declaration and independently verify its exact zero
    // before treating the name as owned storage rather than an alias loan.
    let primitive_zero = !local.initial_value.is_valid()
        && checked
            .primitive_type_reference(local.type_reference)
            .and_then(|primitive| {
                crate::emission::scalar_types::terminal_scalar_type(primitive).ok()
            })
            .is_some_and(|scalar_type| {
                checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        state.symbol,
                        ordinal as u32,
                        checked_trees::CheckedScalarExpressionRole::StorageInitializer,
                    )
                    .is_some_and(|(binding, _)| {
                        binding.destination == local.symbol
                            && crate::expression_preparation::source_custody::validate_pure(
                                checked,
                                binding,
                                scalar_type,
                            )
                            .is_ok()
                    })
            });
    // Establishment already owns the declaration's represented byte predicates.
    // Resolving its home must retain them, not exclude byte-bearing records or
    // recover the initializer as if subsequent mutable calls had not happened.
    ((local.initial_value.is_valid() || zeroed_establishment || primitive_zero)
        && validation::has_plain_owned_contents_with_represented_constraints(
            &checked.typed,
            local.type_reference,
        ))
    .then_some((ordinal, local))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_operations: &[CheckedUnitEffectOperationPlan],
    coordinate: CheckedUnitCallCoordinate,
    target_symbol: SymbolHandle,
    target: &CheckedUnitStructuralParameterPlan,
    argument: &CheckedUnitStructuralArgumentPlan,
    source: &ReceiverSource,
) -> Result<(), LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let (ordinal, local) = declaration(
        checked,
        state,
        coordinate.statement_index as usize,
        source.root,
    )
    .ok_or(LoweringError::Unsupported(
        "local receiver declaration missing",
    ))?;
    let binding =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "local receiver does not name established result storage",
            ))?;
    let mut producers = caller_operations.iter().filter_map(|operation| {
        // A constructed value, a moved field, or a call with a structural
        // result, ordinary or boundary.
        let result = match operation {
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, .. } => Some(result),
            _ => operation
                .discardable_structural_result()
                .map(|(result, _)| result),
        }?;
        (result.binding_ordinal == binding).then_some(result)
    });
    let result = producers.next().ok_or(LoweringError::Unsupported(
        "local receiver producer missing",
    ))?;
    let referent = if source.path.is_empty() {
        local.type_reference
    } else {
        checked
            .data_definitions()
            .iter()
            .flat_map(|data| checked.data_members(data))
            .find_map(|member| match member {
                checked_trees::data::DataMember::Field(field) if field.symbol == source.stamp => {
                    Some(field.type_reference)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "local receiver projected type missing",
            ))?
    };
    if producers.next().is_some()
        || result.statement_index as usize != ordinal
        || result.type_identity
            != checked
                .normalized_type_identity(local.type_reference)
                .as_str()
        || result.multiplicity != checked.type_multiplicity(local.type_reference)
        || !matches!(
            result.multiplicity,
            language_semantics::Multiplicity::Affine
                | language_semantics::Multiplicity::Unrestricted
        )
        || !target.is_self
        || !target.qualifications.is_empty()
        || source.erased_alias
        || argument.path != source.path
        || argument.type_identity != target.type_identity
        || argument.access != target.access
        || checked.normalized_type_identity(referent).as_str() != target.type_identity
        // A detached field is an ordinary owned local. Consuming its whole
        // value transfers custody; borrowing it preserves the same storage.
        // Partial owned projection still needs its own remainder accounting.
        || !match argument.access {
            CheckedStructuralAccess::Owned => source.path.is_empty(),
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow => true,
            _ => false,
        }
        || (argument.access == CheckedStructuralAccess::MutableBorrow && !local.is_mutable)
    {
        return unsupported("local receiver loan substituted its producer, type, path, or access");
    }
    validate_captured_receiver(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        target_symbol,
        source.stamp,
    )
}

/// A call-result reference local lends the producer's referent. Reconstruct
/// the exact result binding, producer flow call, minted loan, release boundary,
/// and consumer receiver instead of treating the permission carrier as an
/// owned local home.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_returned_reference(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_operations: &[CheckedUnitEffectOperationPlan],
    coordinate: CheckedUnitCallCoordinate,
    resolved_target: SymbolHandle,
    authored_target: SymbolHandle,
    target: &CheckedUnitStructuralParameterPlan,
    argument: &CheckedUnitStructuralArgumentPlan,
    source: &ReceiverSource,
    ordinal: usize,
    local: &checked_trees::statement::TableLocalData,
) -> Result<(), LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let binding =
        argument
            .source_structural_result_binding_ordinal()
            .ok_or(LoweringError::Unsupported(
                "returned-reference receiver has no result binding",
            ))?;
    let mut producers = caller_operations.iter().filter_map(|operation| {
        let call = operation.call()?;
        let result = call.structural_result()?;
        (result.binding_ordinal == binding).then_some((call, result))
    });
    let (producer, result) = producers.next().ok_or(LoweringError::Unsupported(
        "returned-reference receiver producer missing",
    ))?;
    let (referent, access) = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .ok_or(LoweringError::Unsupported(
        "returned-reference receiver is not mutable",
    ))?;
    if producers.next().is_some()
        || result.statement_index as usize != ordinal
        || result.type_identity
            != checked
                .normalized_type_identity(local.type_reference)
                .as_str()
        || result.multiplicity != language_semantics::Multiplicity::Affine
        || !target.is_self
        || !target.qualifications.is_empty()
        || source.root != local.symbol
        || source.stamp != local.symbol
        || source.owner != local.symbol
        || source.erased_alias
        || !source.path.is_empty()
        || source.captured_place().root_symbol != local.symbol
        || !source.captured_place().segments.is_empty()
        || argument.path != [checked_trees::CheckedUnitStructuralPathSegment::Referent]
        || argument.type_identity != target.type_identity
        || argument.access != access
        || argument.access != target.access
        || checked.normalized_type_identity(referent).as_str() != target.type_identity
    {
        return unsupported(
            "returned-reference receiver substituted its producer, referent, path, or access",
        );
    }
    let producer_flow =
        crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call(
            checked,
            caller_machine,
            caller_state,
            producer.coordinate,
            producer.target.state,
        )?;
    let loan = validation::reference_result_custody::result_loan(
        &checked.typed,
        &checked.facts,
        caller_machine,
        state,
        producer_flow,
        result,
    )
    .ok_or(LoweringError::Unsupported(
        "returned-reference receiver has no exact producer loan",
    ))?;
    if producer
        .structural_custody()
        .is_none_or(|custody| custody.reference_loan != loan)
    {
        return unsupported("returned-reference receiver changed its producer loan");
    }
    let release = validation::reference_result_custody::release_statement(
        &checked.facts,
        caller_machine,
        caller_state,
        loan,
    )
    .ok_or(LoweringError::Unsupported(
        "returned-reference receiver has no exact release",
    ))?;
    if coordinate.statement_index <= result.statement_index || coordinate.statement_index >= release
    {
        return unsupported("returned-reference receiver is outside its loan lifetime");
    }
    let consumer_flow =
        crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call(
            checked,
            caller_machine,
            caller_state,
            coordinate,
            resolved_target,
        )?;
    if !consumer_flow.has_receiver || consumer_flow.receiver_symbol != source.stamp {
        return unsupported("returned-reference receiver changed its captured flow receiver");
    }
    validate_captured_receiver(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        authored_target,
        source.stamp,
    )
}

fn validate_captured_receiver(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target_symbol: SymbolHandle,
    receiver_symbol: SymbolHandle,
) -> Result<(), LoweringError> {
    let borrow = &checked.facts.borrow;
    let state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == caller_machine && state.state_symbol == caller_state)
        .ok_or(LoweringError::Unsupported(
            "local receiver borrow state missing",
        ))?;
    let mut calls = borrow
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
                && call.target_symbol == target_symbol
        });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "local receiver borrow occurrence missing",
    ))?;
    // The source reader above independently reconstructs the receiver path;
    // implicit receiver capture is separate from explicit argument accesses.
    if calls.next().is_some() || !call.has_receiver || call.receiver_symbol != receiver_symbol {
        return unsupported("local receiver lacks its exact captured call receiver");
    }
    Ok(())
}
