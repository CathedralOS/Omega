//! Descriptor establishment, forwarding, and dynamic dispatch call evidence.

use crate::{
    InternalUnitCallArgumentRecord, InternalUnitScalarCallResultRecord, ScalarCallStackEvidence,
    UnitCallStackEvidence,
};
use omega_abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractReboundDynamicDispatch, AbstractResult,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch,
};
use omega_calling_conventions::{CallPlan, ValueShape};
use psi_core::{EdgeId, MachineId, OperationId, ValueId};
use psi_terminal::ClaimTransfer;

/// Target-resolved two-word dynamic-trait descriptor geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicTraitDescriptorAbiRecord {
    pub instance_byte_offset: u32,
    pub table_byte_offset: u32,
    pub word_byte_size: u32,
    pub total_byte_size: u32,
    pub byte_alignment: u32,
}

/// Architecture-native address materialization for one object-private
/// conformance table. Offsets identify only mutable relocation fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicTableAddressEncoding {
    X86_64Relative32 {
        relocation_offset: usize,
    },
    Aarch64PageAddress {
        page_relocation_offset: usize,
        page_offset_relocation_offset: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicTableAddressMaterialization {
    pub code_offset: usize,
    pub byte_count: usize,
    pub encoding: DynamicTableAddressEncoding,
}

/// One initializer or rebound source installed into the descriptor's instance
/// word. `source_home_*` binds the emitted address back to the independently
/// retained caller-frame home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicInstanceMaterializationRecord {
    pub selection_ordinal: u32,
    pub source: omega_target_operations::TargetStructuralArgument,
    pub source_home_byte_offset: u32,
    pub source_home_indirect: bool,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Exact physical custody for one rebound descriptor call. Object replay must
/// derive the selected slot from `dynamic_dispatch.application`, validate the
/// immutable bytes around every symbolic field, and materialize the complete
/// table rather than only the selected row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicCallRecord {
    pub psi_operation: OperationId,
    pub dynamic_dispatch: AbstractReboundDynamicDispatch,
    pub call_plan: CallPlan,
    /// Present only for scalar-result requirements. Unit requirements retain
    /// the same descriptor and indirect-call evidence without a synthetic
    /// result record.
    pub result: Option<InternalUnitScalarCallResultRecord>,
    pub descriptor_abi: DynamicTraitDescriptorAbiRecord,
    pub descriptor_home_byte_offset: u32,
    pub initial_instance: DynamicInstanceMaterializationRecord,
    pub table_address: DynamicTableAddressMaterialization,
    pub rebound_instance: DynamicInstanceMaterializationRecord,
    pub argument: InternalUnitCallArgumentRecord,
    pub selected_table_byte_offset: u32,
    pub indirect_call_offset: usize,
    pub indirect_call_byte_count: usize,
    pub unit_stack: UnitCallStackEvidence,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Exact emitted establishment of one aggregate-stored descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDynamicDescriptorMaterializationRecord {
    pub psi_operation: OperationId,
    pub stored: AbstractStoredDynamicDescriptor,
    pub descriptor_abi: DynamicTraitDescriptorAbiRecord,
    pub descriptor_home_byte_offset: u32,
    pub instance: DynamicInstanceMaterializationRecord,
    pub table_address: DynamicTableAddressMaterialization,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Exact later reload and indirect invocation through one previously emitted
/// aggregate-stored descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDynamicCallRecord {
    pub establishment: StoredDynamicDescriptorMaterializationRecord,
    pub psi_operation: OperationId,
    pub dynamic_dispatch: AbstractStoredDynamicDispatch,
    pub call_plan: CallPlan,
    pub result: InternalUnitScalarCallResultRecord,
    pub argument: InternalUnitCallArgumentRecord,
    pub selected_table_byte_offset: u32,
    pub indirect_call_offset: usize,
    pub indirect_call_byte_count: usize,
    pub unit_stack: UnitCallStackEvidence,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Architecture-native indirect-call mechanism selected for one forwarded
/// existential descriptor. The tag is part of physical evidence; replay never
/// infers it from bytes or from the target architecture alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicParameterCallMechanismRecord {
    X86MemoryIndirect {
        table: omega_target_operations::MachineRegister,
    },
    Aarch64LoadedIndirect {
        table: omega_target_operations::MachineRegister,
        target: omega_target_operations::MachineRegister,
    },
}

/// Exact physical custody for a call through one descriptor parameter.
/// The callee-facing plan describes the helper's two-word entry while the
/// dispatch plan describes the erased-data adapter ABI held in the selected
/// table slot. Concrete realization layout is intentionally absent here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicParameterCallRecord {
    pub psi_edge: EdgeId,
    pub psi_operation: OperationId,
    pub source_value: Option<ValueId>,
    pub scalar_type: Option<psi_core::ScalarType>,
    pub parameter: psi_terminal::TerminalDynamicDescriptorParameter,
    pub requirement: psi_terminal::TerminalDynamicRequirement,
    pub function_call_plan: CallPlan,
    pub dispatch_call_plan: CallPlan,
    pub instance: omega_target_operations::MachineRegister,
    pub table: omega_target_operations::MachineRegister,
    pub table_slot_byte_offset: u32,
    pub mechanism: DynamicParameterCallMechanismRecord,
    pub indirect_call_offset: usize,
    pub indirect_call_byte_count: usize,
    pub call_stack: ScalarCallStackEvidence,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedDynamicParameterCallRecord {
    pub psi_edge: EdgeId,
    pub psi_operation: OperationId,
    pub source_value: Option<ValueId>,
    pub scalar_type: Option<psi_core::ScalarType>,
    pub callee: MachineId,
    pub argument: AbstractDynamicDescriptorArgument,
    pub parameter: psi_terminal::TerminalDynamicDescriptorParameter,
    pub function_call_plan: CallPlan,
    pub callee_call_plan: CallPlan,
    pub instance: omega_target_operations::MachineRegister,
    pub table: omega_target_operations::MachineRegister,
    pub instance_destination: omega_target_operations::MachineRegister,
    pub table_destination: omega_target_operations::MachineRegister,
    pub direct_call_offset: usize,
    pub direct_call_byte_count: usize,
    pub call_stack: ForwardedDynamicParameterCallStackEvidence,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardedDynamicParameterCallStackEvidence {
    Unit(UnitCallStackEvidence),
    Scalar(ScalarCallStackEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedDynamicDescriptorArgumentRecord {
    pub custody: AbstractDynamicDescriptorArgument,
    pub instance: omega_target_operations::TargetDynamicDescriptorInstanceArgument,
    pub instance_destination: omega_target_operations::MachineRegister,
    pub table_destination: omega_target_operations::MachineRegister,
    pub source_home_byte_offset: u32,
    pub source_home_indirect: bool,
    pub instance_code_offset: usize,
    pub instance_byte_count: usize,
    pub table_address: DynamicTableAddressMaterialization,
    /// Complete adapter set for this application in canonical row order.
    pub adapters: Vec<ForwardedDynamicDescriptorAdapterRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForwardedDynamicDescriptorAdapterIdentity {
    pub application: psi_terminal::ClosedConformanceApplicationCommitment,
    pub row_index: u32,
    pub realization: MachineId,
}

/// One compiler-generated bridge from the erased one-pointer slot ABI to the
/// concrete realization ABI. It is deliberately not a Terminal machine and
/// therefore carries a role-specific identity rather than a fabricated
/// `MachineId` or callback-thunk identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedDynamicDescriptorAdapterRecord {
    pub identity: ForwardedDynamicDescriptorAdapterIdentity,
    pub requirement_identity: String,
    pub realization_identity: String,
    pub realization_callable_identity: String,
    pub result: psi_terminal::ClosedConformanceCallableResult,
    pub erased_call_plan: CallPlan,
    pub realization_call_plan: CallPlan,
    pub source_shape: ValueShape,
    pub bytes: Vec<u8>,
    pub argument_code_offset: usize,
    pub argument_byte_count: usize,
    pub direct_call_offset: usize,
    pub direct_call_byte_count: usize,
    pub return_offset: usize,
    pub return_byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedDynamicDescriptorCallRecord {
    pub psi_operation: OperationId,
    pub semantic_result: Option<AbstractResult>,
    pub result: Option<InternalUnitScalarCallResultRecord>,
    pub callee: MachineId,
    pub call_plan: CallPlan,
    pub dynamic_arguments: Vec<ForwardedDynamicDescriptorArgumentRecord>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub direct_call_offset: usize,
    pub direct_call_byte_count: usize,
    pub unit_stack: UnitCallStackEvidence,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}
