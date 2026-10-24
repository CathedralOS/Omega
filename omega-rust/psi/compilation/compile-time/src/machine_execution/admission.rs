//! Shared admission for machines the compiler executes during build-time
//! evaluation.
//!
//! This is the normalized service/suspension/blocking floor of the complete
//! build-time contract: a compiler-run machine must reach no boundary service
//! and must neither suspend nor block, every checked body in its concrete call
//! closure must carry the ordinary termination guarantee, and no reachable
//! checked body may declare an unadmitted linear runtime carrier. Authority,
//! finer resource admission, and failure remain independent axes and are added
//! here as their checked plans become available. Authored entry preconditions
//! need a checked invocation proof; internal premises behind an unconditional
//! entry use the source judgment. Post-check callers borrow that same checked
//! program rather than replaying checking. Escaping mutation is excluded by
//! the evaluator's fresh-value/snapshot boundary.

use checked_interpreter::{BuildMachineEvaluationRequest, BuildTimeOperationEvaluation};
use checked_trees::CheckedTrees;
use std::collections::VecDeque;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

use crate::BuildTimeValue;

mod closure_validation;
mod concrete_invocation;
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
        &[],
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

/// The program whose machine is evaluated. Early positions have only typed
/// input; post-check positions retain the judgment already produced by Psi.
/// Borrow the checked carrier itself so that its judgment cannot be paired
/// with a different typed program. Neither variant grants execution authority
/// or proves preconditions on the invocation's supplied arguments.
#[derive(Clone, Copy)]
pub enum BuildTimeProgram<'program> {
    Typed(&'program TypedTrees),
    Checked(&'program CheckedTrees),
}

impl<'program> BuildTimeProgram<'program> {
    fn checking_input(self) -> &'program dyn validation::CheckingInput {
        match self {
            Self::Typed(program) => program,
            Self::Checked(program) => program,
        }
    }

    fn typed(self) -> &'program TypedTrees {
        self.checking_input().typed_program()
    }
}

pub struct BuildTimeAdmissionPlan {
    service_reaches: flow_effects::ServiceReachInferencePlan,
    suspension: Vec<BuildTimeSuspensionRow>,
    blocking: Vec<BuildTimeBlockingRow>,
    call_edges: Vec<BuildTimeCallEdge>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    selected_operators: Vec<crate::SelectedBuildTimeBinaryOperator>,
    selected_machine_bodies: Vec<crate::SelectedBuildTimeMachineBody>,
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
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    target_operator_symbol: SymbolHandle,
}

impl BuildTimeCallEdge {
    /// The exact machine this call edge reaches: the settled machine symbol
    /// when the producer filled one, else the state-lane symbol only when it
    /// is itself a machine. Unmeasured terminal recursion deliberately keeps
    /// its machine symbol in the state lane until validation can diagnose the
    /// missing measure, so the walk must retain that edge too.
    fn target_machine(&self, program: &TypedTrees) -> Option<SymbolHandle> {
        if self.target_machine_symbol.is_valid() {
            return Some(self.target_machine_symbol);
        }
        (self.target_state_symbol.is_valid()
            && program.symbols.get(self.target_state_symbol).kind == symbols::SymbolKind::Machine)
            .then_some(self.target_state_symbol)
    }
}

/// Walk the build-time call closure from `root` over the single
/// [`BuildTimeCallEdge::target_machine`] classification: every machine is
/// visited once in depth-first discovery order, and `visit` decides the walk —
/// `Some(outcome)` ends it with that outcome, `None` continues. A consumer
/// whose obligation differs on an unsettled edge lanes that check at the edge
/// rather than re-deriving the closure.
fn walk_call_closure<Outcome>(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: SymbolHandle,
    mut visit: impl FnMut(&Machine) -> Option<Outcome>,
) -> Option<Outcome> {
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
        if let Some(outcome) = visit(machine) {
            return Some(outcome);
        }
        pending.extend(
            call_edges
                .iter()
                .filter(|call| call.source_machine_symbol == symbol)
                .filter_map(|call| call.target_machine(program)),
        );
    }
    None
}

impl BuildTimeAdmissionPlan {
    pub fn infer(
        program: &TypedTrees,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Self {
        let operational = validation::infer_operational_may(program);
        let service_reaches = validation::infer_service_reaches(program, &operational);
        let (suspension, blocking, call_edges) = project_operational_axes(&operational);
        Self {
            service_reaches,
            suspension,
            blocking,
            call_edges,
            selection_authority,
            selected_operators: Vec::new(),
            selected_machine_bodies: Vec::new(),
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
        walk_call_closure(&self.call_edges, program, root, |machine| {
            if machine.structural_type_equations_pending {
                return Some(format!(
                    "machine `{}` retains an undischarged structural type equation",
                    machine.name
                ));
            }
            (!machine.type_parameters.is_empty()).then(|| {
                format!(
                    "machine `{}` retains an unspecialized static application",
                    machine.name
                )
            })
        })
        .map_or(Ok(()), Err)
    }

    /// The realization owner supplies exact retained bodies, without giving
    /// target knowledge to Psi. Rewrite the shared closure edges once so all
    /// admission axes and execution follow the same checked declarations.
    pub fn with_selected_machine_bodies(
        mut self,
        program: &TypedTrees,
        selections: &[crate::SelectedBuildTimeMachineBody],
    ) -> Result<Self, String> {
        if !self.selected_machine_bodies.is_empty() {
            return Err("build-time family-body selections are already settled".to_owned());
        }
        crate::SelectedBuildTimeMachineBody::validate_all(program, selections)?;
        for edge in &mut self.call_edges {
            let target = if edge.target_machine_symbol.is_valid() {
                edge.target_machine_symbol
            } else {
                // Unmeasured terminal recursion keeps a machine-symbol edge
                // in the state lane; it requires the same body selection.
                edge.target_state_symbol
            };
            if let Some(selected) = selections.iter().find(|selected| selected.family == target) {
                edge.target_machine_symbol = selected.body;
                if let Some(entry) = program
                    .machine(selected.body)
                    .and_then(|machine| program.machine_states(machine).first())
                {
                    edge.target_state_symbol = entry.symbol;
                }
            }
        }
        self.selected_machine_bodies = selections.to_vec();
        Ok(self)
    }

    /// The machines in `root`'s call-edge closure, root first: the floor
    /// summarizes exactly the closure the shared walk admits.
    fn closure_machine_symbols(
        &self,
        program: &TypedTrees,
        root: SymbolHandle,
    ) -> Vec<SymbolHandle> {
        let mut closure = Vec::new();
        let _ = walk_call_closure(&self.call_edges, program, root, |machine| {
            closure.push(machine.symbol);
            None::<()>
        });
        closure
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

    pub(crate) fn closure_needs_operator_selection(
        &self,
        program: &TypedTrees,
        root: SymbolHandle,
        facts: &checked_trees::CheckedOperatorFacts,
    ) -> bool {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(machine) = pending.pop() {
            if visited.contains(&machine) {
                continue;
            }
            visited.push(machine);
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
                // Named trait requirements and token views carry the same
                // unsettled realization demand. Read their exact declaration,
                // never the spelling of a call or the absence of an entry.
                program
                    .trait_machine_signature(selected_operator_symbol)
                    .is_some_and(|(owner, _)| owner.is_boundary)
                    || program
                        .operators()
                        .iter()
                        .chain(program.machine_token_bindings())
                        .any(|operator| {
                            operator.symbol == selected_operator_symbol && operator.is_boundary
                        })
            };
            if facts
                .resolved_expression_selections()
                .any(|(_, origin, requirement, _)| {
                    origin.machine_symbol() == Some(machine) && boundary_use(requirement)
                })
            {
                return true;
            }
            pending.extend(
                self.call_edges
                    .iter()
                    .filter(|edge| edge.source_machine_symbol == machine)
                    .map(|edge| edge.target_machine_symbol),
            );
        }
        false
    }

    pub fn require_common_floor(
        &self,
        program: &TypedTrees,
        machine: &Machine,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor(BuildTimeProgram::Typed(program), machine, None, false)
    }

    pub fn require_common_floor_for_invocation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: BuildTimeInvocationCustody,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor(
            BuildTimeProgram::Typed(program),
            machine,
            Some(custody),
            false,
        )
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
        self.require_floor(
            BuildTimeProgram::Typed(program),
            machine,
            Some(custody),
            true,
        )
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
        input: BuildTimeProgram<'_>,
        machine: &Machine,
        custody: Option<BuildTimeInvocationCustody>,
        discharge_authored_requires: bool,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor_with_authority(
            input,
            machine,
            custody,
            discharge_authored_requires,
            self.selection_authority.as_deref(),
        )
    }

    /// Invocation-owned authority is deliberately not retained with reusable
    /// program geometry. Borrow it while admitting the same complete closure.
    pub(crate) fn require_invocation_with_authority(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: BuildTimeInvocationCustody,
        authority: Option<&dyn BuildTimeSelectionAuthority>,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        self.require_floor_with_authority(
            BuildTimeProgram::Typed(program),
            machine,
            Some(custody),
            false,
            authority,
        )
    }

    fn require_floor_with_authority(
        &self,
        input: BuildTimeProgram<'_>,
        machine: &Machine,
        custody: Option<BuildTimeInvocationCustody>,
        discharge_authored_requires: bool,
        authority: Option<&dyn BuildTimeSelectionAuthority>,
    ) -> Result<(), BuildTimeAdmissionRejection> {
        let program = input.typed();
        crate::validate_selected_operators(program, &self.selected_operators)?;
        crate::SelectedBuildTimeMachineBody::validate_all(program, &self.selected_machine_bodies)?;
        // The ordinary inference may stop at a bodyless family contract. Its
        // selected body's complete summaries remain independently mandatory;
        // never grant purity merely by redirecting the executable edge.
        let mut services = Vec::new();
        let mut transitive_may_suspend = false;
        let mut transitive_may_block = false;
        for symbol in self.closure_machine_symbols(program, machine.symbol) {
            let summary = self.service_reaches.for_machine(symbol).ok_or_else(|| {
                format!(
                    "build-time closure machine {symbol:?} has no inferred service-reach summary"
                )
            })?;
            for service in self.service_reaches.services(summary.effective) {
                let definition = program.service_reaches.definition(*service).ok_or_else(|| {
                    format!("build-time closure machine {symbol:?} reaches an unknown canonical service identity")
                })?;
                if !services.contains(&definition.name.as_str()) {
                    services.push(definition.name.as_str());
                }
            }
            transitive_may_suspend |= self.machine_suspension(symbol).ok_or_else(|| {
                format!("build-time closure machine {symbol:?} has no suspension summary")
            })?;
            transitive_may_block |= self.machine_blocking(symbol).ok_or_else(|| {
                format!("build-time closure machine {symbol:?} has no blocking summary")
            })?;
        }
        self.require_closed_static_applications(program, machine.symbol)?;
        // Premises past an unconditional entry are proved at their own
        // source sites, whatever this invocation's arguments.
        let discharge_authored_requires = discharge_authored_requires
            || (self.closure_includes_authored_requires(program, machine)
                && concrete_invocation::argument_independent_closure_discharges(input, machine)?);
        let closure_violation = checked_closure_violation(
            &self.call_edges,
            program,
            machine,
            discharge_authored_requires,
        );
        let selection_violation = selection_authority_violation(
            &self.call_edges,
            input.checking_input(),
            machine,
            custody,
            authority,
            &self.selected_operators,
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
        input: BuildTimeProgram<'_>,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
    ) -> Result<BuildTimeValue, BuildTimeAdmissionRejection> {
        let program = input.typed();
        let machine = program
            .realized_machine_named(machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_floor(input, machine, None, false)?;
        checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
                machine_bodies: &self.selected_machine_bodies,
                ..BuildMachineEvaluationRequest::named(machine_name, arguments)
            },
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
        input: BuildTimeProgram<'_>,
        machine_symbol: SymbolHandle,
        arguments: Vec<BuildTimeValue>,
        custody: BuildTimeInvocationCustody,
    ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
        let program = input.typed();
        let machine = program.machine(machine_symbol).ok_or_else(|| {
            format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            )
        })?;
        self.require_floor(input, machine, Some(custody), false)?;
        checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
                machine_bodies: &self.selected_machine_bodies,
                ..BuildMachineEvaluationRequest::symbol(machine_symbol, arguments)
            },
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
        self.require_floor(
            BuildTimeProgram::Typed(program),
            machine,
            custody,
            discharge,
        )?;
        let value = checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
                product_entry_compatibility: None,
                operators: &self.selected_operators,
                machine_bodies: &self.selected_machine_bodies,
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

    fn evaluate_const_evaluable_invocation_measured(
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
        let measured = checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
                product_entry_compatibility: None,
                operators: &self.selected_operators,
                machine_bodies: &self.selected_machine_bodies,
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
                call_edges.push(BuildTimeCallEdge {
                    source_machine_symbol: machine.symbol,
                    target_machine_symbol: call.target_machine_symbol,
                    target_state_symbol: call.target_state_symbol,
                    target_operator_symbol: call.target_operator_symbol,
                });
            }
        }
    }
    (suspension, blocking, call_edges)
}
