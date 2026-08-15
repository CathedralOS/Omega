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

use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;

use crate::BuildTimeValue;

pub struct BuildTimeAdmissionPlan {
    service_reaches: psi_effects::ServiceReachInferencePlan,
    suspension: Vec<BuildTimeSuspensionRow>,
    blocking: Vec<BuildTimeBlockingRow>,
    call_edges: Vec<BuildTimeCallEdge>,
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
}

impl BuildTimeAdmissionPlan {
    pub fn infer(program: &TypedTrees) -> Self {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        let (suspension, blocking, call_edges) = project_operational_axes(&operational);
        Self {
            service_reaches,
            suspension,
            blocking,
            call_edges,
        }
    }

    pub fn require_common_floor(
        &self,
        program: &TypedTrees,
        machine: &Machine,
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
        let closure_violation = self.checked_closure_violation(program, machine);
        if services.is_empty()
            && !transitive_may_suspend
            && !transitive_may_block
            && closure_violation.is_none()
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

        Err(format!(
            "machine `{}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, and no unadmitted linear runtime carrier across the complete call closure",
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

    fn checked_closure_violation(&self, program: &TypedTrees, root: &Machine) -> Option<String> {
        let mut completed = Vec::new();
        let mut active = Vec::new();
        let mut path = Vec::new();
        self.machine_termination_violation(
            program,
            root.symbol,
            &mut completed,
            &mut active,
            &mut path,
        )
    }

    fn machine_termination_violation(
        &self,
        program: &TypedTrees,
        machine_symbol: SymbolHandle,
        completed: &mut Vec<SymbolHandle>,
        active: &mut Vec<SymbolHandle>,
        path: &mut Vec<String>,
    ) -> Option<String> {
        if completed.contains(&machine_symbol) {
            return None;
        }
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)?;
        path.push(machine.name.as_str().to_owned());

        if let Some(violation) = machine_precondition_violation(program, machine, path) {
            path.pop();
            return Some(violation);
        }
        if let Some(violation) = machine_linear_carrier_violation(program, machine, path) {
            path.pop();
            return Some(violation);
        }

        if active.contains(&machine_symbol) {
            let violation = format!(
                "recursive machine-call cycle has no ordinary termination proof along `{}`",
                path.join(" -> ")
            );
            path.pop();
            return Some(violation);
        }
        active.push(machine_symbol);

        let locally_terminates = machine.supply_mode == MachineSupplyMode::CheckedBody
            && matches!(
                psi_typed_trees_to_checked_trees::infer_machine_termination_summary(
                    program,
                    machine_symbol
                ),
                Some(TerminationGuarantee::Terminates { .. })
            );
        if !locally_terminates {
            let violation = format!(
                "machine call path `{}` has no ordinary checked `Terminates` guarantee",
                path.join(" -> ")
            );
            active.retain(|active_symbol| *active_symbol != machine_symbol);
            path.pop();
            return Some(violation);
        }

        for call in self
            .call_edges
            .iter()
            .filter(|call| call.source_machine_symbol == machine_symbol)
        {
            let target_machine_symbol = if call.target_machine_symbol.is_valid() {
                Some(call.target_machine_symbol)
            } else if call.target_state_symbol.is_valid()
                && program.symbols.get(call.target_state_symbol).kind == SymbolKind::Machine
            {
                // Unmeasured terminal recursion deliberately remains a
                // machine-symbol call until validation can diagnose the
                // missing measure. Semantic evaluation runs earlier,
                // so its admission closure must retain that edge too.
                Some(call.target_state_symbol)
            } else {
                None
            };
            if let Some(target_machine_symbol) = target_machine_symbol {
                if let Some(violation) = self.machine_termination_violation(
                    program,
                    target_machine_symbol,
                    completed,
                    active,
                    path,
                ) {
                    active.retain(|active_symbol| *active_symbol != machine_symbol);
                    path.pop();
                    return Some(violation);
                }
            } else if let Some(violation) =
                callable_contract_violation(program, call.target_state_symbol, path)
            {
                active.retain(|active_symbol| *active_symbol != machine_symbol);
                path.pop();
                return Some(violation);
            }
        }

        active.retain(|active_symbol| *active_symbol != machine_symbol);
        completed.push(machine_symbol);
        path.pop();
        None
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
                });
            }
        }
    }
    (suspension, blocking, call_edges)
}

fn machine_precondition_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    if has_authored_requires(program.machine_contracts(machine)) {
        return Some(format!(
            "machine `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            machine.name,
            path.join(" -> ")
        ));
    }
    program.machine_states(machine).iter().find_map(|state| {
        has_authored_requires(program.state_contracts(state)).then(|| {
            format!(
                "state `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                state.name,
                path.join(" -> ")
            )
        })
    })
}

fn has_authored_requires(contracts: &[psi_typed_trees::signature::SignatureContract]) -> bool {
    contracts.iter().any(|contract| {
        contract.kind == psi_typed_trees::signature::SignatureContractKind::Requires
    })
}

fn machine_linear_carrier_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    let describe = |context: &str, type_reference| {
        (program.type_multiplicity(type_reference)
            == psi_language_semantics::Multiplicity::Linear)
            .then(|| {
                format!(
                    "{context} has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                    program.display_type_reference(type_reference),
                    path.join(" -> ")
                )
            })
    };

    if let Some(attached_data) = machine.attached_data.as_ref()
        && program.data_definitions().iter().any(|definition| {
            definition.name.as_str() == attached_data.as_str()
                && definition.properties.multiplicity
                    == psi_language_semantics::Multiplicity::Linear
        })
    {
        return Some(format!(
            "machine instance `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            machine.name,
            attached_data,
            path.join(" -> ")
        ));
    }

    for owned in program.machine_owned_data(machine) {
        if let Some(violation) = describe(
            &format!("machine-owned value `{}`", owned.name),
            owned.type_reference,
        ) {
            return Some(violation);
        }
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            if parameter.is_self {
                continue;
            }
            if let Some(violation) = describe(
                &format!("state `{}` parameter `{}`", state.name, parameter.name),
                parameter.type_reference,
            ) {
                return Some(violation);
            }
        }
        if let Some(violation) =
            describe(&format!("state `{}` result", state.name), state.return_type)
        {
            return Some(violation);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
                continue;
            };
            if let Some(violation) = describe(
                &format!("state `{}` local `{}`", state.name, local.name),
                local.type_reference,
            ) {
                return Some(violation);
            }
        }
    }

    None
}

fn callable_contract_violation(
    program: &TypedTrees,
    symbol: SymbolHandle,
    path: &[String],
) -> Option<String> {
    if !symbol.is_valid()
        || matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinFunction | SymbolKind::Operator
        )
    {
        return None;
    }

    let signature = program
        .machine_parameter_signature(symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program.traits().iter().find_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
            })
        });
    let signature = signature?;

    if has_authored_requires(program.state_signature_contracts(signature)) {
        return Some(format!(
            "callable contract `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            signature.name,
            path.join(" -> ")
        ));
    }

    for parameter in program.state_signature_parameters(signature) {
        if parameter.is_self {
            continue;
        }
        if program.type_multiplicity(parameter.type_reference)
            == psi_language_semantics::Multiplicity::Linear
        {
            return Some(format!(
                "callable contract `{}` parameter `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                signature.name,
                parameter.name,
                program.display_type_reference(parameter.type_reference),
                path.join(" -> ")
            ));
        }
    }
    if program.type_multiplicity(signature.return_type)
        == psi_language_semantics::Multiplicity::Linear
    {
        return Some(format!(
            "callable contract `{}` result has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            signature.name,
            program.display_type_reference(signature.return_type),
            path.join(" -> ")
        ));
    }

    if signature.terminates_guarantee {
        return None;
    }

    let name = signature.name.as_str();
    Some(format!(
        "callable contract `{name}` reached from `{}` publishes no `Terminates` guarantee",
        path.join(" -> ")
    ))
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
            }]
        );

        let unknown = SymbolHandle::from_arena_index(99);
        assert_eq!(admission.machine_suspension(unknown), None);
        assert_eq!(admission.machine_blocking(unknown), None);
    }
}
