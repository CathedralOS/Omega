mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;
mod validation;

use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_machine_instructions::MachineInstructionKind;
use psi_arena::Handle;
use psi_diagnostics::Diagnostic;

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
        SelectedInstructionKind::DynamicTableCall { .. } => {
            MachineInstructionKind::DynamicTableCallSequence
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
        SelectedInstructionKind::AppendWireScalarSlice { .. } => {
            MachineInstructionKind::WireScalarSliceAppend
        }
        SelectedInstructionKind::ReadWireExpectedByte { .. } => {
            MachineInstructionKind::WireExpectedByteRead
        }
        SelectedInstructionKind::ReadWireScalarVarint { .. } => {
            MachineInstructionKind::WireScalarVarintRead
        }
        SelectedInstructionKind::ReadWireByteSlice { .. } => {
            MachineInstructionKind::WireByteSliceRead
        }
        SelectedInstructionKind::ReadWireNestedOpen { .. } => {
            MachineInstructionKind::WireNestedOpenRead
        }
        SelectedInstructionKind::ReadWireNestedClose { .. } => {
            MachineInstructionKind::WireNestedCloseRead
        }
        SelectedInstructionKind::AppendWireRepeatedScalarVarint { .. } => {
            MachineInstructionKind::WireRepeatedScalarVarintAppend
        }
        SelectedInstructionKind::ReadWireRepeatedScalarVarint { .. } => {
            MachineInstructionKind::WireRepeatedScalarVarintRead
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { .. } => {
            MachineInstructionKind::RuntimeStorageCopyToReturnRegister
        }
        SelectedInstructionKind::WriteEntryArgumentRegister { .. } => {
            MachineInstructionKind::EntryArgumentRegisterWrite
        }
        SelectedInstructionKind::WriteEntryStackArgument { .. } => {
            MachineInstructionKind::EntryStackArgumentWrite
        }
        SelectedInstructionKind::WriteEntryIndirectArgument { .. } => {
            MachineInstructionKind::EntryIndirectArgumentWrite
        }
        SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. } => {
            MachineInstructionKind::EntryArgumentsSliceDescriptorWrite
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::dispatch_terminate_kind(),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::dispatch_case_leave_kind(),
        SelectedInstructionKind::CallInternalFunction { .. } => {
            MachineInstructionKind::InternalFunctionCall
        }
        SelectedInstructionKind::LoadOutgoingStackAddress { .. } => {
            MachineInstructionKind::OutgoingStackAddressLoad
        }
        SelectedInstructionKind::ReserveOutgoingStackFrame { .. } => {
            MachineInstructionKind::OutgoingStackFrameReserve
        }
        SelectedInstructionKind::WriteOutgoingStackU64 { .. } => {
            MachineInstructionKind::OutgoingStackU64Write
        }
        SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack { .. } => {
            MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy
        }
        SelectedInstructionKind::ReleaseOutgoingStackFrame { .. } => {
            MachineInstructionKind::OutgoingStackFrameRelease
        }
        SelectedInstructionKind::MachineHalt => MachineInstructionKind::MachineHalt,
        SelectedInstructionKind::MemoryFence(kind) => MachineInstructionKind::MemoryFence(*kind),
        SelectedInstructionKind::InterruptControl(kind) => {
            MachineInstructionKind::InterruptControl(*kind)
        }
        SelectedInstructionKind::FlagsSnapshot { .. } => MachineInstructionKind::FlagsSnapshot,
        SelectedInstructionKind::FlagsRestore { .. } => MachineInstructionKind::FlagsRestore,
        SelectedInstructionKind::MsrRead { .. } => MachineInstructionKind::MsrRead,
        SelectedInstructionKind::MsrWrite { .. } => MachineInstructionKind::MsrWrite,
        SelectedInstructionKind::ControlRegisterRead { register, .. } => {
            MachineInstructionKind::ControlRegisterRead(*register)
        }
        SelectedInstructionKind::ControlRegisterWrite { register, .. } => {
            MachineInstructionKind::ControlRegisterWrite(*register)
        }
        SelectedInstructionKind::PortWrite { .. } => MachineInstructionKind::PortWrite,
        SelectedInstructionKind::PortRead { .. } => MachineInstructionKind::PortRead,
        SelectedInstructionKind::WriteDataAddressToRuntimeFrame { .. } => {
            MachineInstructionKind::DataAddressToRuntimeFrameWrite
        }
        SelectedInstructionKind::WriteFunctionAddressToRuntimeStorage { .. } => {
            MachineInstructionKind::FunctionAddressToRuntimeStorageWrite
        }
        SelectedInstructionKind::LeaveFunction => dispatch::return_kind(),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => MachineInstructionKind::NoOp,
        _ => unreachable!(
            "selected instruction family should be routed before top-level shape dispatch"
        ),
    })
}
