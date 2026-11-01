//! Shared executable policy content and exact source-issued entry associations.
//!
//! This catalog borrows the same artifact as the final Product. Publication is
//! owned by the compiler's ordinary Psi production request. Source admission
//! precedes each activation; retaining code
//! grants neither invocation nor provider authority.

use super::{CanonicalCallingSignature, TerminalCallingPolicyInvocation};
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
    pub source_records: std::sync::Arc<[checked_trees::typed_trees::SourceRecordCorrespondence]>,
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
        source_policy: SymbolHandle,
        policy: terminal_psi::TargetMachineCallable,
        target: target::TargetProfile,
        declaration: semantic_vocabulary::CallingSignatureId,
        providers: &[backend_plan::TerminalTargetProviderPlan],
    ) -> Result<(CanonicalCallingSignature, ValidatedBoundaryEntryPlan), String> {
        let entry = self
            .entries
            .iter()
            .find(|(source, _)| *source == source_policy)
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
        let selected = terminal_installation::admit_target_machine_selections(
            self.artifact.view(),
            target.target_name(),
        );
        let selected_entry = selected
            .entry(
                selected.terminal_psi(),
                self.artifact.semantic_module(),
                policy,
            )
            .map_err(|error| format!("calling policy canonical selection: {error}"))?;
        if selected_entry != *entry {
            return Err(
                "calling policy canonical body differs from its admitted source entry".into(),
            );
        }
        let requested = CanonicalCallingSignature::extent_demands(
            self.artifact.semantic_module(),
            declaration,
            target,
        )?;
        let mut extents = Vec::with_capacity(requested.len());
        if !requested.is_empty() {
            let providers = backend_plan::TerminalTargetProviderSelection::from_plans(
                providers.to_vec(),
                self.artifact.view(),
            )?
            .admit(self.artifact.view(), target.target_name())
            .map_err(|error| format!("calling ABI scalar providers: {error}"))?;
            let mut meter =
                terminal_fuel::TerminalFuelMeter::with_invocation(account.clone(), None)
                    .map_err(|error| format!("calling ABI scalar fuel: {error:?}"))?;
            let values = super::scalar_completion::complete_policy_scalar_inputs(
                &self.artifact,
                target,
                policy,
                &requested,
                &providers,
                &mut meter,
            )?;
            for application in requested {
                let value = match values.values().value(application) {
                    Some(terminal_installation::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    }) => u64::try_from(value).map_err(|_| "calling ABI extent exceeds u64")?,
                    Some(terminal_installation::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    }) => u64::try_from(value)
                        .map_err(|_| "calling ABI extent is negative or exceeds u64")?,
                    _ => return Err("calling ABI extent has no completed integer value".into()),
                };
                extents.push((application, value));
            }
        }
        let canonical = CanonicalCallingSignature::materialize(
            self.artifact.semantic_module(),
            declaration,
            target,
            &|application| {
                extents
                    .iter()
                    .find(|(identity, _)| *identity == application)
                    .map(|(_, value)| *value)
                    .ok_or(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                        application,
                    ))
            },
        )?;
        let (plan, _) = super::evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact: &self.artifact,
            entry: policy,
            target,
            signature: &canonical.policy,
            account,
            providers,
        })?;
        Ok((canonical, plan))
    }
}
