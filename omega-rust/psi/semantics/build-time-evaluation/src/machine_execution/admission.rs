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
mod const_evaluable;
mod selection_authority;
#[cfg(test)]
mod structural_equations_tests;

use closure_validation::checked_closure_violation;
use const_evaluable::require_const_evaluable_result;
use selection_authority::selection_authority_violation;
pub(crate) use selection_authority::{
    expression_children, require_call_expression_selection, require_closed_expression_custody,
    require_closed_integer_argument,
};

/// Package-neutral authority consulted before the compiler executes an
/// authored machine during early semantic evaluation.
///
/// Psi owns provenance and call-closure discovery. Omega supplies the exact
/// reconciled direct-dependency predicate without leaking resolver or lockfile
/// structures into the language layer.
pub trait BuildTimeSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: semantic_vocabulary::PackageKeyIdentity,
        owner: semantic_vocabulary::PackageKeyIdentity,
    ) -> bool;

    fn package_label(&self, identity: semantic_vocabulary::PackageKeyIdentity) -> String;
}

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

pub struct BuildTimeAdmissionPlan {
    service_reaches: flow_effects::ServiceReachInferencePlan,
    suspension: Vec<BuildTimeSuspensionRow>,
    blocking: Vec<BuildTimeBlockingRow>,
    call_edges: Vec<BuildTimeCallEdge>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    selected_operators: Vec<crate::SelectedBuildTimeBinaryOperator>,
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
        }
    }

    /// Provisional typing retains equation obligations through specialization.
    /// A wrapper cannot execute a pending callee merely because its own
    /// signature has no type binders.
    pub(crate) fn require_discharged_structural_equations(
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
            let Some(machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
            else {
                continue;
            };
            if machine.structural_type_equations_pending {
                return Err(format!(
                    "machine `{}` retains an undischarged structural type equation",
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
            let boundary_use = |selected_operator_symbol| {
                program.operators().iter().any(|operator| {
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
        let service_summary = self
            .service_reaches
            .for_machine(machine.symbol)
            .ok_or_else(|| {
                format!(
                    "machine `{}` has no inferred service-reach summary",
                    machine.name
                )
            })?;
        let transitive_may_suspend = self.machine_suspension(machine.symbol).ok_or_else(|| {
            format!(
                "machine `{}` has no inferred operational summary",
                machine.name
            )
        })?;
        let transitive_may_block = self.machine_blocking(machine.symbol).ok_or_else(|| {
            format!(
                "machine `{}` has no inferred operational summary",
                machine.name
            )
        })?;

        let services = self
            .service_reaches
            .services(service_summary.effective)
            .iter()
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .ok_or_else(|| {
                        format!(
                            "machine `{}` reaches an unknown canonical service identity",
                            machine.name
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.require_discharged_structural_equations(program, machine.symbol)?;
        let closure_violation = checked_closure_violation(
            &self.call_edges,
            program,
            machine,
            discharge_authored_requires,
        );
        let selection_violation = selection_authority_violation(
            &self.call_edges,
            program,
            machine,
            custody,
            self.selection_authority.as_deref(),
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
        program: &TypedTrees,
        machine_name: &str,
        arguments: Vec<BuildTimeValue>,
    ) -> Result<BuildTimeValue, BuildTimeAdmissionRejection> {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor(program, machine)?;
        checked_interpreter::evaluate_build_time_machine(
            program,
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
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor_for_invocation(program, machine, custody)?;
        checked_interpreter::evaluate_build_time_machine(
            program,
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
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .ok_or_else(|| {
                format!(
                    "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
                )
            })?;
        self.require_common_floor_for_invocation(program, machine, custody)?;
        checked_interpreter::evaluate_build_time_machine(
            program,
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
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .ok_or_else(|| {
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
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor(program, machine)?;
        let value = checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
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
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor_for_invocation(program, machine, custody)?;
        let value = checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
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
        if discharge_authored_requires {
            self.require_common_floor_for_concrete_premise_invocation(program, machine, custody)?;
        } else {
            self.require_common_floor_for_invocation(program, machine, custody)?;
        }
        let measured = checked_interpreter::evaluate_build_time_machine(
            program,
            BuildMachineEvaluationRequest {
                operators: &self.selected_operators,
                ..BuildMachineEvaluationRequest::symbol(machine_symbol, arguments)
            },
        )
        .map(BuildTimeOperationEvaluation::into_measured)?;
        require_const_evaluable_result(program, machine, measured.value())?;
        Ok(measured)
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

#[cfg(test)]
mod tests {
    use super::{
        BuildTimeAdmissionPlan, BuildTimeCallEdge, SymbolHandle, project_operational_axes,
    };
    use arena::HandleSpan;
    use flow_effects::{CallOperational, MachineOperational, OperationalPlan, StateOperational};

    #[test]
    fn admission_projection_keeps_axes_and_call_topology_independent() {
        let suspending_machine = SymbolHandle::from_arena_index(1);
        let blocking_machine = SymbolHandle::from_arena_index(2);
        let source_state = SymbolHandle::from_arena_index(3);
        let target_state = SymbolHandle::from_arena_index(4);
        let mut operational = OperationalPlan::default();

        let mut calls = HandleSpan::empty();
        operational.calls.append_to_span(
            &mut calls,
            CallOperational {
                statement_index: 7,
                call_ordinal: 2,
                target_state_symbol: target_state,
                ..Default::default()
            },
        );
        let mut suspending_states = HandleSpan::empty();
        operational.states.append_to_span(
            &mut suspending_states,
            StateOperational {
                symbol: source_state,
                calls,
                ..Default::default()
            },
        );
        operational.machines.append_to_span(
            &mut operational.root_machines,
            MachineOperational {
                symbol: suspending_machine,
                transitive_may_suspend: true,
                transitive_may_block: false,
                states: suspending_states,
                ..Default::default()
            },
        );
        operational.machines.append_to_span(
            &mut operational.root_machines,
            MachineOperational {
                symbol: blocking_machine,
                transitive_may_suspend: false,
                transitive_may_block: true,
                ..Default::default()
            },
        );

        let (suspension, blocking, call_edges) = project_operational_axes(&operational);
        let admission = BuildTimeAdmissionPlan {
            service_reaches: Default::default(),
            suspension,
            blocking,
            call_edges,
            selection_authority: None,
            selected_operators: Vec::new(),
        };

        assert_eq!(admission.machine_suspension(suspending_machine), Some(true));
        assert_eq!(admission.machine_blocking(suspending_machine), Some(false));
        assert_eq!(admission.machine_suspension(blocking_machine), Some(false));
        assert_eq!(admission.machine_blocking(blocking_machine), Some(true));
        assert_eq!(
            admission.call_edges,
            [BuildTimeCallEdge {
                source_machine_symbol: suspending_machine,
                target_machine_symbol: SymbolHandle::invalid(),
                target_state_symbol: target_state,
                target_operator_symbol: SymbolHandle::invalid(),
            }]
        );

        let unknown = SymbolHandle::from_arena_index(99);
        assert_eq!(admission.machine_suspension(unknown), None);
        assert_eq!(admission.machine_blocking(unknown), None);
    }
}
