//! Keep source completion costs joined to the published invocation and its typed caller.
#![forbid(unsafe_code)]

use std::{collections::BTreeMap, sync::Arc};

use build_model::values::BuildTimeValue;
use checked_program_to_psi::{CheckedSourceCall, ProducedTerminalProgram, SourceInvocationSite};
use typed_trees::source_state;

use build_time_evaluation::scalars::PURE_PSI_WORK_BUDGET;
pub use build_time_evaluation::scalars::{
    CanonicalScalarEvaluationReceipt, PureTerminalEvaluationOutcome,
};

/// Only actual canonical-artifact execution contributes an evaluation receipt.
#[derive(Debug, Clone)]
pub enum PureEvaluationRecord {
    CanonicalScalar(Arc<CanonicalScalarEvaluationReceipt>),
    Invocation(Arc<PureTerminalEvaluationReceipt>),
}

impl PureEvaluationRecord {
    // A transient ordering of actual immutable occurrences, never a semantic
    // cache key. Output order remains the incoming account's own order.
    fn occurrence(&self) -> (u8, usize) {
        match self {
            Self::CanonicalScalar(receipt) => (0, Arc::as_ptr(receipt) as usize),
            Self::Invocation(receipt) => (1, Arc::as_ptr(receipt) as usize),
        }
    }
    fn has_complete_usage(&self) -> bool {
        match self {
            Self::CanonicalScalar(receipt) => receipt.returned(),
            Self::Invocation(receipt) => {
                matches!(
                    receipt.outcome,
                    PureTerminalEvaluationOutcome::Materialized { .. }
                )
            }
        }
    }
    fn validate_charge(&self) -> Result<(), String> {
        match self {
            Self::CanonicalScalar(receipt) => receipt.validate_charge(),
            Self::Invocation(receipt) => receipt.validate_charge(),
        }
    }
}

/// A completed invocation keeps its checked source site and execution charge.
#[derive(Debug, Clone)]
pub struct PureTerminalEvaluationReceipt {
    site: CheckedSourceCall,
    requested_by_constant: bool,
    arguments: Arc<[BuildTimeValue]>,
    inputs: Arc<[BuildTimeValue]>,
    admission: build_model::admission::AdmittedCompilerEvaluation,
    execution: Option<build_time_evaluation::invocations::InvocationReceipt>,
    account: build_model::accounting::EvaluationInvocationAccount,
    outcome: PureTerminalEvaluationOutcome,
    usage: terminal_work::WorkUsage,
}

/// The source site and both argument lists used to set up this invocation.
pub struct SourceInvocationInputs<'a> {
    pub site: SourceInvocationSite<'a>,
    pub arguments: &'a Arc<[BuildTimeValue]>,
    pub inputs: &'a Arc<[BuildTimeValue]>,
}

/// The interpreter's result and the evidence of the work it actually performed.
pub struct RecordedSourceInvocation {
    value: Result<build_time_evaluation::invocations::InvocationValue, String>,
    receipt: PureTerminalEvaluationReceipt,
    constant_input: Option<Arc<source_state::TypedTrees>>,
}

impl RecordedSourceInvocation {
    pub fn record(
        call: SourceInvocationInputs<'_>,
        program: &ProducedTerminalProgram,
        admission: build_model::admission::AdmittedCompilerEvaluation,
        meter: &build_model::work::EvaluationWorkMeter,
        run: Result<build_time_evaluation::invocations::InvocationOutcome, String>,
    ) -> Result<Self, String> {
        let checked = call.site.check(program)?;
        let account = meter
            .payment()
            .invocation()
            .ok_or("source invocation requires its actual sponsor")?
            .clone();
        let usage = meter.usage().clone();
        let (value, execution) = match run {
            Ok(run) => {
                let (value, receipt) = run.into_parts();
                (value, Some(receipt))
            }
            Err(reason) => (Err(reason), None),
        };
        let outcome = match &value {
            Ok((value, _)) => {
                let snapshot = account
                    .snapshot()
                    .map_err(|error| format!("source invocation usage: {error:?}"))?;
                PureTerminalEvaluationOutcome::Materialized {
                    value: value.clone(),
                    usage: *snapshot.record(),
                }
            }
            Err(reason) => PureTerminalEvaluationOutcome::Failed {
                reason: reason.clone(),
            },
        };
        let constant_source = call
            .site
            .request_source
            .const_declaration_by_symbol(call.site.requester)
            .map(|_| Arc::clone(call.site.request_source));
        let receipt = PureTerminalEvaluationReceipt {
            site: checked,
            requested_by_constant: constant_source.is_some(),
            arguments: Arc::clone(call.arguments),
            inputs: Arc::clone(call.inputs),
            admission,
            execution,
            account,
            outcome,
            usage,
        };
        receipt.require_program(program)?;
        receipt.validate_charge()?;
        Ok(Self {
            value,
            receipt,
            constant_input: constant_source,
        })
    }

    /// Keep the work that ran even when its caller refuses the returned observations.
    pub fn finish<T>(
        mut self,
        accounting: &mut SourceCompletionAccounting,
        complete: impl FnOnce(build_time_evaluation::invocations::InvocationValue) -> Result<T, String>,
    ) -> Result<T, String> {
        let result = self.value.and_then(complete);
        if let Err(reason) = &result {
            self.receipt.outcome = PureTerminalEvaluationOutcome::Failed {
                reason: reason.clone(),
            };
        }
        if let Some(source) = self.constant_input {
            Arc::make_mut(&mut accounting.constant_inputs)
                .insert(Arc::as_ptr(&source) as usize, source);
        }
        Arc::make_mut(&mut accounting.receipts)
            .push(PureEvaluationRecord::Invocation(Arc::new(self.receipt)));
        result
    }
}

impl PureTerminalEvaluationReceipt {
    pub const fn machine(&self) -> symbols::SymbolHandle {
        self.site.machine()
    }
    pub const fn requester(&self) -> symbols::SymbolHandle {
        self.site.requester()
    }
    pub const fn caller(&self) -> symbols::SymbolHandle {
        self.site.caller()
    }
    pub const fn machine_template(&self) -> Option<symbols::SymbolHandle> {
        self.site.machine_template()
    }
    pub const fn emitted_site(
        &self,
    ) -> Option<(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::InvocationSite,
    )> {
        self.site.emitted_site()
    }
    pub fn require_site(&self, current: &SourceInvocationInputs<'_>) -> Result<(), String> {
        self.site.require_site(&current.site)?;
        if !Arc::ptr_eq(&self.arguments, current.arguments)
            || !Arc::ptr_eq(&self.inputs, current.inputs)
        {
            return Err("evaluation receipt belongs to another call's inputs".into());
        }
        Ok(())
    }
    pub fn require_program(&self, program: &ProducedTerminalProgram) -> Result<(), String> {
        let code = program.view();
        let entry = code
            .module()
            .entry
            .ok_or("source invocation receipt lost its entry")?;
        self.admission
            .require_invocation(code, self.admission.target(), entry)?;
        if let Some(execution) = &self.execution {
            execution.require_admission(code, &self.admission)?;
        }
        Ok(())
    }
    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }
    pub fn admission(&self) -> &build_model::admission::AdmittedCompilerEvaluation {
        &self.admission
    }
    pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
        &self.outcome
    }
    pub fn usage(&self) -> &terminal_work::WorkUsage {
        &self.usage
    }

    pub fn validate_charge(&self) -> Result<(), String> {
        if let Some(execution) = &self.execution {
            execution.require_inputs(&self.arguments, &self.inputs, Some(&self.account))?;
            if execution.usage() != &self.usage {
                return Err(
                    "source invocation receipt changed its actual execution charges".into(),
                );
            }
        } else if matches!(
            self.outcome,
            PureTerminalEvaluationOutcome::Materialized { .. }
        ) {
            return Err("source invocation result has no actual artifact execution receipt".into());
        }
        if self.usage.schedule() != terminal_work::WorkSchedule::CURRENT.identity()
            || self.usage.total_units() > PURE_PSI_WORK_BUDGET
        {
            return Err("source invocation work policy changed".into());
        }
        let total = self
            .usage
            .attribution()
            .values()
            .try_fold(0_u64, |total, row| total.checked_add(row.units()));
        if total != Some(self.usage.total_units()) {
            return Err("source invocation attribution is inconsistent".into());
        }
        match &self.outcome {
            PureTerminalEvaluationOutcome::Materialized { usage, .. } => {
                usage
                    .validate()
                    .map_err(|error| format!("source invocation storage usage: {error:?}"))?;
                if usage.work_units != self.usage.total_units() {
                    return Err("source invocation work and storage measurements differ".into());
                }
            }
            PureTerminalEvaluationOutcome::Failed { .. } => {}
            PureTerminalEvaluationOutcome::Returned { .. } => {
                return Err(
                    "source invocation requires its materialized result measurement".into(),
                );
            }
        }
        Ok(())
    }
}

/// Actual costs incurred before preparation refused its request.
#[derive(Debug)]
pub struct SourcePreparationFailure {
    pub diagnostics: Vec<diagnostics::Diagnostic>,
    pub accounting: SourceCompletionAccounting,
}

#[derive(Debug, Clone, Default)]
pub struct SourceCompletionAccounting {
    receipts: Arc<Vec<PureEvaluationRecord>>,
    // Materializing a named constant must also check the original expressions.
    // Keep those inputs only until the source preparation finishes.
    constant_inputs: Arc<BTreeMap<usize, Arc<source_state::TypedTrees>>>,
}

impl SourceCompletionAccounting {
    /// Constant materialization uses the exact source of the checked call. The
    /// pointer is local to this account and the map keeps that allocation alive.
    pub fn constant_input(
        &self,
        receipt: &PureTerminalEvaluationReceipt,
    ) -> Result<Option<&Arc<source_state::TypedTrees>>, String> {
        if !receipt.requested_by_constant {
            return Ok(None);
        }
        self.constant_inputs
            .get(&(receipt.site.requesting_source().as_ptr() as usize))
            .map(Some)
            .ok_or_else(|| "constant validation requires its original source input".into())
    }

    /// Once preparation returns its result or diagnostics, only the charges travel on.
    pub fn release_constant_inputs(&mut self) {
        if !self.constant_inputs.is_empty() {
            self.constant_inputs = Arc::default();
        }
    }

    pub fn has_complete_usage(&self) -> bool {
        self.receipts
            .iter()
            .all(PureEvaluationRecord::has_complete_usage)
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

impl Extend<CanonicalScalarEvaluationReceipt> for SourceCompletionAccounting {
    fn extend<I: IntoIterator<Item = CanonicalScalarEvaluationReceipt>>(&mut self, receipts: I) {
        let mut receipts = receipts.into_iter().peekable();
        if receipts.peek().is_some() {
            Arc::make_mut(&mut self.receipts).extend(
                receipts.map(|receipt| PureEvaluationRecord::CanonicalScalar(Arc::new(receipt))),
            );
        }
    }
}

impl SourceCompletionAccounting {
    pub fn append(&mut self, other: Self) {
        if self.constant_inputs.is_empty() {
            self.constant_inputs = other.constant_inputs;
        } else if !other.constant_inputs.is_empty()
            && !Arc::ptr_eq(&self.constant_inputs, &other.constant_inputs)
        {
            Arc::make_mut(&mut self.constant_inputs).extend(
                other
                    .constant_inputs
                    .iter()
                    .map(|(key, source)| (*key, Arc::clone(source))),
            );
        }
        if self.receipts.is_empty() {
            self.receipts = other.receipts;
        } else if !other.receipts.is_empty() && !Arc::ptr_eq(&self.receipts, &other.receipts) {
            // Prepare membership over these actual occurrences, not the
            // universe of possible IDs. None sorts before any incoming
            // position, so an existing occurrence always wins the union.
            let mut occurrences = self
                .receipts
                .iter()
                .map(|receipt| (receipt.occurrence(), None))
                .chain(
                    other
                        .receipts
                        .iter()
                        .enumerate()
                        .map(|(position, receipt)| (receipt.occurrence(), Some(position))),
                )
                .collect::<Vec<_>>();
            occurrences.sort_unstable();
            occurrences.dedup_by_key(|(occurrence, _)| *occurrence);
            let mut incoming = vec![false; other.receipts.len()];
            for (_, position) in occurrences {
                if let Some(position) = position {
                    incoming[position] = true;
                }
            }
            if incoming.iter().any(|include| *include) {
                Arc::make_mut(&mut self.receipts).extend(
                    other
                        .receipts
                        .iter()
                        .zip(incoming)
                        .filter(|(_, include)| *include)
                        .map(|(receipt, _)| receipt.clone()),
                );
            }
        }
    }
}
