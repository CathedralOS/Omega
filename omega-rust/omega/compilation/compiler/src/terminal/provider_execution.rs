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
        if supply.selection.application.is_some() {
            return Err(refuse(
                "native binding retains an unrepresented applied requirement".into(),
            ));
        }
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
        if native_bindings
            .iter()
            .any(|(previous, _)| *previous == boundary)
        {
            return Err(refuse(
                "native requirement has competing completed Build supplies".into(),
            ));
        }
        native_bindings.push((boundary, supply));
    }
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
    let mut calling_applications = Vec::new();
    let mut schemas = module
        .provider_selections
        .iter()
        .filter(|choice| choice.target == profile.target_name())
        .map(|choice| (choice.schema_declaration, choice.schema_application))
        .collect::<Vec<_>>();
    for (boundary, _) in &native_bindings {
        let schema =
            backend_plan::DirectBoundaryLeaf::declared_schema(module, *boundary).map_err(refuse)?;
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
        let declaration = module
            .identities
            .text(schema_declaration)
            .ok_or_else(|| refuse("selected provider schema lost its source declaration".into()))?;
        let owner = module
            .identities
            .text(schema_application)
            .ok_or_else(|| refuse("selected provider schema lost its full application".into()))?;
        let requester = selections
            .declaration_custody(
                program.artifact().view().into(),
                declaration,
                SelectionPurpose::Product,
            )
            .map_err(|reason| {
                let purposes = selections
                    .questions_for(program.artifact().view().into())
                    .into_iter()
                    .flat_map(|questions| questions.declarations.iter())
                    .filter(|(_, row)| row.identity == declaration)
                    .take(2)
                    .map(|(_, row)| row.purpose)
                    .collect::<Vec<_>>();
                refuse(format!(
                    "calling policy source for `{declaration}` (application `{owner}`): {reason}; retained declaration purposes {purposes:?}"
                ))
            })?;
        for request in &module.calling_policy_requests {
            if request.declaration.as_ref().ok() != Some(&schema_declaration) {
                continue;
            }
            for instance in request
                .instances
                .as_ref()
                .map_err(|reasons| refuse(reasons.join("; ")))?
            {
                if instance.declaring_application != schema_application {
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
                    let dispatch_only_formal =
                        plan.plan().call.table_parameter.dispatch_only_formal();
                    let application = signature
                        .native_application_with_owner(&plan, dispatch_only_formal, owner)
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
    let mut external_bindings = Vec::with_capacity(native_bindings.len());
    let mut evaluated_native_bindings = Vec::with_capacity(native_bindings.len());
    for (boundary, supply) in native_bindings {
        let boundary_id = boundary;
        let schema =
            backend_plan::DirectBoundaryLeaf::declared_schema(module, boundary).map_err(refuse)?;
        let boundary = module
            .boundary_machine(boundary)
            .ok_or_else(|| refuse("native binding lost its exact boundary".into()))?;
        let requirement = module
            .identities
            .text(boundary.identity)
            .ok_or_else(|| refuse("native binding lost its exact requirement identity".into()))?;
        let mut methods = schema.surface.methods.iter().filter(|method| {
            schema.surface.identity_text(method.requirement_identity) == requirement
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err(refuse(
                "native binding lost its unique complete service method".into(),
            ));
        };
        let applications = calling_applications
            .iter()
            .filter(|application| application.requirement_identity == requirement)
            .cloned()
            .collect::<Vec<_>>();
        if applications.is_empty() {
            return Err(refuse(
                "native binding has no actual evaluated Calling application".into(),
            ));
        }
        let abi = module.calling_abi_identity(boundary_id).map_err(refuse)?;
        let mut matching = applications
            .iter()
            .filter(|application| application.abi == abi);
        let (Some(application), None) = (matching.next(), matching.next()) else {
            return Err(refuse(
                "native binding needs its one exact declaration ABI application".into(),
            ));
        };
        let evaluated = provider_planning::native_bindings::normalize_native_binding(
            supply,
            requirement,
            Some((program.artifact().closed_module(), boundary_id, application)),
            &mut binding_identities,
        )
        .map_err(refuse)?;
        let binding = match &evaluated {
            effects::provider_plan::EvaluatedNativeBinding::Import(import) => {
                calling_conventions::ExternalBindingKind::Import {
                    locator: import.locator().clone(),
                }
            }
            effects::provider_plan::EvaluatedNativeBinding::VtableField(field) => {
                calling_conventions::ExternalBindingKind::VtableField {
                    field: field.storage().field_declaration.clone(),
                    byte_offset: field.storage().byte_offset,
                    table_formal: field.storage().formal_ordinal,
                    table_operand: field.storage().table_operand,
                }
            }
            effects::provider_plan::EvaluatedNativeBinding::TableFunction(field) => {
                calling_conventions::ExternalBindingKind::TableFunction {
                    field: field.storage().field_declaration.clone(),
                    byte_offset: field.storage().byte_offset,
                    table_formal: field.storage().formal_ordinal,
                    table_operand: field.storage().table_operand,
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
