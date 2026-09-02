use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target_operations::{
    BoundaryExecutionBinding, BoundaryScalarArgument, CompletionClaimSource,
    DirectPortReadU8Realization, LinuxExitGroupI32Realization, MachineRegister,
    TargetStructuralParameter,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultClaimTransfer, StructuralResultDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalDynamicDescriptorParameter,
    TerminalDynamicRequirement,
};

use crate::{
    AssignedAggregateCopy, AssignedBooleanControl, AssignedBooleanExpression,
    AssignedConditionalBooleanArm, AssignedConditionalIntegerArm, AssignedIntegerExpression,
    AssignedRankedU32Countdown, AssignedScalarLocation, AssignedUnitBody, ExpressionFrame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedDynamicDescriptorParameterAbi {
    pub parameter: TerminalDynamicDescriptorParameter,
    pub instance: MachineRegister,
    pub table: MachineRegister,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedDynamicParameterCallMechanism {
    X86MemoryIndirect {
        table: MachineRegister,
    },
    Aarch64LoadedIndirect {
        table: MachineRegister,
        target: MachineRegister,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedOperation {
    RankedU32Countdown(AssignedRankedU32Countdown),
    UnitBody(AssignedUnitBody),
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    ReturnDynamicParameterScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        parameter_abi: AssignedDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        function_call_plan: CallPlan,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
        mechanism: AssignedDynamicParameterCallMechanism,
    },
    ReturnStructuralCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        operation_result: StructuralOperationResult,
        result: StructuralResultDeclaration,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        callee_call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    ScalarReturnWithCleanup {
        scalar: Box<AssignedOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: psi_core::BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: DirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: omega_calling_conventions::CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
    },
    ExitProcessI32 {
        constant_operation: OperationId,
        psi_operation: OperationId,
        nominal_return_edge: EdgeId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: LinuxExitGroupI32Realization,
        argument: BoundaryScalarArgument,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// Assigned form of one finite short-circuit Boolean tree. Exact cleanup
    /// ownership remains attached to each leaf's terminal-Psi return edge.
    BooleanControlWithCleanup {
        control: AssignedBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<omega_target_operations::TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    ReturnStructuralParameter {
        call_plan: CallPlan,
        parameters: Vec<StructuralParameterDeclaration>,
        source: StructuralParameterDeclaration,
        result: StructuralResultDeclaration,
        shape: ValueShape,
        source_placement: ValuePlacement,
        result_placement: ValuePlacement,
        psi_edge: EdgeId,
        returned_claims: Vec<ClaimId>,
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        trivial_affine_discards: Vec<PlaceId>,
    },
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    ReturnIntegerImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    ReturnBooleanImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    ReturnIntegerParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: AssignedScalarLocation,
    },
    ReturnBooleanSharedConvergence {
        return_edges: Vec<EdgeId>,
        psi_edge: EdgeId,
        control: AssignedBooleanControl,
    },
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        frame: ExpressionFrame,
        expression: AssignedBooleanExpression,
    },
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        frame: ExpressionFrame,
        expression: AssignedIntegerExpression,
    },
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        scalar_type: IntegerType,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        scalar_type: IntegerType,
        when_true: AssignedConditionalIntegerArm,
        when_false: AssignedConditionalIntegerArm,
    },
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: AssignedScalarLocation,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition_frame: ExpressionFrame,
        condition: AssignedBooleanExpression,
        when_true: AssignedConditionalBooleanArm,
        when_false: AssignedConditionalBooleanArm,
    },
}
