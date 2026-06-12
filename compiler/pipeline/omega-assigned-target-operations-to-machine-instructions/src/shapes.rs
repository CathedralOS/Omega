mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;
mod validation;

use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::MachineInstructionKind;

use validation::ensure_runtime_value_homes;

pub(super) fn lower_machine_instruction_kind(
    assigned_target_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    selected_instruction_handle: Handle<omega_assigned_target_operations::SelectedInstruction>,
    kind: &SelectedInstructionKind,
) -> Result<MachineInstructionKind, Diagnostic> {
    ensure_runtime_value_homes(
        assigned_target_operations,
        selected_instruction_handle,
        kind,
    )?;
    if let Some(kind) = runtime_text::selected_runtime_text_kind(kind) {
        return Ok(kind);
    }
    if let Some(kind) = runtime_storage::selected_runtime_storage_kind(kind) {
        return Ok(kind);
    }

    Ok(match kind {
        SelectedInstructionKind::HostOperation { operation_key, .. } => {
            host::host_operation_kind(*operation_key)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => dispatch::dispatch_loop_enter_kind(*entry_dispatch_index),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            dispatch::dispatch_case_enter_kind(*dispatch_index)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => dispatch::dispatch_guard_compare_static_kind(
            *operator,
            *byte_offset,
            *byte_size,
            *expected_value,
        ),
        SelectedInstructionKind::EvaluateDispatchGuard { .. } => MachineInstructionKind::NoOp,
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            dispatch::dispatch_state_write_kind(*dispatch_index)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger { .. } => {
            MachineInstructionKind::ReturnRegisterIntegerWrite
        }
        SelectedInstructionKind::AppendWireLiteralByte { .. } => {
            MachineInstructionKind::WireLiteralByteAppend
        }
        SelectedInstructionKind::AppendWireScalarVarint { .. } => {
            MachineInstructionKind::WireScalarVarintAppend
        }
        SelectedInstructionKind::AppendWireTextBytes { .. } => {
            MachineInstructionKind::WireTextBytesAppend
        }
        SelectedInstructionKind::ReadWireExpectedByte { .. } => {
            MachineInstructionKind::WireExpectedByteRead
        }
        SelectedInstructionKind::ReadWireScalarVarint { .. } => {
            MachineInstructionKind::WireScalarVarintRead
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { .. } => {
            MachineInstructionKind::RuntimeStorageCopyToReturnRegister
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::dispatch_terminate_kind(),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::dispatch_case_leave_kind(),
        SelectedInstructionKind::LeaveFunction => dispatch::return_kind(),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => MachineInstructionKind::NoOp,
        _ => unreachable!(
            "selected instruction family should be routed before top-level shape dispatch"
        ),
    })
}
