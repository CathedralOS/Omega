//! Shared admission for machines the compiler executes during build-time
//! evaluation.
//!
//! This is the normalized service/operational floor of the complete
//! build-time contract: a compiler-run machine must reach no boundary service
//! and must neither suspend nor block. Authority, trust, resources, failure,
//! termination, and escaping-mutation admission remain independent axes and
//! are added here as their checked plans become available.

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

    pub(super) fn require_service_and_operational_floor(
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
        if services.is_empty()
            && !operational_summary.transitive_may_suspend
            && !operational_summary.transitive_may_block
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

        Err(format!(
            "machine `{}` is not build-time admissible: {}; build-time evaluation requires empty service reach and no possible suspension or blocking",
            machine.name,
            violations.join("; ")
        ))
    }
}
