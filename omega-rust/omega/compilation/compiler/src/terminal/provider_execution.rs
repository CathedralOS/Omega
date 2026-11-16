//! Evaluate only the calling policies an actual provider demand activates.
//! The checked program and explicit package permissions supply every input;
//! retaining a declaration alone does not activate it.

use super::ProductCodeSource;
use diagnostics::Diagnostic;
use products::ProductTerminalPublication;
use provider_planning::calling_policy_plans::CallingProgram;
use provider_planning::target_supply::TargetSupplyExecutionInputs;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::source_selections::SelectionPurpose;
use std::sync::Arc;

// A request with unavailable ownership is still a selected policy request.
// Validate that roster before deciding whether a native binding has no policy.
fn calling_policy_instances(
    module: &terminal_psi::TerminalModule,
) -> Result<
    Vec<(
        terminal_psi::TerminalIdentityIndex,
        &terminal_psi::CallingPolicyInstance,
    )>,
    String,
> {
    let mut instances = Vec::new();
    for request in &module.calling_policy_requests {
        let declaration = *request.declaration.as_ref().map_err(Clone::clone)?;
        if module
            .identities
            .text(declaration)
            .is_none_or(str::is_empty)
        {
            return Err("selected Calling policy lost its source declaration".into());
        }
        let applications = request
            .instances
            .as_ref()
            .map_err(|reasons| reasons.join("; "))?;
        if applications.is_empty() {
            return Err("selected Calling policy has no retained application".into());
        }
        instances.extend(applications.iter().map(|instance| (declaration, instance)));
    }
    Ok(instances)
}

fn require_unselected_calling_policy(
    instances: &[(
        terminal_psi::TerminalIdentityIndex,
        &terminal_psi::CallingPolicyInstance,
    )],
    application: terminal_psi::TerminalIdentityIndex,
) -> Result<(), String> {
    if instances
        .iter()
        .any(|(_, instance)| instance.declaring_application == application)
    {
        return Err("native binding has no result from its selected Calling policy".into());
    }
    Ok(())
}

pub fn prepare_provider_execution_inputs(
    publication: &ProductTerminalPublication,
    sources: &package_compilation::CheckedSourceCustody,
    native_supplies: &[provider_planning::native_bindings::NativeBuildSupply],
    profile: target::TargetProfile,
    reached: &[BoundaryMachineId],
    observer: &mut impl source_compiler::PsiCompilationObserver,
) -> Result<TargetSupplyExecutionInputs, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    sources
        .require_owner(publication.accepted()?.source())
        .map_err(|error| vec![error])?;
    let program = ProductCodeSource {
        publication,
        sources,
    }
    .publish(publication.optimizations(), observer)?;
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
    let accepted = publication.accepted()?;
    let mut binding_identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut native_bindings = Vec::new();
    for supply in native_supplies {
        if supply.selection.target.canonical_name() != profile.target_name() {
            continue;
        }
        let mut actuals =
            accepted
                .closure()
                .elaboration
                .boundaries
                .iter()
                .filter(|(boundary, source)| {
                    **source == supply.selection.requirement && reached.contains(boundary)
                });
        let Some((boundary, _)) = actuals.next() else {
            continue;
        };
        if actuals.next().is_some() {
            return Err(refuse(
                "Build native binding has competing exact boundary occurrences".into(),
            ));
        }
        supply
            .selection
            .require_canonical_requirement(accepted.source())
            .map_err(refuse)?;
        let schema =
            backend_plan::DirectBoundaryLeaf::declared_schema(module, boundary).map_err(refuse)?;
        let declaration = module
            .boundary_machine(boundary)
            .ok_or_else(|| refuse("native binding lost its actual requirement".into()))?;
        if declaration.owner.identity() != schema.declaration {
            return Err(refuse(
                "native binding changes its source declaration owner".into(),
            ));
        }
        let requirement = module
            .identities
            .text(declaration.identity)
            .ok_or_else(|| refuse("native binding lost its canonical requirement".into()))?;
        let evaluated = provider_planning::native_bindings::normalize_native_binding(
            supply,
            requirement,
            &mut binding_identities,
        )
        .map_err(refuse)?;
        if native_bindings.iter().any(
            |previous: &provider_planning::target_supply::NormalizedNativeSupply| {
                previous.boundary == boundary
            },
        ) {
            return Err(refuse(
                "native requirement has competing completed Build supplies".into(),
            ));
        }
        let abi = backend_plan::DirectBoundaryLeaf::declared_calling_abi(module, boundary)
            .map_err(refuse)?;
        requirements.push(
            module
                .identities
                .text(abi.requirement_identity)
                .ok_or_else(|| refuse("native binding lost its original method template".into()))?,
        );
        native_bindings.push(provider_planning::target_supply::NormalizedNativeSupply {
            boundary,
            binding: evaluated,
        });
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
    let authority = sources.dependency_closure().map(|graph| {
        graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
    });
    let requested_instances = calling_policy_instances(module).map_err(refuse)?;
    let mut calling_applications = Vec::new();
    let mut schemas = module
        .provider_selections
        .iter()
        .filter(|choice| choice.target == profile.target_name())
        .map(|choice| (choice.schema_declaration, choice.schema_application))
        .collect::<Vec<_>>();
    for supply in &native_bindings {
        let schema = backend_plan::DirectBoundaryLeaf::declared_schema(module, supply.boundary)
            .map_err(refuse)?;
        schemas.push((schema.declaration, schema.application));
    }
    schemas.sort_unstable();
    schemas.dedup();
    for (schema_declaration, schema_application) in schemas {
        let schema = module
            .service_schema(schema_declaration, schema_application)
            .map_err(refuse)?;
        if !schema.surface.methods.iter().any(|method| {
            requirements
                .binary_search(&schema.surface.identity_text(method.requirement_identity))
                .is_ok()
        }) {
            continue;
        }
        let owner = module
            .identities
            .text(schema_application)
            .ok_or_else(|| refuse("selected provider schema lost its full application".into()))?;
        // Nominal schema ownership and source-selection custody have different
        // identities. The retained full declaring application joins them; its
        // request still supplies the authenticated semantic source declaration.
        for &(declaration, instance) in &requested_instances {
            if instance.declaring_application != schema_application {
                continue;
            }
            let declaration = module.identities.text(declaration).ok_or_else(|| {
                refuse("selected Calling policy lost its source declaration".into())
            })?;
            let requester = selections
                .declaration_custody(
                    program.artifact().view().into(),
                    declaration,
                    SelectionPurpose::Product,
                )
                .map_err(refuse)?;
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
                    .ok_or_else(|| refuse("provider ABI lost its requirement identity".into()))?;
                if requirements.binary_search(&requirement).is_err() {
                    continue;
                }
                let (signature, plan) = calling
                    .evaluate(instance, profile, signature, &[], requester, authority)
                    .map_err(refuse)?;
                let application = signature
                    .native_application_with_owner(&plan, None, owner)
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
    // Explicit selections remain authoritative. Only an undeclared policy uses
    // the existing native default planner; a failed or missing selected result
    // cannot silently become a default ABI.
    let mut default_bindings = Vec::new();
    for supply in &native_bindings {
        let abi = backend_plan::DirectBoundaryLeaf::declared_calling_abi(module, supply.boundary)
            .map_err(refuse)?
            .identity(&module.identities)
            .map_err(refuse)?;
        if calling_applications
            .iter()
            .any(|application| application.abi == abi)
        {
            continue;
        }
        let schema = backend_plan::DirectBoundaryLeaf::declared_schema(module, supply.boundary)
            .map_err(refuse)?;
        require_unselected_calling_policy(&requested_instances, schema.application)
            .map_err(refuse)?;
        default_bindings.push(supply.clone());
    }
    calling_applications.extend(
        provider_planning::calling_policy_plans::native_calling_applications(
            profile,
            &calling,
            &default_bindings,
        )?,
    );
    let mut external_bindings = Vec::with_capacity(native_bindings.len());
    let mut evaluated_native_bindings = Vec::with_capacity(native_bindings.len());
    for supply in native_bindings {
        let evaluated = supply.binding;
        let schema = backend_plan::DirectBoundaryLeaf::declared_schema(module, supply.boundary)
            .map_err(refuse)?;
        let abi = backend_plan::DirectBoundaryLeaf::declared_calling_abi(module, supply.boundary)
            .map_err(refuse)?
            .identity(&module.identities)
            .map_err(refuse)?;
        let boundary = module
            .boundary_machine(supply.boundary)
            .ok_or_else(|| refuse("native binding lost its exact boundary".into()))?;
        let requirement = module
            .identities
            .text(boundary.identity)
            .ok_or_else(|| refuse("native binding lost its exact requirement identity".into()))?;
        let mut methods = schema.surface.methods.iter().filter(|method| {
            schema.surface.identity_text(method.requirement_identity) == abi.requirement_identity
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err(refuse(
                "native binding lost its unique complete service method".into(),
            ));
        };
        let applications = calling_applications
            .iter()
            .filter(|application| application.abi == abi)
            .cloned()
            .collect::<Vec<_>>();
        if applications.is_empty() {
            return Err(refuse(
                "native binding has no actual evaluated Calling application".into(),
            ));
        }
        let binding = match &evaluated {
            effects::provider_plan::EvaluatedNativeBinding::Import(import) => {
                calling_conventions::ExternalBindingKind::Import {
                    locator: import.locator().clone(),
                }
            }
            effects::provider_plan::EvaluatedNativeBinding::Syscall(syscall) => {
                calling_conventions::ExternalBindingKind::Syscall {
                    number: syscall.number(),
                }
            }
        };
        external_bindings.push(calling_conventions::ExternalBindingRow {
            target_name: profile.target_name().to_owned(),
            trait_name: requirement.to_owned(),
            method: method.name.clone(),
            requirement_identity: requirement.to_owned(),
            provider_type: String::new(),
            calling_applications: applications,
            binding,
        });
        evaluated_native_bindings.push(evaluated);
    }
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    for binding in sources.accepted_semantic_bindings() {
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
    let schemas = provider_planning::compiler_primitive_schemas(module).map_err(refuse)?;
    let primitives = provider_planning::direct_compiler_primitive_leaves(
        module,
        &schemas,
        profile.target_name(),
        reached.iter().copied(),
    )?;
    let policy = native_compiler::current_terminal_authority_policy();
    for leaf in primitives {
        let backend_plan::DirectBoundaryExecution::Intrinsic(Some(execution)) = leaf.execution
        else {
            return Err(refuse(
                "core primitive lost its exact native execution".into(),
            ));
        };
        let permitted = policy.classify(execution).map_err(|reason| {
            refuse(format!(
                "core primitive has no physical authority classification: {reason:?}"
            ))
        })?;
        for row in &leaf.plan.rows {
            let requirement = leaf.plan.identities.text(row.declared_requirement_identity);
            let permission = effects::ServiceTerminalAuthorityPermission::new(
                leaf.plan.schema_identity_digest(),
                identities.intern(requirement),
                permitted.clone(),
            );
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    }
    Ok(TargetSupplyExecutionInputs {
        calling_applications,
        external_bindings,
        evaluated_native_bindings,
        evaluated_binding_identities: binding_identities.build(),
        terminal_authority_permissions: permissions,
        terminal_authority_permission_identities: identities.build(),
    })
}

#[cfg(test)]
mod tests {
    use super::{calling_policy_instances, require_unselected_calling_policy};

    #[test]
    fn unavailable_selected_calling_policy_is_not_an_absent_policy() {
        let mut module = terminal_psi::TerminalModule::default();
        assert!(calling_policy_instances(&module).unwrap().is_empty());
        module
            .calling_policy_requests
            .push(terminal_psi::CallingPolicyRequest {
                declaration: Err("selected owner is unavailable".into()),
                display_name: "selected".into(),
                instances: Ok(Vec::new()),
            });
        assert_eq!(
            calling_policy_instances(&module).unwrap_err(),
            "selected owner is unavailable"
        );
    }
    #[test]
    fn native_default_requires_absence_of_the_exact_policy_application() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let declaration = identities.intern("semantic declaration");
        let selected = identities.intern("selected full application");
        let other = identities.intern("another full application");
        let mut module = terminal_psi::TerminalModule {
            identities: identities.build(),
            ..Default::default()
        };
        require_unselected_calling_policy(&calling_policy_instances(&module).unwrap(), selected)
            .unwrap();
        module
            .calling_policy_requests
            .push(terminal_psi::CallingPolicyRequest {
                declaration: Ok(declaration),
                display_name: "selected".into(),
                instances: Ok(vec![terminal_psi::CallingPolicyInstance {
                    declaring_application: selected,
                    policy_declaration: Err("unavailable policy declaration".into()),
                    policy: Err("unavailable policy body".into()),
                    signatures: Err("missing evaluated Calling result".into()),
                }]),
            });
        let instances = calling_policy_instances(&module).unwrap();
        assert_eq!(
            require_unselected_calling_policy(&instances, selected).unwrap_err(),
            "native binding has no result from its selected Calling policy"
        );
        require_unselected_calling_policy(&instances, other).unwrap();
    }
}
