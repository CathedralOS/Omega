//! Bind the returned Build's selections to the dependency's own Product declarations.
use crate::compile_product::provider_declarations::capture_provider_selection;
use crate::{StageSource, declaration_selection};
use declaration_selection::{CapturedNativeIntent, NativeBindingFormation};
use diagnostics::Diagnostic;
use retained_product_selections::{CapturedNativeDeclaration, RetainedProviderSelection};
use semantic_vocabulary::source_selections::ProductTarget;

pub struct PackageHandoffDeclarations {
    pub providers: Vec<RetainedProviderSelection>,
    pub native: Vec<CapturedNativeDeclaration>,
    pub intents: Vec<CapturedNativeIntent>,
}

/// The providers selected for one immediate interpreted product. Keep overrides
/// and package defaults separate until the receiving Build settles each slot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterpretedProviderSelections {
    pub(crate) overrides: Vec<RetainedProviderSelection>,
    pub(crate) defaults: Vec<RetainedProviderSelection>,
}

impl InterpretedProviderSelections {
    /// Package admission authenticates the supplier. Source judgment still
    /// re-forms each applicable declaration in the receiving Build context.
    pub fn extend_package_defaults(
        &mut self,
        defaults: impl IntoIterator<Item = RetainedProviderSelection>,
    ) {
        for choice in defaults {
            if choice
                .target
                .is_none_or(|target| target == ProductTarget::Interpreter)
                && !self.defaults.contains(&choice)
            {
                self.defaults.push(choice);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty() && self.defaults.is_empty()
    }
}

impl StageSource<'_> {
    /// Resolve the predecessor's executed selections while the original Build
    /// authors and their view of the successor's Product sources are available.
    pub fn interpreted_provider_selections(
        self,
    ) -> Result<InterpretedProviderSelections, Vec<Diagnostic>> {
        let applicable = |target: Option<ProductTarget>| {
            target.is_none_or(|target| target == ProductTarget::Interpreter)
        };
        let defaults = self
            .product
            .default_provider_selections
            .iter()
            .filter(|choice| applicable(choice.target))
            .cloned()
            .collect();
        let has_override = self.product.policy.executed.iter().any(|event| {
            event.request.operation == declaration_selection::BuildOperation::ProviderSelection
                && applicable(event.target)
        });
        let mut selections = InterpretedProviderSelections {
            defaults,
            ..Default::default()
        };
        if has_override {
            let (product, declarations) = self.resolve_declarations(&mut Default::default())?;
            let mut typed = declarations.typed().clone();
            let (_, admitted) = product.policy.admit(&mut typed, product.subject, None)?;
            selections.overrides = capture_providers(
                &typed,
                admitted
                    .providers
                    .iter()
                    .filter(|choice| applicable(choice.target)),
            )?;
        }
        Ok(selections)
    }
}

fn capture_providers<'a>(
    typed: &crate::source_state::TypedTrees,
    selected: impl Iterator<Item = &'a crate::compile_product::provider_declarations::ProviderSelection>,
) -> Result<Vec<RetainedProviderSelection>, Vec<Diagnostic>> {
    selected
        .map(|row| {
            capture_provider_selection(typed, row).map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .collect()
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
    let providers = capture_providers(&typed, admitted.providers.iter())?;
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
