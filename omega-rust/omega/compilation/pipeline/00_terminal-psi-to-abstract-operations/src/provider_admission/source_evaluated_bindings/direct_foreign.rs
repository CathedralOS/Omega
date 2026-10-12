//! Demand closes concrete foreign realizations without selecting providers.
use diagnostics::Diagnostic;
use effects::{DirectForeignBinding, DirectForeignRealization};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Debug, Clone)]
pub struct PreparedDirectForeignBinding<'input> {
    pub(crate) realization: &'input Arc<DirectForeignRealization>,
    pub(crate) same_stack: Option<task_plans::AdmittedSameStackContribution>,
}

pub(crate) fn prepare_direct_foreign_bindings<'input>(
    candidates: &'input [Arc<DirectForeignRealization>],
    demanded: &BTreeSet<String>,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<PreparedDirectForeignBinding<'input>>, Vec<Diagnostic>> {
    let mut prepared = Vec::new();
    for realization in candidates {
        let callable = realization.callable_identity();
        if !demanded.contains(callable) {
            continue;
        }
        let same_stack = match realization.binding() {
            DirectForeignBinding::Import(_) => {
                let requirement = identities.index_of(callable).ok_or_else(|| vec![Diagnostic::error(format!("direct foreign callable `{callable}` is absent from its Terminal identity table"))])?;
                let identity = realization.identity();
                let report =
                    u64::from_le_bytes(identity[..8].try_into().expect("fixed digest prefix"))
                        .max(1);
                let commitment = task_plans::SameStackExecutionCommitment::DirectForeign(identity);
                let receipt =
                    task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                        report,
                    )
                    .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
                Some(
                    task_plans::admit_same_stack_contribution(
                        task_plans::SameStackContributionAdmissionCandidate {
                            provider_plan_report_identity: report,
                            provider_plan_commitment: commitment,
                            requirement_identity: requirement,
                            receipt,
                            bytes: super::SAME_STACK_BYTES,
                            alignment: super::SAME_STACK_ALIGNMENT,
                        },
                        report,
                        commitment,
                        requirement,
                    )
                    .map_err(|error| vec![Diagnostic::error(error.to_string())])?,
                )
            }
            DirectForeignBinding::Syscall(_) | DirectForeignBinding::CompilerIntrinsic { .. } => {
                None
            }
        };
        prepared.push(PreparedDirectForeignBinding {
            realization,
            same_stack,
        });
    }
    Ok(prepared)
}
