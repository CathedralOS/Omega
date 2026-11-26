//! Select the exact contract source that discovery chose for a target entry.
//! This nominates source to check; package policy still admits its schema.

use crate::compile_product::source_targets::SourceTarget;
use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
use crate::source_state::{FrozenTypedTrees, ProductSourceIdentity};
use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SelectedProgramEntryContractSource {
    owner: ProductSourceIdentity,
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
    let sources = typed
        .symbols
        .source_map()
        .ok_or_else(|| refuse("entry contract selection has no source inventory".into()))?;
    let mut matches = sources.selected_files().filter(|file| contract.owns(file));
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
        let path = typed.trait_declaration_path(declaration);
        let parent = typed.symbols.get(declaration.symbol).parent;
        path == default_schema
            && parent.is_valid()
            && matches!(
                typed.symbols.get(parent).kind,
                symbols::SymbolKind::Root | symbols::SymbolKind::Module
            )
    });
    let (Some(schema), None) = (declarations.next(), declarations.next()) else {
        return Err(refuse(
            "selected entry contract source does not declare one exact boundary schema".into(),
        ));
    };
    let application = AppliedTraitScope::abstract_scope(schema);
    let environment = application.environment();
    Ok(SelectedProgramEntryContractSource {
        owner: ProductSourceIdentity::new(typed),
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
        // The slot names the package-relative declaration in its authenticated source.
        if typed.trait_declaration_path(definition) != schema {
            continue;
        }
        let owns = typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| typed.symbols.source_file(span))
            .is_some_and(|file| {
                contract.owns(file)
                    && declaration.package == file.package_identity
                    && (file.package_identity.is_some()
                        || exact_sources
                            .iter()
                            .any(|(source, _)| *source == file.source_id))
            });
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

pub(super) fn selected_program_entry(
    root_bindings: &[crate::RootBinding],
    selected_profile: &SourceTarget,
    targets: &[SourceTarget],
) -> Result<Option<usize>, Vec<Diagnostic>> {
    if root_bindings.is_empty() {
        return Ok(None);
    }
    let mut diagnostics = Vec::new();
    let mut selected_ordinals = Vec::new();
    for (ordinal, binding) in root_bindings.iter().enumerate() {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match crate::compile_product::source_targets::target_owner(targets, profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let required_slot = &profile.entry;
        if required_slot.slot_name != slot_name {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.name, binding.slot
            )));
            continue;
        };
        if profile.name == selected_profile.name && required_slot == &selected_profile.entry {
            selected_ordinals.push(ordinal);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let slot = &selected_profile.entry;
    match selected_ordinals.as_slice() {
        [ordinal] => Ok(Some(*ordinal)),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no bound required root slot `{}::{}`",
            selected_profile.name, slot.owner, slot.slot_name
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has more than one bound required root slot `{}::{}`",
            selected_profile.name, slot.owner, slot.slot_name
        ))]),
    }
}
