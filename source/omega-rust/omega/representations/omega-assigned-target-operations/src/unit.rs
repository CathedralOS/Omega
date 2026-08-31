use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target_operations::{
    BoundaryByteSequenceArgument, BoundaryRealization, BoundaryScalarArgument,
    CompletionClaimSource, MachineRegister, NormalizedForeignScalarArgument,
    ProviderExecutionBinding, RankedU32CountdownCustody, TargetStructuralParameter,
};
use psi_core::{
    BoundaryMachineId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, StructuralArgument, StructuralPathSegment,
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
    pub parameters: Vec<TargetStructuralParameter>,
    pub operations: Vec<AssignedUnitOperation>,
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

/// Exact physical source of one attached-Unit scalar-call argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedUnitScalarArgumentSource {
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
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> IntegerType {
        match self {
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
    },
    NormalizedForeignCall {
        psi_operation: OperationId,
        boundary: BoundaryMachineId,
        provider_execution: ProviderExecutionBinding,
        binding: omega_target_operations::NormalizedForeignCallBinding,
        scalar_arguments: Vec<NormalizedForeignScalarArgument>,
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
