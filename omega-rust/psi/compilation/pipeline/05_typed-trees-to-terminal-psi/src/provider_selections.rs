//! Source provider policy becomes declared Terminal selections before source
//! is released. Selection is descriptive; body refinement and permissions are
//! separate ordinary judgments.

use arena::Handle;
use language_semantics::declaration_selection::CompositionMode;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::{TypedTrees, trait_definition::TraitApplicationEnvironment};

#[derive(Debug, Clone)]
pub struct ProviderSelectionInput {
    pub target: String,
    pub schema: TraitApplicationEnvironment,
    pub provider: SymbolHandle,
    pub provider_application:
        Option<Handle<typed_trees::product_operand::ProductOperandApplication>>,
    pub composition_mode: CompositionMode,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderSelections<'source> {
    pub source: &'source TypedTrees,
    pub choices: &'source [ProviderSelectionInput],
}

pub(crate) struct SourceProviderSelections<'source> {
    pub(crate) source: &'source typed_trees::TypedTrees,
    pub(crate) rows: Vec<SourceProviderSelection>,
}

pub(crate) struct SourceProviderSelection {
    pub(crate) target: String,
    pub(crate) schema_declaration: String,
    pub(crate) schema_application: String,
    pub(crate) surface: std::sync::Arc<language_semantics::service_surface::ServiceSurface>,
    pub(crate) provider_declaration: SymbolHandle,
    pub(crate) provider_identity: String,
    pub(crate) provider_application: Option<String>,
    pub(crate) provider_package: Option<PackageKeyIdentity>,
    pub(crate) provider_custody: DeclarationCustody,
    pub(crate) composition_mode: CompositionMode,
}

pub(crate) fn publish(
    context: MachineId,
    source: &SourceProviderSelections<'_>,
    elaboration: &mut crate::Elaboration,
) -> Checked<()> {
    use crate::read::services::unavailable;
    if !elaboration.module.provider_selections.is_empty() {
        return Err(unavailable(
            context,
            "competing source provider-selection publication",
        ));
    }
    let source_identities =
        source::source_identity::toolchain_source_identities(source.source.symbols.source_files())
            .map_err(|reason| unavailable(context, reason))?;
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    let mut rows: Vec<terminal_psi::TerminalProviderSelection> =
        Vec::with_capacity(source.rows.len());
    let mut schemas: Vec<terminal_psi::TerminalServiceSchema> = Vec::new();
    let source_owner = source.source;
    let prepared = (|| {
        for source in &source.rows {
            if source.target.is_empty()
                || source.provider_identity.is_empty()
                || !source.provider_declaration.is_valid()
                || source.schema_declaration.is_empty()
                || source.surface.root_application_identity()
                    != Some(source.schema_application.as_str())
                || source
                    .provider_application
                    .as_ref()
                    .is_some_and(String::is_empty)
            {
                return Err(unavailable(
                    context,
                    "source provider selection lost its exact normalized meaning",
                ));
            }
            let declaration = identities.intern(&source.schema_declaration);
            let application = identities.intern(&source.schema_application);
            let mut existing = elaboration
                .module
                .service_schemas
                .iter()
                .chain(&schemas)
                .filter(|row| row.declaration == declaration && row.application == application);
            match (existing.next(), existing.next()) {
                (None, None) => schemas.push(terminal_psi::TerminalServiceSchema {
                    declaration,
                    application,
                    surface: std::sync::Arc::clone(&source.surface),
                }),
                (Some(existing), None) if existing.surface == source.surface => {}
                _ => {
                    return Err(unavailable(
                        context,
                        "provider schema differs from an existing canonical source schema",
                    ));
                }
            }
            if rows.iter().any(|row| {
                row.target == source.target
                    && row.schema_declaration == declaration
                    && row.schema_application == application
            }) {
                return Err(unavailable(
                    context,
                    "source provider selection repeats an exact target slot",
                ));
            }
            let provider_declaration = crate::declare::nominal(
                source_owner,
                &source_identities,
                source.provider_declaration,
                &mut identities,
            )
            .map_err(|reason| unavailable(context, reason))?;
            rows.push(terminal_psi::TerminalProviderSelection {
                target: source.target.clone(),
                schema_declaration: declaration,
                schema_application: application,
                provider_declaration,
                provider_identity: identities.intern(&source.provider_identity),
                provider_application: source
                    .provider_application
                    .as_ref()
                    .map(|value| identities.intern(value)),
                provider_package: source.provider_package,
                provider_custody: source.provider_custody,
                composition_mode: source.composition_mode,
            });
        }
        Ok(())
    })();
    elaboration.module.identities = identities.build();
    prepared?;
    elaboration.module.service_schemas.extend(schemas);
    elaboration.module.provider_selections = rows;
    Ok(())
}
