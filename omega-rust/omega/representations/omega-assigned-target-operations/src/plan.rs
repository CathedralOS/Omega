use omega_target::NativeTarget;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::TerminalPsiIdentity;

use crate::{AssignedCallDestination, AssignedOperation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<AssignedFunction>,
}

/// One native-only callback function after exact physical destination
/// assignment. The target row remains intact so later emission can replay the
/// symbolic function identity and complete native-parameter application
/// without reconstructing either from this destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedNativeCallbackArgument {
    pub target: omega_target_operations::TargetNativeCallbackArgument,
    pub destination: AssignedCallDestination,
}

/// Compatibility-preserving assigned plan plus its exact native-only callback
/// arguments. The callback rows remain separate from semantic scalar values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlanWithNativeCallbacks {
    pub plan: AssignedOperationPlan,
    pub native_callback_arguments: Vec<AssignedNativeCallbackArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
    pub mixed_structural_scalar_abi:
        Option<omega_target_operations::MixedStructuralScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub operation: AssignedOperation,
}
