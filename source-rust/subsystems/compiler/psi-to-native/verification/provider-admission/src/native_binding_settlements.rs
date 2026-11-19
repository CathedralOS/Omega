//! Produce physical settlements for the Build bindings reached by this execution.

use native_binding_admission::CheckedNativeBinding;
use provider_plans::EvaluatedNativeBinding;

pub(crate) fn settle<'execution>(
    input: &crate::AdmissionSource<'_>,
    request: &crate::ProviderAdmissionRequest<'_, 'execution>,
) -> Result<
    (
        Vec<crate::AdmittedBoundarySettlement<'execution>>,
        Vec<crate::AdmittedTerminalMechanism>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    use crate::AdmittedBoundaryExecution;
    use target_operations::{
        BoundarySettlementRealization, NormalizedForeignCallBinding,
        NormalizedForeignCallMechanism, NormalizedForeignExecution, NormalizedSystemCallBinding,
    };
    let retained = request.native_bindings;
    let refuse = |reason: String| {
        vec![diagnostics::Diagnostic::error(format!(
            "native Build binding admission: {reason}"
        ))]
    };
    let mut settlements = Vec::new();
    let mut mechanisms = Vec::new();
    let mut boundaries = retained
        .rows
        .iter()
        .map(|row| row.boundary)
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    if boundaries.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(refuse("duplicate exact boundary supply".into()));
    }
    for row in &retained.rows {
        let module = input.module();
        let contract =
            CheckedNativeBinding::admit(row, &retained.identities, module, request.profile)
                .map_err(refuse)?;
        let requirement = module
            .identities
            .text(contract.boundary().identity)
            .ok_or_else(|| refuse("boundary has no terminal identity".into()))?;
        if request
            .selected_provider_plans
            .selected_requirement_row(requirement)
            .is_some()
            || request
                .compiler_builtins
                .iter()
                .any(|builtin| builtin.boundary == row.boundary)
        {
            return Err(refuse(
                "boundary has competing native and provider executions".into(),
            ));
        }
        // The module may retain choices for other roots. Only the actual
        // admitted execution scope produces a physical settlement here.
        if input.plan().boundary_machine(row.boundary).is_none() {
            continue;
        }
        let abi = module.calling_abi_identity(row.boundary).map_err(refuse)?;
        let mut applications = request
            .installed_calling
            .iter()
            .filter(|application| application.abi == abi);
        let application = applications
            .next()
            .filter(|_| applications.next().is_none())
            .ok_or_else(|| refuse("binding has no unique exact calling application".into()))?;
        let execution = contract.execution(application).map_err(refuse)?;
        let receipt = row.binding.receipt().identity_digest();
        let entry = application.plan.plan().clone();
        let (mechanism, physical) = match &row.binding {
            EvaluatedNativeBinding::Import(import) => {
                if entry.call.policy
                    != calling_conventions::CallingPolicy::native_for_target(request.target())
                {
                    return Err(refuse("import has another target calling policy".into()));
                }
                if request.profile.program_entry_slot().schema
                    != target::ProgramEntrySchema::HostedApplication
                {
                    return Err(refuse(
                        "foreign import needs admitted demand on a fixed or freestanding stack"
                            .into(),
                    ));
                }
                let physical = crate::normalized_foreign_terminal_mechanism(
                    import.locator(),
                    &entry,
                )
                .map_err(refuse)?;
                (
                    NormalizedForeignCallMechanism::HostedImport {
                        locator: import.locator().clone(),
                        receipt,
                        requirement_identity: requirement.to_owned(),
                    },
                    physical,
                )
            }
            EvaluatedNativeBinding::Syscall(syscall) => {
                let policy =
                    calling_conventions::CallingPolicy::native_syscall_for_target(request.target())
                        .ok_or_else(|| refuse("syscall target is unsupported".into()))?;
                let signature = calling_conventions::CallSignature::of_placements(
                    &entry.call.parameters,
                    entry.call.result.as_ref(),
                );
                if calling_conventions::evaluate_call_plan(policy, &signature).as_ref()
                    != Ok(&entry.call)
                {
                    return Err(refuse(
                        "syscall has another exact kernel calling policy".into(),
                    ));
                }
                let physical = crate::conservative_syscall_terminal_mechanism(
                    request.profile,
                    syscall.number(),
                    input.plan(),
                    row.boundary,
                    input.closed_module(),
                )
                .map_err(refuse)?;
                (
                    NormalizedForeignCallMechanism::SystemCall(NormalizedSystemCallBinding {
                        target: request.profile,
                        number: syscall.number(),
                        requirement_identity: requirement.to_owned(),
                        execution: NormalizedForeignExecution::NativeBinding(receipt),
                    }),
                    physical,
                )
            }
        };
        let foreign = NormalizedForeignCallBinding {
            boundary_entry_plan: entry,
            mechanism,
        };
        if !foreign.has_valid_entry_mechanism() {
            return Err(refuse(
                "binding has an invalid physical entry mechanism".into(),
            ));
        }
        settlements.push(crate::AdmittedBoundarySettlement {
            boundary: row.boundary,
            execution: AdmittedBoundaryExecution::NativeBinding {
                execution,
                schema: row.schema.identity_digest(),
            },
            realization: BoundarySettlementRealization::NormalizedForeignCall(foreign),
        });
        mechanisms.push(crate::AdmittedTerminalMechanism {
            boundary: row.boundary,
            mechanism: physical,
        });
    }
    Ok((settlements, mechanisms))
}
