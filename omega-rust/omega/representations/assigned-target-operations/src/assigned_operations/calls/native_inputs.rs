//! calls native inputs in the assigned operations program.

use crate::AssignedCallDestination;
use crate::AssignedOperationPlan;

/// One native-only callback function after exact physical destination
/// assignment. The target row remains intact so later emission can replay the
/// symbolic function identity and complete native-parameter application
/// without reconstructing either from this destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedNativeCallbackArgument {
    pub target: target_operations::TargetNativeCallbackArgument,
    pub destination: AssignedCallDestination,
}

/// Compatibility-preserving assigned plan plus its exact native-only callback
/// arguments. The callback rows remain separate from semantic scalar values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperationPlanWithNativeCallbacks {
    pub plan: AssignedOperationPlan,
    pub native_callback_arguments: Vec<AssignedNativeCallbackArgument>,
}
