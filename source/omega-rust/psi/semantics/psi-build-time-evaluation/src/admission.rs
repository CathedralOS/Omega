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

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::BuildTimeValue;

mod closure_validation;
mod const_evaluable;
mod selection_authority;

use closure_validation::checked_closure_violation;
use const_evaluable::require_const_evaluable_result;
use selection_authority::selection_authority_violation;

/// Package-neutral authority consulted before the compiler executes an
/// authored machine during early semantic evaluation.
///
/// Psi owns provenance and call-closure discovery. Omega supplies the exact
/// reconciled direct-dependency predicate without leaking resolver or lockfile
/// structures into the language layer.
pub trait BuildTimeSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: psi_core::PackageKeyIdentity,
        owner: psi_core::PackageKeyIdentity,
    ) -> bool;

    fn package_label(&self, identity: psi_core::PackageKeyIdentity) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTimeInvocationCustody {
    Source(psi_source::SourceSpan),
    Symbol(SymbolHandle),
}

pub struct BuildTimeAdmissionPlan {
    service_reaches: psi_effects::ServiceReachInferencePlan,
    suspension: Vec<BuildTimeSuspensionRow>,
    blocking: Vec<BuildTimeBlockingRow>,
    call_edges: Vec<BuildTimeCallEdge>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
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
    pub fn infer(program: &TypedTrees) -> Self {
        Self::infer_with_selection_authority(program, None)
    }

    pub fn infer_with_selection_authority(
        program: &TypedTrees,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
    ) -> Self {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        let (suspension, blocking, call_edges) = project_operational_axes(&operational);
        Self {
            service_reaches,
            suspension,
            blocking,
            call_edges,
            selection_authority,
        }
    }

    pub fn require_common_floor(
        &self,
        program: &TypedTrees,
        machine: &Machine,
    ) -> Result<(), String> {
        self.require_common_floor_with_custody(program, machine, None)
    }

    pub fn require_common_floor_for_invocation(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: BuildTimeInvocationCustody,
    ) -> Result<(), String> {
        self.require_common_floor_with_custody(program, machine, Some(custody))
    }

    fn require_common_floor_with_custody(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        custody: Option<BuildTimeInvocationCustody>,
    ) -> Result<(), String> {
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
        let closure_violation = checked_closure_violation(&self.call_edges, program, machine);
        let selection_violation = self.selection_authority.as_ref().and_then(|authority| {
            selection_authority_violation(
                &self.call_edges,
                program,
                machine,
                custody,
                authority.as_ref(),
            )
        });
        if services.is_empty()
            && !transitive_may_suspend
            && !transitive_may_block
            && closure_violation.is_none()
            && selection_violation.is_none()
        {
            return Ok(());
        }

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
            violations.push(violation);
        }

        Err(format!(
            "machine `{}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure",
            machine.name,
            violations.join("; ")
        ))
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
    ) -> Result<BuildTimeValue, String> {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
        self.require_common_floor(program, machine)?;
        psi_checked_interpreter::evaluate_build_time_machine(program, machine_name, arguments)
    }

    pub fn evaluate_machine_for_invocation(
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
        psi_checked_interpreter::evaluate_build_time_machine(program, machine_name, arguments)
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
        psi_checked_interpreter::evaluate_build_time_machine_symbol_measured(
            program,
            machine_symbol,
            arguments,
        )
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
        let value =
            psi_checked_interpreter::evaluate_build_time_machine(program, machine_name, arguments)?;
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
        let value =
            psi_checked_interpreter::evaluate_build_time_machine(program, machine_name, arguments)?;
        require_const_evaluable_result(program, machine, &value)?;
        Ok(value)
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
    operational: &psi_effects::OperationalPlan,
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
    use super::*;
    use psi_arena::HandleSpan;
    use psi_effects::{CallOperational, MachineOperational, OperationalPlan, StateOperational};

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
