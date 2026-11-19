//! Select the exact contract source that discovery chose for a target entry.
//! This nominates source to check; package policy still admits its schema.

use crate::product::source_targets::{
    SourceEntryContractOwner as EntryContractSource, SourceTarget,
};
use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
use crate::source_state::{FrozenTypedTrees, ProductSourceIdentity};
use diagnostics::Diagnostic;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Debug, Clone)]
pub struct SelectedProgramEntryContractSource {
    owner: ProductSourceIdentity,
    source: source_text::SourceId,
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
    pub const fn source(&self) -> source_text::SourceId {
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
    profile: &SourceTarget,
) -> Result<SelectedProgramEntryContractSource, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let contract = profile.entry_contract.as_ref().ok_or_else(|| {
        refuse(format!(
            "target `{}` has no physical entry contract",
            profile.name,
        ))
    })?;
    let default_schema = profile
        .boundary_schema
        .ok_or_else(|| refuse("selected target entry has no boundary schema".into()))?;
    let selection = contract.owner;
    if selection == EntryContractSource::Unavailable {
        return Err(refuse(
            "target entry contract has no unambiguous admitted source owner".into(),
        ));
    }
    let binding = contract.accepted_declaration.as_deref();
    let sources = typed
        .symbols
        .source_map()
        .ok_or_else(|| refuse("entry contract selection has no source inventory".into()))?;
    let mut matches = sources.selected_files().filter(|file| {
        file.dependency_scope == source_provenance::DependencyScope::Product
            && file.path.strip_prefix(&file.package_root).ok()
                == Some(std::path::Path::new(contract.relative_source))
            && match selection {
                EntryContractSource::Bundled => {
                    file.origin == source_admission::SourceOrigin::ToolchainContract
                        && file.package_identity.is_none()
                }
                EntryContractSource::Package(owner) => {
                    file.origin == source_admission::SourceOrigin::User
                        && file.package_identity == Some(owner)
                }
                EntryContractSource::Unavailable => false,
            }
    });
    let (Some(file), None) = (matches.next(), matches.next()) else {
        return Err(refuse(format!(
            "target `{}` entry contract does not have one exact selected source occurrence",
            profile.name
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
            typed.trait_declaration_path(declaration) == binding
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

/// Resolve the selected target schema inside its admitted contract source.
pub fn selected_program_entry_boundary(
    catalog: crate::source_state::service_declarations::ServiceCatalog<'_>,
    profile: &SourceTarget,
) -> Result<symbols::SymbolHandle, Vec<diagnostics::Diagnostic>> {
    use crate::source_state::service_declarations::ProviderSchemaDeclaration;
    use diagnostics::Diagnostic;
    let failure = |message| vec![Diagnostic::error(message)];
    let context = catalog.context();
    if context.scope != source_provenance::DependencyScope::Product {
        return Err(failure(
            "selected entry boundary requires the Product service catalog",
        ));
    }
    let typed = context.typed;
    let exact_sources = catalog.exact_toolchain_sources();
    let contract = profile
        .entry_contract
        .as_ref()
        .ok_or_else(|| failure("selected target has no source physical entry contract"))?;
    let schema = profile
        .boundary_schema
        .ok_or_else(|| failure("selected target has no calling boundary schema"))?;
    let selected_owner = contract.owner;
    if selected_owner == EntryContractSource::Unavailable {
        return Err(failure(
            "selected target entry contract has no unambiguous accepted supplier",
        ));
    }
    let mut matching = Vec::new();
    for declaration in catalog.declared_traits() {
        let ProviderSchemaDeclaration::BoundaryTrait(symbol) = declaration.schema else {
            continue;
        };
        let definition = typed
            .traits()
            .iter()
            .find(|row| row.symbol == symbol)
            .ok_or_else(|| failure("catalog boundary has no actual typed declaration"))?;
        // Slot schema is an unqualified name inside the authenticated contract source.
        if definition.name.as_str() != schema {
            continue;
        }
        let owns = match selected_owner {
            EntryContractSource::Package(package) => {
                declaration.package == Some(package)
                    && typed
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .and_then(|span| typed.symbols.source_file(span))
                        .is_some_and(|file| {
                            file.path.strip_prefix(&file.package_root).ok()
                                == Some(std::path::Path::new(contract.relative_source))
                        })
            }
            EntryContractSource::Bundled => typed
                .symbols
                .symbol_provenance_source_span(symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source_admission::SourceOrigin::ToolchainContract
                        && file.path.ends_with(contract.relative_source)
                        && exact_sources
                            .iter()
                            .any(|(source, _)| *source == file.source_id)
                }),
            EntryContractSource::Unavailable => false,
        };
        if !owns {
            continue;
        }
        // PreparedServiceCatalog authenticated this exact package/toolchain
        // nominal owner with the supplied digest registry. Text is not enough.
        declaration
            .nominal_identity
            .as_ref()
            .map_err(|error| vec![error.clone()])?;
        let surface = declaration.surface.map_err(|error| vec![error])?;
        surface.exact_requirements().map_err(|error| vec![error])?;
        matching.push(symbol);
    }
    match matching.as_slice() {
        [symbol] => Ok(*symbol),
        [] => Err(failure(
            "selected entry source does not publish its exact target boundary schema",
        )),
        _ => Err(failure(
            "selected entry source publishes competing target boundary declarations",
        )),
    }
}
