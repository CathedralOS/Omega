use std::collections::{BTreeMap, BTreeSet};

use proof_admission::{
    AcceptedFact, AdmissionProfile, EvidenceError, RecursiveComponentAcceptance,
    RecursiveComponentError, verify_obligation_with_machine_parameters, verify_recursive_component,
};
use semantic_vocabulary::{EvidenceIdentity, EvidenceTermId, ObligationId};
use terminal_psi::TerminalModule;

use crate::{
    ModuleError, ValidatedTerminalModule, VerifiedTerminalStructuralFrontiers,
    reconstruct_validated_structural_ownership_frontiers, validate_module,
    validate_module_for_interpretation, validate_module_for_native_ranked_countdown,
    validate_module_for_optimization,
};

mod call_composition;
mod evidence_provenance;
mod float_meaning_projection;
mod proof_bundle;
mod reconstruction;
mod substitution;

use evidence_provenance::validate_evidence_producer_provenance;
pub use float_meaning_projection::*;
pub use proof_bundle::*;
use reconstruction::reconstruct_validated_terminal_obligations;
pub use reconstruction::{
    ReconstructedOperationObligation, ReconstructedTerminalObligation,
    ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet,
    reconstruct_interpretable_operation_obligations,
    reconstruct_interpretable_terminal_obligations, reconstruct_operation_obligations,
    reconstruct_terminal_obligations,
};
pub(crate) use substitution::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

#[derive(Debug)]
pub struct VerifiedTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

#[derive(Debug)]
pub struct VerifiedInterpretableTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

/// Proof-checked authority for target-neutral optimizer analysis of ordinary
/// acyclic Terminal Psi plus the exact existing unsigned-countdown carrier.
///
/// This opaque result grants no execution, interpretation, fixed-fuel,
/// native-lowering, or publication authority.
#[derive(Debug)]
pub struct VerifiedOptimizableTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

/// Proof-checked authority for deriving a whole-entry fixed-fuel theorem for
/// the exact ranked-countdown slice.
///
/// This carrier is deliberately distinct from both ordinary execution
/// authority and interpreter authority. In particular, it cannot authorize
/// native lowering or be supplied to the reference interpreter.
#[derive(Debug)]
pub struct VerifiedFixedFuelTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

/// Proof-checked authority for native lowering of the exact structural Unit
/// `u32` ranked-countdown slice.
///
/// This carrier cannot be constructed from ordinary, interpreter, or
/// fixed-fuel authority. Its private state keeps native admission distinct
/// even while all current ranked consumers share proof reconstruction.
#[derive(Debug)]
pub struct VerifiedNativeRankedTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

#[derive(Debug)]
struct VerifiedTerminalModuleState<'module> {
    validated: ValidatedTerminalModule<'module>,
    proof_bundle: ProofBundle,
    reconstructed_obligations: ReconstructedTerminalObligationSet,
    accepted_facts: Vec<AcceptedFact>,
    accepted_recursive_components: Vec<RecursiveComponentAcceptance>,
    structural_frontiers: VerifiedTerminalStructuralFrontiers,
}

impl<'module> VerifiedTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.state.accepted_facts
    }

    pub fn accepted_recursive_components(&self) -> &[RecursiveComponentAcceptance] {
        &self.state.accepted_recursive_components
    }

    /// Exact artifact evidence accepted for this module. Retaining the bundle
    /// lets artifact consumers re-encode the verified semantic/proof pair
    /// without consulting producer state.
    pub const fn proof_bundle(&self) -> &ProofBundle {
        &self.state.proof_bundle
    }

    /// The complete verifier-reconstructed proof question consumed for this
    /// result. This is retained separately from producer-selected proof routes.
    pub const fn reconstructed_obligations(&self) -> &ReconstructedTerminalObligationSet {
        &self.state.reconstructed_obligations
    }

    /// Exact block-, operation-, and edge-scoped custody snapshots produced by
    /// the same verifier walk that admitted this module.
    pub const fn structural_frontiers(&self) -> &VerifiedTerminalStructuralFrontiers {
        &self.state.structural_frontiers
    }
}

impl<'module> VerifiedInterpretableTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }
}

impl<'module> VerifiedOptimizableTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.state.accepted_facts
    }

    pub const fn proof_bundle(&self) -> &ProofBundle {
        &self.state.proof_bundle
    }

    pub const fn reconstructed_obligations(&self) -> &ReconstructedTerminalObligationSet {
        &self.state.reconstructed_obligations
    }

    pub const fn structural_frontiers(&self) -> &VerifiedTerminalStructuralFrontiers {
        &self.state.structural_frontiers
    }
}

impl<'module> VerifiedFixedFuelTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }
}

impl<'module> VerifiedNativeRankedTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.state.accepted_facts
    }

    pub fn accepted_recursive_components(&self) -> &[RecursiveComponentAcceptance] {
        &self.state.accepted_recursive_components
    }

    pub const fn proof_bundle(&self) -> &ProofBundle {
        &self.state.proof_bundle
    }

    pub const fn reconstructed_obligations(&self) -> &ReconstructedTerminalObligationSet {
        &self.state.reconstructed_obligations
    }

    pub const fn structural_frontiers(&self) -> &VerifiedTerminalStructuralFrontiers {
        &self.state.structural_frontiers
    }
}

pub fn verify_module<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedTerminalModule<'module>, VerificationError> {
    let validated = validate_module(module).map_err(VerificationError::Module)?;
    verify_validated_module(validated, proof_bundle, profile)
        .map(|state| VerifiedTerminalModule { state })
}

/// Verify the exact subset accepted by the reference interpreter.
///
/// The distinct result carrier cannot be passed to fixed-fuel or native
/// consumers that require their own proof-checked authority.
pub fn verify_module_for_interpretation<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedInterpretableTerminalModule<'module>, VerificationError> {
    let validated =
        validate_module_for_interpretation(module).map_err(VerificationError::Module)?;
    verify_validated_module(validated.validated(), proof_bundle, profile)
        .map(|state| VerifiedInterpretableTerminalModule { state })
}

/// Verify the target-neutral optimizer subset without conferring authority on
/// any executable or publication consumer.
pub fn verify_module_for_optimization<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedOptimizableTerminalModule<'module>, VerificationError> {
    let validated = validate_module_for_optimization(module).map_err(VerificationError::Module)?;
    verify_validated_module(validated.validated(), proof_bundle, profile)
        .map(|state| VerifiedOptimizableTerminalModule { state })
}

/// Verify the exact ranked-countdown subset accepted for whole-entry
/// fixed-fuel derivation.
///
/// Validation and proof reconstruction currently match the interpreter slice,
/// but the distinct result carrier prevents one consumer's authority from
/// silently authorizing the other or any ordinary/native consumer.
pub fn verify_module_for_fixed_fuel<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedFixedFuelTerminalModule<'module>, VerificationError> {
    let validated =
        validate_module_for_interpretation(module).map_err(VerificationError::Module)?;
    verify_validated_module(validated.validated(), proof_bundle, profile)
        .map(|state| VerifiedFixedFuelTerminalModule { state })
}

/// Verify the exact structural Unit `u32` ranked-countdown subset admitted for
/// native lowering.
///
/// Ordinary execution continues to reject ranked control. Interpreter and
/// fixed-fuel verification use different opaque result types and therefore
/// cannot authorize this native boundary.
pub fn verify_module_for_native_ranked_countdown<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedNativeRankedTerminalModule<'module>, VerificationError> {
    let validated =
        validate_module_for_native_ranked_countdown(module).map_err(VerificationError::Module)?;
    verify_validated_module(validated, proof_bundle, profile)
        .map(|state| VerifiedNativeRankedTerminalModule { state })
}

fn verify_validated_module<'module>(
    validated: ValidatedTerminalModule<'module>,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedTerminalModuleState<'module>, VerificationError> {
    let module = validated.module();
    let structural_frontiers = reconstruct_validated_structural_ownership_frontiers(module)
        .map_err(VerificationError::Module)?;
    let reconstructed_obligations =
        reconstruct_validated_terminal_obligations(module).map_err(VerificationError::Module)?;
    let reconstructed_recursive_components =
        crate::proof_recursion::reconstruct_validated_proof_recursive_component_obligations(module);
    validate_evidence_producer_provenance(module, proof_bundle)?;
    let contexts = module
        .machines
        .iter()
        .map(|machine| {
            validated
                .value_context(machine)
                .map(|context| (machine.id, context))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(VerificationError::Module)?;
    let mut evidence = BTreeMap::new();
    for entry in &proof_bundle.evidence {
        if evidence
            .insert(entry.obligation, entry.route.clone())
            .is_some()
        {
            return Err(VerificationError::DuplicateEvidence(entry.obligation));
        }
    }

    let mut accepted_facts = Vec::new();
    for site in reconstructed_obligations.obligations() {
        let context = contexts
            .get(&site.owner.machine())
            .expect("validated reconstructed obligation owner exists");
        let machine = validated
            .machine(site.owner.machine())
            .expect("validated reconstructed obligation machine exists");
        let machine_parameter_values = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<BTreeSet<_>>();
        let route = evidence
            .remove(&site.obligation.id)
            .ok_or(VerificationError::MissingEvidence(site.obligation.id))?;
        let accepted = verify_obligation_with_machine_parameters(
            context,
            &site.obligation,
            &site.requirements,
            &site.semantic_axioms,
            &machine_parameter_values,
            route,
            profile,
        )
        .map_err(|error| VerificationError::RejectedEvidence {
            obligation: site.obligation.id,
            error,
        })?;
        accepted_facts.push(accepted);
    }

    if let Some(obligation) = evidence.keys().next().copied() {
        return Err(VerificationError::UnknownEvidence(obligation));
    }

    let mut recursive_evidence = BTreeMap::new();
    let mut previous_component = None;
    for entry in &proof_bundle.recursive_components {
        if previous_component.is_some_and(|previous| previous >= entry.component) {
            return Err(VerificationError::NonCanonicalRecursiveComponentEvidence);
        }
        previous_component = Some(entry.component);
        if recursive_evidence
            .insert(entry.component, entry.certificate.clone())
            .is_some()
        {
            return Err(VerificationError::DuplicateRecursiveComponentEvidence(
                entry.component,
            ));
        }
    }
    let mut accepted_recursive_components = Vec::new();
    for (component, obligation) in module
        .proof_recursive_components
        .iter()
        .zip(reconstructed_recursive_components.iter())
    {
        let identity = crate::proof_recursive_component_identity(component);
        let certificate = recursive_evidence.remove(&identity).ok_or(
            VerificationError::MissingRecursiveComponentEvidence(identity),
        )?;
        let acceptance = verify_recursive_component(
            &semantic_vocabulary::PropositionContext::default(),
            obligation,
            certificate,
            profile,
        )
        .map_err(|error| VerificationError::RejectedRecursiveComponent {
            component: identity,
            error,
        })?;
        accepted_recursive_components.push(acceptance);
    }
    if let Some(component) = recursive_evidence.keys().next().copied() {
        return Err(VerificationError::UnknownRecursiveComponentEvidence(
            component,
        ));
    }
    Ok(VerifiedTerminalModuleState {
        validated,
        proof_bundle: proof_bundle.clone(),
        reconstructed_obligations,
        accepted_facts,
        accepted_recursive_components,
        structural_frontiers,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Module(ModuleError),
    NonDenseEvidenceProducer {
        expected: EvidenceIdentity,
        actual: EvidenceIdentity,
    },
    NonCanonicalEvidenceProducerOrder,
    DuplicateEvidenceProducerTerm(EvidenceTermId),
    UnknownEvidenceProducerTerm(EvidenceTermId),
    UnusedEvidenceProducerTerm(EvidenceTermId),
    MissingEvidenceProducer(EvidenceTermId),
    InvalidEvidenceProducer(EvidenceIdentity),
    EvidenceProducerInterfaceMismatch(EvidenceTermId),
    NonCanonicalEvidenceProducerRows(EvidenceIdentity),
    DuplicateEvidence(ObligationId),
    MissingEvidence(ObligationId),
    UnknownEvidence(ObligationId),
    NonCanonicalRecursiveComponentEvidence,
    DuplicateRecursiveComponentEvidence(semantic_vocabulary::RecursiveComponentId),
    MissingRecursiveComponentEvidence(semantic_vocabulary::RecursiveComponentId),
    UnknownRecursiveComponentEvidence(semantic_vocabulary::RecursiveComponentId),
    RejectedRecursiveComponent {
        component: semantic_vocabulary::RecursiveComponentId,
        error: RecursiveComponentError,
    },
    RejectedEvidence {
        obligation: ObligationId,
        error: EvidenceError,
    },
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VerificationError {}
