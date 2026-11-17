//! Native entry selection and calling-policy realization consume the accepted
//! artifact and admitted Build permissions. Products only retain the result.
use diagnostics::Diagnostic;
use products::PublishedCompilation;
use native_program_planning::program_entry::{
    ProgramEntryCallingPlanRequest, SelectedCompilerProgramEntry, SelectedProgramEntryCallingPlans,
};

pub(crate) fn program_entry_with_calling_plans(
    checked: &PublishedCompilation,
    program: &products::PublishedProgram,
    providers: &[effects::provider_plan::ProviderPlan],
) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    if !std::ptr::eq(checked.program().as_ref(), program) {
        return Err(refuse(
            "entry planning requires this compilation's published program".into(),
        ));
    }
    let selected = selected_program_entry(checked)?;
    let slot = selected.source_signature().target_slot();
    let module = program.artifact().semantic_module();
    let contract = module
        .program_entry_contract(slot.owner.target_name())
        .map_err(refuse)?;
    let declared = module
        .service_schema(contract.schema_declaration, contract.schema_application)
        .map_err(refuse)?;
    let schema = effects::provider_plan::ServiceSchema::from_declaration(std::sync::Arc::clone(
        &declared.surface,
    ));
    let declaring_schema = module
        .identities
        .text(contract.schema_declaration)
        .ok_or_else(|| refuse("entry schema lost its canonical declaration identity".into()))?;
    let requester = program
        .entry_schema_custody(contract.schema_declaration)
        .map_err(refuse)?;
    admit_entry_schema(checked.package_inputs(), slot, &schema, requester).map_err(refuse)?;
    let physical = slot
        .physical_arrival_requirement
        .ok_or_else(|| refuse("selected entry slot has no physical arrival requirement".into()))?;
    let semantic =
        requirement_identity(&schema, slot.semantic_arrival_requirement).map_err(refuse)?;
    let physical = requirement_identity(&schema, physical).map_err(refuse)?;
    let authority = checked.package_inputs().dependency_closure().map(|graph| {
        graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
    });
    let owner = schema
        .root_application_identity()
        .ok_or_else(|| refuse("entry schema has no exact root application identity".into()))?;
    let policy =
        native_program_planning::program_entry::entry_calling_policy(module, declaring_schema, owner)
            .map_err(refuse)?;
    let admitted_providers = backend_plan::TerminalTargetProviderSelection::from_plans(
        providers.to_vec(),
        program.artifact().view(),
    )
    .and_then(|selected| selected.admit(program.artifact().view(), slot.owner.target_name()))
    .map_err(refuse)?;
    let admission = terminal_installation::admit_compiler_evaluation(
        program.artifact().view(),
        slot.owner.target_name(),
        *policy
            .policy_declaration
            .as_ref()
            .map_err(|reason| refuse(reason.clone()))?,
        admitted_providers,
        requester,
        authority,
    )
    .map_err(refuse)?;
    let plans = SelectedProgramEntryCallingPlans::evaluate(
        &native_program_planning::calling_policy_plans::CallingProgram {
            artifact: std::sync::Arc::clone(program.artifact()),
        },
        ProgramEntryCallingPlanRequest {
            slot,
            schema: schema.clone(),
            declaring_schema,
            semantic_requirement: semantic,
            physical_requirement: physical,
            physical_source_digest: contract.physical_source_digest,
            providers,
            admission,
        },
    )
    .map_err(refuse)?;
    selected.with_calling_plans(plans)
}

pub(crate) fn selected_program_entry(
    checked: &PublishedCompilation,
) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    let profile = checked
        .execution()
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
    let program = checked.program();
    let module = program.artifact().semantic_module();
    let selected = module
        .program_entry_contract(profile.target_name())
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    SelectedCompilerProgramEntry::from_terminal(
        module,
        selected.entry,
        profile.program_entry_slot(),
    )
}

fn requirement_identity<'schema>(
    schema: &'schema effects::provider_plan::ServiceSchema,
    nominated: &str,
) -> Result<&'schema str, String> {
    let (owner, name) = nominated.rsplit_once("::").ok_or_else(|| {
        format!("installed entry requirement `{nominated}` has no declaring owner")
    })?;
    let mut methods = schema
        .declaration
        .methods
        .iter()
        .filter(|method| method.requirement_owner == owner && method.name == name);
    let (Some(method), None) = (methods.next(), methods.next()) else {
        return Err(format!(
            "selected entry schema does not declare exactly one `{nominated}` arrival",
        ));
    };
    let identity = schema.identity_text(method.requirement_identity);
    if identity.is_empty() {
        return Err(format!(
            "entry requirement `{nominated}` lost its checked identity"
        ));
    }
    Ok(identity)
}

fn admit_entry_schema(
    sources: &products::ExecutablePackageInputs,
    slot: target::ProgramEntrySlotDeclaration,
    schema: &effects::provider_plan::ServiceSchema,
    custody: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
) -> Result<(), String> {
    use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
    let package = slot
        .physical_contract_package
        .ok_or("entry slot has no physical contract package")?;
    let role = package_compilation::program_entry_semantic_binding_role(package);
    let binding = sources.accepted_semantic_binding(role);
    match (custody, binding) {
        (DeclarationCustody::ToolchainContract, None)
            if schema.declaration.trait_package_identity.is_none() =>
        {
            Ok(())
        }
        (DeclarationCustody::Package(owner), Some(binding))
            if binding.package() == owner
                && schema.declaration.trait_package_identity == Some(owner)
                && schema.identity_text(schema.declaration.trait_name)
                    == binding.declaration_path()
                && package_compilation::accepted_service_schema_digest(role, schema)
                    == binding.normalized_schema_digest() =>
        {
            Ok(())
        }
        // Without an accepted binding, entry contract selection admits only the
        // package that alone supplies the target's contract source, and only its
        // root-level schema bearing the target's default name. Admit exactly
        // that schema here; any other unbound package schema stays refused.
        (DeclarationCustody::Package(owner), None)
            if schema.declaration.trait_package_identity == Some(owner)
                && slot.boundary_schema.is_some_and(|default| {
                    schema.identity_text(schema.declaration.trait_name) == default
                }) =>
        {
            Ok(())
        }
        _ => Err(
            "entry schema does not match the consumer's accepted source package and contract"
                .into(),
        ),
    }
}
