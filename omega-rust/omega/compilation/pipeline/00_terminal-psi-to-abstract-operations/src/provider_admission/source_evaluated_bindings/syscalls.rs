//! Derive execution coordinates for demanded, uniquely selected kernel leaves.
//! These records are proposals: settlement independently rejoins the selected
//! provider, retained external row and validated supervisor-entry plan.

use super::{SourceEvaluatedBindingExecution, diagnostic, fnv1a, nonzero};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use std::collections::BTreeSet;

#[derive(Debug)]
pub(super) struct PreparedSystemCall<'plan> {
    pub plan: &'plan ProviderPlan,
    pub execution: SourceEvaluatedBindingExecution,
}

pub(super) fn prepare_system_calls<'plan>(
    plans: &'plan [ProviderPlan],
    demanded: &BTreeSet<String>,
) -> Result<Vec<PreparedSystemCall<'plan>>, Vec<Diagnostic>> {
    let mut prepared = Vec::new();
    for plan in plans {
        for row in &plan.rows {
            let ProviderBinding::Syscall { number } = row.binding else {
                continue;
            };
            let requirement = row.requirement_identity.as_str();
            if !demanded.contains(requirement) {
                continue;
            }
            let count = plans
                .iter()
                .flat_map(|plan| &plan.rows)
                .filter(|row| row.requirement_identity == requirement)
                .count();
            if count != 1 {
                return Err(diagnostic(
                    "source-evaluated syscall",
                    format!(
                        "demanded requirement `{requirement}` resolves to {count} selected rows"
                    ),
                ));
            }
            let plan_report_identity = plan.report_fingerprint();
            let commitment = plan.identity_digest();
            let execution = SourceEvaluatedBindingExecution {
                requirement_identity: requirement.to_owned(),
                provider_plan_report_identity: plan_report_identity,
                provider_execution_report_identity: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-execution",
                    requirement.as_bytes(),
                    commitment.as_bytes(),
                ])),
                provider_execution_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-fingerprint",
                    requirement.as_bytes(),
                    commitment.as_bytes(),
                    &number.to_le_bytes(),
                ])),
                boundary_contract_report_fingerprint: nonzero(fnv1a(&[
                    b"source-evaluated-syscall-contract",
                    commitment.as_bytes(),
                    &number.to_le_bytes(),
                ])),
            };
            prepared.push(PreparedSystemCall { plan, execution });
        }
    }
    Ok(prepared)
}
