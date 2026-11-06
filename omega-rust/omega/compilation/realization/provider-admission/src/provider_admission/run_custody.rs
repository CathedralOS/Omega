//! Empty-run release transfers existing backing without calling a provider.
//!
//! Only the source-authenticated core operation receives this realization.
//! Its checked precondition proves zero cells, including zero-footprint cells;
//! the native operation transports the original storage descriptor unchanged.

use super::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedTerminalMechanism,
    NativeRealizationInput, SettledCompilerBuiltin, TerminalAuthorityPolicy,
};
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use terminal_psi::CheckedBoundaryExecution;

pub(super) fn settle_empty_releases<'execution>(
    input: &NativeRealizationInput,
    policy: &TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let module = input.module();
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    let mut reached: Vec<_> = input
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            abstract_operations::AbstractOperation::BoundaryCall { boundary, .. } => {
                Some(*boundary)
            }
            _ => None,
        })
        .collect();
    reached.sort_unstable();
    reached.dedup();
    for boundary_id in reached {
        let Some(boundary) = module.boundary_machine(boundary_id) else {
            return Err(vec![Diagnostic::error(
                "reached boundary has no Terminal declaration",
            )]);
        };
        if boundary.declaration.as_ref().map(|row| row.execution)
            != Some(CheckedBoundaryExecution::EmptyRunRelease)
        {
            continue;
        }
        let fail = |reason: &str| {
            vec![Diagnostic::error(format!(
                "empty-run release {:?}: {reason}",
                boundary.id,
            ))]
        };
        terminal_semantics::empty_run_release_contract(&module.structural_types, boundary)
            .map_err(fail)?;
        let mechanism = CompilerIntrinsicExecutionIdentity::EmptyRunRelease;
        policy.classify(mechanism).map_err(|error| {
            fail(&format!(
                "missing execution classification for {:?}",
                error.mechanism(),
            ))
        })?;
        let settled = SettledCompilerBuiltin::from_identity(mechanism, boundary.id, None)
            .ok_or_else(|| fail("missing compiler execution catalog entry"))?;
        settlements.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(settled),
            realization: target_operations::BoundarySettlementRealization::Builtin(
                target_operations::BoundaryRealization::EmptyRunRelease,
            ),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: boundary.id,
            mechanism: mechanism.into(),
        });
    }
    Ok((settlements, mechanisms))
}
