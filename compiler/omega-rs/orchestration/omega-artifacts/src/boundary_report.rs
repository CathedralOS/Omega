//! Human-readable boundary and capability-blast-radius presentation.

use psi_diagnostics::Diagnostic;

use super::{ArtifactWriter, BoundaryReport};

impl ArtifactWriter {
    pub fn write_boundary_report(
        &self,
        boundary_report: &BoundaryReport,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Boundary\n\n");
        output.push_str(&format!("targets: {}\n", boundary_report.targets.len()));
        output.push_str(&format!(
            "boundary contracts: {}\n",
            boundary_report.contracts.len()
        ));
        output.push_str(&format!(
            "unchecked policies: {}\n",
            boundary_report.unchecked_policies.len()
        ));
        output.push_str("## Targets\n");
        if boundary_report.targets.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, target) in boundary_report.targets.iter() {
                output.push_str(&format!(
                    "- target `{}` host `{}` settings {} checked boundaries {} unchecked boundaries {}\n",
                    target.name,
                    target.host_provider,
                    target.host_settings,
                    target.checked_boundaries,
                    target.unchecked_boundaries
                ));
            }
        }

        output.push_str("\n## Boundary Contracts\n");
        if boundary_report.contracts.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, contract) in boundary_report.contracts.iter() {
                output.push_str(&format!(
                    "- {}.{} boundary `{}` requires {} ensures {}\n",
                    contract.capability,
                    contract.state,
                    contract.boundary,
                    contract.requires_count,
                    contract.ensures_count
                ));
            }
        }

        output.push_str("\n## Unchecked Policies\n");
        if boundary_report.unchecked_policies.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, policy) in boundary_report.unchecked_policies.iter() {
                output.push_str(&format!(
                    "- target `{}` boundary unchecked `{}`\n",
                    policy.target, policy.name
                ));
            }
        }

        output.push_str("\n## Capability Blast Radius\n");
        if boundary_report.capability_blast_radius.is_empty() {
            output.push_str("none\n");
        } else {
            for (_, radius) in boundary_report.capability_blast_radius.iter() {
                let provider = if radius.approved_provider {
                    "approved provider"
                } else {
                    "in-package provider"
                };
                output.push_str(&format!(
                    "- capability `{}` [{}] authority is the capability value; uses {} acquires {} returns {} stores {} derives {}\n",
                    radius.capability,
                    provider,
                    radius.uses,
                    radius.acquires,
                    radius.returns,
                    radius.stores,
                    radius.derives,
                ));
                for flow in &radius.flows {
                    output.push_str(&format!(
                        "  - `{}` [{}] {} at statement {} call {}",
                        flow.state,
                        flow.machine_overload_identity,
                        flow.authority_flow,
                        flow.statement_index,
                        flow.call_ordinal,
                    ));
                    if let Some(via) = &flow.via {
                        output.push_str(&format!(
                            " via `{}` [{}]",
                            via.state, via.machine_overload_identity
                        ));
                    } else {
                        output.push_str(" direct");
                    }
                    output.push('\n');
                }
            }
        }

        self.write_html_report("10_boundary.html", "boundary", &output)
    }
}
