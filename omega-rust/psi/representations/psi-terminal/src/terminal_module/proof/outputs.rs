use crate::ClosedConformanceApplicationCommitment;
use psi_core::{EvidenceTermId, MachineId, OperationId, PropositionId, ScalarType};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputCall {
    pub caller: MachineId,
    /// Dense canonical order within the caller; source coordinates erase.
    pub ordinal: u32,
    /// Canonical checked callable identity, never a diagnostic display path.
    pub target_machine_identity: String,
    /// Exact private realization selected for a static trait-requirement call.
    /// The public target above remains the requirement callable identity; this
    /// row binds it to one closed conformance application and its emitted
    /// runtime realization without exposing the satisfier's evidence term.
    pub static_requirement_dispatch: Option<StaticRequirementDispatch>,
    /// Declared execution shape, independent of the operation link so a
    /// missing or spurious link is verifier-visible. `None` is erased proof
    /// construction; `Unit` and `Scalar` each require one ordinary call.
    pub runtime_result: Option<ProofOutputRuntimeResult>,
    /// Exact canonical ordinary call which produced `runtime_result`.
    pub runtime_call: Option<ProofOutputRuntimeCall>,
    /// Explicit erased inputs supplied to the callee's named `requires` lane.
    pub evidence_arguments: Vec<ProofOutputEvidenceArgument>,
    /// Complete canonical proof-output set, ordered by callee lane.
    pub outputs: Vec<ProofOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticRequirementDispatch {
    /// Non-authoritative compatibility coordinate for the exact application
    /// owned by `ProofOutputCall::caller`.
    pub conformance_application_report_fingerprint: u64,
    /// Authority-bearing join to the complete closed application.
    pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
    /// Canonical public requirement overload exposed to the caller. This is
    /// deliberately distinct from the selected row's declaration path.
    pub public_requirement_identity: String,
    /// Exact selected row within that closed application.
    pub declaring_trait_identity: String,
    pub requirement_identity: String,
    pub realization_identity: String,
    /// Canonical source callable independently joined through the selected
    /// closed-conformance row to `realization`.
    pub realization_callable_identity: String,
    /// Artifact-local machine emitted for the selected realization.
    pub realization: MachineId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputEvidenceArgument {
    pub input_position: u32,
    /// Formal proposition declared at this target lane. The lane itself is
    /// identified by target-machine identity plus `input_position`; it is not
    /// a produced evidence term.
    pub callee_proposition: PropositionId,
    pub source: EvidenceTermId,
    pub instantiated_proposition: PropositionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofOutputRuntimeResult {
    Unit,
    Scalar(ScalarType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutputRuntimeCall {
    pub operation: OperationId,
    pub callee: MachineId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofOutput {
    pub output_position: u32,
    /// Exact public proof selector from the callee lane.
    pub output_field: String,
    /// Formal proposition declared by the target lane.
    pub callee_proposition: PropositionId,
    /// Distinct producer-backed witness declaration. A directly forwarded
    /// input has no new callee term and records its input position below.
    pub callee_output: Option<EvidenceTermId>,
    /// Exact proposition after substituting this invocation's ordinary Type
    /// arguments, including when the caller omits this witness.
    pub instantiated_proposition: PropositionId,
    /// Input lane whose exact witness this output forwards. `None` means the
    /// callee produced a distinct witness with retained producer provenance.
    pub forwarded_input_position: Option<u32>,
    /// Distinct caller-local copy, or `None` when omitted or discarded.
    pub output: Option<EvidenceTermId>,
}
