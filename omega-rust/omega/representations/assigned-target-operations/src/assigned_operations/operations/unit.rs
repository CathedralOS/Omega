//! operations unit in the assigned operations program.

pub use crate::assigned_operations::storage::scalar_call::{
    AssignedNormalizedForeignScalarArgument, AssignedUnitScalarArgumentSource,
    AssignedUnitScalarCallArgument, AssignedUnitScalarHome,
};
use calling_conventions::CallPlan;
use calling_conventions::ValuePlacement;
use calling_conventions::ValueShape;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::IeeeFloatFormat;
use semantic_vocabulary::IeeeFloatValue;
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::ServiceId;
use semantic_vocabulary::StructuralFieldId;
use semantic_vocabulary::StructuralTypeId;
use semantic_vocabulary::ValueId;
use target_operations::AbstractDynamicDescriptorArgument;
use target_operations::AbstractReboundDynamicDispatch;
use target_operations::AbstractResult;
use target_operations::AbstractStoredDynamicDescriptor;
use target_operations::AbstractStoredDynamicDispatch;
use target_operations::BoundaryByteSequenceArgument;
use target_operations::BoundaryRealization;
use target_operations::BoundaryScalarArgument;
use target_operations::CompletionClaimSource;
use target_operations::MachineRegister;
use target_operations::ProviderExecutionBinding;
use target_operations::RankedU32CountdownCustody;
use target_operations::TargetStructuralParameter;
use target_operations::UnitScalarAbiValue;
use terminal_psi::ClaimTransfer;
use terminal_psi::CompletionReceipt;
use terminal_psi::CrashRouteBucket;
use terminal_psi::ProviderCandidateConformance;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralOperationResult;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralPlaceDeclaration;
use terminal_psi::StructuralResultClaimTransfer;
use terminal_psi::StructuralResultDeclaration;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::TerminalAffineCleanupAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedRankedU32Countdown {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    /// Stable mutable home of the loop-carried rank. The first exact slice
    /// requires this to be the canonical incoming target-native register.
    pub rank_home: MachineRegister,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitBody {
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<UnitScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<AssignedUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedDynamicDescriptorInstanceArgument {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source: ValuePlacement,
    pub destination: MachineRegister,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedDynamicDescriptorArgument {
    pub custody: AbstractDynamicDescriptorArgument,
    pub instance: AssignedDynamicDescriptorInstanceArgument,
    pub table_destination: MachineRegister,
}

/// One exact raw-bit IEEE FMA operand after physical XMM assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedIeeeFloatFmaOperand {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub value: IeeeFloatValue,
    pub register: MachineRegister,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedAggregateCopy {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
}

/// Durable caller-frame home assigned to one exact structural boundary
/// result. The layout is retained whole so emission and artifact replay can
/// validate tag, payload, size, and alignment without trusting the offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedStructuralHome {
    pub requirement: target_operations::TargetStructuralHomeRequirement,
    pub byte_offset: u32,
}

impl AssignedStructuralHome {
    /// Return the exact scalar shape at one case payload offset. `None`
    /// rejects an offset that does not name exactly one retained field.
    pub fn layout_field(&self, case_index: usize, byte_offset: u32) -> Option<ValueShape> {
        self.requirement
            .layout
            .cases
            .get(case_index)?
            .fields
            .iter()
            .find(|field| u32::from(field.byte_offset) == byte_offset)
            .map(|field| field.shape)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedBoundaryResult {
    Unit,
    Structural(AssignedStructuralHome),
}

/// Target-resolved coordinates copied from the owning runtime ABI plan.
///
/// This assigned carrier records the exact result of ABI planning without
/// giving a representations crate an upward dependency on the backend owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedDynamicTraitDescriptorAbi {
    instance_byte_offset: u32,
    table_byte_offset: u32,
    word_byte_size: u32,
    total_byte_size: u32,
    byte_alignment: u32,
}

impl AssignedDynamicTraitDescriptorAbi {
    pub const fn new(
        instance_byte_offset: u32,
        table_byte_offset: u32,
        word_byte_size: u32,
        total_byte_size: u32,
        byte_alignment: u32,
    ) -> Self {
        Self {
            instance_byte_offset,
            table_byte_offset,
            word_byte_size,
            total_byte_size,
            byte_alignment,
        }
    }

    pub const fn instance_offset(self) -> u32 {
        self.instance_byte_offset
    }

    pub const fn table_offset(self) -> u32 {
        self.table_byte_offset
    }

    pub const fn word_size(self) -> u32 {
        self.word_byte_size
    }

    pub const fn total_size(self) -> u32 {
        self.total_byte_size
    }

    pub const fn align(self) -> u32 {
        self.byte_alignment
    }
}

/// Independently replayed physical source for one whole-root primitive store.
/// It remains separate from scalar-call sources so a new store family cannot
/// widen call or boundary acceptance by representation alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedUnitWriteOnlyPrimitiveStoreSource {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: ScalarType,
        location: crate::AssignedScalarLocation,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    BooleanImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: bool,
    },
    IeeeFloatImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        value: IeeeFloatValue,
    },
    Home(AssignedUnitScalarHome),
}

impl AssignedUnitWriteOnlyPrimitiveStoreSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. }
            | Self::IntegerImmediate { source_value, .. }
            | Self::BooleanImmediate { source_value, .. }
            | Self::IeeeFloatImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::BooleanImmediate { .. } => ScalarType::Boolean,
            Self::IeeeFloatImmediate { value, .. } => ScalarType::IeeeFloat(value.format()),
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedUnitStructuralCasePayload {
    pub field: semantic_vocabulary::StructuralFieldId,
    pub field_byte_offset: u32,
    pub home: AssignedUnitScalarHome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitStructuralCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: semantic_vocabulary::StructuralCaseId,
    pub case_tag: i32,
    pub operation_ordinal: u32,
    pub nominal_return_edge: EdgeId,
    pub payloads: Vec<AssignedUnitStructuralCasePayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedUnitOperation {
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
    WriteOnlyPrimitiveStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        destination_type: StructuralTypeDeclaration,
        destination_placement: ValuePlacement,
        source: AssignedUnitWriteOnlyPrimitiveStoreSource,
    },
    StructuralScalarFieldStore {
        psi_operation: OperationId,
        destination: StructuralParameterDeclaration,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        destination_placement: ValuePlacement,
        field_byte_offset: u32,
        source: AssignedUnitScalarArgumentSource,
    },
    IeeeFloatConstant {
        psi_operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
    NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: OperationId,
        result: ValueId,
        format: IeeeFloatFormat,
        left: AssignedIeeeFloatFmaOperand,
        right: AssignedIeeeFloatFmaOperand,
        addend: AssignedIeeeFloatFmaOperand,
        destination: MachineRegister,
        settlement: target_operations::TargetX86ScalarFmaSettlement,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    EstablishAffineScalarRecord {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        field: semantic_vocabulary::StructuralFieldId,
        value: IntegerValue,
        shape: ValueShape,
    },
    Call {
        psi_operation: OperationId,
        callee: MachineId,
        result: Option<ScalarType>,
        call_plan: CallPlan,
        scalar_arguments: Vec<AssignedUnitScalarCallArgument>,
        /// Scalar transport is absent for aggregate-only calls, whose ABI
        /// transfer is distinct from the mixed scalar transport protocol.
        transport: Option<crate::UnitScalarTransportPlan>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One real in-module fixed-width integer call in an attached Unit body.
    /// The result home survives subsequent call-register clobbers and is the
    /// only accepted source for a later scalar-call argument.
    ScalarCall {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        arguments: Vec<AssignedUnitScalarCallArgument>,
        transport: crate::UnitScalarTransportPlan,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        scalar_arguments: Vec<AssignedUnitScalarCallArgument>,
        transport: crate::UnitScalarTransportPlan,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralResultCall {
        psi_operation: OperationId,
        result: StructuralOperationResult,
        callee: MachineId,
        callee_result: StructuralResultDeclaration,
        call_plan: CallPlan,
        scalar_arguments: Vec<AssignedUnitScalarCallArgument>,
        transport: crate::UnitScalarTransportPlan,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralScalarCallWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        copies: Vec<AssignedAggregateCopy>,
        dynamic_arguments: Vec<AssignedDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralUnitCallWithDynamicArguments {
        psi_operation: OperationId,
        callee: MachineId,
        call_plan: CallPlan,
        copies: Vec<AssignedAggregateCopy>,
        dynamic_arguments: Vec<AssignedDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StoreDynamicDescriptor {
        psi_operation: OperationId,
        stored: AbstractStoredDynamicDescriptor,
        descriptor_abi: AssignedDynamicTraitDescriptorAbi,
        descriptor_home_byte_offset: u32,
        source_copy: AssignedAggregateCopy,
    },
    StoredDynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractStoredDynamicDispatch,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        descriptor_abi: AssignedDynamicTraitDescriptorAbi,
        descriptor_home_byte_offset: u32,
        source_copy: AssignedAggregateCopy,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    DynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        descriptor_abi: AssignedDynamicTraitDescriptorAbi,
        descriptor_home_byte_offset: u32,
        initial_copy: AssignedAggregateCopy,
        rebound_copy: AssignedAggregateCopy,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// One physically assigned rebound Unit invocation. The descriptor frame
    /// region is real ABI custody, while no scalar result home is invented.
    DynamicUnitCall {
        psi_operation: OperationId,
        dynamic_dispatch: AbstractReboundDynamicDispatch,
        call_plan: CallPlan,
        descriptor_abi: AssignedDynamicTraitDescriptorAbi,
        descriptor_home_byte_offset: u32,
        initial_copy: AssignedAggregateCopy,
        rebound_copy: AssignedAggregateCopy,
        requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralCase {
        source: AssignedStructuralHome,
        cases: Vec<AssignedUnitStructuralCaseSuccessor>,
    },
    ConditionalIntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: AssignedUnitScalarArgumentSource,
        right: AssignedUnitScalarArgumentSource,
        when_true: target_operations::TargetUnitConditionalSuccessor,
        when_false: target_operations::TargetUnitConditionalSuccessor,
    },
    ConditionalBoolean {
        condition: AssignedUnitScalarHome,
        when_true: target_operations::TargetUnitConditionalSuccessor,
        when_false: target_operations::TargetUnitConditionalSuccessor,
    },
    /// One Boolean caller parameter after its exact target ABI placement has
    /// been reduced to the physical register or incoming-stack coordinate
    /// consumed by the branch encoder.
    ConditionalBooleanParameter {
        condition: UnitScalarAbiValue,
        location: crate::AssignedScalarLocation,
        when_true: target_operations::TargetUnitConditionalSuccessor,
        when_false: target_operations::TargetUnitConditionalSuccessor,
    },
    ConditionalDispatch {
        fallthrough_edge: EdgeId,
    },
    NonreturningTail {
        psi_edge: EdgeId,
    },
    /// One selected Unit provider invoked through an admitted boundary with
    /// an exact fixed-integer scalar ABI. This is deliberately distinct from
    /// both anonymous internal scalar calls and the optimized structural
    /// installed-provider lane.
    InstalledProviderCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider: ProviderCandidateConformance,
        call_plan: CallPlan,
        scalar_arguments: Vec<AssignedUnitScalarCallArgument>,
        source_arguments: Vec<StructuralArgument>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        completion_claim_sources: Vec<target_operations::CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: target_operations::NormalizedForeignCallBinding,
        scalar_arguments: Vec<AssignedNormalizedForeignScalarArgument>,
        result_home: Option<AssignedUnitScalarHome>,
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
        result: AssignedBoundaryResult,
        execution: target_operations::BoundaryExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
        runtime_scalar_arguments: Vec<AssignedNormalizedForeignScalarArgument>,
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
