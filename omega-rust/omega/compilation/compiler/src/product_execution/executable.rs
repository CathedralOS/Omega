//! Prepare executable output from the accepted program and settled providers.
//! The entry receipt, arrival plans and permission rows all describe that same
//! artifact; application completion owns native compilation and publication.

use crate::terminal::ProducedTerminalProduct;
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use std::sync::Arc;

pub(super) fn prepare(
    checked: &CheckedCompilation,
    produced: ProducedTerminalProduct,
    selections: &OptimizationSelections,
) -> Result<
    (
        compilation_products::RetainedTerminalArtifact,
        native_compiler::TerminalAuthorityPermissionPolicy,
    ),
    Vec<Diagnostic>,
> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let program = &produced.program;
    let artifact = program.artifact();
    let module = artifact.semantic_module();
    let execution = checked.execution();
    let realization = execution.realization();
    let profile = execution
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
    let declarations = realization.provider_declarations();
    let mut entry = checked.program_entry_with_calling_plans(
        program,
        &declarations,
        build_settings::CALLING_POLICY_EVALUATOR,
    )?;
    entry.bind_physical_adapter(module, &declarations)?;
    let selected = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        profile.target_name(),
    );
    let calls = checked_provider_calls(module, &declarations)?;
    let root = module
        .program_entry_contract(profile.target_name())
        .map_err(refuse)?
        .entry;
    let closure = terminal_installation::SynchronousCallGraph::new(module, Some(&selected), None)
        .and_then(|graph| graph.with_boundary_calls(calls))
        .and_then(|graph| graph.from_root(root))
        .map_err(refuse)?;
    let active_operation = |identity| {
        closure.machines.iter().any(|machine| {
            machine
                .operations()
                .any(|operation| operation.id == identity)
        })
    };
    if module
        .callback_applications
        .iter()
        .any(|callback| active_operation(callback.registrar))
        || module
            .installed_interrupt_registrations
            .iter()
            .any(|registration| active_operation(registration.operation))
    {
        return Err(refuse(
            "executable callback and interrupt entries need their canonical composition roots before application preparation".into(),
        ));
    }
    let custody = psi_compiler::TerminalApplicationCustody::prepare(
        entry.nominal_root(),
        Some(entry.source_signature().identity().bytes()),
        Some(psi_compiler::TerminalCompositionRoots {
            callbacks: None,
            physical_adapter: entry.physical_adapter(),
        }),
    );
    let application =
        program.attach_application(&custody, &mut psi_compiler::UnmeasuredProduction)?;
    let receipt = application
        .receipt?
        .ok_or_else(|| refuse("executable application has no checked entry receipt".into()))?;
    let mut roots = Vec::with_capacity(application.roots.len());
    for root in application.roots {
        let root = root?;
        roots.push(
            module
                .scalar_applications
                .proof_scope
                .callable_declaration(terminal_psi::TargetMachineCallable::Machine(root.machine))
                .ok_or_else(|| {
                    refuse("executable root has no actual checked declaration".into())
                })?,
        );
    }
    roots.sort_unstable();
    roots.dedup();
    let mut provider_proposal = produced.proposal;
    provider_proposal.application_roots = Some(roots);
    let plans = entry
        .calling_plans()
        .ok_or_else(|| refuse("executable entry lost its paired arrival plans".into()))?;
    let provenance = declarations
        .iter()
        .zip(realization.row_executions())
        .map(|(declaration, executions)| {
            let schema = realization
                .direct_boundary_leaves()
                .iter()
                .find(|leaf| leaf.plan == declaration.plan)
                .map_or(&declaration.plan.schema, |leaf| &leaf.satisfied_schema);
            native_compiler::CompilerIntrinsicPlanProvenance {
                plan: &declaration.plan,
                semantic_schema: schema,
                row_executions: executions,
                row_numeric_realizations: &declaration.row_numeric_realizations,
            }
        })
        .collect();
    let permissions = native_compiler::admit_package_permissions(
        realization.terminal_authority_permissions().to_vec(),
        realization
            .terminal_authority_permission_identities()
            .clone(),
    )?;
    let terminal = native_compiler::TerminalApplication {
        artifact: Arc::clone(artifact),
        receipt,
        provider_proposals: vec![provider_proposal],
        physical_entry_adapter: entry.physical_adapter().is_some(),
        evidence: program.evidence().cloned(),
    }
    .prepare(native_compiler::TerminalNativeProposalRequest {
        evaluated_binding_identities: realization.evaluated_binding_identities(),
        direct_boundary_leaves: realization.direct_boundary_leaves().iter().collect(),
        provider_declarations: &declarations,
        target_profile: Some(profile),
        family_realizations: realization.family_realizations(),
        boundaries: native_compiler::BoundaryApplicationSettlement {
            selected_provider_plans: realization.plans(),
            selected_provider_provenance: provenance,
            calling_applications: realization.calling_applications().to_vec(),
        },
        native_target: Some(profile.native_target()),
        program_entry: Some(native_compiler::NativeProgramEntrySettlement::new(
            entry.source_signature(),
            Some((
                &plans.semantic_calling_application,
                &plans.physical_calling_application,
                &plans.storage_entry,
            )),
        )),
        application: checked.native_application_metadata(),
        optimizations: selections.project_post_terminal(),
        external_bindings: realization.external_bindings(),
        behavior_exclusions: &checked.build().config.behavior_exclusions,
        package_terminal_authority_permissions: realization
            .terminal_authority_permissions()
            .to_vec(),
        package_terminal_authority_permission_identities: realization
            .terminal_authority_permission_identities()
            .clone(),
        x86_scalar_fma_provider: checked
            .build()
            .config
            .settings
            .x86_deployment_claim
            .admit_for(Some(profile))
            .map_err(refuse)?,
    })?;
    Ok((terminal, permissions))
}

fn checked_provider_calls(
    module: &terminal_psi::TerminalModule,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<
    Vec<(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::MachineId,
    )>,
    Vec<Diagnostic>,
> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let mut calls = Vec::new();
    for declaration in declarations {
        if !declaration.plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let owner = declaration.provider_declaration.as_deref().ok_or_else(|| {
            refuse("checked provider lost its exact selecting declaration".into())
        })?;
        for row in backend_plan::selected_provider_candidate_rows(&declaration.plan, owner, module)
            .map_err(refuse)?
        {
            for position in row.candidate_positions {
                let candidate = &module.provider_candidates[position];
                calls.push((candidate.boundary, candidate.candidate));
            }
        }
    }
    calls.sort_unstable();
    calls.dedup();
    Ok(calls)
}
