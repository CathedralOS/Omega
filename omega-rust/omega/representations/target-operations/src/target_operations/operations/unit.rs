//! Ordered Unit-body operations, including moves, cleanup and calls.

use crate::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, ProviderExecutionBinding, TargetBoundaryResult,
    TargetDynamicDescriptorArgument, TargetDynamicDescriptorParameterAbi,
    TargetIeeeFloatFmaOperand, TargetStructuralArgument, TargetStructuralHomeRequirement,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource, TargetX86ScalarFmaSettlement,
};
use abstract_operations::{
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch, AbstractResult,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch, CompletionClaimSource,
};
use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, OperationId, PlaceId, ServiceId, StructuralFieldId, ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, ProviderCandidateConformance,
    StructuralArgument, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    TerminalAffineCleanupAction,
};

/// Exact semantic origin retained beside ordinary native call transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCallOrigin {
    Authored,
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    IeeeFloatCompare {
        result_home: TargetUnitScalarHomeRequirement,
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: IeeeFloatFormat,
        left: TargetUnitScalarArgumentSource,
        right: TargetUnitScalarArgumentSource,
    },
    EstablishPrimitiveLocal {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        value: AbstractResult,
        shape: ValueShape,
    },
    PrimitiveLocalStore {
        psi_operation: OperationId,
        destination: PlaceId,
        value: AbstractResult,
    },
    PrimitiveScalarRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    },
    StructuralCaseMembership {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        tag_byte_offset: u32,
        case: semantic_vocabulary::StructuralCaseId,
        case_tag: u32,
    },
    StructuralScalarFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        field: StructuralFieldId,
    },
    /// Observe bounded inline storage metadata, not the field's byte contents.
    StructuralByteSequenceFieldLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        field: StructuralFieldId,
    },
    /// One ordered scalar definition; residence is assigned downstream, not
    /// prescribed as a stack slot by target lowering.
    ScalarDefinition {
        result_home: TargetUnitScalarHomeRequirement,
        expression: crate::TargetScalarExpression,
    },
    /// Replace bounded inline bytes from an immutable live view. The exact
    /// destination field and source-length proof survive physical projection.
    StructuralByteSequenceFieldStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: StructuralFieldId,
        source: PlaceId,
        length: TargetUnitScalarArgumentSource,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Mutate one inline byte under the exact current field-length observation.
    StructuralByteSequenceFieldByteStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: StructuralFieldId,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Establish one checked descriptor without copying its immutable backing bytes.
    ByteSequenceSubslice {
        result: StructuralOperationResult,
        view: crate::TargetByteView,
    },
    /// Replace one initialized byte through the original mutable descriptor.
    /// Scalar origins and the exact same-view length proof remain independent.
    ByteSequenceWrite {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        view: crate::TargetByteView,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
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
    /// Non-observing primitive replacement through an exact mutable or write-only
    /// root and declaration-identified static projection.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
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
    /// A complete record established in its exact operation-result home.
    EstablishRecord {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    /// Establish one verified reference carrier. `result_home` is the
    /// carrier's canonical zero-byte metadata home — a custody identity, not
    /// storage; the referent is located only through `source`, never through
    /// pointer bits.
    EstablishReference {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        source: StructuralArgument,
    },
    /// Close the loan on the carrier at `source`. Referent storage is
    /// unaffected; this row carries custody metadata only.
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
    },
    /// One direct call, independent of its scalar/structural argument mix.
    /// Scalar arguments occupy the prefix of the complete ABI plan;
    /// structural arguments retain the remaining placements. Result storage,
    /// returned references and ownership are explicit result custody, not
    /// separate call operations. Receiving checks reconstruct the full plan
    /// and custody from the declaration and pre-call state.
    Call {
        origin: NativeCallOrigin,
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        result: crate::TargetCallResult,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Establish all row-major primitive leaves in one exact aggregate home.
    EstablishScalarArray {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        elements: Vec<ValueId>,
    },
    /// Establish a fresh sum in its exact durable aggregate home.
    EstablishScalarCase {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<terminal_psi::ScalarCaseField>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
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
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One indirect scalar call through a requirement slot of this function's
    /// own borrowed descriptor parameter. `parameter_abi` binds the incoming
    /// `{instance, table}` pair to the graph's call plan, `requirement` is the
    /// closed interface row `dispatch_call_plan` invokes, and
    /// `table_slot_byte_offset` is the byte offset of its entry in the
    /// incoming table. `dispatch_call_plan` retains the erased one-pointer
    /// adapter ABI — policy, clobbers, stack contract — while the semantic
    /// dispatch row never names a concrete realization.
    DynamicParameterScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: terminal_psi::TerminalDynamicRequirement,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
        result_home: TargetUnitScalarHomeRequirement,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One indirect Unit call through a requirement slot of this function's
    /// own borrowed descriptor parameter. Descriptor and table custody are
    /// identical to the scalar form, while the signature and operation carry
    /// no result or result-home carrier.
    DynamicParameterUnitCall {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractParameterDynamicDispatch,
        parameter_abi: TargetDynamicDescriptorParameterAbi,
        requirement: terminal_psi::TerminalDynamicRequirement,
        dispatch_call_plan: CallPlan,
        table_slot_byte_offset: u32,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One Unit-returning evaluated import leaf. Native settlement rejoins
    /// this exact carrier; lowering never accepts locator or calling-plan
    /// strings from the call site. The bounded scalar lane admits fixed-width
    /// integer constants and exact preceding scalar-result homes in the
    /// evaluated plan's register placements. The bounded structural lane
    /// admits source-rooted borrowed flat-record arguments whose projected
    /// referent pointer occupies one exact plan placement.
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
        /// Source-rooted structural arguments in evaluated boundary-plan
        /// order. Lowering admits them only while the scalar lane is empty:
        /// the Terminal declaration erases the authored interleave of scalar
        /// and structural formals, so a mixed signature cannot rejoin its
        /// exact plan positions without a wider coordinate.
        structural_arguments: Vec<NormalizedForeignStructuralArgument>,
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
    /// An adjacent fallthrough edge retains cleanup at the actual continuation.
    Continue {
        psi_edge: EdgeId,
        source_block: BlockId,
        target_block: BlockId,
        bindings: Vec<abstract_operations::ValueBinding>,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
}
