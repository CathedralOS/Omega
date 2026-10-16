//! Ordered Unit-body operations, including moves, cleanup and calls.

use crate::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, ProviderExecutionBinding, TargetBoundaryResult,
    TargetIeeeFloatFmaOperand, TargetStructuralArgument, TargetStructuralHomeRequirement,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource, TargetX86ScalarFmaSettlement,
};
use abstract_operations::{AbstractResult, CompletionClaimSource};
use calling_conventions::{CallPlan, ValueShape};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, OperationId, PlaceId, ServiceId, StructuralFieldId, ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, ProviderCandidateConformance,
    StructuralArgument, StructuralOperationResult, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

/// Exact semantic origin retained beside ordinary native call transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCallOrigin {
    Authored,
    /// Exact internal deployment selection, not a provider installation.
    /// The receiving owner rejoins this record with the immutable Terminal
    /// catalog and the invocation's sealed target selection.
    TargetFamily {
        boundary: BoundaryMachineId,
        member: MachineId,
        target: String,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    InstalledProvider {
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

/// Exact completed inputs of a post-call float adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFloatResultPolicy {
    Trapping {
        raw_result: TargetUnitScalarArgumentSource,
    },
    Saturating {
        raw_result: TargetUnitScalarArgumentSource,
        eligible: TargetUnitScalarArgumentSource,
    },
}
impl TargetFloatResultPolicy {
    pub fn operands(self) -> impl Iterator<Item = TargetUnitScalarArgumentSource> {
        match self {
            Self::Trapping { raw_result } => [None, Some(raw_result)],
            Self::Saturating {
                eligible,
                raw_result,
            } => [Some(eligible), Some(raw_result)],
        }
        .into_iter()
        .flatten()
    }
    pub fn semantic_policy(self) -> terminal_psi::FloatResultPolicy {
        match self {
            Self::Trapping { raw_result } => terminal_psi::FloatResultPolicy::Trapping {
                raw_result: raw_result.source_value(),
            },
            Self::Saturating {
                eligible,
                raw_result,
            } => terminal_psi::FloatResultPolicy::Saturating {
                eligible: eligible.source_value(),
                raw_result: raw_result.source_value(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    /// One observable non-atomic scalar event on the original placed backing.
    PlacedScalarAccess {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        realization: crate::PlacedScalarAccessRealization,
        location: StructuralArgument,
        interpretation: crate::TargetPlacedInterpretation,
        operand: Option<TargetUnitScalarArgumentSource>,
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    /// A portable ordering event without an accessed location or result.
    AtomicFence {
        psi_operation: OperationId,
        ordering: terminal_psi::FenceOrdering,
    },
    /// Define an opaque selector word naming one complete immutable table.
    SelectImplementation {
        result_home: TargetUnitScalarHomeRequirement,
        table: u32,
    },
    /// Adapt the completed boundary payload without changing its provider or
    /// replaying its operands. A trapping adapter keeps its own operation site.
    FloatResultPolicy {
        psi_operation: OperationId,
        result_home: TargetUnitScalarHomeRequirement,
        format: IeeeFloatFormat,
        policy: TargetFloatResultPolicy,
    },
    /// One indivisible access at an aligned scalar field. Scalar inputs are
    /// resolved in the event's operand order; an observing result is defined
    /// by this event, never by an independent load of the same location.
    AtomicAccess {
        psi_operation: OperationId,
        location: StructuralArgument,
        field: StructuralFieldId,
        scalar_type: IntegerType,
        field_byte_offset: u32,
        event: terminal_psi::AtomicAccessEvent,
        operands: Vec<TargetUnitScalarArgumentSource>,
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    IeeeFloatCompare {
        result_home: TargetUnitScalarHomeRequirement,
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: IeeeFloatFormat,
        left: TargetUnitScalarArgumentSource,
        right: TargetUnitScalarArgumentSource,
    },
    /// The operand's stored bits read at `result_home`'s carrier of the same
    /// width (`terminal_semantics::ScalarRepresentationViewPair`). The home
    /// names the defining Terminal operation and the target carrier; the
    /// operand keeps its own carrier, so the pair is replayed, not trusted.
    ScalarRepresentationView {
        result_home: TargetUnitScalarHomeRequirement,
        operand: TargetUnitScalarArgumentSource,
    },
    /// One Terminal boundary call whose admitted settlement is a compiler
    /// float-arithmetic builtin, realized inline in the caller. `boundary`
    /// and `execution` keep the call's identity and the admitted catalog
    /// row; `arguments` retains its exact semantic arity and authored order, and
    /// `result_home` its scalar result. No frame or call exists.
    /// Complete structural classification result, with no fabricated scalar
    /// source identity. Selection owns its packed implementation temporary.
    IeeeFloatClassify {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: crate::NearestIeeeFloatArithmeticRealization,
        result_home: TargetStructuralHomeRequirement,
        argument: TargetUnitScalarArgumentSource,
    },
    NearestIeeeFloatArithmetic {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: crate::NearestIeeeFloatArithmeticRealization,
        result_home: TargetUnitScalarHomeRequirement,
        arguments: Vec<TargetUnitScalarArgumentSource>,
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
    /// One fresh observation of a primitive local or of a primitive leaf
    /// beneath a readable borrowed root. `path` is the verified Terminal
    /// projection; `indices` resolves each of its `RuntimeIndex` segments, in
    /// path order, to the operand the address scales by its array stride.
    PrimitiveScalarRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// Observe a sum's active case. `tag_byte_offset` is the path's static
    /// part; `indices` resolves each `RuntimeIndex` segment, in path order, to
    /// the operand the tag's address scales by its array stride
    /// (`items[i] is Color::Red`), exactly as a primitive read does.
    StructuralCaseMembership {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        tag_byte_offset: u32,
        case: semantic_vocabulary::StructuralCaseId,
        case_tag: u32,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// One scalar field observation beneath a readable root. `source.path`
    /// is the runtime carrier. With `case`, it ends at a sum and `field` is a
    /// payload field of that declared case: the payload overlays the sum's own
    /// place, so the case adds no path segment but selects which overlay's
    /// layout the field offset comes from. Terminal verification proved the
    /// case active; consumers rebuild the offset from declarations.
    StructuralScalarFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        case: Option<semantic_vocabulary::StructuralCaseId>,
        field: StructuralFieldId,
    },
    /// One byte copy of a subtree out of a live readable root into a fresh
    /// home. `source` names the root place and `path`/`byte_offset` the
    /// inspected bytes inside it; `result_home` is the fresh activation-local
    /// storage the copy must occupy. Two abstract operations realize as this
    /// row. A leaf copy of an `Unrestricted` subtree leaves the root in full
    /// custody: copying observes contents like a structural field read and
    /// never aliases the result back into the root. A borrowed-window move
    /// (`MoveStructuralField`) copies the vacated field under the spelled
    /// path extended by that field's segment; the old bytes stay in place
    /// because the verified restoration window forbids observing them before
    /// `StoreStructuralField` reseats the field.
    StructuralLeafCopy {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        source: PlaceId,
        path: abstract_operations::StructuralCopyPath,
        byte_offset: u32,
        /// Runtime traversals through `RuntimeIndex` segments, in path
        /// order: each resolved operand scales by its `stride` and joins
        /// `byte_offset` ahead of the copy's first load. Empty for fully
        /// static projections.
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// Reseat the field a borrowed-window move vacated: the inverse of the
    /// window's `StructuralLeafCopy`. The complete bytes of `value`, the
    /// consumed owned subtree in a local home or incoming ABI parameter, are written at
    /// `byte_offset` beneath `destination`'s referent, the resolved offset of
    /// `field` under the spelled `path`. The value is spent and the
    /// borrowed root regains full custody of the field. A root window
    /// (`field: None`, empty path) writes the whole referent at offset zero.
    StoreStructuralField {
        psi_operation: OperationId,
        destination: abstract_operations::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: PlaceId,
        byte_offset: u32,
        /// Runtime traversals through `RuntimeIndex` segments, in path
        /// order, resolved like `StructuralLeafCopy::indices` — the spelled
        /// path may end in an element selection (the window's element leaf),
        /// whose selector scales into the same `byte_offset` the static
        /// prefix resolved. Empty for fully static projections.
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// Read one inline byte under the exact current field-length observation.
    StructuralByteSequenceFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        field: StructuralFieldId,
        index: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// Observe bounded inline storage metadata, not the field's byte contents.
    StructuralByteSequenceFieldLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        field: StructuralFieldId,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// One ordered scalar definition; residence is assigned downstream, not
    /// prescribed as a stack slot by target lowering.
    ScalarDefinition {
        result_home: TargetUnitScalarHomeRequirement,
        expression: crate::TargetScalarExpression,
    },
    /// Replace bounded inline bytes from an immutable live view. The exact
    /// destination field and source-length proof survive physical projection.
    ByteSequenceStore {
        psi_operation: OperationId,
        destination: PlaceId,
        source: PlaceId,
        length: TargetUnitScalarArgumentSource,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteSequenceFieldStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: StructuralFieldId,
        source: PlaceId,
        length: TargetUnitScalarArgumentSource,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Replace one shared borrowed-view slot's two-word descriptor with an
    /// established view's: a record's view `field`, or with no field the
    /// whole destination home. No bytes move; the slot then denotes the
    /// source's backing, whose lifetime Terminal replay already proved.
    StructuralByteViewFieldStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: Option<StructuralFieldId>,
        source: PlaceId,
        indices: Vec<TargetStructuralRuntimeIndex>,
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
    /// Establish one checked element descriptor over a collection source
    /// without copying its backing elements.
    EstablishElementView {
        result: StructuralOperationResult,
        view: crate::TargetElementView,
    },
    /// Derive one checked element-range descriptor without copying its
    /// backing elements.
    ElementViewSubslice {
        result: StructuralOperationResult,
        view: crate::TargetElementView,
    },
    /// Replace one initialized byte through the original mutable descriptor.
    /// Scalar origins and the exact same-view length proof remain independent.
    ByteSequenceWrite {
        psi_operation: OperationId,
        destination: semantic_vocabulary::PlaceId,
        view: crate::TargetByteView,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Store one scalar element through the exact mutable element view.
    /// `destination` is the view place: an `&mut [T]` parameter or a local
    /// whose establishment lent the collection under `MutableBorrow`.
    /// Scalar origins and the exact same-view element-length proof remain
    /// independent.
    ElementViewWrite {
        path: Vec<terminal_psi::StructuralPathSegment>,
        psi_operation: OperationId,
        destination: terminal_psi::StructuralArgument,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    /// Copy the two checked live extents into a fresh activation-owned result.
    /// The exact source paths survive target refinement for independent replay.
    EstablishByteSequenceConcatenation {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
        capacity_obligation: semantic_vocabulary::ObligationId,
    },
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
        /// Domain memberships the literal mints onto its destination.
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
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
    /// Non-observing primitive replacement through an exact mutable or
    /// write-only root and its verified Terminal projection. `indices`
    /// resolves each `RuntimeIndex` segment of `path`, in path order, to the
    /// operand the address scales by its array stride; the segment keeps the
    /// obligation proving it inside the array.
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        destination_type: StructuralTypeDeclaration,
        destination_storage: crate::TargetStructuralArgumentSource,
        source: TargetUnitWriteOnlyPrimitiveStoreSource,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    /// One verifier-approved fixed-width integer write into an exact field of
    /// a borrowed structural parameter. Entry parameters retain their ABI
    /// placement; state parameters retain their exact block pointer slot.
    /// Semantic location and physical offset stay together so emission can replay
    /// the join independently. `field_byte_offset` is the static part of the
    /// carrier path plus the field; `indices` resolves each runtime element
    /// of the carrier, in path order, as for a primitive store.
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        destination_storage: crate::TargetStructuralArgumentSource,
        field_byte_offset: u32,
        source: TargetUnitScalarArgumentSource,
        indices: Vec<TargetStructuralRuntimeIndex>,
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
    /// Establish one empty-record affine local. It occupies no bytes, so the
    /// row realizes no storage: it is the owner's establishment identity,
    /// which a whole owned call argument names as its `StructuralHome`
    /// producer.
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    /// Establish a whole zero-constructible structural value in its exact
    /// aggregate home: every scalar leaf begins zero with no element roster.
    EstablishZeroedStructural {
        psi_operation: OperationId,
        result_home: crate::target_operations::TargetStructuralHomeRequirement,
    },
    /// Establish a structural local's exact aggregate home for memberwise
    /// establishment: subsequent field stores write the members.
    EstablishStructuralLocal {
        psi_operation: OperationId,
        result_home: crate::target_operations::TargetStructuralHomeRequirement,
    },
    /// A complete record established in its exact operation-result home.
    EstablishRecord {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    /// Establish a verified reference in its own descriptor home. Aggregate
    /// loans retain the backing address; scalar loans additionally retain the
    /// storage codec. Custody identities constrain authority, while `source`
    /// forms the actual address once without copying referent contents.
    EstablishPlacedReference {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        source: StructuralArgument,
        correspondence: terminal_psi::PlacedRamCorrespondence,
    },
    EstablishReference {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        source: StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    },
    /// Close the loan on the carrier at `source`. Referent storage is
    /// unaffected; this row carries custody metadata only.
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    /// One direct call, independent of its scalar/structural argument mix.
    /// Scalar arguments occupy the prefix of the complete ABI plan;
    /// structural arguments retain the remaining placements. Result storage,
    /// returned references and ownership are explicit result custody, not
    /// separate call operations. Receiving checks reconstruct the full plan
    /// and custody from the declaration and pre-call state.
    /// An invocation owned by one normal-return cleanup action, not a source operation.
    CleanupCall {
        owner: crate::CallSiteOwner,
        callee: MachineId,
        call_plan: CallPlan,
        arguments: Vec<TargetStructuralArgument>,
    },
    Call {
        origin: NativeCallOrigin,
        psi_operation: OperationId,
        target: crate::NativeCallTarget,
        call_plan: CallPlan,
        result: crate::TargetCallResult,
        scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<TargetStructuralArgument>,
        /// Complete exact formal-demand bindings in argument order. Pointer
        /// transport remains owned by the referenced ordinary argument row.
        placed_bindings: Vec<crate::TargetPlacedCallBinding>,
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
    /// Bind complete elements into the declared fixed-array home.
    EstablishStructuralArray {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        elements: Vec<terminal_psi::StructuralArgument>,
    },
    /// Establish a fresh sum in its exact durable aggregate home.
    EstablishCase {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    /// One evaluated foreign leaf. Native settlement rejoins
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
        /// Completion establishes this exact scalar or structural home.
        /// ABI indirection belongs to the evaluated plan, not semantic access.
        result: TargetBoundaryResult,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    RegisterAccess {
        psi_operation: OperationId,
        service: ServiceId,
        event: terminal_psi::RegisterAccessEvent<TargetUnitScalarArgumentSource>,
        /// Reads define this home; writes produce no scalar.
        result_home: Option<TargetUnitScalarHomeRequirement>,
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

/// One `RuntimeIndex` traversal of a structural projection (a leaf copy, a
/// primitive read or store, or a scalar field store's carrier): the resolved
/// operand scales by the array's element `stride` before joining the static
/// byte offset, in path order. The path segment itself keeps the obligation
/// proving the operand inside the array; this row never restates a bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralRuntimeIndex {
    pub operand: TargetUnitScalarArgumentSource,
    pub stride: u32,
}

impl TargetUnitOperation {
    /// Exact produced value, independent of whether its storage was allocated
    /// eagerly or its descriptor payload is materialized by a later consumer.
    /// This query carries no backing permission or lifetime authority.
    pub fn structural_result(&self) -> Option<(OperationId, &StructuralOperationResult)> {
        match self {
            Self::IeeeFloatClassify {
                psi_operation,
                result_home,
                ..
            } => result_home
                .operation_result()
                .map(|(_, result)| (*psi_operation, result)),
            Self::ByteSequenceSubslice {
                result,
                view:
                    crate::TargetByteView::Subslice {
                        psi_operation,
                        place,
                        ..
                    },
            }
            | Self::ElementViewSubslice {
                result,
                view:
                    crate::TargetElementView::Subslice {
                        psi_operation,
                        place,
                        ..
                    },
            }
            | Self::EstablishElementView {
                result,
                view:
                    crate::TargetElementView::Established {
                        psi_operation,
                        place,
                        ..
                    },
            } if *place == result.place => Some((*psi_operation, result)),
            Self::EstablishPrimitiveLocal {
                psi_operation,
                result,
                ..
            }
            | Self::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                ..
            }
            | Self::Call {
                psi_operation,
                result: crate::TargetCallResult::Structural { result, .. },
                ..
            } => Some((*psi_operation, result)),
            _ => {
                let (producer, home) = self.structural_result_home()?;
                match &home.origin {
                    crate::TargetStructuralHomeOrigin::OperationResult { operation, result }
                        if *operation == producer =>
                    {
                        Some((producer, result))
                    }
                    _ => None,
                }
            }
        }
    }

    /// Required storage for an operation's structural result. The operation
    /// identity is retained beside the home so receiving replay can compare
    /// the home origin with both its producer and the abstract declaration.
    /// Borrowed descriptors and scalar homes are separate storage contracts.
    /// Foreign and builtin boundary calls establish the same owned result
    /// home. Consumers must retain that producer identity through ordinary
    /// returns and control-flow joins regardless of the call's entry mechanism.
    pub fn structural_result_home(
        &self,
    ) -> Option<(OperationId, &TargetStructuralHomeRequirement)> {
        match self {
            Self::IeeeFloatClassify {
                psi_operation,
                result_home,
                ..
            }
            | Self::StructuralLeafCopy {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishZeroedStructural {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishStructuralLocal {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishRecord {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishReference {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishPlacedReference {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishScalarArray {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishStructuralArray {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishCase {
                psi_operation,
                result_home,
                ..
            }
            | Self::Call {
                psi_operation,
                result:
                    crate::TargetCallResult::Structural {
                        result_home: Some(result_home),
                        ..
                    },
                ..
            }
            | Self::BoundarySettlement {
                psi_operation,
                result: TargetBoundaryResult::Structural(result_home),
                ..
            }
            | Self::NormalizedForeignCall {
                psi_operation,
                result: TargetBoundaryResult::Structural(result_home),
                ..
            } => Some((*psi_operation, result_home)),
            _ => None,
        }
    }
}
