//! Emit task-register instructions after their ordinary call operands complete.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::shared::lowering_error::LoweringError;
use crate::shared::terminal_identities::{lookup_service_id, mint_with, value_id};
use language_core::task_register::TaskRegisterOperation;
use semantic_vocabulary::{QualifiedScalarType, ServiceId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralArgument, ValueDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredTaskRegisterBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
    pub(crate) source_site: checked_trees::NominalMachineUseSite,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) operation: TaskRegisterOperation,
    pub(crate) service: language_semantics::ServiceReachId,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
}

/// Both statement and expression calls consume the same checked operand roster.
pub(crate) fn operation_kind(
    action: TaskRegisterOperation,
    service: ServiceId,
    arguments: &[ValueDeclaration],
    storage: Vec<StructuralArgument>,
) -> Result<OperationKind, LoweringError> {
    let missing = |operand: &str| {
        LoweringError::InvalidDeclarationIdentity(format!(
            "checked task-register instruction lost its {operand} operand"
        ))
    };
    Ok(match action {
        TaskRegisterOperation::Activate { entries } => {
            let selector = arguments.first().ok_or_else(|| missing("selector"))?.id;
            let mut storage = storage.into_iter();
            OperationKind::ActivateTaskRegister {
                service,
                entries,
                selector,
                gdt: storage.next().ok_or_else(|| missing("GDT"))?,
                task_state: storage.next().ok_or_else(|| missing("task-state"))?,
                scratch: storage.next().ok_or_else(|| missing("scratch"))?,
            }
        }
        TaskRegisterOperation::Selector => OperationKind::ReadTaskRegister { service },
    })
}

pub(super) fn emit(
    call: &LoweredTaskRegisterBinding,
    parameters: &[ValueDeclaration],
    frontier: &lowered_psi::LoweredSourceFrontierBindings,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let service = lookup_service_id(calls.service_ids, call.service)?;
    let parameter_types = parameters
        .iter()
        .map(|value| value.value_type())
        .collect::<Vec<_>>();
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let value_type = argument.value_type(&parameter_types)?;
            Ok(ValueDeclaration {
                id: emit_direct_expression(argument, parameters, next_value_identity, operations),
                scalar_type: value_type.scalar_type,
                qualifications: value_type.qualifications,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let storage = call
        .structural_arguments
        .iter()
        .map(|argument| argument.emit(parameters, calls, next_value_identity, operations))
        .collect::<Result<Vec<_>, _>>()?;
    let kind = operation_kind(call.operation, service, &arguments, storage)?;
    let result = mint_with(
        next_value_identity,
        value_id,
        "task-register result identity overflow",
    )?;
    let operation = operations.allocate();
    operations.record_source_call(
        call.source_coordinate,
        Some(call.source_site),
        operation,
        call.target_machine,
        frontier,
    )?;
    operations.push(Operation::unsuspended(
        operation,
        OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: call.result_type.qualifications,
        }),
        kind,
    ));
    Ok(result)
}
