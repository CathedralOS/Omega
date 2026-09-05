//! Emitted program functions and separately identified compiler-private functions.

pub mod exit_contract;
pub use exit_contract::*;

use crate::{
    BoundarySettlementRecord, DynamicCallRecord, DynamicParameterCallRecord, ForeignCallRelocation,
    ForwardedDynamicDescriptorCallRecord, ForwardedDynamicParameterCallRecord,
    InstalledProviderUnitScalarCallRecord, InternalCallRelocation, InternalUnitCallRecord,
    InternalUnitScalarCallRecord, MachineCodePlan, PortEffectRecord,
    RankedU32CountdownMachineCodeRecord, ScalarControlAffineCleanupRecord, ScalarStackEvidence,
    ScalarStructuralScalarFieldStoreRecord, SemanticCodeAttribution, StoredDynamicCallRecord,
    StructuralCallScalarReturnEvidence, StructuralReturnRecord, UnitAffineCleanupRecord,
    UnitAffineScalarRecordEstablishmentRecord, UnitIntegerConstantRecord, UnitParameterHomeRecord,
    UnitParameterRecord, UnitScalarFunctionAbiRecord, UnitScalarHomeRecord, UnitStackEvidence,
    UnitStructuralScalarFieldStoreRecord, UnitWriteOnlyPrimitiveStoreRecord,
    X86FloatingControlRecord, X86ScalarFmaFragment, X86ScalarFmaOccurrenceRecord,
};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::TerminalPsiIdentity;

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
    /// into staged attached-Unit structural parameter homes, including
    /// ordinary non-receiver parameters.
    pub unit_structural_scalar_field_stores: Vec<UnitStructuralScalarFieldStoreRecord>,
    /// Exact semantic and physical custody for non-observing immediate or
    /// retained scalar-parameter writes through whole-root primitive borrows.
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
