/*
    Compile-time evaluation receipts, work and cell usage accounting, and
    canonical scalar evaluation records.

    We record per-invocation step and cell usage across pre-resolution and
    pre-check evaluation rounds and bind canonical scalar evaluation receipts to
    their verified Terminal execution outcomes.
*/

use std::sync::{Arc, Mutex};

use crate::evaluation_requests::{
    BuildMachineEvaluationError, BuildMachineEvaluationRequest, BuildMachineProgram,
    BuildTimeOperationEvaluation,
};
use typed_trees::TypedTrees;

use crate::{BuildTimeValue, SelectedBuildTimeBinaryOperator};
use compiler_storage::{RecordError, RecordReader};
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

#[derive(Debug, Clone, compiler_storage::Record)]
pub enum PureEvaluationProgram {
    Typed(Arc<TypedTrees>),
    Checked(Arc<checked_trees::CheckedTrees>),
    CheckedClosure {
        execution: Arc<checked_trees::CheckedTrees>,
        product_declarations: Arc<TypedTrees>,
    },
}

impl PureEvaluationProgram {
    fn retain(program: BuildMachineProgram<'_>) -> Self {
        match program {
            BuildMachineProgram::Typed(typed) => Self::Typed(Arc::new(typed.clone())),
            BuildMachineProgram::Checked(checked) => Self::Checked(Arc::new(checked.clone())),
            BuildMachineProgram::CheckedClosure {
                execution,
                product_declarations,
            } => Self::CheckedClosure {
                execution: Arc::new(execution.clone()),
                product_declarations: Arc::new(product_declarations.clone()),
            },
        }
    }

    fn matches(&self, program: BuildMachineProgram<'_>) -> bool {
        match (self, program) {
            (Self::Typed(retained), BuildMachineProgram::Typed(current)) => {
                retained.as_ref() == current
            }
            (Self::Checked(retained), BuildMachineProgram::Checked(current)) => {
                retained.as_ref() == current
            }
            (
                Self::CheckedClosure {
                    execution,
                    product_declarations,
                },
                BuildMachineProgram::CheckedClosure {
                    execution: current,
                    product_declarations: current_product,
                },
            ) => execution.as_ref() == current && product_declarations.as_ref() == current_product,
            _ => false,
        }
    }

    fn typed(&self) -> &TypedTrees {
        match self {
            Self::Typed(typed) => typed,
            Self::Checked(checked) => &checked.typed,
            Self::CheckedClosure { execution, .. } => &execution.typed,
        }
    }
}

#[derive(Debug, Clone, compiler_storage::Record)]
pub enum PureEvaluationRecord {
    Terminal(PureTerminalEvaluationReceipt),
    CanonicalScalar(CanonicalScalarEvaluationReceipt),
}

impl PureEvaluationRecord {
    fn has_complete_usage(&self) -> bool {
        match self {
            Self::CanonicalScalar(receipt) => receipt.returned(),
            Self::Terminal(receipt) => {
                receipt.program.is_some()
                    && matches!(
                        &receipt.outcome,
                        PureTerminalEvaluationOutcome::Returned { .. }
                            | PureTerminalEvaluationOutcome::Materialized { .. }
                    )
            }
        }
    }

    fn validate_charge(&self) -> Result<(), String> {
        match self {
            Self::Terminal(receipt) => receipt.validate_charge(),
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
    pub(crate) fn new(value: T, usage: evaluation_accounting::BuildEvaluationUsage) -> Self {
        Self { value, usage }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> evaluation_accounting::BuildEvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub(crate) fn map_value(self, map: impl FnOnce(T) -> T) -> Self {
        Self {
            value: map(self.value),
            usage: self.usage,
        }
    }

    pub fn into_parts(self) -> (T, evaluation_accounting::BuildEvaluationUsage) {
        (self.value, self.usage)
    }
}

// A standalone pure evaluation has no package sponsor. Its private account
// enforces the pure work budget; storage is measured, not capped, exactly as
// the checked evaluator measured cells without a ceiling.
fn pure_terminal_invocation_account()
-> Result<evaluation_accounting::EvaluationInvocationAccount, String> {
    let limits = evaluation_accounting::BuildEvaluationSponsorLimits::new(
        PURE_PSI_WORK_BUDGET,
        1,
        1,
        1,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    )
    .ok_or("pure Terminal evaluation limits are invalid")?;
    let measurement = evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
    };
    evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(limits, measurement)
        .map_err(|error| format!("pure Terminal evaluation sponsor: {error:?}"))?
        .begin_invocation(measurement, PURE_PSI_WORK_BUDGET)
        .map_err(|error| format!("pure Terminal evaluation account: {error:?}"))
}

// The private sponsor above is not a package session, so the published record
// carries only the invocation's own counts, like an unsponsored checked record.
fn unsponsored(
    usage: evaluation_accounting::BuildEvaluationUsage,
) -> evaluation_accounting::BuildEvaluationUsage {
    evaluation_accounting::BuildEvaluationUsage {
        sponsor_schema_version: None,
        session_work_budget: None,
        session_build_log_byte_ceiling: None,
        session_filesystem_attempt_ceiling: None,
        session_live_filesystem_handle_ceiling: None,
        session_live_storage_cell_ceiling: None,
        session_live_payload_byte_ceiling: None,
        session_result_storage_cell_ceiling: None,
        session_result_payload_byte_ceiling: None,
        session_peak_live_filesystem_handles: 0,
        session_peak_live_storage_cells: 0,
        session_peak_live_payload_bytes: 0,
        ..usage
    }
}

#[derive(Debug, Clone, compiler_storage::Record)]
pub struct PureTerminalEvaluationReceipt {
    source_program: Arc<PureEvaluationProgram>,
    machine: symbols::SymbolHandle,
    arguments: Vec<BuildTimeValue>,
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    program: Option<Arc<psi_terminal_production::ProducedTerminalProgram>>,
    outcome: PureTerminalEvaluationOutcome,
    usage: psi_work::WorkUsage,
    work_budget: u64,
}

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

impl PureTerminalEvaluationReceipt {
    pub fn source_program(&self) -> &PureEvaluationProgram {
        &self.source_program
    }

    pub fn machine(&self) -> symbols::SymbolHandle {
        self.machine
    }

    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }

    pub fn operators(&self) -> &[SelectedBuildTimeBinaryOperator] {
        &self.operators
    }

    pub fn program(&self) -> Option<&psi_terminal_production::ProducedTerminalProgram> {
        self.program.as_deref()
    }

    pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
        &self.outcome
    }

    pub fn usage(&self) -> &psi_work::WorkUsage {
        &self.usage
    }

    pub fn work_budget(&self) -> u64 {
        self.work_budget
    }

    pub fn validate_charge(&self) -> Result<(), String> {
        if self.usage.schedule() != psi_work::WorkSchedule::CURRENT.identity()
            || self.work_budget != PURE_PSI_WORK_BUDGET
            || self.usage.total_units() > self.work_budget
        {
            return Err("source-completion Terminal work policy or schedule changed".to_owned());
        }
        let attributed = self
            .usage
            .attribution()
            .values()
            .try_fold(0_u64, |total, row| total.checked_add(row.units()));
        if attributed != Some(self.usage.total_units()) {
            return Err("source-completion Terminal work attribution is inconsistent".to_owned());
        }
        if let PureTerminalEvaluationOutcome::Materialized { usage, .. } = &self.outcome {
            usage.validate().map_err(|reason| {
                format!("source-completion Terminal storage usage is invalid: {reason:?}")
            })?;
            if usage.work_units != self.usage.total_units() {
                return Err("source-completion Terminal work and storage records differ".into());
            }
        }
        if matches!(
            &self.outcome,
            PureTerminalEvaluationOutcome::Returned { .. }
                | PureTerminalEvaluationOutcome::Materialized { .. }
        ) {
            crate::validate_selected_operators(self.source_program.typed(), &self.operators)?;
            if self.program.is_none() {
                return Err(
                    "source-completion Terminal execution lost its exact artifact".to_owned(),
                );
            }
            if self
                .source_program
                .typed()
                .machine_by_symbol(self.machine)
                .is_none()
            {
                return Err(
                    "source-completion Terminal execution lost its exact machine".to_owned(),
                );
            }
        }
        Ok(())
    }
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

#[derive(Clone, Default)]
pub(crate) struct EvaluationAccounting {
    state: Option<Arc<Mutex<EvaluationAccountingState>>>,
}

#[derive(Default)]
struct EvaluationAccountingState {
    programs: Vec<Arc<PureEvaluationProgram>>,
    receipts: Vec<Option<PureEvaluationRecord>>,
}

impl EvaluationAccounting {
    pub(crate) fn recording() -> Self {
        Self {
            state: Some(Arc::new(Mutex::new(EvaluationAccountingState::default()))),
        }
    }

    pub(crate) fn finish(&self) -> Result<SourceCompletionAccounting, String> {
        let receipts = match &self.state {
            None => Vec::new(),
            Some(receipts) => receipts
                .lock()
                .map_err(|_| "source-completion accounting owner is poisoned".to_owned())?
                .receipts
                .iter()
                .cloned()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    "source-completion accounting has an unfinished invocation".to_owned()
                })?,
        };
        let accounting = SourceCompletionAccounting {
            receipts: receipts.into(),
            reused: false,
        };
        accounting.validate_charges()?;
        Ok(accounting)
    }

    fn reserve_invocation(
        &self,
        program: BuildMachineProgram<'_>,
    ) -> Result<Option<(usize, Arc<PureEvaluationProgram>)>, String> {
        let Some(state) = &self.state else {
            return Ok(None);
        };
        let mut state = state
            .lock()
            .map_err(|_| "source-completion accounting owner is poisoned".to_owned())?;
        let retained = match state
            .programs
            .iter()
            .find(|retained| retained.matches(program))
        {
            Some(retained) => Arc::clone(retained),
            None => {
                let retained = Arc::new(PureEvaluationProgram::retain(program));
                state.programs.push(Arc::clone(&retained));
                retained
            }
        };
        let ordinal = state.receipts.len();
        state.receipts.push(None);
        Ok(Some((ordinal, retained)))
    }

    /// Run one admitted checked invocation as a Terminal program and record the
    /// exact arguments, artifact, materialized value, work and storage usage.
    /// The caller's `execute` owns production, instantiation and readback; it
    /// pays every charge through `meter`, whose invocation account supplies the
    /// storage measurement returned with the value.
    pub(crate) fn evaluate_terminal_invocation(
        &self,
        program: BuildMachineProgram<'_>,
        machine: symbols::SymbolHandle,
        arguments: &[BuildTimeValue],
        operators: &[SelectedBuildTimeBinaryOperator],
        execute: impl FnOnce(
            &mut psi_work::WorkMeter,
            &mut Option<Arc<psi_terminal_production::ProducedTerminalProgram>>,
        ) -> Result<BuildTimeValue, String>,
    ) -> Result<MeasuredEvaluation<BuildTimeValue>, String> {
        if !matches!(
            program,
            BuildMachineProgram::Checked(_) | BuildMachineProgram::CheckedClosure { .. }
        ) {
            return Err("Terminal source-completion accounting requires checked input".to_owned());
        }
        let account = pure_terminal_invocation_account()?;
        let input = self.reserve_invocation(program)?;
        let recorded = input
            .as_ref()
            .map(|_| (arguments.to_vec(), operators.to_vec()));
        let mut meter = psi_work::WorkMeter::with_invocation(account.clone(), None)
            .map_err(|error| format!("pure Terminal work meter: {error:?}"))?;
        let mut artifact = None;
        let evaluation = execute(&mut meter, &mut artifact).and_then(|value| {
            let usage = account
                .snapshot()
                .map_err(|error| format!("pure Terminal usage: {error:?}"))?;
            Ok(MeasuredEvaluation::new(value, unsponsored(*usage.record())))
        });
        let work = meter.into_usage();
        if let (Some(state), Some((ordinal, source_program)), Some((arguments, operators))) =
            (&self.state, input, recorded)
        {
            let receipt = PureTerminalEvaluationReceipt {
                source_program,
                machine,
                arguments,
                operators,
                program: artifact,
                outcome: match &evaluation {
                    Ok(measured) => PureTerminalEvaluationOutcome::Materialized {
                        value: measured.value().clone(),
                        usage: measured.usage(),
                    },
                    Err(reason) => PureTerminalEvaluationOutcome::Failed {
                        reason: reason.clone(),
                    },
                },
                usage: work,
                work_budget: PURE_PSI_WORK_BUDGET,
            };
            receipt.validate_charge()?;
            state
                .lock()
                .map_err(|_| "source-completion accounting owner is poisoned".to_owned())?
                .receipts[ordinal] = Some(PureEvaluationRecord::Terminal(receipt));
        }
        evaluation
    }

    // The seam for a machine that runs before the program is checked. The tree
    // interpreter that served it is removed and no build-time job produces a
    // Terminal program from trees that are not checked yet, so every request is
    // refused; none is skipped and none is answered with a default value.
    pub(crate) fn evaluate(
        &self,
        _program: BuildMachineProgram<'_>,
        _request: BuildMachineEvaluationRequest<'_>,
        job: &str,
    ) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, BuildMachineEvaluationError> {
        Err(crate::evaluation_requests::no_typed_tree_engine(job))
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

impl compiler_storage::Record for EvaluationAccounting {
    fn write(&self, output: &mut compiler_storage::RecordWriter) -> Result<(), RecordError> {
        if self.state.is_some() {
            return Err(
                output.error("source continuation still owns an active accounting collector")
            );
        }
        Ok(())
    }
    fn read<Context>(_: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Ok(Self::default())
    }
}
