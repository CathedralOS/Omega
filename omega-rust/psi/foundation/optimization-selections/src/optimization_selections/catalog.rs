use crate::PsiOptimization;

/// Canonical execution order for every target-neutral Psi optimization pass.
/// The post-Terminal-only passes still order here so selection identities
/// stay canonical across phases.
pub const PRETERMINAL_PSI_PASS_CATALOG: [PsiOptimization; 9] = [
    PsiOptimization::ControlFlowCleanup,
    PsiOptimization::SparseConditionalConstantPropagation,
    PsiOptimization::CopyPropagation,
    PsiOptimization::GlobalValueNumbering,
    PsiOptimization::DeadPureScalarElimination,
    PsiOptimization::ProofCheckElision,
    PsiOptimization::StateSpecialization,
    PsiOptimization::RepresentationSpecialization,
    PsiOptimization::LoopInvariantScalarMotion,
];
