#![forbid(unsafe_code)]

//! Lower verified terminal Psi into source-independent Omega realization
//! requirements.

use std::collections::BTreeMap;

use omega_terminal_abstract_operations::{
    TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
    TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractParameter,
    TerminalAbstractResult, TerminalAbstractSuccessor, TerminalCompletionClaimSource,
    TerminalValueBinding,
};
use psi_core::{BlockId, MachineId, ObligationId, OperationId, ScalarType, StructuralPlaceKind};
use psi_terminal::{
    CompletionReceipt, OperationKind, OperationResult, ProviderCandidateConformance,
    StructuralArgument, StructuralMultiplicity, StructuralResultDeclaration,
    TerminalAffineCleanupAction, TerminalMachine, Terminator,
};
use psi_terminal_codec::{CodecError, terminal_psi_identity};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Required optimizer input produced only after canonical artifact decoding,
/// Terminal-Psi validation, proof reconstruction, and evidence admission.
///
/// The ordinary native path may consume the bare abstract plan for backwards
/// compatibility. Optimizer entry points must instead require this carrier so
/// proof, ownership, and path-sensitive semantic context cannot become an
/// optional side channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTerminalOptimizationInput {
    plan: TerminalAbstractOperationPlan,
    context: VerifiedTerminalOptimizationContext,
}

impl VerifiedTerminalOptimizationInput {
    pub const fn plan(&self) -> &TerminalAbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedTerminalOptimizationContext {
        &self.context
    }
}

/// Verifier-owned semantic and proof context retained beside the reconstructible
/// Omega plan. The complete immutable Terminal module is intentional: narrow
/// projections may be derived from it, but cannot recreate discarded place
/// paths, call obligations, edge cleanup, or borrow frontiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTerminalOptimizationContext {
    terminal_module: psi_terminal::TerminalModule,
    proof_bundle: psi_terminal_verifier::ProofBundle,
    proof_bundle_fingerprint: psi_terminal_codec::ProofBundleFingerprint,
    reconstructed_obligations: psi_terminal_verifier::ReconstructedTerminalObligationSet,
    accepted_facts: Vec<psi_proof_admission::AcceptedFact>,
    structural_frontiers: psi_terminal_verifier::VerifiedTerminalStructuralFrontiers,
}

impl VerifiedTerminalOptimizationContext {
    pub const fn terminal_module(&self) -> &psi_terminal::TerminalModule {
        &self.terminal_module
    }

    pub const fn proof_bundle(&self) -> &psi_terminal_verifier::ProofBundle {
        &self.proof_bundle
    }

    pub const fn proof_bundle_fingerprint(&self) -> psi_terminal_codec::ProofBundleFingerprint {
        self.proof_bundle_fingerprint
    }

    pub const fn reconstructed_obligations(
        &self,
    ) -> &psi_terminal_verifier::ReconstructedTerminalObligationSet {
        &self.reconstructed_obligations
    }

    pub fn accepted_facts(&self) -> &[psi_proof_admission::AcceptedFact] {
        &self.accepted_facts
    }

    pub const fn structural_frontiers(
        &self,
    ) -> &psi_terminal_verifier::VerifiedTerminalStructuralFrontiers {
        &self.structural_frontiers
    }
}

/// A reconstructible optimizer unit that cannot detach from the exact
/// verifier context which authorized its proof- and borrow-sensitive facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationUnit {
    input: VerifiedTerminalOptimizationInput,
    unit: omega_optimization_unit::PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationUnit {
    pub const fn input(&self) -> &VerifiedTerminalOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &omega_optimization_unit::PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(
        self,
    ) -> (
        VerifiedTerminalOptimizationInput,
        omega_optimization_unit::PsiOptimizationUnit,
    ) {
        (self.input, self.unit)
    }
}

/// The only optimizer-facing unit constructor. Consuming the verified carrier
/// prevents callers from pairing a plan with evidence admitted for a different
/// Terminal-Psi artifact.
pub fn build_verified_psi_optimization_unit(
    input: VerifiedTerminalOptimizationInput,
    fuel_schedule: psi_core::FuelScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        fuel_schedule,
    )?;
    let context = input.context();
    seed.structural_domains = context.terminal_module().structural_domains.clone().into();
    for function in &mut seed.functions {
        let source = context
            .terminal_module()
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .ok_or(
                VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine(
                    function.machine,
                ),
            )?;
        function.structural_places = source.structural_places.clone();
        function.content_entry_claims = source.content_entry_claims.clone();
    }
    seed.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&seed);
    let proof_fingerprint = *context.proof_bundle_fingerprint().as_bytes();
    let mut facts = Vec::new();
    for function in &seed.functions {
        for reference in &function.facts {
            let omega_optimization_unit::OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = reference
            else {
                continue;
            };
            let reconstructed = context
                .reconstructed_obligations()
                .obligations()
                .iter()
                .find(|row| {
                    row.obligation.id == *obligation
                        && row.owner
                            == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                                machine: function.machine,
                                operation: *support,
                            }
                })
                .ok_or(VerifiedPsiOptimizationUnitBuildError::MissingReconstructedObligation {
                    machine: function.machine,
                    operation: *support,
                    obligation: *obligation,
                })?;
            let accepted = context
                .accepted_facts()
                .iter()
                .find(|fact| fact.obligation == *obligation)
                .filter(|fact| fact.proposition == reconstructed.obligation.proposition)
                .ok_or(
                    VerifiedPsiOptimizationUnitBuildError::MissingAcceptedObligation {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                )?;
            let proposition =
                psi_terminal_codec::canonical_proposition_order_key(&accepted.proposition)?;
            facts.push(omega_optimization_unit::AcceptedObligationFact::new(
                seed.terminal_psi,
                proof_fingerprint,
                function.machine,
                *support,
                *obligation,
                proposition,
            ));
        }
    }
    let unit = omega_optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    let ownership_frontiers = project_ownership_frontiers(&input)?;
    let unit = omega_optimization_unit::attach_ownership_frontier_facts(unit, ownership_frontiers)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

fn project_ownership_frontiers(
    input: &VerifiedTerminalOptimizationInput,
) -> Result<
    Vec<omega_optimization_unit::OwnershipFrontierFact>,
    VerifiedPsiOptimizationUnitBuildError,
> {
    use omega_optimization_unit::OwnershipFrontierSite as Site;

    let mut facts = Vec::new();
    let context = input.context();
    for machine in &context.terminal_module().machines {
        let frontiers = context.structural_frontiers().machine(machine.id).ok_or(
            VerifiedPsiOptimizationUnitBuildError::MissingStructuralFrontierMachine(machine.id),
        )?;
        for block in &machine.blocks {
            push_ownership_frontier(
                &mut facts,
                input.plan().terminal_psi,
                machine.id,
                Site::BlockEntry(block.id),
                frontiers.block_entry(block.id),
            )?;
            for operation in &block.operations {
                push_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    Site::OperationEntry(operation.id),
                    frontiers.operation_entry(operation.id),
                )?;
                push_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    Site::OperationExit(operation.id),
                    frontiers.operation_exit(operation.id),
                )?;
            }
            for edge in block.terminator.edges() {
                push_ownership_frontier(
                    &mut facts,
                    input.plan().terminal_psi,
                    machine.id,
                    Site::EdgeEntry(edge),
                    frontiers.edge_entry(edge),
                )?;
                if let Some(snapshot) = frontiers.edge_exit(edge) {
                    facts.push(omega_optimization_unit::OwnershipFrontierFact::new(
                        input.plan().terminal_psi,
                        machine.id,
                        Site::EdgeExit(edge),
                        ownership_frontier_snapshot(snapshot),
                    ));
                }
            }
        }
    }
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Ok(facts)
}

fn push_ownership_frontier(
    facts: &mut Vec<omega_optimization_unit::OwnershipFrontierFact>,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    site: omega_optimization_unit::OwnershipFrontierSite,
    snapshot: Option<&psi_terminal_verifier::VerifiedStructuralOwnershipFrontier>,
) -> Result<(), VerifiedPsiOptimizationUnitBuildError> {
    let snapshot = snapshot.ok_or(
        VerifiedPsiOptimizationUnitBuildError::MissingStructuralFrontier { machine, site },
    )?;
    facts.push(omega_optimization_unit::OwnershipFrontierFact::new(
        terminal_psi,
        machine,
        site,
        ownership_frontier_snapshot(snapshot),
    ));
    Ok(())
}

fn ownership_frontier_snapshot(
    snapshot: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) -> omega_optimization_unit::OwnershipFrontierSnapshot {
    omega_optimization_unit::OwnershipFrontierSnapshot {
        claims: snapshot
            .claims()
            .iter()
            .map(
                |claim| omega_optimization_unit::OwnershipFrontierLiveClaim {
                    claim: claim.claim,
                    input: claim.input,
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                },
            )
            .collect(),
        owned_places: snapshot
            .owned_places()
            .iter()
            .map(
                |place| omega_optimization_unit::OwnershipFrontierOwnedPlace {
                    place: place.place,
                    multiplicity: place.multiplicity,
                },
            )
            .collect(),
        partial_custody: snapshot
            .partial_custody()
            .iter()
            .map(
                |partial| omega_optimization_unit::OwnershipFrontierPartialCustody {
                    place: partial.place,
                    moved_paths: partial.moved_paths.clone(),
                },
            )
            .collect(),
    }
}

#[derive(Debug)]
pub enum VerifiedPsiOptimizationUnitBuildError {
    Unit(omega_optimization_unit::OptimizationUnitBuildError),
    MissingReconstructedObligation {
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
    },
    MissingAcceptedObligation {
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
    },
    PropositionCodec(CodecError),
    FactIndex(omega_optimization_unit::AcceptedObligationFactIndexError),
    OwnershipFrontierFactIndex(omega_optimization_unit::OwnershipFrontierFactIndexError),
    MissingStructuralCatalogMachine(MachineId),
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralFrontier {
        machine: MachineId,
        site: omega_optimization_unit::OwnershipFrontierSite,
    },
}

impl std::fmt::Display for VerifiedPsiOptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct verified Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for VerifiedPsiOptimizationUnitBuildError {}

impl From<omega_optimization_unit::OptimizationUnitBuildError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::OptimizationUnitBuildError) -> Self {
        Self::Unit(error)
    }
}

impl From<CodecError> for VerifiedPsiOptimizationUnitBuildError {
    fn from(error: CodecError) -> Self {
        Self::PropositionCodec(error)
    }
}

impl From<omega_optimization_unit::AcceptedObligationFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::AcceptedObligationFactIndexError) -> Self {
        Self::FactIndex(error)
    }
}

impl From<omega_optimization_unit::OwnershipFrontierFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::OwnershipFrontierFactIndexError) -> Self {
        Self::OwnershipFrontierFactIndex(error)
    }
}

/// Canonical-decode and verify terminal-Psi semantic/proof artifact sections
/// before constructing Omega's source-independent realization requirements.
/// Producer-owned modules and frontend trees cannot cross this boundary.
pub fn lower_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<TerminalAbstractOperationPlan, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)
}

/// Construct the required optimizer carrier without affecting the ordinary
/// empty-selection path. This API intentionally repeats canonical artifact
/// admission only when an optimizer consumer explicitly asks for it.
pub fn lower_artifact_sections_for_optimization(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<VerifiedTerminalOptimizationInput, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    retain_verified_optimization_input(&verified)
}

/// Decode a persisted obligation ledger, reconstruct it from the exact semantic
/// section under the current verifier trust graph, and require exact equality
/// before proof checking or lowering. The producer-authored ledger is never a
/// verdict and cannot choose the proof question.
pub fn lower_replay_artifact_sections(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<TerminalAbstractOperationPlan, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let obligation_ledger =
        psi_terminal_codec::decode_terminal_obligation_ledger(obligation_ledger_bytes)
            .map_err(ArtifactLoweringError::ObligationLedgerDecode)?;
    let trust_graph = psi_terminal_codec::current_terminal_trust_graph()
        .map_err(ArtifactLoweringError::TrustGraph)?;
    psi_terminal_codec::validate_terminal_obligation_ledger(
        &obligation_ledger,
        &module,
        &trust_graph,
    )
    .map_err(ArtifactLoweringError::ObligationReplay)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)
}

/// Replay the persisted obligation ledger and retain the complete admitted
/// verifier context required by optimization.
pub fn lower_replay_artifact_sections_for_optimization(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<VerifiedTerminalOptimizationInput, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let obligation_ledger =
        psi_terminal_codec::decode_terminal_obligation_ledger(obligation_ledger_bytes)
            .map_err(ArtifactLoweringError::ObligationLedgerDecode)?;
    let trust_graph = psi_terminal_codec::current_terminal_trust_graph()
        .map_err(ArtifactLoweringError::TrustGraph)?;
    psi_terminal_codec::validate_terminal_obligation_ledger(
        &obligation_ledger,
        &module,
        &trust_graph,
    )
    .map_err(ArtifactLoweringError::ObligationReplay)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    retain_verified_optimization_input(&verified)
}

fn retain_verified_optimization_input(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<VerifiedTerminalOptimizationInput, ArtifactLoweringError> {
    let plan = lower_decoded_verified_module(verified).map_err(ArtifactLoweringError::Lowering)?;
    let proof_bundle_fingerprint =
        psi_terminal_codec::proof_bundle_fingerprint(verified.proof_bundle())
            .map_err(ArtifactLoweringError::ProofFingerprint)?;
    Ok(VerifiedTerminalOptimizationInput {
        plan,
        context: VerifiedTerminalOptimizationContext {
            terminal_module: verified.module().clone(),
            proof_bundle: verified.proof_bundle().clone(),
            proof_bundle_fingerprint,
            reconstructed_obligations: verified.reconstructed_obligations().clone(),
            accepted_facts: verified.accepted_facts().to_vec(),
            structural_frontiers: verified.structural_frontiers().clone(),
        },
    })
}

/// Bind Omega's provider policy only to exact rows preserved from the verified
/// terminal catalog. Psi independently replays artifact verification before it
/// returns the private-field installation carrier consumed by its interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapter {
    pub requirement_identity: String,
    pub provider_identity: String,
    pub machine_identity: String,
}

/// One exact structural Unit boundary occurrence bound to the checked provider
/// row selected for its requirement. Private fields prevent target lowering
/// from reconstructing this authority from a candidate machine ID alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledProviderUnitCall {
    caller: MachineId,
    psi_operation: OperationId,
    boundary: psi_core::BoundaryMachineId,
    provider: ProviderCandidateConformance,
    structural_arguments: Vec<StructuralArgument>,
    completion_claim_sources: Vec<TerminalCompletionClaimSource>,
    completion_receipts: Vec<CompletionReceipt>,
}

impl AdmittedInstalledProviderUnitCall {
    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn boundary(&self) -> psi_core::BoundaryMachineId {
        self.boundary
    }

    pub const fn provider(&self) -> &ProviderCandidateConformance {
        &self.provider
    }

    pub fn structural_arguments(&self) -> &[StructuralArgument] {
        &self.structural_arguments
    }

    pub fn completion_claim_sources(&self) -> &[TerminalCompletionClaimSource] {
        &self.completion_claim_sources
    }

    pub fn completion_receipts(&self) -> &[CompletionReceipt] {
        &self.completion_receipts
    }
}

/// Omega-owned installation custody. The Psi carrier remains sealed and is
/// exposed only by reference for reference execution; physical consumers use
/// the fully replayed provider rows and call occurrences retained alongside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedTerminalProviderInstallation {
    psi_installation: psi_terminal_interpreter::AdmittedProviderInstallation,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    installed_candidates: Vec<ProviderCandidateConformance>,
    installed_unit_calls: Vec<AdmittedInstalledProviderUnitCall>,
}

impl AdmittedTerminalProviderInstallation {
    pub const fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn psi_installation(
        &self,
    ) -> &psi_terminal_interpreter::AdmittedProviderInstallation {
        &self.psi_installation
    }

    pub fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_unit_calls(&self) -> &[AdmittedInstalledProviderUnitCall] {
        &self.installed_unit_calls
    }
}

impl omega_terminal_installation_evidence::TerminalProviderInstallationEvidence
    for AdmittedTerminalProviderInstallation
{
    fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.terminal_psi
    }

    fn installed_provider_unit_calls(
        &self,
    ) -> Vec<omega_terminal_installation_evidence::TerminalInstalledProviderUnitCallEvidence> {
        self.installed_unit_calls
            .iter()
            .map(|call| {
                omega_terminal_installation_evidence::TerminalInstalledProviderUnitCallEvidence {
                    caller: call.caller,
                    psi_operation: call.psi_operation,
                    boundary: call.boundary,
                    provider: call.provider.clone(),
                    structural_arguments: call.structural_arguments.clone(),
                    completion_claim_sources: call
                        .completion_claim_sources
                        .iter()
                        .map(|source| {
                            omega_terminal_installation_evidence::TerminalInstalledProviderCompletionClaimSource {
                                claim: source.claim,
                                entry: source.entry.clone(),
                                content: source.content.clone(),
                            }
                        })
                        .collect(),
                    completion_receipts: call.completion_receipts.clone(),
                }
            })
            .collect()
    }
}

pub fn admit_provider_installation(
    plan: &TerminalAbstractOperationPlan,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedTerminalProviderInstallation, ProviderInstallationError> {
    let replayed = lower_artifact_sections_for_optimization(semantic_bytes, proof_bytes, profile)
        .map_err(ProviderInstallationError::ArtifactReplay)?;
    if replayed.plan() != plan {
        return Err(ProviderInstallationError::PlanReplayMismatch);
    }
    let mut selections = Vec::new();
    let mut installed_candidates = Vec::new();
    let mut boundaries = plan
        .provider_candidates
        .iter()
        .map(|candidate| candidate.boundary)
        .collect::<Vec<_>>();
    boundaries.sort();
    boundaries.dedup();
    for boundary in boundaries {
        let candidates = plan
            .provider_candidates
            .iter()
            .filter(|candidate| candidate.boundary == boundary)
            .collect::<Vec<_>>();
        let requirement_identity = candidates[0].requirement_identity.as_str();
        if requirement_identity.is_empty()
            || candidates
                .iter()
                .any(|candidate| candidate.requirement_identity != requirement_identity)
        {
            return Err(ProviderInstallationError::InvalidLoweredCatalog);
        }
        let selected_rows = selected
            .iter()
            .filter(|row| row.requirement_identity == requirement_identity)
            .map(|row| {
                (
                    row.provider_identity.as_str(),
                    row.machine_identity.as_str(),
                )
            })
            .collect::<Vec<_>>();
        if selected_rows.is_empty() {
            return Err(ProviderInstallationError::MissingSelectedProvider { boundary });
        }
        let exact = candidates
            .iter()
            .filter(|candidate| {
                selected_rows.iter().any(|(provider, machine)| {
                    candidate.provider_identity == *provider
                        && candidate.candidate_identity == *machine
                })
            })
            .collect::<Vec<_>>();
        let [candidate] = exact.as_slice() else {
            return Err(if exact.is_empty() {
                ProviderInstallationError::SelectedProviderMismatch { boundary }
            } else {
                ProviderInstallationError::AmbiguousSelectedProvider { boundary }
            });
        };
        selections.push(psi_terminal_interpreter::ProviderInstallationSelection {
            boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        });
        installed_candidates.push((**candidate).clone());
    }
    let installed_unit_calls = replay_installed_provider_unit_calls(
        plan,
        replayed.context().terminal_module(),
        &installed_candidates,
    )?;
    let installation = psi_terminal_interpreter::admit_provider_installation_from_artifact(
        semantic_bytes,
        proof_bytes,
        profile,
        &selections,
    )
    .map_err(ProviderInstallationError::PsiAdmission)?;
    if installation.terminal_psi() != plan.terminal_psi {
        return Err(ProviderInstallationError::TerminalIdentityMismatch);
    }
    Ok(AdmittedTerminalProviderInstallation {
        terminal_psi: installation.terminal_psi(),
        psi_installation: installation,
        installed_candidates,
        installed_unit_calls,
    })
}

fn replay_installed_provider_unit_calls(
    plan: &TerminalAbstractOperationPlan,
    module: &psi_terminal::TerminalModule,
    installed: &[ProviderCandidateConformance],
) -> Result<Vec<AdmittedInstalledProviderUnitCall>, ProviderInstallationError> {
    let mut calls = Vec::new();
    for caller in &plan.functions {
        for operation in &caller.operations {
            let TerminalAbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } = operation
            else {
                continue;
            };
            let Some(provider) = installed.iter().find(|row| row.boundary == *boundary) else {
                continue;
            };
            let malformed = || ProviderInstallationError::InstalledUnitCallReplayMismatch {
                caller: caller.machine,
                operation: *psi_operation,
                boundary: *boundary,
            };
            let boundary_declaration = plan
                .boundary_machines
                .iter()
                .find(|row| row.id == *boundary)
                .ok_or_else(malformed)?;
            let candidate = plan
                .functions
                .iter()
                .find(|function| function.machine == provider.candidate)
                .ok_or_else(malformed)?;
            let terminal_candidate = module
                .machines
                .iter()
                .find(|machine| machine.id == provider.candidate)
                .ok_or_else(malformed)?;
            if result.is_some()
                || !arguments.is_empty()
                || boundary_declaration.identity != provider.requirement_identity
                || !boundary_declaration.scalar_parameters.is_empty()
                || boundary_declaration.result.is_some()
                || !candidate.parameters.is_empty()
                || !matches!(&candidate.result, TerminalAbstractFunctionResult::Unit)
                || structural_arguments.len() != provider.signature.parameters.len()
                || boundary_declaration.structural_parameters.len() != structural_arguments.len()
                || candidate.structural_parameters.len() != structural_arguments.len()
            {
                return Err(malformed());
            }
            for (index, (((argument, signature), boundary_parameter), candidate_parameter)) in
                structural_arguments
                    .iter()
                    .zip(&provider.signature.parameters)
                    .zip(&boundary_declaration.structural_parameters)
                    .zip(&candidate.structural_parameters)
                    .enumerate()
            {
                let Some(caller_parameter) = caller
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                else {
                    return Err(malformed());
                };
                if !argument.path.is_empty()
                    || signature.position as usize != index
                    || argument.access != signature.access
                    || boundary_parameter.position != signature.position
                    || boundary_parameter.is_self != signature.is_self
                    || boundary_parameter.structural_type != signature.structural_type
                    || boundary_parameter.multiplicity != signature.multiplicity
                    || boundary_parameter.access != signature.access
                    || boundary_parameter.qualifications != signature.qualifications
                    || candidate_parameter.position != signature.position
                    || candidate_parameter.is_self != signature.is_self
                    || candidate_parameter.structural_type != signature.structural_type
                    || candidate_parameter.multiplicity != signature.multiplicity
                    || candidate_parameter.access != signature.access
                    || candidate_parameter.qualifications != signature.qualifications
                    || caller_parameter.structural_type != signature.structural_type
                    || caller_parameter.multiplicity != signature.multiplicity
                    || caller_parameter.access != signature.access
                    || caller_parameter.qualifications != signature.qualifications
                {
                    return Err(malformed());
                }
            }

            let mut expected_claims = Vec::new();
            for claim in &terminal_candidate.entry_claims {
                if !claim.path.is_empty() {
                    return Err(malformed());
                }
                let argument_index = terminal_candidate
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == claim.input)
                    .ok_or_else(malformed)? as u32;
                expected_claims.push((argument_index, claim.claim));
            }
            if completion_receipts.len() != expected_claims.len() {
                return Err(malformed());
            }
            for (receipt, (argument_index, candidate_claim)) in
                completion_receipts.iter().zip(&expected_claims)
            {
                let argument = structural_arguments
                    .get(*argument_index as usize)
                    .ok_or_else(malformed)?;
                let source = completion_claim_sources
                    .iter()
                    .find(|source| source.claim == receipt.claim)
                    .ok_or_else(malformed)?;
                let entry = source.entry.as_ref().ok_or_else(malformed)?;
                if receipt.argument_index != *argument_index
                    || entry.input != argument.place
                    || !entry.path.is_empty()
                {
                    return Err(malformed());
                }
                if let Some(candidate_content) = terminal_candidate
                    .content_entry_claims
                    .iter()
                    .find(|content| content.claim == *candidate_claim)
                {
                    let caller_content = source.content.as_ref().ok_or_else(malformed)?;
                    if caller_content.input.root != argument.place
                        || caller_content.input.segments != candidate_content.input.segments
                        || caller_content.projections != candidate_content.projections
                    {
                        return Err(malformed());
                    }
                } else if source.content.is_some() {
                    return Err(malformed());
                }
            }
            if terminal_candidate
                .content_entry_claims
                .iter()
                .any(|content| {
                    !terminal_candidate
                        .entry_claims
                        .iter()
                        .any(|entry| entry.claim == content.claim)
                })
            {
                return Err(malformed());
            }
            calls.push(AdmittedInstalledProviderUnitCall {
                caller: caller.machine,
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider: provider.clone(),
                structural_arguments: structural_arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            });
        }
    }
    Ok(calls)
}

#[derive(Debug)]
pub enum ProviderInstallationError {
    ArtifactReplay(ArtifactLoweringError),
    PlanReplayMismatch,
    InvalidLoweredCatalog,
    MissingSelectedProvider {
        boundary: psi_core::BoundaryMachineId,
    },
    SelectedProviderMismatch {
        boundary: psi_core::BoundaryMachineId,
    },
    AmbiguousSelectedProvider {
        boundary: psi_core::BoundaryMachineId,
    },
    PsiAdmission(psi_terminal_interpreter::ProviderInstallationError),
    TerminalIdentityMismatch,
    InstalledUnitCallReplayMismatch {
        caller: MachineId,
        operation: OperationId,
        boundary: psi_core::BoundaryMachineId,
    },
}

/// Consume the complete verified module after the artifact entry has decoded
/// and verified it. The initial terminal vocabulary has one unconditional
/// executable chain per machine, so its Omega requirement stream is flat and
/// ordered.
fn lower_decoded_verified_module(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<TerminalAbstractOperationPlan, LoweringError> {
    let module = verified.module();
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == module.entry)
    {
        return Err(LoweringError::VerifiedEntryMachineMissing(module.entry));
    }
    let functions = module
        .machines
        .iter()
        .map(|machine| lower_machine(machine, &module.structural_types))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalAbstractOperationPlan {
        terminal_psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry: module.entry,
        structural_types: module.structural_types.clone(),
        boundary_machines: module.boundary_machines.clone(),
        provider_candidates: module.provider_candidates.clone(),
        functions,
    })
}

fn lower_machine(
    machine: &TerminalMachine,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
) -> Result<TerminalAbstractFunction, LoweringError> {
    if let Some(operation) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishPayloadlessCase { .. }
            ) || matches!(operation.kind, OperationKind::CallStructural { .. })
                && operation.result.structural().is_some_and(|result| {
                    result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
                })
        })
    {
        return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
    }
    if let Some(result) = machine.result.structural() {
        return lower_structural_machine(machine, result, structural_types);
    }
    let result = machine.result.scalar();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut operations = Vec::new();
    let mut block_entries = Vec::with_capacity(machine.blocks.len());
    let value_types = machine
        .parameters
        .iter()
        .chain(result.iter())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|value| (value.id, value.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let byte_sequence_literals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let unit_affine_locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut lowered_unit_affine_locals = Vec::new();
    let mut lowered_byte_sequence_literals = 0_usize;

    for block in &machine.blocks {
        block_entries.push(TerminalAbstractBlockEntry {
            block: block.id,
            parameters: block
                .parameters
                .iter()
                .map(|parameter| TerminalAbstractParameter {
                    value: parameter.id,
                    scalar_type: parameter.scalar_type,
                })
                .collect(),
            operation_offset: operations.len(),
        });
        for operation in &block.operations {
            match operation.kind.clone() {
                OperationKind::EstablishPayloadlessCase { .. } => {
                    return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
                }
                OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
                    let (place, ordinal, structural_type) = byte_sequence_literals
                        .iter()
                        .find(|(place, _, _)| place.id == destination)
                        .copied()
                        .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
                    let declaration = structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                        .cloned()
                        .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
                    if usize::try_from(ordinal) != Ok(lowered_byte_sequence_literals)
                        || !matches!(
                            declaration.shape,
                            psi_terminal::StructuralTypeShape::ByteSequence(
                                psi_terminal::ByteSequenceCarrier::BorrowedView
                            )
                        )
                    {
                        return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
                    }
                    lowered_byte_sequence_literals += 1;
                    operations.push(TerminalAbstractOperation::EstablishByteSequenceLiteral {
                        psi_operation: operation.id,
                        place: *place,
                        structural_type: declaration,
                        bytes,
                    });
                }
                OperationKind::EstablishTrivialAffineLocal { destination } => {
                    let (place, ordinal, structural_type) = unit_affine_locals
                        .iter()
                        .find(|(place, _, _)| place.id == destination)
                        .copied()
                        .ok_or(LoweringError::UnsupportedStructuralReturn {
                            machine: machine.id,
                            edge: block.terminator.edge(),
                        })?;
                    let declaration = structural_types
                        .iter()
                        .find(|declaration| declaration.id == structural_type)
                        .cloned()
                        .ok_or(LoweringError::UnsupportedStructuralReturn {
                            machine: machine.id,
                            edge: block.terminator.edge(),
                        })?;
                    if usize::try_from(ordinal) != Ok(lowered_unit_affine_locals.len())
                        || !matches!(declaration.shape, psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
                    {
                        return Err(LoweringError::UnsupportedStructuralReturn {
                            machine: machine.id,
                            edge: block.terminator.edge(),
                        });
                    }
                    lowered_unit_affine_locals.push((operation.id, *place, declaration.clone()));
                    operations.push(TerminalAbstractOperation::EstablishTrivialAffineLocal {
                        psi_operation: operation.id,
                        place: *place,
                        structural_type: declaration,
                    });
                }
                OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    ..
                } => {
                    operations.push(TerminalAbstractOperation::CallUnit {
                        psi_operation: operation.id,
                        callee,
                        structural_arguments,
                        claim_transfers,
                    });
                }
                OperationKind::CallStructuralScalar {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    ..
                } => {
                    let result = operation.result.expect_scalar();
                    operations.push(TerminalAbstractOperation::CallStructuralScalar {
                        psi_operation: operation.id,
                        result: TerminalAbstractResult {
                            value: result.id,
                            scalar_type: result.scalar_type,
                        },
                        callee,
                        structural_arguments,
                        claim_transfers,
                    });
                }
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    returned_claim_transfers,
                    ..
                } => {
                    let Some(result) = operation.result.structural().cloned() else {
                        return Err(LoweringError::UnsupportedStructuralResult(machine.id));
                    };
                    operations.push(TerminalAbstractOperation::CallStructural {
                        psi_operation: operation.id,
                        result,
                        callee,
                        structural_arguments,
                        claim_transfers,
                        returned_claim_transfers,
                    });
                }
                OperationKind::BoundaryCall {
                    boundary,
                    arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    let mut completion_claim_sources = machine
                        .entry_claims
                        .iter()
                        .cloned()
                        .map(|entry| TerminalCompletionClaimSource {
                            claim: entry.claim,
                            entry: Some(entry),
                            content: None,
                        })
                        .collect::<Vec<_>>();
                    for content in &machine.content_entry_claims {
                        if let Some(source) = completion_claim_sources
                            .iter_mut()
                            .find(|source| source.claim == content.claim)
                        {
                            source.content = Some(content.clone());
                        } else {
                            completion_claim_sources.push(TerminalCompletionClaimSource {
                                claim: content.claim,
                                entry: None,
                                content: Some(content.clone()),
                            });
                        }
                    }
                    completion_claim_sources.sort();
                    operations.push(TerminalAbstractOperation::BoundaryCall {
                        psi_operation: operation.id,
                        result: operation
                            .result
                            .scalar()
                            .map(|result| TerminalAbstractResult {
                                value: result.id,
                                scalar_type: result.scalar_type,
                            }),
                        boundary,
                        arguments,
                        structural_arguments,
                        completion_claim_sources,
                        completion_receipts,
                    });
                }
                OperationKind::PortWrite {
                    service,
                    port,
                    value,
                } => {
                    operations.push(TerminalAbstractOperation::PortWrite {
                        psi_operation: operation.id,
                        service,
                        port,
                        value,
                    });
                }
                OperationKind::Call {
                    callee, arguments, ..
                } => {
                    operations.push(TerminalAbstractOperation::Call {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type: operation.result.expect_scalar().scalar_type,
                        callee,
                        arguments,
                    });
                }
                OperationKind::IntegerConstant { value } => {
                    operations.push(TerminalAbstractOperation::IntegerConstant {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type: operation.result.expect_scalar().scalar_type,
                        value,
                    });
                }
                OperationKind::BooleanConstant { value } => {
                    operations.push(TerminalAbstractOperation::BooleanConstant {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        value,
                    });
                }
                OperationKind::BooleanStructuralField { source, field } => {
                    operations.push(TerminalAbstractOperation::BooleanStructuralField {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        source,
                        field,
                    });
                }
                OperationKind::BooleanNot { operand } => {
                    operations.push(TerminalAbstractOperation::BooleanNot {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        operand,
                    });
                }
                OperationKind::BooleanEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::BooleanEqual {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerEqual {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerLessThan { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerLessThan {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerLessOrEqual { left, right } => {
                    operations.push(TerminalAbstractOperation::IntegerLessOrEqual {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        left,
                        right,
                    });
                }
                OperationKind::IntegerBitwiseNot { operand } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::IntegerBitwiseNot {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        operand,
                    });
                }
                OperationKind::IntegerWiden { operand } => {
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::IntegerWiden {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        source_type,
                        target_type,
                        operand,
                    });
                }
                OperationKind::IntegerExactCast {
                    operand,
                    obligation,
                } => {
                    let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                    else {
                        return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                            operation.id,
                        ));
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::IntegerExactCast {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        source_type,
                        target_type,
                        operand,
                    });
                }
                OperationKind::IntegerBitwiseAnd { left, right }
                | OperationKind::IntegerBitwiseOr { left, right }
                | OperationKind::IntegerBitwiseXor { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::IntegerBitwiseAnd { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseAnd {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseOr { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseOr {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::IntegerBitwiseXor { .. } => {
                            TerminalAbstractOperation::IntegerBitwiseXor {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::WrappingIntegerShiftLeft { .. } => {
                            TerminalAbstractOperation::WrappingIntegerShiftLeft {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                value_type,
                                count_type,
                                value,
                                count,
                            }
                        }
                        OperationKind::WrappingIntegerShiftRight { .. } => {
                            TerminalAbstractOperation::WrappingIntegerShiftRight {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                value_type,
                                count_type,
                                value,
                                count,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::ExactIntegerShiftRight {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerShiftRight {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    });
                }
                OperationKind::ExactIntegerShiftLeft {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied()
                    else {
                        return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerShiftLeft {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    });
                }
                OperationKind::ExactIntegerAdd { left, right, .. }
                | OperationKind::WrappingIntegerAdd { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingAddMalformed(operation.id));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::ExactIntegerAdd { obligation, .. } => {
                            TerminalAbstractOperation::ExactIntegerAdd {
                                psi_operation: operation.id,
                                obligation,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::WrappingIntegerAdd { .. } => {
                            TerminalAbstractOperation::WrappingIntegerAdd {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::ExactIntegerSubtract { left, right, .. }
                | OperationKind::WrappingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingSubtractMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::ExactIntegerSubtract { obligation, .. } => {
                            TerminalAbstractOperation::ExactIntegerSubtract {
                                psi_operation: operation.id,
                                obligation,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::WrappingIntegerSubtract { .. } => {
                            TerminalAbstractOperation::WrappingIntegerSubtract {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::ExactIntegerMultiply { left, right, .. }
                | OperationKind::WrappingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingMultiplyMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(match operation.kind.clone() {
                        OperationKind::ExactIntegerMultiply { obligation, .. } => {
                            TerminalAbstractOperation::ExactIntegerMultiply {
                                psi_operation: operation.id,
                                obligation,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        OperationKind::WrappingIntegerMultiply { .. } => {
                            TerminalAbstractOperation::WrappingIntegerMultiply {
                                psi_operation: operation.id,
                                result: operation.result.expect_scalar().id,
                                scalar_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactDivideMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerDivide {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedExactRemainderMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::ExactIntegerRemainder {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::WrappingIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingDivideMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerDivide {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::WrappingIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedWrappingRemainderMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::WrappingIntegerRemainder {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedSaturatingDivideMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerDivide {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedSaturatingRemainderMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerRemainder {
                        psi_operation: operation.id,
                        obligation,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerAdd { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedSaturatingAddMalformed(operation.id));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerAdd {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedSaturatingSubtractMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerSubtract {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
                OperationKind::SaturatingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(scalar_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(LoweringError::VerifiedSaturatingMultiplyMalformed(
                            operation.id,
                        ));
                    };
                    operations.push(TerminalAbstractOperation::SaturatingIntegerMultiply {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    });
                }
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } => {
                let target_block =
                    blocks
                        .get(target)
                        .copied()
                        .ok_or(LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: *target,
                        })?;
                if target_block.parameters.len() != arguments.len() {
                    return Err(LoweringError::VerifiedJumpArityMismatch { edge: *edge });
                }
                operations.push(TerminalAbstractOperation::Jump {
                    psi_edge: *edge,
                    target: *target,
                    bindings: target_block
                        .parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| TerminalValueBinding {
                            parameter: parameter.id,
                            argument: *argument,
                            scalar_type: parameter.scalar_type,
                        })
                        .collect(),
                });
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let lower_successor = |successor: &psi_terminal::SuccessorEdge| {
                    let target_block = blocks.get(&successor.target).copied().ok_or(
                        LoweringError::VerifiedBlockMissing {
                            machine: machine.id,
                            block: successor.target,
                        },
                    )?;
                    if target_block.parameters.len() != successor.arguments.len() {
                        return Err(LoweringError::VerifiedJumpArityMismatch {
                            edge: successor.edge,
                        });
                    }
                    Ok(TerminalAbstractSuccessor {
                        psi_edge: successor.edge,
                        target: successor.target,
                        bindings: target_block
                            .parameters
                            .iter()
                            .zip(&successor.arguments)
                            .map(|(parameter, argument)| TerminalValueBinding {
                                parameter: parameter.id,
                                argument: *argument,
                                scalar_type: parameter.scalar_type,
                            })
                            .collect(),
                    })
                };
                operations.push(TerminalAbstractOperation::Conditional {
                    condition: *condition,
                    when_true: lower_successor(when_true)?,
                    when_false: lower_successor(when_false)?,
                });
            }
            Terminator::Return {
                edge,
                value,
                cleanup_actions,
            } => {
                let result =
                    result.ok_or(LoweringError::ScalarReturnFromUnitMachine(machine.id))?;
                operations.push(TerminalAbstractOperation::Return {
                    psi_edge: *edge,
                    result: result.id,
                    value: *value,
                    scalar_type: result.scalar_type,
                    cleanup_actions: cleanup_actions
                        .iter()
                        .cloned()
                        .map(|action| match action {
                            TerminalAffineCleanupAction::InvokeNominal(mut cleanup) => {
                                // Psi has already verified these proof-site identities. They
                                // carry no native realization meaning and must not become a
                                // second semantic authority in Omega artifacts.
                                cleanup.cleanup_receiver = None;
                                cleanup.requirement_obligations.clear();
                                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            }
                            action => action,
                        })
                        .collect(),
                });
            }
            Terminator::ReturnUnit {
                edge,
                trivial_affine_discards,
            } => {
                if result.is_some() {
                    return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
                }
                let expected_locals = lowered_unit_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                if !trivial_affine_discards.starts_with(&expected_locals) {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(TerminalAbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: trivial_affine_discards
                        .iter()
                        .copied()
                        .map(TerminalAffineCleanupAction::DiscardRoot)
                        .collect(),
                });
            }
            Terminator::ReturnUnitPartialAffine {
                edge,
                trivial_affine_discards,
                residual_affine_discards,
            } => {
                if result.is_some() {
                    return Err(LoweringError::UnitReturnFromScalarMachine(machine.id));
                }
                let expected_locals = lowered_unit_affine_locals
                    .iter()
                    .rev()
                    .map(|(_, place, _)| place.id)
                    .collect::<Vec<_>>();
                if !trivial_affine_discards.starts_with(&expected_locals) {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(TerminalAbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: trivial_affine_discards
                        .iter()
                        .copied()
                        .map(TerminalAffineCleanupAction::DiscardRoot)
                        .chain(
                            residual_affine_discards
                                .iter()
                                .cloned()
                                .map(TerminalAffineCleanupAction::DiscardResidual),
                        )
                        .collect(),
                });
            }
            Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
                if result.is_some() || !lowered_unit_affine_locals.is_empty() {
                    return Err(LoweringError::UnsupportedStructuralReturn {
                        machine: machine.id,
                        edge: *edge,
                    });
                }
                operations.push(TerminalAbstractOperation::ReturnUnit {
                    psi_edge: *edge,
                    cleanup_actions: cleanups
                        .iter()
                        .cloned()
                        .map(|mut cleanup| {
                            // Psi has already verified these proof-site identities. They
                            // carry no native realization meaning and must not become a
                            // second semantic authority in Omega artifacts.
                            cleanup.cleanup_receiver = None;
                            cleanup.requirement_obligations.clear();
                            TerminalAffineCleanupAction::InvokeNominal(cleanup)
                        })
                        .collect(),
                });
            }
            Terminator::ReturnStructural { edge, .. } => {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: *edge,
                });
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                operations.push(TerminalAbstractOperation::Crash {
                    psi_edge: *edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
        }
    }

    Ok(TerminalAbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        entry: machine.entry,
        parameters: machine
            .parameters
            .iter()
            .map(|parameter| TerminalAbstractParameter {
                value: parameter.id,
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        structural_parameters: machine.structural_parameters.clone(),
        result: match result {
            Some(result) => TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                value: result.id,
                scalar_type: result.scalar_type,
            }),
            None => TerminalAbstractFunctionResult::Unit,
        },
        entry_claims: machine.entry_claims.clone(),
        published_service_ceiling: machine.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}

/// Lower only the first complete structural ABI requirement: one verified
/// whole-root linear parameter is returned unchanged with its one live claim.
/// Wider verified terminal programs remain fenced until their target-neutral
/// carrier and Omega realization land together.
fn lower_structural_machine(
    machine: &TerminalMachine,
    result: &StructuralResultDeclaration,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
) -> Result<TerminalAbstractFunction, LoweringError> {
    let unsupported = || LoweringError::UnsupportedStructuralResult(machine.id);
    let Some(parameter) = machine.structural_parameters.first() else {
        return Err(unsupported());
    };
    let discarded = machine.structural_parameters.get(1..).unwrap_or_default();
    if machine.structural_parameters.is_empty() {
        return Err(unsupported());
    }
    let [entry_claim] = machine.entry_claims.as_slice() else {
        return Err(unsupported());
    };
    let [block] = machine.blocks.as_slice() else {
        return Err(unsupported());
    };
    if let [operation] = block.operations.as_slice()
        && let OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence: _,
        } = &operation.kind
        && let Some(operation_result) = operation.result.structural()
        && let Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            trivial_affine_discards,
        } = &block.terminator
    {
        let [argument] = structural_arguments.as_slice() else {
            return Err(unsupported());
        };
        let [claim_transfer] = claim_transfers.as_slice() else {
            return Err(unsupported());
        };
        let [returned_transfer] = returned_claim_transfers.as_slice() else {
            return Err(unsupported());
        };
        let [result_claim] = operation_result.claims.as_slice() else {
            return Err(unsupported());
        };
        let operation_place = machine
            .structural_places
            .iter()
            .find(|place| place.id == operation_result.place)
            .ok_or_else(unsupported)?;
        if machine.structural_parameters.len() != 1
            || !discarded.is_empty()
            || !machine.parameters.is_empty()
            || parameter.position != 0
            || parameter.is_self
            || parameter.multiplicity != StructuralMultiplicity::Linear
            || result.multiplicity != StructuralMultiplicity::Linear
            || parameter.structural_type != result.structural_type
            || parameter.qualifications != result.qualifications
            || operation_result.structural_type != result.structural_type
            || operation_result.multiplicity != result.multiplicity
            || operation_result.qualifications != result.qualifications
            || argument.place != parameter.place
            || argument.access != psi_terminal::StructuralAccess::Owned
            || !argument.path.is_empty()
            || claim_transfer.argument_index != 0
            || claim_transfer.claim != entry_claim.claim
            || returned_transfer.caller_claim != entry_claim.claim
            || result_claim.claim != entry_claim.claim
            || !result_claim.path.is_empty()
            || *source != operation_result.place
            || returned_claims.as_slice() != [entry_claim.claim]
            || !trivial_affine_discards.is_empty()
            || !requirement_obligations.is_empty()
            || !crash_continuations.is_empty()
            || block.id != machine.entry
            || !block.parameters.is_empty()
            || !machine.published_service_ceiling.is_empty()
            || !machine.contract.crash_routes.is_empty()
            || !machine.contract.requires.is_empty()
            || !machine.contract.ensures.is_empty()
            || machine.structural_places.len() != 3
            || !matches!(
                operation_place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type
            )
        {
            return Err(unsupported());
        }
        return Ok(TerminalAbstractFunction {
            machine: machine.id,
            attachment: machine.attachment,
            entry: machine.entry,
            parameters: Vec::new(),
            structural_parameters: machine.structural_parameters.clone(),
            result: TerminalAbstractFunctionResult::Structural(result.clone()),
            entry_claims: vec![entry_claim.clone()],
            published_service_ceiling: Vec::new(),
            block_entries: vec![TerminalAbstractBlockEntry {
                block: block.id,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                TerminalAbstractOperation::CallStructural {
                    psi_operation: operation.id,
                    result: operation_result.clone(),
                    callee: *callee,
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers: claim_transfers.clone(),
                    returned_claim_transfers: returned_claim_transfers.clone(),
                },
                TerminalAbstractOperation::ReturnStructural {
                    psi_edge: *edge,
                    source: *source,
                    returned_claims: returned_claims.clone(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            ],
        });
    }
    let parameter_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == parameter.place)
        .ok_or_else(unsupported)?;
    let result_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == result.place)
        .ok_or_else(unsupported)?;
    let discarded_places = discarded
        .iter()
        .map(|discarded| {
            machine
                .structural_places
                .iter()
                .find(|place| place.id == discarded.place)
                .ok_or_else(unsupported)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let trivial_affine_locals = block
        .operations
        .iter()
        .map(|operation| {
            let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind else {
                return Err(unsupported());
            };
            if operation.result != OperationResult::Unit {
                return Err(unsupported());
            }
            let declaration = machine
                .structural_places
                .iter()
                .find(|place| place.id == destination)
                .cloned()
                .ok_or_else(unsupported)?;
            let psi_core::StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = declaration.kind
            else {
                return Err(unsupported());
            };
            let local_type = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or_else(unsupported)?;
            if !matches!(
                local_type.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
            ) {
                return Err(unsupported());
            }
            Ok((operation.id, declaration, local_type))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Terminator::ReturnStructural {
        edge,
        source,
        returned_claims,
        trivial_affine_discards,
    } = &block.terminator
    else {
        return Err(unsupported());
    };

    if !machine.parameters.is_empty()
        || parameter.position != 0
        || parameter.is_self
        || discarded.iter().enumerate().any(|(index, discarded)| {
            usize::try_from(discarded.position) != Ok(index + 1)
                || discarded.is_self
                || discarded.multiplicity != StructuralMultiplicity::Affine
                || !discarded.qualifications.is_empty()
        })
        || machine
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != machine.structural_parameters.len()
        || parameter.multiplicity != StructuralMultiplicity::Linear
        || result.multiplicity != StructuralMultiplicity::Linear
        || parameter.structural_type != result.structural_type
        || parameter.qualifications != result.qualifications
        || parameter.place != *source
        || entry_claim.input != parameter.place
        || !entry_claim.path.is_empty()
        || returned_claims.as_slice() != [entry_claim.claim]
        || trivial_affine_discards
            != &trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(discarded.iter().rev().map(|discarded| discarded.place))
                .collect::<Vec<_>>()
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !machine.published_service_ceiling.is_empty()
        || !machine.contract.crash_routes.is_empty()
        || !machine.contract.requires.is_empty()
        || !machine.contract.ensures.is_empty()
        || parameter_place.id != parameter.place
        || !matches!(
            parameter_place.kind,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false
            }
        )
        || result_place.id != result.place
        || result_place.kind != StructuralPlaceKind::Result
        || discarded_places.iter().enumerate().any(|(index, place)| {
            !matches!(
                place.kind,
                StructuralPlaceKind::Parameter {
                    position,
                    is_self: false
                } if usize::try_from(position) == Ok(index + 1)
            )
        })
        || trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(index, (_, local, local_type))| {
                !matches!(
                    local.kind,
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type
                    } if usize::try_from(declaration_ordinal) == Ok(index)
                        && structural_type == local_type.id
                )
            })
        || trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != trivial_affine_locals.len()
        || machine.structural_places.len()
            != machine.structural_parameters.len() + trivial_affine_locals.len() + 1
    {
        return Err(unsupported());
    }

    Ok(TerminalAbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        entry: machine.entry,
        parameters: Vec::new(),
        structural_parameters: machine.structural_parameters.clone(),
        result: TerminalAbstractFunctionResult::Structural(result.clone()),
        entry_claims: vec![entry_claim.clone()],
        published_service_ceiling: Vec::new(),
        block_entries: vec![TerminalAbstractBlockEntry {
            block: block.id,
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![TerminalAbstractOperation::ReturnStructural {
            psi_edge: *edge,
            source: *source,
            returned_claims: returned_claims.clone(),
            trivial_affine_locals,
            trivial_affine_discards: trivial_affine_discards.clone(),
        }],
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    SemanticIdentity(CodecError),
    /// Terminal preserves the exact payloadless sum case, but Omega has no
    /// target-neutral abstract operation for realizing that structural value.
    UnsupportedPayloadlessCase(psi_core::OperationId),
    /// Psi preserves exact byte-sequence literals, but native realization is
    /// deliberately fenced until the selected boundary has a byte-view ABI.
    UnsupportedByteSequenceLiteral(psi_core::OperationId),
    ScalarReturnFromUnitMachine(MachineId),
    UnitReturnFromScalarMachine(MachineId),
    /// The verified structural-result machine is wider than the exact
    /// singleton whole-root passthrough currently carried into Omega.
    UnsupportedStructuralResult(MachineId),
    /// A structural return appeared on a non-structural-result machine.
    UnsupportedStructuralReturn {
        machine: MachineId,
        edge: psi_core::EdgeId,
    },
    VerifiedEntryMachineMissing(MachineId),
    VerifiedBlockMissing {
        machine: MachineId,
        block: BlockId,
    },
    VerifiedControlCycle {
        machine: MachineId,
        block: BlockId,
    },
    VerifiedJumpArityMismatch {
        edge: psi_core::EdgeId,
    },
    VerifiedWrappingAddMalformed(psi_core::OperationId),
    VerifiedSaturatingAddMalformed(psi_core::OperationId),
    VerifiedWrappingSubtractMalformed(psi_core::OperationId),
    VerifiedSaturatingSubtractMalformed(psi_core::OperationId),
    VerifiedWrappingMultiplyMalformed(psi_core::OperationId),
    VerifiedExactDivideMalformed(psi_core::OperationId),
    VerifiedExactRemainderMalformed(psi_core::OperationId),
    VerifiedWrappingDivideMalformed(psi_core::OperationId),
    VerifiedWrappingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingDivideMalformed(psi_core::OperationId),
    VerifiedSaturatingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingMultiplyMalformed(psi_core::OperationId),
    VerifiedIntegerBitwiseMalformed(psi_core::OperationId),
    VerifiedIntegerWidenMalformed(psi_core::OperationId),
    VerifiedIntegerExactCastMalformed(psi_core::OperationId),
    VerifiedWrappingShiftMalformed(psi_core::OperationId),
    VerifiedExactShiftMalformed(psi_core::OperationId),
}

#[derive(Debug)]
pub enum ArtifactLoweringError {
    SemanticDecode(psi_terminal_codec::CodecError),
    ObligationLedgerDecode(psi_terminal_codec::CodecError),
    TrustGraph(psi_terminal_codec::TrustGraphError),
    ObligationReplay(psi_terminal_codec::CodecError),
    ProofDecode(psi_terminal_codec::ProofCodecError),
    ProofFingerprint(psi_terminal_codec::ProofCodecError),
    Verification(psi_terminal_verifier::VerificationError),
    Lowering(LoweringError),
}

impl std::fmt::Display for ArtifactLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactLoweringError {}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
