//! Emit one selector call (`Trait::member(selector, data...)`) inside an
//! operand evaluation.
//!
//! The computation expansion completes the selector first and then the data
//! operands, so all are parameters of the emitting block. The call becomes
//! one `CallDynamic` naming its interface slot; the selector is the dispatch
//! operand, never one of the arguments the selected implementation receives.
//! The Unit operation frame emits a statement-level selector call the same
//! way (`unit/attached_unit/operation_frame/dynamic_calls.rs`).

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::value_id;
use semantic_vocabulary::{QualifiedScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

/// A completed selector call: its selector and data operands are parameters
/// of the emitting block, and its result is one bare scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredDynamicCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    /// The authored call expression this invocation executes.
    pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
    /// The instantiated slot the call targets.
    pub(crate) slot: symbols::SymbolHandle,
    pub(crate) interface_identity: String,
    pub(crate) requirement_identity: String,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) selector: LoweredDirectExpression,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
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
    let interface = operations
        .dynamic_dispatch
        .interface(&call.interface_identity)
        .ok_or(LoweringError::Unsupported(
            "selector call names an interface no implementation is selected for",
        ))?;
    let slot = operations
        .dynamic_dispatch
        .slot(interface, &call.requirement_identity)
        .ok_or(LoweringError::Unsupported(
            "selector call names a requirement absent from its interface",
        ))?;
    let selector =
        emit_direct_expression(&call.selector, parameters, next_value_identity, operations);
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            emit_direct_expression(argument, parameters, next_value_identity, operations)
        })
        .collect();
    let structural_arguments = call
        .structural_arguments
        .iter()
        .map(|argument| argument.emit(parameters, call_emission, next_value_identity, operations))
        .collect::<Result<Vec<_>, _>>()?;
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
    )?;
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: Default::default(),
        }),
        kind: OperationKind::CallDynamic {
            selector,
            interface,
            slot,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    Ok(result)
}
