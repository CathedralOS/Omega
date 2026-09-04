//! The target operation vocabulary consumed by physical assignment.

use crate::{
    BoundaryExecutionBinding, BoundaryScalarArgument, DirectPortReadU8Realization,
    FixedIntegerScalarAbiValue, LinuxExitGroupI32Realization, ScalarParameterLocation,
    TargetBooleanControl, TargetBooleanExpression, TargetConditionalBooleanArm,
    TargetConditionalIntegerArm, TargetDynamicDescriptorParameterAbi, TargetIntegerExpression,
    TargetRankedU32Countdown, TargetScalarStructuralFieldStore, TargetStructuralArgument,
    TargetStructuralParameter, TargetUnitBody,
};
use omega_abstract_operations::{AbstractDynamicDescriptorArgument, CompletionClaimSource};
use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultClaimTransfer, StructuralResultDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalDynamicRequirement,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOperation {
    /// Exact admitted structural-Unit `u32` countdown. This carrier preserves
    /// the verifier/fuel custody and cyclic graph identity directly; it is not
    /// an acyclic conditional-control tree.
    RankedU32Countdown(TargetRankedU32Countdown),
    /// Ordered straight-line Unit/effect body. Scalar expression trees remain
    /// a separate representation so value-less execution cannot fabricate a
    /// pseudo-result.
    UnitBody(TargetUnitBody),
    /// One bounded whole-root structural call whose scalar ABI result is
    /// returned directly. Keeping this carrier distinct from `UnitBody`
    /// prevents a result-bearing call from being erased into a value-less
    /// operation stream while still sharing the aggregate-copy ABI lane.
    ReturnStructuralScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Return the result of one call through an existential descriptor passed
    /// into the current function. `function_call_plan` owns the helper's
    /// `{data, table}` entry ABI; `dispatch_call_plan` owns the erased adapter
    /// ABI reached through the selected table slot. The slot never names the
    /// concrete-layout realization directly.
    ReturnDynamicParameterScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        function_call_plan: CallPlan,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
    },
    /// Return one scalar result obtained by forwarding the current function's
    /// complete existential descriptor parameter to another helper. This is a
    /// direct helper call, not a table dispatch: `argument` retains the exact
    /// caller-parameter/callee-parameter interface join while the two call
    /// plans retain both sides of the native ABI handoff.
    ReturnForwardedDynamicParameterScalarCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        argument: AbstractDynamicDescriptorArgument,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        function_call_plan: CallPlan,
        callee_call_plan: CallPlan,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Forward the current function's complete existential descriptor
    /// parameter to another Unit helper, then return Unit. Both sides retain
    /// the exact result-less two-word ABI; no scalar result carrier exists.
    ForwardDynamicParameterUnitCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        callee: MachineId,
        argument: AbstractDynamicDescriptorArgument,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        function_call_plan: CallPlan,
        callee_call_plan: CallPlan,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one Unit-result requirement through an existential descriptor
    /// parameter, then return Unit. This is a function-level carrier because
    /// the descriptor pair is the helper's complete incoming ABI.
    DynamicParameterUnitCall {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: TerminalDynamicRequirement,
        function_call_plan: CallPlan,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
    },
    /// One exact whole-root structural call whose direct ABI result is returned
    /// unchanged by the caller.
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
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        returned_claims: Vec<ClaimId>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// A scalar return plus the exact structural cleanup frontier that runs
    /// after result materialization and before native return teardown.
    ScalarReturnWithCleanup {
        scalar: Box<TargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        psi_edge: EdgeId,
    },
    /// Execute one, two, or three exact ordered direct mutable-self Boolean or
    /// fixed-integer literal stores before the existing direct
    /// structural-field scalar return. The wrapper owns the effect/return
    /// sequencing without turning scalar functions into Unit operation
    /// streams.
    ScalarReturnAfterStructuralScalarFieldStores {
        stores: Vec<TargetScalarStructuralFieldStore>,
        scalar: Box<TargetOperation>,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
    },
    /// One direct x86 provider realization for a verified bodyless boundary
    /// returning an unsigned byte. Structural arguments and receipts are
    /// semantic custody metadata; the selected port read produces the ABI
    /// scalar result.
    ReturnBoundaryPortReadU8 {
        psi_edge: EdgeId,
        psi_operation: OperationId,
        source_value: ValueId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: DirectPortReadU8Realization,
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
    },
    /// One verified `exit_process(i32)` call realized directly by Linux
    /// `exit_group`. `nominal_return_edge` remains zero-byte provenance: if
    /// the nominally nonreturning syscall returns, the emitted code traps
    /// before that semantic tail can execute.
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
    /// One finite short-circuit Boolean tree whose value-return leaves all
    /// execute the same complete structural cleanup stream. Each leaf retains
    /// its own terminal-Psi return edge in `control`; there is deliberately no
    /// synthetic shared cleanup edge.
    BooleanControlWithCleanup {
        control: TargetBooleanControl,
        structural_types: Vec<StructuralTypeDeclaration>,
        call_plan: CallPlan,
        structural_parameters: Vec<TargetStructuralParameter>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Return one exact whole-root structural parameter through the selected
    /// native ABI while retaining its zero-runtime custody metadata.
    ReturnStructuralParameter {
        call_plan: CallPlan,
        /// Ordered fixed-integer ABI prefix. The established structural-only
        /// return lane retains an empty row.
        scalar_parameters: Vec<FixedIntegerScalarAbiValue>,
        /// Complete ordered structural input signature used to derive the ABI
        /// plan. Cleanup-only parameters remain semantic inputs even though
        /// disposing them emits no instruction.
        parameters: Vec<StructuralParameterDeclaration>,
        source: StructuralParameterDeclaration,
        result: StructuralResultDeclaration,
        shape: ValueShape,
        source_placement: ValuePlacement,
        result_placement: ValuePlacement,
        psi_edge: EdgeId,
        returned_claims: Vec<ClaimId>,
        /// Typed no-ABI local declarations retained as zero-code metadata.
        trivial_affine_locals: Vec<(
            OperationId,
            StructuralPlaceDeclaration,
            StructuralTypeDeclaration,
        )>,
        /// Exact verifier-owned reverse-declaration cleanup order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// End the execution domain at one verified terminal-Psi crash edge.
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    /// Return one compile-time integer through the target's ordinary scalar
    /// function-result convention. Register and instruction encoding are
    /// chosen by machine emission.
    ReturnIntegerImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    /// Return a compile-time Boolean as the target ABI's canonical zero/one
    /// scalar result.
    ReturnBooleanImmediate {
        psi_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    },
    /// Return one caller-supplied integer from its selected native ABI
    /// location. The source value remains the terminal-Psi parameter identity.
    ReturnIntegerParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return one caller-supplied Boolean from its selected native ABI
    /// location.
    ReturnBooleanParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// Return the logical negation of one caller-supplied canonical Boolean.
    ReturnBooleanNotParameter {
        psi_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    /// One verified terminal-Psi Boolean convergence tree whose value leaves
    /// join one physical return/cleanup tail.
    ReturnBooleanSharedConvergence {
        /// Exact true-before-false DFS roster of source return edges reaching
        /// the shared native tail. A source-level convergence block therefore
        /// contributes one edge, while distinct uniform return leaves retain
        /// every edge without duplicating the physical cleanup.
        return_edges: Vec<EdgeId>,
        psi_edge: EdgeId,
        control: TargetBooleanControl,
    },
    /// Return a runtime Boolean expression lowered from terminal-Psi logical
    /// operations. Every node produces a canonical zero/one Boolean.
    ReturnBooleanExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: TargetBooleanExpression,
    },
    /// Return a runtime integer expression lowered from exact-width terminal
    /// Psi operations. Every node has the enclosing result's integer type.
    ReturnIntegerExpression {
        psi_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
    /// Execute an acyclic conditional-control tree whose leaves return integer
    /// expressions. Every structural and return edge remains explicit.
    ReturnIntegerConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute integer-returning control whose root condition is a recursive
    /// runtime Boolean expression rather than one direct ABI parameter.
    ReturnIntegerExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        scalar_type: IntegerType,
        when_true: TargetConditionalIntegerArm,
        when_false: TargetConditionalIntegerArm,
    },
    /// Execute an acyclic conditional-control tree whose leaves return
    /// canonical Boolean values.
    ReturnBooleanConditionalControl {
        condition_source: ValueId,
        condition_parameter_index: usize,
        condition_location: ScalarParameterLocation,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
    /// Execute Boolean control whose root condition is a recursive runtime
    /// Boolean expression rather than one direct ABI parameter.
    ReturnBooleanExpressionConditionalControl {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetConditionalBooleanArm,
        when_false: TargetConditionalBooleanArm,
    },
}
