//! Invocation-owned accounting for pure source-completion evaluations.
//!
//! The ordinary admission and interpreter still execute each cold job. Its exact
//! immutable program, request, result and real evaluator usage stay together so
//! retaining a completed source never turns warm work into a zero logical charge.
//! This owner neither grants execution nor debits a Build activation's sponsor.

use std::sync::{Arc, Mutex};

use checked_interpreter::{
    BuildMachineEntry, BuildMachineEvaluationRequest, BuildMachineProgram,
    BuildTimeOperationEvaluation, EvaluationSemanticsIdentity, EvaluationUsage,
};
use typed_trees::TypedTrees;

use crate::{BuildTimeValue, SelectedBuildTimeBinaryOperator, SelectedBuildTimeMachineBody};

/// The actual executable input of a successful pure invocation. Checked inputs
/// retain their facts and separate Product declaration frontier unchanged.
#[derive(Debug, Clone)]
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

/// One root invocation, not one row per interpreted child call. The private
/// constructor keeps request/result association with the measured execution.
#[derive(Debug, Clone)]
pub struct PureEvaluationReceipt {
    program: Arc<PureEvaluationProgram>,
    machine: symbols::SymbolHandle,
    arguments: Vec<BuildTimeValue>,
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    machine_bodies: Vec<SelectedBuildTimeMachineBody>,
    outcome: PureEvaluationOutcome,
    semantics: EvaluationSemanticsIdentity,
}

/// Failed pure evaluation currently returns no measured usage. Retain that
/// absence explicitly; a tolerated probe failure must not refuse its source,
/// and an incompletely measured completion must never become a warm hit.
#[derive(Debug, Clone)]
pub enum PureEvaluationOutcome {
    Returned {
        value: BuildTimeValue,
        usage: EvaluationUsage,
    },
    UnmeasuredFailure {
        reason: String,
    },
}

impl PureEvaluationReceipt {
    pub fn program(&self) -> &PureEvaluationProgram {
        &self.program
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
    pub fn machine_bodies(&self) -> &[SelectedBuildTimeMachineBody] {
        &self.machine_bodies
    }
    pub fn outcome(&self) -> &PureEvaluationOutcome {
        &self.outcome
    }
    pub fn semantics(&self) -> EvaluationSemanticsIdentity {
        self.semantics
    }

    pub fn usage(&self) -> Option<EvaluationUsage> {
        match &self.outcome {
            PureEvaluationOutcome::Returned { usage, .. } => Some(*usage),
            PureEvaluationOutcome::UnmeasuredFailure { .. } => None,
        }
    }

    /// Reapply the recorded invocation's fixed logical ceiling. Pure jobs have
    /// no aggregate Build sponsor; its independent charges are never replayed
    /// or waived here. This is accounting consistency, not renewed admission.
    pub fn validate_charge(&self) -> Result<(), String> {
        if self.semantics != checked_interpreter::CURRENT_EVALUATION_SEMANTICS {
            return Err("source-completion evaluator semantics changed".to_owned());
        }
        if let Some(usage) = self.usage() {
            SelectedBuildTimeMachineBody::validate_all(self.program.typed(), &self.machine_bodies)?;
            if usage.schema() != checked_interpreter::CURRENT_EVALUATION_USAGE_SCHEMA
                || usage.schedule() != checked_interpreter::CURRENT_EVALUATION_STEP_SCHEDULE
                || usage.fuel_ceiling() != checked_interpreter::pure_evaluation_step_ceiling()
            {
                return Err(
                    "source-completion evaluation policy, usage schema or step schedule changed"
                        .to_owned(),
                );
            }
            checked_interpreter::capture_checked_build_evaluation_usage(usage, None).map_err(
                |reason| format!("source-completion evaluation usage is invalid: {reason:?}"),
            )?;
            if self
                .program
                .typed()
                .machine_by_symbol(self.machine)
                .is_none()
            {
                return Err("source-completion evaluation lost its exact machine".to_owned());
            }
        }
        Ok(())
    }
}

/// One root execution in source order, with its engine's own usage vocabulary.
/// A Terminal receipt never manufactures checked-interpreter steps.
#[derive(Debug, Clone)]
pub enum PureEvaluationRecord {
    Checked(PureEvaluationReceipt),
    Terminal(PureTerminalEvaluationReceipt),
}

impl PureEvaluationRecord {
    fn has_complete_usage(&self) -> bool {
        match self {
            Self::Checked(receipt) => receipt.usage().is_some(),
            Self::Terminal(receipt) => {
                receipt.program.is_some()
                    && matches!(
                        &receipt.outcome,
                        PureTerminalEvaluationOutcome::Returned { .. }
                    )
            }
        }
    }

    fn validate_charge(&self) -> Result<(), String> {
        match self {
            Self::Checked(receipt) => receipt.validate_charge(),
            Self::Terminal(receipt) => receipt.validate_charge(),
        }
    }
}

/// The ordinary constant route's fixed Terminal semantic allowance. This is
/// deliberately separate from the checked interpreter's step schedule.
const PURE_TERMINAL_FUEL_CEILING: u64 = 100_000;

/// One Terminal execution with the exact checked source, produced artifact,
/// selected numeric imports, result and meter actually used by that execution.
/// A failed production may have no artifact; it never becomes a warm hit.
#[derive(Debug, Clone)]
pub struct PureTerminalEvaluationReceipt {
    source_program: Arc<PureEvaluationProgram>,
    machine: symbols::SymbolHandle,
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    program: Option<Arc<psi_terminal_production::ProducedTerminalProgram>>,
    outcome: PureTerminalEvaluationOutcome,
    usage: terminal_fuel::TerminalFuelUsage,
    fuel_ceiling: u64,
}

#[derive(Debug, Clone)]
pub enum PureTerminalEvaluationOutcome {
    Returned {
        value: terminal_interpreter::TerminalExecutionResult,
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

    pub fn operators(&self) -> &[SelectedBuildTimeBinaryOperator] {
        &self.operators
    }

    pub fn program(&self) -> Option<&psi_terminal_production::ProducedTerminalProgram> {
        self.program.as_deref()
    }

    pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
        &self.outcome
    }

    pub fn usage(&self) -> &terminal_fuel::TerminalFuelUsage {
        &self.usage
    }

    pub fn fuel_ceiling(&self) -> u64 {
        self.fuel_ceiling
    }

    pub fn validate_charge(&self) -> Result<(), String> {
        if self.usage.schedule() != terminal_fuel::TerminalFuelSchedule::CURRENT.identity()
            || self.fuel_ceiling != PURE_TERMINAL_FUEL_CEILING
            || self.usage.total_units() > self.fuel_ceiling
        {
            return Err("source-completion Terminal fuel policy or schedule changed".to_owned());
        }
        let attributed = self
            .usage
            .attribution()
            .values()
            .try_fold(0_u64, |total, row| total.checked_add(row.units()));
        if attributed != Some(self.usage.total_units()) {
            return Err("source-completion Terminal fuel attribution is inconsistent".to_owned());
        }
        if matches!(
            &self.outcome,
            PureTerminalEvaluationOutcome::Returned { .. }
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

/// Immutable charge ledger for one successful completion. Reuse preserves
/// source order, multiplicity, schema and ceiling rather than adding nested
/// interpreter counts a second time or reporting a fresh execution.
#[derive(Debug, Clone, Default)]
pub struct SourceCompletionAccounting {
    receipts: Arc<[PureEvaluationRecord]>,
    reused: bool,
}

impl SourceCompletionAccounting {
    /// Exact inherited ledger identity for invocation-local completion reuse.
    /// Equal typed input cannot substitute another execution's charges.
    pub fn same_receipts(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.receipts, &other.receipts) && self.reused == other.reused
    }

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

/// Explicit scratch owner passed by the pending-work domain. Ordinary callers
/// that do not retain completion use the disabled default, with no snapshots.
#[derive(Clone, Default)]
pub(crate) struct EvaluationAccounting {
    state: Option<Arc<Mutex<EvaluationAccountingState>>>,
}

#[derive(Default)]
struct EvaluationAccountingState {
    /// One immutable custody owner per exact execution-program version. Full
    /// content comparison precedes sharing, including all checked facts and
    /// Product frontier custody; mutable addresses and names are not keys.
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

    /// The production callback retains the artifact before entering its
    /// instance. Accounting owns the finite meter, including real partial work
    /// on failures; only a normal result with that exact input is reusable.
    pub(crate) fn evaluate_terminal(
        &self,
        program: BuildMachineProgram<'_>,
        machine: symbols::SymbolHandle,
        operators: &[SelectedBuildTimeBinaryOperator],
        execute: impl FnOnce(
            &mut terminal_fuel::TerminalFuelMeter,
            &mut Option<Arc<psi_terminal_production::ProducedTerminalProgram>>,
        ) -> Result<terminal_interpreter::TerminalExecutionResult, String>,
    ) -> Result<terminal_interpreter::TerminalExecutionResult, String> {
        if !matches!(
            program,
            BuildMachineProgram::Checked(_) | BuildMachineProgram::CheckedClosure { .. }
        ) {
            return Err("Terminal source-completion accounting requires checked input".to_owned());
        }
        let input = self.reserve_invocation(program)?;
        let operators = input.as_ref().map(|_| operators.to_vec());
        let mut meter =
            terminal_fuel::TerminalFuelMeter::with_allowance(PURE_TERMINAL_FUEL_CEILING);
        let mut artifact = None;
        let evaluation = execute(&mut meter, &mut artifact);
        if let (Some(state), Some((ordinal, source_program)), Some(operators)) =
            (&self.state, input, operators)
        {
            let receipt = PureTerminalEvaluationReceipt {
                source_program,
                machine,
                operators,
                program: artifact,
                outcome: match &evaluation {
                    Ok(value) => PureTerminalEvaluationOutcome::Returned {
                        value: value.clone(),
                    },
                    Err(reason) => PureTerminalEvaluationOutcome::Failed {
                        reason: reason.clone(),
                    },
                },
                usage: meter.into_usage(),
                fuel_ceiling: PURE_TERMINAL_FUEL_CEILING,
            };
            receipt.validate_charge()?;
            state
                .lock()
                .map_err(|_| "source-completion accounting owner is poisoned".to_owned())?
                .receipts[ordinal] = Some(PureEvaluationRecord::Terminal(receipt));
        }
        evaluation
    }

    pub(crate) fn evaluate(
        &self,
        program: BuildMachineProgram<'_>,
        request: BuildMachineEvaluationRequest<'_>,
    ) -> Result<BuildTimeOperationEvaluation<BuildTimeValue>, String> {
        // Snapshot before execution. Neither the retained result nor a later
        // mutation of the caller's tree can redefine the measured input.
        let input = self
            .reserve_invocation(program)?
            .map(|(ordinal, retained)| {
                (
                    ordinal,
                    retained,
                    request.entry,
                    request.arguments.clone(),
                    request.operators.to_vec(),
                    request.machine_bodies.to_vec(),
                )
            });
        let evaluation =
            checked_interpreter::evaluate_prepared_build_time_machine(program, request);
        if let (
            Some(receipts),
            Some((ordinal, program, entry, arguments, operators, machine_bodies)),
        ) = (&self.state, input)
        {
            let machine = match entry {
                BuildMachineEntry::Symbol(symbol) => symbol,
                BuildMachineEntry::Name(name) => program
                    .typed()
                    .realized_machine_named(name)
                    .map(|machine| machine.symbol)
                    .unwrap_or_default(),
            };
            let receipt = PureEvaluationReceipt {
                program,
                machine,
                arguments,
                operators,
                machine_bodies,
                outcome: match &evaluation {
                    Ok(evaluation) => PureEvaluationOutcome::Returned {
                        value: evaluation.value().clone(),
                        usage: evaluation.usage(),
                    },
                    Err(reason) => PureEvaluationOutcome::UnmeasuredFailure {
                        reason: reason.clone(),
                    },
                },
                semantics: checked_interpreter::CURRENT_EVALUATION_SEMANTICS,
            };
            receipt.validate_charge()?;
            receipts
                .lock()
                .map_err(|_| "source-completion accounting owner is poisoned".to_owned())?
                .receipts[ordinal] = Some(PureEvaluationRecord::Checked(receipt));
        }
        evaluation
    }
}

/// A complete precheck's pending work and its real pure evaluation charges.
/// The driver combines cohorts in source order before retaining completion.
pub struct RecordedPreCheck {
    pub pending: Option<crate::PreCheckEvaluation>,
    pub accounting: SourceCompletionAccounting,
}

impl SourceCompletionAccounting {
    pub fn append(&mut self, other: Self) {
        let reused = self.reused || other.reused;
        if self.receipts.is_empty() {
            *self = other;
        } else if !other.receipts.is_empty() {
            let mut receipts = self.receipts.to_vec();
            receipts.extend(other.receipts.iter().cloned());
            self.receipts = receipts.into();
        }
        self.reused = reused;
    }
}
