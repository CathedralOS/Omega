use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target_operations::{
    AbstractDynamicDescriptorArgument, AbstractReboundDynamicScalarDispatch, AbstractResult,
    BoundaryByteSequenceArgument, BoundaryRealization, BoundaryScalarArgument,
    CompletionClaimSource, FixedIntegerScalarAbiValue, MachineRegister, ProviderExecutionBinding,
    RankedU32CountdownCustody, TargetStructuralParameter,
};
use psi_core::{
    BoundaryMachineId, EdgeId, IeeeFloatFormat, IeeeFloatValue, IntegerType, IntegerValue,
    MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralFieldId, StructuralTypeId,
    ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, CrashRouteBucket, ProviderCandidateConformance,
    StructuralArgument, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

use crate::AssignedCallDestination;

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
    pub scalar_parameters: Vec<FixedIntegerScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<AssignedUnitOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedDynamicDescriptorInstanceArgument {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
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
    pub access: psi_terminal::StructuralAccess,
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

/// Durable physical home assigned to one fixed-width integer value produced
/// by a scalar call in an attached Unit body.
///
/// `byte_offset` is relative to the function's allocated Unit frame. Machine
/// emission independently reconstructs the complete structural-plus-scalar
/// frame and rejects a stale, overlapping, or substituted home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignedUnitScalarHome {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: IntegerType,
    pub shape: ValueShape,
    pub byte_offset: u32,
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

/// Exact physical source of one attached-Unit scalar-call argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedUnitScalarArgumentSource {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: IntegerType,
        location: crate::AssignedScalarLocation,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Home(AssignedUnitScalarHome),
}

impl AssignedUnitScalarArgumentSource {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> IntegerType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::IntegerImmediate { scalar_type, .. } => scalar_type,
            Self::Home(home) => home.scalar_type,
        }
    }
}

/// One positional scalar argument after durable-home assignment. The complete
/// ABI placement remains explicit; it is not reconstructed from register
/// ordinals during emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedUnitScalarCallArgument {
    pub parameter_index: u32,
    pub source: AssignedUnitScalarArgumentSource,
    pub destination: AssignedCallDestination,
}

/// One normalized foreign-call scalar argument after exact durable-home
/// assignment. Unlike an in-module scalar call, the complete evaluated ABI
/// placement remains explicit for later source-free object replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedNormalizedForeignScalarArgument {
    pub parameter_index: u32,
    pub source: AssignedUnitScalarArgumentSource,
    pub placement: ValuePlacement,
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
        settlement: omega_target_operations::TargetX86ScalarFmaSettlement,
    },
    EstablishTrivialAffineLocal {
        psi_operation: OperationId,
        place: StructuralPlaceDeclaration,
        structural_type: StructuralTypeDeclaration,
    },
    Call {
        psi_operation: OperationId,
        callee: MachineId,
        result: Option<ScalarType>,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
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
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        copies: Vec<AssignedAggregateCopy>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    StructuralScalarCallWithDynamicArguments {
        psi_operation: OperationId,
        result: AbstractResult,
        callee: MachineId,
        call_plan: CallPlan,
        copies: Vec<AssignedAggregateCopy>,
        dynamic_arguments: Vec<AssignedDynamicDescriptorArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    DynamicScalarCall {
        psi_operation: OperationId,
        result: AbstractResult,
        dynamic_dispatch: AbstractReboundDynamicScalarDispatch,
        call_plan: CallPlan,
        result_home: AssignedUnitScalarHome,
        descriptor_abi: AssignedDynamicTraitDescriptorAbi,
        descriptor_home_byte_offset: u32,
        initial_copy: AssignedAggregateCopy,
        rebound_copy: AssignedAggregateCopy,
        requirement_obligations: Vec<psi_core::ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    ConditionalIntegerEqual {
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: AssignedUnitScalarArgumentSource,
        right: AssignedUnitScalarArgumentSource,
        when_true: omega_target_operations::TargetUnitConditionalSuccessor,
        when_false: omega_target_operations::TargetUnitConditionalSuccessor,
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
        completion_claim_sources: Vec<omega_target_operations::CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    },
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: omega_target_operations::NormalizedForeignCallBinding,
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
        execution: omega_target_operations::BoundaryExecutionBinding,
        realization: BoundaryRealization,
        scalar_arguments: Vec<BoundaryScalarArgument>,
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
