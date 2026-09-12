//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal. The consuming
//! lowerer independently accepts it through its local target catalog.

use crate::pipeline::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use provider_planning::plans::CompilerIntrinsicExecutionIdentity;
use provider_planning::plans::SelectedProviderReviewProvenance;
use std::collections::BTreeSet;

#[derive(Debug)]
pub(super) struct CompilerIntrinsicSettlementProposal {
    pub(super) requirement_identity: String,
    pub(super) plan_index: usize,
    pub(super) execution: target_operations::CompilerBuiltinExecution,
}

pub(super) fn demanded_boundary_identities(
    module: &terminal_psi::TerminalModule,
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
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
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
    derive_selected_intrinsic_settlement_proposals(
        checked.selected_provider_plans().plans(),
        checked.selected_provider_provenance(),
        demanded_boundaries,
    )
}

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    plan_name: &'a str,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
}

fn derive_selected_intrinsic_settlement_proposals(
    plans: &[ProviderPlan],
    provenance: &[SelectedProviderReviewProvenance],
    demanded_boundaries: &BTreeSet<String>,
) -> Result<Vec<CompilerIntrinsicSettlementProposal>, Vec<Diagnostic>> {
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

    // Keep the row/execution association from its producing plan. Sorting only
    // this borrowed selection leaves plan identity and row ownership unchanged.
    let mut selected_rows = Vec::new();
    if !demanded_boundaries.is_empty() {
        for (plan_index, (plan, retained)) in plans.iter().zip(provenance).enumerate() {
            for (row, execution) in plan
                .rows
                .iter()
                .zip(&retained.row_compiler_intrinsic_executions)
            {
                if matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                ) {
                    selected_rows.push(SelectedIntrinsicRow {
                        requirement_identity: &row.requirement_identity,
                        plan_index,
                        plan_name: &plan.name,
                        execution: *execution,
                    });
                }
            }
        }
        selected_rows.sort_unstable_by(|left, right| {
            left.requirement_identity.cmp(right.requirement_identity)
        });
    }

    // Both sides are lexical. Each selected row is consumed once, including
    // complete duplicate groups, without allocating a match list per demand.
    let mut remaining_rows = selected_rows.as_slice();
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let preceding_count = remaining_rows
            .iter()
            .take_while(|row| row.requirement_identity < requirement.as_str())
            .count();
        remaining_rows = &remaining_rows[preceding_count..];
        let matching_count = remaining_rows
            .iter()
            .take_while(|row| row.requirement_identity == requirement)
            .count();
        let (matches, later_rows) = remaining_rows.split_at(matching_count);
        remaining_rows = later_rows;
        let [selected] = matches else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows",
                    matches.len(),
                )));
            }
            continue;
        };
        let Some(execution) = selected.execution else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no closed native catalog identity",
                selected.plan_name,
            )));
            continue;
        };
        let Some(execution) = compiler_builtin_execution(execution) else {
            diagnostics.push(Diagnostic::error(format!(
                "selected compiler intrinsic `{}` for Terminal boundary `{requirement}` has no native boundary realization",
                selected.plan_name,
            )));
            continue;
        };
        evidence.push(CompilerIntrinsicSettlementProposal {
            requirement_identity: requirement.clone(),
            plan_index: selected.plan_index,
            execution,
        });
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

fn compiler_builtin_execution(
    identity: CompilerIntrinsicExecutionIdentity,
) -> Option<target_operations::CompilerBuiltinExecution> {
    use target_operations::CompilerBuiltinExecution;
    match identity {
        CompilerIntrinsicExecutionIdentity::HostedExitProcessI32 => {
            Some(CompilerBuiltinExecution::HostedExitProcessI32)
        }
        CompilerIntrinsicExecutionIdentity::HostedWriteByteI32 => {
            Some(CompilerBuiltinExecution::HostedWriteByteI32)
        }
        CompilerIntrinsicExecutionIdentity::HostedReadByte => {
            Some(CompilerBuiltinExecution::HostedReadByte)
        }
        CompilerIntrinsicExecutionIdentity::BuiltinFunction(_)
        | CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. }
        | CompilerIntrinsicExecutionIdentity::NamedFloatNegation(_)
        | CompilerIntrinsicExecutionIdentity::NamedFloatConversion { .. } => None,
    }
}

#[cfg(test)]
mod tests;
