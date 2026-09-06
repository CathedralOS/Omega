//! Fuel-bounded reference execution for verified terminal-Psi artifacts.
//!
//! The public entry accepts only canonical semantic/proof bytes and an
//! admission profile. It decodes and verifies those bytes before constructing
//! execution state; no source or checked-tree representation crosses this
//! boundary.

mod effect_results;
mod semantic_value_comparison;

pub use effect_results::TerminalEffectResult;

pub use semantic_value_comparison::{
    TerminalTraceScalarComparisonError, TerminalTraceScalarValueSide,
    TerminalTraceStructuralComparisonError, TerminalTraceStructuralValueSide,
    compare_terminal_trace_scalar_values, compare_terminal_trace_structural_values,
};

use std::collections::{BTreeMap, BTreeSet};

use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, IeeeFloatFormat, IeeeFloatValue, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ServiceId, StructuralCaseId,
    StructuralDomainId, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_fuel::{FuelExhaustion, FuelMeterError, TerminalFuelMeter, TerminalFuelUsage};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, ClaimTransfer, CompletionReceipt,
    CrashCause, EntryClaim, NominalAffineCleanup, OperationKind, OperationResult, StructuralAccess,
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultClaimTransfer,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalMachineResult, Terminator,
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

/// Exact target-neutral runtime carrier for a payloadless structural sum case.
///
/// This is deliberately distinct from an opaque host structural value: a
/// producer-created case has no host identity to preserve or invent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalPayloadlessCaseValue {
    pub structural_type: StructuralTypeId,
    pub result_case: StructuralCaseId,
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
        /// Exact byte payload aligned with `structural_arguments`. Only a
        /// first-class byte-sequence literal contributes `Some`; opaque host
        /// structural arguments remain `None`.
        byte_sequence_arguments: Vec<Option<Vec<u8>>>,
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
    PayloadlessCase(TerminalPayloadlessCaseResult),
}

/// A structural value returned with the exact live claims transferred into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralResult {
    pub value: TerminalStructuralValue,
    pub claims: Vec<ClaimId>,
}

/// A returned payloadless sum case. Such a value carries no runtime payload
/// and therefore cannot carry structural claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPayloadlessCaseResult {
    pub value: TerminalPayloadlessCaseValue,
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
    machines: BTreeMap<MachineId, ExecutableMachine>,
    dynamic_scalar_calls: BTreeMap<(MachineId, u32), (MachineId, StructuralArgument)>,
    dynamic_descriptor_templates: BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    dynamic_selection_templates: BTreeMap<(MachineId, u32), RuntimeDynamicDescriptorTemplate>,
    dynamic_descriptor_arguments:
        BTreeMap<(MachineId, OperationId), Vec<terminal_psi::TerminalDynamicDescriptorArgument>>,
    dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    boundary_machines: BTreeMap<BoundaryMachineId, BoundaryMachineDeclaration>,
    provider_candidates: BTreeSet<BoundaryMachineId>,
    provider_installation: BTreeMap<BoundaryMachineId, MachineId>,
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    /// Mutable primitive contents live outside call frames. Machine-local
    /// place maps are only views into this stable logical storage arena.
    structural_primitive_storage: BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
    structural_primitive_entry_places: BTreeMap<u32, StructuralRuntimePlace>,
    /// Scalar leaves written below aggregate structural values. Keys use the
    /// invocation-independent opaque identity and resolved parent path, so a
    /// projected call observes the same field without native layout claims.
    structural_scalar_fields: BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>,
    payloadless_case_values: BTreeMap<PlaceId, TerminalPayloadlessCaseValue>,
    /// Immutable exact literal payloads keyed by invocation-independent
    /// terminal machine/place identity. Literal operations are canonical and
    /// idempotently establish the same bytes on every invocation.
    byte_sequence_literals: BTreeMap<(MachineId, PlaceId), Vec<u8>>,
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

#[derive(Clone)]
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
    blocks: BTreeMap<BlockId, Block>,
    values: BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: BTreeMap<PlaceId, TerminalStructuralValue>,
    payloadless_case_values: BTreeMap<PlaceId, TerminalPayloadlessCaseValue>,
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
    },
    NominalCleanups {
        completed: (NominalAffineCleanup, TerminalStructuralValue),
        remaining: Vec<(NominalAffineCleanup, TerminalStructuralValue)>,
        final_result: Option<TerminalScalarValue>,
    },
}

impl TerminalExecution {
    /// Canonical-decode, verify, and begin one resumable artifact execution.
    /// The resulting state owns its verified entry block graph, so no decoded
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
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let verified =
            terminal_verifier::verify_module_for_interpretation(&module, &proof, profile)
                .map_err(TerminalArtifactInterpretError::Verification)?;
        Self::start_verified_module(
            verified.module(),
            scalar_arguments,
            structural_arguments,
            structural_boolean_fields,
            structural_primitive_values,
            None,
        )
        .map_err(TerminalArtifactInterpretError::Execution)
    }

    /// Begin execution with one explicit provider installation previously
    /// admitted against these exact semantic/proof sections.
    pub fn start_artifact_with_provider_installation(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        installation: &AdmittedProviderInstallation,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        let proof = terminal_codec::decode_proof_bundle(proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let verified = terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        Self::start_verified_module(
            verified.module(),
            scalar_arguments,
            structural_arguments,
            &[],
            &[],
            Some(installation),
        )
        .map_err(TerminalArtifactInterpretError::Execution)
    }

    fn start_verified_module(
        module: &terminal_psi::TerminalModule,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        structural_boolean_field_arguments: &[TerminalStructuralBooleanFieldValue],
        structural_primitive_value_arguments: &[TerminalStructuralPrimitiveValue],
        installation: Option<&AdmittedProviderInstallation>,
    ) -> Result<Self, TerminalInterpretError> {
        let terminal_psi = terminal_codec::terminal_psi_identity(module)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        if installation.is_some_and(|installation| installation.terminal_psi != terminal_psi) {
            return Err(TerminalInterpretError::ProviderInstallationIdentityMismatch);
        }
        let machines = module
            .machines
            .iter()
            .map(|machine| {
                (
                    machine.id,
                    ExecutableMachine {
                        parameters: machine.parameters.clone(),
                        structural_parameters: machine.structural_parameters.clone(),
                        structural_places: machine.structural_places.clone(),
                        entry_claims: machine.entry_claims.clone(),
                        content_entry_claims: machine.content_entry_claims.clone(),
                        result: machine.result.clone(),
                        entry: machine.entry,
                        blocks: machine
                            .blocks
                            .iter()
                            .map(|block| (block.id, block.clone()))
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
        let structural_boolean_fields = bind_structural_boolean_fields(
            machine,
            &structural_types,
            &structural_values,
            structural_boolean_field_arguments,
        )?;
        let live_affine_frontier =
            bind_affine_frontier(&machine.structural_parameters, &structural_values)?;
        let live_claims = bind_entry_claims(
            &machine.entry_claims,
            &machine.content_entry_claims,
            &machine.structural_parameters,
            &structural_values,
        )?;
        let blocks = machine.blocks.clone();
        let current = machine.entry;
        Ok(Self {
            structural_types,
            machines,
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
            blocks,
            values,
            structural_values,
            structural_primitive_storage,
            structural_primitive_entry_places,
            structural_scalar_fields: structural_boolean_fields,
            payloadless_case_values: BTreeMap::new(),
            byte_sequence_literals: BTreeMap::new(),
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

    fn resolve_dynamic_call_arguments(
        &self,
        operation: OperationId,
    ) -> Result<BTreeMap<u32, RuntimeDynamicDescriptor>, TerminalInterpretError> {
        let mut resolved = BTreeMap::new();
        for argument in self
            .dynamic_descriptor_arguments
            .get(&(self.current_machine, operation))
            .into_iter()
            .flatten()
        {
            let descriptor = match argument.source {
                terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal } => {
                    let template = self
                        .dynamic_selection_templates
                        .get(&(self.current_machine, ordinal))
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    let sources = resolve_structural_arguments(
                        &self.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(&template.source),
                    )?;
                    let [source] = sources.as_slice() else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    RuntimeDynamicDescriptor {
                        source: source.clone(),
                        callables: template.callables.clone(),
                    }
                }
                terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                    let template = self
                        .dynamic_descriptor_templates
                        .get(&(self.current_machine, ordinal))
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    let sources = resolve_structural_arguments(
                        &self.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(&template.source),
                    )?;
                    let [source] = sources.as_slice() else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    RuntimeDynamicDescriptor {
                        source: source.clone(),
                        callables: template.callables.clone(),
                    }
                }
                terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal } => self
                    .dynamic_parameters
                    .get(&ordinal)
                    .cloned()
                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
            };
            if resolved
                .insert(argument.parameter_ordinal, descriptor)
                .is_some()
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        }
        Ok(resolved)
    }

    /// Enter one structural Unit callee after the operation-specific argument
    /// checks have succeeded. Ordinary calls and admitted provider dispatch
    /// share this exact ownership and continuation transition.
    fn begin_unit_call(
        &mut self,
        callee_id: MachineId,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        resolved_arguments: &[TerminalStructuralValue],
        claim_transfers: &[ClaimTransfer],
        dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self
            .machines
            .get(&callee_id)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if callee.result != TerminalMachineResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, resolved_arguments)?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;
        self.next_operation += 1;
        self.live_claims = remaining_claims;
        let mut caller_affine_frontier = std::mem::take(&mut self.live_affine_frontier);
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                consume_affine_projection(
                    &self.structural_types,
                    &self.structural_values,
                    &mut caller_affine_frontier,
                    argument,
                )?;
            }
        }
        let mut caller_structural_values = std::mem::take(&mut self.structural_values);
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if caller_structural_values.remove(&argument.place).is_none() {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }
        self.call_stack.push(SuspendedCall {
            blocks: std::mem::take(&mut self.blocks),
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Unit,
        });
        self.blocks = callee.blocks;
        self.values = values;
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = dynamic_parameters;
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    fn begin_structural_scalar_call(
        &mut self,
        callee_id: MachineId,
        result: terminal_psi::ValueDeclaration,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        claim_transfers: &[ClaimTransfer],
        dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self
            .machines
            .get(&callee_id)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if callee.result.scalar().map(|result| result.scalar_type) != Some(result.scalar_type) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let arguments = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            structural_arguments,
        )?;
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &arguments)?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;
        self.next_operation += 1;
        self.live_claims = remaining_claims;
        let mut caller_affine_frontier = std::mem::take(&mut self.live_affine_frontier);
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                consume_affine_projection(
                    &self.structural_types,
                    &self.structural_values,
                    &mut caller_affine_frontier,
                    argument,
                )?;
            }
        }
        let mut caller_structural_values = std::mem::take(&mut self.structural_values);
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if caller_structural_values.remove(&argument.place).is_none() {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }
        self.call_stack.push(SuspendedCall {
            blocks: std::mem::take(&mut self.blocks),
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Scalar(result.id),
        });
        self.blocks = callee.blocks;
        self.values = values;
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = dynamic_parameters;
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    fn begin_structural_result_call(
        &mut self,
        callee_id: MachineId,
        result: StructuralOperationResult,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        claim_transfers: &[ClaimTransfer],
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self
            .machines
            .get(&callee_id)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        let Some(callee_result) = callee.result.structural() else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let exact_payloadless_call = scalar_arguments.is_empty()
            && structural_arguments.is_empty()
            && callee.parameters.is_empty()
            && callee.structural_parameters.is_empty()
            && callee.entry_claims.is_empty()
            && callee.content_entry_claims.is_empty()
            && claim_transfers.is_empty()
            && returned_claim_transfers.is_empty()
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualifications.is_empty()
            && result.claims.is_empty();
        if structural_arguments
            .iter()
            .any(|argument| !argument.path.is_empty())
            || result.structural_type != callee_result.structural_type
            || result.multiplicity != callee_result.multiplicity
            || result.qualifications != callee_result.qualifications
            || (result.multiplicity == StructuralMultiplicity::Unrestricted
                && !exact_payloadless_call)
            || self.structural_values.contains_key(&result.place)
            || self.payloadless_case_values.contains_key(&result.place)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let arguments = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            structural_arguments,
        )?;
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &arguments)?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;

        let mut caller_affine_frontier = self.live_affine_frontier.clone();
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                consume_affine_projection(
                    &self.structural_types,
                    &self.structural_values,
                    &mut caller_affine_frontier,
                    argument,
                )?;
            }
        }
        let mut caller_structural_values = self.structural_values.clone();
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if caller_structural_values.remove(&argument.place).is_none() {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }

        self.next_operation += 1;
        self.call_stack.push(SuspendedCall {
            blocks: std::mem::take(&mut self.blocks),
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: remaining_claims,
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Structural {
                result,
                returned_claim_transfers,
            },
        });
        self.blocks = callee.blocks;
        self.values = values;
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    fn begin_runtime_dynamic_scalar_call(
        &mut self,
        callee_id: MachineId,
        result: terminal_psi::ValueDeclaration,
        source: TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self
            .machines
            .get(&callee_id)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if !callee.parameters.is_empty()
            || callee.structural_parameters.len() != 1
            || callee.result.scalar().map(|result| result.scalar_type) != Some(result.scalar_type)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &[source])?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
        self.next_operation += 1;
        self.call_stack.push(SuspendedCall {
            blocks: std::mem::take(&mut self.blocks),
            values: std::mem::take(&mut self.values),
            structural_values: std::mem::take(&mut self.structural_values),
            payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Scalar(result.id),
        });
        self.blocks = callee.blocks;
        self.values = BTreeMap::new();
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    fn begin_runtime_dynamic_unit_call(
        &mut self,
        callee_id: MachineId,
        source: TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self
            .machines
            .get(&callee_id)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if !callee.parameters.is_empty()
            || callee.structural_parameters.len() != 1
            || callee.result != TerminalMachineResult::Unit
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &[source])?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
        self.next_operation += 1;
        self.call_stack.push(SuspendedCall {
            blocks: std::mem::take(&mut self.blocks),
            values: std::mem::take(&mut self.values),
            structural_values: std::mem::take(&mut self.structural_values),
            payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Unit,
        });
        self.blocks = callee.blocks;
        self.values = BTreeMap::new();
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
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

        loop {
            while let Some(operation) = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .operations
                .get(self.next_operation)
                .cloned()
            {
                if let Err(error) = meter.charge_operation(&operation) {
                    return meter_status(error);
                }
                match operation.kind.clone() {
                    OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
                        if operation.result != terminal_psi::OperationResult::Unit
                            || !self
                                .dynamic_descriptor_templates
                                .contains_key(&(self.current_machine, descriptor_ordinal))
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                    }
                    OperationKind::EstablishPayloadlessCase { result_case } => {
                        let terminal_psi::OperationResult::Structural(result) = &operation.result
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if self.structural_values.contains_key(&result.place)
                            || self.payloadless_case_values.contains_key(&result.place)
                            || result.multiplicity != StructuralMultiplicity::Unrestricted
                            || !result.qualifications.is_empty()
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
                        if !cases
                            .iter()
                            .any(|case| case.id == result_case && case.fields.is_empty())
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        self.payloadless_case_values.insert(
                            result.place,
                            TerminalPayloadlessCaseValue {
                                structural_type: result.structural_type,
                                result_case,
                            },
                        );
                    }
                    OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
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
                        let key = (self.current_machine, destination);
                        if self
                            .byte_sequence_literals
                            .insert(key, bytes.clone())
                            .is_some_and(|previous| previous != bytes)
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
                    OperationKind::EstablishAffineScalarRecord { field, value } => {
                        let terminal_psi::OperationResult::Structural(result) = &operation.result
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if self.structural_values.contains_key(&result.place)
                            || result.multiplicity != StructuralMultiplicity::Affine
                            || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty()
                            || !result.claims.is_empty()
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let Some(ScalarType::Integer(scalar_type)) = direct_scalar_field_type(
                            &self.structural_types,
                            result.structural_type,
                            field,
                        ) else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Signed
                            || scalar_type.bits() != 64
                            || !scalar_type.admits(value)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let structural_value = TerminalStructuralValue {
                            opaque_identity: result.place.get(),
                            structural_type: result.structural_type,
                            qualifications: Vec::new(),
                            path: Vec::new(),
                        };
                        self.structural_scalar_fields.insert(
                            StructuralScalarRuntimeField {
                                parent: StructuralRuntimePlace::from(&structural_value),
                                field,
                            },
                            TerminalScalarValue::Integer { scalar_type, value },
                        );
                        self.structural_values
                            .insert(result.place, structural_value);
                        self.live_affine_frontier.insert(StructuralAffineDiscard {
                            place: result.place,
                            path: Vec::new(),
                            structural_type: result.structural_type,
                        });
                    }
                    OperationKind::CallUnit {
                        callee,
                        arguments: scalar_argument_ids,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let arguments = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            &structural_arguments,
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
                        let dynamic_parameters =
                            self.resolve_dynamic_call_arguments(operation.id)?;
                        self.begin_unit_call(
                            callee,
                            &scalar_arguments,
                            &structural_arguments,
                            &arguments,
                            &claim_transfers,
                            dynamic_parameters,
                        )?;
                        continue;
                    }
                    OperationKind::CallStructuralScalar {
                        callee,
                        arguments,
                        structural_arguments,
                        claim_transfers,
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
                            &structural_arguments,
                            &claim_transfers,
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
                        structural_arguments,
                        claim_transfers,
                        returned_claim_transfers,
                        ..
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .cloned()
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        self.begin_structural_result_call(
                            callee,
                            result,
                            &[],
                            &structural_arguments,
                            &claim_transfers,
                            returned_claim_transfers,
                        )?;
                        continue;
                    }
                    OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        arguments,
                        structural_arguments,
                        claim_transfers,
                        returned_claim_transfers,
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
                        self.begin_structural_result_call(
                            callee,
                            result,
                            &scalar_arguments,
                            &structural_arguments,
                            &claim_transfers,
                            returned_claim_transfers,
                        )?;
                        continue;
                    }
                    OperationKind::BoundaryCall {
                        boundary,
                        arguments: scalar_argument_ids,
                        structural_arguments,
                        completion_receipts,
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
                        let arguments = resolve_structural_arguments(
                            &self.structural_types,
                            &self.structural_values,
                            &structural_arguments,
                        )?;
                        bind_structural_arguments(
                            &boundary_declaration.structural_parameters,
                            &arguments,
                        )?;
                        validate_boundary_requirements(boundary_declaration, &arguments)?;
                        if self.provider_candidates.contains(&boundary) {
                            if !matches!(operation.result, terminal_psi::OperationResult::Unit)
                                || !scalar_argument_ids.is_empty()
                                || !boundary_declaration.scalar_parameters.is_empty()
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            let callee_id =
                                self.provider_installation.get(&boundary).copied().ok_or(
                                    TerminalInterpretError::ProviderInstallationMissing(boundary),
                                )?;
                            let claim_transfers = completion_receipts
                                .iter()
                                .map(|receipt| ClaimTransfer {
                                    claim: receipt.claim,
                                    argument_index: receipt.argument_index,
                                })
                                .collect::<Vec<_>>();
                            self.begin_unit_call(
                                callee_id,
                                &[],
                                &structural_arguments,
                                &arguments,
                                &claim_transfers,
                                BTreeMap::new(),
                            )?;
                            continue;
                        }
                        self.preflight_boundary_result(&operation.result)?;
                        let remaining_claims = complete_claims(
                            &self.live_claims,
                            &structural_arguments,
                            &completion_receipts,
                            &boundary_declaration.structural_parameters,
                        )?;
                        let effect = TerminalEffect::BoundaryCall {
                            operation: operation.id,
                            boundary,
                            arguments: scalar_arguments,
                            structural_arguments: arguments,
                            byte_sequence_arguments: structural_arguments
                                .iter()
                                .map(|argument| {
                                    argument
                                        .path
                                        .is_empty()
                                        .then(|| {
                                            self.byte_sequence_literals
                                                .get(&(self.current_machine, argument.place))
                                                .cloned()
                                        })
                                        .flatten()
                                })
                                .collect(),
                            completion_receipts,
                            result: boundary_declaration.result.clone(),
                        };
                        let returned =
                            handler.handle_effect_result(&effect).map_err(|rejection| {
                                TerminalInterpretError::EffectRejected {
                                    operation: operation.id,
                                    rejection,
                                }
                            })?;
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
                        callee, arguments, ..
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
                        let callee =
                            self.machines.get(&callee_id).cloned().ok_or(
                                TerminalInterpretError::VerifiedCallTargetMissing(callee_id),
                            )?;
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
                            blocks: std::mem::take(&mut self.blocks),
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            payloadless_case_values: std::mem::take(
                                &mut self.payloadless_case_values,
                            ),
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
                        self.blocks = callee.blocks;
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
                    OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let machine = self.machines.get(&self.current_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                        )?;
                        let parameter = machine
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == destination)
                            .filter(|parameter| {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                ) && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                    && parameter.qualifications.is_empty()
                            })
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let Some(StructuralTypeShape::PrimitiveScalar(expected_type)) = self
                            .structural_types
                            .get(&parameter.structural_type)
                            .map(|declaration| &declaration.shape)
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        let source = self
                            .values
                            .get(&value)
                            .copied()
                            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
                        if source.scalar_type() != *expected_type
                            || !terminal_scalar_belongs_to_type(source)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let destination_view = self.structural_values.get(&destination).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(destination),
                        )?;
                        if destination_view.structural_type != parameter.structural_type {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let storage_place = StructuralRuntimePlace::from(destination_view);
                        let stored = self
                            .structural_primitive_storage
                            .get_mut(&storage_place)
                            .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                                destination,
                            ))?;
                        *stored = source;
                    }
                    OperationKind::StructuralScalarFieldStore {
                        destination,
                        path,
                        field,
                        value,
                    } => {
                        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let machine = self.machines.get(&self.current_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
                        )?;
                        let parameter = machine
                            .structural_parameters
                            .iter()
                            .find(|parameter| parameter.place == destination)
                            .filter(|parameter| {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                ) && matches!(
                                    parameter.multiplicity,
                                    StructuralMultiplicity::Unrestricted
                                        | StructuralMultiplicity::Affine
                                ) && parameter.qualifications.is_empty()
                                    && parameter.projected_qualifications.is_empty()
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
                                path,
                                access: parameter.access,
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
                    OperationKind::BooleanStructuralField { source, field } => {
                        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let structural_value = self.structural_values.get(&source).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
                        )?;
                        let value = self
                            .structural_scalar_fields
                            .get(&StructuralScalarRuntimeField {
                                parent: StructuralRuntimePlace::from(structural_value),
                                field,
                            })
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
                    OperationKind::IntegerStructuralField { source, field } => {
                        let result = operation.result.expect_scalar();
                        if !matches!(result.scalar_type, ScalarType::Integer(_)) {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let structural_value = self.structural_values.get(&source).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
                        )?;
                        if direct_scalar_field_type(
                            &self.structural_types,
                            structural_value.structural_type,
                            field,
                        ) != Some(result.scalar_type)
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        let value = self
                            .structural_scalar_fields
                            .get(&StructuralScalarRuntimeField {
                                parent: StructuralRuntimePlace::from(structural_value),
                                field,
                            })
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
                        let result = match operation.kind.clone() {
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
                        let value = match operation.kind.clone() {
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
                        let value = match operation.kind.clone() {
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
                        let value = match operation.kind.clone() {
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
            let terminator = self
                .blocks
                .get(&self.current)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?
                .terminator
                .clone();
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
                    if self.structural_values.len() != cleanup_values.len()
                        || self.live_affine_frontier != expected_frontier
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    for (cleanup, _) in &cleanup_values {
                        self.structural_values.remove(&cleanup.place).expect(
                            "validated nominal cleanup roots remain live through edge charge",
                        );
                    }
                    if !self.structural_values.is_empty() {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    let (completed, remaining) = cleanup_values
                        .split_first()
                        .expect("non-empty nominal cleanup list was validated");
                    let completed = completed.clone();
                    let remaining = remaining.to_vec();
                    let callee = self
                        .machines
                        .get(&completed.0.cleanup_machine)
                        .cloned()
                        .expect("all nominal cleanup targets were validated before edge charge");
                    self.call_stack.push(SuspendedCall {
                        blocks: std::mem::take(&mut self.blocks),
                        values: std::mem::take(&mut self.values),
                        structural_values: std::mem::take(&mut self.structural_values),
                        payloadless_case_values: std::mem::take(&mut self.payloadless_case_values),
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
                    self.blocks = callee.blocks;
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
                        let value = self.structural_values.get(place).ok_or(
                            TerminalInterpretError::VerifiedStructuralPlaceMissing(*place),
                        )?;
                        if !expected_frontier.insert(StructuralAffineDiscard {
                            place: *place,
                            path: Vec::new(),
                            structural_type: value.structural_type,
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }

                    // The edge is now committed. Root cleanup may release its
                    // root-addressed carrier; projected cleanup removes only the
                    // exact semantic paths and leaves the opaque root untouched.
                    for place in trivial_affine_discards {
                        self.structural_values.remove(place);
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
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.structural_values = caller.structural_values;
                        self.payloadless_case_values = caller.payloadless_case_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Unit;
                    self.result = Some(result.clone());
                    return Ok(TerminalExecutionStatus::Complete(result));
                }
                Terminator::Jump {
                    target,
                    arguments,
                    trivial_affine_discards,
                    ..
                } => {
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(target)
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for place in trivial_affine_discards {
                        if self.structural_values.remove(place).is_none() {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
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
                    if let Err(error) = meter.charge_edge(successor.edge, &terminator) {
                        return meter_status(error);
                    }
                    let target_block = self
                        .blocks
                        .get(&successor.target)
                        .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
                    let transferred = successor
                        .arguments
                        .iter()
                        .map(|argument| {
                            self.values
                                .get(argument)
                                .copied()
                                .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for place in &successor.trivial_affine_discards {
                        if self.structural_values.remove(place).is_none() {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    for (parameter, value) in target_block.parameters.iter().zip(transferred) {
                        self.values.insert(parameter.id, value);
                    }
                    self.current = successor.target;
                    self.next_operation = 0;
                }
                // Payload-bearing boundary results are opaque to the current
                // target-neutral host carrier. Native execution owns this
                // closed-sum inspection lane; the interpreter fails closed
                // until its embedding API can supply an exact case/payload.
                Terminator::StructuralCase { .. } => {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let result = self
                        .values
                        .get(value)
                        .copied()
                        .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
                    for parameter in machine.structural_parameters.iter().filter(|parameter| {
                        parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            || (parameter.is_self && parameter.access != StructuralAccess::Owned)
                    }) {
                        self.structural_values.remove(&parameter.place);
                    }
                    let cleanups = commit_cleanup_actions(
                        &self.structural_types,
                        &self.machines,
                        &mut self.structural_values,
                        &mut self.live_affine_frontier,
                        &mut self.live_claims,
                        cleanup_actions,
                    )?;
                    if let Some((completed, remaining)) = cleanups.split_first() {
                        let completed = completed.clone();
                        let callee = self
                            .machines
                            .get(&completed.0.cleanup_machine)
                            .cloned()
                            .expect("verified cleanup target remains installed");
                        self.call_stack.push(SuspendedCall {
                            blocks: std::mem::take(&mut self.blocks),
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            payloadless_case_values: std::mem::take(
                                &mut self.payloadless_case_values,
                            ),
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
                        self.blocks = callee.blocks;
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
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.values.insert(result_value, result);
                        self.structural_values = caller.structural_values;
                        self.payloadless_case_values = caller.payloadless_case_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        continue;
                    }
                    let result = TerminalExecutionResult::Scalar(result);
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    for place in trivial_affine_discards {
                        if self.structural_values.remove(place).is_none() {
                            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                                *place,
                            ));
                        }
                        remove_affine_root(&mut self.live_affine_frontier, *place);
                    }
                    if let Some(caller) = self.call_stack.pop() {
                        let result = caller.result;
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.structural_values = caller.structural_values;
                        self.payloadless_case_values = caller.payloadless_case_values;
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
                                    let callee = self
                                        .machines
                                        .get(&completed.0.cleanup_machine)
                                        .cloned()
                                        .ok_or(
                                            TerminalInterpretError::VerifiedCallTargetMissing(
                                                completed.0.cleanup_machine,
                                            ),
                                        )?;
                                    self.call_stack.push(SuspendedCall {
                                        blocks: std::mem::take(&mut self.blocks),
                                        values: std::mem::take(&mut self.values),
                                        structural_values: std::mem::take(
                                            &mut self.structural_values,
                                        ),
                                        payloadless_case_values: std::mem::take(
                                            &mut self.payloadless_case_values,
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
                                    self.blocks = callee.blocks;
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
                                    self.blocks = caller.blocks;
                                    self.values = caller.values;
                                    self.values.insert(result_value, returned);
                                    self.structural_values = caller.structural_values;
                                    self.payloadless_case_values = caller.payloadless_case_values;
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
                    if let Some(value) = self.payloadless_case_values.get(source).copied() {
                        let internal_result = match self.call_stack.last() {
                            Some(SuspendedCall {
                                structural_values,
                                payloadless_case_values,
                                live_affine_frontier,
                                result:
                                    SuspendedCallResult::Structural {
                                        result,
                                        returned_claim_transfers,
                                    },
                                ..
                            }) if result.multiplicity == StructuralMultiplicity::Unrestricted
                                && result.qualifications.is_empty()
                                && result.claims.is_empty()
                                && returned_claim_transfers.is_empty()
                                && !structural_values.contains_key(&result.place)
                                && !payloadless_case_values.contains_key(&result.place)
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
                                        && !self.payloadless_case_values.contains_key(place))
                            })
                        {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        }
                        if let Err(error) = meter.charge_terminator(&terminator) {
                            return meter_status(error);
                        }
                        self.payloadless_case_values.remove(source);
                        remove_affine_root(&mut self.live_affine_frontier, *source);
                        for place in trivial_affine_discards {
                            self.structural_values.remove(place);
                            self.payloadless_case_values.remove(place);
                            remove_affine_root(&mut self.live_affine_frontier, *place);
                        }
                        if let Some(result) = internal_result {
                            let caller = self
                                .call_stack
                                .pop()
                                .expect("an internal payloadless return has a caller frame");
                            let SuspendedCallResult::Structural { .. } = caller.result else {
                                unreachable!("payloadless return preflight matched its caller")
                            };
                            self.blocks = caller.blocks;
                            self.values = caller.values;
                            self.structural_values = caller.structural_values;
                            self.payloadless_case_values = caller.payloadless_case_values;
                            if self
                                .payloadless_case_values
                                .insert(result.place, value)
                                .is_some()
                            {
                                return Err(TerminalInterpretError::VerifiedOperationMalformed);
                            }
                            self.live_affine_frontier = caller.live_affine_frontier;
                            self.live_claims = caller.live_claims;
                            self.dynamic_parameters = caller.dynamic_parameters;
                            self.current_machine = caller.current_machine;
                            self.current = caller.current;
                            self.next_operation = caller.next_operation;
                            continue;
                        }
                        let result = TerminalExecutionResult::PayloadlessCase(
                            TerminalPayloadlessCaseResult { value },
                        );
                        self.result = Some(result.clone());
                        return Ok(TerminalExecutionStatus::Complete(result));
                    }
                    let value = self.structural_values.get(source).cloned().ok_or(
                        TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
                    )?;
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    // Commit only after fuel and every structural/claim check succeeds.
                    self.structural_values.remove(source);
                    remove_affine_root(&mut self.live_affine_frontier, *source);
                    for claim in returned_claims {
                        self.live_claims.remove(claim);
                    }
                    for place in trivial_affine_discards {
                        self.structural_values.remove(place);
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
                        self.blocks = caller.blocks;
                        self.values = caller.values;
                        self.structural_values = caller.structural_values;
                        self.payloadless_case_values = caller.payloadless_case_values;
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
                    if let Err(error) = meter.charge_terminator(&terminator) {
                        return meter_status(error);
                    }
                    let crash = TerminalCrash {
                        edge: *edge,
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

fn commit_cleanup_actions(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    machines: &BTreeMap<MachineId, ExecutableMachine>,
    structural_values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    live_claims: &mut BTreeMap<ClaimId, LiveClaim>,
    actions: &[TerminalAffineCleanupAction],
) -> Result<Vec<(NominalAffineCleanup, TerminalStructuralValue)>, TerminalInterpretError> {
    let mut nominal = Vec::new();
    for action in actions {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                if structural_values.remove(place).is_none()
                    || !remove_affine_root(frontier, *place)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                live_claims.retain(|_, claim| claim.place != Some(*place));
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                let root = structural_values.get(&discard.place).ok_or(
                    TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
                )?;
                if discard.path.is_empty()
                    || resolve_structural_path_type(
                        structural_types,
                        root.structural_type,
                        &discard.path,
                    )? != discard.structural_type
                    || !frontier.remove(discard)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                if !frontier.iter().any(|entry| entry.place == discard.place) {
                    structural_values.remove(&discard.place);
                }
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                let value = structural_values.remove(&cleanup.place).ok_or(
                    TerminalInterpretError::VerifiedStructuralPlaceMissing(cleanup.place),
                )?;
                if value.structural_type != cleanup.structural_type
                    || !machines.contains_key(&cleanup.cleanup_machine)
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                nominal.push((cleanup.clone(), value));
                live_claims.retain(|_, claim| claim.place != Some(cleanup.place));
            }
        }
    }
    let pending_nominal = nominal
        .iter()
        .map(|(cleanup, value)| StructuralAffineDiscard {
            place: cleanup.place,
            path: Vec::new(),
            structural_type: value.structural_type,
        })
        .collect::<BTreeSet<_>>();
    if !structural_values.is_empty() || *frontier != pending_nominal || !live_claims.is_empty() {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    Ok(nominal)
}

fn bind_arguments(
    parameters: &[terminal_psi::ValueDeclaration],
    arguments: &[TerminalScalarValue],
) -> Result<BTreeMap<ValueId, TerminalScalarValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::ArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.scalar_type != argument.scalar_type() {
            return Err(TerminalInterpretError::ArgumentType {
                value: parameter.id,
                expected: parameter.scalar_type,
                actual: argument.scalar_type(),
            });
        }
        if let TerminalScalarValue::Integer { scalar_type, value } = argument
            && !scalar_type.admits(*value)
        {
            return Err(TerminalInterpretError::ArgumentIntegerOutsideType {
                value: parameter.id,
            });
        }
        values.insert(parameter.id, *argument);
    }
    Ok(values)
}

fn bind_boundary_arguments(
    parameters: &[ScalarType],
    arguments: &[TerminalScalarValue],
) -> Result<(), TerminalInterpretError> {
    if arguments.len() != parameters.len()
        || parameters
            .iter()
            .zip(arguments)
            .any(|(parameter, argument)| *parameter != argument.scalar_type())
    {
        return Err(TerminalInterpretError::VerifiedOperationMalformed);
    }
    Ok(())
}

fn bind_structural_arguments(
    parameters: &[StructuralParameterDeclaration],
    arguments: &[TerminalStructuralValue],
) -> Result<BTreeMap<PlaceId, TerminalStructuralValue>, TerminalInterpretError> {
    if arguments.len() != parameters.len() {
        return Err(TerminalInterpretError::StructuralArgumentCount {
            expected: parameters.len(),
            actual: arguments.len(),
        });
    }
    let mut bound_identities = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if argument
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(TerminalInterpretError::StructuralQualificationsNonCanonical);
        }
        if parameter.structural_type != argument.structural_type {
            return Err(TerminalInterpretError::StructuralArgumentType {
                place: parameter.place,
                expected: parameter.structural_type,
                actual: argument.structural_type,
            });
        }
        if parameter
            .qualifications
            .iter()
            .any(|domain| !argument.qualifications.contains(domain))
        {
            return Err(TerminalInterpretError::StructuralQualificationMissing(
                parameter.place,
            ));
        }
        if let Some(previous) =
            bound_identities.insert(argument.opaque_identity, parameter.multiplicity)
            && (previous != StructuralMultiplicity::Unrestricted
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted)
        {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(
                argument.opaque_identity,
            ));
        }
        if values.insert(parameter.place, argument.clone()).is_some() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(values)
}

fn bind_structural_primitive_values(
    machine: &ExecutableMachine,
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralPrimitiveValue],
) -> Result<
    (
        BTreeMap<StructuralRuntimePlace, TerminalScalarValue>,
        BTreeMap<u32, StructuralRuntimePlace>,
    ),
    TerminalInterpretError,
> {
    let primitive_parameters = machine
        .structural_parameters
        .iter()
        .enumerate()
        .filter_map(|(argument_index, parameter)| {
            matches!(
                structural_types
                    .get(&parameter.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::PrimitiveScalar(_))
            )
            .then_some((argument_index as u32, parameter))
        })
        .collect::<BTreeMap<_, _>>();
    if primitive_parameters.len() != arguments.len() {
        return Err(TerminalInterpretError::StructuralPrimitiveValueCount {
            expected: primitive_parameters.len(),
            actual: arguments.len(),
        });
    }

    let mut storage = BTreeMap::new();
    let mut entry_places = BTreeMap::new();
    for argument in arguments {
        let parameter = primitive_parameters
            .get(&argument.argument_index)
            .copied()
            .ok_or(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            })?;
        let Some(StructuralTypeShape::PrimitiveScalar(expected)) = structural_types
            .get(&parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        };
        if argument.value.scalar_type() != *expected
            || !terminal_scalar_belongs_to_type(argument.value)
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueType {
                argument_index: argument.argument_index,
                expected: *expected,
                actual: argument.value.scalar_type(),
            });
        }
        let view = structural_values.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        let place = StructuralRuntimePlace::from(view);
        if entry_places
            .insert(argument.argument_index, place.clone())
            .is_some()
            || storage.insert(place, argument.value).is_some()
        {
            return Err(TerminalInterpretError::StructuralPrimitiveValueInvalid {
                argument_index: argument.argument_index,
            });
        }
    }
    Ok((storage, entry_places))
}

fn terminal_scalar_belongs_to_type(value: TerminalScalarValue) -> bool {
    match value {
        TerminalScalarValue::Boolean(_) => true,
        TerminalScalarValue::Integer { scalar_type, value } => scalar_type.admits(value),
        TerminalScalarValue::IeeeFloat(_) => true,
    }
}

fn nearest_ieee_float_fused_multiply_add(
    format: IeeeFloatFormat,
    left: IeeeFloatValue,
    right: IeeeFloatValue,
    addend: IeeeFloatValue,
) -> IeeeFloatValue {
    match (format, left, right, addend) {
        (
            IeeeFloatFormat::Binary32,
            IeeeFloatValue::Binary32(left),
            IeeeFloatValue::Binary32(right),
            IeeeFloatValue::Binary32(addend),
        ) => IeeeFloatValue::Binary32(
            FloatSemantics::fused_multiply_add(
                FloatFormat::BINARY32,
                &FloatMeaning::from_f32(f32::from_bits(left)),
                &FloatMeaning::from_f32(f32::from_bits(right)),
                &FloatMeaning::from_f32(f32::from_bits(addend)),
            )
            .to_f32()
            .to_bits(),
        ),
        (
            IeeeFloatFormat::Binary64,
            IeeeFloatValue::Binary64(left),
            IeeeFloatValue::Binary64(right),
            IeeeFloatValue::Binary64(addend),
        ) => IeeeFloatValue::Binary64(
            FloatSemantics::fused_multiply_add(
                FloatFormat::BINARY64,
                &FloatMeaning::from_f64(f64::from_bits(left)),
                &FloatMeaning::from_f64(f64::from_bits(right)),
                &FloatMeaning::from_f64(f64::from_bits(addend)),
            )
            .to_f64()
            .to_bits(),
        ),
        _ => unreachable!("verified IEEE FMA operands have one exact format"),
    }
}

fn bind_structural_boolean_fields(
    machine: &ExecutableMachine,
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[TerminalStructuralBooleanFieldValue],
) -> Result<BTreeMap<StructuralScalarRuntimeField, TerminalScalarValue>, TerminalInterpretError> {
    let required = machine
        .blocks
        .values()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::BooleanStructuralField { source, field } => {
                let argument_index = machine
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == source)?;
                Some((argument_index as u32, field))
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut values = BTreeMap::new();
    for argument in arguments {
        let parameter = machine
            .structural_parameters
            .get(argument.argument_index as usize)
            .ok_or(
                TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                    argument_index: argument.argument_index,
                    field: argument.field,
                },
            )?;
        let root = structural_values.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        let parent_type =
            resolve_structural_path_type(structural_types, root.structural_type, &argument.path)
                .map_err(
                    |_| TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                        argument_index: argument.argument_index,
                        field: argument.field,
                    },
                )?;
        if direct_scalar_field_type(structural_types, parent_type, argument.field)
            != Some(ScalarType::Boolean)
        {
            return Err(
                TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                    argument_index: argument.argument_index,
                    field: argument.field,
                },
            );
        }
        let mut parent = StructuralRuntimePlace::from(root);
        parent.path.extend(argument.path.clone());
        if values
            .insert(
                StructuralScalarRuntimeField {
                    parent,
                    field: argument.field,
                },
                TerminalScalarValue::Boolean(argument.value),
            )
            .is_some()
        {
            return Err(
                TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                    argument_index: argument.argument_index,
                    field: argument.field,
                },
            );
        }
    }
    for (argument_index, field) in required {
        let parameter = &machine.structural_parameters[argument_index as usize];
        let root = structural_values
            .get(&parameter.place)
            .expect("verified entry parameter has a bound structural value");
        if !values.contains_key(&StructuralScalarRuntimeField {
            parent: StructuralRuntimePlace::from(root),
            field,
        }) {
            return Err(TerminalInterpretError::StructuralBooleanFieldMissing {
                source: parameter.place,
                field,
            });
        }
    }
    Ok(values)
}

fn bind_affine_frontier(
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeSet<StructuralAffineDiscard>, TerminalInterpretError> {
    let mut frontier = BTreeSet::new();
    // Match verifier frontier reconstruction: a borrowed receiver is present
    // in the signature but is never owned by this machine.
    for parameter in parameters.iter().filter(|parameter| {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && !(parameter.is_self && parameter.access != StructuralAccess::Owned)
    }) {
        let value = values.get(&parameter.place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(parameter.place),
        )?;
        if value.structural_type != parameter.structural_type
            || !frontier.insert(StructuralAffineDiscard {
                place: parameter.place,
                path: Vec::new(),
                structural_type: parameter.structural_type,
            })
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    Ok(frontier)
}

fn remove_affine_root(frontier: &mut BTreeSet<StructuralAffineDiscard>, place: PlaceId) -> bool {
    let Some(root) = frontier
        .iter()
        .find(|entry| entry.place == place && entry.path.is_empty())
        .cloned()
    else {
        return false;
    };
    frontier.remove(&root)
}

fn consume_affine_projection(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    argument: &StructuralArgument,
) -> Result<(), TerminalInterpretError> {
    let root = values.get(&argument.place).ok_or(
        TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
    )?;
    let Some(containing) = frontier
        .iter()
        .find(|entry| {
            entry.place == argument.place && argument.path.starts_with(entry.path.as_slice())
        })
        .cloned()
    else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    if containing.path.is_empty() && containing.structural_type != root.structural_type {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    }
    frontier.remove(&containing);
    split_affine_frontier_at_projection(structural_types, frontier, containing, &argument.path)
}

fn split_affine_frontier_at_projection(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    frontier: &mut BTreeSet<StructuralAffineDiscard>,
    current: StructuralAffineDiscard,
    projected_path: &[StructuralPathSegment],
) -> Result<(), TerminalInterpretError> {
    if current.path == projected_path {
        return Ok(());
    }
    let Some(next_segment) = projected_path.get(current.path.len()) else {
        return Err(TerminalInterpretError::AffineFrontierMismatch);
    };
    let declaration = structural_types
        .get(&current.structural_type)
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
    let selected = match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            let mut selected = None;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let terminal_psi::StructuralFieldType::Structural(field_type) = field.field_type
                else {
                    continue;
                };
                let segment = StructuralPathSegment::Field(field.identity.clone());
                let mut path = current.path.clone();
                path.push(segment.clone());
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: field_type,
                };
                if &segment == next_segment {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::FixedArray {
            element,
            length,
        } if matches!(next_segment, StructuralPathSegment::FixedIndex(index) if index < length) => {
            let StructuralPathSegment::FixedIndex(selected_index) = next_segment else {
                unreachable!()
            };
            let mut selected = None;
            for index in 0..*length {
                let mut path = current.path.clone();
                path.push(StructuralPathSegment::FixedIndex(index));
                let child = StructuralAffineDiscard {
                    place: current.place,
                    path,
                    structural_type: *element,
                };
                if index == *selected_index {
                    selected = Some(child);
                } else if !frontier.insert(child) {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            }
            selected
        }
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::FixedArray { .. }
        | StructuralTypeShape::Sum { .. }
        | StructuralTypeShape::Mixed { .. } => None,
    }
    .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?;
    split_affine_frontier_at_projection(structural_types, frontier, selected, projected_path)
}

fn resolve_structural_path_type(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<StructuralTypeId, TerminalInterpretError> {
    let mut structural_type = root;
    for segment in path {
        let declaration = structural_types
            .get(&structural_type)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())
                    .ok_or(TerminalInterpretError::AffineProjectionNotRepresentable)?;
                let terminal_psi::StructuralFieldType::Structural(next) = field.field_type else {
                    return Err(TerminalInterpretError::AffineProjectionNotRepresentable);
                };
                next
            }
            _ => return Err(TerminalInterpretError::AffineProjectionNotRepresentable),
        };
    }
    Ok(structural_type)
}

fn bind_entry_claims(
    entry_claims: &[EntryClaim],
    content_entry_claims: &[terminal_psi::ContentEntryClaim],
    parameters: &[StructuralParameterDeclaration],
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut claims = BTreeMap::new();
    for entry_claim in entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !values.contains_key(&parameter.place)
            || claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        path: entry_claim.path.clone(),
                        multiplicity: Some(if entry_claim.path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
    }
    for entry_claim in content_entry_claims {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == entry_claim.input.root);
        claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: parameter.map(|_| entry_claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    Ok(claims)
}

fn resolve_structural_arguments(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    arguments: &[StructuralArgument],
) -> Result<Vec<TerminalStructuralValue>, TerminalInterpretError> {
    arguments
        .iter()
        .map(|argument| {
            let mut value = values.get(&argument.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )?;
            let mut structural_type = value.structural_type;
            for segment in &argument.path {
                let declaration = structural_types
                    .get(&structural_type)
                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                structural_type = match (segment, &declaration.shape) {
                    (
                        StructuralPathSegment::FixedIndex(index),
                        StructuralTypeShape::FixedArray { element, length },
                    ) if index < length => *element,
                    (
                        StructuralPathSegment::Field(identity),
                        StructuralTypeShape::Record { fields },
                    ) => {
                        let field = fields
                            .iter()
                            .find(|field| {
                                field.identity == *identity && !field.relevance.is_erased()
                            })
                            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                        let terminal_psi::StructuralFieldType::Structural(next) = field.field_type
                        else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        next
                    }
                    _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
                };
            }
            value.structural_type = structural_type;
            if !argument.path.is_empty() {
                value.qualifications.clear();
            }
            value.path.extend(argument.path.clone());
            Ok(value)
        })
        .collect()
}

fn direct_scalar_field_type(
    structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = structural_types.get(&structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                terminal_psi::StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::Structural(_)
                | terminal_psi::StructuralFieldType::Erased { .. } => None,
            })
    })
}

#[allow(clippy::too_many_arguments)]
fn transfer_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    _caller_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
    caller_arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    callee_parameters: &[StructuralParameterDeclaration],
    callee_entry_claims: &[EntryClaim],
    callee_content_entry_claims: &[terminal_psi::ContentEntryClaim],
    callee_values: &BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<(BTreeMap<ClaimId, LiveClaim>, BTreeMap<ClaimId, LiveClaim>), TerminalInterpretError> {
    if transfers.len() != callee_entry_claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    if caller_arguments
        .iter()
        .zip(callee_parameters)
        .any(|(argument, parameter)| {
            !argument.path.is_empty()
                && (callee_entry_claims
                    .iter()
                    .any(|claim| claim.input == parameter.place && !claim.path.is_empty())
                    || callee_content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
        })
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut expected_by_argument = BTreeMap::<u32, Vec<&EntryClaim>>::new();
    for entry_claim in callee_entry_claims {
        let (index, parameter) = callee_parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.place == entry_claim.input)
            .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || !callee_values.contains_key(&parameter.place)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        expected_by_argument
            .entry(index as u32)
            .or_default()
            .push(entry_claim);
    }
    let mut actual_by_argument = BTreeMap::<u32, Vec<&ClaimTransfer>>::new();
    for transfer in transfers {
        if transfer.argument_index as usize >= caller_arguments.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        actual_by_argument
            .entry(transfer.argument_index)
            .or_default()
            .push(transfer);
    }
    if expected_by_argument.keys().collect::<Vec<_>>()
        != actual_by_argument.keys().collect::<Vec<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let mut remaining = caller_claims.clone();
    let mut callee_claims = BTreeMap::new();
    for (argument_index, expected) in expected_by_argument {
        let actual = &actual_by_argument[&argument_index];
        if actual.len() != expected.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let caller_place = caller_arguments[argument_index as usize].place;
        let argument_path = &caller_arguments[argument_index as usize].path;
        for (transfer, entry_claim) in actual.iter().zip(expected) {
            let caller_claim = remaining
                .remove(&transfer.claim)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            let expected_caller_path = argument_path
                .iter()
                .cloned()
                .chain(entry_claim.path.iter().cloned())
                .collect::<Vec<_>>();
            if caller_claim.place != Some(caller_place) || caller_claim.path != expected_caller_path
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let parameter = callee_parameters
                .get(argument_index as usize)
                .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
            if callee_claims
                .insert(
                    entry_claim.claim,
                    LiveClaim {
                        place: Some(parameter.place),
                        path: entry_claim.path.clone(),
                        multiplicity: Some(if entry_claim.path.is_empty() {
                            parameter.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
    }
    for entry_claim in callee_content_entry_claims {
        callee_claims.entry(entry_claim.claim).or_insert(LiveClaim {
            place: None,
            path: Vec::new(),
            multiplicity: None,
        });
    }
    Ok((remaining, callee_claims))
}

fn rebind_structural_result_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    callee_claims: &BTreeMap<ClaimId, LiveClaim>,
    source: PlaceId,
    result: &StructuralOperationResult,
    transfers: &[StructuralResultClaimTransfer],
    returned_claims: &[ClaimId],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let mut bindings = BTreeMap::new();
    for binding in &result.claims {
        if bindings
            .insert(binding.claim, binding.path.as_slice())
            .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if bindings.len() != transfers.len() || returned_claims.len() != transfers.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }

    let expected_callee = returned_claims.iter().copied().collect::<BTreeSet<_>>();
    if expected_callee.len() != returned_claims.len() {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    let mut mapped_callee = BTreeSet::new();
    let mut mapped_caller = BTreeSet::new();
    let mut rebound = caller_claims.clone();
    for transfer in transfers {
        if !mapped_callee.insert(transfer.callee_claim)
            || !mapped_caller.insert(transfer.caller_claim)
            || !expected_callee.contains(&transfer.callee_claim)
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let Some(returned) = callee_claims.get(&transfer.callee_claim) else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        let Some(path) = bindings.get(&transfer.caller_claim) else {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        };
        let expected_multiplicity = if path.is_empty() {
            result.multiplicity
        } else {
            StructuralMultiplicity::Linear
        };
        if returned.place != Some(source)
            || returned.path != *path
            || returned.multiplicity != Some(expected_multiplicity)
            || rebound
                .insert(
                    transfer.caller_claim,
                    LiveClaim {
                        place: Some(result.place),
                        path: path.to_vec(),
                        multiplicity: Some(expected_multiplicity),
                    },
                )
                .is_some()
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
    }
    if mapped_callee != expected_callee
        || mapped_caller != bindings.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(TerminalInterpretError::ClaimTransferMismatch);
    }
    Ok(rebound)
}

fn validate_boundary_requirements(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[TerminalStructuralValue],
) -> Result<(), TerminalInterpretError> {
    for requirement in &boundary.requires {
        let argument = arguments
            .get(requirement.argument_index as usize)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if !argument.qualifications.contains(&requirement.domain) {
            return Err(TerminalInterpretError::BoundaryQualificationMissing {
                boundary: boundary.id,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn complete_claims(
    caller_claims: &BTreeMap<ClaimId, LiveClaim>,
    caller_arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    _boundary_parameters: &[StructuralParameterDeclaration],
) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
    let expected = caller_arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller_claims.iter().filter_map(move |(claim, live)| {
                (live.place == Some(argument.place)
                    && (argument.path.is_empty() || live.path == argument.path))
                    .then_some((index as u32, *claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let mut remaining = caller_claims.clone();
    let mut actual = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim))
            || !expected.contains(&(receipt.argument_index, receipt.claim))
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
        let argument = caller_arguments
            .get(receipt.argument_index as usize)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        let claim = remaining
            .remove(&receipt.claim)
            .ok_or(TerminalInterpretError::CompletionReceiptMismatch)?;
        if claim.place != Some(argument.place)
            || (!argument.path.is_empty() && claim.path != argument.path)
        {
            return Err(TerminalInterpretError::CompletionReceiptMismatch);
        }
    }
    if actual != expected {
        return Err(TerminalInterpretError::CompletionReceiptMismatch);
    }
    Ok(remaining)
}

fn has_live_linear_claims(claims: &BTreeMap<ClaimId, LiveClaim>) -> bool {
    claims
        .values()
        .any(|claim| claim.multiplicity == Some(StructuralMultiplicity::Linear))
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
/// `frontier_lower_bound` is the machine-local claim frontier recorded by the
/// artifact. It is not an assertion that no wider runtime state was abandoned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCrash {
    pub edge: semantic_vocabulary::EdgeId,
    pub cause: CrashCause,
    pub site_guard: Vec<terminal_psi::CrashPredicateTerm>,
    pub frontier_lower_bound: Vec<ClaimId>,
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
    StructuralScalarFieldMissing {
        source: PlaceId,
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
