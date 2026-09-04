//! Ordered Unit-body operations, including moves, cleanup and calls.

use crate::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    ProviderExecutionBinding, TargetBoundaryResult, TargetDynamicDescriptorArgument,
    TargetIeeeFloatFmaOperand, TargetStructuralArgument, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitConditionalSuccessor, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitStructuralCaseSuccessor, TargetUnitWriteOnlyPrimitiveStoreSource,
    TargetX86ScalarFmaSettlement, UnitScalarAbiValue,
};
use omega_abstract_operations::{
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, CompletionClaimSource,
};
use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use psi_core::{
    BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, OperationId, ServiceId, StructuralFieldId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, ProviderCandidateConformance,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitBody {
    /// Canonical verifier-owned structural declaration closure used to replay
    /// projected-layout and partial-cleanup partitions at artifact boundaries.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    /// Ordered scalar parameters and their exact incoming ABI placements.
    /// The bounded lane currently admits fixed integers and canonical
    /// Booleans; both remain distinct from zero-payload structural custody.
    pub scalar_parameters: Vec<UnitScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<TargetUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    /// One verifier-approved non-observing immediate replacement through an
    /// exact whole-root mutable or write-only primitive parameter.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        destination_type: StructuralTypeDeclaration,
        destination_placement: ValuePlacement,
        source: TargetUnitWriteOnlyPrimitiveStoreSource,
    },
    /// One verifier-approved fixed-width integer write into an exact field of
    /// a staged attached-Unit structural parameter (receiver or ordinary
    /// parameter). Semantic location and
    /// physical offset remain together so assignment and emission can replay
    /// the join independently.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        destination_placement: ValuePlacement,
        field_byte_offset: u32,
        source: TargetUnitScalarArgumentSource,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    /// One nearest-even scalar FMA whose first bounded Unit lane consumes
    /// three exact preceding IEEE constants. Physical XMM assignment remains
    /// the next stage's responsibility.
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: TargetIeeeFloatFmaOperand,
        right: TargetIeeeFloatFmaOperand,
        addend: TargetIeeeFloatFmaOperand,
        settlement: TargetX86ScalarFmaSettlement,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    /// One complete owned-affine one-i64-field record retained until its
    /// exact owned use. The empty source placement used by that call is a
    /// checked virtual aggregate, not a physical stack location.
    EstablishAffineScalarRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        field: StructuralFieldId,
        value: IntegerValue,
        shape: ValueShape,
    },
    /// One direct Unit-result call. Scalar arguments occupy the prefix of the
    /// complete ABI plan; structural arguments retain the remaining placements.
    Call {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One service-free, in-module fixed-width integer call inside an attached
    /// Unit body. The complete ABI plan identifies the transient result and
    /// argument placements; `result_home` separately requires downstream
    /// assignment to preserve the result for later Unit operations.
    ScalarCall {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        arguments: Vec<TargetUnitScalarCallArgument>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One projected structural call whose fixed-width scalar result is
    /// intentionally discarded by this bounded attached-Unit lane. The
    /// complete result and call plan remain explicit even though no durable
    /// result home is allocated.
    StructuralScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One bounded mixed-input call whose whole affine structural result is
    /// retained only until the immediately following Unit return discards it.
    /// The result has no invented scalar home: its ABI placement and semantic
    /// custody remain explicit until physical replay.
    StructuralResultCall {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        callee_result: StructuralResultDeclaration,
        call_plan: CallPlan,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One direct scalar-result call whose authored parameter roster contains
    /// one or more existential descriptors. This role remains distinct from
    /// an ordinary structural call because each descriptor expands to an
    /// ordered `{data, table}` ABI pair and requires adapter-table custody.
    StructuralScalarCallWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        /// Durable home required when later Unit operations consume the
        /// forwarded call's fixed-width scalar result.
        result_home: TargetUnitScalarHomeRequirement,
        structural_arguments: Vec<TargetStructuralArgument>,
        dynamic_arguments: Vec<TargetDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One direct Unit-result call whose authored parameter roster contains
    /// existential descriptors. The descriptor ABI is identical to the
    /// scalar form, but no semantic result or durable scalar home exists.
    StructuralUnitCallWithDynamicArguments {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        structural_arguments: Vec<TargetStructuralArgument>,
        dynamic_arguments: Vec<TargetDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Materialize one selected descriptor into a durable two-word local. The
    /// structural argument retains the exact instance projection and the
    /// destination placement shared with its later indirect call.
    StoreDynamicDescriptor {
        psi_operation: OperationId,
        stored: AbstractStoredDynamicDescriptor,
        source_argument: TargetStructuralArgument,
    },
    /// Reload and invoke a descriptor established by the unique preceding
    /// store with the same target-neutral custody.
    StoredDynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractStoredDynamicDispatch,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        source_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One exact rebound dynamic invocation. Both source versions are lowered
    /// against the selected realization ABI, but only `rebound_argument`
    /// supplies the runtime instance. The retained dispatch row is private
    /// table content; later assignment/emission must call through that table.
    DynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        result_home: TargetUnitScalarHomeRequirement,
        initial_argument: TargetStructuralArgument,
        rebound_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One exact rebound Unit invocation. Descriptor and table custody are
    /// identical to the scalar form, while the native call signature and
    /// operation deliberately contain no result or result-home carrier.
    DynamicUnitCall {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        initial_argument: TargetStructuralArgument,
        rebound_argument: TargetStructuralArgument,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Inspect one exact structural boundary-result home and dispatch to the
    /// physically laid-out arm matching its canonical signed-i32 case tag.
    StructuralCase {
        source: TargetStructuralHomeRequirement,
        cases: Vec<TargetUnitStructuralCaseSuccessor>,
    },
    /// One bounded equality decision after a durable Unit scalar result.
    /// The true arm is laid out first and both arms must end in admitted
    /// nonreturning boundary settlements. This is deliberately not a general
    /// Unit CFG carrier.
    ConditionalIntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: TargetUnitScalarArgumentSource,
        right: TargetUnitScalarArgumentSource,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// One bounded truth decision over an exact durable Boolean result home.
    ConditionalBoolean {
        condition: TargetUnitScalarHomeRequirement,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// Branch on one canonical Boolean supplied directly by the caller.
    /// Keeping the complete incoming placement here lets assignment prove
    /// that control consumes the declared Unit ABI parameter rather than a
    /// coincidentally equal transient home.
    ConditionalBooleanParameter {
        condition: UnitScalarAbiValue,
        when_true: TargetUnitConditionalSuccessor,
        when_false: TargetUnitConditionalSuccessor,
    },
    /// Zero-code ordinal marker for the source conditional operation that
    /// consumes the preceding equality. The true edge owns the fallthrough
    /// site; the paired false edge remains in the equality carrier.
    ConditionalDispatch { fallthrough_edge: EdgeId },
    /// Zero-code semantic tail after an admitted nonreturning boundary. The
    /// edge remains independently attributable even when another conditional
    /// arm follows physically in the same function.
    NonreturningTail { psi_edge: EdgeId },
    /// One bodyless boundary occurrence projected through an opaque admitted
    /// installation into an exact checked Unit provider call. The original
    /// receipt evidence remains alongside its call-transfer interpretation so
    /// later legalization can replay the join without treating it as a
    /// source-authored `CallUnit`.
    InstalledProviderCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        /// Exact native plan of the selected provider candidate. The plan is
        /// retained even for the historical zero-scalar structural lane.
        call_plan: CallPlan,
        /// Ordered fixed-integer arguments bound to `call_plan.parameters`.
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        source_arguments: Vec<StructuralArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    /// One Unit-returning evaluated import leaf. Native settlement rejoins
    /// this exact carrier; lowering never accepts locator or calling-plan
    /// strings from the call site. The bounded scalar lane admits fixed-width
    /// integer constants and exact preceding scalar-result homes in the
    /// evaluated plan's register placements.
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
        /// Optional fixed-integer result retained in the attached Unit frame.
        /// The evaluated plan must place the complete 8/16/32/64-bit signed or
        /// unsigned value in one register; later calls may consume this home.
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    BoundarySettlement {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        result: TargetBoundaryResult,
        execution: BoundaryExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
        /// Returning compiler-builtin scalar inputs retain the same exact
        /// source and ABI-placement custody as evaluated native calls.
        runtime_scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<StructuralArgument>,
        byte_sequence_arguments: Vec<BoundaryByteSequenceArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
}
