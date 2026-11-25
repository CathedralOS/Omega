//! Bind the returned Build's selections to the dependency's own Product declarations.
use crate::compile_product::provider_declarations::capture_provider_selection;
use crate::{StageSource, declaration_selection};
use declaration_selection::{CapturedNativeIntent, NativeBindingFormation};
use diagnostics::Diagnostic;
use retained_product_selections::{CapturedNativeDeclaration, RetainedProviderSelection};

pub struct PackageHandoffDeclarations {
    pub providers: Vec<RetainedProviderSelection>,
    pub native: Vec<CapturedNativeDeclaration>,
    pub intents: Vec<CapturedNativeIntent>,
}

pub fn package_handoff_declarations(
    source: StageSource<'_>,
    formations: &[NativeBindingFormation],
) -> Result<PackageHandoffDeclarations, Vec<Diagnostic>> {
    let package = source.product.subject.ok_or_else(|| {
        vec![Diagnostic::error(
            "a dependency Build handoff requires a named package",
        )]
    })?;
    let (product, declarations) = source.resolve_declarations(&mut Default::default())?;
    // Dependency handoff uses only the package's typed declarations and Build requests.
    let mut typed = declarations.typed().clone();
    let (_, admitted) = product.policy.admit(&mut typed, Some(package), None)?;
    let providers = admitted
        .providers
        .iter()
        .map(|selected| {
            capture_provider_selection(&typed, selected)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (native, intents) = crate::compile_product::capture_package_native_bindings(
        &mut typed,
        package,
        formations,
        product.policy.native_bindings,
    )?;
    Ok(PackageHandoffDeclarations {
        providers,
        native,
        intents,
    })
}
