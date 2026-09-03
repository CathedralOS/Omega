#![forbid(unsafe_code)]

//! Owned target machine code emitted from the clean terminal-Psi realization
//! lane.

mod function_fragments;
mod x86_fma;

pub use function_fragments::*;
pub use x86_fma::*;

use omega_abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractReboundDynamicDispatch, AbstractResult,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch, RankedU32CountdownCustody,
};
use omega_calling_conventions::{CallPlan, ConventionalSumLayout, ValuePlacement, ValueShape};
use omega_target::NativeTarget;
use omega_target_operations::MachineRegister;
use omega_target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundaryScalarArgument, CallSiteOwner,
    CompilerBuiltinExecution, CompletionClaimSource, ProviderExecutionBinding,
    TargetStructuralParameter, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
    ClaimTransfer, CompletionReceipt, ProviderCandidateConformance, StructuralArgument,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<MachineCodeFunction>,
}

/// One emitted compiler-private function whose Terminal machine identity lives
/// in a separate artifact namespace from the program plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerPrivateMachineCodeFunction {
    pub identity: omega_function_identity::MachineFunctionIdentity,
    pub private_symbol: std::sync::Arc<str>,
    pub source_psi: TerminalPsiIdentity,
    pub function: MachineCodeFunction,
}

/// Compatibility wrapper for plans that also own compiler-private functions.
/// Keeping these rows separate prevents an artifact-local `MachineId` from
/// impersonating a semantic program function with the same numeric identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlanWithPrivateFunctions {
    pub plan: MachineCodePlan,
    pub private_functions: Vec<CompilerPrivateMachineCodeFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodeFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
    /// Independently supplied and emission-validated ABI for the bounded
    /// scalar-result function family with fixed-integer and structural
    /// parameters. Results may be a fixed integer or Boolean. Mixed calls must
    /// join this row by exact machine identity; caller records cannot
    /// manufacture it.
    pub mixed_structural_scalar_abi:
        Option<omega_target_operations::MixedStructuralScalarFunctionAbi>,
    /// Exact semantic return evidence for the bounded structural-call/scalar-
    /// return carrier. A Unit body may contain and discard a scalar-producing
    /// call, so object replay must not infer this role from mechanical shape.
    pub structural_call_scalar_return: Option<StructuralCallScalarReturnEvidence>,
    /// Exact Unit-returning fixed-integer input ABI when this function belongs
    /// to the bounded scalar-provider cohort.
    pub unit_scalar_abi: Option<UnitScalarFunctionAbiRecord>,
    pub provenance: TerminalPsiProvenance,
    pub bytes: Vec<u8>,
    /// Feature-requiring scalar FMA3 instruction intervals. These records do
    /// not admit AVX/FMA3; they make the requirement impossible to erase
    /// before independent object replay.
    pub x86_scalar_fma: Vec<X86ScalarFmaFragment>,
    /// Source/Terminal occurrences joined one-to-one to the mechanics
    /// fragments above by exact fragment identity.
    pub x86_scalar_fma_occurrences: Vec<X86ScalarFmaOccurrenceRecord>,
    /// Canonical floating-control save/install/restore custody for functions
    /// that execute IEEE scalar operations.
    pub x86_floating_control: Option<X86FloatingControlRecord>,
    /// Target-emitter-owned stack facts for the aggregate-frame body closure:
    /// Unit bodies and the bounded direct structural-call/scalar-return
    /// carrier. Other terminal function forms remain deliberately unreported
    /// until their complete temporary-stack accounting is retained.
    pub unit_stack: Option<UnitStackEvidence>,
    /// Complete ordered incoming structural-parameter homes for an aggregate-
    /// frame body.
    /// Object validation binds projected-call custody to this independently
    /// retained caller frame plan instead of trusting per-call offsets.
    pub unit_parameter_homes: Vec<UnitParameterHomeRecord>,
    /// Independent ordered semantic signature for an aggregate-frame body. Object
    /// validation binds each mutable ABI home back to this declaration row.
    pub unit_parameters: Vec<UnitParameterRecord>,
    /// Exact ordered stack mutations and admitted control-flow shape for a
    /// scalar function. Object construction replays the target instructions
    /// and derives the numeric peak; unsupported scalar forms remain `None`.
    pub scalar_stack: Option<ScalarStackEvidence>,
    /// Typed internal-call relocation fields, ordered by `offset`. Each row
    /// points at the mutable immediate bits of one architecture-native call;
    /// object construction validates the surrounding opcode before accepting
    /// the relocation.
    pub internal_calls: Vec<InternalCallRelocation>,
    /// Source-free foreign call sites whose physical locator was already
    /// normalized against the selected target. Unlike an internal call, the
    /// target is one atomic object-format locator rather than a semantic
    /// machine identity. Object construction must replay the exact native call
    /// placeholder before it can publish an unresolved import symbol and
    /// relocation.
    pub foreign_calls: Vec<ForeignCallRelocation>,
    /// Complete ordered semantic and ABI custody for in-module Unit calls.
    pub internal_unit_calls: Vec<InternalUnitCallRecord>,
    /// Complete ordered custody for fixed-width scalar calls executed inside
    /// attached Unit bodies. These rows are distinct from aggregate-copy Unit
    /// calls because scalar values have `ValueId` homes rather than `PlaceId`
    /// projections.
    pub internal_unit_scalar_calls: Vec<InternalUnitScalarCallRecord>,
    /// Selected-provider Unit calls whose scalar ABI has no result. These
    /// remain distinct from anonymous internal scalar calls so provider and
    /// completion authority survive object replay.
    pub installed_provider_unit_scalar_calls: Vec<InstalledProviderUnitScalarCallRecord>,
    /// Complete descriptor, table-address, receiver-copy, and indirect-call
    /// custody for rebound named-dynamic calls in attached Unit bodies.
    /// The table contents remain semantic demands until object construction
    /// binds every canonical row to an exact function symbol.
    pub dynamic_calls: Vec<DynamicCallRecord>,
    /// Split establishment/reload custody for descriptors stored in local
    /// aggregates. Each row binds the earlier materialization bytes and table
    /// relocation to the later indirect call using one shared descriptor home.
    pub stored_dynamic_calls: Vec<StoredDynamicCallRecord>,
    /// Complete semantic, ABI, register, slot, stack, and byte custody for
    /// scalar calls through an existential descriptor received as a function
    /// parameter. Unlike `dynamic_calls`, these rows do not materialize
    /// or relocate a table: the caller supplies both descriptor words.
    pub dynamic_parameter_calls: Vec<DynamicParameterCallRecord>,
    /// Complete direct-call custody for a scalar helper that forwards its
    /// incoming existential descriptor parameter unchanged to another helper.
    /// No local table or instance address is materialized by this call.
    pub forwarded_dynamic_parameter_calls: Vec<ForwardedDynamicParameterCallRecord>,
    /// Complete caller-side materialization and direct-call custody for
    /// existential descriptors passed to another Terminal machine.
    pub forwarded_dynamic_descriptor_calls: Vec<ForwardedDynamicDescriptorCallRecord>,
    /// Complete ordered durable scalar homes in an attached Unit frame.
    pub unit_scalar_homes: Vec<UnitScalarHomeRecord>,
    /// Complete ordered zero-code integer definitions available to attached
    /// Unit scalar calls. These rows let object replay distinguish an authored
    /// constant from a scalar-call result without trusting the call child.
    pub unit_integer_constants: Vec<UnitIntegerConstantRecord>,
    /// Exact zero-code affine-record constructors whose bits are materialized
    /// only at their sole owned Unit-call use.
    pub unit_affine_scalar_records: Vec<UnitAffineScalarRecordEstablishmentRecord>,
    /// Exact semantic and physical custody for fixed-width immediate writes
    /// into staged attached-Unit structural parameter homes.
    pub unit_structural_scalar_field_stores: Vec<UnitStructuralScalarFieldStoreRecord>,
    /// Exact semantic and physical custody for non-observing immediate writes
    /// through whole-root primitive borrowed parameters.
    pub unit_write_only_primitive_stores: Vec<UnitWriteOnlyPrimitiveStoreRecord>,
    /// Exact one-store prefix for the bounded mutable-self scalar-return
    /// carrier. Unlike Unit stores, this writes through the incoming borrowed
    /// reference directly; no staged aggregate home or value copy exists.
    pub scalar_structural_scalar_field_stores: Vec<ScalarStructuralScalarFieldStoreRecord>,
    /// Exact zero-code affine-local establishment and Unit-return cleanup
    /// custody for the bounded one-state Unit slice.
    pub unit_affine_cleanup: Option<UnitAffineCleanupRecord>,
    /// Structural custody consumed by a scalar return after its result has
    /// been materialized. The record deliberately reuses the exact cleanup
    /// vocabulary while remaining distinct from a Unit body.
    pub scalar_affine_cleanup: Option<UnitAffineCleanupRecord>,
    /// Canonical true-before-false DFS leaves for the exact bounded
    /// two-decision/three-return Boolean-control carrier. Each row binds one
    /// real terminal-Psi return edge and physical cleanup suffix to the
    /// independently replayable result/link preservation for that suffix.
    /// Branch-free scalar cleanup continues to use `scalar_affine_cleanup`.
    pub scalar_control_affine_cleanups: Vec<ScalarControlAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<UnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<UnitParameterHomeRecord>,
    /// Exact first-slice ranked countdown custody and its target byte layout.
    /// Object construction remains fail-closed until it independently replays
    /// this record; ordinary scalar/control evidence cannot stand in for it.
    pub ranked_u32_countdown: Option<RankedU32CountdownMachineCodeRecord>,
    /// Exact semantic operation/edge ownership of emitted byte intervals.
    pub semantic_code_attribution: Vec<SemanticCodeAttribution>,
    /// Privileged effects retained with their exact semantic service and byte
    /// range. Installation can therefore bind emitted instructions to the
    /// selected provider execution instead of inferring privilege from bytes.
    pub port_effects: Vec<PortEffectRecord>,
    /// Verified metadata-only boundary settlements retained at their exact
    /// code position. They emit no duplicate hardware effect.
    pub boundary_settlements: Vec<BoundarySettlementRecord>,
    /// Exact ownership-bearing structural return bound to the emitted byte
    /// interval. Claim identities are semantic metadata, never hidden ABI
    /// words.
    pub structural_return: Option<StructuralReturnRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralCallScalarReturnEvidence {
    pub psi_edge: EdgeId,
    pub psi_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub callee: MachineId,
}

/// Exact source-free custody for one call to a normalized foreign locator.
///
/// `offset` names the mutable relocation field: the four-byte displacement
/// following x86-64 `CALL rel32`, or the complete AArch64 `BL` instruction.
/// Raw object/symbol/version bytes remain private inside the normalized locator
/// and are never reconstructed from an Omega or object-local symbol name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallRelocation {
    pub owner: CallSiteOwner,
    /// Ordinal of the exact attached-Unit operation that owns this call.
    pub operation_ordinal: usize,
    pub offset: usize,
    pub locator: omega_target::NormalizedForeignLocator,
    pub provider_execution: ProviderExecutionRecord,
    /// Exact admitted boundary contract consumed before assignment.
    pub boundary_entry_plan: omega_calling_conventions::BoundaryEntryPlan,
    /// Exact source-selected ABI plan consumed to emit this call.
    pub call_plan: omega_calling_conventions::CallPlan,
    /// Canonically ordered evaluated scalar arguments materialized before the
    /// unresolved native procedure-call placeholder. Each source is either an
    /// exact authored constant or an exact preceding scalar-result home. The
    /// bounded native carrier admits the target's complete register-resident
    /// fixed-integer argument bank.
    pub scalar_arguments: Vec<ForeignCallScalarArgumentRecord>,
    /// Exact compiler-private function-address materialization occupying one
    /// native-only callback parameter of this registrar call.
    pub callback_address: Option<CallbackAddressMaterialization>,
    /// Optional fixed-integer result normalized from its evaluated ABI
    /// placement and stored in a durable attached-Unit scalar home.
    pub scalar_result: Option<ForeignCallScalarResultRecord>,
    /// Complete MXCSR preservation around this returning x86 foreign call.
    pub x86_floating_control: Option<X86ForeignCallFloatingControlRecord>,
    /// Complete FPCR preservation around this returning AArch64 foreign call.
    pub aarch64_floating_control: Option<Aarch64ForeignCallFloatingControlRecord>,
    /// Byte-addressed outbound stack custody plus the independently admitted
    /// opaque same-stack contribution for the foreign leaf.
    pub unit_stack: UnitCallStackEvidence,
    pub same_stack_contribution: omega_task_plans::AdmittedSameStackContribution,
}

/// Physical destination retained without inventing a semantic callback value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAddressDestination {
    Register(omega_target_operations::MachineRegister),
    OutgoingStack { byte_offset: u32 },
}

/// Architecture-native symbolic address encoding. Offsets are relative to the
/// containing machine-code function and identify only mutable relocation
/// fields; every surrounding instruction bit remains final-byte checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAddressEncoding {
    X86_64Relative32 {
        relocation_offset: usize,
    },
    Aarch64PageAddress {
        page_relocation_offset: usize,
        page_offset_relocation_offset: usize,
    },
}

/// Source-free custody for one callback function address loaded immediately
/// before the exact normalized registrar call that consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackAddressMaterialization {
    pub target: omega_target_operations::TargetNativeCallbackArgument,
    pub destination: CallbackAddressDestination,
    pub code_offset: usize,
    pub byte_count: usize,
    pub encoding: CallbackAddressEncoding,
}

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

/// Per-call proof that one returning AArch64 foreign boundary preserved the
/// caller's complete FPCR. The slot may be reused by sequential calls, while
/// each call retains distinct save/restore instruction intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ForeignCallFloatingControlRecord {
    pub target: omega_target::NativeTarget,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}

/// Exact source-free custody for one normalized foreign scalar result.
///
/// The byte interval begins after the native call and any outbound-stack
/// release. It covers only canonical result normalization and the durable-home
/// store, never the mutable import relocation field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallScalarResultRecord {
    pub home: UnitScalarHomeRecord,
    pub source: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Exact occurrence-specific source and ABI custody for one evaluated foreign-
/// call scalar value. The byte interval names only its register
/// materialization; the unresolved call field remains owned by
/// [`ForeignCallRelocation::offset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallScalarArgumentRecord {
    pub parameter_index: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub placement: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Complete machine-code custody for the one admitted structural Unit / `u32`
/// countdown. Target layout is deliberately not copied here: object replay
/// must derive it independently from the target's canonical encoding and bind
/// the generic fuel rows to that result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedU32CountdownMachineCodeRecord {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAffineCleanupRecord {
    pub psi_edge: EdgeId,
    /// Canonical verifier-owned type closure retained so object and installed
    /// artifact validation can reconstruct the exact residual partition.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact semantic execution order for every cleanup committed by this edge.
    pub actions: Vec<TerminalAffineCleanupAction>,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarControlAffineCleanupRecord {
    pub cleanup: UnitAffineCleanupRecord,
    pub preservation: ScalarCleanupPreservationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub byte_offset: u32,
    pub indirect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParameterRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    pub shape: ValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralReturnRecord {
    pub psi_edge: EdgeId,
    /// Ordered fixed-integer prefix preceding the structural input roster.
    /// Empty for the established claim-bearing structural-only family.
    pub scalar_parameters: Vec<omega_target_operations::FixedIntegerScalarAbiValue>,
    /// Complete ordered structural input signature. This binds the returned
    /// place and every zero-code cleanup place to its exact type/multiplicity.
    pub parameters: Vec<StructuralParameterDeclaration>,
    /// Structural ABI placements corresponding one-for-one with `parameters`;
    /// scalar-prefix placements live in `scalar_parameters`.
    pub parameter_placements: Vec<ValuePlacement>,
    pub source: StructuralParameterDeclaration,
    pub result: StructuralResultDeclaration,
    pub shape: ValueShape,
    pub source_placement: ValuePlacement,
    pub result_placement: ValuePlacement,
    pub returned_claims: Vec<ClaimId>,
    /// Typed no-ABI local declarations. These never receive a placement and
    /// therefore cannot silently become runtime inputs.
    pub trivial_affine_locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact verifier-owned reverse-declaration no-code cleanup order.
    pub trivial_affine_discards: Vec<PlaceId>,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Semantic operation or edge owning one exact emitted byte interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticCodeSite {
    Operation(OperationId),
    Edge(EdgeId),
}

/// Source-free semantic-to-code custody used by independent object replay.
/// It carries no runtime budget, charge, meter, or execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticCodeAttribution {
    pub site: SemanticCodeSite,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryScalarResultRecord {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub placement: ValuePlacement,
    /// Exact terminal edge that returns this boundary-produced value.
    pub return_edge: EdgeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryStructuralResultRecord {
    pub defining_operation: OperationId,
    pub result: StructuralOperationResult,
    pub layout: ConventionalSumLayout,
    /// Offset within the complete allocated Unit frame.
    pub home_byte_offset: u32,
}

/// Closed physical result role for one emitted boundary settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryResultRecord {
    Unit,
    Scalar(BoundaryScalarResultRecord),
    Structural(BoundaryStructuralResultRecord),
}

impl BoundaryResultRecord {
    pub const fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub const fn scalar(&self) -> Option<&BoundaryScalarResultRecord> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&BoundaryStructuralResultRecord> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySettlementRecord {
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub execution: BoundaryExecutionRecord,
    pub realization: BoundaryRealization,
    /// Ordered scalar inputs consumed by executable boundary code. Metadata-
    /// only settlements retain an empty list; native realizations retain the
    /// exact terminal value, type, immediate, and ABI destination.
    pub scalar_arguments: Vec<BoundaryScalarArgument>,
    /// Exact assigned source and emitted materialization for returning
    /// compiler-builtin scalar inputs.
    pub runtime_scalar_arguments: Vec<ForeignCallScalarArgumentRecord>,
    /// Exact typed Psi custody arguments, including structural projections.
    /// These are provider-settlement evidence and do not describe an internal
    /// Unit-call ABI.
    pub arguments: Vec<StructuralArgument>,
    /// Exact native byte-sequence arguments and their independently replayable
    /// semantic and physical custody.
    pub byte_sequence_arguments: Vec<BoundaryByteSequenceArgumentRecord>,
    /// Complete canonical caller claim-source catalog needed to independently
    /// reconstruct the exact successful-completion receipt set.
    pub completion_claim_sources: Vec<CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    /// Exact structural join between each successful-completion receipt, its
    /// retained caller source, and the admitted provider execution that owns
    /// this settlement. This is custody evidence only: it does not authorize
    /// content introduction, prove backing, or derive residual geometry.
    pub completion_provider_custody: Vec<CompletionProviderCustodyBinding>,
    /// Exact closed physical result role. Unit settlements cannot manufacture
    /// a scalar or structural result after lowering.
    pub native_result: BoundaryResultRecord,
    /// Position in the verified Unit operation sequence. This remains the
    /// canonical tie-break when multiple metadata rows share a code offset.
    pub operation_ordinal: usize,
    /// Byte offset immediately after all preceding executable operations.
    pub code_offset: usize,
    /// Zero for metadata-only settlements; otherwise the exact provider
    /// instruction interval that produces the boundary result.
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryByteSequenceArgumentRecord {
    pub argument: StructuralArgument,
    pub literal_operation: OperationId,
    pub structural_type: StructuralTypeDeclaration,
    pub bytes: Vec<u8>,
    /// Function-relative executable interval that consumes this payload.
    pub code_offset: usize,
    pub code_byte_count: usize,
    /// Function-relative exact literal-plus-newline interval.
    pub data_offset: usize,
    pub data_byte_count: usize,
}

/// Non-authoritative artifact custody for one exact completion receipt.
///
/// The row deliberately repeats the exact source and provider-execution
/// records instead of replacing either with a producer-authored aggregate or
/// authorization fingerprint. Object and installation validation rederive the
/// complete ordered catalog from the enclosing settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionProviderCustodyBinding {
    pub source: CompletionClaimSource,
    pub receipt: CompletionReceipt,
    pub provider_execution: ProviderExecutionRecord,
}

/// Source-free physical custody for one realized boundary execution role.
/// Compiler builtins remain structural catalog identities and never acquire
/// provider-execution report coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryExecutionRecord {
    AdmittedProvider(ProviderExecutionRecord),
    CompilerBuiltin(CompilerBuiltinExecution),
}

impl From<BoundaryExecutionBinding> for BoundaryExecutionRecord {
    fn from(binding: BoundaryExecutionBinding) -> Self {
        match binding {
            BoundaryExecutionBinding::AdmittedProvider(execution) => {
                Self::AdmittedProvider(execution.into())
            }
            BoundaryExecutionBinding::CompilerBuiltin(execution) => {
                Self::CompilerBuiltin(execution)
            }
        }
    }
}

pub fn derive_completion_provider_custody(
    execution: BoundaryExecutionRecord,
    sources: &[CompletionClaimSource],
    receipts: &[CompletionReceipt],
) -> Option<Vec<CompletionProviderCustodyBinding>> {
    let BoundaryExecutionRecord::AdmittedProvider(provider_execution) = execution else {
        return (sources.is_empty() && receipts.is_empty()).then(Vec::new);
    };
    receipts
        .iter()
        .map(|receipt| {
            let source = sources
                .iter()
                .find(|source| source.claim() == receipt.claim)?;
            Some(CompletionProviderCustodyBinding {
                source: source.clone(),
                receipt: *receipt,
                provider_execution,
            })
        })
        .collect()
}

/// Non-authoritative serialized projection of an admitted provider execution.
/// This can be decoded for validation/reporting but cannot be used to invoke
/// target lowering, which requires the ledger-owned admitted binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExecutionRecord {
    pub provider_plan_report_identity: u64,
    pub provider_execution_report_identity: u64,
    pub provider_execution_report_fingerprint: u64,
    pub normalized_root_report_identity: u64,
    pub boundary_contract_report_fingerprint: u64,
}

impl ProviderExecutionRecord {
    pub fn new(
        provider_plan_report_identity: u64,
        provider_execution_report_identity: u64,
        provider_execution_report_fingerprint: u64,
        normalized_root_report_identity: u64,
        boundary_contract_report_fingerprint: u64,
    ) -> Option<Self> {
        [
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        ]
        .iter()
        .all(|identity| *identity != 0)
        .then_some(Self {
            provider_plan_report_identity,
            provider_execution_report_identity,
            provider_execution_report_fingerprint,
            normalized_root_report_identity,
            boundary_contract_report_fingerprint,
        })
    }
}

impl From<ProviderExecutionBinding> for ProviderExecutionRecord {
    fn from(binding: ProviderExecutionBinding) -> Self {
        Self {
            provider_plan_report_identity: binding.provider_plan_report_identity().get(),
            provider_execution_report_identity: binding.provider_execution_report_identity(),
            provider_execution_report_fingerprint: binding.provider_execution_report_fingerprint(),
            normalized_root_report_identity: binding.normalized_root_report_identity(),
            boundary_contract_report_fingerprint: binding.boundary_contract_report_fingerprint(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortEffectRecord {
    pub psi_operation: OperationId,
    pub service: ServiceId,
    pub port: u16,
    pub value: u8,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalCallRelocation {
    /// Exact semantic owner of the encoded call. Cleanup-edge calls retain an
    /// [`EdgeId`] here instead of borrowing or inventing an operation identity.
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// Exact caller-owned stack live immediately after this aggregate-frame
    /// call enters its callee. Absent for function forms whose full stack
    /// accounting has not yet migrated.
    pub unit_stack: Option<UnitCallStackEvidence>,
    /// Exact scalar-expression call-site stack facts. The object boundary
    /// replays the surrounding ordered mutations and derives caller-live
    /// bytes; this evidence names only the encoded outbound area and, on
    /// AArch64, the explicit link-register save/restore.
    pub scalar_stack: Option<ScalarCallStackEvidence>,
    /// Byte offset within this function at which the relocation field begins.
    /// On x86-64 this points at the four-byte displacement following `CALL`;
    /// on AArch64 it points at the `BL` instruction word.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitCallRecord {
    /// Must match the owner of the corresponding relocation exactly.
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// `None` for a value-less structural call; otherwise the exact scalar
    /// result returned through the ordinary target ABI.
    pub result: Option<ScalarType>,
    /// Exact Terminal scalar value produced by this call when later semantic
    /// custody needs to distinguish production from return. The ABI result
    /// above remains the independently replayed physical type.
    pub semantic_result: Option<AbstractResult>,
    /// Exact structural result custody for the bounded direct-return call
    /// family. Mutually exclusive with `result`; absent on Unit/scalar calls.
    pub structural_result: Option<InternalStructuralCallResult>,
    /// Exact fixed-integer arguments in the scalar prefix of the shared call
    /// plan. Aggregate-only calls retain the canonical empty roster.
    pub scalar_arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub arguments: Vec<InternalUnitCallArgumentRecord>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalStructuralCallResult {
    pub operation_result: psi_terminal::StructuralOperationResult,
    pub function_result: StructuralResultDeclaration,
    pub returned_claim_transfers: Vec<psi_terminal::StructuralResultClaimTransfer>,
    pub returned_claims: Vec<ClaimId>,
    pub caller_result_placement: ValuePlacement,
    pub callee_result_placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitCallArgumentRecord {
    pub place: PlaceId,
    pub access: psi_terminal::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub source_home_byte_offset: u32,
    pub call_stack_bytes: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub source: ValuePlacement,
    pub destination: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
    /// Immutable target bytes that realize this exact source-to-destination copy.
    pub bytes: Vec<u8>,
}

/// One durable scalar home in an attached Unit frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitScalarHomeRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<omega_target_operations::UnitScalarAbiValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitScalarParameterLocationRecord {
    Register(MachineRegister),
    IncomingStack { byte_offset: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitIntegerConstantRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub operation_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAffineScalarRecordEstablishmentRecord {
    pub psi_operation: OperationId,
    pub result: psi_terminal::StructuralOperationResult,
    pub field: psi_core::StructuralFieldId,
    pub value: IntegerValue,
    pub shape: ValueShape,
    pub operation_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: psi_terminal::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: psi_core::StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitWriteOnlyPrimitiveStoreRecord {
    pub psi_operation: OperationId,
    pub destination: psi_terminal::StructuralParameterDeclaration,
    pub destination_type: psi_terminal::StructuralTypeDeclaration,
    pub destination_placement: ValuePlacement,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub parameter_home_byte_offset: u32,
    pub parameter_home_indirect: bool,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStructuralScalarFieldStoreRecord {
    pub psi_operation: OperationId,
    pub destination: psi_terminal::StructuralParameterDeclaration,
    pub path: Vec<StructuralPathSegment>,
    pub field: psi_core::StructuralFieldId,
    pub destination_placement: ValuePlacement,
    pub field_byte_offset: u32,
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub immediate: omega_target_operations::TargetScalarImmediate,
    pub return_operation: OperationId,
    pub return_source_value: ValueId,
    pub return_field: psi_core::StructuralFieldId,
    pub return_field_byte_offset: u32,
    pub return_scalar_type: ScalarType,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

/// Exact semantic and physical source of one attached-Unit scalar argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalUnitScalarArgumentSourceRecord {
    Parameter {
        parameter_index: u32,
        source_value: ValueId,
        scalar_type: IntegerType,
        location: UnitScalarParameterLocationRecord,
    },
    IntegerImmediate {
        defining_operation: OperationId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Home(UnitScalarHomeRecord),
}

impl InternalUnitScalarArgumentSourceRecord {
    pub const fn source_value(self) -> ValueId {
        match self {
            Self::Parameter { source_value, .. } => source_value,
            Self::IntegerImmediate { source_value, .. } => source_value,
            Self::Home(home) => home.source_value,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IntegerImmediate { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::Home(home) => home.scalar_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitScalarCallArgumentRecord {
    pub parameter_index: u32,
    pub source: InternalUnitScalarArgumentSourceRecord,
    pub destination: ValuePlacement,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Compatibility name for the shared scalar-result custody record. Internal
/// and normalized foreign calls deliberately use one result/home vocabulary.
pub type InternalUnitScalarCallResultRecord = ForeignCallScalarResultRecord;

/// Real emitted fixed-width integer call in an attached Unit body.
///
/// The relocation validates the call instruction and target; this record owns
/// the argument and result-store bytes around it. Object construction must
/// independently reconstruct every byte before admitting the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUnitScalarCallRecord {
    pub owner: CallSiteOwner,
    pub target: MachineId,
    pub call_plan: CallPlan,
    pub result: InternalUnitScalarCallResultRecord,
    pub arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// One emitted Unit-returning fixed-i32 call through an exact selected
/// provider. The internal relocation owns the mutable call field; this row
/// owns the complete semantic selection and scalar argument interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProviderUnitScalarCallRecord {
    pub owner: CallSiteOwner,
    pub boundary: BoundaryMachineId,
    pub provider: ProviderCandidateConformance,
    pub call_plan: CallPlan,
    pub arguments: Vec<InternalUnitScalarCallArgumentRecord>,
    pub source_arguments: Vec<StructuralArgument>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub completion_claim_sources: Vec<omega_target_operations::CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitStackEvidence {
    /// Exact function-lifetime stack allocation and matching release. The
    /// object boundary validates both target encodings before deriving any
    /// numeric stack demand. `None` is valid only for an x86-64 Unit leaf with
    /// no parameter-home frame.
    pub frame: Option<StackAdjustmentPair>,
    /// AArch64 Unit functions retain the incoming link register in their
    /// function-lifetime frame. Both accesses are validated against the exact
    /// encoded instructions; x86-64 uses the implicit CALL/RET stack link.
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitCallStackEvidence {
    /// Exact outgoing argument/shadow allocation and matching release around
    /// this call. The object boundary derives the transient contribution from
    /// these validated target instructions plus architecture-owned linkage.
    pub outbound: Option<StackAdjustmentPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCallStackEvidence {
    pub outbound: Option<StackAdjustmentPair>,
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackAdjustmentPair {
    pub byte_size: u32,
    pub allocation_offset: usize,
    pub allocation_byte_count: usize,
    pub release_offset: usize,
    pub release_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ReturnLinkEvidence {
    pub frame_byte_offset: u32,
    pub store_offset: usize,
    pub load_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarStackEvidence {
    pub mutations: Vec<ScalarStackMutation>,
    pub control_flow: ScalarControlFlowEvidence,
    pub stack_alignment: u32,
    /// Exact ABI-result preservation around an appended scalar-return cleanup
    /// suffix. Ordinary scalar functions have no such suffix and retain
    /// `None`; object construction validates every named access independently
    /// from the generic mutation trace.
    pub cleanup_preservation: Option<ScalarCleanupPreservationEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCleanupPreservationEvidence {
    pub frame: StackAdjustmentPair,
    pub result_byte_offset: u32,
    pub result_store_offset: usize,
    pub result_load_offset: usize,
    /// AArch64 additionally preserves the caller's link register in the same
    /// lifetime frame. X86-64 uses its implicit stack link and retains `None`.
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarControlFlowEvidence {
    Linear,
    /// A direct scalar return containing one or more compiler-generated x86-64
    /// signed division/remainder diamonds. Each branch selects the `-1`
    /// overflow handling arm or the ordinary DIV/IDIV arm, and both paths
    /// reconverge before expression evaluation continues. The object boundary
    /// validates both branch targets and replays the two stack paths
    /// independently.
    LinearWithDivisionBranches {
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    /// One acyclic Boolean decision tree whose branches are retained in
    /// increasing physical code order. Its terminal bitmap is in physical
    /// true-before-false DFS order and therefore contains exactly one more
    /// entry than `decisions`. Ordered x86 division diamonds are partitioned
    /// across decision prefixes and returning leaves during object replay.
    ConditionalTree {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        crash_leaves: Vec<bool>,
        branches: Vec<ScalarDivisionBranchEvidence>,
    },
    /// One native Boolean decision tree whose ordered value leaves reconverge
    /// at an exact shared return/cleanup tail. Every non-final leaf ends in one
    /// retained unconditional branch targeting `merge_offset`; the final leaf
    /// falls through to that same offset.
    BooleanSharedConvergence {
        decisions: Vec<ScalarConditionalBranchEvidence>,
        joins: Vec<ScalarJoinBranchEvidence>,
        /// Exact source return edges in physical true-before-false leaf order.
        /// One source convergence block has one row even when several value
        /// leaves reach it; otherwise each uniform source return is retained.
        return_edges: Vec<EdgeId>,
        /// Source return edge of the final physical leaf, which falls through
        /// rather than owning a [`ScalarJoinBranchEvidence`] row.
        fallthrough_return_edge: EdgeId,
        /// Exact emitted condition regions containing structural-field reads.
        /// Object replay checks these bytes independently from the generic
        /// scalar instruction walk before accepting the shared tail.
        structural_conditions: Vec<BooleanStructuralConditionEvidence>,
        merge_offset: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanStructuralConditionEvidence {
    pub reads: Vec<BooleanStructuralFieldRead>,
    pub code_offset: usize,
    pub byte_count: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanStructuralFieldRead {
    pub psi_operation: OperationId,
    pub source: PlaceId,
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    /// Exact native interval which loads this field and normalizes it to a
    /// Boolean result. Object replay reconstructs these bytes independently
    /// from the retained structural home and canonical layout.
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarJoinBranchEvidence {
    /// Exact source return edge whose value leaf owns this native join.
    pub return_edge: EdgeId,
    pub join_offset: usize,
    pub join_byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarDivisionBranchEvidence {
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub ordinary_arm_offset: usize,
    pub join_offset: usize,
    pub join_byte_count: usize,
    pub merge_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarConditionalBranchEvidence {
    pub condition: ScalarConditionalCondition,
    pub branch_offset: usize,
    pub branch_byte_count: usize,
    pub false_arm_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarConditionalCondition {
    Parameter,
    Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarStackMutation {
    pub offset: usize,
    pub byte_count: usize,
    pub kind: ScalarStackMutationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStackMutationKind {
    Allocate { byte_size: u32 },
    Release { byte_size: u32 },
    X86ReleasePreservingFlags { byte_size: u32 },
    X86Push,
    X86Pop,
}
