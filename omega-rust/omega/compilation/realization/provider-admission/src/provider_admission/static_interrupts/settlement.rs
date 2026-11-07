//! Settle only operations whose exact installation or arrival was admitted.
//! A recognized declaration alone never supplies a native implementation.
use super::*;
use crate::provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedTerminalMechanism,
    SettledCompilerBuiltin, TerminalAuthorityPolicy,
};
use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::{BoundaryMachineId, OperationId};

pub(in super::super) fn settle<'execution>(
    input: &NativeRealizationInput<'_>,
    cohort: &AdmittedInstalledInterruptCohort,
    policy: &TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'execution>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let fail = |reason: &str| {
        vec![Diagnostic::error(format!(
            "installed interrupt execution: {reason}"
        ))]
    };
    if cohort.psi() != input.plan().psi {
        return Err(fail("cohort belongs to another Terminal program"));
    }
    let mut scopes = cohort
        .scopes()
        .iter()
        .map(|scope| (scope.operation(), scope))
        .collect::<Vec<_>>();
    scopes.sort_unstable_by_key(|(operation, _)| *operation);
    if scopes.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(fail("installation operation has duplicate scopes"));
    }
    let mut helpers = cohort
        .helpers()
        .iter()
        .map(|helper| (helper.operation(), helper.kind()))
        .collect::<Vec<_>>();
    helpers.sort_unstable();
    if helpers
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
    {
        return Err(fail("arrival helper has conflicting operation kinds"));
    }
    helpers.dedup();
    let mut boundaries = input
        .plan()
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect::<Vec<_>>();
    boundaries.sort_unstable_by_key(|(boundary, _)| *boundary);
    if boundaries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(fail("boundary declaration is duplicated"));
    }
    let mut executions: Vec<(BoundaryMachineId, StaticInterruptOperation)> = Vec::new();
    for row in input.plan().operations() {
        let abstract_operations::AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            ..
        } = row
        else {
            continue;
        };
        let declaration = boundaries
            .binary_search_by_key(boundary, |(id, _)| *id)
            .ok()
            .and_then(|position| boundaries[position].1.declaration.as_ref());
        let Some(kind) =
            declaration.and_then(|declaration| declaration.execution.static_interrupt_operation())
        else {
            continue;
        };
        match kind {
            StaticInterruptOperation::Install => {
                let scope = scopes
                    .binary_search_by_key(psi_operation, |(operation, _)| *operation)
                    .ok()
                    .map(|position| scopes[position].1)
                    .ok_or_else(|| fail("reached installation has no exact admitted scope"))?;
                if scope.boundary() != *boundary {
                    return Err(fail("installation changed its admitted boundary"));
                }
            }
            _ => {
                let position = helpers
                    .binary_search_by_key(psi_operation, |(operation, _): &(OperationId, _)| {
                        *operation
                    })
                    .map_err(|_| fail("reached helper has no admitted arrival association"))?;
                if helpers[position].1 != kind {
                    return Err(fail("helper changed its admitted kind"));
                }
            }
        }
        executions.push((*boundary, kind));
    }
    executions.sort_unstable();
    if executions
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
    {
        return Err(fail("boundary has conflicting installed operation kinds"));
    }
    executions.dedup();
    let mut settlements = Vec::with_capacity(executions.len());
    let mut mechanisms = Vec::with_capacity(executions.len());
    for (boundary, operation) in executions {
        let identity = effects::CompilerIntrinsicExecutionIdentity::StaticInterrupt(operation);
        policy.classify(identity).map_err(|error| {
            fail(&format!(
                "operation has no receiving physical classification: {error:?}"
            ))
        })?;
        let settled = SettledCompilerBuiltin::from_identity(identity, boundary, None)
            .ok_or_else(|| fail("operation is absent from the compiler execution catalog"))?;
        settlements.push(AdmittedBoundarySettlement {
            boundary,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(settled),
            realization: target_operations::BoundarySettlementRealization::Builtin(
                target_operations::BoundaryRealization::StaticInterrupt {
                    boundary,
                    operation,
                },
            ),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary,
            mechanism: identity.into(),
        });
    }
    Ok((settlements, mechanisms))
}
