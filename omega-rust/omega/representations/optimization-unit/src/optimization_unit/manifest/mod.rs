//! Optimizer module role: stage group. Persistable pre-physical report data and canonical encoding, not validation authority.

mod codec;
mod model;
mod rendering;

pub use model::*;

use crate::{
    ProvenanceDisposition, ProvenanceRewrite, PsiProvenance, PsiRealizationSite,
    PsiTransformationLedger,
};
use optimization_core::{
    BaselineDecisionLog, OptimizationCandidateVerdict, OptimizationFactReference,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;
use terminal_psi::TerminalPsiIdentity;
