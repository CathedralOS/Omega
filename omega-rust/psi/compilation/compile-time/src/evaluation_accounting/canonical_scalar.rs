//! Source-free inputs and actual charges for canonical scalar entry jobs.
//! Descriptive provider inputs are retained, never a serialized admission.

use std::sync::Arc;

use compiler_storage::Record;
use semantic_vocabulary::{ScalarApplicationId, ScalarType, ValueId};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_installation::{AssumedBindingSelection, ProviderInstallationSelection};

use super::{PURE_TERMINAL_FUEL_CEILING, PureTerminalEvaluationOutcome};

#[derive(Debug, Clone)]
struct RecordedArtifact(Arc<CanonicalTerminalArtifact>);

impl Record for RecordedArtifact {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.0.to_bytes().write(output)
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let bytes = Vec::<u8>::read(input)?;
        let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
            .map_err(|error| input.error(error.to_string()))?;
        Ok(Self(Arc::new(artifact)))
    }
}

#[derive(Debug, Clone, Record)]
pub(crate) struct CanonicalScalarScope {
    artifact: RecordedArtifact,
    target: String,
    providers: Vec<ProviderInstallationSelection>,
    assumptions: Vec<AssumedBindingSelection>,
    carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
}

impl CanonicalScalarScope {
    pub(crate) fn capture(
        artifact: &Arc<CanonicalTerminalArtifact>,
        target: &str,
        providers: &terminal_installation::AdmittedProviderSelections,
        carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    ) -> Result<Arc<Self>, String> {
        if artifact.view().identity() != providers.terminal_psi() {
            return Err("scalar evaluation accounting has a different artifact".into());
        }
        let module = artifact.semantic_module();
        let mut selections = Vec::new();
        for candidate in module
            .provider_candidates
            .iter()
            .filter(|candidate| providers.contains_candidate(candidate))
        {
            selections.push(ProviderInstallationSelection {
                boundary: candidate.boundary,
                candidate: candidate.candidate,
                provider_identity: module
                    .identities
                    .text(candidate.provider_identity)
                    .ok_or("scalar accounting lost provider identity")?
                    .to_owned(),
                provider_declaration: module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    .ok_or("scalar accounting lost selecting declaration")?
                    .to_owned(),
            });
        }
        if selections.len() != providers.providers().count() {
            return Err("scalar accounting lost exact selected provider rows".into());
        }
        let assumptions = providers
            .assumed_bindings()
            .iter()
            .map(|binding| AssumedBindingSelection {
                required: binding.required,
                supplied_requirement: binding.supplied_requirement,
            })
            .collect();
        Ok(Arc::new(Self {
            artifact: RecordedArtifact(Arc::clone(artifact)),
            target: target.to_owned(),
            providers: selections,
            assumptions,
            carrier_layout,
        }))
    }
}

/// One actual entry invocation, with its finite per-invocation allowance.
/// Earlier scalar values are inputs, not results fabricated for this entry.
#[derive(Debug, Clone, Record)]
pub struct CanonicalScalarEvaluationReceipt {
    scope: Arc<CanonicalScalarScope>,
    application: ScalarApplicationId,
    values: Vec<(
        ScalarApplicationId,
        terminal_installation::TerminalScalarValue,
    )>,
    captures: Vec<(ValueId, ScalarType)>,
    outcome: PureTerminalEvaluationOutcome,
    usage: terminal_fuel::TerminalFuelUsage,
}

impl CanonicalScalarEvaluationReceipt {
    pub(crate) fn new(
        scope: Arc<CanonicalScalarScope>,
        application: ScalarApplicationId,
        values: Vec<(
            ScalarApplicationId,
            terminal_installation::TerminalScalarValue,
        )>,
        captures: Vec<(ValueId, ScalarType)>,
        outcome: PureTerminalEvaluationOutcome,
        usage: terminal_fuel::TerminalFuelUsage,
    ) -> Self {
        Self {
            scope,
            application,
            values,
            captures,
            outcome,
            usage,
        }
    }

    pub fn application(&self) -> ScalarApplicationId {
        self.application
    }
    pub fn artifact(&self) -> &CanonicalTerminalArtifact {
        &self.scope.artifact.0
    }
    pub fn target(&self) -> &str {
        &self.scope.target
    }
    pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
        &self.outcome
    }
    pub fn usage(&self) -> &terminal_fuel::TerminalFuelUsage {
        &self.usage
    }

    pub(crate) fn returned(&self) -> bool {
        matches!(self.outcome, PureTerminalEvaluationOutcome::Returned { .. })
    }

    pub fn validate_charge(&self) -> Result<(), String> {
        if self.usage.schedule() != terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
            || self.usage.total_units() > PURE_TERMINAL_FUEL_CEILING
        {
            return Err("canonical scalar evaluation fuel policy changed".into());
        }
        let total = self
            .usage
            .attribution()
            .values()
            .try_fold(0_u64, |total, row| total.checked_add(row.units()));
        if total != Some(self.usage.total_units()) {
            return Err("canonical scalar evaluation attribution is inconsistent".into());
        }
        if !matches!(
            self.artifact()
                .semantic_module()
                .scalar_applications
                .declaration(self.application),
            Some(terminal_psi::ScalarApplication::Evaluate { .. })
        ) {
            return Err("canonical scalar receipt has no actual evaluation entry".into());
        }
        Ok(())
    }
}
