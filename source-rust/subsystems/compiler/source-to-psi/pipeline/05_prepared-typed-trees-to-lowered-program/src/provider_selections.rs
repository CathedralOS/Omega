//! Source provider policy becomes declared Terminal selections before source
//! is released. Selection is descriptive; body refinement and permissions are
//! separate ordinary judgments.

use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use semantic_vocabulary::source_selections::CompositionMode;
use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

pub use lowered_source_publication::provider_selections::{
    ProviderSelectionInput, ProviderSelections,
};

pub(crate) struct SourceProviderSelections<'source> {
    pub(crate) source: &'source crate::source_state::TypedTrees,
    pub(crate) rows: Vec<SourceProviderSelection>,
}

pub(crate) struct SourceProviderSelection {
    pub(crate) target: String,
    pub(crate) schema_declaration: String,
    pub(crate) schema_application: String,
    pub(crate) surface: std::sync::Arc<service_surface::ServiceSurface>,
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
    if !elaboration.module.interfaces.provider_selections.is_empty() {
        return Err(unavailable(
            context,
            "competing source provider-selection publication",
        ));
    }
    let source_identities = source_admission::source_identity::toolchain_source_identities(
        source.source.symbols.source_files(),
    )
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
            schemas.push(terminal_psi::TerminalServiceSchema {
                declaration,
                application,
                surface: std::sync::Arc::clone(&source.surface),
            });
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
    elaboration.module.interfaces.service_schemas =
        terminal_psi::TerminalServiceSchema::join(&elaboration.module.interfaces.service_schemas, &schemas)
            .map_err(|reason| unavailable(context, reason))?;
    elaboration.module.interfaces.provider_selections = rows;
    Ok(())
}
