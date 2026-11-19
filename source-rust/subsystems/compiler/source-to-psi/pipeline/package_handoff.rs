//! Bind the returned Build's selections to the dependency's own Product declarations.
use super::*;
use declaration_selection::{CapturedNativeIntent, NativeBindingFormation};
use evaluation::native_bindings::CapturedNativeDeclaration;
use evaluation::provider_declarations::RetainedProviderSelection;

pub struct PackageHandoffDeclarations {
    pub providers: Vec<RetainedProviderSelection>,
    pub native: Vec<CapturedNativeDeclaration>,
    pub intents: Vec<CapturedNativeIntent>,
}

pub fn package_handoff_declarations(
    source: StageSource<'_>,
    formations: &[NativeBindingFormation],
) -> Result<PackageHandoffDeclarations, Vec<Diagnostic>> {
    let package = source.selections.subject.ok_or_else(|| {
        vec![Diagnostic::error(
            "a dependency Build handoff requires a named package",
        )]
    })?;
    let mut declarations = source.resolve()?;
    let (_, admitted) = source
        .selections
        .policy
        .admit(&mut declarations, Some(package))?;
    let providers = admitted
        .providers
        .iter()
        .map(|selected| {
            RetainedProviderSelection::capture(declarations.typed(), selected)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (native, intents) = declarations.capture_package_native_bindings(
        package,
        formations,
        source.selections.policy.native_bindings,
    )?;
    Ok(PackageHandoffDeclarations {
        providers,
        native,
        intents,
    })
}
