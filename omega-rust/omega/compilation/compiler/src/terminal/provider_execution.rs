//! Evaluate only the calling policies an actual provider demand activates.
//! The checked program and explicit package permissions supply every input;
//! retaining a declaration alone does not activate it.

use super::ProductCodeSource;
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::CallingProgram;
use provider_planning::target_supply::TargetSupplyExecutionInputs;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::source_selections::SelectionPurpose;
use std::sync::Arc;

pub(crate) fn prepare_provider_execution_inputs(
    source: &CheckedProductSource,
    profile: target::TargetProfile,
    reached: &[BoundaryMachineId],
) -> Result<TargetSupplyExecutionInputs, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let mut timings = source.timings.clone();
    let program = ProductCodeSource {
        publication: &source.terminal_publication,
        sources: &source.sources,
    }
    .publish(
        source.terminal_publication.optimizations(),
        &mut crate::observations::PsiStageTimings::new(&mut timings),
    )?;
    let module = program.artifact().semantic_module();
    let mut requirements = Vec::with_capacity(reached.len());
    for &identity in reached {
        let boundary = module
            .boundary_machine(identity)
            .ok_or_else(|| refuse("provider input demand names no actual boundary".into()))?;
        let requirement = module
            .identities
            .text(boundary.identity)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| refuse("provider input demand lost its canonical identity".into()))?;
        requirements.push(requirement);
        // Closed applications keep their original public ABI coordinate.
        for candidate in module
            .provider_candidates
            .iter()
            .filter(|row| row.boundary == identity)
        {
            requirements.push(
                module
                    .identities
                    .text(candidate.source_requirement_identity)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| {
                        refuse("provider application lost its public requirement identity".into())
                    })?,
            );
        }
        for external in module.external_providers.iter().filter(|external| {
            module.identities.text(external.realization.callable) == Some(requirement)
                || module.identities.text(external.requirement_identity) == Some(requirement)
        }) {
            if !matches!(
                external.binding,
                language_semantics::ExternalBindingIdentity::CompilerIntrinsic
            ) {
                return Err(refuse(format!(
                    "native requirement `{requirement}` needs its canonical evaluated Build binding association before physical calling inputs can be formed",
                )));
            }
        }
    }
    requirements.sort_unstable();
    requirements.dedup();
    let calling = CallingProgram {
        artifact: Arc::clone(program.artifact()),
        evaluator: build_settings::CALLING_POLICY_EVALUATOR,
    };
    let selections = program
        .source_selections()
        .ok_or_else(|| refuse("provider policies lost their source selection custody".into()))?;
    let authority = source.sources.dependency_closure().map(|graph| {
        graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
    });
    let mut calling_applications = Vec::new();
    for choice in module
        .provider_selections
        .iter()
        .filter(|choice| choice.target == profile.target_name())
    {
        let schema = module
            .service_schema(choice.schema_declaration, choice.schema_application)
            .map_err(refuse)?;
        if !schema.surface.methods.iter().any(|method| {
            requirements
                .binary_search(&schema.surface.identity_text(method.requirement_identity))
                .is_ok()
        }) {
            continue;
        }
        let declaration = module
            .identities
            .text(choice.schema_declaration)
            .ok_or_else(|| refuse("selected provider schema lost its source declaration".into()))?;
        let owner = module
            .identities
            .text(choice.schema_application)
            .ok_or_else(|| refuse("selected provider schema lost its full application".into()))?;
        let requester = selections
            .declaration_custody(
                program.artifact().view().into(),
                declaration,
                SelectionPurpose::Product,
            )
            .map_err(refuse)?;
        for request in &module.calling_policy_requests {
            if request.declaration.as_ref().ok() != Some(&choice.schema_declaration) {
                continue;
            }
            for instance in request
                .instances
                .as_ref()
                .map_err(|reasons| refuse(reasons.join("; ")))?
            {
                if instance.declaring_application != choice.schema_application {
                    continue;
                }
                for &signature in instance
                    .signatures
                    .as_ref()
                    .map_err(|reason| refuse(reason.clone()))?
                {
                    let abi = module.calling_abi_signature(signature).ok_or_else(|| {
                        refuse("provider policy names an absent ABI declaration".into())
                    })?;
                    let requirement = module
                        .identities
                        .text(abi.requirement_identity)
                        .ok_or_else(|| {
                            refuse("provider ABI lost its requirement identity".into())
                        })?;
                    if requirements.binary_search(&requirement).is_err() {
                        continue;
                    }
                    let (signature, plan) = calling
                        .evaluate(instance, profile, signature, &[], requester, authority)
                        .map_err(refuse)?;
                    let application = signature
                        .native_application_with_owner(&plan, 0, owner)
                        .map_err(refuse)?;
                    if let Some(previous) = calling_applications.iter().find(
                        |previous: &&calling_conventions::BoundaryCallingApplication| {
                            previous.abi == application.abi
                        },
                    ) {
                        if previous != &application {
                            return Err(refuse(
                                "provider ABI has competing calling policy results".into(),
                            ));
                        }
                    } else {
                        calling_applications.push(application);
                    }
                }
            }
        }
    }
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    for binding in source.sources.accepted_semantic_bindings() {
        for permission in binding.terminal_authority_permissions() {
            let requirement = binding
                .terminal_authority_permission_identities()
                .text(permission.requirement_identity())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    refuse("accepted package permission lost its requirement identity".into())
                })?;
            let mut permission = permission.clone();
            permission.rekey_requirement_identity(identities.intern(requirement));
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    }
    Ok(TargetSupplyExecutionInputs {
        calling_applications,
        // Every reached foreign row was explicitly refused above. Dormant
        // Build supplies cannot become an executed foreign binding here.
        external_bindings: Vec::new(),
        evaluated_native_bindings: Vec::new(),
        evaluated_binding_identities: terminal_psi::TerminalIdentityTable::default(),
        terminal_authority_permissions: permissions,
        terminal_authority_permission_identities: identities.build(),
    })
}
