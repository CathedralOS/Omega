/*
Ordered nonterminal operations inside a target basic block combine Terminal Psi operation identities with evaluated ABI call plans, boundary realizations, resolved byte offsets, and durable result-home requirements. We define `TargetUnitOperation`, `NativeCallOrigin`, `TargetFloatResultPolicy`, `TargetInstallationCall`, `TargetStructuralRuntimeIndex`, and `atomic_storage_integer_type` in one module.

Every operation that establishes a structural value exposes its exact `structural_result` and `structural_result_home` queries so downstream legalization and home assignment can replay producer identities across both ordinary and boundary calls.
*/

use crate::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, ProviderExecutionBinding, TargetBoundaryResult,
    TargetStructuralArgument, TargetStructuralHomeRequirement, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource,
};
use abstract_operations::{AbstractResult, CompletionClaimSource};
use calling_conventions::{CallPlan, ValueShape};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralFieldId,
    ValueId,
};
use terminal_psi::{
    AtomicAccessEvent, ClaimTransfer, CompletionReceipt, CrashRouteBucket,
    ProviderCandidateConformance, StructuralArgument, StructuralOperationResult,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    TerminalAffineCleanupAction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCallOrigin {
    Authored,
    ProgramEntry {
        boundary: BoundaryMachineId,
        entry: MachineId,
        source_signature: [u8; 32],
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
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
        assumption: Option<crate::InstalledAssumedBindingEvidence>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
}

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
pub struct TargetInstallationCall {
    pub role: crate::InstallationCallRole,
    pub callee: MachineId,
    pub call_plan: CallPlan,
    pub argument: TargetStructuralArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitOperation {
    InstallStaticInterrupt {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        arguments: [StructuralArgument; 6],
        calls: [TargetInstallationCall; 2],
        result_home: TargetUnitScalarHomeRequirement,
    },
    StaticInterrupt {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        operation: language_core::static_interrupts::StaticInterruptOperation,
        token: StructuralArgument,
        result_home: Option<TargetStructuralHomeRequirement>,
    },
    AccessLifecycle {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        operands: abstract_operations::AccessLifecycleOperands,
        result_home: TargetStructuralHomeRequirement,
    },
    EstablishActivationClaim {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        bound: u64,
        committed: TargetUnitScalarArgumentSource,
        backing_type: semantic_vocabulary::StructuralTypeId,
        established_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
    },
    ReleaseActivationClaim {
        psi_operation: OperationId,
        source: StructuralArgument,
        claim: semantic_vocabulary::ClaimId,
    },
    SplitActivationClaim {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        source: StructuralArgument,
        claim: semantic_vocabulary::ClaimId,
        bound_at: u64,
        committed: TargetUnitScalarArgumentSource,
        covered_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
    },
    MergeActivationClaim {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        lower: StructuralArgument,
        upper: StructuralArgument,
        lower_claim: semantic_vocabulary::ClaimId,
        upper_claim: semantic_vocabulary::ClaimId,
        covered_case: semantic_vocabulary::StructuralCaseId,
        rejected_case: semantic_vocabulary::StructuralCaseId,
    },
    EstablishNativeCodeLayout {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        machine: semantic_vocabulary::MachineId,
    },
    AccessorScalarAccess {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        realization: crate::AccessorScalarAccessRealization,
        location: StructuralArgument,
        operand: Option<TargetUnitScalarArgumentSource>,
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    AtomicFence {
        psi_operation: OperationId,
        ordering: terminal_psi::FenceOrdering,
    },
    SelectImplementation {
        result_home: TargetUnitScalarHomeRequirement,
        table: u32,
    },
    SelectMachine {
        result_home: TargetUnitScalarHomeRequirement,
        descriptor: semantic_vocabulary::MachineDescriptorId,
    },
    FloatResultPolicy {
        psi_operation: OperationId,
        result_home: TargetUnitScalarHomeRequirement,
        format: IeeeFloatFormat,
        policy: TargetFloatResultPolicy,
    },
    AtomicAccess {
        psi_operation: OperationId,
        location: StructuralArgument,
        scalar_type: IntegerType,
        field_byte_offset: u32,
        event: terminal_psi::AtomicAccessEvent,
        operands: Vec<TargetUnitScalarArgumentSource>,
        result_home: Option<TargetUnitScalarHomeRequirement>,
        outcome_home: Option<TargetStructuralHomeRequirement>,
    },
    IeeeFloatCompare {
        result_home: TargetUnitScalarHomeRequirement,
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: IeeeFloatFormat,
        left: TargetUnitScalarArgumentSource,
        right: TargetUnitScalarArgumentSource,
    },
    ScalarRepresentationView {
        result_home: TargetUnitScalarHomeRequirement,
        operand: TargetUnitScalarArgumentSource,
    },
    IeeeFloatClassify {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        realization: crate::NearestIeeeFloatArithmeticRealization,
        result_home: TargetStructuralHomeRequirement,
        argument: TargetUnitScalarArgumentSource,
    },
    ProgramImageAddress {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        execution: BoundaryExecutionBinding,
        result_home: TargetUnitScalarHomeRequirement,
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
    StorageAddress {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    PrimitiveScalarRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: PlaceId,
        path: Vec<StructuralPathSegment>,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
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
    StructuralScalarFieldRead {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        case: Option<semantic_vocabulary::StructuralCaseId>,
        field: StructuralFieldId,
    },
    StructuralLeafCopy {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        source: PlaceId,
        path: abstract_operations::StructuralCopyPath,
        byte_offset: u32,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    StoreStructuralField {
        psi_operation: OperationId,
        destination: abstract_operations::StructuralWindowRoot,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: PlaceId,
        byte_offset: u32,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
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
    StructuralByteSequenceFieldLength {
        psi_operation: OperationId,
        result: AbstractResult,
        source: StructuralArgument,
        field: StructuralFieldId,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    ScalarDefinition {
        result_home: TargetUnitScalarHomeRequirement,
        expression: crate::TargetScalarExpression,
    },
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
    StructuralByteViewFieldStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: Option<StructuralFieldId>,
        source: PlaceId,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
    StructuralByteSequenceFieldByteStore {
        psi_operation: OperationId,
        destination: StructuralArgument,
        field: StructuralFieldId,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceSubslice {
        result: StructuralOperationResult,
        view: crate::TargetByteView,
    },
    EstablishElementView {
        result: StructuralOperationResult,
        view: crate::TargetElementView,
    },
    ElementViewSubslice {
        result: StructuralOperationResult,
        view: crate::TargetElementView,
    },
    ByteSequenceWrite {
        psi_operation: OperationId,
        destination: semantic_vocabulary::PlaceId,
        view: crate::TargetByteView,
        index: TargetUnitScalarArgumentSource,
        value: TargetUnitScalarArgumentSource,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
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
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: abstract_operations::StructuralScalarStoreDestination,
        path: Vec<StructuralPathSegment>,
        destination_type: StructuralTypeDeclaration,
        destination_storage: crate::TargetStructuralArgumentSource,
        source: TargetUnitWriteOnlyPrimitiveStoreSource,
        indices: Vec<TargetStructuralRuntimeIndex>,
    },
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
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    EstablishZeroedStructural {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
    },
    EstablishStructuralLocal {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
    },
    EstablishRecord {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    EstablishAccessReference {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        source: StructuralArgument,
        correspondence: abstract_operations::AccessReferenceBacking,
    },
    EstablishReference {
        psi_operation: OperationId,
        result_home: crate::TargetStructuralHomeRequirement,
        source: StructuralArgument,
        storage: Option<terminal_psi::ReferenceStorageView>,
    },
    ReleaseReference {
        psi_operation: OperationId,
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
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
        access_bindings: Vec<crate::TargetAccessCallBinding>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    EstablishScalarArray {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        elements: Vec<ValueId>,
    },
    EstablishStructuralArray {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        elements: Vec<terminal_psi::StructuralArgument>,
    },
    EstablishCase {
        psi_operation: OperationId,
        result_home: TargetStructuralHomeRequirement,
        result_case: semantic_vocabulary::StructuralCaseId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
        structural_arguments: Vec<NormalizedForeignStructuralArgument>,
        result: TargetBoundaryResult,
        descriptor_input: Option<crate::DescriptorInputGuard>,
    },
    PortWrite {
        psi_operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
    ActivateTaskRegister {
        psi_operation: OperationId,
        service: ServiceId,
        entries: u16,
        gdt: StructuralArgument,
        task_state: StructuralArgument,
        selector: TargetUnitScalarArgumentSource,
        scratch: StructuralArgument,
        result_home: TargetUnitScalarHomeRequirement,
    },
    ReloadSegmentDescriptors {
        psi_operation: OperationId,
        service: ServiceId,
        entries: u16,
        kind: terminal_psi::SegmentReload,
        gdt: StructuralArgument,
        selector: TargetUnitScalarArgumentSource,
        scratch: StructuralArgument,
        result_home: TargetUnitScalarHomeRequirement,
    },
    ReadSegmentSelector {
        psi_operation: OperationId,
        service: ServiceId,
        register: terminal_psi::SegmentRegister,
        result_home: TargetUnitScalarHomeRequirement,
    },
    ReadTaskRegister {
        psi_operation: OperationId,
        service: ServiceId,
        result_home: TargetUnitScalarHomeRequirement,
    },
    GlobalDescriptorTable {
        psi_operation: OperationId,
        service: ServiceId,
        operation: terminal_psi::GlobalDescriptorTableOperation,
        source: terminal_psi::StructuralArgument,
    },
    RegisterAccess {
        psi_operation: OperationId,
        service: Option<ServiceId>,
        event: terminal_psi::RegisterAccessEvent<TargetUnitScalarArgumentSource>,
        result_home: Option<TargetUnitScalarHomeRequirement>,
    },
    BoundarySettlement {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        result: TargetBoundaryResult,
        execution: BoundaryExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
        runtime_scalar_arguments: Vec<TargetUnitScalarCallArgument>,
        arguments: Vec<StructuralArgument>,
        byte_sequence_arguments: Vec<BoundaryByteSequenceArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralRuntimeIndex {
    pub operand: TargetUnitScalarArgumentSource,
    pub stride: u32,
}

impl TargetUnitOperation {
    pub fn installation_operation(&self) -> Option<OperationId> {
        match self {
            Self::InstallStaticInterrupt { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        }
    }

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

    pub fn structural_result_home(
        &self,
    ) -> Option<(OperationId, &TargetStructuralHomeRequirement)> {
        match self {
            Self::AtomicAccess {
                psi_operation,
                outcome_home: Some(home),
                ..
            } => Some((*psi_operation, home)),
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
            | Self::EstablishNativeCodeLayout {
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
            | Self::EstablishAccessReference {
                psi_operation,
                result_home,
                ..
            }
            | Self::EstablishActivationClaim {
                psi_operation,
                result_home,
                ..
            }
            | Self::SplitActivationClaim {
                psi_operation,
                result_home,
                ..
            }
            | Self::MergeActivationClaim {
                psi_operation,
                result_home,
                ..
            }
            | Self::StaticInterrupt {
                psi_operation,
                result_home: Some(result_home),
                ..
            }
            | Self::AccessLifecycle {
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

pub fn atomic_storage_integer_type(
    event: AtomicAccessEvent,
    logical_type: ScalarType,
) -> Option<IntegerType> {
    if !event.ordering_is_legal() {
        return None;
    }
    match logical_type {
        ScalarType::Boolean
            if matches!(
                event,
                AtomicAccessEvent::Load { .. } | AtomicAccessEvent::Store { .. }
            ) =>
        {
            IntegerType::new(IntegerSign::Unsigned, 8).ok()
        }
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(integer)
        }
        _ => None,
    }
}
