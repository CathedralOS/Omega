//! Optimizer module role: executable entrance. Optimizer-input entrance: reconstruct the complete unit seed, bind admitted
//! proof facts, proof questions, and ownership frontiers, then seal the unit
//! beside the verifier-owned input context.

mod accepted_obligations;
mod error;
mod ownership_frontiers;
mod proof_questions;

pub use error::VerifiedPsiOptimizationUnitBuildError;

use abstract_operations::AbstractOperationPlan;
use accepted_obligations::project_accepted_obligation_facts;
use ownership_frontiers::project_ownership_frontiers;
use proof_questions::project_proof_questions;

/// The only optimizer-facing unit constructor. Consuming the verified carrier
/// prevents callers from pairing a plan with evidence admitted for a different
/// Terminal-Psi artifact.
pub fn build_verified_psi_optimization_unit(
    input: VerifiedPsiOptimizationInput,
    fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), fuel_schedule)?;
    let context = input.context();
    optimization_unit::attach_verified_module_context(&mut seed, context.module())
        .map_err(VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine)?;
    let facts = project_accepted_obligation_facts(&seed, context)?;
    let unit = optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    let proof_questions = project_proof_questions(&input)?;
    let unit = optimization_unit::attach_proof_questions(unit, proof_questions)?;
    let ownership_frontiers = project_ownership_frontiers(&input)?;
    let unit = optimization_unit::attach_ownership_frontier_facts(unit, ownership_frontiers)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

/// Required optimizer input produced only after canonical artifact decoding,
/// Terminal-Psi validation, proof reconstruction, and evidence admission.
///
/// Optimizer entry points require this carrier so proof, ownership, and
/// path-sensitive semantic context cannot become an optional side channel.
/// Native artifact admission retains it inside its distinct authority carrier;
/// optimizer-only input cannot grant native admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
    pub(crate) plan: AbstractOperationPlan,
    pub(crate) context: VerifiedPsiOptimizationContext,
}

impl VerifiedPsiOptimizationInput {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
    }
}

/// Verifier-owned semantic and proof context retained beside the reconstructible
/// Omega plan. The complete immutable Terminal module is intentional: narrow
/// projections may be derived from it, but cannot recreate discarded place
/// paths, call obligations, edge cleanup, or borrow frontiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    pub(crate) module: terminal_psi::TerminalModule,
    pub(crate) proof_bundle: terminal_verifier::ProofBundle,
    pub(crate) proof_bundle_fingerprint: terminal_codec::ProofBundleFingerprint,
    pub(crate) reconstructed_obligations: terminal_verifier::ReconstructedTerminalObligationSet,
    pub(crate) accepted_facts: Vec<proof_admission::AcceptedFact>,
    pub(crate) structural_frontiers: terminal_verifier::VerifiedTerminalStructuralFrontiers,
}

impl VerifiedPsiOptimizationContext {
    pub const fn module(&self) -> &terminal_psi::TerminalModule {
        &self.module
    }

    pub const fn proof_bundle(&self) -> &terminal_verifier::ProofBundle {
        &self.proof_bundle
    }

    pub const fn proof_bundle_fingerprint(&self) -> terminal_codec::ProofBundleFingerprint {
        self.proof_bundle_fingerprint
    }

    pub const fn reconstructed_obligations(
        &self,
    ) -> &terminal_verifier::ReconstructedTerminalObligationSet {
        &self.reconstructed_obligations
    }

    pub fn accepted_facts(&self) -> &[proof_admission::AcceptedFact] {
        &self.accepted_facts
    }

    pub const fn structural_frontiers(
        &self,
    ) -> &terminal_verifier::VerifiedTerminalStructuralFrontiers {
        &self.structural_frontiers
    }
}

/// A reconstructible optimizer unit that cannot detach from the exact
/// verifier context which authorized its proof- and borrow-sensitive facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationUnit {
    pub(crate) input: VerifiedPsiOptimizationInput,
    pub(crate) unit: optimization_unit::PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationUnit {
    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &optimization_unit::PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(
        self,
    ) -> (
        VerifiedPsiOptimizationInput,
        optimization_unit::PsiOptimizationUnit,
    ) {
        (self.input, self.unit)
    }
}
