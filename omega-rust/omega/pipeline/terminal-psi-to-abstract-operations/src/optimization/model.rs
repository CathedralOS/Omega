use crate::shared::*;

/// Required optimizer input produced only after canonical artifact decoding,
/// Terminal-Psi validation, proof reconstruction, and evidence admission.
///
/// The ordinary native path may consume the bare abstract plan for backwards
/// compatibility. Optimizer entry points must instead require this carrier so
/// proof, ownership, and path-sensitive semantic context cannot become an
/// optional side channel.
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
