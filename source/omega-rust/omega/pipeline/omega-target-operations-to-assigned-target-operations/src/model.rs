use omega_target::{Architecture, NativeTarget};
use omega_target_operations::MachineRegister;
use psi_core::{MachineId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    DynamicDescriptorAssignmentMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    EntryFunctionMissing(MachineId),
    RankedCountdownAbiMismatch(ValueId),
    RankedCountdownRequiresRegister(ValueId),
    UnsupportedScalarCleanup(MachineId),
    InstalledProviderCallRequiresOptimizedLane {
        machine: MachineId,
        operation: OperationId,
        boundary: psi_core::BoundaryMachineId,
    },
    InstalledProviderScalarCallCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
        boundary: psi_core::BoundaryMachineId,
    },
    BoundaryPortReadUnsupported {
        machine: MachineId,
        architecture: Architecture,
    },
    LinuxExitGroupUnsupported {
        machine: MachineId,
        target: NativeTarget,
    },
    LinuxExitGroupArgumentMismatch(MachineId),
    UnsupportedStructuralPlacement(psi_core::PlaceId),
    StructuralRegisterArchitectureMismatch {
        place: psi_core::PlaceId,
        register: MachineRegister,
        architecture: Architecture,
    },
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterAssignmentMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackFrameNotEncodable,
    UnitScalarFrameNotEncodable,
    UnitScalarHomeNotEncodable(ValueId),
    UnitScalarCallCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    UnitScalarCallSourceMismatch(ValueId),
    UnitCallCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    StructuralScalarFieldStoreCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    StructuralScalarCallCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    DynamicDescriptorCallArgumentMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    DynamicScalarCallCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    IeeeFloatFmaCustodyMismatch {
        machine: MachineId,
        operation: OperationId,
    },
    DuplicateNativeCallbackArgument(OperationId),
    MultipleNativeCallbackArguments,
    UnknownNativeCallbackArgument(OperationId),
    InvalidNativeCallbackArgument(OperationId),
    ExpressionRegisterCannotHoldParameter {
        value: ValueId,
        register: MachineRegister,
    },
    UnsupportedCallArgumentRegister(MachineRegister),
}

impl std::fmt::Display for AssignmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AssignmentError {}
