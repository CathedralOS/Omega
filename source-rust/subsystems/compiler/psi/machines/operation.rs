pub use super::operands::{
    ByteSequenceOperand, ElementViewRegion, ReferenceStorageView, StructuralStoreDisposition,
};
pub use super::operation_result::{
    OperationResult, StructuralOperationResult, StructuralResultClaimBinding,
};
use crate::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, OutcomeSpecificCallEvidence,
    StructuralArgument, StructuralPathSegment,
};
use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, IeeeFloatValue, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, ProofTerm, ScalarTerm, ServiceId,
    StructuralCaseId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    SuspensionCrossingId, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub static_reach_binding: Option<u32>,
    /// Keep the public entry contract beside this exact operation. The
    /// implementation remains the kind's physical target; no other call
    /// of that implementation acquires this route.
    pub required_contract: Option<semantic_vocabulary::ContractId>,
    pub suspension_crossing: Option<SuspensionCrossingId>,
    pub id: OperationId,
    pub result: OperationResult,
    pub kind: OperationKind,
}

impl Operation {
    pub const fn unsuspended(
        id: OperationId,
        result: OperationResult,
        kind: OperationKind,
    ) -> Self {
        Self {
            static_reach_binding: None,
            required_contract: None,
            suspension_crossing: None,
            id,
            result,
            kind,
        }
    }

    pub fn module_invocation(&self) -> Option<super::ModuleInvocationInputs<'_>> {
        let mut invocation = self.kind.module_invocation()?;
        invocation.required_contract = self.required_contract;
        Some(invocation)
    }

    pub fn declared_invocation(
        &self,
    ) -> Option<super::ModuleInvocationInputs<'_, super::TargetMachineCallable>> {
        let mut invocation = self.kind.declared_invocation()?;
        invocation.required_contract = self.required_contract;
        Some(invocation)
    }
}

/// Two observations of the same captured storage path. Only address
/// exposure yields an address that can participate in identity comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageLocationQuery {
    Address,
    Alignment,
}

impl StorageLocationQuery {
    pub fn admits_result(self, ty: semantic_vocabulary::ScalarType) -> bool {
        use semantic_vocabulary::{IntegerSign, ScalarType};
        let ScalarType::Integer(integer) = ty else {
            return false;
        };
        match self {
            Self::Address => integer.is_address(),
            Self::Alignment => {
                !integer.is_address()
                    && integer.bits() == Some(64)
                    && integer.sign() == IntegerSign::Unsigned
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {    ActivateTaskRegister {
        service: ServiceId,
        entries: u16,
        gdt: StructuralArgument,
        task_state: StructuralArgument,
        selector: ValueId,
        scratch: StructuralArgument,
    },
    PackedIntegerUnary {
        operation: semantic_vocabulary::PackedIntegerUnaryOperation,
        source: StructuralArgument,
    },
    PackedIntegerBinary {
        operation: semantic_vocabulary::PackedIntegerBinaryOperation,
        left: StructuralArgument,
        right: StructuralArgument,
    },
    ReadTaskRegister {
        service: ServiceId,
    },
    ReloadSegmentDescriptors {
        service: ServiceId,
        entries: u16,
        kind: language_core::descriptor_tables::SegmentReload,
        gdt: StructuralArgument,
        selector: ValueId,
        scratch: StructuralArgument,
    },
    ReadSegmentSelector {
        service: ServiceId,
        register: language_core::descriptor_tables::SegmentRegister,
    },
    ReadScalarApplication {
        application: semantic_vocabulary::ScalarApplicationId,
    },
    EstablishNativeCodeLayout {
        machine: MachineId,
    },
    StorageAddress {
        source: StructuralArgument,
    },
    /// Observe the alignment of this live projected location without lending
    /// the storage or loading the value at that location.
    StorageAlignment {
        source: StructuralArgument,
    },
    GlobalDescriptorTable {
        service: ServiceId,
        operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
        source: StructuralArgument,
    },
    EstablishAccessReference {
        source: StructuralArgument,
        resident: Option<crate::ResidentReferenceSource>,
    },
    EstablishReference {
        source: StructuralArgument,
        storage: Option<ReferenceStorageView>,
    },
    ReleaseReference {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    EstablishScalarArray {
        elements: Vec<ValueId>,
    },
    EstablishActivationClaim {
        bound: u64,
        committed: ValueId,
        backing_type: StructuralTypeId,
        established_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    ReleaseActivationClaim {
        source: StructuralArgument,
        claim: ClaimId,
    },
    SplitActivationClaim {
        source: StructuralArgument,
        claim: ClaimId,
        bound_at: u64,
        committed_at: ValueId,
        covered_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    MergeActivationClaim {
        lower: StructuralArgument,
        upper: StructuralArgument,
        lower_claim: ClaimId,
        upper_claim: ClaimId,
        covered_case: StructuralCaseId,
        rejected_case: StructuralCaseId,
    },
    EstablishStructuralArray {
        elements: Vec<StructuralArgument>,
    },
    EstablishZeroedStructural {},
    EstablishStructuralLocal {},
    EstablishPrimitiveLocal {
        value: ValueId,
        range_obligation: Option<ObligationId>,
    },
    PrimitiveScalarRead {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    StructuralByteSequenceFieldLength {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
    },
    StructuralByteSequenceFieldByteStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    WriteOnlyPrimitiveStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        value: ValueId,
        range_obligation: Option<ObligationId>,
    },
    ByteSequenceStore {
        destination: PlaceId,
        source: PlaceId,
        length: ValueId,
        obligation: ObligationId,
    },
    StructuralByteSequenceFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        source: PlaceId,
        length: ValueId,
        obligation: ObligationId,
    },
    StructuralByteViewFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        source: PlaceId,
        predicate_obligation: Option<ObligationId>,
    },
    StructuralByteSequenceFieldRead {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    StructuralScalarFieldStore {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
        range_obligation: Option<ObligationId>,
    },
    MoveStructuralField {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
    },
    StoreStructuralField {
        disposition: StructuralStoreDisposition,
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: StructuralArgument,
    },
    EstablishScalarCase {
        result_case: StructuralCaseId,
        fields: Vec<crate::ScalarCaseField>,
    },
    EstablishStructuralCase {
        result_case: StructuralCaseId,
        fields: Vec<crate::RecordFieldInitializer>,
    },
    StructuralCaseMembership {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        case: StructuralCaseId,
    },
    MoveStructuralValue {
        source: StructuralArgument,
    },
    /// Partition a complete owned record at its existing storage home.
    /// End its intrinsic wrapper claim or authorized selected cleanup;
    /// retain every component's independent claims and cleanup obligations.
    DecomposeStructuralValue {
        subject: StructuralArgument,
    },
    TransformContent {
        source: StructuralArgument,
        parents: Vec<StructuralArgument>,
        derivations: Vec<crate::ContentTransformationDerivation>,
    },
    StructuralLeafCopy {
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
    },
    StructuralCaseLeafCopy {
        source: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    EstablishByteSequenceConcatenation {
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        capacity_obligation: ObligationId,
    },
    EstablishByteSequenceLiteral {
        destination: PlaceId,
        bytes: Vec<u8>,
        qualifications: Vec<StructuralDomainId>,
    },
    ByteSequenceLength {
        source: StructuralArgument,
    },
    ByteSequenceRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceWrite {
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceSubslice {
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ByteSequencePredicate {
        operand: ByteSequenceOperand,
        predicate: crate::ByteSequencePredicate,
    },
    ByteSequenceEqual {
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
    },
    EstablishElementView {
        destination: PlaceId,
        source: StructuralArgument,
        element: StructuralTypeId,
        region: ElementViewRegion,
    },
    EstablishMutableElementView {
        destination: PlaceId,
        source: StructuralArgument,
        element: StructuralTypeId,
        region: ElementViewRegion,
    },
    ElementViewLength {
        source: StructuralArgument,
    },
    ElementViewRead {
        source: StructuralArgument,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
        path: Vec<StructuralPathSegment>,
    },
    ElementViewSubslice {
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ElementViewWrite {
        destination: StructuralArgument,
        path: Vec<StructuralPathSegment>,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    EstablishTrivialAffineLocal {
        destination: PlaceId,
    },
    EstablishRecord {
        fields: Vec<crate::RecordFieldInitializer>,
    },
    SelectImplementation {
        table: u32,
    },
    SelectMachine {
        descriptor: semantic_vocabulary::MachineDescriptorId,
    },
    /// Calls the evaluated machine value under its retained callable contract.
    /// The descriptor supplies code; it never supplies an implicit argument.
    CallMachine {
        callee: ValueId,
        contract: semantic_vocabulary::CallableContractId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallDynamic {
        selector: ValueId,
        interface: u32,
        slot: u32,
        arguments: Vec<ValueId>,
        family_actuals: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    Call {
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallUnit {
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructuralScalar {
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    CallStructural {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    CallStructuralWithScalarArguments {
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        selected_evidence: Vec<OutcomeSpecificCallEvidence>,
    },
    BoundaryCall {
        boundary: BoundaryMachineId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
        completion_receipts: Vec<CompletionReceipt>,
        requirement_obligations: Vec<ObligationId>,
    },
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    RegisterAccess {
        service: Option<ServiceId>,
        event: super::RegisterAccessEvent,
    },
    IntegerConstant {
        value: IntegerValue,
    },
    BooleanConstant {
        value: bool,
    },
    IeeeFloatConstant {
        value: IeeeFloatValue,
    },
    FloatResultPolicy {
        policy: crate::FloatResultPolicy,
    },
    IeeeFloatCompare {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        left: ValueId,
        right: ValueId,
    },
    ScalarRepresentationView {
        operand: ValueId,
        landing_obligation: Option<ObligationId>,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        left: ValueId,
        right: ValueId,
        addend: ValueId,
    },
    BooleanStructuralField {
        source: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    ScalarStructuralField {
        source: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
    BooleanNot {
        operand: ValueId,
    },
    BooleanEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        operand: ValueId,
    },
    IntegerWiden {
        operand: ValueId,
    },
    IntegerExactCast {
        operand: ValueId,
        obligation: ObligationId,
    },
    IntegerBitwiseAnd {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerShiftRight {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerAdd {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerSubtract {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerMultiply {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
    TrappingInteger {
        operation: crate::TrappingIntegerOperation,
    },
    AtomicAccess {
        place: PlaceId,
        path: Vec<StructuralPathSegment>,
        event: crate::AtomicAccessEvent,
        resident_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    AtomicFence {
        ordering: language_core::atomic::FenceOrdering,
    },
}
