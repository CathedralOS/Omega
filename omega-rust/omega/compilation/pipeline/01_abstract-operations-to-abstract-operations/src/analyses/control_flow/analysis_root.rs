//! Optimizer module role: shared root. The identity preconditions every
//! replayable countdown analysis proves before it reads a custody row.
//!
//! Custody, certificates, and each analysis snapshot are derived from one
//! immutable optimization-unit revision. A stale content identity, a foreign
//! Terminal Psi, or a snapshot bound to another revision makes every row read
//! beneath it meaningless, so the check belongs to one owner rather than to
//! each analysis's own copy of it.

use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{PsiOptimizationUnit, recompute_psi_optimization_unit_identity};

use crate::validation::ValidatedOptimizerCycleComponents;

/// The identity root one analysis snapshot carries: the unit revision and the
/// Terminal-Psi identity its facts were derived under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisRoot {
    pub revision: OptimizationUnitIdentity,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
}

impl AnalysisRoot {
    pub(super) const fn of(unit: &PsiOptimizationUnit) -> Self {
        Self {
            revision: unit.identity,
            terminal_psi: unit.psi,
        }
    }
}

/// Why the analysis inputs at hand do not belong to the unit under analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisRootError {
    StaleUnitIdentity {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    TerminalIdentityMismatch,
    RevisionMismatch,
}

/// The unit's stored content identity still names its content.
pub(in crate::analyses) fn validate_unit_identity(
    unit: &PsiOptimizationUnit,
) -> Result<(), AnalysisRootError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if recomputed != unit.identity {
        return Err(AnalysisRootError::StaleUnitIdentity {
            stored: unit.identity,
            recomputed,
        });
    }
    Ok(())
}

/// The one root precondition: the unit's content identity re-derives, custody
/// and every supplied snapshot were derived under the unit's Terminal Psi, and
/// each snapshot is bound to this unit revision.
pub(in crate::analyses) fn validate_analysis_root(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    snapshots: &[AnalysisRoot],
) -> Result<(), AnalysisRootError> {
    validate_unit_identity(unit)?;
    if unit.psi != custody.terminal_psi()
        || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
        || snapshots
            .iter()
            .any(|snapshot| snapshot.terminal_psi != unit.psi)
    {
        return Err(AnalysisRootError::TerminalIdentityMismatch);
    }
    if snapshots
        .iter()
        .any(|snapshot| snapshot.revision != unit.identity)
    {
        return Err(AnalysisRootError::RevisionMismatch);
    }
    Ok(())
}
