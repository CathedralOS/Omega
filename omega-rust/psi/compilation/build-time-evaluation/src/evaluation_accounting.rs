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
    Invocation(PureTerminalEvaluationReceipt),
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
            Self::Invocation(_) => Err(output.error(
                "invocation-local evaluation receipts cannot be saved as portable evidence",
            )),
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

/// A temporary caller can establish a real invocation without becoming a
/// portable program. Retain its source and code together with the actual
/// arguments and charges; saving this receipt remains explicitly unsupported.
#[derive(Debug, Clone)]
pub struct PureTerminalEvaluationReceipt {
    source: Arc<typed_trees::FrozenTypedTrees>,
    machine: symbols::SymbolHandle,
    requester: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    call: typed_trees::expression::ExpressionHandle,
    arguments: Vec<BuildTimeValue>,
    program: Arc<psi_terminal_production::ProducedTerminalProgram>,
    admission: terminal_installation::AdmittedCompilerEvaluation,
    outcome: PureTerminalEvaluationOutcome,
    usage: psi_work::WorkUsage,
}

impl PureTerminalEvaluationReceipt {
    pub(crate) fn new(
        accepted: &psi_terminal_production::AcceptedTerminalSource,
        call: typed_trees::expression::ExpressionHandle,
        entry: &crate::PreparedBuildMachineEntry,
        arguments: &[BuildTimeValue],
        program: Arc<psi_terminal_production::ProducedTerminalProgram>,
        admission: terminal_installation::AdmittedCompilerEvaluation,
        outcome: PureTerminalEvaluationOutcome,
        usage: psi_work::WorkUsage,
    ) -> Result<Self, String> {
        let machine = entry.symbol();
        let requester = entry.requester();
        let entry = accepted
            .entry()
            .ok_or("evaluation receipt lost its accepted entry")?;
        let caller = *accepted
            .closure()
            .elaboration
            .machines
            .get(entry)
            .ok_or("evaluation receipt lost its actual caller")?;
        if !accepted.is_invocation()
            || &accepted.closure().elaboration.module != program.view().module()
        {
            return Err("evaluation receipt has another accepted code owner".into());
        }
        let receipt = Self {
            source: Arc::clone(accepted.source()),
            machine,
            requester,
            caller,
            call,
            arguments: arguments.to_vec(),
            program,
            admission,
            outcome,
            usage,
        };
        receipt.validate_charge()?;
        Ok(receipt)
    }

    pub fn source(&self) -> &typed_trees::FrozenTypedTrees {
        &self.source
    }
    pub const fn machine(&self) -> symbols::SymbolHandle {
        self.machine
    }
    pub const fn requester(&self) -> symbols::SymbolHandle {
        self.requester
    }
    pub const fn caller(&self) -> symbols::SymbolHandle {
        self.caller
    }
    /// The actual generated call belongs to this receipt's immutable source.
    /// Consumers inspect its selected operand here, not a separately supplied
    /// type handle or a later search through the structural catalogue.
    pub fn call(&self) -> Result<&typed_trees::expression::TableCallExpression, String> {
        if !self.source.expression_table.expression_is_valid(self.call) {
            return Err("source invocation call has another source owner".into());
        }
        self.source
            .expression_table
            .call_node(self.call)
            .ok_or_else(|| "source invocation lost its retained call".into())
    }
    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }
    pub fn program(&self) -> &psi_terminal_production::ProducedTerminalProgram {
        &self.program
    }
    pub fn admission(&self) -> &terminal_installation::AdmittedCompilerEvaluation {
        &self.admission
    }
    pub fn outcome(&self) -> &PureTerminalEvaluationOutcome {
        &self.outcome
    }
    pub fn usage(&self) -> &psi_work::WorkUsage {
        &self.usage
    }

    pub fn validate_charge(&self) -> Result<(), String> {
        if self.usage.schedule() != psi_work::WorkSchedule::CURRENT.identity()
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
        if self.source.machine_by_symbol(self.machine).is_none()
            || !self
                .source
                .symbols
                .symbols()
                .nodes()
                .is_valid(self.requester)
            || self.source.machine_by_symbol(self.caller).is_none()
            || self.source.symbols.get(self.caller).generated_from != self.requester
            || self
                .program
                .view()
                .module()
                .identities
                .invocation_scope()
                .is_none()
            || self.program.evidence().is_none()
        {
            return Err("source invocation lost its accepted source/code custody".into());
        }
        let caller = self
            .source
            .machine_by_symbol(self.caller)
            .ok_or("source invocation lost its caller declaration")?;
        let [state] = self.source.machine_states(caller) else {
            return Err("source invocation lost its unique caller entry".into());
        };
        let Some(typed_trees::statement::StatementNode::Transition(transition)) = self
            .source
            .statement_table
            .statements(state.statement_nodes)
            .last()
        else {
            return Err("source invocation lost its actual return call".into());
        };
        if !self
            .source
            .statement_table
            .transition_target_is_valid(transition.target)
            || !matches!(
                self.source.statement_table.transition_target(transition.target),
                typed_trees::statement::TransitionTargetNode::Value(value) if *value == self.call
            )
        {
            return Err("source invocation receipt names another call".into());
        }
        let call = self.call()?;
        if call.target_symbol != self.machine
            && self
                .source
                .machine_holding_state(call.target_symbol)
                .is_none_or(|machine| machine.symbol != self.machine)
        {
            return Err("source invocation call selects another declaration".into());
        }
        let selected = self
            .program
            .view()
            .module()
            .entry
            .ok_or("source invocation receipt lost its entry")?;
        self.admission.require_invocation(
            self.program.view(),
            self.admission.target(),
            selected,
        )?;
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

pub(crate) fn pure_terminal_invocation_account()
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
    .ok_or("pure invocation limits are invalid")?;
    let measurement = evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_accounting::TerminalStorageScheduleIdentity::CURRENT,
    };
    evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(limits, measurement)
        .map_err(|error| format!("pure invocation sponsor: {error:?}"))?
        .begin_invocation(measurement, PURE_PSI_WORK_BUDGET)
        .map_err(|error| format!("pure invocation account: {error:?}"))
}

#[derive(Debug, Clone, Default, compiler_storage::Record)]
pub struct SourceCompletionAccounting {
    receipts: Arc<[PureEvaluationRecord]>,
    reused: bool,
}

impl SourceCompletionAccounting {
    pub(crate) fn append_invocation(&mut self, receipt: PureTerminalEvaluationReceipt) {
        self.append(Self {
            receipts: vec![PureEvaluationRecord::Invocation(receipt)].into(),
            reused: false,
        });
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
