//! Emit one selector call (`Trait::member(selector, data...)`) inside an
//! operand evaluation.
//!
//! The computation expansion completes the selector first and then the data
//! operands, so all are parameters of the emitting block. The call becomes
//! one `CallDynamic` naming its interface slot; the selector is the dispatch
//! operand, never one of the arguments the selected implementation receives.
//! The Unit operation frame emits a statement-level selector call the same
//! way (`unit/attached_unit/operation_frame/dynamic_calls.rs`).
//! Owned operands transfer their checked claims under that slot's typed
//! relative paths, exactly like a static call. Choosing an implementation
//! neither creates custody nor changes which data arguments are consumed.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::value_id;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::{QualifiedScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

/// A completed selector call: its selector and data operands are parameters
/// of the emitting block, and its result is one bare scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredDynamicCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
    /// The authored call expression this invocation executes.
    pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
    /// The instantiated slot the call targets.
    pub(crate) slot: symbols::SymbolHandle,
    pub(crate) interface_identity: typed_trees::type_identity::TypeIdentityId,
    pub(crate) requirement_identity: typed_trees::type_identity::CallableIdentityId,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) selector: LoweredDirectExpression,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    /// Evaluated finite-family actuals in roster-binder order; dispatch
    /// matches them against each family slot's declared tuple identities.
    pub(crate) family_actuals: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
    pub(crate) claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
}

pub(super) fn emit_dynamic_call_binding(
    call: &LoweredDynamicCallBinding,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    if !call.result_type.qualifications.is_empty() {
        return unsupported("a selector call's result carries qualifications");
    }
    if !call.family_actuals.is_empty() {
        return unsupported("finite-family selector call outside its dispatch expansion");
    }
    let (interface, slot) = operations
        .dynamic_dispatch
        .resolve_slot(&call.interface_identity, &call.requirement_identity)?;
    let selector =
        emit_direct_expression(&call.selector, parameters, next_value_identity, operations);
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            emit_direct_expression(argument, parameters, next_value_identity, operations)
        })
        .collect();
    let family_actuals = call
        .family_actuals
        .iter()
        .map(|actual| emit_direct_expression(actual, parameters, next_value_identity, operations))
        .collect();
    let structural_arguments = call
        .structural_arguments
        .iter()
        .map(|argument| argument.emit(parameters, call_emission, next_value_identity, operations))
        .collect::<Result<Vec<_>, _>>()?;
    let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
        &call.claim_transfers,
        call_emission.claims.bindings(),
    )?;
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "selector call value identity overflow",
        ))?;
    let operation = operations.allocate();
    operations.record_source_call(
        call.source_coordinate,
        Some(checked_trees::NominalMachineUseSite::Expression(
            call.source_expression,
        )),
        operation,
        call.slot,
        &lowered_psi::LoweredSourceFrontierBindings::default(),
    )?;
    emit_dynamic_call_leaf(
        selector,
        interface,
        slot,
        arguments,
        family_actuals,
        structural_arguments,
        claim_transfers,
        OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: Default::default(),
        }),
        operation,
        operations,
    );
    Ok(result)
}

/// Push one resolved selector call: the one `CallDynamic` shape every
/// selector-call emitter produces, for an operand evaluation, a statement
/// in the Unit operation frame, or one tuple leaf of a finite-family
/// dispatch. `produced` names the result the enclosing dispatch merges;
/// leaf callers pass their own tuple slot so each emitted indirect call
/// keeps the ordinary `{selector, interface, slot}` shape.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_dynamic_call_leaf(
    selector: ValueId,
    interface: u32,
    slot: u32,
    arguments: Vec<ValueId>,
    family_actuals: Vec<ValueId>,
    structural_arguments: Vec<terminal_psi::StructuralArgument>,
    claim_transfers: Vec<terminal_psi::ClaimTransfer>,
    produced: OperationResult,
    operation: OperationId,
    operations: &mut OperationBuffer,
) {
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: produced,
        kind: OperationKind::CallDynamic {
            selector,
            interface,
            slot,
            arguments,
            family_actuals,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            claim_transfers,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
}

/// Emit one completed invocation, including finite-family row selection. Every
/// caller supplies already evaluated operands and its surrounding block span;
/// result shape does not select another dispatch pipeline. A structural family
/// result receives fresh leaf homes and rejoins the authored result at the merge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_completed_dynamic_call(
    interface_identity: &typed_trees::type_identity::TypeIdentityId,
    requirement_identity: &typed_trees::type_identity::CallableIdentityId,
    operands: &[ValueDeclaration],
    family_actuals: &[ValueDeclaration],
    structural_arguments: Vec<terminal_psi::StructuralArgument>,
    claim_transfers: Vec<terminal_psi::ClaimTransfer>,
    result: &OperationResult,
    operation: OperationId,
    span: &mut crate::unit::attached_unit::argument_evaluation::BlockEmissionSpan,
    blocks: &mut Vec<terminal_psi::Block>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    mut structural_result_place: impl FnMut(
        usize,
    ) -> Result<semantic_vocabulary::PlaceId, LoweringError>,
) -> Result<bool, LoweringError> {
    let (selector, arguments) = operands.split_first().ok_or(LoweringError::Unsupported(
        "selector call has no evaluated selector operand",
    ))?;
    let (interface, slot) = operations
        .dynamic_dispatch
        .resolve_slot(interface_identity, requirement_identity)?;
    let arguments = arguments.iter().map(|value| value.id).collect::<Vec<_>>();
    let actuals = family_actuals
        .iter()
        .map(|value| value.id)
        .collect::<Vec<_>>();
    if family_actuals.is_empty() {
        emit_dynamic_call_leaf(
            selector.id,
            interface,
            slot,
            arguments,
            actuals,
            structural_arguments,
            claim_transfers,
            result.clone(),
            operation,
            operations,
        );
        return Ok(false);
    }
    let roster =
        operations
            .dynamic_dispatch
            .family_roster(interface, slot, requirement_identity)?;
    let actual_types = family_actuals
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    let mut first_leaf = Some(operation);
    crate::unit::attached_unit::argument_evaluation::emit_dynamic_family_dispatch(
        span,
        blocks,
        operations,
        next_value,
        next_block,
        next_edge,
        &roster,
        &actuals,
        &actual_types,
        result,
        |operations, next_value, tuple| {
            let produced = match result {
                OperationResult::Unit => OperationResult::Unit,
                OperationResult::Scalar(value) => OperationResult::Scalar(ValueDeclaration {
                    id: value_id(crate::terminal_identities::allocate_dense(next_value)?),
                    ..*value
                }),
                OperationResult::Structural(value) => {
                    let mut value = value.clone();
                    value.place = structural_result_place(tuple)?;
                    OperationResult::Structural(value)
                }
            };
            let operation = first_leaf.take().unwrap_or_else(|| operations.allocate());
            let slot = slot
                .checked_add(
                    u32::try_from(tuple)
                        .map_err(|_| LoweringError::Unsupported("dynamic family slot overflows"))?,
                )
                .ok_or(LoweringError::Unsupported("dynamic family slot overflows"))?;
            emit_dynamic_call_leaf(
                selector.id,
                interface,
                slot,
                arguments.clone(),
                actuals.clone(),
                structural_arguments.clone(),
                claim_transfers.clone(),
                produced.clone(),
                operation,
                operations,
            );
            Ok(produced)
        },
    )?;
    Ok(true)
}
