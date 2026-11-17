//! Select the exact contract source that discovery chose for a target entry.
//! This nominates source to check; package policy still admits its schema.

use super::EntryContractSource;
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
use typed_trees::{FrozenTypedTrees, ProductSourceIdentity};

#[derive(Debug, Clone)]
pub struct SelectedProgramEntryContractSource {
    owner: ProductSourceIdentity,
    source: source::SourceId,
    schema: SymbolHandle,
    custody: DeclarationCustody,
    application: TraitApplicationEnvironment,
}

impl SelectedProgramEntryContractSource {
    pub fn require_owner(&self, source: &Arc<FrozenTypedTrees>) -> Result<(), Diagnostic> {
        if !self.owner.is_for(source) {
            return Err(Diagnostic::error(
                "entry contract selection belongs to another frozen source owner",
            ));
        }
        Ok(())
    }
    pub const fn source(&self) -> source::SourceId {
        self.source
    }
    pub const fn schema(&self) -> SymbolHandle {
        self.schema
    }
    pub const fn custody(&self) -> DeclarationCustody {
        self.custody
    }
    pub fn application(&self) -> &TraitApplicationEnvironment {
        &self.application
    }
}

pub fn select_program_entry_contract(
    typed: &Arc<FrozenTypedTrees>,
    profile: target::TargetProfile,
    packages: Option<&PackageCompilationInputs>,
) -> Result<SelectedProgramEntryContractSource, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let slot = profile.program_entry_slot();
    let contract = slot.physical_contract_package.ok_or_else(|| {
        refuse(format!(
            "target `{}` has no physical entry contract",
            profile.target_name(),
        ))
    })?;
    let default_schema = slot
        .boundary_schema
        .ok_or_else(|| refuse("selected target entry has no boundary schema".into()))?;
    let selection = EntryContractSource::select(contract, packages);
    if selection == EntryContractSource::Unavailable {
        return Err(refuse(
            "target entry contract has no unambiguous admitted source owner".into(),
        ));
    }
    let role = crate::program_entry_semantic_binding_role(contract);
    let binding = packages.and_then(|inputs| inputs.accepted_semantic_binding(role));
    let sources = typed
        .symbols
        .source_map()
        .ok_or_else(|| refuse("entry contract selection has no source inventory".into()))?;
    let mut matches = sources.selected_files().filter(|file| {
        file.dependency_scope == source::DependencyScope::Product
            && file.path.strip_prefix(&file.package_root).ok()
                == Some(std::path::Path::new(contract.package_relative_source()))
            && match selection {
                EntryContractSource::Bundled => {
                    file.origin == source::SourceOrigin::ToolchainContract
                        && file.package_identity.is_none()
                }
                EntryContractSource::Package(owner) => {
                    file.origin == source::SourceOrigin::User
                        && file.package_identity == Some(owner)
                }
                EntryContractSource::Unavailable => false,
            }
    });
    let (Some(file), None) = (matches.next(), matches.next()) else {
        return Err(refuse(format!(
            "target `{}` entry contract does not have one exact selected source occurrence",
            profile.target_name()
        )));
    };
    let mut declarations = typed.traits().iter().filter(|declaration| {
        if !declaration.is_boundary
            || !typed
                .symbols
                .symbol_source_span(declaration.symbol)
                .is_some_and(|span| span.source_id == file.source_id)
        {
            return false;
        }
        if let Some(binding) = binding {
            typed.trait_declaration_path(declaration) == binding.declaration_path()
        } else {
            let parent = typed.symbols.get(declaration.symbol).parent;
            declaration.name.as_str() == default_schema
                && parent.is_valid()
                && matches!(
                    typed.symbols.get(parent).kind,
                    symbols::SymbolKind::Root | symbols::SymbolKind::Module
                )
        }
    });
    let (Some(schema), None) = (declarations.next(), declarations.next()) else {
        return Err(refuse(
            "selected entry contract source does not declare one exact boundary schema".into(),
        ));
    };
    let custody = match selection {
        EntryContractSource::Bundled => DeclarationCustody::ToolchainContract,
        EntryContractSource::Package(owner) => DeclarationCustody::Package(owner),
        EntryContractSource::Unavailable => unreachable!("unavailable source refused above"),
    };
    let application = AppliedTraitScope::abstract_scope(schema);
    let environment = application.environment();
    Ok(SelectedProgramEntryContractSource {
        owner: ProductSourceIdentity::new(typed),
        source: file.source_id,
        schema: schema.symbol,
        custody,
        application: environment,
    })
}
