//! Plan Build's selected entry from the accepted program and its normalized
//! contracts. Later product stages never reopen typed source to find the entry.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use provider_planning::program_entry::{
    ProgramEntryCallingPlanRequest, SelectedCompilerProgramEntry, SelectedProgramEntryCallingPlans,
};

impl CheckedCompilation {
    pub fn program_entry_with_calling_plans(
        &self,
        program: &psi_compiler::PortableTerminalProgram,
        providers: &[backend_plan::TerminalTargetProviderPlan],
        evaluator: provider_planning::calling_policy_plans::CallingPolicyEvaluator,
    ) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let publication = self.terminal_publication();
        if !publication
            .published()
            .is_some_and(|published| std::ptr::eq(published, program))
        {
            return Err(refuse(
                "entry planning requires this compilation's published program".into(),
            ));
        }
        let selected = self.selected_program_entry()?;
        let slot = selected.source_signature().target_slot();
        let module = program.artifact().semantic_module();
        let contract = module
            .program_entry_contract(slot.owner.target_name())
            .map_err(refuse)?;
        let declared = module
            .service_schema(contract.schema_declaration, contract.schema_application)
            .map_err(refuse)?;
        let schema = effects::provider_plan::ServiceSchema::from_declaration(
            std::sync::Arc::clone(&declared.surface),
        );
        let declaring_schema = module
            .identities
            .text(contract.schema_declaration)
            .ok_or_else(|| refuse("entry schema lost its canonical declaration identity".into()))?;
        let requester = schema_custody(program, declaring_schema).map_err(refuse)?;
        admit_entry_schema(self.sources(), slot, &schema, requester).map_err(refuse)?;
        let physical = slot.physical_arrival_requirement.ok_or_else(|| {
            refuse("selected entry slot has no physical arrival requirement".into())
        })?;
        let semantic =
            requirement_identity(&schema, slot.semantic_arrival_requirement).map_err(refuse)?;
        let physical = requirement_identity(&schema, physical).map_err(refuse)?;
        let authority = self.sources().dependency_closure().map(|graph| {
            graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
        });
        let plans = SelectedProgramEntryCallingPlans::evaluate(
            &provider_planning::calling_policy_plans::CallingProgram {
                artifact: std::sync::Arc::clone(program.artifact()),
                evaluator,
            },
            ProgramEntryCallingPlanRequest {
                slot,
                schema: schema.clone(),
                declaring_schema,
                semantic_requirement: semantic,
                physical_requirement: physical,
                physical_source_digest: contract.physical_source_digest,
                providers,
                requester,
                authority,
            },
        )
        .map_err(refuse)?;
        selected.with_calling_plans(plans)
    }

    pub fn selected_program_entry(&self) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
        let profile = self
            .execution()
            .target_profile()
            .unwrap_or_else(target::TargetProfile::host);
        let module = self.terminal_publication().accepted()?.module();
        let selected = module
            .program_entry_contract(profile.target_name())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        SelectedCompilerProgramEntry::from_terminal(
            module,
            selected.entry,
            profile.program_entry_slot(),
        )
    }
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

fn schema_custody(
    program: &psi_compiler::PortableTerminalProgram,
    declaration: &str,
) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, String> {
    program
        .source_selections()
        .ok_or("entry schema has no authenticated source selection custody")?
        .declaration_custody(
            program.artifact().view().into(),
            declaration,
            semantic_vocabulary::source_selections::SelectionPurpose::Product,
        )
}

fn admit_entry_schema(
    sources: &package_compilation::CheckedSourceCustody,
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
        _ => Err(
            "entry schema does not match the consumer's accepted source package and contract"
                .into(),
        ),
    }
}
