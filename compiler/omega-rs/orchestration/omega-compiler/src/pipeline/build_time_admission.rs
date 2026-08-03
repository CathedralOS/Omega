//! Shared admission for machines the compiler executes during build-time
//! evaluation.
//!
//! This is the normalized service/operational floor of the complete
//! build-time contract: a compiler-run machine must reach no boundary service
//! and must neither suspend nor block, and every checked body in its concrete
//! call closure must carry the ordinary termination guarantee. Authority,
//! trust, resources, failure, and escaping-mutation admission remain
//! independent axes and are added here as their checked plans become
//! available.

use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;

pub(super) struct BuildTimeAdmissionPlan {
    operational: psi_effects::OperationalPlan,
    service_reaches: psi_effects::ServiceReachInferencePlan,
}

impl BuildTimeAdmissionPlan {
    pub(super) fn infer(program: &TypedTrees) -> Self {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        Self {
            operational,
            service_reaches,
        }
    }

    pub(super) fn require_common_floor(
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
        let operational_summary = self
            .operational
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine.symbol)
            .ok_or_else(|| {
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
        let termination_violation = self.termination_violation(program, machine);
        if services.is_empty()
            && !operational_summary.transitive_may_suspend
            && !operational_summary.transitive_may_block
            && termination_violation.is_none()
        {
            return Ok(());
        }

        let mut violations = Vec::new();
        if !services.is_empty() {
            violations.push(format!("service reach [{}]", services.join(", ")));
        }
        if operational_summary.transitive_may_suspend {
            violations.push("may suspend".to_owned());
        }
        if operational_summary.transitive_may_block {
            violations.push("may block".to_owned());
        }
        if let Some(violation) = termination_violation {
            violations.push(violation);
        }

        Err(format!(
            "machine `{}` is not build-time admissible: {}; build-time evaluation requires empty service reach, no possible suspension or blocking, and ordinary checked termination across the complete call closure",
            machine.name,
            violations.join("; ")
        ))
    }

    fn termination_violation(&self, program: &TypedTrees, root: &Machine) -> Option<String> {
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

        let operational = self
            .operational
            .machines()
            .iter()
            .find(|summary| summary.symbol == machine_symbol);
        if let Some(operational) = operational {
            for state in self.operational.states.span_or_empty(operational.states) {
                for call in self.operational.calls.span_or_empty(state.calls) {
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
                        callable_termination_violation(program, call.target_state_symbol, path)
                    {
                        active.retain(|active_symbol| *active_symbol != machine_symbol);
                        path.pop();
                        return Some(violation);
                    }
                }
            }
        }

        active.retain(|active_symbol| *active_symbol != machine_symbol);
        completed.push(machine_symbol);
        path.pop();
        None
    }
}

fn callable_termination_violation(
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
    if signature.terminates_guarantee {
        return None;
    }

    let name = signature.name.as_str();
    Some(format!(
        "callable contract `{name}` reached from `{}` publishes no `Terminates` guarantee",
        path.join(" -> ")
    ))
}
