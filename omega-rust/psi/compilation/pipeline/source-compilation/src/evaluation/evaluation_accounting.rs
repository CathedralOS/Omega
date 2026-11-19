/*
    Compile-time evaluation receipts, work and cell usage accounting, and
    canonical scalar evaluation records.

    We record per-invocation step and cell usage across pre-resolution and
    pre-check evaluation rounds and bind canonical scalar evaluation receipts to
    their verified Terminal execution outcomes.
*/

use std::sync::Arc;

use crate::evaluation::BuildTimeValue;

pub(crate) use artifact_evaluation::scalar_receipts::PURE_PSI_WORK_BUDGET;
pub use artifact_evaluation::scalar_receipts::{
    CanonicalScalarEvaluationReceipt, PureTerminalEvaluationOutcome,
};

/// Only actual canonical-artifact execution contributes an evaluation receipt.
#[derive(Debug, Clone)]
pub enum PureEvaluationRecord {
    CanonicalScalar(Arc<CanonicalScalarEvaluationReceipt>),
    Invocation(Arc<PureTerminalEvaluationReceipt>),
}

impl compiler_storage::Record for PureEvaluationRecord {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| match self {
            Self::CanonicalScalar(receipt) => {
                // This coordinate retains an optional deployment explicitly;
                // older scalar receipts always encoded a target string.
                compiler_storage::Record::write(&2_u32, output)?;
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
                2 => Ok(Self::CanonicalScalar(compiler_storage::Record::read(
                    input,
                )?)),
                0 => Err(input.error("no product route yet: retired tree-evaluation receipt")),
                1 => Err(input.error("retired scalar receipt without explicit deployment scope")),
                _ => Err(input.error("unknown PureEvaluationRecord variant")),
            },
        )
    }
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

/// A build-time value with the engine-tagged usage record of the evaluation
/// that produced it. Both engines publish `BuildEvaluationUsage`, whose
/// measurement identity names the work and storage schedules, so consumers
/// such as evaluated binding digests never interpret one engine's counts as
/// another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredEvaluation<T> {
    value: T,
    usage: evaluation_contracts::BuildEvaluationUsage,
}

impl<T> MeasuredEvaluation<T> {
    pub(crate) fn new(value: T, usage: evaluation_contracts::BuildEvaluationUsage) -> Self {
        Self { value, usage }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn usage(&self) -> evaluation_contracts::BuildEvaluationUsage {
        self.usage
    }

    pub fn into_value(self) -> T {
        self.value
    }

    pub fn into_parts(self) -> (T, evaluation_contracts::BuildEvaluationUsage) {
        (self.value, self.usage)
    }
}

// A standalone pure evaluation has no package sponsor. Its private account
// enforces the pure work budget; storage is measured, not capped, exactly as
// the checked evaluator measured cells without a ceiling.

// The private sponsor above is not a package session, so the published record
// carries only the invocation's own counts, like an unsponsored checked record.

/// A temporary caller can establish a real invocation without becoming a
/// portable program. Retain its source and code together with the actual
/// arguments and charges; saving this receipt remains explicitly unsupported.
#[derive(Debug, Clone)]
pub struct PureTerminalEvaluationReceipt {
    request_source: Arc<crate::source_state::TypedTrees>,
    source: Arc<crate::source_state::FrozenTypedTrees>,
    machine: symbols::SymbolHandle,
    requester: symbols::SymbolHandle,
    caller: symbols::SymbolHandle,
    call: crate::source_state::expression::ExpressionHandle,
    call_site: Option<(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::InvocationSite,
    )>,
    arguments: Arc<[BuildTimeValue]>,
    inputs: Arc<[BuildTimeValue]>,
    program: Arc<crate::production::ProducedTerminalProgram>,
    admission: terminal_installation::AdmittedCompilerEvaluation,
    execution: Option<artifact_evaluation::InvocationReceipt>,
    account: evaluation_accounting::EvaluationInvocationAccount,
    outcome: PureTerminalEvaluationOutcome,
    usage: terminal_work::WorkUsage,
}

impl PureTerminalEvaluationReceipt {
    pub(crate) fn new(
        entry: &crate::evaluation::PreparedBuildMachineEntry,
        program: Arc<crate::production::ProducedTerminalProgram>,
        admission: terminal_installation::AdmittedCompilerEvaluation,
        execution: Option<artifact_evaluation::InvocationReceipt>,
        account: evaluation_accounting::EvaluationInvocationAccount,
        outcome: PureTerminalEvaluationOutcome,
        usage: terminal_work::WorkUsage,
    ) -> Result<Self, String> {
        let accepted = entry.accepted();
        let call = entry.call_handle();
        let machine = entry.symbol();
        let requester = entry.requester();
        let request_source = entry.retained_source();
        let arguments = entry.shared_arguments();
        let inputs = entry.shared_inputs();
        let entry = accepted
            .entry()
            .ok_or("evaluation receipt lost its accepted entry")?;
        let caller = *accepted
            .closure()
            .elaboration()
            .machines
            .get(entry)
            .ok_or("evaluation receipt lost its actual caller")?;
        if !accepted.is_invocation()
            || &accepted.closure().elaboration().module != program.view().module()
        {
            return Err("evaluation receipt has another accepted code owner".into());
        }
        let mut sites = accepted.source_call_sites(caller, call);
        let call_site = sites.next();
        if sites.next().is_some() {
            return Err("evaluation receipt has multiple sites for its actual call".into());
        }
        let receipt = Self {
            request_source,
            source: Arc::clone(accepted.source()),
            machine,
            requester,
            caller,
            call,
            call_site,
            arguments,
            inputs,
            program,
            admission,
            execution,
            account,
            outcome,
            usage,
        };
        receipt.validate_charge()?;
        Ok(receipt)
    }

    pub fn source(&self) -> &crate::source_state::FrozenTypedTrees {
        &self.source
    }
    pub fn request_source(&self) -> &crate::source_state::TypedTrees {
        &self.request_source
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
    pub fn call(&self) -> Result<&crate::source_state::expression::TableCallExpression, String> {
        if !self.source.expression_table.expression_is_valid(self.call) {
            return Err("source invocation call has another source owner".into());
        }
        self.source
            .expression_table
            .call_node(self.call)
            .ok_or_else(|| "source invocation lost its retained call".into())
    }
    /// The operation is the one associated with this receipt's retained call
    /// before execution. A source description can materialize without a call
    /// operation; an ordinary or selected execution cannot substitute one.
    pub fn invocation(
        &self,
    ) -> Result<
        Option<(
            semantic_vocabulary::MachineId,
            terminal_psi::ModuleInvocation<'_>,
        )>,
        String,
    > {
        let Some((caller, site)) = self.call_site else {
            return Ok(None);
        };
        let module = self.program.view().module();
        if module.entry != Some(semantic_vocabulary::TargetMachineCallable::Machine(caller)) {
            return Err("source invocation receipt names another canonical caller".into());
        }
        let invocation = module
            .machine(caller)
            .and_then(|machine| {
                machine
                    .module_invocations()
                    .find(|invocation| invocation.site == site)
            })
            .ok_or("source invocation receipt lost its exact emitted call site")?;
        Ok(Some((caller, invocation)))
    }

    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }
    pub fn program(&self) -> &crate::production::ProducedTerminalProgram {
        &self.program
    }
    pub fn admission(&self) -> &terminal_installation::AdmittedCompilerEvaluation {
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
            execution.require_admission(self.program.view(), &self.admission)?;
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
        let Some([state]) = self.source.machine_states(caller).as_array() else {
            return Err("source invocation lost its unique caller entry".into());
        };
        let Some(crate::source_state::statement::StatementNode::Transition(transition)) = self
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
                crate::source_state::statement::TransitionTargetNode::Value(value) if *value == self.call
            )
        {
            return Err("source invocation receipt names another call".into());
        }
        let call = self.call()?;
        let source_view =
            crate::source_state::references::StaticArgumentCall::Expression(self.call, call);
        if source_view.applications().any(|application| {
            !self
                .source
                .expression_table
                .static_machine_application_is_valid(application)
        }) {
            return Err("source invocation has a stale application in its retained call".into());
        }
        let lexical_owner = source_view.lexical_owner(&self.source);
        let lexical_owner = if lexical_owner.is_valid() {
            lexical_owner
        } else {
            self.caller
        };
        if let Some((_, invocation)) = self.invocation()? {
            let module = self.program.view().module();
            let body = module
                .machine(invocation.inputs.callee)
                .ok_or("source invocation lost its actual executable body")?;
            let declared = self
                .source
                .machine_by_symbol(self.machine)
                .ok_or("source invocation lost its executable source declaration")?;
            let expected = self
                .source
                .machine_callable_identity_text(declared)
                .filter(|identity| !identity.is_empty())
                .ok_or("source invocation body has no complete source callable identity")?;
            if body
                .declaration
                .as_ref()
                .and_then(|declaration| module.identities.text(declaration.callable))
                != Some(expected.as_str())
            {
                return Err("source invocation site names another executable declaration".into());
            }
            match self
                .source
                .source_call_declaration(lexical_owner, source_view)?
            {
                crate::source_state::callable_signature_shapes::CallableDeclaration::Machine(_) => {
                    if invocation.inputs.required_contract.is_some() {
                        return Err(
                            "direct source invocation was replaced by a selected call".into()
                        );
                    }
                }
                crate::source_state::callable_signature_shapes::CallableDeclaration::Signature(
                    required,
                ) => {
                    let contract = invocation
                        .inputs
                        .required_contract
                        .and_then(|required| {
                            module.invocation_machine_contract(
                                invocation.inputs.callee,
                                Some(required),
                            )
                        })
                        .ok_or("selected source invocation lost its actual public contract")?;
                    let (owner, _) = self
                        .source
                        .trait_machine_signature(required.symbol)
                        .ok_or("selected source invocation lost its public source declaration")?;
                    let expected = self
                        .source
                        .trait_requirement_callable_identity_text(owner, required);
                    if contract
                        .declaration
                        .as_ref()
                        .and_then(|declaration| module.identities.text(declaration.callable))
                        != Some(expected.as_str())
                        || contract.selected_application.is_none()
                    {
                        return Err(
                            "selected source invocation changed its exact public application"
                                .into(),
                        );
                    }
                }
            }
        } else {
            if crate::source_state::reflection::construction::declaration_query_argument_role(
                &self.source,
                source_view,
            ) != crate::source_state::references::StaticArgumentRole::DeclarationDescription
            {
                return Err("source invocation lost its actual emitted call association".into());
            }
            if call.target_symbol != self.machine
                && self
                    .source
                    .machine_holding_state(call.target_symbol)
                    .is_none_or(|machine| machine.symbol != self.machine)
            {
                return Err("source description call selects another declaration".into());
            }
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
    let limits = evaluation_contracts::BuildEvaluationSponsorLimits::new(
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
    let measurement = evaluation_contracts::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: evaluation_contracts::TerminalStorageScheduleIdentity::CURRENT,
    };
    evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(limits, measurement)
        .map_err(|error| format!("pure invocation sponsor: {error:?}"))?
        .begin_invocation(measurement, PURE_PSI_WORK_BUDGET)
        .map_err(|error| format!("pure invocation account: {error:?}"))
}

#[derive(Debug, Clone, Default, compiler_storage::Record)]
pub struct SourceCompletionAccounting {
    receipts: Arc<Vec<PureEvaluationRecord>>,
    reused: bool,
}

impl SourceCompletionAccounting {
    pub(crate) fn append_invocation(&mut self, receipt: PureTerminalEvaluationReceipt) {
        // This constructor just completed one actual run, so its receipt is
        // fresh. Share it when accounts later join; do not recopy its payload.
        Arc::make_mut(&mut self.receipts).push(PureEvaluationRecord::Invocation(Arc::new(receipt)));
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
    pub pending: Option<crate::evaluation::PreCheckEvaluation>,
    pub accounting: SourceCompletionAccounting,
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
        self.reused |= other.reused;
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
