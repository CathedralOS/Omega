// Assemble the native request from this application's canonical artifact and
// completed entry plans. Checked provider rows keep their canonical adapters;
// exact core primitives supply direct native leaves and narrowly keyed host
// permission rows. Unsupported Build demands still refuse at their owner.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use native_compiler::{
    BoundaryApplicationSettlement, CompilerIntrinsicPlanProvenance, NativeProgramEntrySettlement,
    TerminalNativeProposalRequest, current_terminal_authority_policy,
};
use optimization_core::OptimizationSelections;
use source_compiler::ProducedTerminalApplication;

use super::ProducedTerminalProduct;

/// This receiving host is consuming its own in-process Psi producer, not a
/// caller-supplied module. Record that host decision once for the actual
/// producer and bind each exact product without encoding or checking it again.
fn authorize_produced_artifact(
    program: &source_compiler::PortableTerminalProgram,
) -> Result<terminal_installation::AuthorizedTerminalArtifact, Vec<Diagnostic>> {
    static PRODUCER: std::sync::OnceLock<terminal_installation::HostProducerTrust> =
        std::sync::OnceLock::new();
    let producer = PRODUCER.get_or_init(|| {
        terminal_installation::HostProducerTrust::accept("this host's in-process Psi compiler")
    });
    terminal_installation::authorize_terminal_artifact(
        std::sync::Arc::clone(program.artifact()),
        Some(producer),
    )
    .map_err(|error| vec![Diagnostic::error(error.to_string())])
}

pub(crate) fn prepare(
    checked: &CheckedCompilation,
    produced: ProducedTerminalProduct,
    entry: NativeProgramEntrySettlement<'_>,
    application: ProducedTerminalApplication,
    metadata: target::NativeApplicationMetadata,
    optimizations: &OptimizationSelections,
    request: Option<&products::NativeProductRequest>,
) -> Result<
    (
        products::RetainedTerminalArtifact,
        native_compiler::TerminalAuthorityPermissionPolicy,
        Option<Vec<u8>>,
    ),
    Vec<Diagnostic>,
> {
    let failure = |reason| vec![Diagnostic::error(reason)];
    let build = &checked.build().config;
    if !build.native_bindings.is_empty() {
        return Err(failure(
            "no product route yet: executable native bindings need their completed application plans",
        ));
    }
    if !checked.execution().plans().assumed_bindings().is_empty() {
        return Err(failure(
            "no product route yet: executable assumed provider bindings need their admitted applications",
        ));
    }
    if !checked
        .build()
        .independent_components()
        .selections
        .is_empty()
    {
        return Err(failure(
            "no product route yet: executable components need their accepted component inputs",
        ));
    }
    if !build.test_groups.is_empty() {
        return Err(failure(
            "no product route yet: executable test groups need their separately accepted application plans",
        ));
    }
    let receipt = application
        .receipt?
        .ok_or_else(|| failure("executable product has no program entry receipt"))?;
    let mut roots = Vec::new();
    for root in application.roots {
        let root = root?;
        let declaration = produced
            .module()
            .scalar_applications
            .proof_scope
            .callable_declaration(terminal_psi::TargetMachineCallable::Machine(root.machine))
            .ok_or_else(|| {
                failure("executable application root has no canonical proof declaration")
            })?;
        if !roots.contains(&declaration) {
            roots.push(declaration);
        }
    }
    if roots.is_empty() {
        return Err(failure(
            "executable product has no admitted application root",
        ));
    }
    let ProducedTerminalProduct { program, proposal } = produced;
    let mut proposal =
        proposal.ok_or_else(|| failure("native entry has no selected target proposal"))?;
    let module = program.artifact().semantic_module();
    proposal.application_roots = Some(roots);
    let selected = proposal.selection.as_ref().map_err(|reasons| {
        reasons
            .iter()
            .cloned()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;
    if selected.plans != checked.execution().plans().plans() {
        return Err(failure(
            "native proposal changed the settled provider plans",
        ));
    }
    let profile = entry.source().target_slot().owner;
    let schemas = selected
        .plans
        .iter()
        .map(|plan| plan.schema.clone())
        .collect::<Vec<_>>();
    let leaves =
        provider_planning::direct_hosted_primitive_leaves(module, &schemas, profile.target_name())?;
    // Checked wrappers carry no compiler execution identity. Their actual
    // canonical candidates are admitted downstream; only the independently
    // checked core primitive supplies a native catalog operation.
    let row_executions = selected
        .plans
        .iter()
        .map(|plan| {
            if plan.rows.iter().any(|row| {
                !matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
                )
            }) {
                return Err(failure(
                    "native selected provider row has no canonical execution producer",
                ));
            }
            Ok(vec![None; plan.rows.len()])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let policy = current_terminal_authority_policy();
    let mut permission_identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permission_rows = Vec::new();
    for leaf in &leaves {
        let backend_plan::DirectBoundaryExecution::Intrinsic(Some(execution)) = leaf.execution
        else {
            return Err(failure("core primitive lost its exact native execution"));
        };
        let permitted = policy
            .classify(execution)
            .map_err(|_| failure("core primitive has no physical authority classification"))?;
        for row in &leaf.plan.rows {
            let requirement = leaf.plan.identities.text(row.declared_requirement_identity);
            permission_rows.push(effects::ServiceTerminalAuthorityPermission::new(
                leaf.plan.schema_identity_digest(),
                permission_identities.intern(requirement),
                permitted.clone(),
            ));
        }
    }
    // The receiving host grants only the exact admitted primitive rows above.
    // Ordinary packages and unrelated checked instructions keep no permission.
    let permissions =
        native_compiler::admit_package_permissions(permission_rows, permission_identities.build())?;
    let semantic = entry
        .semantic_calling_application()
        .ok_or_else(|| failure("native entry lost its semantic calling application"))?;
    let physical = entry
        .physical_calling_application()
        .ok_or_else(|| failure("native entry lost its physical calling application"))?;
    let proposals = vec![proposal.clone()];
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: &module.identities,
        direct_boundary_leaves: leaves.iter().collect(),
        target_profile: Some(profile),
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: checked.execution().plans(),
            selected_provider_provenance: selected
                .plans
                .iter()
                .zip(&row_executions)
                .map(|(plan, executions)| CompilerIntrinsicPlanProvenance {
                    plan,
                    semantic_schema: &plan.schema,
                    row_executions: executions,
                })
                .collect(),
            calling_applications: vec![semantic.clone(), physical.clone()],
        },
        native_target: Some(profile.native_target()),
        program_entry: Some(entry.with_checked_entry(&receipt)),
        application: metadata,
        optimizations: optimizations.project_post_terminal(),
        external_bindings: &[],
        behavior_exclusions: &build.behavior_exclusions,
        package_terminal_authority_permissions: permissions.rows().to_vec(),
        package_terminal_authority_permission_identities: permissions.identities().clone(),
        x86_scalar_fma_provider: None,
    };
    let retained_inputs = request
        .filter(|request| request.retain_native_inputs)
        .map(|request| {
            native_compiler::retain_native_derivation_inputs(
                &receipt,
                false,
                &proposals,
                &native,
                &permissions,
                &request.installation_assumptions,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .transpose()?;
    let terminal = native_compiler::TerminalApplication {
        artifact: authorize_produced_artifact(&program)?,
        receipt: receipt.clone(),
        provider_proposals: proposals,
        physical_entry_adapter: false,
        evidence: program.evidence().cloned(),
    }
    .prepare(native)?;
    Ok((terminal, permissions, retained_inputs))
}
