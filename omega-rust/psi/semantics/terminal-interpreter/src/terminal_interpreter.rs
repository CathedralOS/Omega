//! Fuel-bounded reference execution for verified terminal-Psi artifacts.
//!
//! The public entry accepts only canonical semantic/proof bytes and an
//! admission profile. It decodes and verifies those bytes before constructing
//! execution state; no source or checked-tree representation crosses this
//! boundary.

mod call_frames;
mod custody;
mod scalar_operations;
use custody::{
    bind_affine_frontier, bind_affine_frontier_types, bind_arguments, bind_boundary_arguments,
    bind_entry_claims, bind_structural_arguments, bind_structural_primitive_values,
    commit_cleanup_actions, complete_claims, consume_affine_projection, direct_scalar_field_type,
    has_live_linear_claims, rebind_structural_result_claims, remove_affine_root,
    resolve_structural_arguments, resolve_structural_path_type, transfer_claims,
    validate_boundary_requirements,
};
use scalar_operations::{
    ieee_float_compare, nearest_ieee_float_fused_multiply_add, terminal_scalar_belongs_to_type,
};
mod block_bindings;
mod boundary_byte_buffers;
mod byte_sequence_binding;
mod byte_sequence_subslice;
use byte_sequence_binding::{ByteSequenceBinding, StructuralCallArguments};
mod byte_sequence_view;
mod byte_sequence_write;
mod case_membership;
use case_membership::StructuralCaseContents;
pub use case_membership::TerminalStructuralCaseValue;
mod record;
mod reference;
mod scalar_array;
mod scalar_case_arguments;
mod structural_byte_arrays;
pub use scalar_array::{TerminalScalarArrayResult, TerminalScalarArrayValue};
pub use structural_byte_arrays::TerminalStructuralByteArrayValue;
mod effect_results;
mod primitive_storage;
mod structural_byte_sequence_index_store;
mod structural_byte_sequence_store;
mod structural_scalar_fields;
use byte_sequence_view::ByteSequenceView;
mod semantic_value_comparison;
#[cfg(test)]
mod structural_argument_binding_tests;

pub use boundary_byte_buffers::TerminalBoundaryByteBuffer;
pub use effect_results::TerminalEffectResult;
pub use structural_scalar_fields::TerminalStructuralScalarFieldValue;

/// Exact initialized contents supplied by the embedding host for structural
/// entry arguments. All paths remain rooted in the original referents.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    pub scalar_fields: &'input [TerminalStructuralScalarFieldValue],
    pub primitive_values: &'input [TerminalStructuralPrimitiveValue],
    pub cases: &'input [TerminalStructuralCaseValue],
}

pub use semantic_value_comparison::{
    TerminalTraceScalarComparisonError, TerminalTraceScalarValueSide,
    TerminalTraceStructuralComparisonError, TerminalTraceStructuralValueSide,
    compare_terminal_trace_scalar_values, compare_terminal_trace_structural_values,
};

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, IeeeFloatValue, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_fuel::{FuelExhaustion, FuelMeterError, TerminalFuelMeter, TerminalFuelUsage};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, ClaimTransfer, CompletionReceipt,
    CrashCause, EntryClaim, NominalAffineCleanup, OperationKind, OperationResult, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachineResult, Terminator,
};

/// Decode, verify, and execute the canonical semantic and proof sections of one
/// terminal-Psi artifact. This is the reference-interpreter trust boundary for
/// executable artifact content: no source, checked tree, producer-owned module,
/// or prevalidated Rust object crosses it. Installation and debug sections are
/// separately bound by the artifact manifest and do not affect interpretation.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut handler = AcceptTerminalEffects;
    interpret_terminal_artifact_with_effect_handler_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        arguments,
        &[],
        &mut handler,
    )
}

/// Decode one complete portable Terminal-Psi envelope, independently verify
/// its semantic and proof sections, and execute it with a fresh effect-policy
/// input supplied by the receiver. The envelope contains no checked-tree or
/// build-process object.
pub fn interpret_serialized_terminal_artifact_with_effect_handler_measured(
    artifact_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(artifact_bytes)
        .map_err(TerminalArtifactInterpretError::ArtifactDecode)?;
    interpret_terminal_artifact_with_effect_handler_measured(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        scalar_arguments,
        structural_arguments,
        handler,
    )
}

/// Execute one verified artifact with opaque structural runtime arguments and
/// an injected deterministic effect handler. The interpreter records every
/// accepted effect in semantic execution order; the handler cannot inspect or
/// mutate fuel, values, claims, or control state.
pub fn interpret_terminal_artifact_with_effect_handler_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_boolean_fields_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        &[],
        handler,
    )
}

/// Execute with exact target-neutral values for direct Boolean fields of
/// structural entry arguments. Field IDs are terminal semantic identities;
/// this input never exposes or assumes native layout.
pub fn interpret_terminal_artifact_with_structural_boolean_fields_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_runtime_values_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        structural_boolean_fields,
        &[],
        handler,
    )
}

/// Execute with exact initial values for direct primitive structural roots.
/// The returned measurement retains their final values after all internal
/// calls have completed. This is target-neutral logical storage, not a native
/// address or layout contract.
pub fn interpret_terminal_artifact_with_structural_primitive_values_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_runtime_values_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        &[],
        structural_primitive_values,
        handler,
    )
}

#[allow(clippy::too_many_arguments)]
fn interpret_terminal_artifact_with_structural_runtime_values_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
    structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut execution = TerminalExecution::start_artifact_with_structural_runtime_values(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        structural_boolean_fields,
        structural_primitive_values,
    )?;
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume_with_effect_handler(&mut meter, handler)
        .map_err(TerminalArtifactInterpretError::Execution)?
    {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Fuel(FuelMeterError::Exhausted(exhaustion)),
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(crash),
            ));
        }
    };
    let structural_primitive_values = execution.final_structural_primitive_values();
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
        effects: execution.effects,
        structural_primitive_values,
    })
}

/// Decode, verify, and execute canonical terminal-Psi semantic/proof artifact
/// sections, returning only their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(semantic_bytes, proof_bytes, profile, arguments)
        .map(MeasuredTerminalExecution::into_value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalScalarValue {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    IeeeFloat(IeeeFloatValue),
}

/// Opaque target-neutral runtime carrier for one structural argument.
///
/// `opaque_identity` is chosen by the embedding host and is only preserved for
/// argument forwarding and deterministic effect observation. Psi never treats
/// it as an address or layout. Qualification IDs are semantic runtime facts
/// supplied by the root installation and must be strictly increasing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
    pub path: Vec<StructuralPathSegment>,
}

/// Exact target-neutral runtime carrier for a selected scalar sum case.
///
/// This is deliberately distinct from an opaque host structural value: a
/// producer-created case has no host identity to preserve or invent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarCaseValue {
    pub structural_type: StructuralTypeId,
    pub result_case: StructuralCaseId,
    pub fields: Vec<(semantic_vocabulary::StructuralFieldId, TerminalScalarValue)>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalStructuralBooleanFieldValue {
    pub argument_index: u32,
    /// Structural path from the entry argument to the record containing
    /// `field`. Empty retains the original direct-field input form.
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
    pub value: bool,
}

/// Existing target-neutral value for one direct primitive structural entry
/// argument. `argument_index` is the dense structural-parameter position, not
/// a scalar parameter or a machine-local place identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalStructuralPrimitiveValue {
    pub argument_index: u32,
    pub value: TerminalScalarValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StructuralRuntimePlace {
    opaque_identity: u64,
    path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StructuralScalarRuntimeField {
    parent: StructuralRuntimePlace,
    field: StructuralFieldId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StructuralByteSequenceRuntimeField {
    parent: StructuralRuntimePlace,
    field: StructuralFieldId,
}

impl From<&TerminalStructuralValue> for StructuralRuntimePlace {
    fn from(value: &TerminalStructuralValue) -> Self {
        Self {
            opaque_identity: value.opaque_identity,
            path: value.path.clone(),
        }
    }
}

/// One externally observable terminal-Psi effect in semantic execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffect {
    BoundaryCall {
        operation: OperationId,
        boundary: BoundaryMachineId,
        arguments: Vec<TerminalScalarValue>,
        structural_arguments: Vec<TerminalStructuralValue>,
        /// Exact byte payload aligned with `structural_arguments`. Whole
        /// immutable literals, forwarded views, and mutable inline fields
        /// contribute `Some`; other structural types contribute `None`.
        /// Mutable payloads record the pre-call snapshot, not replacement bytes.
        /// A byte view without executable
        /// contents rejects before invoking the handler.
        byte_sequence_arguments: Vec<Option<Vec<u8>>>,
        /// Required receipts on normal completion, not evidence that completion
        /// occurred. A valid crashing invocation remains an observable effect
        /// but commits none of these receipts.
        completion_receipts: Vec<CompletionReceipt>,
        result: BoundaryMachineResult,
    },
    PortWrite {
        operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
}

/// Injected semantic effect sink used by the oracle and tests. Native provider
/// selection and hardware realization remain outside the Psi interpreter.
pub trait TerminalEffectHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection>;

    /// Handle an effect with staged, bounded mutable byte arguments. The
    /// default rejects before performing an effect whose writeback it cannot
    /// supply; existing immutable effects retain their result handler.
    fn handle_effect_with_byte_buffers(
        &mut self,
        effect: &TerminalEffect,
        buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "handler does not support mutable boundary byte buffers",
            ));
        }
        self.handle_effect_result(effect)
    }

    /// Return the boundary's exact declared result. The default Unit handler
    /// rejects structural results before performing an effect it cannot finish.
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(
            effect,
            TerminalEffect::BoundaryCall {
                result: BoundaryMachineResult::Structural(_),
                ..
            }
        ) {
            return Err(TerminalEffectRejection::new(
                "handler does not supply structural boundary results",
            ));
        }
        self.handle_effect(effect)?;
        Ok(TerminalEffectResult::Unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEffectRejection {
    pub reason: String,
}

/// Omega-owned policy input naming one exact verified terminal provider row.
/// Selection is intentionally separate from terminal-Psi semantic bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderInstallationSelection {
    pub boundary: BoundaryMachineId,
    pub provider_identity: String,
    pub candidate: MachineId,
}

/// Validated provider installation bound to one exact terminal-Psi identity.
/// Private fields prevent callers from manufacturing a boundary-to-machine
/// redirect without replaying terminal decoding and verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    installed: BTreeMap<BoundaryMachineId, MachineId>,
}

impl AdmittedProviderInstallation {
    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi
    }
}

/// Decode and verify an artifact, then admit only selections that exactly name
/// rows in its canonical provider catalog.
pub fn admit_provider_installation_from_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ProviderInstallationError::ProofDecode)?;
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ProviderInstallationError::Verification)?;
    let mut installed = BTreeMap::new();
    for selection in selections {
        if selection.provider_identity.is_empty()
            || installed
                .insert(selection.boundary, selection.candidate)
                .is_some()
            || !verified
                .module()
                .provider_candidates
                .iter()
                .any(|candidate| {
                    candidate.boundary == selection.boundary
                        && candidate.provider_identity == selection.provider_identity
                        && candidate.candidate == selection.candidate
                })
        {
            return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                boundary: selection.boundary,
                candidate: selection.candidate,
            });
        }
    }
    Ok(AdmittedProviderInstallation {
        terminal_psi: terminal_codec::terminal_psi_identity(verified.module())
            .map_err(ProviderInstallationError::SemanticDecode)?,
        installed,
    })
}

impl TerminalEffectRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct AcceptTerminalEffects;

impl TerminalEffectHandler for AcceptTerminalEffects {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }
}

/// The normal result of terminal-Psi execution.
///
/// Unit is a successful absence of a value, not a distinguished scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionResult {
    Unit,
    Scalar(TerminalScalarValue),
    Structural(TerminalStructuralResult),
    ScalarCase(TerminalScalarCaseResult),
    ScalarArray(TerminalScalarArrayResult),
}

/// A structural value returned with the exact live claims transferred into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralResult {
    pub value: TerminalStructuralValue,
    pub claims: Vec<ClaimId>,
}

/// A returned selected case whose primitive payload carries no structural claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarCaseResult {
    pub value: TerminalScalarCaseValue,
}

impl TerminalScalarValue {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
        }
    }
}

/// Resumable execution state created from canonical terminal-Psi artifact
/// sections.
///
/// Fuel exhaustion never advances `next_operation` or the current terminator,
/// so a sponsor can replenish the same meter and resume without replaying
/// semantic work or charging it twice.
pub struct TerminalExecution {
    structural_types: BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    // One immutable code owner survives suspension. A resume or call borrows
    // this owner independently of mutable runtime storage; frames retain only
    // machine identity. There is no per-machine or per-operation Arc allocation.
    machines: std::sync::Arc<BTreeMap<MachineId, ExecutableMachine>>,
    dynamic_scalar_calls: BTreeMap<(MachineId, u32), (MachineId, StructuralArgument)>,
    dynamic_descriptor_templates: BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    dynamic_selection_templates: BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    dynamic_descriptor_arguments:
        BTreeMap<(MachineId, OperationId), Vec<terminal_psi::TerminalDynamicDescriptorArgument>>,
    dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    boundary_machines: BTreeMap<BoundaryMachineId, BoundaryMachineDeclaration>,
    provider_candidates: BTreeSet<BoundaryMachineId>,
    provider_installation: BTreeMap<BoundaryMachineId, MachineId>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    /// A reference carrier owns this descriptor, never the referent's backing.
    /// Carrier identities survive structural call/return moves across frames.
    reference_referents: BTreeMap<StructuralRuntimePlace, TerminalStructuralValue>,
    /// Mutable primitive contents live outside call frames. Machine-local
    /// place maps are only views into this stable logical storage arena.
    structural_primitive_storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    structural_primitive_entry_places: BTreeMap<u32, StructuralRuntimePlace>,
    local_structural_identities: primitive_storage::LocalStructuralIdentities,
    /// Scalar leaves written below aggregate structural values. Keys use the
    /// invocation-independent opaque identity and resolved parent path, so a
    /// projected call observes the same field without native layout claims.
    structural_scalar_fields: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    /// Owned byte contents are keyed by referent identity, not a callee-local
    /// parameter. Immutable backing implements a logical copy of the live prefix.
    structural_byte_sequence_fields: BTreeMap<StructuralByteSequenceRuntimeField, ByteSequenceView>,
    structural_byte_arrays: BTreeMap<StructuralRuntimePlace, ByteSequenceView>,
    /// Entry-provided discriminators remain attached to the original referent
    /// across projected and forwarded calls, independent of machine-local IDs.
    structural_cases: BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
    scalar_case_values: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    /// Frame-local immutable descriptors or exact boundary-introduced mutable
    /// field loans, rebound to callee parameters. Opaque identities alone do
    /// not supply either byte contents or permission to mutate a field.
    byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    /// Exact claim-free affine ownership frontier. Opaque structural storage is
    /// root-addressed, so projected moves must be represented here rather than
    /// by unsoundly deleting their containing root.
    live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    live_claims: BTreeMap<ClaimId, LiveClaim>,
    current_machine: MachineId,
    current: BlockId,
    next_operation: usize,
    call_stack: Vec<SuspendedCall>,
    result: Option<TerminalExecutionResult>,
    crash: Option<TerminalCrash>,
    effects: Vec<TerminalEffect>,
}

struct ExecutableMachine {
    parameters: Vec<terminal_psi::ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    structural_places: Vec<terminal_psi::StructuralPlaceDeclaration>,
    entry_claims: Vec<EntryClaim>,
    content_entry_claims: Vec<terminal_psi::ContentEntryClaim>,
    result: TerminalMachineResult,
    entry: BlockId,
    blocks: BTreeMap<BlockId, Block>,
}

struct SuspendedCall {
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    scalar_case_values: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    scalar_array_values: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    byte_sequence_values: BTreeMap<PlaceId, ByteSequenceBinding>,
    live_affine_frontier: BTreeSet<StructuralAffineDiscard>,
    live_claims: BTreeMap<ClaimId, LiveClaim>,
    dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    current_machine: MachineId,
    current: BlockId,
    next_operation: usize,
    result: SuspendedCallResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDynamicDescriptorTemplate {
    source: StructuralArgument,
    callables: Vec<MachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDynamicDescriptor {
    source: TerminalStructuralValue,
    callables: Vec<MachineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    place: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

enum SuspendedCallResult {
    Scalar(ValueId),
    Unit,
    Structural {
        result: StructuralOperationResult,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
        expected_reference_backings: Vec<TerminalStructuralValue>,
    },
    NominalCleanups {
        completed: (NominalAffineCleanup, TerminalStructuralValue),
        remaining: Vec<(NominalAffineCleanup, TerminalStructuralValue)>,
        final_result: Option<TerminalScalarValue>,
    },
}

impl TerminalExecution {
    /// Canonical-decode, verify, and begin one resumable artifact execution.
    /// The resulting state owns the verified program's code, so no decoded
    /// producer object or self-referential verifier borrow escapes this entry.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        arguments: &[TerminalScalarValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_arguments(
            semantic_bytes,
            proof_bytes,
            profile,
            arguments,
            &[],
        )
    }

    pub fn start_artifact_with_structural_arguments(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_arguments_and_boolean_fields(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
            &[],
        )
    }

    pub fn start_artifact_with_structural_arguments_and_boolean_fields(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_runtime_values(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
            structural_boolean_fields,
            &[],
        )
    }

    pub fn start_artifact_with_structural_arguments_and_primitive_values(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_runtime_values(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
            &[],
            structural_primitive_values,
        )
    }

    /// Decode and verify an artifact, then bind explicitly supplied scalar fields
    /// by structural argument, typed path, and field identity. No native layout or
    /// default field contents are inferred. Duplicate referents and mistyped values reject.
    pub fn start_artifact_with_structural_arguments_and_scalar_fields(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_fields: &[TerminalStructuralScalarFieldValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_scalar_runtime_values(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
            structural_scalar_fields,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_artifact_with_structural_runtime_values(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
        structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let fields = structural_boolean_fields
            .iter()
            .map(TerminalStructuralScalarFieldValue::from)
            .collect::<Vec<_>>();
        Self::start_artifact_with_scalar_runtime_values(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
            &fields,
            structural_primitive_values,
        )
        .map_err(|error| match error {
            TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index,
                    field,
                },
            ) => TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                    argument_index,
                    field,
                },
            ),
            other => other,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn start_artifact_with_scalar_runtime_values(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_fields: &[TerminalStructuralScalarFieldValue],
        structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        Self::start_artifact_with_structural_inputs(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            TerminalStructuralInputs {
                arguments: structural_arguments,
                scalar_fields: structural_scalar_fields,
                primitive_values: structural_primitive_values,
                cases: &[],
            },
        )
    }

    /// Decode and independently verify an artifact, then bind its initialized
    /// entry contents before committing any operation or custody transfer.
    pub fn start_artifact_with_structural_inputs(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified =
            terminal_verifier::verify_module_for_interpretation(&module, &proof, profile)
                .map_err(TerminalArtifactInterpretError::Verification)?;
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_inputs.arguments,
            structural_inputs.scalar_fields,
            structural_inputs.primitive_values,
            None,
        )
        .map_err(TerminalArtifactInterpretError::Execution)?;
        execution
            .bind_structural_cases(structural_inputs.cases)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        Ok(execution)
    }

    /// Begin execution with one explicit provider installation previously
    /// admitted against these exact semantic/proof sections. Fixed-array inputs
    /// supply exact initialized backing; an installed provider grants no storage.
    pub fn start_artifact_with_provider_installation(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        byte_arrays: &[TerminalStructuralByteArrayValue],
        installation: &AdmittedProviderInstallation,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified = terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_arguments,
            &[],
            &[],
            Some(installation),
        )
        .map_err(TerminalArtifactInterpretError::Execution)?;
        execution
            .bind_byte_arrays(byte_arrays)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        Ok(execution)
    }

    fn start_verified_module(
        mut module: terminal_psi::TerminalModule,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_scalar_field_arguments: &[TerminalStructuralScalarFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        installation: Option<&AdmittedProviderInstallation>,
    ) -> Result<Self, TerminalInterpretError> {
        let terminal_psi = terminal_codec::terminal_psi_identity(&module)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        if installation.is_some_and(|installation| installation.terminal_psi != terminal_psi) {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let local_structural_identities =
            primitive_storage::LocalStructuralIdentities::new(&module, structural_arguments);
        let machines = std::mem::take(&mut module.machines)
            .into_iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        parameters: machine.parameters,
                        structural_parameters: machine.structural_parameters,
                        structural_places: machine.structural_places,
                        entry_claims: machine.entry_claims,
                        content_entry_claims: machine.content_entry_claims,
                        result: machine.result,
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .into_iter()
                            .map(|block| (block.id, block))
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut dynamic_scalar_calls = module
            .dynamic_dispatch
            .indirect_dispatches
            .iter()
            .map(|dispatch| {
                let descriptor = module
                    .dynamic_dispatch
                    .rebound_descriptors
                    .iter()
                    .find(|descriptor| {
                        descriptor.owner == dispatch.owner
                            && descriptor.ordinal == dispatch.descriptor_ordinal
                    })
                    .expect("verified indirect dispatch has one descriptor");
                let selection = module
                    .dynamic_dispatch
                    .selections
                    .iter()
                    .find(|selection| {
                        selection.owner == descriptor.owner
                            && selection.ordinal == descriptor.rebound_selection_ordinal
                    })
                    .expect("verified descriptor has one latest selection");
                (
                    (dispatch.owner, dispatch.descriptor_ordinal),
                    (dispatch.realization, selection.source.clone()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for dispatch in &module.dynamic_dispatch.stored_dispatches {
            let descriptor = module
                .dynamic_dispatch
                .stored_descriptors
                .iter()
                .find(|descriptor| {
                    descriptor.owner == dispatch.owner
                        && descriptor.ordinal == dispatch.descriptor_ordinal
                })
                .expect("verified stored dispatch has one descriptor");
            let selection = module
                .dynamic_dispatch
                .selections
                .iter()
                .find(|selection| {
                    selection.owner == descriptor.owner
                        && selection.ordinal == descriptor.selection_ordinal
                })
                .expect("verified stored descriptor has one selection");
            assert!(
                dynamic_scalar_calls
                    .insert(
                        (dispatch.owner, dispatch.descriptor_ordinal),
                        (dispatch.realization, selection.source.clone()),
                    )
                    .is_none(),
                "verified dynamic descriptor coordinates must be disjoint"
            );
        }
        let dynamic_selection_templates = module
            .dynamic_dispatch
            .selections
            .iter()
            .map(|selection| {
                let application = module
                    .closed_conformance_applications
                    .iter()
                    .find(|application| {
                        application.owner == selection.owner
                            && application.report_fingerprint
                                == selection.conformance_application_report_fingerprint
                            && application.commitment
                                == selection.conformance_application_commitment
                    })
                    .expect("verified selection has one conformance application");
                let callables = application
                    .rows
                    .iter()
                    .map(|row| {
                        let identity = row
                            .realization_callable_identity
                            .as_ref()
                            .expect("verified dynamic row has one callable identity");
                        application
                            .realization_callables
                            .iter()
                            .find(|callable| callable.source_callable_identity == *identity)
                            .map(|callable| callable.machine)
                            .expect("verified dynamic row has one callable")
                    })
                    .collect();
                (
                    (selection.owner, selection.ordinal),
                    RuntimeDynamicDescriptorTemplate {
                        source: selection.source.clone(),
                        callables,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut dynamic_descriptor_templates = module
            .dynamic_dispatch
            .rebound_descriptors
            .iter()
            .map(|descriptor| {
                let template = dynamic_selection_templates
                    .get(&(descriptor.owner, descriptor.rebound_selection_ordinal))
                    .expect("verified descriptor has one latest selection")
                    .clone();
                ((descriptor.owner, descriptor.ordinal), template)
            })
            .collect::<BTreeMap<_, _>>();
        for descriptor in &module.dynamic_dispatch.stored_descriptors {
            let template = dynamic_selection_templates
                .get(&(descriptor.owner, descriptor.selection_ordinal))
                .expect("verified stored descriptor has one selection")
                .clone();
            assert!(
                dynamic_descriptor_templates
                    .insert((descriptor.owner, descriptor.ordinal), template)
                    .is_none(),
                "verified dynamic descriptor coordinates must be disjoint"
            );
        }
        let mut dynamic_descriptor_arguments = BTreeMap::<
            (MachineId, OperationId),
            Vec<terminal_psi::TerminalDynamicDescriptorArgument>,
        >::new();
        for argument in &module.dynamic_dispatch.arguments {
            dynamic_descriptor_arguments
                .entry((argument.owner, argument.operation))
                .or_default()
                .push(argument.clone());
        }
        let boundary_machines = module
            .boundary_machines
            .iter()
            .cloned()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let structural_types = module
            .structural_types
            .iter()
            .cloned()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let machine = machines
            .get(&module.entry)
            .ok_or(TerminalInterpretError::VerifiedEntryMachineMissing)?;
        if machine
            .parameters
            .iter()
            .any(|parameter| !parameter.qualifications.is_empty())
        {
            return Err(TerminalInterpretError::ScalarEntryQualificationUnsupported);
        }
        let values = bind_arguments(&machine.parameters, scalar_arguments)?;
        let structural_values =
            bind_structural_arguments(&machine.structural_parameters, structural_arguments)?;
        let (structural_primitive_storage, structural_primitive_entry_places) =
            bind_structural_primitive_values(
                machine,
                &structural_types,
                &structural_values,
                structural_primitive_value_arguments,
            )?;
        let structural_scalar_fields = structural_scalar_fields::bind(
            machine,
            &structural_types,
            &structural_values,
            structural_scalar_field_arguments,
        )?;
        let live_affine_frontier =
            bind_affine_frontier(&machine.structural_parameters, &structural_values)?;
        let live_claims = bind_entry_claims(
            &machine.entry_claims,
            &machine.content_entry_claims,
            &machine.structural_parameters,
            &structural_values,
        )?;
        let current = machine.entry;
        Ok(Self {
            structural_types,
            machines: std::sync::Arc::new(machines),
            dynamic_scalar_calls,
            dynamic_descriptor_templates,
            dynamic_selection_templates,
            dynamic_descriptor_arguments,
            dynamic_parameters: BTreeMap::new(),
            boundary_machines,
            provider_candidates: module
                .provider_candidates
                .iter()
                .map(|candidate| candidate.boundary)
                .collect(),
            provider_installation: installation
                .map(|installation| installation.installed.clone())
                .unwrap_or_default(),
            values,
            structural_values,
            reference_referents: BTreeMap::new(),
            structural_primitive_storage,
            structural_primitive_entry_places,
            local_structural_identities,
            structural_scalar_fields,
            structural_byte_sequence_fields: BTreeMap::new(),
            structural_byte_arrays: BTreeMap::new(),
            structural_cases: BTreeMap::new(),
            scalar_case_values: BTreeMap::new(),
            scalar_array_values: BTreeMap::new(),
            byte_sequence_values: BTreeMap::new(),
            live_affine_frontier,
            live_claims,
            current_machine: module.entry,
            current,
            next_operation: 0,
            call_stack: Vec::new(),
            result: None,
            crash: None,
            effects: Vec::new(),
        })
    }

    pub fn resume(
        &mut self,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        let mut handler = AcceptTerminalEffects;
        self.resume_with_effect_handler(meter, &mut handler)
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    /// Final values of direct primitive structural entry arguments, ordered by
    /// their dense structural-argument positions.
    pub fn structural_primitive_values(&self) -> Vec<TerminalStructuralPrimitiveValue> {
        self.final_structural_primitive_values()
    }

    fn final_structural_primitive_values(&self) -> Vec<TerminalStructuralPrimitiveValue> {
        self.structural_primitive_entry_places
            .iter()
            .filter_map(|(argument_index, place)| {
                self.structural_primitive_storage
                    .get(place)
                    .copied()
                    .map(|value| TerminalStructuralPrimitiveValue {
                        argument_index: *argument_index,
                        value,
                    })
            })
            .collect()
    }

    pub fn live_claim_frontier(&self) -> impl Iterator<Item = ClaimId> + '_ {
        self.live_claims.keys().copied()
    }

    /// Exact live affine structural paths, ordered canonically. This is
    /// semantic ownership state, not a runtime object-layout bitmap.
    pub fn live_affine_frontier(&self) -> impl Iterator<Item = &StructuralAffineDiscard> + '_ {
        self.live_affine_frontier.iter()
    }

    pub fn resume_with_effect_handler(
        &mut self,
        meter: &mut TerminalFuelMeter,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
        if let Some(result) = &self.result {
            return Ok(TerminalExecutionStatus::Complete(result.clone()));
        }
        if let Some(crash) = &self.crash {
            return Ok(TerminalExecutionStatus::Crashed(crash.clone()));
        }

        let machines = std::sync::Arc::clone(&self.machines);
        loop {
            while let Some(operation) = machines
                .get(&self.current_machine)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                    self.current_machine,
                ))?
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .operations
                .get(self.next_operation)
            {
                if let Err(error) = meter.charge_operation(operation) {
                    return meter_status(error);
                }
                match operation.kind {
                    OperationKind::EstablishReference { ref source } => {
                        self.establish_reference(operation, source)?;
                    }
                    OperationKind::ReleaseReference { source } => {
                        self.release_reference(operation, source)?;
                    }
                    OperationKind::EstablishScalarArray { ref elements } => {
                        self.execute_scalar_array_establishment(operation, elements)?;
                    }
                    OperationKind::EstablishPrimitiveLocal { value } => {
                        self.execute_primitive_establishment(operation, value)?;
                    }
                    OperationKind::PrimitiveScalarRead { source, ref path } => {
                        self.execute_primitive_read(operation, source, path)?;
                    }
                    OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
                        if operation.result != terminal_psi::OperationResult::Unit
                            || !self
                                .dynamic_descriptor_templates
                                .contains_key(&(self.current_machine, descriptor_ordinal))
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                    }
                    OperationKind::EstablishScalarCase {
                        result_case,
                        ref fields,
                    } => {
                        let terminal_psi::OperationResult::Structural(result) = &operation.result
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if self.structural_values.contains_key(&result.place)
                            || self.scalar_case_values.contains_key(&result.place)
                            || !matches!(
                                result.multiplicity,
                                StructuralMultiplicity::Unrestricted
                                    | StructuralMultiplicity::Affine
                            )
                            || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty()
                            || !result.claims.is_empty()
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let Some(StructuralTypeDeclaration {
                            shape: StructuralTypeShape::Sum { cases },
                            ..
                        }) = self.structural_types.get(&result.structural_type)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let Some(selected) = cases.iter().find(|case| case.id == result_case)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if selected.fields.len() != fields.len() {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let mut payload = Vec::with_capacity(fields.len());
                        for (declaration, binding) in selected.fields.iter().zip(fields) {
                            let value = self.values.get(&binding.value).copied().ok_or(
                                TerminalInterpretError::VerifiedValueMissing(binding.value),
                            )?;
                            if declaration.id != binding.field
                                || declaration.field_type.scalar_type() != Some(value.scalar_type())
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            if let terminal_psi::StructuralFieldType::BoundedInteger(bounds) =
                                declaration.field_type
                            {
                                let TerminalScalarValue::Integer { value, .. } = value else {
                                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                                };
                                if !bounds.contains(value) {
                                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                                }
                            }
                            payload.push((binding.field, value));
                        }
                        self.scalar_case_values.insert(
                            result.place,
                            TerminalScalarCaseValue {
                                structural_type: result.structural_type,
                                result_case,
                                fields: payload,
                            },
                        );
                        if result.multiplicity == StructuralMultiplicity::Affine {
                            self.live_affine_frontier.insert(StructuralAffineDiscard {
                                place: result.place,
                                path: Vec::new(),
                                structural_type: result.structural_type,
                            });
                        }
                    }
                    OperationKind::EstablishByteSequenceLiteral {
                        destination,
                        ref bytes,
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let machine = self.machines.get(&self.current_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                        )?;
                        let Some(terminal_psi::StructuralPlaceDeclaration {
                            kind:
                                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                                    structural_type,
                                    ..
                                },
                            ..
                        }) = machine
                            .structural_places
                            .iter()
                            .find(|place| place.id == destination)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if let Some(previous) = self.structural_values.get(&destination) {
                            // Reentering the unique literal producer retains the
                            // same immutable value. Fuel was charged above;
                            // existing aliases keep their exact original bytes.
                            if previous.opaque_identity != destination.get()
                                || previous.structural_type != *structural_type
                                || !previous.path.is_empty()
                                || !previous.qualifications.is_empty()
                                || self
                                    .byte_sequence_values
                                    .get(&destination)
                                    .is_none_or(|binding| !matches!(binding.immutable(), Ok(view) if view.bytes() == bytes))
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                        } else {
                            if self
                                .byte_sequence_values
                                .insert(
                                    destination,
                                    ByteSequenceBinding::Immutable(ByteSequenceView::new(
                                        bytes.clone(),
                                    )),
                                )
                                .is_some()
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            self.structural_values.insert(
                                destination,
                                TerminalStructuralValue {
                                    opaque_identity: destination.get(),
                                    structural_type: *structural_type,
                                    qualifications: Vec::new(),
                                    path: Vec::new(),
                                },
                            );
                        }
                    }
                    OperationKind::EstablishTrivialAffineLocal { destination } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
                            || self.structural_values.contains_key(&destination)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let machine = self.machines.get(&self.current_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                        )?;
                        let Some(terminal_psi::StructuralPlaceDeclaration {
                            kind:
                                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                                    structural_type,
                                    ..
                                },
                            ..
                        }) = machine
                            .structural_places
                            .iter()
                            .find(|place| place.id == destination)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.structural_values.insert(
                            destination,
                            TerminalStructuralValue {
                                opaque_identity: destination.get(),
                                structural_type: *structural_type,
                                qualifications: Vec::new(),
                                path: Vec::new(),
                            },
                        );
                        self.live_affine_frontier.insert(StructuralAffineDiscard {
                            place: destination,
                            path: Vec::new(),
                            structural_type: *structural_type,
                        });
                    }
                    OperationKind::EstablishRecord { ref fields } => {
                        self.establish_record(
                            operation
                                .result
                                .structural()
                                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
                            fields,
                        )?;
                    }
                    OperationKind::CallUnit {
                        callee,
                        arguments: ref scalar_argument_ids,
                        ref structural_arguments,
                        ref claim_transfers,
                        ..
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let scalar_arguments = scalar_argument_ids
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let dynamic_parameters =
                            self.resolve_dynamic_call_arguments(operation.id)?;
                        let prepared_arguments =
                            self.prepare_structural_call_arguments(callee, structural_arguments)?;
                        self.begin_unit_call(
                            callee,
                            &scalar_arguments,
                            structural_arguments,
                            prepared_arguments,
                            claim_transfers,
                            dynamic_parameters,
                        )?;
                        continue;
                    }
                    OperationKind::CallStructuralScalar {
                        callee,
                        ref arguments,
                        ref structural_arguments,
                        ref claim_transfers,
                        ..
                    } => {
                        let result = operation
                            .result
                            .scalar()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let dynamic_parameters =
                            self.resolve_dynamic_call_arguments(operation.id)?;
                        let scalar_arguments = arguments
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        self.begin_structural_scalar_call(
                            callee,
                            result,
                            &scalar_arguments,
                            structural_arguments,
                            claim_transfers,
                            dynamic_parameters,
                        )?;
                        continue;
                    }
                    OperationKind::CallDynamicScalar {
                        descriptor_ordinal, ..
                    } => {
                        let result = operation
                            .result
                            .scalar()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let (callee, source) = self
                            .dynamic_scalar_calls
                            .get(&(self.current_machine, descriptor_ordinal))
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.begin_structural_scalar_call(
                            callee,
                            result,
                            &[],
                            &[source],
                            &[],
                            BTreeMap::new(),
                        )?;
                        continue;
                    }
                    OperationKind::CallDynamicParameterScalar {
                        parameter_ordinal,
                        requirement_slot,
                        ..
                    } => {
                        let result = operation
                            .result
                            .scalar()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let descriptor =
                            self.dynamic_parameters
                                .get(&parameter_ordinal)
                                .cloned()
                                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let slot = usize::try_from(requirement_slot)
                            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
                        let callee = descriptor
                            .callables
                            .get(slot)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.begin_runtime_dynamic_scalar_call(callee, result, descriptor.source)?;
                        continue;
                    }
                    OperationKind::CallDynamicUnit {
                        descriptor_ordinal, ..
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let (callee, source) = self
                            .dynamic_scalar_calls
                            .get(&(self.current_machine, descriptor_ordinal))
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let arguments = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            std::slice::from_ref(&source),
                        )?;
                        let [source] = arguments.as_slice() else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.begin_runtime_dynamic_unit_call(callee, source.clone())?;
                        continue;
                    }
                    OperationKind::CallDynamicParameterUnit {
                        parameter_ordinal,
                        requirement_slot,
                        ..
                    } => {
                        if operation.result != terminal_psi::OperationResult::Unit {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let descriptor =
                            self.dynamic_parameters
                                .get(&parameter_ordinal)
                                .cloned()
                                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let slot = usize::try_from(requirement_slot)
                            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
                        let callee = descriptor
                            .callables
                            .get(slot)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.begin_runtime_dynamic_unit_call(callee, descriptor.source)?;
                        continue;
                    }
                    OperationKind::CallStructural {
                        callee,
                        ref structural_arguments,
                        ref claim_transfers,
                        ref returned_claim_transfers,
                        ..
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let prepared_arguments =
                            self.prepare_structural_call_arguments(callee, structural_arguments)?;
                        self.begin_structural_result_call(
                            callee,
                            result,
                            &[],
                            structural_arguments,
                            prepared_arguments,
                            claim_transfers,
                            returned_claim_transfers.clone(),
                        )?;
                        continue;
                    }
                    OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        ref arguments,
                        ref structural_arguments,
                        ref claim_transfers,
                        ref returned_claim_transfers,
                        ..
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let scalar_arguments = arguments
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let prepared_arguments =
                            self.prepare_structural_call_arguments(callee, structural_arguments)?;
                        self.begin_structural_result_call(
                            callee,
                            result,
                            &scalar_arguments,
                            structural_arguments,
                            prepared_arguments,
                            claim_transfers,
                            returned_claim_transfers.clone(),
                        )?;
                        continue;
                    }
                    OperationKind::BoundaryCall {
                        boundary,
                        arguments: ref scalar_argument_ids,
                        ref structural_arguments,
                        ref completion_receipts,
                        ..
                    } => {
                        let boundary_declaration = self.boundary_machines.get(&boundary).ok_or(
                            TerminalInterpretError::VerifiedBoundaryMachineMissing(boundary),
                        )?;
                        let scalar_arguments = scalar_argument_ids
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        bind_boundary_arguments(
                            &boundary_declaration.scalar_parameters,
                            &scalar_arguments,
                        )?;
                        if self.provider_candidates.contains(&boundary) {
                            let boundary_arguments = self.prepare_boundary_arguments(
                                &boundary_declaration.structural_parameters,
                                structural_arguments,
                            )?;
                            bind_structural_arguments(
                                &boundary_declaration.structural_parameters,
                                &boundary_arguments.values,
                            )?;
                            validate_boundary_requirements(
                                boundary_declaration,
                                &boundary_arguments.values,
                            )?;
                            let supported_result = match &operation.result {
                                terminal_psi::OperationResult::Unit => true,
                                terminal_psi::OperationResult::Structural(result) => {
                                    result.multiplicity == StructuralMultiplicity::Affine
                                        && result.qualifications.is_empty()
                                        && result.projected_qualifications.is_empty()
                                        && result.claims.is_empty()
                                }
                                terminal_psi::OperationResult::Scalar(_) => false,
                            };
                            if !supported_result {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            let callee_id =
                                self.provider_installation.get(&boundary).copied().ok_or(
                                    TerminalInterpretError::ProviderInstallationMissing(boundary),
                                )?;
                            let callee = self.machines.get(&callee_id).ok_or(
                                TerminalInterpretError::VerifiedCallTargetMissing(callee_id),
                            )?;
                            let prepared_arguments = boundary_arguments
                                .into_call_arguments(&callee.structural_parameters)?;
                            let claim_transfers = completion_receipts
                                .iter()
                                .map(|receipt| ClaimTransfer {
                                    claim: receipt.claim,
                                    argument_index: receipt.argument_index,
                                })
                                .collect::<Vec<_>>();
                            match &operation.result {
                                terminal_psi::OperationResult::Unit => self.begin_unit_call(
                                    callee_id,
                                    // Boundary binding and installed conformance preserve
                                    // this ordered scalar lane; the ordinary call binder
                                    // validates it again against the selected callee.
                                    &scalar_arguments,
                                    structural_arguments,
                                    prepared_arguments,
                                    &claim_transfers,
                                    BTreeMap::new(),
                                )?,
                                terminal_psi::OperationResult::Structural(result) => {
                                    self.begin_structural_result_call(
                                        callee_id,
                                        result.clone(),
                                        &scalar_arguments,
                                        structural_arguments,
                                        prepared_arguments,
                                        &claim_transfers,
                                        Vec::new(),
                                    )?;
                                }
                                terminal_psi::OperationResult::Scalar(_) => {
                                    unreachable!("scalar provider results were rejected above")
                                }
                            }
                            continue;
                        }
                        let mut boundary_arguments = self.resolve_boundary_arguments(
                            &boundary_declaration.structural_parameters,
                            structural_arguments,
                        )?;
                        bind_structural_arguments(
                            &boundary_declaration.structural_parameters,
                            &boundary_arguments.values,
                        )?;
                        validate_boundary_requirements(
                            boundary_declaration,
                            &boundary_arguments.values,
                        )?;
                        self.preflight_boundary_result(&operation.result)?;
                        let remaining_claims = complete_claims(
                            &self.live_claims,
                            structural_arguments,
                            completion_receipts,
                            &boundary_declaration.structural_parameters,
                        )?;
                        let effect = TerminalEffect::BoundaryCall {
                            operation: operation.id,
                            boundary,
                            arguments: scalar_arguments,
                            structural_arguments: std::mem::take(&mut boundary_arguments.values),
                            byte_sequence_arguments: std::mem::take(&mut boundary_arguments.bytes),
                            completion_receipts: completion_receipts.clone(),
                            result: boundary_declaration.result.clone(),
                        };
                        let returned = handler
                            .handle_effect_with_byte_buffers(
                                &effect,
                                &mut boundary_arguments.buffers,
                            )
                            .map_err(|rejection| TerminalInterpretError::EffectRejected {
                                operation: operation.id,
                                rejection,
                            })?;
                        if let TerminalEffectResult::Crash(cause) = returned {
                            // A boundary crash belongs to the invocation, not a
                            // fabricated CFG edge. Validate before publishing a
                            // result, writeback, disposal, or completion receipt.
                            let crash =
                                self.admit_boundary_crash(boundary_declaration, &effect, cause)?;
                            self.effects.push(effect);
                            self.crash = Some(crash.clone());
                            return Ok(TerminalExecutionStatus::Crashed(crash));
                        }
                        boundary_arguments.validate_writeback()?;
                        if let TerminalEffectResult::Structural(value) = &returned {
                            self.local_structural_identities.reserve_host(value)?;
                        }
                        effect_results::commit_boundary_result(
                            &mut self.values,
                            &mut self.structural_values,
                            &mut self.live_affine_frontier,
                            &operation.result,
                            &boundary_declaration.result,
                            returned,
                        )?;
                        for (argument, parameter) in structural_arguments
                            .iter()
                            .zip(&boundary_declaration.structural_parameters)
                            .filter(|(argument, parameter)| {
                                argument.path.is_empty()
                                    && parameter.access == StructuralAccess::Owned
                                    && parameter.multiplicity
                                        != StructuralMultiplicity::Unrestricted
                            })
                        {
                            if self.structural_values.remove(&argument.place).is_none() {
                                return Err(
                                    TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                        argument.place,
                                    ),
                                );
                            }
                            if parameter.multiplicity == StructuralMultiplicity::Affine {
                                remove_affine_root(&mut self.live_affine_frontier, argument.place);
                            }
                        }
                        self.live_claims = remaining_claims;
                        boundary_arguments.commit(self);
                        self.effects.push(effect);
                    }
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let effect = TerminalEffect::PortWrite {
                            operation: operation.id,
                            service,
                            port,
                            value,
                        };
                        handler.handle_effect(&effect).map_err(|rejection| {
                            TerminalInterpretError::EffectRejected {
                                operation: operation.id,
                                rejection,
                            }
                        })?;
                        self.effects.push(effect);
                    }
                    OperationKind::Call {
                        callee,
                        ref arguments,
                        ..
                    } => {
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                self.values
                                    .get(argument)
                                    .copied()
                                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let callee_id = callee;
                        let callee = machines
                            .get(&callee_id)
                            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
                        if !callee.structural_parameters.is_empty()
                            || !callee.entry_claims.is_empty()
                            || !callee.content_entry_claims.is_empty()
                            || !matches!(callee.result, TerminalMachineResult::Scalar(_))
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let values = bind_arguments(&callee.parameters, &arguments)?;
                        self.next_operation += 1;
                        self.call_stack.push(SuspendedCall {
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
                            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
                            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
                            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                            live_claims: std::mem::take(&mut self.live_claims),
                            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
                            current_machine: self.current_machine,
                            current: self.current,
                            next_operation: self.next_operation,
                            result: SuspendedCallResult::Scalar(
                                operation.result.expect_scalar().id,
                            ),
                        });
                        self.values = values;
                        self.structural_values = BTreeMap::new();
                        self.live_affine_frontier = BTreeSet::new();
                        self.live_claims = BTreeMap::new();
                        self.dynamic_parameters = BTreeMap::new();
                        self.current_machine = callee_id;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        continue;
                    }
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        value,
                        ref path,
                    } => {
                        self.execute_primitive_store(operation, destination, value, path)?;
                    }
                    OperationKind::StructuralScalarFieldStore {
                        destination,
                        ref path,
                        field,
                        value,
                        ..
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let machine = self.machines.get(&self.current_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                        )?;
                        let access = machine
                            .structural_parameters
                            .iter()
                            .chain(machine.blocks.values().flat_map(|block| &block.structural_parameters))
                            .find(|parameter| parameter.place == destination)
                            .filter(|parameter| {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::Owned
                                        | StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                ) && matches!(
                                    parameter.multiplicity,
                                    StructuralMultiplicity::Unrestricted
                                        | StructuralMultiplicity::Affine
                                ) && parameter.qualifications.is_empty()
                                    && parameter.projected_qualifications.is_empty()
                            })
                            .map(|parameter| parameter.access)
                            .or_else(|| {
                                machine.blocks.values().flat_map(|block| &block.operations)
                                    .filter(|producer| matches!(producer.kind,
                                        OperationKind::EstablishRecord { .. }
                                            | OperationKind::CallStructural { .. }
                                            | OperationKind::CallStructuralWithScalarArguments { .. }))
                                    .filter_map(|producer| producer.result.structural())
                                    .find(|result| result.place == destination
                                        && matches!(result.multiplicity,
                                            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine)
                                        && result.qualifications.is_empty()
                                        && result.projected_qualifications.is_empty()
                                        && result.claims.is_empty())
                                    .map(|_| StructuralAccess::Owned)
                            })
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let source = self
                            .values
                            .get(&value)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
                        let parent = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            &[StructuralArgument {
                                place: destination,
                                path: path.clone(),
                                access,
                            }],
                        )?
                        .pop()
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        if direct_scalar_field_type(
                            &self.structural_types,
                            parent.structural_type,
                            field,
                        ) != Some(source.scalar_type())
                            || !terminal_scalar_belongs_to_type(source)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.structural_scalar_fields.insert(
                            StructuralScalarRuntimeField {
                                parent: StructuralRuntimePlace::from(&parent),
                                field,
                            },
                            source,
                        );
                    }
                    OperationKind::StructuralByteSequenceFieldStore { .. } => {
                        self.execute_structural_byte_sequence_field_store(operation)?;
                    }
                    OperationKind::StructuralByteSequenceFieldLength { .. } => {
                        self.execute_structural_byte_sequence_field_length(operation)?;
                    }
                    OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
                        self.execute_structural_byte_sequence_field_byte_store(operation)?;
                    }
                    OperationKind::IntegerConstant { value } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::IeeeFloatConstant { value } => {
                        if operation.result.expect_scalar().scalar_type
                            != ScalarType::IeeeFloat(value.format())
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::IeeeFloat(value),
                        );
                    }
                    OperationKind::IeeeFloatCompare {
                        comparison,
                        left,
                        right,
                    } => {
                        let read_float = |operand| match self.values.get(&operand).copied() {
                            Some(TerminalScalarValue::IeeeFloat(value)) => Ok(value),
                            Some(_) => Err(TerminalInterpretError::VerifiedOperationMalformed),
                            None => Err(TerminalInterpretError::VerifiedValueMissing(operand)),
                        };
                        let left = read_float(left)?;
                        let right = read_float(right)?;
                        if left.format() != right.format()
                            || operation.result.expect_scalar().scalar_type != ScalarType::Boolean
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(ieee_float_compare(
                                comparison, left, right,
                            )),
                        );
                    }
                    OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                        left,
                        right,
                        addend,
                    } => {
                        let ScalarType::IeeeFloat(format) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::IeeeFloat(left) = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::IeeeFloat(right) = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::IeeeFloat(addend) = self
                            .values
                            .get(&addend)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(addend))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left.format() != format
                            || right.format() != format
                            || addend.format() != format
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let result =
                            nearest_ieee_float_fused_multiply_add(format, left, right, addend);
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::IeeeFloat(result),
                        );
                    }
                    OperationKind::BooleanConstant { value } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(value),
                        );
                    }
                    OperationKind::StructuralCaseMembership {
                        source,
                        ref path,
                        case,
                    } => {
                        let active_case = self.observe_structural_case(source, path)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(active_case == case),
                        );
                    }
                    OperationKind::BooleanStructuralField {
                        source,
                        ref path,
                        field,
                    } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let structural_value = self.structural_values.get(&source).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
                        )?;
                        let carrier = terminal_semantics::record_field_carrier(
                            self.structural_types.values(),
                            structural_value.structural_type,
                            path,
                        )
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let mut parent = StructuralRuntimePlace::from(structural_value);
                        parent.path.extend(carrier.path);
                        let value = self
                            .structural_scalar_fields
                            .get(&StructuralScalarRuntimeField { parent, field })
                            .copied()
                            .ok_or(TerminalInterpretError::StructuralBooleanFieldMissing {
                                source,
                                field,
                            })?;
                        let TerminalScalarValue::Boolean(value) = value else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(value),
                        );
                    }
                    OperationKind::IntegerStructuralField {
                        source,
                        ref path,
                        field,
                    } => {
                        let result = operation.result.expect_scalar();
                        if !matches!(result.scalar_type, ScalarType::Integer(_)) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let structural_value = self.structural_values.get(&source).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
                        )?;
                        let carrier = terminal_semantics::record_field_carrier(
                            self.structural_types.values(),
                            structural_value.structural_type,
                            path,
                        )
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        if direct_scalar_field_type(
                            &self.structural_types,
                            carrier.structural_type,
                            field,
                        ) != Some(result.scalar_type)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let mut parent = StructuralRuntimePlace::from(structural_value);
                        parent.path.extend(carrier.path);
                        let value = self
                            .structural_scalar_fields
                            .get(&StructuralScalarRuntimeField { parent, field })
                            .copied()
                            .ok_or(TerminalInterpretError::StructuralScalarFieldMissing {
                                source,
                                field,
                            })?;
                        if value.scalar_type() != result.scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(result.id, value);
                    }
                    OperationKind::ByteSequenceSubslice { .. } => {
                        self.execute_byte_sequence_subslice(operation)?;
                    }
                    OperationKind::ByteSequenceWrite { .. } => {
                        self.execute_byte_sequence_write(operation)?;
                    }
                    OperationKind::ByteSequenceRead {
                        source,
                        index,
                        length,
                        ..
                    } => {
                        let result = operation.result.expect_scalar();
                        let byte_type =
                            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                                .expect("u8 is valid");
                        let count_type =
                            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                                .expect("u64 is valid");
                        if result.scalar_type != ScalarType::Integer(byte_type) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let count = |operand| -> Result<u64, TerminalInterpretError> {
                            let value = self
                                .values
                                .get(&operand)
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?;
                            let TerminalScalarValue::Integer {
                                scalar_type,
                                value: IntegerValue::Unsigned(value),
                            } = value
                            else {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            };
                            if *scalar_type != count_type {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            u64::try_from(*value)
                                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)
                        };
                        let byte_index = count(index)?;
                        let byte_length = count(length)?;
                        let bytes = self
                            .byte_sequence_values
                            .get(&source)
                            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                source,
                            ))?
                            .immutable()?;
                        if u64::try_from(bytes.len()).ok() != Some(byte_length)
                            || byte_index >= byte_length
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let byte_index = usize::try_from(byte_index)
                            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
                        let byte = bytes
                            .get(byte_index)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            result.id,
                            TerminalScalarValue::Integer {
                                scalar_type: byte_type,
                                value: IntegerValue::Unsigned(u128::from(byte)),
                            },
                        );
                    }
                    OperationKind::ByteSequenceLength { source } => {
                        let result = operation.result.expect_scalar();
                        let integer_type =
                            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                                .expect("u64 is valid");
                        if result.scalar_type != ScalarType::Integer(integer_type) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let length = self.byte_sequence_length(source)?;
                        self.values.insert(
                            result.id,
                            TerminalScalarValue::Integer {
                                scalar_type: integer_type,
                                value: IntegerValue::Unsigned(u128::from(length)),
                            },
                        );
                    }
                    OperationKind::BooleanNot { operand } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(value) = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(!value),
                        );
                    }
                    OperationKind::BooleanEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Boolean(left) = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Boolean(right) = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(left == right),
                        );
                    }
                    OperationKind::IntegerLessThan { left, right }
                    | OperationKind::IntegerLessOrEqual { left, right } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != right_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let ordering = left_type
                            .compare(left_value, right_value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let result = match operation.kind {
                            OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
                            OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                            _ => unreachable!(),
                        };
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Boolean(result),
                        );
                    }
                    OperationKind::IntegerBitwiseNot { operand } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: operand_type,
                            value: operand,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if operand_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .bitwise_not(operand)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::IntegerWiden { operand } => {
                        let ScalarType::Integer(target_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .widen_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
                        );
                    }
                    OperationKind::IntegerExactCast { operand, .. } => {
                        let ScalarType::Integer(target_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: source_type,
                            value,
                        } = self
                            .values
                            .get(&operand)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let value = source_type
                            .exact_cast_value_to(target_type, value)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: target_type,
                                value,
                            },
                        );
                    }
                    OperationKind::IntegerBitwiseAnd { left, right }
                    | OperationKind::IntegerBitwiseOr { left, right }
                    | OperationKind::IntegerBitwiseXor { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: left_type,
                            value: left_value,
                        } = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: right_type,
                            value: right_value,
                        } = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind {
                            OperationKind::IntegerBitwiseAnd { .. } => {
                                scalar_type.bitwise_and(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseOr { .. } => {
                                scalar_type.bitwise_or(left_value, right_value)
                            }
                            OperationKind::IntegerBitwiseXor { .. } => {
                                scalar_type.bitwise_xor(left_value, right_value)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::WrappingIntegerShiftLeft { value, count }
                    | OperationKind::WrappingIntegerShiftRight { value, count }
                    | OperationKind::ExactIntegerShiftLeft { value, count, .. }
                    | OperationKind::ExactIntegerShiftRight { value, count, .. } => {
                        let ScalarType::Integer(value_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: actual_value_type,
                            value,
                        } = self
                            .values
                            .get(&value)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let TerminalScalarValue::Integer {
                            scalar_type: count_type,
                            value: count,
                        } = self
                            .values
                            .get(&count)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(count))?
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if actual_value_type != value_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind {
                            OperationKind::WrappingIntegerShiftLeft { .. } => {
                                value_type.wrapping_shift_left(value, count_type, count)
                            }
                            OperationKind::WrappingIntegerShiftRight { .. } => {
                                value_type.wrapping_shift_right(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftLeft { .. } => {
                                value_type.exact_shift_left(value, count_type, count)
                            }
                            OperationKind::ExactIntegerShiftRight { .. } => {
                                value_type.exact_shift_right(value, count_type, count)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer {
                                scalar_type: value_type,
                                value,
                            },
                        );
                    }
                    OperationKind::ExactIntegerAdd { left, right, .. }
                    | OperationKind::WrappingIntegerAdd { left, right }
                    | OperationKind::ExactIntegerSubtract { left, right, .. }
                    | OperationKind::WrappingIntegerSubtract { left, right }
                    | OperationKind::ExactIntegerMultiply { left, right, .. }
                    | OperationKind::ExactIntegerDivide { left, right, .. }
                    | OperationKind::ExactIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerDivide { left, right, .. }
                    | OperationKind::WrappingIntegerRemainder { left, right, .. }
                    | OperationKind::SaturatingIntegerDivide { left, right, .. }
                    | OperationKind::SaturatingIntegerRemainder { left, right, .. }
                    | OperationKind::WrappingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = match operation.kind {
                            OperationKind::ExactIntegerAdd { .. } => {
                                scalar_type.exact_add(left, right)
                            }
                            OperationKind::WrappingIntegerAdd { .. } => {
                                scalar_type.wrapping_add(left, right)
                            }
                            OperationKind::ExactIntegerSubtract { .. } => {
                                scalar_type.exact_sub(left, right)
                            }
                            OperationKind::WrappingIntegerSubtract { .. } => {
                                scalar_type.wrapping_sub(left, right)
                            }
                            OperationKind::ExactIntegerMultiply { .. } => {
                                scalar_type.exact_mul(left, right)
                            }
                            OperationKind::ExactIntegerDivide { .. } => {
                                scalar_type.exact_div(left, right)
                            }
                            OperationKind::ExactIntegerRemainder { .. } => {
                                scalar_type.exact_rem(left, right)
                            }
                            OperationKind::WrappingIntegerDivide { .. } => {
                                scalar_type.wrapping_div(left, right)
                            }
                            OperationKind::WrappingIntegerRemainder { .. } => {
                                scalar_type.wrapping_rem(left, right)
                            }
                            OperationKind::SaturatingIntegerDivide { .. } => {
                                scalar_type.saturating_div(left, right)
                            }
                            OperationKind::SaturatingIntegerRemainder { .. } => {
                                scalar_type.saturating_rem(left, right)
                            }
                            OperationKind::WrappingIntegerMultiply { .. } => {
                                scalar_type.wrapping_mul(left, right)
                            }
                            _ => unreachable!(),
                        }
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerAdd { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_add(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerSubtract { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_sub(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                    OperationKind::SaturatingIntegerMultiply { left, right } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let left = self
                            .values
                            .get(&left)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
                        let right = self
                            .values
                            .get(&right)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
                        let (
                            TerminalScalarValue::Integer {
                                scalar_type: left_type,
                                value: left,
                            },
                            TerminalScalarValue::Integer {
                                scalar_type: right_type,
                                value: right,
                            },
                        ) = (left, right)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if left_type != scalar_type || right_type != scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = scalar_type
                            .saturating_mul(left, right)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.values.insert(
                            operation.result.expect_scalar().id,
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                    }
                }
                self.next_operation += 1;
            }
            let terminator = &machines
                .get(&self.current_machine)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                    self.current_machine,
                ))?
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator;
            match &terminator {
                Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if machine.result != TerminalMachineResult::Unit
                        || has_live_linear_claims(&self.live_claims)
                        || !self.live_claims.is_empty()
                        || cleanups.is_empty()
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    let mut expected_frontier = BTreeSet::new();
                    let mut cleanup_values = Vec::with_capacity(cleanups.len());
                    for cleanup in cleanups {
                        let value = self.structural_values.get(&cleanup.place).cloned().ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(cleanup.place),
                        )?;
                        if value.structural_type != cleanup.structural_type
                            || !expected_frontier.insert(StructuralAffineDiscard {
                                place: cleanup.place,
                                path: Vec::new(),
                                structural_type: cleanup.structural_type,
                            })
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        self.machines.get(&cleanup.cleanup_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(
                                cleanup.cleanup_machine,
                            ),
                        )?;
                        cleanup_values.push((cleanup.clone(), value));
                    }
                    if self.structural_values.len()
                        != cleanup_values.len() + self.primitive_local_count()
                        || self.live_affine_frontier != expected_frontier
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    for (cleanup, _) in &cleanup_values {
                        self.structural_values.remove(&cleanup.place).expect(
                            "validated nominal cleanup roots remain live through edge charge",
                        );
                    }
                    self.retire_plain_locals();
                    if !self.structural_values.is_empty() {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    let (completed, remaining) = cleanup_values
                        .split_first()
                        .expect("non-empty nominal cleanup list was validated");
                    let completed = completed.clone();
                    let remaining = remaining.to_vec();
                    let machines = std::sync::Arc::clone(&self.machines);
                    let callee = machines
                        .get(&completed.0.cleanup_machine)
                        .expect("all nominal cleanup targets were validated before edge charge");
                    self.call_stack.push(SuspendedCall {
                        values: std::mem::take(&mut self.values),
                        structural_values: std::mem::take(&mut self.structural_values),
                        byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
                        scalar_case_values: std::mem::take(&mut self.scalar_case_values),
                        scalar_array_values: std::mem::take(&mut self.scalar_array_values),
                        live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                        live_claims: std::mem::take(&mut self.live_claims),
                        dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
                        current_machine: self.current_machine,
                        current: self.current,
                        next_operation: self.next_operation,
                        result: SuspendedCallResult::NominalCleanups {
                            completed,
                            remaining,
                            final_result: None,
                        },
                    });
                    self.values = BTreeMap::new();
                    self.structural_values = BTreeMap::new();
                    self.live_affine_frontier = BTreeSet::new();
                    self.live_claims = BTreeMap::new();
                    self.dynamic_parameters = BTreeMap::new();
                    self.current_machine = cleanups[0].cleanup_machine;
                    self.current = callee.entry;
                    self.next_operation = 0;
                    continue;
                }
                Terminator::ReturnUnitPartialAffine {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if machine.result != TerminalMachineResult::Unit
                        || has_live_linear_claims(&self.live_claims)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }

                    // Validate the entire cleanup transaction before charging or
                    // mutating state. In particular, a projected cleanup may not
                    // be approximated by deleting its root-addressed carrier.
                    let mut expected_frontier = BTreeSet::new();
                    for place in trivial_affine_discards {
                        let structural_type = self
                            .structural_values
                            .get(place)
                            .map(|value| value.structural_type)
                            .or_else(|| {
                                self.scalar_case_values
                                    .get(place)
                                    .map(|value| value.structural_type)
                            })
                            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ))?;
                        if !expected_frontier.insert(StructuralAffineDiscard {
                            place: *place,
                            path: Vec::new(),
                            structural_type,
                        }) {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                    }
                    for discard in residual_affine_discards {
                        let root = self.structural_values.get(&discard.place).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
                        )?;
                        let actual_type = resolve_structural_path_type(
                            &self.structural_types,
                            root.structural_type,
                            &discard.path,
                        )?;
                        if actual_type != discard.structural_type
                            || discard.path.is_empty()
                            || !expected_frontier.insert(discard.clone())
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                    }
                    if expected_frontier != self.live_affine_frontier {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }

                    // The edge is now committed. Root cleanup may release its
                    // root-addressed carrier; projected cleanup removes only the
                    // exact semantic paths and leaves the opaque root untouched.
                    for place in trivial_affine_discards {
                        reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        );
                        self.scalar_case_values.remove(place);
                        self.live_affine_frontier.remove(&StructuralAffineDiscard {
                            place: *place,
                            path: Vec::new(),
                            structural_type: expected_frontier
                                .iter()
                                .find(|entry| entry.place == *place && entry.path.is_empty())
                                .expect("validated root affine cleanup")
                                .structural_type,
                        });
                    }
                    for discard in residual_affine_discards {
                        self.live_affine_frontier.remove(discard);
                    }
                    debug_assert!(self.live_affine_frontier.is_empty());

                    if let Some(caller) = self.call_stack.pop() {
                        if !matches!(caller.result, SuspendedCallResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.values = caller.values;
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Unit;
                    self.retire_plain_locals();
                    self.result = Some(result.clone());
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Jump {
                    target,
                    arguments,
                    structural_arguments,
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => {
                    if let Some(first) = residual_affine_discards.first() {
                        let root = self.structural_values.get(&first.place).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(first.place),
                        )?;
                        let mut expected = BTreeSet::new();
                        for discard in residual_affine_discards {
                            if discard.place != first.place
                                || discard.path.is_empty()
                                || resolve_structural_path_type(
                                    &self.structural_types,
                                    root.structural_type,
                                    &discard.path,
                                )? != discard.structural_type
                                || !expected.insert(discard.clone())
                            {
                                return Err(TerminalInterpretError::AffineFrontierMismatch);
                            }
                        }
                        if !trivial_affine_discards.is_empty()
                            || self
                                .live_affine_frontier
                                .iter()
                                .filter(|entry| entry.place == first.place)
                                .cloned()
                                .collect::<BTreeSet<_>>()
                                != expected
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    let bindings =
                        self.prepare_block_bindings(*target, arguments, structural_arguments)?;
                    bindings.validate_discards(
                        self,
                        trivial_affine_discards,
                        residual_affine_discards,
                    )?;
                    for discard in residual_affine_discards {
                        self.live_affine_frontier.remove(discard);
                    }
                    if let Some(first) = residual_affine_discards.first() {
                        // Every remaining semantic path was validated and disposed.
                        // Only now may the dead result's opaque backing leave storage.
                        reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            first.place,
                        );
                    }
                    for place in trivial_affine_discards {
                        if reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        )
                        .is_none()
                            && self.scalar_case_values.remove(place).is_none()
                        {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    bindings.commit(self);
                    self.current = *target;
                    self.next_operation = 0;
                }
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let condition = self
                        .values
                        .get(condition)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*condition))?;
                    let TerminalScalarValue::Boolean(condition) = condition else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    let successor = if condition { when_true } else { when_false };
                    if let Err(error) = meter.charge_edge(successor.edge, terminator) {
                        return meter_status(error);
                    }
                    let bindings = self.prepare_block_bindings(
                        successor.target,
                        &successor.arguments,
                        &successor.structural_arguments,
                    )?;
                    bindings.validate_discards(self, &successor.trivial_affine_discards, &[])?;
                    for place in &successor.trivial_affine_discards {
                        if reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        )
                        .is_none()
                            && self.scalar_case_values.remove(place).is_none()
                        {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    bindings.commit(self);
                    self.current = successor.target;
                    self.next_operation = 0;
                }
                Terminator::StructuralCase { source, cases } => {
                    // Only internally established values carry a discriminator
                    // and scalar payload. Opaque host roots remain unsupported.
                    let value = self.scalar_case_values.get(source).ok_or(
                        TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
                    )?;
                    let successor = cases
                        .iter()
                        .find(|successor| successor.case == value.result_case)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    let target = self
                        .machines
                        .get(&self.current_machine)
                        .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                            self.current_machine,
                        ))?
                        .blocks
                        .get(&successor.target)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    if target.parameters.len() != successor.payload_fields.len() {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    let mut bindings = Vec::with_capacity(target.parameters.len());
                    for (parameter, field) in
                        target.parameters.iter().zip(&successor.payload_fields)
                    {
                        let scalar = value
                            .fields
                            .iter()
                            .find(|(identity, _)| identity == field)
                            .map(|(_, scalar)| *scalar)
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        if scalar.scalar_type() != parameter.scalar_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        bindings.push((parameter.id, scalar));
                    }
                    for place in &successor.trivial_affine_discards {
                        if !self.structural_values.contains_key(place)
                            && !self.scalar_case_values.contains_key(place)
                        {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                    }
                    if let Err(error) = meter.charge_edge(successor.edge, terminator) {
                        return meter_status(error);
                    }
                    for place in &successor.trivial_affine_discards {
                        reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        );
                        self.scalar_case_values.remove(place);
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    self.values.extend(bindings);
                    self.current = successor.target;
                    self.next_operation = 0;
                }
                Terminator::Return {
                    value,
                    cleanup_actions,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if !matches!(machine.result, TerminalMachineResult::Scalar(_))
                        || has_live_linear_claims(&self.live_claims)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    let result = self
                        .values
                        .get(value)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
                    for parameter in machine
                        .structural_parameters
                        .iter()
                        .chain(
                            machine
                                .blocks
                                .values()
                                .flat_map(|block| &block.structural_parameters),
                        )
                        .filter(|parameter| {
                            parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                || (parameter.is_self
                                    && parameter.access != StructuralAccess::Owned)
                        })
                    {
                        self.structural_values.remove(&parameter.place);
                    }
                    // Frame-local byte views and loans end here and carry
                    // no affine cleanup, including locally established literals.
                    for place in self.byte_sequence_values.keys() {
                        reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        );
                    }
                    self.byte_sequence_values.clear();
                    self.retire_plain_locals();
                    let cleanups = commit_cleanup_actions(
                        &self.structural_types,
                        &self.machines,
                        &mut self.structural_values,
                        &mut self.reference_referents,
                        &mut self.scalar_case_values,
                        &mut self.live_affine_frontier,
                        &mut self.live_claims,
                        cleanup_actions,
                    )?;
                    if let Some((completed, remaining)) = cleanups.split_first() {
                        let completed = completed.clone();
                        let machines = std::sync::Arc::clone(&self.machines);
                        let callee = machines
                            .get(&completed.0.cleanup_machine)
                            .expect("verified cleanup target remains installed");
                        self.call_stack.push(SuspendedCall {
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
                            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
                            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
                            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                            live_claims: std::mem::take(&mut self.live_claims),
                            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
                            current_machine: self.current_machine,
                            current: self.current,
                            next_operation: self.next_operation,
                            result: SuspendedCallResult::NominalCleanups {
                                completed: completed.clone(),
                                remaining: remaining.to_vec(),
                                final_result: Some(result),
                            },
                        });
                        self.values = BTreeMap::new();
                        self.structural_values = BTreeMap::new();
                        self.live_affine_frontier = BTreeSet::new();
                        self.live_claims = BTreeMap::new();
                        self.dynamic_parameters = BTreeMap::new();
                        self.current_machine = completed.0.cleanup_machine;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        continue;
                    }
                    if let Some(caller) = self.call_stack.pop() {
                        let SuspendedCallResult::Scalar(result_value) = caller.result else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values = caller.values;
                        self.values.insert(result_value, result);
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Scalar(result);
                    self.retire_plain_locals();
                    self.result = Some(result.clone());
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    if machine.result != TerminalMachineResult::Unit
                        || has_live_linear_claims(&self.live_claims)
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    for place in trivial_affine_discards {
                        if reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        )
                        .is_none()
                            && self.scalar_case_values.remove(place).is_none()
                        {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    if let Some(caller) = self.call_stack.pop() {
                        let result = caller.result;
                        self.values = caller.values;
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        match result {
                            SuspendedCallResult::Unit => {}
                            SuspendedCallResult::NominalCleanups {
                                completed,
                                mut remaining,
                                final_result,
                            } => {
                                if !self.structural_values.is_empty()
                                    || !self.live_affine_frontier.remove(&StructuralAffineDiscard {
                                        place: completed.0.place,
                                        path: Vec::new(),
                                        structural_type: completed.1.structural_type,
                                    })
                                {
                                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                                }
                                if !remaining.is_empty() {
                                    let completed = remaining.remove(0);
                                    let machines = std::sync::Arc::clone(&self.machines);
                                    let callee = machines.get(&completed.0.cleanup_machine).ok_or(
                                        TerminalInterpretError::VerifiedCallTargetMissing(
                                            completed.0.cleanup_machine,
                                        ),
                                    )?;
                                    self.call_stack.push(SuspendedCall {
                                        values: std::mem::take(&mut self.values),
                                        structural_values: std::mem::take(
                                            &mut self.structural_values,
                                        ),
                                        byte_sequence_values: std::mem::take(
                                            &mut self.byte_sequence_values,
                                        ),
                                        scalar_case_values: std::mem::take(
                                            &mut self.scalar_case_values,
                                        ),
                                        scalar_array_values: std::mem::take(
                                            &mut self.scalar_array_values,
                                        ),
                                        live_affine_frontier: std::mem::take(
                                            &mut self.live_affine_frontier,
                                        ),
                                        live_claims: std::mem::take(&mut self.live_claims),
                                        dynamic_parameters: std::mem::take(
                                            &mut self.dynamic_parameters,
                                        ),
                                        current_machine: self.current_machine,
                                        current: self.current,
                                        next_operation: self.next_operation,
                                        result: SuspendedCallResult::NominalCleanups {
                                            completed: completed.clone(),
                                            remaining,
                                            final_result,
                                        },
                                    });
                                    self.values = BTreeMap::new();
                                    self.structural_values = BTreeMap::new();
                                    self.live_affine_frontier = BTreeSet::new();
                                    self.live_claims = BTreeMap::new();
                                    self.dynamic_parameters = BTreeMap::new();
                                    self.current_machine = completed.0.cleanup_machine;
                                    self.current = callee.entry;
                                    self.next_operation = 0;
                                    continue;
                                }
                                if !self.live_affine_frontier.is_empty() {
                                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                                }
                                if let Some(returned) = final_result
                                    && let Some(caller) = self.call_stack.pop()
                                {
                                    let SuspendedCallResult::Scalar(result_value) = caller.result
                                    else {
                                        return Err(
                                            TerminalInterpretError::VerifiedOperationMalformed,
                                        );
                                    };
                                    self.values = caller.values;
                                    self.values.insert(result_value, returned);
                                    self.retire_plain_locals();
                                    self.structural_values = caller.structural_values;
                                    self.scalar_case_values = caller.scalar_case_values;
                                    self.scalar_array_values = caller.scalar_array_values;
                                    self.byte_sequence_values = caller.byte_sequence_values;
                                    self.live_affine_frontier = caller.live_affine_frontier;
                                    self.live_claims = caller.live_claims;
                                    self.dynamic_parameters = caller.dynamic_parameters;
                                    self.current_machine = caller.current_machine;
                                    self.current = caller.current;
                                    self.next_operation = caller.next_operation;
                                    continue;
                                }
                                let result = final_result.map_or(
                                    TerminalExecutionResult::Unit,
                                    TerminalExecutionResult::Scalar,
                                );
                                self.retire_plain_locals();
                                self.result = Some(result.clone());
                                return Ok(TerminalExecutionStatus::Complete(result));
                            }
                            SuspendedCallResult::Scalar(_)
                            | SuspendedCallResult::Structural { .. } => {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                        }
                        continue;
                    }
                    let result = TerminalExecutionResult::Unit;
                    self.retire_plain_locals();
                    self.result = Some(result.clone());
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::ReturnStructural {
                    source,
                    returned_claims,
                    trivial_affine_discards,
                    ..
                } => {
                    let machine = self.machines.get(&self.current_machine).ok_or(
                        TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                    )?;
                    let Some(signature) = machine.result.structural() else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    if terminal_semantics::scalar_array_leaf_shape(
                        self.structural_types.values(),
                        signature.structural_type,
                    )
                    .is_some()
                    {
                        if let Some(status) = self.return_scalar_array(terminator, meter)? {
                            return Ok(status);
                        }
                        continue;
                    }
                    if let Some(value) = self.scalar_case_values.get(source).cloned() {
                        let internal_result = match self.call_stack.last() {
                            Some(SuspendedCall {
                                structural_values,
                                scalar_case_values,
                                live_affine_frontier,
                                result:
                                    SuspendedCallResult::Structural {
                                        result,
                                        returned_claim_transfers,
                                        ..
                                    },
                                ..
                            }) if result.multiplicity == signature.multiplicity
                                && result.qualifications.is_empty()
                                && result.claims.is_empty()
                                && returned_claim_transfers.is_empty()
                                && !structural_values.contains_key(&result.place)
                                && !scalar_case_values.contains_key(&result.place)
                                && live_affine_frontier
                                    .iter()
                                    .all(|entry| entry.place != result.place) =>
                            {
                                Some(result.clone())
                            }
                            Some(_) => {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            None => None,
                        };
                        if value.structural_type != signature.structural_type
                            || !signature.qualifications.is_empty()
                            || !returned_claims.is_empty()
                            || !self.live_claims.is_empty()
                            || trivial_affine_discards.iter().any(|place| {
                                *place == *source
                                    || (!self.structural_values.contains_key(place)
                                        && !self.scalar_case_values.contains_key(place))
                            })
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        if let Err(error) = meter.charge_terminator(terminator) {
                            return meter_status(error);
                        }
                        self.scalar_case_values.remove(source);
                        remove_affine_root(&mut self.live_affine_frontier, *source);
                        for place in trivial_affine_discards {
                            reference::discard_structural_value(
                                &mut self.structural_values,
                                &mut self.reference_referents,
                                *place,
                            );
                            self.scalar_case_values.remove(place);
                            remove_affine_root(&mut self.live_affine_frontier, *place);
                        }
                        if let Some(result) = internal_result {
                            let caller = self
                                .call_stack
                                .pop()
                                .expect("an internal scalar-case return has a caller frame");
                            let SuspendedCallResult::Structural { .. } = caller.result else {
                                unreachable!("scalar-case return preflight matched its caller")
                            };
                            self.values = caller.values;
                            self.retire_plain_locals();
                            self.structural_values = caller.structural_values;
                            self.scalar_case_values = caller.scalar_case_values;
                            self.scalar_array_values = caller.scalar_array_values;
                            self.byte_sequence_values = caller.byte_sequence_values;
                            if self
                                .scalar_case_values
                                .insert(result.place, value)
                                .is_some()
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            self.live_affine_frontier = caller.live_affine_frontier;
                            if result.multiplicity == StructuralMultiplicity::Affine {
                                self.live_affine_frontier.insert(StructuralAffineDiscard {
                                    place: result.place,
                                    path: Vec::new(),
                                    structural_type: result.structural_type,
                                });
                            }
                            self.live_claims = caller.live_claims;
                            self.dynamic_parameters = caller.dynamic_parameters;
                            self.current_machine = caller.current_machine;
                            self.current = caller.current;
                            self.next_operation = caller.next_operation;
                            continue;
                        }
                        let result =
                            TerminalExecutionResult::ScalarCase(TerminalScalarCaseResult { value });
                        self.retire_plain_locals();
                        self.result = Some(result.clone());
                        return Ok(TerminalExecutionStatus::Complete(result));
                    }
                    let value = self.structural_values.get(source).cloned().ok_or(
                        TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
                    )?;
                    self.validate_reference_return(signature, &value)?;
                    if value.structural_type != signature.structural_type
                        || signature
                            .qualifications
                            .iter()
                            .any(|domain| !value.qualifications.contains(domain))
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    let actual_claims = self
                        .live_claims
                        .iter()
                        .filter_map(|(claim, live)| (live.place == Some(*source)).then_some(*claim))
                        .collect::<Vec<_>>();
                    if actual_claims != *returned_claims
                        || self
                            .live_claims
                            .keys()
                            .any(|claim| !returned_claims.contains(claim))
                        || trivial_affine_discards.iter().any(|place| {
                            *place == *source || !self.structural_values.contains_key(place)
                        })
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    let internal_return = match self.call_stack.last() {
                        Some(SuspendedCall {
                            structural_values,
                            live_affine_frontier,
                            live_claims,
                            result:
                                SuspendedCallResult::Structural {
                                    result,
                                    returned_claim_transfers,
                                    ..
                                },
                            ..
                        }) => {
                            if result.structural_type != signature.structural_type
                                || result.multiplicity != signature.multiplicity
                                || result.qualifications != signature.qualifications
                                || structural_values.contains_key(&result.place)
                                || live_affine_frontier
                                    .iter()
                                    .any(|entry| entry.place == result.place)
                                || result
                                    .qualifications
                                    .iter()
                                    .any(|domain| !value.qualifications.contains(domain))
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            Some((
                                result.clone(),
                                rebind_structural_result_claims(
                                    live_claims,
                                    &self.live_claims,
                                    *source,
                                    result,
                                    returned_claim_transfers,
                                    returned_claims,
                                )?,
                            ))
                        }
                        Some(_) => {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        None => None,
                    };
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    // Commit only after fuel and every structural/claim check succeeds.
                    self.structural_values.remove(source);
                    remove_affine_root(&mut self.live_affine_frontier, *source);
                    for claim in returned_claims {
                        self.live_claims.remove(claim);
                    }
                    for place in trivial_affine_discards {
                        reference::discard_structural_value(
                            &mut self.structural_values,
                            &mut self.reference_referents,
                            *place,
                        );
                        self.scalar_case_values.remove(place);
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    if let Some((result, rebound_claims)) = internal_return {
                        let caller = self
                            .call_stack
                            .pop()
                            .expect("an internal structural return has a caller frame");
                        let SuspendedCallResult::Structural { .. } = caller.result else {
                            unreachable!("preflight matched the structural caller frame")
                        };
                        self.values = caller.values;
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        if self.structural_values.insert(result.place, value).is_some() {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.live_affine_frontier = caller.live_affine_frontier;
                        if result.multiplicity == StructuralMultiplicity::Affine
                            && !self.live_affine_frontier.insert(StructuralAffineDiscard {
                                place: result.place,
                                path: Vec::new(),
                                structural_type: result.structural_type,
                            })
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        self.live_claims = rebound_claims;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Structural(TerminalStructuralResult {
                        value,
                        claims: returned_claims.clone(),
                    });
                    self.retire_plain_locals();
                    self.result = Some(result.clone());
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Crash {
                    edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                    ..
                } => {
                    if frontier_lower_bound != &self.live_claims.keys().copied().collect::<Vec<_>>()
                    {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    if let Err(error) = meter.charge_terminator(terminator) {
                        return meter_status(error);
                    }
                    let crash = TerminalCrash {
                        site: TerminalCrashSite::Edge(*edge),
                        cause: *cause,
                        site_guard: site_guard.clone(),
                        frontier_lower_bound: frontier_lower_bound.clone(),
                    };
                    self.crash = Some(crash.clone());
                    return Ok(TerminalExecutionStatus::Crashed(crash));
                }
            }
        }
    }
}

fn meter_status(error: FuelMeterError) -> Result<TerminalExecutionStatus, TerminalInterpretError> {
    match error {
        FuelMeterError::Exhausted(exhaustion) => {
            Ok(TerminalExecutionStatus::SponsorExhausted(exhaustion))
        }
        other => Err(TerminalInterpretError::Fuel(other)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalExecutionStatus {
    Complete(TerminalExecutionResult),
    SponsorExhausted(FuelExhaustion),
    Crashed(TerminalCrash),
}

/// The explicit terminal-Psi crash outcome reached by an execution.
///
/// `frontier_lower_bound` is the artifact-retained frontier for an authored edge
/// or the interpreter's current machine-local live claims for a boundary crash.
/// Neither asserts that no suspended caller or wider runtime state was abandoned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCrash {
    pub site: TerminalCrashSite,
    pub cause: CrashCause,
    /// Static guard on an authored crash edge. Boundary invocations instead
    /// validate declaration-local routes against their observed effect inputs;
    /// they do not invent a local edge guard.
    pub site_guard: Vec<terminal_psi::CrashPredicateTerm>,
    pub frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCrashSite {
    Edge(semantic_vocabulary::EdgeId),
    BoundaryCall {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
}

/// A successful semantic result paired with deterministic terminal-Psi fuel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredTerminalExecution {
    value: TerminalExecutionResult,
    usage: TerminalFuelUsage,
    effects: Vec<TerminalEffect>,
    structural_primitive_values: Vec<TerminalStructuralPrimitiveValue>,
}

impl MeasuredTerminalExecution {
    pub fn value(&self) -> TerminalExecutionResult {
        self.value.clone()
    }

    pub const fn usage(&self) -> &TerminalFuelUsage {
        &self.usage
    }

    pub fn effects(&self) -> &[TerminalEffect] {
        &self.effects
    }

    pub fn structural_primitive_values(&self) -> &[TerminalStructuralPrimitiveValue] {
        &self.structural_primitive_values
    }

    pub fn into_value(self) -> TerminalExecutionResult {
        self.value
    }

    pub fn into_parts(self) -> (TerminalExecutionResult, TerminalFuelUsage) {
        (self.value, self.usage)
    }

    pub fn into_parts_with_effects(
        self,
    ) -> (
        TerminalExecutionResult,
        TerminalFuelUsage,
        Vec<TerminalEffect>,
    ) {
        (self.value, self.usage, self.effects)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
    ScalarEntryQualificationUnsupported,
    UnsupportedSemanticVariant(&'static str),
    /// The artifact's exact affine cleanup transaction does not match the
    /// interpreter's live ownership paths.
    AffineFrontierMismatch,
    /// A projection cannot be represented exactly by the interpreter's current
    /// path-aware structural model, so execution fails closed.
    AffineProjectionNotRepresentable,
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        value: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    ArgumentIntegerOutsideType {
        value: ValueId,
    },
    StructuralArgumentCount {
        expected: usize,
        actual: usize,
    },
    StructuralArgumentType {
        place: PlaceId,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    StructuralQualificationsNonCanonical,
    StructuralQualificationMissing(PlaceId),
    StructuralArgumentAliasing(u64),
    StructuralPrimitiveValueCount {
        expected: usize,
        actual: usize,
    },
    StructuralPrimitiveValueInvalid {
        argument_index: u32,
    },
    StructuralPrimitiveValueType {
        argument_index: u32,
        expected: ScalarType,
        actual: ScalarType,
    },
    StructuralPrimitiveStorageMissing(PlaceId),
    StructuralIdentityExhausted,
    StructuralScalarFieldMissing {
        source: PlaceId,
        field: StructuralFieldId,
    },
    StructuralScalarFieldArgumentInvalid {
        argument_index: u32,
        field: StructuralFieldId,
    },
    StructuralBooleanFieldArgumentInvalid {
        argument_index: u32,
        field: StructuralFieldId,
    },
    StructuralBooleanFieldMissing {
        source: PlaceId,
        field: StructuralFieldId,
    },
    BoundaryQualificationMissing {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    ClaimTransferMismatch,
    CompletionReceiptMismatch,
    ProviderInstallationIdentityMismatch,
    ProviderInstallationMissing(BoundaryMachineId),
    VerifiedEntryMachineMissing,
    VerifiedCallTargetMissing(MachineId),
    VerifiedBoundaryMachineMissing(BoundaryMachineId),
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
    VerifiedStructuralPlaceMissing(PlaceId),
    VerifiedValueMissing(ValueId),
    EffectRejected {
        operation: OperationId,
        rejection: TerminalEffectRejection,
    },
    BoundaryCrashNotPermitted {
        operation: OperationId,
        boundary: BoundaryMachineId,
        cause: CrashCause,
        reason: terminal_verifier::BoundaryCrashOutcomeError,
    },
    Crash(TerminalCrash),
    Fuel(FuelMeterError),
}

#[derive(Debug)]
pub enum ProviderInstallationError {
    SemanticDecode(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    UnknownOrDuplicateSelection {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
}

#[derive(Debug)]
pub enum TerminalArtifactInterpretError {
    ArtifactDecode(terminal_codec::CanonicalTerminalArtifactError),
    SemanticDecode(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    Execution(TerminalInterpretError),
}

impl std::fmt::Display for TerminalArtifactInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactInterpretError {}

impl From<FuelMeterError> for TerminalInterpretError {
    fn from(error: FuelMeterError) -> Self {
        Self::Fuel(error)
    }
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInterpretError {}
