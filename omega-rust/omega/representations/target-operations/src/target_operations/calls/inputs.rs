//! Native callback and placed-view inputs, retained with their exact application.

use crate::TargetOperationPlan;
use calling_conventions::{BoundaryEntryPlan, CallPlan, ValuePlacement};
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalPlacedViewInput;

/// One target-owned native-only callback argument joined to an exact
/// Terminal boundary-call occurrence.
///
/// This is deliberately kept beside the ordinary target-operation plan. The
/// callback has no Terminal [`semantic_vocabulary::ValueId`] and therefore must not be inserted
/// into a boundary call's semantic scalar-argument roster. A later assignment
/// stage must bind `application.placement` to an exact physical operand while
/// retaining the symbolic callback-function identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetNativeCallbackArgument {
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    /// Retained compiler-origin provenance. Target lowering cannot
    /// independently authenticate this commitment from the reduced tuple.
    pub registrar_application_commitment: [u8; 32],
}

/// Compatibility-preserving target plan plus the exact native-only callback
/// arguments consumed by its normalized foreign calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlanWithNativeCallbacks {
    pub plan: TargetOperationPlan,
    pub native_callback_arguments: Vec<TargetNativeCallbackArgument>,
}

/// One exact plan-laid input joined to its target pointer placement.
///
/// The referent geometry remains explicit audit data. The pointer placement
/// carries no backing, lifetime, or access-event authority by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPlacedViewInput {
    pub terminal: TerminalPlacedViewInput,
    pub abi_parameter_ordinal: u32,
    pub referent_byte_size: u64,
    pub referent_alignment: u64,
    pub placement: ValuePlacement,
}

/// Staging contract for the first direct-entry plan-laid ABI slice.
///
/// `entry_call_plan` is the complete entry ABI for this carrier. The nested
/// ordinary plan intentionally retains its pre-existing ABI and cannot be
/// mistaken for the placed-input-aware entry without explicitly unwrapping
/// this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlanWithPlacedViewInputs {
    pub plan: TargetOperationPlan,
    pub entry_call_plan: CallPlan,
    pub placed_view_inputs: Vec<TargetPlacedViewInput>,
}
