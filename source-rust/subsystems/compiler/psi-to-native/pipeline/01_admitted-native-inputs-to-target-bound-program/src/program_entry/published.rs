//! Select and plan the entry of the published program against its settled providers.
use super::{
    ProgramEntryCallingPlanRequest, SelectedCompilerProgramEntry, SelectedProgramEntryCallingPlans,
};
use declaration_scope::{DeclarationSelectionAuthority, ProductDeclarationScope};
use diagnostics::Diagnostic;
use provider_acceptance::AcceptedSemanticBinding;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedEntryNomination {
    program: terminal_psi::TerminalPsiIdentity,
    selected: SelectedCompilerProgramEntry,
    schema_declaration: terminal_psi::TerminalIdentityIndex,
}

impl PublishedEntryNomination {
    pub(crate) fn from_module(
        module: &terminal_psi::TerminalModule,
        program: terminal_psi::TerminalPsiIdentity,
        target: Option<target::TargetProfile>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let profile = target.unwrap_or_else(target::TargetProfile::host);
        let contract = module
            .program_entry_contract(profile.target_name())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let selected = SelectedCompilerProgramEntry::from_terminal(
            module,
            contract.entry,
            profile.program_entry_slot(),
        )?;
        Ok(Self {
            program,
            selected,
            schema_declaration: contract.schema_declaration,
        })
    }

    pub fn program(&self) -> terminal_psi::TerminalPsiIdentity {
        self.program
    }
    pub fn source_signature(&self) -> &program_entry_contracts::SelectedProgramEntrySignature {
        self.selected.source_signature()
    }
    pub fn nominal_root(&self) -> Result<terminal_psi::PreparedNominalRoot, Vec<Diagnostic>> {
        self.selected.nominal_root()
    }
    pub fn schema_declaration(&self) -> terminal_psi::TerminalIdentityIndex {
        self.schema_declaration
    }
}

pub fn plan_published_entry_calling_conventions(
    nomination: &PublishedEntryNomination,
    artifact: &Arc<CanonicalTerminalArtifact>,
    providers: &[provider_plans::ProviderPlan],
    requester: DeclarationCustody,
    binding: Option<&AcceptedSemanticBinding>,
    permissions: Option<&ProductDeclarationScope>,
) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let selected = nomination.selected.clone();
    let slot = selected.source_signature().target_slot();
    let module = artifact.semantic_module();
    let contract = module
        .program_entry_contract(slot.owner.target_name())
        .map_err(refuse)?;
    let declared = module
        .service_schema(contract.schema_declaration, contract.schema_application)
        .map_err(refuse)?;
    let schema = provider_plans::ServiceSchema::from_declaration(std::sync::Arc::clone(
        &declared.surface,
    ));
    let declaring_schema = module
        .identities
        .text(contract.schema_declaration)
        .ok_or_else(|| refuse("entry schema lost its canonical declaration identity".into()))?;
    admit_entry_schema(binding, slot, &schema, requester).map_err(refuse)?;
    let physical = slot
        .physical_arrival_requirement
        .ok_or_else(|| refuse("selected entry slot has no physical arrival requirement".into()))?;
    let semantic =
        requirement_identity(&schema, slot.semantic_arrival_requirement).map_err(refuse)?;
    let physical = requirement_identity(&schema, physical).map_err(refuse)?;
    let authority = permissions.map(|scope| scope as &dyn DeclarationSelectionAuthority);
    let owner = schema
        .root_application_identity()
        .ok_or_else(|| refuse("entry schema has no exact root application identity".into()))?;
    let policy = super::entry_calling_policy(module, declaring_schema, owner).map_err(refuse)?;
    let admitted_providers = target_provider_admission::TerminalTargetProviderSelection::from_plans(
        providers.to_vec(),
        artifact.view(),
    )
    .and_then(|selected| selected.admit(artifact.view(), slot.owner.target_name()))
    .map_err(refuse)?;
    let admission = evaluation_admission::admit_compiler_evaluation(
        artifact.view(),
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
        &crate::calling_policy_plans::CallingProgram {
            artifact: std::sync::Arc::clone(artifact),
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
    let mut entry = selected.with_calling_plans(plans)?;
    entry.bind_physical_adapter(module, providers)?;
    Ok(entry)
}

fn requirement_identity<'schema>(
    schema: &'schema provider_plans::ServiceSchema,
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
    binding: Option<&AcceptedSemanticBinding>,
    slot: target::ProgramEntrySlotDeclaration,
    schema: &provider_plans::ServiceSchema,
    custody: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
) -> Result<(), String> {
    use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
    let package = slot
        .physical_contract_package
        .ok_or("entry slot has no physical contract package")?;
    let role = provider_acceptance::program_entry_semantic_binding_role(package);
    match (custody, binding) {
        (DeclarationCustody::ToolchainContract, None)
            if schema.declaration.trait_package_identity.is_none() =>
        {
            Ok(())
        }
        (DeclarationCustody::Package(owner), Some(binding))
            if binding.role() == role
                && binding.package() == owner
                && schema.declaration.trait_package_identity == Some(owner)
                && schema.identity_text(schema.declaration.trait_name)
                    == binding.declaration_path()
                && provider_acceptance::accepted_service_schema_digest(role, schema)
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
