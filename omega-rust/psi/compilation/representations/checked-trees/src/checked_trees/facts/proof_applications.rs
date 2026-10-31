//! Checked coordinates for the shared, source-independent application walk.

use flow_effects::OperationalPlan;
use semantic_vocabulary::proof_applications;
use symbols::SymbolHandle;

pub type ProofApplicationMachine = proof_applications::ProofApplicationMachine<SymbolHandle>;
pub type ProofApplicationTargetMember =
    proof_applications::ProofApplicationTargetMember<SymbolHandle>;
pub type ProofDeclarationDependencies =
    proof_applications::ProofDeclarationDependencies<SymbolHandle>;
pub type ProofApplicationPlan = proof_applications::ProofApplicationPlan<SymbolHandle>;
pub type ProofApplications<'a> =
    proof_applications::ProofApplications<'a, SymbolHandle, OperationalPlan>;

#[cfg(test)]
mod tests;
