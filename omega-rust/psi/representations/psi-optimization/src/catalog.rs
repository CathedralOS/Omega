use crate::PsiOptimization;

/// Canonical execution order for every target-neutral Psi optimization pass.
pub const PRETERMINAL_PSI_PASS_CATALOG: [PsiOptimization; 6] = [
    PsiOptimization::ControlFlowCleanup,
    PsiOptimization::SparseConditionalConstantPropagation,
    PsiOptimization::CopyPropagation,
    PsiOptimization::GlobalValueNumbering,
    PsiOptimization::DeadPureScalarElimination,
    PsiOptimization::ProofCheckElision,
];
