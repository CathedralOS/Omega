/*
We define the executable `AbstractOperation` vocabulary—including structural
establishments, moves, restoration windows, leaf copies, byte and element view
accesses, atomic memory events, scalar arithmetic, calls, and control transfers—
along with structural result metadata and exhaustiveness-preserving pattern
macros.

Every operation that produces a structural result classifies its carrier role
via `AbstractOperation::structural_result_metadata` (`OwnedValue`,
`PrimitiveLocalStorage`, or `ViewRecipe`) so downstream stages share one
authoritative table instead of guessing storage custody from variant names.
Every runtime-selected path element (`RuntimeIndex { index, obligation }`) is
collected in operand order by `AbstractOperation::runtime_indices` so optimizer
rewrites preserve the exact selector values named by discharged bounds
certificates.
*/

use crate::{
    AbstractAtomicEvent, AbstractBoundaryResult, AbstractCallTarget, AbstractResult,
    AbstractStructuralBinding, AbstractStructuralCallResult, AbstractStructuralCaseSuccessor,
    AbstractSuccessor, AtomicResidencyEdge, CompletionClaimSource, IntegerArithmetic,
    IntegerArithmeticOperator, ValueBinding,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId,
    ValueId,
};
use terminal_psi::{
    ClaimTransfer, CompletionReceipt, CrashCause, CrashRouteBucket, StructuralArgument,
    StructuralOperationResult as OperationResult, StructuralPathSegment,
    StructuralPlaceDeclaration as PlaceDeclaration, StructuralResultClaimBinding,
    StructuralTypeDeclaration, TerminalAffineCleanupAction as CleanupAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralStorageDeclaration {
    pub place: PlaceId,
    pub structural_type: semantic_vocabulary::StructuralTypeId,
    pub access: terminal_psi::StructuralAccess,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralResultRole {
    OwnedValue, // Completed value owns its carrier; contained loans remain separate.
    PrimitiveLocalStorage, // Activation-local primitive storage supplies loans, never owned escape.
    ViewRecipe, // View retains a checked source recipe, not initialized descriptor words.
}

#[derive(Debug, Clone, Copy)]
pub struct StructuralResultMetadata<'operation> {
    pub operation: OperationId,
    pub result: &'operation OperationResult,
    pub role: StructuralResultRole,
}

#[derive(Clone, Copy)]
pub struct ModuleInvocation<'a> {
    pub site: semantic_vocabulary::InvocationSite,
    pub callee: MachineId,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub claim_transfers: &'a [ClaimTransfer],
    pub requirement_obligations: &'a [semantic_vocabulary::ObligationId],
    pub crash_continuations: &'a [CrashRouteBucket],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperation {
    EstablishActivationClaim {
        psi_operation: OperationId,
        result: OperationResult,
        bound: u64,
        committed: ValueId,
        backing_type: semantic_vocabulary::StructuralTypeId,
        established_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    ReleaseActivationClaim {
        psi_operation: OperationId,
        source: StructuralArgument,
        claim: ClaimId,
    },
    SplitActivationClaim {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        claim: ClaimId,
        bound_at: u64,
        committed_at: ValueId,
        covered_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    MergeActivationClaim {
        psi_operation: OperationId,
        result: OperationResult,
        lower: StructuralArgument,
        upper: StructuralArgument,
        lower_claim: ClaimId,
        upper_claim: ClaimId,
        covered_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    EstablishNativeCodeLayout {
        psi_operation: OperationId,
        result: OperationResult,
        machine: semantic_vocabulary::MachineId,
    },
    ActivateTaskRegister {
        psi_operation: OperationId,
        service: ServiceId,
        result: AbstractResult,
        entries: u16,
        gdt: StructuralArgument,
        task_state: StructuralArgument,
        selector: ValueId,
        scratch: StructuralArgument,
    },
    ReloadSegmentDescriptors {
        psi_operation: OperationId,
        service: ServiceId,
        result: AbstractResult,
        entries: u16,
        kind: terminal_psi::SegmentReload,
        gdt: StructuralArgument,
        selector: ValueId,
        scratch: StructuralArgument,
    },
    ReadSegmentSelector {
        psi_operation: OperationId,
        service: ServiceId,
        register: terminal_psi::SegmentRegister,
        result: AbstractResult,
    },
    ReadTaskRegister {
        psi_operation: OperationId,
        service: ServiceId,
        result: AbstractResult,
    },
    GlobalDescriptorTable {
        psi_operation: OperationId,
        service: ServiceId,
        operation: terminal_psi::GlobalDescriptorTableOperation,
        source: StructuralArgument,
    },
    StorageAddress {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    EstablishScalarArray {
        psi_operation: OperationId,
        result: OperationResult,
        elements: Vec<ValueId>,
    },
    EstablishStructuralArray {
        psi_operation: OperationId,
        result: OperationResult,
        elements: Vec<StructuralArgument>,
    },
    EstablishPrimitiveLocal {
        psi_operation: OperationId,
        result: OperationResult,
        value: AbstractResult,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
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
        path: Vec<StructuralPathSegment>,
    },
    StructuralCaseMembership {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        case: StructuralCaseId,
    },
    ByteSequenceSubslice {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequencePredicate {
        psi_operation: OperationId,
        result: AbstractResult,
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
    },
    ByteSequenceEqual {
        psi_operation: OperationId,
        result: AbstractResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
    },
    ByteSequenceWrite {
        psi_operation: OperationId,
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteSequenceFieldByteStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceStore {
        psi_operation: OperationId,
        destination: PlaceId,
        source: PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteSequenceFieldStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        source: PlaceId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteViewFieldStore {
        psi_operation: OperationId,
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        source: PlaceId,
        predicate_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    StructuralByteSequenceFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    StructuralByteSequenceFieldLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    ByteSequenceLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
    },
    EstablishElementView {
        psi_operation: OperationId,
        result: OperationResult,
        destination: PlaceId,
        source: StructuralArgument,
        element: semantic_vocabulary::StructuralTypeId,
        region: terminal_psi::ElementViewRegion,
    },
    ElementViewLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
    },
    ElementViewRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    ElementViewWrite {
        path: Vec<terminal_psi::StructuralPathSegment>,
        psi_operation: OperationId,
        destination: StructuralArgument,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ElementViewSubslice {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: super::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        value: AbstractResult,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: super::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        value: AbstractResult,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    MoveStructuralField {
        psi_operation: OperationId,
        result: OperationResult,
        source: super::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
    },
    MoveStructuralValue {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
    },
    TransformContent {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        parents: Vec<StructuralArgument>,
        derivations: Vec<terminal_psi::ContentTransformationDerivation>,
    },
    StructuralLeafCopy {
        psi_operation: OperationId,
        result: OperationResult,
        source: PlaceId,
        path: super::StructuralCopyPath,
    },
    StoreStructuralField {
        disposition: terminal_psi::StructuralStoreDisposition,
        psi_operation: OperationId,
        destination: super::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<semantic_vocabulary::StructuralFieldId>,
        value: StructuralArgument,
    },
    AtomicEvent {
        psi_operation: OperationId,
        event: AbstractAtomicEvent,
        resident_obligation: Option<semantic_vocabulary::ObligationId>, // Distinct atomic resident establishment retained through refinements.
        reads_from: Option<AtomicResidencyEdge>,
        modification_after: Option<AtomicResidencyEdge>,
    },
    EstablishCase {
        psi_operation: OperationId,
        result: OperationResult,
        result_case: StructuralCaseId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    EstablishByteSequenceConcatenation {
        psi_operation: OperationId,
        result: OperationResult,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
        capacity_obligation: semantic_vocabulary::ObligationId,
    },
    EstablishByteSequenceLiteral {
        psi_operation: OperationId,
        place: PlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
        bytes: Vec<u8>,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>, // Domain memberships the verified literal mints onto its destination.
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: PlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    EstablishZeroedStructural {
        psi_operation: OperationId,
        result: OperationResult,
    },
    EstablishStructuralLocal {
        psi_operation: OperationId,
        result: OperationResult,
    },
    EstablishRecord {
        psi_operation: OperationId,
        result: OperationResult,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    CallStructural {
        psi_operation: OperationId,
        callee: AbstractCallTarget,
        arguments: Vec<ValueId>, // Runtime scalar arguments in exact Terminal call order.
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        result: AbstractStructuralCallResult,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    SelectImplementation {
        psi_operation: OperationId,
        result: AbstractResult,
        table: u32,
    },
    SelectMachine {
        psi_operation: OperationId,
        result: AbstractResult,
        descriptor: semantic_vocabulary::MachineDescriptorId,
    },
    BoundaryCall {
        psi_operation: OperationId,
        result: AbstractBoundaryResult,
        boundary: BoundaryMachineId,
        arguments: Vec<ValueId>, // Runtime scalar arguments in exact Terminal Psi call order.
        structural_arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    RegisterAccess {
        psi_operation: OperationId,
        service: Option<ServiceId>,
        event: terminal_psi::RegisterAccessEvent,
        result: Option<AbstractResult>,
    },
    Call {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        callee: MachineId,
        arguments: Vec<ValueId>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    IntegerConstant {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: ScalarType,
        value: IntegerValue,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    FloatResultPolicy {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        policy: terminal_psi::FloatResultPolicy,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    IeeeFloatCompare {
        psi_operation: OperationId,
        result: ValueId,
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: IeeeFloatFormat,
        left: ValueId,
        right: ValueId,
    },
    BooleanConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
    },
    StructuralScalarFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
    },
    BooleanNot {
        psi_operation: OperationId,
        result: ValueId,
        operand: ValueId,
    },
    BooleanEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        operand: ValueId,
    },
    IntegerWiden {
        psi_operation: OperationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ValueId,
    },
    ScalarRepresentationView {
        psi_operation: OperationId,
        result: ValueId,
        source_type: ScalarType,
        target_type: ScalarType,
        operand: ValueId,
    },
    IntegerBitwiseAnd {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
    },
    IntegerShift {
        psi_operation: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        policy: crate::IntegerShiftPolicy,
        direction: crate::IntegerShiftDirection,
        value: ValueId,
        count: ValueId,
    },
    IntegerArithmetic {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        arithmetic: crate::IntegerArithmetic,
        left: ValueId,
        right: ValueId,
    },
    TrappingInteger {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType, // Result carrier shared by binary arithmetic operands and shifted values.
        operand_type: IntegerType, // Shift count type, conversion source type, or `scalar_type` for binary arithmetic.
        operation: terminal_psi::TrappingIntegerOperation,
    },
    Jump {
        psi_edge: EdgeId,
        target: BlockId,
        bindings: Vec<ValueBinding>,
        structural_bindings: Vec<AbstractStructuralBinding>,
        cleanup_actions: Vec<CleanupAction>, // Exact selected-edge cleanup order; never regroup by kind.
    },
    Conditional {
        condition: ValueId,
        when_true: AbstractSuccessor,
        when_false: AbstractSuccessor,
    },
    StructuralCase {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        cases: Vec<AbstractStructuralCaseSuccessor>,
    },
    Return {
        psi_edge: EdgeId,
        result: ValueId,
        value: ValueId,
        scalar_type: ScalarType,
        cleanup_actions: Vec<CleanupAction>, // Executed after the scalar result is materialized.
    },
    ReturnUnit {
        psi_edge: EdgeId,
        cleanup_actions: Vec<CleanupAction>,
    },
    ReturnStructural {
        psi_edge: EdgeId,
        source: PlaceId,
        returned_claims: Vec<StructuralResultClaimBinding>,
        trivial_affine_locals: Vec<(OperationId, PlaceDeclaration, StructuralTypeDeclaration)>,
        cleanup_actions: Vec<CleanupAction>,
    },
    EstablishAccessReference {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        correspondence: super::AccessReferenceBacking,
    },
    EstablishReference {
        psi_operation: OperationId,
        result: OperationResult,
        source: StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    },
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    InvokeNoNormal {
        psi_edge: EdgeId,
        callee: MachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    ExternalCompletion {
        psi_edge: EdgeId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        kind: semantic_vocabulary::ExternalCompletionKind,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        frontier_lower_bound: Vec<ClaimId>,
    },
    Crash {
        psi_edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

impl AbstractOperation {
    pub fn module_invocation(&self) -> Option<ModuleInvocation<'_>> {
        let (
            site,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ) = match self {
            Self::Call {
                psi_operation,
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                (*psi_operation).into(),
                *callee,
                arguments.as_slice(),
                &[][..],
                &[][..],
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            Self::CallStructural {
                psi_operation,
                callee: AbstractCallTarget::Machine(callee),
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                (*psi_operation).into(),
                *callee,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            Self::InvokeNoNormal {
                psi_edge,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                semantic_vocabulary::InvocationSite::InvokeNoNormal(*psi_edge),
                *callee,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            _ => return None,
        };
        Some(ModuleInvocation {
            site,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        })
    }

    pub fn invocation_arguments(&self) -> &[StructuralArgument] {
        match self {
            Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            }
            | Self::ExternalCompletion {
                structural_arguments,
                ..
            }
            | Self::InvokeNoNormal {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => &[],
        }
    }

    pub fn cleanup_rosters(&self) -> impl Iterator<Item = (EdgeId, &[CleanupAction])> {
        let (first, second, cases): (_, _, &[crate::AbstractStructuralCaseSuccessor]) = match self {
            Self::Jump {
                psi_edge,
                cleanup_actions,
                ..
            }
            | Self::Return {
                psi_edge,
                cleanup_actions,
                ..
            }
            | Self::ReturnUnit {
                psi_edge,
                cleanup_actions,
            }
            | Self::ReturnStructural {
                psi_edge,
                cleanup_actions,
                ..
            } => (Some((*psi_edge, cleanup_actions.as_slice())), None, &[]),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (
                Some((when_true.psi_edge, when_true.cleanup_actions.as_slice())),
                Some((when_false.psi_edge, when_false.cleanup_actions.as_slice())),
                &[],
            ),
            Self::StructuralCase { cases, .. } => (None, None, cases),
            _ => (None, None, &[]),
        };
        first.into_iter().chain(second).chain(
            cases
                .iter()
                .map(|case| (case.psi_edge, case.cleanup_actions.as_slice())),
        )
    }

    pub fn successor_target(&self, edge: EdgeId) -> Option<BlockId> {
        match self {
            Self::Jump {
                psi_edge, target, ..
            } if *psi_edge == edge => Some(*target),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            Self::StructuralCase { cases, .. } => cases
                .iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            _ => None,
        }
    }

    pub fn owned_transfer(&self, edge: EdgeId) -> Option<(BlockId, &[AbstractStructuralBinding])> {
        match self {
            Self::Jump {
                psi_edge,
                target,
                structural_bindings,
                ..
            } if *psi_edge == edge => Some((*target, structural_bindings.as_slice())),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| (successor.target, successor.structural_bindings.as_slice())),
            Self::StructuralCase { cases, .. } => cases
                .iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| (successor.target, successor.structural_bindings.as_slice())),
            _ => None,
        }
    }

    pub fn materialized_structural_result(&self) -> Option<(OperationId, &OperationResult)> {
        let metadata = self.structural_result_metadata()?;
        match metadata.role {
            StructuralResultRole::OwnedValue | StructuralResultRole::PrimitiveLocalStorage => {
                Some((metadata.operation, metadata.result))
            }
            StructuralResultRole::ViewRecipe => None,
        }
    }

    pub fn structural_storage(&self) -> Option<StructuralStorageDeclaration> {
        if let Self::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = self
        {
            return Some(StructuralStorageDeclaration {
                place: place.id,
                structural_type: structural_type.id,
                access: terminal_psi::StructuralAccess::SharedBorrow,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            });
        }
        let (_, result) = self.structural_result()?;
        Some(StructuralStorageDeclaration {
            place: result.place,
            structural_type: result.structural_type,
            access: terminal_psi::StructuralAccess::Owned,
            multiplicity: result.multiplicity,
        })
    }

    pub fn structural_result(&self) -> Option<(OperationId, &OperationResult)> {
        let metadata = self.structural_result_metadata()?;
        Some((metadata.operation, metadata.result))
    }

    pub fn structural_result_metadata(&self) -> Option<StructuralResultMetadata<'_>> {
        let (result, role) = match self {
            Self::EstablishPrimitiveLocal { result, .. } => {
                (result, StructuralResultRole::PrimitiveLocalStorage)
            }
            Self::ByteSequenceSubslice { result, .. }
            | Self::EstablishElementView { result, .. }
            | Self::ElementViewSubslice { result, .. } => {
                (result, StructuralResultRole::ViewRecipe)
            }
            Self::AtomicEvent {
                event:
                    crate::AbstractAtomicEvent::CompareExchangeOnce {
                        outcome: result, ..
                    },
                ..
            }
            | Self::EstablishActivationClaim { result, .. }
            | Self::SplitActivationClaim { result, .. }
            | Self::MergeActivationClaim { result, .. }
            | Self::EstablishScalarArray { result, .. }
            | Self::EstablishStructuralArray { result, .. }
            | Self::MoveStructuralField { result, .. }
            | Self::StructuralLeafCopy { result, .. }
            | Self::MoveStructuralValue { result, .. }
            | Self::TransformContent { result, .. }
            | Self::EstablishCase { result, .. }
            | Self::EstablishByteSequenceConcatenation { result, .. }
            | Self::EstablishZeroedStructural { result, .. }
            | Self::EstablishStructuralLocal { result, .. }
            | Self::EstablishNativeCodeLayout { result, .. }
            | Self::EstablishRecord { result, .. }
            | Self::EstablishReference { result, .. }
            | Self::EstablishAccessReference { result, .. }
            | Self::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | Self::BoundaryCall {
                result: AbstractBoundaryResult::Structural(result),
                ..
            } => (result, StructuralResultRole::OwnedValue),
            _ => return None,
        };
        Some(StructuralResultMetadata {
            operation: self.psi_operation()?,
            result,
            role,
        })
    }

    pub fn runtime_indices(&self) -> Vec<(ValueId, semantic_vocabulary::ObligationId)> {
        self.structural_paths()
            .into_iter()
            .flatten()
            .filter_map(StructuralPathSegment::runtime_index)
            .collect()
    }

    pub fn structural_paths(&self) -> Vec<&[StructuralPathSegment]> {
        let mut paths = match self {
            Self::AtomicEvent { event, .. } => event
                .location()
                .map(|location| vec![location.path.as_slice()])
                .unwrap_or_default(),
            Self::ActivateTaskRegister {
                gdt,
                task_state,
                scratch,
                ..
            } => vec![
                gdt.path.as_slice(),
                task_state.path.as_slice(),
                scratch.path.as_slice(),
            ],
            Self::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                vec![gdt.path.as_slice(), scratch.path.as_slice()]
            }
            Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => vec![],
            Self::GlobalDescriptorTable { source, .. } => vec![source.path.as_slice()],
            Self::StorageAddress { path, .. }
            | Self::PrimitiveScalarRead { path, .. }
            | Self::ElementViewRead { path, .. }
            | Self::ReleaseReference { path, .. }
            | Self::WriteOnlyPrimitiveStore { path, .. }
            | Self::StructuralScalarFieldStore { path, .. }
            | Self::StructuralCaseMembership { path, .. }
            | Self::StructuralByteSequenceFieldByteStore { path, .. }
            | Self::StructuralByteSequenceFieldStore { path, .. }
            | Self::StructuralByteViewFieldStore { path, .. }
            | Self::StructuralByteSequenceFieldLength { path, .. }
            | Self::StructuralByteSequenceFieldRead { path, .. }
            | Self::MoveStructuralField { path, .. } => vec![path.as_slice()],
            Self::ElementViewWrite {
                destination, path, ..
            } => {
                vec![destination.path.as_slice(), path.as_slice()]
            }
            Self::MoveStructuralValue { source, .. }
            | Self::ReleaseActivationClaim { source, .. }
            | Self::SplitActivationClaim { source, .. } => vec![source.path.as_slice()],
            Self::MergeActivationClaim { lower, upper, .. } => {
                vec![lower.path.as_slice(), upper.path.as_slice()]
            }
            Self::TransformContent {
                source, parents, ..
            } => std::iter::once(source.path.as_slice())
                .chain(parents.iter().map(|parent| parent.path.as_slice()))
                .collect(),
            Self::StructuralLeafCopy { path, .. } => match path {
                super::StructuralCopyPath::Ordinary(path) => vec![path.as_slice()],
                super::StructuralCopyPath::Case(_) => Vec::new(),
            },
            Self::StoreStructuralField { path, value, .. } => {
                vec![path.as_slice(), value.path.as_slice()]
            }
            Self::EstablishRecord { fields, .. } | Self::EstablishCase { fields, .. } => fields
                .iter()
                .filter_map(|field| field.value.as_structural())
                .map(|argument| argument.path.as_slice())
                .collect(),
            Self::EstablishByteSequenceConcatenation { left, right, .. }
            | Self::ByteSequenceEqual { left, right, .. } => {
                vec![
                    left.argument().path.as_slice(),
                    right.argument().path.as_slice(),
                ]
            }
            Self::ByteSequencePredicate { operand, .. } => {
                vec![operand.argument().path.as_slice()]
            }
            Self::Jump {
                structural_bindings,
                ..
            } => structural_bindings
                .iter()
                .map(|binding| binding.argument.path.as_slice())
                .collect(),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => when_true
                .structural_bindings
                .iter()
                .chain(&when_false.structural_bindings)
                .map(|binding| binding.argument.path.as_slice())
                .collect(),
            Self::StructuralCase { path, cases, .. } => std::iter::once(path.as_slice())
                .chain(cases.iter().flat_map(|case| {
                    case.structural_bindings
                        .iter()
                        .map(|binding| binding.argument.path.as_slice())
                }))
                .collect(),
            Self::EstablishReference { source, .. }
            | Self::EstablishAccessReference { source, .. }
            | Self::EstablishElementView { source, .. }
            | Self::ElementViewLength { source, .. }
            | Self::ByteSequenceLength { source, .. }
            | Self::ByteSequenceSubslice { source, .. }
            | Self::ElementViewSubslice { source, .. } => {
                vec![source.path.as_slice()]
            }
            Self::EstablishStructuralArray {
                elements: structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .map(|argument| argument.path.as_slice())
                .collect(),
            _ => Vec::new(),
        };
        paths.extend(
            self.invocation_arguments()
                .iter()
                .map(|argument| argument.path.as_slice()),
        );
        paths.extend(
            self.cleanup_rosters()
                .flat_map(|(_, actions)| actions.iter().map(CleanupAction::path)),
        );
        paths
    }

    pub fn psi_operation(&self) -> Option<OperationId> {
        match self {
            Self::EstablishActivationClaim { psi_operation, .. }
            | Self::SplitActivationClaim { psi_operation, .. }
            | Self::MergeActivationClaim { psi_operation, .. }
            | Self::ReleaseActivationClaim { psi_operation, .. }
            | Self::WriteOnlyPrimitiveStore { psi_operation, .. }
            | Self::ByteSequenceWrite { psi_operation, .. }
            | Self::StructuralByteSequenceFieldByteStore { psi_operation, .. }
            | Self::ByteSequenceStore { psi_operation, .. }
            | Self::StructuralByteSequenceFieldStore { psi_operation, .. }
            | Self::StructuralByteViewFieldStore { psi_operation, .. }
            | Self::EstablishPrimitiveLocal { psi_operation, .. }
            | Self::PrimitiveLocalStore { psi_operation, .. }
            | Self::StorageAddress { psi_operation, .. }
            | Self::GlobalDescriptorTable { psi_operation, .. }
            | Self::ActivateTaskRegister { psi_operation, .. }
            | Self::ReadTaskRegister { psi_operation, .. }
            | Self::ReloadSegmentDescriptors { psi_operation, .. }
            | Self::ReadSegmentSelector { psi_operation, .. }
            | Self::PrimitiveScalarRead { psi_operation, .. }
            | Self::StructuralCaseMembership { psi_operation, .. }
            | Self::StructuralByteSequenceFieldLength { psi_operation, .. }
            | Self::StructuralByteSequenceFieldRead { psi_operation, .. }
            | Self::StructuralScalarFieldStore { psi_operation, .. }
            | Self::MoveStructuralField { psi_operation, .. }
            | Self::StoreStructuralField { psi_operation, .. }
            | Self::StructuralLeafCopy { psi_operation, .. }
            | Self::MoveStructuralValue { psi_operation, .. }
            | Self::TransformContent { psi_operation, .. }
            | Self::EstablishScalarArray { psi_operation, .. }
            | Self::EstablishStructuralArray { psi_operation, .. }
            | Self::EstablishCase { psi_operation, .. }
            | Self::EstablishByteSequenceLiteral { psi_operation, .. }
            | Self::EstablishByteSequenceConcatenation { psi_operation, .. }
            | Self::EstablishTrivialAffineLocal { psi_operation, .. }
            | Self::EstablishZeroedStructural { psi_operation, .. }
            | Self::EstablishStructuralLocal { psi_operation, .. }
            | Self::EstablishNativeCodeLayout { psi_operation, .. }
            | Self::EstablishRecord { psi_operation, .. }
            | Self::EstablishReference { psi_operation, .. }
            | Self::EstablishAccessReference { psi_operation, .. }
            | Self::ReleaseReference { psi_operation, .. }
            | Self::AtomicEvent { psi_operation, .. }
            | Self::CallStructural { psi_operation, .. }
            | Self::SelectImplementation { psi_operation, .. }
            | Self::SelectMachine { psi_operation, .. }
            | Self::BoundaryCall { psi_operation, .. }
            | Self::PortWrite { psi_operation, .. }
            | Self::RegisterAccess { psi_operation, .. }
            | Self::Call { psi_operation, .. }
            | Self::IntegerConstant { psi_operation, .. }
            | Self::IeeeFloatConstant { psi_operation, .. }
            | Self::IeeeFloatCompare { psi_operation, .. }
            | Self::FloatResultPolicy { psi_operation, .. }
            | Self::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. }
            | Self::BooleanConstant { psi_operation, .. }
            | Self::StructuralScalarFieldRead { psi_operation, .. }
            | Self::ByteSequenceRead { psi_operation, .. }
            | Self::ByteSequencePredicate { psi_operation, .. }
            | Self::ByteSequenceEqual { psi_operation, .. }
            | Self::ByteSequenceSubslice { psi_operation, .. }
            | Self::ByteSequenceLength { psi_operation, .. }
            | Self::EstablishElementView { psi_operation, .. }
            | Self::ElementViewLength { psi_operation, .. }
            | Self::ElementViewRead { psi_operation, .. }
            | Self::ElementViewWrite { psi_operation, .. }
            | Self::ElementViewSubslice { psi_operation, .. }
            | Self::BooleanNot { psi_operation, .. }
            | Self::BooleanEqual { psi_operation, .. }
            | Self::IntegerEqual { psi_operation, .. }
            | Self::IntegerLessThan { psi_operation, .. }
            | Self::IntegerLessOrEqual { psi_operation, .. }
            | Self::IntegerBitwiseNot { psi_operation, .. }
            | Self::IntegerWiden { psi_operation, .. }
            | Self::IntegerExactCast { psi_operation, .. }
            | Self::ScalarRepresentationView { psi_operation, .. }
            | Self::IntegerBitwiseAnd { psi_operation, .. }
            | Self::IntegerBitwiseOr { psi_operation, .. }
            | Self::IntegerBitwiseXor { psi_operation, .. }
            | Self::IntegerShift { psi_operation, .. }
            | Self::IntegerArithmetic { psi_operation, .. }
            | Self::TrappingInteger { psi_operation, .. } => Some(*psi_operation),
            Self::Jump { .. }
            | Self::Conditional { .. }
            | Self::StructuralCase { .. }
            | Self::Return { .. }
            | Self::ReturnUnit { .. }
            | Self::ReturnStructural { .. }
            | Self::ExternalCompletion { .. }
            | Self::InvokeNoNormal { .. }
            | Self::Crash { .. } => None,
        }
    }

    pub fn integer_arithmetic(&self) -> Option<IntegerArithmeticOperands> {
        let Self::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } = self
        else {
            return None;
        };
        Some(IntegerArithmeticOperands {
            psi_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            arithmetic: *arithmetic,
            left: *left,
            right: *right,
        })
    }

    pub fn integer_arithmetic_operands(
        &self,
        operator: IntegerArithmeticOperator,
    ) -> Option<IntegerArithmeticOperands> {
        let operands = self.integer_arithmetic()?;
        (operands.arithmetic.operator() == operator).then_some(operands)
    }

    pub fn integer_shift(&self) -> Option<IntegerShiftOperands> {
        let Self::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } = self
        else {
            return None;
        };
        Some(IntegerShiftOperands {
            psi_operation: *psi_operation,
            result: *result,
            value_type: *value_type,
            count_type: *count_type,
            policy: *policy,
            direction: *direction,
            value: *value,
            count: *count,
        })
    }

    pub fn operates_on_byte_sequence_storage(&self) -> bool {
        matches!(
            self,
            Self::ByteSequenceRead { .. }
                | Self::ByteSequencePredicate { .. }
                | Self::ByteSequenceEqual { .. }
                | Self::StructuralByteSequenceFieldRead { .. }
                | Self::ByteSequenceWrite { .. }
                | Self::ByteSequenceSubslice { .. }
                | Self::ByteSequenceStore { .. }
                | Self::StructuralByteSequenceFieldStore { .. }
                | Self::StructuralByteSequenceFieldByteStore { .. }
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IntegerShiftOperands {
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub value_type: IntegerType,
    pub count_type: IntegerType,
    pub policy: crate::IntegerShiftPolicy,
    pub direction: crate::IntegerShiftDirection,
    pub value: ValueId,
    pub count: ValueId,
}

#[derive(Debug, Clone, Copy)]
pub struct IntegerArithmeticOperands {
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub arithmetic: IntegerArithmetic,
    pub left: ValueId,
    pub right: ValueId,
}

#[macro_export]
macro_rules! return_or_return_unit_pattern {
    () => {
        $crate::AbstractOperation::Return { .. } | $crate::AbstractOperation::ReturnUnit { .. }
    };
    ($($fields:tt)+) => {
        $crate::AbstractOperation::Return { $($fields)+ }
            | $crate::AbstractOperation::ReturnUnit { $($fields)+ }
    };
}

#[macro_export]
macro_rules! jump_or_conditional_pattern {
    () => {
        $crate::AbstractOperation::Jump { .. } | $crate::AbstractOperation::Conditional { .. }
    };
}

#[macro_export]
macro_rules! establish_case_or_record_fields_pattern {
    ($fields:ident) => {
        $crate::AbstractOperation::EstablishCase { $fields, .. }
            | $crate::AbstractOperation::EstablishRecord { $fields, .. }
    };
}

#[macro_export]
macro_rules! establish_case_or_record_result_pattern {
    ($result:ident) => {
        $crate::AbstractOperation::EstablishCase { $result, .. }
            | $crate::AbstractOperation::EstablishRecord { $result, .. }
    };
}

#[macro_export]
macro_rules! return_structural_or_crash_pattern {
    () => {
        $crate::AbstractOperation::ReturnStructural { .. } | $crate::AbstractOperation::Crash { .. }
    };
}

#[macro_export]
macro_rules! call_structural_or_boundary_call_pattern {
    ($structural_arguments:ident) => {
        $crate::AbstractOperation::InvokeNoNormal {
            $structural_arguments,
            ..
        } | $crate::AbstractOperation::CallStructural {
            $structural_arguments,
            ..
        } | $crate::AbstractOperation::BoundaryCall {
            $structural_arguments,
            ..
        }
    };
}

#[macro_export]
macro_rules! move_or_store_structural_field_pattern {
    () => {
        $crate::AbstractOperation::MoveStructuralField { .. }
            | $crate::AbstractOperation::StoreStructuralField { .. }
    };
}

#[macro_export]
macro_rules! structural_leaf_copy_pair_pattern {
    () => {
        $crate::AbstractOperation::StructuralLeafCopy { .. }
            | $crate::AbstractOperation::StructuralCaseLeafCopy { .. }
    };
}

#[macro_export]
macro_rules! byte_sequence_store_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceStore { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_sequence_write_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceWrite { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldByteStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! element_view_or_byte_subslice_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ElementViewSubslice { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_or_xor_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseOr { $($fields)* }
            | $crate::AbstractOperation::IntegerBitwiseXor { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_equal_less_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerEqual { $($fields)* }
            | $crate::AbstractOperation::IntegerLessThan { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_len_or_field_read_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StructuralByteSequenceFieldLength { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_not_and_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseNot { $($fields)* }
            | $crate::AbstractOperation::IntegerBitwiseAnd { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_or_field_read_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! element_view_read_write_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ElementViewRead { $($fields)* }
            | $crate::AbstractOperation::ElementViewWrite { $($fields)* }
    };
}

#[macro_export]
macro_rules! boolean_not_or_equal_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BooleanNot { $($fields)* }
            | $crate::AbstractOperation::BooleanEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! primitive_storage_observation_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StorageAddress { $($fields)* }
            | $crate::AbstractOperation::PrimitiveScalarRead { $($fields)* }
            | $crate::AbstractOperation::StructuralCaseMembership { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_widen_or_exact_cast_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerWiden { $($fields)* }
            | $crate::AbstractOperation::IntegerExactCast { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_record_or_case_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishRecord { $($fields)* }
            | $crate::AbstractOperation::EstablishCase { $($fields)* }
    };
}

#[macro_export]
macro_rules! structural_array_or_case_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
            | $crate::AbstractOperation::EstablishCase { $($fields)* }
    };
}

#[macro_export]
macro_rules! call_or_call_structural_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::Call { $($fields)* }
            | $crate::AbstractOperation::CallStructural { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_predicate_equal_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::ByteSequencePredicate { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_byte_literal_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceLiteral { $($fields)* }
            | $crate::AbstractOperation::EstablishTrivialAffineLocal { $($fields)* }
    };
}

#[macro_export]
macro_rules! primitive_local_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::PrimitiveLocalStore { $($fields)* }
            | $crate::AbstractOperation::WriteOnlyPrimitiveStore { $($fields)* }
    };
}

#[macro_export]
macro_rules! structural_establish_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishZeroedStructural { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishRecord { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_view_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StructuralByteViewFieldStore { $($fields)* }
            | $crate::AbstractOperation::StructuralByteSequenceFieldByteStore { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_not_or_less_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseNot { $($fields)* }
            | $crate::AbstractOperation::IntegerLessOrEqual { $($fields)* }
    };
}

#[macro_export]
macro_rules! bitwise_and_or_view_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerBitwiseAnd { $($fields)* }
            | $crate::AbstractOperation::ScalarRepresentationView { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_literal_establish_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::EstablishByteSequenceLiteral { $($fields)* }
            | $crate::AbstractOperation::EstablishTrivialAffineLocal { $($fields)* }
    };
}

#[macro_export]
macro_rules! boundary_or_structural_call_pair_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BoundaryCall { $($fields)* }
            | $crate::AbstractOperation::CallStructural { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_view_length_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceLength { $($fields)* }
            | $crate::AbstractOperation::ElementViewLength { $($fields)* }
            | $crate::AbstractOperation::EstablishElementView { $($fields)* }
    };
}

#[macro_export]
macro_rules! byte_read_subslice_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceLength { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceRead { $($fields)* }
            | $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
    };
}

#[macro_export]
macro_rules! integer_dispatch_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::IntegerArithmetic { $($fields)* }
            | $crate::AbstractOperation::IntegerShift { $($fields)* }
            | $crate::AbstractOperation::TrappingInteger { $($fields)* }
    };
}

#[macro_export]
macro_rules! constant_value_triple_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::BooleanConstant { $($fields)* }
            | $crate::AbstractOperation::IeeeFloatConstant { $($fields)* }
            | $crate::AbstractOperation::IntegerConstant { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_set_quint_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishScalarArray { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
    };
}

#[macro_export]
macro_rules! scalar_storage_observation_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::StorageAddress { $($fields)* }
            | $crate::AbstractOperation::PrimitiveScalarRead { $($fields)* }
            | $crate::AbstractOperation::StructuralCaseMembership { $($fields)* }
            | $crate::AbstractOperation::StructuralScalarFieldRead { $($fields)* }
    };
}

#[macro_export]
macro_rules! establish_or_subslice_set_pattern {
    ($($fields:tt)*) => {
        $crate::AbstractOperation::ByteSequenceSubslice { $($fields)* }
            | $crate::AbstractOperation::ElementViewSubslice { $($fields)* }
            | $crate::AbstractOperation::EstablishByteSequenceConcatenation { $($fields)* }
            | $crate::AbstractOperation::EstablishElementView { $($fields)* }
            | $crate::AbstractOperation::EstablishPrimitiveLocal { $($fields)* }
            | $crate::AbstractOperation::EstablishScalarArray { $($fields)* }
            | $crate::AbstractOperation::EstablishStructuralArray { $($fields)* }
    };
}
