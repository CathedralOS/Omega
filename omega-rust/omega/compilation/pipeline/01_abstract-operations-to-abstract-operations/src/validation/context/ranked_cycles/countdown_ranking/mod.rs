//! Optimizer module role: executable entrance. Exact countdown ranking-evidence reconstruction.

use super::{
    OptimizationUnitValidationError, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, PsiOptimizationUnit,
    ValidatedOptimizerRankingCertificates,
};
mod current;
mod terminal;

/// Reauthenticate replayable well-founded ranking evidence against separately
/// reconstructed Terminal and current optimizer bodies.
pub fn validate_psi_ranking_certificate_snapshot(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    candidate: &OptimizerRankingCertificateSnapshot,
) -> Result<ValidatedOptimizerRankingCertificates, OptimizationUnitValidationError> {
    let validated = super::super::validate_psi_optimization_unit_with_context(input, unit, false)?;
    if candidate != validated.ranking_certificates().snapshot() {
        return Err(OptimizationUnitValidationError::RankedCycleRankingCertificateSnapshotMismatch);
    }
    Ok(validated.ranking_certificates().clone())
}

pub(super) fn rederive_exact_certificates(
    module: &::terminal_psi::TerminalModule,
    unit: &PsiOptimizationUnit,
    components: &OptimizerCycleComponentSnapshot,
) -> Result<OptimizerRankingCertificateSnapshot, OptimizationUnitValidationError> {
    // Only natural components matching the unsigned-countdown idiom supply
    // these analysis certificates. Other verified components retain their
    // source and frozen body without acquiring a countdown certificate or
    // fixed-work bound.
    let mut recognized = Vec::new();
    let mut terminal = Vec::new();
    for component in &components.components {
        if let Some(certificate) = self::terminal::project(module, component)? {
            recognized.push(component.clone());
            terminal.push(certificate);
        }
    }
    terminal.sort_by(|left, right| left.component.cmp(&right.component));
    let countdown = OptimizerCycleComponentSnapshot {
        terminal_psi: components.terminal_psi,
        components: recognized,
    };
    let current = current::derive(unit, &countdown)?;
    if terminal != current {
        let machine = components
            .components
            .first()
            .map_or(module.entry, |component| component.id.machine);
        return Err(
            OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch { machine },
        );
    }
    Ok(OptimizerRankingCertificateSnapshot {
        terminal_psi: components.terminal_psi,
        certificates: current,
    })
}
