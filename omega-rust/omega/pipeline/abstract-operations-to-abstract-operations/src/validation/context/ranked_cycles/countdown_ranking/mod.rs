//! Optimizer module role: executable entrance. Exact countdown ranking-evidence reconstruction.

use super::*;

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
    // Only legacy unsigned countdowns supply these analysis certificates.
    // Natural and unranked components retain their verified source and frozen
    // body without acquiring a countdown certificate or fixed-work bound.
    let countdown = OptimizerCycleComponentSnapshot {
        terminal_psi: components.terminal_psi,
        components: components
            .components
            .iter()
            .filter(|component| is_unsigned_countdown(module, component.id.machine))
            .cloned()
            .collect(),
    };
    let terminal = self::terminal::derive(module, &countdown)?;
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

pub(super) fn is_unsigned_countdown(
    module: &::terminal_psi::TerminalModule,
    machine: MachineId,
) -> bool {
    module.machines.iter().any(|candidate| {
        candidate.id == machine
            && candidate
                .ranked_scc
                .as_ref()
                .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
    })
}
