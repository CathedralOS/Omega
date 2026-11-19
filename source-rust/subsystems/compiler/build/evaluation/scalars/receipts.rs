//! Preserve the artifact, exact scope and measured work of each scalar evaluation.
use build_time_values::BuildTimeValue;
use terminal_interpreter::TerminalExecutionResult;

pub const PURE_PSI_WORK_BUDGET: u64 = 100_000;

#[derive(Debug, Clone, record_storage::Record)]
pub enum PureTerminalEvaluationOutcome {
    Returned {
        value: TerminalExecutionResult,
    },
    // A Terminal invocation whose result the compiler read back from the
    // completed instance, with the storage measurement of that invocation.
    Materialized {
        value: BuildTimeValue,
        usage: evaluation_contracts::BuildEvaluationUsage,
    },
    Failed {
        reason: String,
    },
}

mod canonical_scalar {
    use super::{PURE_PSI_WORK_BUDGET, PureTerminalEvaluationOutcome};
    use psi_provider_selections::{AssumedBindingSelection, ProviderInstallationSelection};
    use record_storage::{Record, RecordError, RecordReader};
    use semantic_vocabulary::{ScalarApplicationId, ScalarType, ValueId};
    use std::sync::Arc;
    use terminal_codec::CanonicalTerminalArtifact;
    use terminal_semantics::entry_values::TerminalScalarValue;

    #[derive(Debug, Clone)]
    struct RecordedArtifact(Arc<CanonicalTerminalArtifact>);

    impl Record for RecordedArtifact {
        fn write(&self, output: &mut record_storage::RecordWriter) -> Result<(), RecordError> {
            self.0.to_bytes().write(output)
        }

        fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
            let bytes = Vec::<u8>::read(input)?;
            let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
                .map_err(|error| input.error(error.to_string()))?;
            Ok(Self(Arc::new(artifact)))
        }
    }

    #[derive(Debug, Clone, Record)]
    pub(crate) struct CanonicalScalarScope {
        artifact: RecordedArtifact,
        // Replay must preserve unscoped evaluation instead of substituting
        // whichever deployment happens to host the reader.
        target: Option<String>,
        providers: Vec<ProviderInstallationSelection>,
        assumptions: Vec<AssumedBindingSelection>,
        carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    }

    impl CanonicalScalarScope {
        pub(crate) fn capture(
            artifact: &Arc<CanonicalTerminalArtifact>,
            target: Option<&str>,
            providers: &psi_provider_selections::AdmittedProviderSelections,
            carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
        ) -> Result<Arc<Self>, String> {
            if terminal_codec::TerminalExecutionView::from(artifact.view()).custody_identity()
                != *providers.execution_identity()
            {
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
                target: target.map(str::to_owned),
                providers: selections,
                assumptions,
                carrier_layout,
            }))
        }
    }

    #[derive(Debug, Clone, Record)]
    pub struct CanonicalScalarEvaluationReceipt {
        scope: Arc<CanonicalScalarScope>,
        application: ScalarApplicationId,
        values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
        captures: Vec<(ValueId, ScalarType)>,
        outcome: PureTerminalEvaluationOutcome,
        usage: terminal_work::WorkUsage,
    }

    impl CanonicalScalarEvaluationReceipt {
        pub(crate) fn new(
            scope: Arc<CanonicalScalarScope>,
            application: ScalarApplicationId,
            values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
            captures: Vec<(ValueId, ScalarType)>,
            outcome: PureTerminalEvaluationOutcome,
            usage: terminal_work::WorkUsage,
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
        pub fn target(&self) -> Option<&str> {
            self.scope.target.as_deref()
        }
        pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
            &self.outcome
        }
        pub fn usage(&self) -> &terminal_work::WorkUsage {
            &self.usage
        }

        pub fn returned(&self) -> bool {
            matches!(self.outcome, PureTerminalEvaluationOutcome::Returned { .. })
        }

        pub fn validate_charge(&self) -> Result<(), String> {
            if self.usage.schedule() != terminal_work::WorkSchedule::CURRENT.identity()
                || self.usage.total_units() > PURE_PSI_WORK_BUDGET
            {
                return Err("canonical scalar evaluation work policy changed".into());
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
}
pub use canonical_scalar::CanonicalScalarEvaluationReceipt;
pub(crate) use canonical_scalar::CanonicalScalarScope;
