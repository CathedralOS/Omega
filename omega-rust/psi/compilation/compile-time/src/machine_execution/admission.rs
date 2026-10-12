//! Shared admission for machines the compiler executes during build-time
//! evaluation.
//!
//! This is the normalized service/suspension/blocking floor of the complete
//! build-time contract: a compiler-run machine must reach no boundary service
//! and must neither suspend nor block, every checked body in its concrete call
//! closure must carry the ordinary termination guarantee, and no reachable
//! checked body may declare an unadmitted linear runtime carrier. Authority,
//! finer resource admission, and failure remain independent axes and are added
//! here as their checked plans become available. Authored preconditions reject
//! until the pre-check invocation supplies a checked proof context. Escaping
//! mutation is excluded by the evaluator's fresh-value/snapshot boundary.

use checked_interpreter::{BuildMachineEvaluationRequest, BuildTimeOperationEvaluation};
use std::collections::VecDeque;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

use crate::BuildTimeValue;

mod closure_validation;
mod concrete_invocation;
mod kernel_applications;
mod supplied_values;

pub(crate) use concrete_invocation::{SnapshotArgument, snapshot_arguments_discharge};
pub(crate) use supplied_values::require_established_arguments;
pub(crate) mod const_evaluable;
mod selection_authority;

use closure_validation::checked_closure_violation;
use const_evaluable::require_const_evaluable_result;
use selection_authority::selection_authority_violation;
pub(crate) use selection_authority::{
    expression_children, require_call_expression_selection, require_closed_expression_custody,
    require_closed_integer_argument,
};

pub use validation::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;

/// Check the existing declaration-selection gate for a call-free expression probe.
pub(crate) fn require_const_expression_selection(
    program: &TypedTrees,
    machine: &Machine,
    source: source::SourceSpan,
    authority: Option<&dyn BuildTimeSelectionAuthority>,
) -> Result<(), String> {
    match selection_authority_violation(
        &[],
        program,
        machine,
        Some(BuildTimeInvocationCustody::Source(source)),
        authority,
        kernel_applications::KernelApplications::new(program, &[], None),
    ) {
        Some(violation) => Err(violation.message),
        None => Ok(()),
    }
}

/// One refused build-time admission. `source_span` locates the authored
/// occurrence that decided the refusal when one exists (a declaration
/// selection, an operator without builtin meaning); a machine-level floor
/// violation carries none, and the reporting owner falls back to the
/// invocation it was evaluating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTimeAdmissionRejection {
    pub reason: String,
    pub source_span: Option<source::SourceSpan>,
}

impl From<String> for BuildTimeAdmissionRejection {
    fn from(reason: String) -> Self {
        Self {
            reason,
            source_span: None,
        }
    }
}

impl From<BuildTimeAdmissionRejection> for String {
    fn from(rejection: BuildTimeAdmissionRejection) -> Self {
        rejection.reason
    }
}

impl std::fmt::Display for BuildTimeAdmissionRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTimeInvocationCustody {
    Source(source::SourceSpan),
    Symbol(SymbolHandle),
}

pub struct BuildTimeAdmissionPlan {
    service_reaches: flow_effects::ServiceReachInferencePlan,
    suspension: Vec<BuildTimeSuspensionRow>,
    blocking: Vec<BuildTimeBlockingRow>,
    call_edges: Vec<BuildTimeCallEdge>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    selected_operators: Vec<crate::SelectedBuildTimeBinaryOperator>,
    checked_execution: Option<crate::CheckedBuildProgram>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuildTimeSuspensionRow {
    machine_symbol: SymbolHandle,
    transitive_may_suspend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuildTimeBlockingRow {
    machine_symbol: SymbolHandle,
    transitive_may_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuildTimeCallEdge {
    source_machine_symbol: SymbolHandle,
    source_state_symbol: SymbolHandle,
    statement_index: usize,
    authored_expression: typed_trees::expression::ExpressionHandle,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    target_operator_symbol: SymbolHandle,
}

impl BuildTimeAdmissionPlan {
    pub fn infer(
        program: &TypedTrees,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Self {
        let operational = validation::infer_operational_may(program);
        let service_reaches = validation::infer_service_reaches(program, &operational);
        let (suspension, blocking, call_edges) = project_operational_axes(program, &operational);
        Self {
            service_reaches,
            suspension,
            blocking,
            call_edges,
            selection_authority,
            selected_operators: Vec::new(),
            checked_execution: None,
        }
    }

    /// Bind admission and interpretation to one immutable checked preparation.
    /// The old early typed entrance remains distinct; it cannot impersonate
    /// this checked dependency owner by sharing handle numbers or type labels.
    pub(crate) fn infer_prepared(
        prepared: &crate::PreparedBuildMachineProgram,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Result<Self, String> {
        let checked = prepared
            .checked_program()
            .ok_or("constant dependency preparation has no ordinary checking judgment")?;
        let mut admission = Self::infer(prepared.typed(), selection_authority);
        // Public operational inference deliberately judges original calls.
        // Execution closure custody instead consumes this exact checking
        // judgment's actual endpoints, including selected providers/helpers.
        admission.call_edges = project_checked_execution_edges(checked.checked());
        admission.checked_execution = Some(checked);
        Ok(admission)
    }

    pub(crate) fn snapshot_source_program(
        &self,
        program: &TypedTrees,
    ) -> Result<crate::constant_inputs::SnapshotSourceProgram, String> {
        let checked = self
            .checked_execution
            .as_ref()
            .ok_or("snapshot invocation has no owned checked preparation")?;
        if !std::ptr::eq(checked.typed(), program) {
            return Err("snapshot invocation does not belong to its checked preparation".into());
        }
        Ok(crate::constant_inputs::SnapshotSourceProgram::Checked(
            checked.clone(),
        ))
    }

    fn interpreter_program<'program>(
        &'program self,
        program: &'program TypedTrees,
    ) -> Result<checked_interpreter::BuildMachineProgram<'program>, String> {
        if let Some(checked) = &self.checked_execution {
            if !std::ptr::eq(checked.typed(), program) {
                return Err(
                    "constant invocation does not belong to its checked preparation".into(),
                );
            }
            Ok(checked.interpreter_program())
        } else {
            Ok(checked_interpreter::BuildMachineProgram::Typed(program))
        }
    }

    /// Provisional typing retains equation and pending-type obligations.
    /// Every reachable call must have closed its static tuple before execution;
    /// a wrapper's concrete signature cannot discharge its callee's obligations.
    pub(crate) fn require_closed_static_applications(
        &self,
        program: &TypedTrees,
        root: SymbolHandle,
    ) -> Result<(), String> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(symbol) = pending.pop() {
            if visited.contains(&symbol) {
                continue;
            }
            visited.push(symbol);
            let Some(machine) = program.machine(symbol) else {
                continue;
            };
            if machine.structural_type_equations_pending {
                return Err(format!(
                    "machine `{}` retains an undischarged structural type equation",
                    machine.name
                ));
            }
            if !machine.type_parameters.is_empty() {
                return Err(format!(
                    "machine `{}` retains an unspecialized static application",
                    machine.name
                ));
            }
            for edge in self
                .call_edges
                .iter()
                .filter(|edge| edge.source_machine_symbol == symbol)
            {
                if edge.target_machine_symbol.is_valid() {
                    pending.push(edge.target_machine_symbol);
                } else if program.symbols.get(edge.target_state_symbol).kind
                    == symbols::SymbolKind::Machine
                {
                    pending.push(edge.target_state_symbol);
                }
            }
        }
        Ok(())
    }

    pub fn with_selected_operators(
        mut self,
        program: &TypedTrees,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
    ) -> Result<Self, String> {
        crate::validate_selected_operators(program, operators)?;
        self.selected_operators = operators.to_vec();
        Ok(self)
    }

    /// Checked preparation supplies actual execution edges; early typed
    /// inference projects its still-authored occurrences. Runtime offerings
    /// cannot seed this semantic dependency roster.
    pub(crate) fn reachable_machine_symbols(&self, roots: &[SymbolHandle]) -> Vec<SymbolHandle> {
        let mut pending = roots.to_vec();
        let mut visited = Vec::new();
        while let Some(machine) = pending.pop() {
            if visited.contains(&machine) {
                continue;
            }
            visited.push(machine);
            pending.extend(
                self.call_edges
                    .iter()
                    .filter(|edge| edge.source_machine_symbol == machine)
                    .map(|edge| edge.target_machine_symbol),
            );
        }
        visited
    }

    pub(crate) fn closure_needs_operator_selection(
        &self,
        program: &TypedTrees,
        root: SymbolHandle,
        facts: &checked_trees::CheckedOperatorFacts,
    ) -> bool {
        for machine in self.reachable_machine_symbols(&[root]) {
            // Named and normalized token requirements appear as ordinary
            // calls. Their bodies are supplied by selected providers, not by
            // the requirement declarations in this closure.
            if program.machines().iter().any(|declaration| {
                declaration.symbol == machine
                    && declaration.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) {
                return true;
            }
            let boundary_use = |selected_operator_symbol| {
                // Before call normalization the same machine requirement is
                // selected through its token-signature view instead.
                program
                    .operators()
                    .iter()
                    .chain(program.machine_token_bindings())
                    .any(|operator| {
                        operator.symbol == selected_operator_symbol && operator.is_boundary
                    })
            };
            if facts
                .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
                .any(|fact| {
                    fact.origin.machine_symbol() == Some(machine)
                        && boundary_use(fact.selected_operator_symbol)
                })
                || facts.named_uses().any(|fact| {
                    fact.origin.machine_symbol() == Some(machine)
                        && boundary_use(fact.selected_operator_symbol)
                })
            {
                return true;
            }
        }
        false
    }

    pub fn require_common_floor(
        &self,
        program: &TypedTrees,
        machine: &Machine,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor(program, machine, None, false)
    }

    pub fn require_common_floor_for_invocation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: BuildTimeInvocationCustody,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor(program, machine, Some(custody), false)
    }

    /// Common floor for one concrete invocation whose own checked probe
    /// discharges authored `requires` premises at snapshot arguments. An
    /// authored premise has no meaning outside an invocation, so the
    /// conservative closure fence stands down on that axis only: the probe
    /// re-runs ordinary contract checking on the exact call before any
    /// interpretation, and termination, linear carriers, service reach, and
    /// selection authority still apply unchanged.
    pub(crate) fn require_common_floor_for_concrete_premise_invocation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: BuildTimeInvocationCustody,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor(program, machine, Some(custody), true)
    }

    /// Whether `machine`'s call closure carries any authored `requires`
    /// premise — on a reachable machine, one of its states, or a callable
    /// signature target. Callers that answer `true` must discharge those
    /// premises through the concrete checked probe before interpreting.
    pub(crate) fn closure_includes_authored_requires(
        &self,
        program: &TypedTrees,
        machine: &Machine,
    ) -> bool {
        closure_validation::closure_has_authored_requires(&self.call_edges, program, machine.symbol)
    }

    /// Whether the authored `requires` premises in `machine`'s call closure
    /// are discharged by this exact zero-argument `machine()` invocation:
    /// the synthesized call has no snapshot obligations to reproduce, so a
    /// private probe machine carrying it can re-run ordinary contract
    /// checking and decide every premise at the concrete call. A
    /// non-zero-argument invocation has authored arguments a synthesized
    /// `machine()` call cannot reproduce, so only the empty-arguments
    /// invocation gains the discharge; every other admission path keeps the
    /// conservative fence on that axis.
    fn zero_argument_invocation_discharge(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        arguments: &[BuildTimeValue],
        custody: Option<BuildTimeInvocationCustody>,
    ) -> bool {
        arguments.is_empty()
            && self.closure_includes_authored_requires(program, machine)
            && concrete_invocation::zero_argument_invocation_discharges(
                program,
                machine,
                custody.and_then(|custody| match custody {
                    BuildTimeInvocationCustody::Source(span) => Some(span),
                    BuildTimeInvocationCustody::Symbol(symbol) => {
                        program.symbols.symbol_source_span(symbol)
                    }
                }),
            )
    }

    /// Whether the only authored `requires` premises in `machine`'s call
    /// closure are parameter-domain premises on `machine`'s own states. A
    /// caller that has proved each concrete argument's membership in its
    /// parameter's declared domain may then use the concrete-premise
    /// invocation; any other premise keeps the conservative closure fence.
    pub(crate) fn closure_requires_are_entry_parameter_domains(
        &self,
        program: &TypedTrees,
        machine: &Machine,
    ) -> bool {
        closure_validation::closure_requires_are_root_parameter_domains(
            &self.call_edges,
            program,
            machine.symbol,
        )
    }

    fn require_floor(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: Option<BuildTimeInvocationCustody>,
        discharge_authored_requires: bool,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        crate::validate_selected_operators(program, &self.selected_operators)?;
        if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some() {
            let closure = self.reachable_machine_symbols(&[machine.symbol]);
            eprintln!(
                "CONST FLOOR root={} checked={} operators={} closure={:?}",
                machine.name,
                self.checked_execution.is_some(),
                self.selected_operators.len(),
                closure
            );
            for symbol in closure.iter().filter(|symbol| symbol.is_valid()).take(8) {
                let target = program.machine_by_symbol(*symbol);
                let summary = self
                    .checked_execution
                    .as_ref()
                    .and_then(|checked| checked.checked().facts.termination.for_machine(*symbol))
                    .map(|plan| &plan.checked_summary);
                eprintln!(
                    "CONST FLOOR member={symbol:?} name={} mode={:?} retained={summary:?}",
                    program.symbols.name(*symbol),
                    target.map(|machine| machine.supply_mode)
                );
            }
            if let Some(checked) = &self.checked_execution {
                let control = &checked.checked().facts.flow.control;
                for (_, state) in control
                    .states
                    .iter()
                    .filter(|(_, state)| closure.contains(&state.machine_symbol))
                    .take(8)
                {
                    for call in control.calls.span_or_empty(state.calls).iter().take(8) {
                        let physical = call
                            .authored_expression
                            .is_valid()
                            .then(|| program.expression_table.call_node(call.authored_expression))
                            .flatten()
                            .map(|row| row.target_symbol);
                        let original = call
                            .authored_expression
                            .is_valid()
                            .then(|| {
                                program
                                    .expression_table
                                    .source_call_node(call.authored_expression)
                            })
                            .flatten()
                            .map(|row| row.target_symbol);
                        eprintln!(
                            "CONST FLOOR call state={:?} statement={} ordinal={} flow={:?} physical={physical:?} original={original:?} retired={}",
                            state.state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                            call.target_symbol,
                            control.is_retired(state.state_symbol, call)
                        );
                    }
                }
            }
        }
        // Preserve the caller's public invocation envelope, then check each
        // physically reached body's own envelope. A selected private body
        // cannot inherit empty reach/suspension/blocking from its requirement.
        let mut services = Vec::new();
        let mut transitive_may_suspend = false;
        let mut transitive_may_block = false;
        for reached in self.reachable_machine_symbols(&[machine.symbol]) {
            if !reached.is_valid() {
                continue;
            }
            let service_summary = self.service_reaches.for_machine(reached).ok_or_else(|| {
                format!("machine {reached:?} has no inferred service-reach summary")
            })?;
            transitive_may_suspend |= self.machine_suspension(reached).ok_or_else(|| {
                format!("machine {reached:?} has no inferred operational summary")
            })?;
            transitive_may_block |= self.machine_blocking(reached).ok_or_else(|| {
                format!("machine {reached:?} has no inferred operational summary")
            })?;
            for service in self.service_reaches.services(service_summary.effective) {
                let name = program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .ok_or_else(|| {
                        format!("machine {reached:?} reaches an unknown canonical service identity")
                    })?;
                if !services.iter().any(|existing| existing == name) {
                    services.push(name.to_owned());
                }
            }
        }
        self.require_closed_static_applications(program, machine.symbol)?;
        // Premises past an unconditional entry are proved at their own
        // source sites, whatever this invocation's arguments.
        let discharge_authored_requires = discharge_authored_requires
            || (self.closure_includes_authored_requires(program, machine)
                && concrete_invocation::argument_independent_closure_discharges(program, machine)
                    .map_err(|diagnostics| BuildTimeAdmissionRejection {
                        reason: format!(
                            "build-time premise closure of `{}` failed ordinary preparation/checking: {}",
                            machine.name,
                            diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
                        ),
                        source_span: diagnostics.iter().find_map(|diagnostic| diagnostic.source_span),
                    })?);
        let kernels = kernel_applications::KernelApplications::new(
            program,
            &self.selected_operators,
            self.checked_execution.as_ref().map(|owner| owner.checked()),
        );
        let closure_violation = checked_closure_violation(
            &self.call_edges,
            program,
            machine,
            discharge_authored_requires,
            kernels,
        );
        let selection_violation = selection_authority_violation(
            &self.call_edges,
            program,
            machine,
            custody,
            self.selection_authority.as_deref(),
            kernels,
        );
        if services.is_empty()
            && !transitive_may_suspend
            && !transitive_may_block
            && closure_violation.is_none()
            && selection_violation.is_none()
        {
            return Ok(());
        }

        let source_span = selection_violation
            .as_ref()
            .and_then(|violation| violation.source_span);
        let mut violations = Vec::new();
        if !services.is_empty() {
            violations.push(format!("service reach [{}]", services.join(", ")));
        }
        if transitive_may_suspend {
            violations.push("may suspend".to_owned());
        }
        if transitive_may_block {
            violations.push("may block".to_owned());
        }
        if let Some(violation) = closure_violation {
            violations.push(violation);
        }
        if let Some(violation) = selection_violation {
            violations.push(violation.message);
        }

        Err(BuildTimeAdmissionRejection {
            reason: format!(
                "machine `{}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
                machine.name,
                violations.join("; ")
            ),
            source_span,
        })
    }

    /// Admit and evaluate one result-bearing semantic machine against this
    /// inferred program plan. Callers construct the semantic argument snapshot;
    /// Psi owns machine lookup, the common admission floor, and interpreter
    /// execution. Decoding and validating a position-specific result remains
    /// with that position's normalized-plan owner.
    pub fn evaluate_machine(
        &self,
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
    ) -> Result<BuildTimeValue, BuildTimeAdmissionRejection> {
        let machine = program
            .realized_machine_named(machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor(program, machine)?;
        checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(program)?,
            BuildMachineEvaluationRequest::named(machine_name, arguments),
        )
        .map(BuildTimeOperationEvaluation::into_value)
        .map_err(BuildTimeAdmissionRejection::from)
    }

    pub fn evaluate_machine_for_invocation(
        &self,
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<BuildTimeValue, BuildTimeAdmissionRejection> {
        let machine = program
            .realized_machine_named(machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor_for_invocation(program, machine, custody)?;
        checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(program)?,
            BuildMachineEvaluationRequest::named(machine_name, arguments),
        )
        .map(BuildTimeOperationEvaluation::into_value)
        .map_err(BuildTimeAdmissionRejection::from)
    }

    /// Admit and evaluate the exact result-bearing machine selected by a
    /// typed invocation. This is the source-boundary seam for positions such
    /// as `via`: the resolved symbol remains authoritative, and deterministic
    /// evaluator usage stays attached to the returned structured value.
    pub fn evaluate_machine_symbol_for_invocation_measured(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
        let machine = program.machine(machine_symbol).ok_or_else(|| {
            format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            )
        })?;
        self.require_common_floor_for_invocation(program, machine, custody)?;
        checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(program)?,
            BuildMachineEvaluationRequest::symbol(machine_symbol, arguments),
        )
        .map(BuildTimeOperationEvaluation::into_measured)
    }

    /// Return the exact checked-machine closure admitted for one compiler
    /// invocation. The root is included. Consumers may commit this set beside
    /// a decoded result, but must still derive stable symbol and source
    /// identities rather than persisting arena handles.
    pub fn admitted_machine_closure_symbols(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        custody: BuildTimeInvocationCustody,
    ) -> Result<Vec<SymbolHandle>, String> {
        let machine = program.machine(machine_symbol).ok_or_else(|| {
            format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            )
        })?;
        self.require_common_floor_for_invocation(program, machine, custody)?;

        let machine_symbols = program
            .machines()
            .iter()
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        let mut pending = VecDeque::from([machine_symbol]);
        let mut closure = Vec::new();
        while let Some(source) = pending.pop_front() {
            if closure.contains(&source) {
                continue;
            }
            closure.push(source);
            for target in self
                .call_edges
                .iter()
                .filter(|edge| edge.source_machine_symbol == source)
                .map(|edge| edge.target_machine_symbol)
            {
                if !target.is_valid() || !machine_symbols.contains(&target) {
                    return Err(format!(
                        "build-time machine closure rooted at `{}` contains an unresolved machine target",
                        machine.name,
                    ));
                }
                if !closure.contains(&target) {
                    pending.push_back(target);
                }
            }
        }
        Ok(closure)
    }

    /// Admit and evaluate one machine whose result position explicitly
    /// requires the target-neutral `ConstEvaluable(T, value)` judgment.
    /// Existing compiler-owned structured plan positions remain on
    /// [`Self::evaluate_machine`] until their result vocabularies opt in.
    pub fn evaluate_const_evaluable_machine(
        &self,
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
    ) -> Result<BuildTimeValue, String> {
        self.evaluate_const_evaluable_machine_for_invocation_opt(
            program,
            machine_name,
            arguments,
            None,
        )
    }

    fn evaluate_const_evaluable_machine_for_invocation_opt(
        &self,
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
        custody: Option<BuildTimeInvocationCustody>,
    ) -> Result<BuildTimeValue, String> {
        let machine = program
            .realized_machine_named(machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        let discharge =
            self.zero_argument_invocation_discharge(program, machine, &arguments, custody);
        self.require_floor(program, machine, custody, discharge)?;
        let value = checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(program)?,
            BuildMachineEvaluationRequest {
                product_entry_compatibility: None,
                operators: &self.selected_operators,
                ..BuildMachineEvaluationRequest::symbol(machine.symbol, arguments)
            },
        )
        .map(BuildTimeOperationEvaluation::into_measured)?
        .into_parts()
        .0;
        require_const_evaluable_result(program, machine, &value)?;
        Ok(value)
    }

    pub fn evaluate_const_evaluable_machine_for_invocation(
        &self,
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<BuildTimeValue, String> {
        self.evaluate_const_evaluable_machine_for_invocation_opt(
            program,
            machine_name,
            arguments,
            Some(custody),
        )
    }

    pub fn evaluate_const_evaluable_machine_symbol_for_invocation(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<BuildTimeValue, String> {
        self.evaluate_const_evaluable_machine_symbol_for_invocation_measured(
            program,
            machine_symbol,
            arguments,
            custody,
        )
        .map(crate::MeasuredEvaluation::into_value)
    }

    /// [`Self::evaluate_const_evaluable_machine_symbol_for_invocation`] for a
    /// concrete invocation whose own checked probe has already discharged the
    /// closure's authored `requires` premises at these exact snapshot
    /// arguments. The premise evidence comes from the probe's ordinary
    /// contract checking, never from interpretation succeeding.
    pub(crate) fn evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<BuildTimeValue, String> {
        self.evaluate_const_evaluable_invocation_measured(
            program,
            machine_symbol,
            arguments,
            custody,
            true,
        )
        .map(crate::MeasuredEvaluation::into_value)
    }

    /// [`Self::evaluate_const_evaluable_machine_symbol_for_invocation`]
    /// retaining the deterministic evaluator usage beside the admitted
    /// snapshot. Invocation positions such as `via` that commit the measured
    /// usage as evidence select this entry once their result vocabulary opts
    /// into the target-neutral `ConstEvaluable` judgment.
    pub fn evaluate_const_evaluable_machine_symbol_for_invocation_measured(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
        self.evaluate_const_evaluable_invocation_measured(
            program,
            machine_symbol,
            arguments,
            custody,
            false,
        )
    }

    pub(crate) fn evaluate_const_evaluable_invocation_measured(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
        discharge_authored_requires: bool,
    ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
        let matching: Vec<_> = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol)
            .collect();
        let [machine] = matching.as_slice() else {
            return Err("build-time invocation has no unique exact machine".into());
        };
        self.require_const_evaluable_invocation(
            program,
            machine,
            &arguments,
            custody,
            discharge_authored_requires,
        )?;
        let measured = checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(program)?,
            BuildMachineEvaluationRequest {
                product_entry_compatibility: None,
                operators: &self.selected_operators,
                ..BuildMachineEvaluationRequest::symbol(machine_symbol, arguments)
            },
        )
        .map(BuildTimeOperationEvaluation::into_measured)?;
        require_const_evaluable_result(program, machine, measured.value())?;
        Ok(measured)
    }

    /// Admission is independent of the evaluator's result carrier. A prepared
    /// checked job keeps the same source invocation and selected semantics
    /// across ordinary call normalization without dropping its CheckFacts.
    pub(crate) fn require_const_evaluable_invocation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        arguments: &[BuildTimeValue],
        custody: BuildTimeInvocationCustody,
        discharge_authored_requires: bool,
    ) -> Result<(), String> {
        let discharge_authored_requires = discharge_authored_requires
            || self.zero_argument_invocation_discharge(program, machine, arguments, Some(custody));
        if discharge_authored_requires {
            self.require_common_floor_for_concrete_premise_invocation(program, machine, custody)?;
        } else {
            self.require_common_floor_for_invocation(program, machine, custody)?;
        }
        Ok(())
    }

    fn machine_suspension(&self, machine_symbol: SymbolHandle) -> Option<bool> {
        self.suspension
            .iter()
            .find(|row| row.machine_symbol == machine_symbol)
            .map(|row| row.transitive_may_suspend)
    }

    fn machine_blocking(&self, machine_symbol: SymbolHandle) -> Option<bool> {
        self.blocking
            .iter()
            .find(|row| row.machine_symbol == machine_symbol)
            .map(|row| row.transitive_may_block)
    }
}

fn project_operational_axes(
    program: &TypedTrees,
    operational: &flow_effects::OperationalPlan,
) -> (
    Vec<BuildTimeSuspensionRow>,
    Vec<BuildTimeBlockingRow>,
    Vec<BuildTimeCallEdge>,
) {
    let mut suspension = Vec::new();
    let mut blocking = Vec::new();
    let mut call_edges = Vec::new();
    for machine in operational.machines() {
        suspension.push(BuildTimeSuspensionRow {
            machine_symbol: machine.symbol,
            transitive_may_suspend: machine.transitive_may_suspend,
        });
        blocking.push(BuildTimeBlockingRow {
            machine_symbol: machine.symbol,
            transitive_may_block: machine.transitive_may_block,
        });
        for state in operational.states.span_or_empty(machine.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                let executable = validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .and_then(|site| match site {
                    validation::CallSite::Expression { call, .. }
                        if call.source_call.is_valid() =>
                    {
                        Some(call.target_symbol)
                    }
                    _ => None,
                });
                call_edges.push(executable.map_or(
                    BuildTimeCallEdge {
                        source_machine_symbol: machine.symbol,
                        source_state_symbol: SymbolHandle::invalid(),
                        statement_index: usize::MAX,
                        authored_expression: typed_trees::expression::ExpressionHandle::invalid(),
                        target_machine_symbol: call.target_machine_symbol,
                        target_state_symbol: call.target_state_symbol,
                        target_operator_symbol: call.target_operator_symbol,
                    },
                    |target| execution_edge(program, machine.symbol, target),
                ));
            }
        }
    }
    (suspension, blocking, call_edges)
}

/// Project actual endpoints from the same immutable checked closure consumed
/// by interpretation. Original public contracts and effects remain on its
/// checked source payloads; they cannot replace this physical execution graph.
fn project_checked_execution_edges(
    checked: &checked_trees::CheckedTrees,
) -> Vec<BuildTimeCallEdge> {
    let control = &checked.facts.flow.control;
    let mut edges = Vec::new();
    for (_, state) in control.states.iter() {
        for call in control.calls.span_or_empty(state.calls) {
            if !control.is_retired(state.state_symbol, call) {
                let mut edge =
                    execution_edge(&checked.typed, state.machine_symbol, call.target_symbol);
                edge.source_state_symbol = state.state_symbol;
                edge.statement_index = call.statement_index;
                edge.authored_expression = call.authored_expression;
                edges.push(edge);
            }
        }
    }
    edges
}

fn execution_edge(
    program: &TypedTrees,
    source_machine_symbol: SymbolHandle,
    target: SymbolHandle,
) -> BuildTimeCallEdge {
    let machine = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target));
    BuildTimeCallEdge {
        source_machine_symbol,
        source_state_symbol: SymbolHandle::invalid(),
        statement_index: usize::MAX,
        authored_expression: typed_trees::expression::ExpressionHandle::invalid(),
        target_machine_symbol: machine.map_or(SymbolHandle::invalid(), |machine| machine.symbol),
        target_state_symbol: target,
        target_operator_symbol: typed_trees::operator::declaration_by_symbol(program, target)
            .map_or(SymbolHandle::invalid(), |operator| operator.symbol),
    }
}
