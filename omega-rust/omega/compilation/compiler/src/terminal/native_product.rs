// Assemble the native request from this application's canonical artifact and
// completed entry plans. The remaining unsupported Build demands refuse here;
// their absence is checked before empty supply/permission rosters are formed.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use native_compiler::{
    BoundaryApplicationSettlement, NativeProgramEntrySettlement, TerminalNativeProposalRequest,
};
use optimization_core::OptimizationSelections;
use psi_compiler::ProducedTerminalApplication;

use super::ProducedTerminalProduct;

pub(crate) fn prepare(
    checked: &CheckedCompilation,
    produced: ProducedTerminalProduct,
    entry: NativeProgramEntrySettlement<'_>,
    application: ProducedTerminalApplication,
    metadata: target::NativeApplicationMetadata,
    optimizations: &OptimizationSelections,
    request: Option<&compilation_products::NativeProductRequest>,
) -> Result<
    (
        compilation_products::RetainedTerminalArtifact,
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
    if !checked.execution().plans().plans().is_empty()
        || !checked.execution().plans().assumed_bindings().is_empty()
    {
        return Err(failure(
            "no product route yet: executable providers need their canonical row realizations and package permissions",
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
    let ProducedTerminalProduct {
        program,
        mut proposal,
    } = produced;
    let module = program.artifact().semantic_module();
    proposal.application_roots = Some(roots);
    let selected = proposal.selection.as_ref().map_err(|reasons| {
        reasons
            .iter()
            .cloned()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;
    if !selected.plans.is_empty() || !selected.checked_candidates.is_empty() {
        return Err(failure(
            "no product route yet: native provider declarations need their selected row evidence",
        ));
    }
    // No selected provider row exists above, so no package permission can be
    // granted. This is a deny-by-absence policy, not inferred leaf authority.
    let permissions = native_compiler::admit_package_permissions(Vec::new(), Default::default())?;
    let profile = entry.source().target_slot().owner;
    let semantic = entry
        .semantic_calling_application()
        .ok_or_else(|| failure("native entry lost its semantic calling application"))?;
    let physical = entry
        .physical_calling_application()
        .ok_or_else(|| failure("native entry lost its physical calling application"))?;
    let proposals = vec![proposal.clone()];
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: &module.identities,
        direct_boundary_leaves: Vec::new(),
        provider_declarations: &selected.plans,
        target_profile: Some(profile),
        family_realizations: &[],
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: checked.execution().plans(),
            selected_provider_provenance: Vec::new(),
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
        artifact: std::sync::Arc::clone(program.artifact()),
        receipt: receipt.clone(),
        provider_proposals: proposals,
        physical_entry_adapter: false,
        evidence: program.evidence().cloned(),
    }
    .prepare(native)?;
    Ok((terminal, permissions, retained_inputs))
}
