//! Shared executable policy content and exact source-issued entry associations.
//!
//! This catalog is a compile-time dependency of provider settlement, not the
//! final Product artifact. Publication is owned by the compiler's ordinary Psi
//! production request. Source admission precedes each activation; retaining code
//! grants neither invocation nor provider authority.

use super::{MaterializedBoundarySignature, TerminalCallingPolicyInvocation};
use calling_conventions::ValidatedBoundaryEntryPlan;
use diagnostics::Diagnostic;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_codec::CanonicalTerminalArtifact;

pub struct CallingPolicyCode {
    /// Association only: this never keeps source alive or enters the interpreter.
    pub source_owner: std::sync::Weak<checked_trees::CheckedTrees>,
    pub artifact: std::sync::Arc<CanonicalTerminalArtifact>,
    pub entries: Vec<(SymbolHandle, Result<MachineId, Vec<Diagnostic>>)>,
}

impl CallingPolicyCode {
    pub(crate) fn require_owner(
        &self,
        checked: &checked_trees::CheckedTrees,
    ) -> Result<(), String> {
        if std::ptr::eq(self.source_owner.as_ptr(), checked) {
            return Ok(());
        }
        Err("calling-policy code belongs to a different checked source owner".into())
    }

    pub(crate) fn evaluate(
        &self,
        policy: SymbolHandle,
        target: target::TargetProfile,
        signature: &MaterializedBoundarySignature,
    ) -> Result<ValidatedBoundaryEntryPlan, String> {
        let entry = self
            .entries
            .iter()
            .find(|(source, _)| *source == policy)
            .ok_or("calling policy has no exact published entry")?
            .1
            .as_ref()
            .map_err(|errors| format!("calling policy publication: {errors:?}"))?;
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        // Pure compiler-run policies had a finite 100,000-step allowance.
        // Terminal uses its own semantic fuel units, never relabelled tree
        // charges. Logical storage/results are independently metered. No host
        // import is installed, regardless of the account's nonzero I/O limits.
        let limits = BuildEvaluationSponsorLimits::new(
            100_000, 1, 1, 1, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
        )
        .ok_or("invalid compiler calling-policy resource limits")?;
        let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, measurement)
            .map_err(|error| error.to_string())?;
        let account = sponsor
            .begin_invocation(measurement, limits.maximum_fuel_units())
            .map_err(|error| error.to_string())?;
        let signature = signature.policy_signature()?;
        super::evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact: &self.artifact,
            entry: terminal_psi::TargetMachineCallable::Machine(*entry),
            target,
            signature: &signature,
            account,
        })
        .map(|(plan, _usage)| plan)
    }
}
