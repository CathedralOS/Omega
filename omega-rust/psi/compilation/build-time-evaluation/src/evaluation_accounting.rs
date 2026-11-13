/*
    Compile-time evaluation receipts, work and cell usage accounting, and
    canonical scalar evaluation records.

    We record per-invocation step and cell usage across pre-resolution and
    pre-check evaluation rounds and bind canonical scalar evaluation receipts to
    their verified Terminal execution outcomes.
*/

use std::sync::Arc;

use crate::BuildTimeValue;
use terminal_interpreter::TerminalExecutionResult;

mod canonical_scalar {
    use super::{PURE_PSI_WORK_BUDGET, PureTerminalEvaluationOutcome};
    use compiler_storage::{Record, RecordError, RecordReader};
    use semantic_vocabulary::{ScalarApplicationId, ScalarType, ValueId};
    use std::sync::Arc;
    use terminal_codec::CanonicalTerminalArtifact;
    use terminal_installation::{
        AssumedBindingSelection, ProviderInstallationSelection, TerminalScalarValue,
    };

    #[derive(Debug, Clone)]
    struct RecordedArtifact(Arc<CanonicalTerminalArtifact>);

    impl Record for RecordedArtifact {
        fn write(&self, output: &mut compiler_storage::RecordWriter) -> Result<(), RecordError> {
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

    #[derive(Debug, Clone, Record)]
    pub struct CanonicalScalarEvaluationReceipt {
        scope: Arc<CanonicalScalarScope>,
        application: ScalarApplicationId,
        values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
        captures: Vec<(ValueId, ScalarType)>,
        outcome: PureTerminalEvaluationOutcome,
        usage: psi_work::WorkUsage,
    }

    impl CanonicalScalarEvaluationReceipt {
        pub(crate) fn new(
            scope: Arc<CanonicalScalarScope>,
            application: ScalarApplicationId,
            values: Vec<(ScalarApplicationId, TerminalScalarValue)>,
            captures: Vec<(ValueId, ScalarType)>,
            outcome: PureTerminalEvaluationOutcome,
            usage: psi_work::WorkUsage,
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
        pub fn usage(&self) -> &psi_work::WorkUsage {
            &self.usage
        }

        pub(crate) fn returned(&self) -> bool {
            matches!(self.outcome, PureTerminalEvaluationOutcome::Returned { .. })
        }

        pub fn validate_charge(&self) -> Result<(), String> {
            if self.usage.schedule() != psi_work::WorkSchedule::CURRENT.identity()
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

/// Only actual canonical-artifact execution contributes an evaluation receipt.
#[derive(Debug, Clone)]
pub enum PureEvaluationRecord {
    CanonicalScalar(CanonicalScalarEvaluationReceipt),
}

impl compiler_storage::Record for PureEvaluationRecord {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| match self {
            Self::CanonicalScalar(receipt) => {
                // Preserve the existing canonical-scalar wire coordinate. Slot
                // zero belonged to retired tree-based evaluation receipts.
                compiler_storage::Record::write(&1_u32, output)?;
                compiler_storage::Record::write(receipt, output)
            }
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(
            |input| match <u32 as compiler_storage::Record>::read(input)? {
                1 => Ok(Self::CanonicalScalar(compiler_storage::Record::read(
                    input,
                )?)),
                0 => Err(input.error("no product route yet: retired tree-evaluation receipt")),
                _ => Err(input.error("unknown PureEvaluationRecord variant")),
            },
        )
    }
}

impl PureEvaluationRecord {
    fn has_complete_usage(&self) -> bool {
        match self {
            Self::CanonicalScalar(receipt) => receipt.returned(),
        }
    }
    fn validate_charge(&self) -> Result<(), String> {
        match self {
            Self::CanonicalScalar(receipt) => receipt.validate_charge(),
        }
    }
}

pub(crate) const PURE_PSI_WORK_BUDGET: u64 = 100_000;

/// A build-time value with the engine-tagged usage record of the evaluation
/// that produced it. Both engines publish `BuildEvaluationUsage`, whose
/// measurement identity names the work and storage schedules, so consumers
/// such as evaluated binding digests never interpret one engine's counts as
/// another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    value: T,
    usage: evaluation_accounting::BuildEvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> evaluation_accounting::BuildEvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, evaluation_accounting::BuildEvaluationUsage) {
        (self.value, self.usage)
    }
}

// A standalone pure evaluation has no package sponsor. Its private account
// enforces the pure work budget; storage is measured, not capped, exactly as
// the checked evaluator measured cells without a ceiling.

// The private sponsor above is not a package session, so the published record
// carries only the invocation's own counts, like an unsponsored checked record.

#[derive(Debug, Clone, compiler_storage::Record)]
pub enum PureTerminalEvaluationOutcome {
    Returned {
        value: TerminalExecutionResult,
    },
    // A Terminal invocation whose result the compiler read back from the
    // completed instance, with the storage measurement of that invocation.
    Materialized {
        value: BuildTimeValue,
        usage: evaluation_accounting::BuildEvaluationUsage,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Default, compiler_storage::Record)]
pub struct SourceCompletionAccounting {
    receipts: Arc<[PureEvaluationRecord]>,
    reused: bool,
}

impl SourceCompletionAccounting {
    pub fn was_reused(&self) -> bool {
        self.reused
    }

    pub fn has_complete_usage(&self) -> bool {
        self.receipts
            .iter()
            .all(PureEvaluationRecord::has_complete_usage)
    }

    pub fn mark_reused(&mut self) {
        self.reused = true;
    }

    pub fn receipts(&self) -> &[PureEvaluationRecord] {
        &self.receipts
    }

    pub fn validate_charges(&self) -> Result<(), String> {
        for receipt in self.receipts() {
            receipt.validate_charge()?;
        }
        Ok(())
    }
}

pub struct RecordedPreCheck {
    pub pending: Option<crate::PreCheckEvaluation>,
    pub accounting: SourceCompletionAccounting,
}

impl SourceCompletionAccounting {
    pub(crate) fn append_scalar_evaluations(
        &mut self,
        records: Vec<CanonicalScalarEvaluationReceipt>,
    ) {
        if records.is_empty() {
            return;
        }
        self.append(Self {
            receipts: records
                .into_iter()
                .map(PureEvaluationRecord::CanonicalScalar)
                .collect::<Vec<_>>()
                .into(),
            reused: false,
        });
    }

    pub fn append(&mut self, other: Self) {
        if self.receipts.is_empty() {
            *self = other;
        } else if !other.receipts.is_empty() {
            let mut receipts = self.receipts.to_vec();
            receipts.extend(other.receipts.iter().cloned());
            self.receipts = receipts.into();
        }
    }
}
