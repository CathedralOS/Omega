//! Proof verification of one validated terminal module.
//!
//! `verify_module` (and its interpretation, optimization and fixed-fuel
//! variants) runs `verify_validated_module`: it reconstructs every
//! executable-site obligation and all-path fact (`reconstruction`), composes
//! call contracts under the three exact call policies (`call_composition`),
//! preserves copied scalar facts across storage expiry (`field_snapshots`),
//! substitutes retained terms capture-free (`substitution`), reconstructs
//! float-meaning projections (`float_meaning_projection`), and discharges
//! each obligation against the proof bundle through `proof_admission`,
//! recording evidence producers (`evidence_provenance`).

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::{
    AcceptedFact, AdmissionProfile, EvidenceError, RecursiveComponentAcceptance,
    RecursiveComponentError, verify_obligation_with_machine_parameters, verify_recursive_component,
};
use semantic_vocabulary::{EvidenceIdentity, EvidenceTermId, ObligationId, Proposition};
use terminal_psi::TerminalModule;

use crate::{
    ModuleError, ValidatedTerminalModule, VerifiedTerminalStructuralFrontiers,
    reconstruct_validated_structural_ownership_frontiers, validate_module,
    validate_module_for_interpretation, validate_module_for_optimization,
};

mod call_composition;
mod evidence_provenance;
mod field_snapshots;
mod float_meaning_projection;
mod proof_bundle;
mod reconstruction;
pub(crate) use reconstruction::reconstruct_validated_control_edge_axioms;
mod substitution;

use evidence_provenance::validate_evidence_producer_provenance;
pub use float_meaning_projection::{
    FloatMeaningProjectionVerificationError, ReconstructedFloatMeaningProjection,
    reconstruct_float_meaning_projection,
};
pub(crate) use float_meaning_projection::{
    verify_direct_block_float_parameter, verify_direct_call_float_result,
    verify_direct_float_parameter, verify_direct_float_result,
    verify_direct_operation_float_result, verify_direct_structural_float_leaf,
};
pub use proof_bundle::{
    ControlCycleEvidence, CrashCertificate, CrashObligationEvidence, CrashObligationOwner,
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence, ProofBundle, RecursiveComponentEvidence,
};
use reconstruction::reconstruct_validated_crash_obligations;
use reconstruction::reconstruct_validated_terminal_obligations;
pub use reconstruction::{
    CrashObligationQuestion, ReconstructedCrashObligation, ReconstructedOperationObligation,
    ReconstructedTerminalObligation, ReconstructedTerminalObligationOwner,
    ReconstructedTerminalObligationSet, reconstruct_crash_obligations,
    reconstruct_execution_crash_obligations, reconstruct_execution_terminal_obligations,
    reconstruct_interpretable_crash_obligations, reconstruct_interpretable_operation_obligations,
    reconstruct_interpretable_terminal_obligations, reconstruct_operation_obligations,
    reconstruct_optimizable_crash_obligations, reconstruct_optimizable_terminal_obligations,
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

/// Proof-checked authority for target-neutral optimizer analysis of Terminal
/// Psi whose recursive control is certified by the common natural-cycle
/// relation.
///
/// This opaque result grants no execution, interpretation, fixed-fuel,
/// native-lowering, or publication authority.
#[derive(Debug)]
pub struct VerifiedOptimizableTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

/// Proof-checked authority for deriving a whole-entry fixed-fuel theorem.
///
/// This carrier is deliberately distinct from both ordinary execution
/// authority and interpreter authority. In particular, it cannot authorize
/// native lowering or be supplied to the reference interpreter.
#[derive(Debug)]
pub struct VerifiedFixedFuelTerminalModule<'module> {
    state: VerifiedTerminalModuleState<'module>,
}

#[derive(Debug)]
struct VerifiedTerminalModuleState<'module> {
    validated: ValidatedTerminalModule<'module>,
    proof_bundle: ProofBundle,
    reconstructed_obligations: ReconstructedTerminalObligationSet,
    accepted_facts: Vec<AcceptedFact>,
    accepted_recursive_components: Vec<RecursiveComponentAcceptance>,
    accepted_control_cycles: Vec<crate::AcceptedControlCycle>,
    structural_frontiers: VerifiedTerminalStructuralFrontiers,
}

impl<'module> VerifiedTerminalModule<'module> {
    /// Check optimizer eligibility for this exact already-verified module.
    /// Proof reconstruction and admission are retained, not repeated. This is
    /// a one-way transfer; optimizer authority cannot recover execution authority.
    pub fn into_optimization(
        mut self,
    ) -> Result<VerifiedOptimizableTerminalModule<'module>, VerificationError> {
        let validated =
            validate_module_for_optimization(self.module()).map_err(VerificationError::Module)?;
        self.state.validated = validated.validated();
        Ok(VerifiedOptimizableTerminalModule { state: self.state })
    }

    pub const fn module(&self) -> &'module TerminalModule {
        self.state.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.state.accepted_facts
    }

    pub fn accepted_recursive_components(&self) -> &[RecursiveComponentAcceptance] {
        &self.state.accepted_recursive_components
    }

    pub fn accepted_control_cycles(&self) -> &[crate::AcceptedControlCycle] {
        &self.state.accepted_control_cycles
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

/// Verify a module for whole-entry fixed-fuel derivation.
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
    let reconstructed_cycles =
        crate::control_cycles::reconstruct_validated_control_cycle_obligations(module)
            .map_err(VerificationError::Module)?;
    let mut cycle_evidence = BTreeMap::new();
    let mut previous_cycle = None;
    for entry in &proof_bundle.control_cycles {
        if previous_cycle.is_some_and(|previous| previous >= entry.component) {
            return Err(VerificationError::NonCanonicalControlCycleEvidence);
        }
        previous_cycle = Some(entry.component);
        cycle_evidence.insert(entry.component, entry.certificate.clone());
    }
    let mut accepted_control_cycles = Vec::new();
    for question in reconstructed_cycles {
        let certificate = cycle_evidence.remove(&question.component).ok_or(
            VerificationError::MissingControlCycleEvidence(question.component),
        )?;
        let context = contexts
            .get(&question.machine)
            .expect("validated machine context");
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == question.machine)
            .expect("validated cycle owner");
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        let acceptance = proof_admission::verify_recursive_component_with_machine_parameters(
            context,
            &question.obligation,
            &parameters,
            certificate,
            profile,
        )
        .map_err(|error| VerificationError::RejectedControlCycle {
            component: question.component,
            error,
        })?;
        accepted_control_cycles.push(crate::AcceptedControlCycle {
            machine: question.machine,
            component: question.component,
            acceptance,
        });
    }
    if let Some(component) = cycle_evidence.keys().next().copied() {
        return Err(VerificationError::UnknownControlCycleEvidence(component));
    }
    // Crash obligations are reconstructed from the module alone — every
    // asserted crash-site guard against each reconstructed path, and every
    // uncovered call continuation's coverage or refutation goal. The supplied
    // roster is matched by owner and each certificate is re-decided under
    // the question's own goal, requirements, and axioms; nothing here
    // searches or trusts producer claims.
    let crash_questions =
        reconstruct_validated_crash_obligations(module).map_err(VerificationError::Module)?;
    let mut crash_evidence = BTreeMap::new();
    let mut previous_crash = None;
    for entry in &proof_bundle.crash_obligations {
        if previous_crash.is_some_and(|previous| previous >= entry.owner) {
            return Err(VerificationError::NonCanonicalCrashObligationEvidence);
        }
        previous_crash = Some(entry.owner);
        if crash_evidence.insert(entry.owner, entry).is_some() {
            return Err(VerificationError::DuplicateCrashObligationEvidence(
                entry.owner,
            ));
        }
    }
    for question in &crash_questions {
        let evidence = crash_evidence.remove(&question.owner).ok_or(
            VerificationError::MissingCrashObligationEvidence(question.owner),
        )?;
        if !crash_obligation_discharged(question, evidence) {
            return Err(VerificationError::RejectedCrashObligationEvidence {
                owner: question.owner,
            });
        }
    }
    if let Some(owner) = crash_evidence.keys().next().copied() {
        return Err(VerificationError::UnknownCrashObligationEvidence(owner));
    }
    Ok(VerifiedTerminalModuleState {
        validated,
        proof_bundle: proof_bundle.clone(),
        reconstructed_obligations,
        accepted_facts,
        accepted_recursive_components,
        accepted_control_cycles,
        structural_frontiers,
    })
}

/// Whether the supplied crash-certificate roster discharges one reconstructed
/// obligation. Slot arity is part of the canonical row shape: a `Site` row
/// carries one coverage roster per asserted guard and one refutation roster
/// per reconstructed path; a `Continuation` row carries one coverage roster
/// per formed coverage goal and a refutation roster exactly when the
/// uncovered routes formed a complement goal. Every check replays the
/// certificate's recorded denotation lane; no search runs here.
///
/// Producers replay this same predicate to refuse emitting a roster their own
/// receiver would reject; verification runs it again on untrusted input.
pub fn crash_obligation_discharged(
    question: &ReconstructedCrashObligation,
    evidence: &CrashObligationEvidence,
) -> bool {
    let Some(context) = &question.context else {
        return false;
    };
    let check = |goal: &Proposition,
                 semantic_axioms: &[Proposition],
                 certificate: &terminal_psi::CrashCertificate| {
        proof_admission::check_denotation_certificate(
            context,
            goal,
            &question.requirements,
            semantic_axioms,
            certificate,
        )
    };
    match &question.question {
        CrashObligationQuestion::Site { guards, paths } => {
            // A site with no reconstructed path is not vacuously true: the
            // missing-path row is exactly what proof supply must not erase.
            if paths.is_empty()
                || evidence.coverage.len() != guards.len()
                || evidence.refutation.len() != paths.len()
            {
                return false;
            }
            guards.iter().enumerate().all(|(guard_index, guard)| {
                paths.iter().enumerate().all(|(path_index, axioms)| {
                    evidence.coverage[guard_index].iter().any(|certificate| {
                        check(guard, axioms, certificate)
                    })
                        // An infeasible path cannot reach this terminator, but
                        // only a kernel-checked contradiction certificate
                        // discharges it — missing supply never removes a path.
                        || evidence.refutation[path_index].iter().any(|certificate| {
                            check(&Proposition::Falsehood, axioms, certificate)
                        })
                })
            })
        }
        CrashObligationQuestion::Continuation {
            coverage_goals,
            refutation_goal,
        } => {
            if evidence.coverage.len() != coverage_goals.len()
                || evidence.refutation.len() != usize::from(refutation_goal.is_some())
            {
                return false;
            }
            // Entry-requirement questions cite no reconstructed CFG facts.
            let covered =
                coverage_goals
                    .iter()
                    .zip(&evidence.coverage)
                    .any(|(goal, certificates)| {
                        certificates
                            .iter()
                            .any(|certificate| check(goal, &[], certificate))
                    });
            covered
                || refutation_goal.as_ref().is_some_and(|goal| {
                    evidence.refutation[0]
                        .iter()
                        .any(|certificate| check(goal, &[], certificate))
                })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Module(ModuleError),
    NonCanonicalCrashObligationEvidence,
    DuplicateCrashObligationEvidence(CrashObligationOwner),
    MissingCrashObligationEvidence(CrashObligationOwner),
    UnknownCrashObligationEvidence(CrashObligationOwner),
    RejectedCrashObligationEvidence {
        owner: CrashObligationOwner,
    },
    NonCanonicalControlCycleEvidence,
    MissingControlCycleEvidence(semantic_vocabulary::CycleComponentId),
    UnknownControlCycleEvidence(semantic_vocabulary::CycleComponentId),
    RejectedControlCycle {
        component: semantic_vocabulary::CycleComponentId,
        error: RecursiveComponentError<semantic_vocabulary::BlockId>,
    },
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
