// Assemble the native request from the accepted program and its selected
// supplies. Native bindings retain their Build receipts; checked providers
// retain the bodies and schemas already joined to the published artifact.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use native_compiler::{
    BoundaryApplicationSettlement, NativeProgramEntrySettlement, TerminalNativeProposalRequest,
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

// Package acceptance rows carry identities in their own tables. Join those
// exact identities before the native admission validates the combined rows.
fn accepted_package_permissions(
    bindings: &[package_compilation::AcceptedSemanticBinding],
) -> Result<native_compiler::TerminalAuthorityPermissionPolicy, Vec<Diagnostic>> {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    for binding in bindings {
        let source = binding.terminal_authority_permission_identities();
        for permission in binding.terminal_authority_permissions() {
            let requirement = source
                .text(permission.requirement_identity())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "accepted package permission has no exact requirement identity",
                    )]
                })?;
            let mut permission = permission.clone();
            permission.rekey_requirement_identity(identities.intern(requirement));
            permissions.push(permission);
        }
    }
    native_compiler::admit_package_permissions(permissions, identities.build())
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
    if !checked.execution().plans().assumed_bindings().is_empty() {
        return Err(failure(
            "no product route yet: executable assumed bindings need their admitted contracts",
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
    if selected
        .plans
        .iter()
        .flat_map(|plan| &plan.rows)
        .any(|row| {
            !matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        })
    {
        return Err(failure(
            "no product route yet: opaque provider rows need their authenticated execution evidence",
        ));
    }
    let permissions = accepted_package_permissions(checked.sources().accepted_semantic_bindings())?;
    let profile = entry.source().target_slot().owner;
    let semantic = entry
        .semantic_calling_application()
        .ok_or_else(|| failure("native entry lost its semantic calling application"))?;
    let physical = entry
        .physical_calling_application()
        .ok_or_else(|| failure("native entry lost its physical calling application"))?;
    let native_supplies = checked.execution().native_bindings();
    let accepted = checked.terminal_publication().accepted()?;
    let declared = accepted
        .boundary_requirements_declared()
        .map_err(|error| vec![error])?;
    let schemas = native_supplies
        .rows
        .iter()
        .map(|supply| {
            let requirement = declared
                .iter()
                .find(|requirement| requirement.boundary == supply.boundary)
                .ok_or_else(|| {
                    failure("selected native binding has no accepted boundary declaration")
                })?;
            let surface = accepted.slot_schema(requirement.slot).ok_or_else(|| {
                failure("selected native binding lost its complete requirement schema")
            })?;
            Ok(effects::provider_plan::ServiceSchema::from_declaration(
                surface,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let native_bindings = native_supplies
        .rows
        .iter()
        .zip(&schemas)
        .map(|(supply, schema)| native_compiler::NativeBindingInput {
            boundary: supply.boundary,
            binding: &supply.binding,
            identities: &native_supplies.identities,
            schema,
        })
        .collect::<Vec<_>>();
    let calling_program = provider_planning::calling_policy_plans::CallingProgram {
        artifact: std::sync::Arc::clone(program.artifact()),
        evaluator: build_settings::CALLING_POLICY_EVALUATOR,
    };
    let plans = checked.execution().plans().plans();
    let mut calling_applications = vec![semantic.clone(), physical.clone()];
    // Checked providers in this product use their authenticated local machine
    // installations and ordinary Omega calls. Their private home layouts do
    // not become public foreign ABIs. Independent components, rejected above,
    // require their own admitted cross-artifact calling applications.
    for application in provider_planning::calling_policy_plans::native_calling_applications(
        profile,
        &calling_program,
        &native_supplies.rows,
        |boundary| {
            declared
                .iter()
                .find(|requirement| requirement.boundary == boundary)
                .map(|requirement| requirement.name.clone())
                .unwrap_or_else(|| format!("boundary {}", boundary.get()))
        },
    )? {
        if !calling_applications.contains(&application) {
            calling_applications.push(application);
        }
    }
    // These plans contain checked bodies only. They have no compiler-intrinsic
    // execution identity; the proposal independently rejoins their candidates.
    let row_executions = plans
        .iter()
        .map(|plan| vec![None; plan.rows.len()])
        .collect::<Vec<_>>();
    let selected_provider_provenance = plans
        .iter()
        .zip(&row_executions)
        .map(
            |(plan, row_executions)| native_compiler::CompilerIntrinsicPlanProvenance {
                plan,
                semantic_schema: &plan.schema,
                row_executions,
            },
        )
        .collect();
    let proposals = vec![proposal.clone()];
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: &module.identities,
        native_bindings: &native_bindings,
        direct_boundary_leaves: Vec::new(),
        target_profile: Some(profile),
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: checked.execution().plans(),
            selected_provider_provenance,
            calling_applications,
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

#[cfg(test)]
mod tests {
    use super::accepted_package_permissions;
    use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
    use effects::{
        ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
    };
    use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};

    #[test]
    fn package_permission_transport_keeps_each_bindings_identity_table() {
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let mut bindings = Vec::new();
        let mut expected = Vec::new();
        for (byte, requirement, class) in [
            (3, "Left::write", TerminalAuthorityClass::ProcessOutput),
            (4, "Right::read", TerminalAuthorityClass::ProcessInput),
        ] {
            let schema = ServiceSchemaDigest::from_digest([byte; 32]);
            let permitted = TerminalAuthorityDisposition::from_classes([class]);
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let permission = ServiceTerminalAuthorityPermission::new(
                schema,
                identities.intern(requirement),
                permitted.clone(),
            );
            let binding = AcceptedSemanticBinding::new(
                AcceptedSemanticBindingRole::ConsoleIo,
                package,
                "Console",
                schema,
                ProviderPlanDigest::from_digest([byte; 32]),
            )
            .unwrap()
            .with_terminal_authority_permissions(vec![permission], &identities.build())
            .unwrap();
            bindings.push(binding);
            expected.push((schema, requirement.to_owned(), permitted));
        }
        assert_eq!(
            bindings[0].terminal_authority_permissions()[0].requirement_identity(),
            bindings[1].terminal_authority_permissions()[0].requirement_identity(),
            "the source-local indices collide before transport"
        );
        let accepted = accepted_package_permissions(&bindings).unwrap();
        let actual = accepted
            .rows()
            .iter()
            .map(|permission| {
                (
                    permission.service_schema(),
                    accepted
                        .identities()
                        .text(permission.requirement_identity())
                        .unwrap()
                        .to_owned(),
                    permission.permitted().clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(accepted_package_permissions(&[]).unwrap().rows().is_empty());
    }
}
