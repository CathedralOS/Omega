//! Select and plan the entry of the published program against its settled providers.
use super::{
    ProgramEntryCallingPlanRequest, SelectedCompilerProgramEntry, SelectedProgramEntryCallingPlans,
};
use declaration_scope::{DeclarationSelectionAuthority, ProductDeclarationScope};
use diagnostics::Diagnostic;
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
    admit_entry_schema(slot, &schema, requester).map_err(refuse)?;
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
    let admission = build_model::admission::admit_compiler_evaluation(
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
    let method = schema.declaration.entry_requirement(nominated)?;
    let identity = schema.identity_text(method.requirement_identity);
    if identity.is_empty() {
        return Err(format!(
            "entry requirement `{nominated}` lost its checked identity"
        ));
    }
    Ok(identity)
}

fn admit_entry_schema(
    slot: target::ProgramEntrySlotDeclaration,
    schema: &provider_plans::ServiceSchema,
    custody: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
) -> Result<(), String> {
    use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
    let owner = match custody {
        DeclarationCustody::Package(owner) => Some(owner),
        DeclarationCustody::ToolchainContract => None,
        _ => return Err("entry schema lacks authenticated contract custody".into()),
    };
    if schema.declaration.trait_package_identity != owner
        || slot.boundary_schema != Some(schema.identity_text(schema.declaration.trait_name))
    {
        return Err("published entry schema differs from the designated target declaration".into());
    }
    Ok(())
}
