//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal. The consuming
//! lowerer independently accepts it through its local target catalog.

use crate::pipeline::CheckedCompilation;
use omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity;
use psi_diagnostics::Diagnostic;
use std::collections::BTreeSet;

#[derive(Debug)]
pub(super) struct CompilerIntrinsicSettlementProposal {
    pub(super) requirement_identity: String,
    pub(super) plan_index: usize,
    pub(super) execution: omega_target_operations::CompilerBuiltinExecution,
}

pub(super) fn demanded_boundary_identities(
    module: &psi_terminal::TerminalModule,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let declarations = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let psi_terminal::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            continue;
        };
        let Some(identity) = declarations.get(boundary).copied() else {
            return Err(vec![Diagnostic::error(format!(
                "Terminal intrinsic settlement demand cites absent boundary {:?}",
                boundary,
            ))]);
        };
        demanded.insert(identity.to_owned());
    }
    Ok(demanded)
}

pub(super) fn derive_compiler_intrinsic_settlement_proposals(
    checked: &CheckedCompilation,
    demanded_boundaries: &BTreeSet<String>,
) -> Result<Vec<CompilerIntrinsicSettlementProposal>, Vec<Diagnostic>> {
    let plans = checked.selected_provider_plans().plans();
    let provenance = checked.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    for (plan, retained) in plans.iter().zip(provenance) {
        if retained.plan != *plan
            || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
    }

    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let matches = plans
            .iter()
            .zip(provenance)
            .enumerate()
            .flat_map(|(plan_index, (plan, retained))| {
                plan.rows
                    .iter()
                    .zip(&retained.row_compiler_intrinsic_executions)
                    .filter_map(move |(row, execution)| {
                        (row.requirement_identity == *requirement
                            && matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                                    ..
                                }
                            ))
                        .then_some((plan_index, plan, retained, row, *execution))
                    })
            })
            .collect::<Vec<_>>();
        let [(plan_index, plan, retained, _row, execution)] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows",
                    matches.len(),
                )));
            }
            continue;
        };
        debug_assert_eq!(retained.plan, **plan);
        let Some(execution) = execution else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no closed native catalog identity",
                plan.name,
            )));
            continue;
        };
        if !matches!(
            *execution,
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32
                | CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no native boundary realization",
                plan.name,
            )));
            continue;
        }
        let execution = match execution {
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
                omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32
            }
            CompilerIntrinsicExecutionIdentity::LinuxWriteByteI32 => {
                omega_target_operations::CompilerBuiltinExecution::LinuxWriteByteI32
            }
            _ => unreachable!("native boundary intrinsic was checked above"),
        };
        evidence.push(CompilerIntrinsicSettlementProposal {
            requirement_identity: requirement.clone(),
            plan_index: *plan_index,
            execution,
        });
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}
