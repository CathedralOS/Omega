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

    /// Files needed to re-form explicit selections, and defaults whose subjects
    /// have entered this Build. Generated files arrive through their tree handoff.
    pub fn required_source_paths(
        &self,
        loaded: &crate::representations::source_inputs::SourceMap,
    ) -> Result<Vec<std::path::PathBuf>, Vec<Diagnostic>> {
        use source_admission::SourceResolutionStratum;
        let build_files = loaded
            .selected_files()
            .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build)
            .map(|file| file.path.as_path())
            .collect::<std::collections::HashSet<_>>();
        let defaults = self.defaults.iter().filter(|choice| {
            choice
                .subject
                .declarations()
                .any(|subject| build_files.contains(subject.source().path.as_path()))
        });
        let mut missing = std::collections::BTreeSet::new();
        let mut require = |file: &source_admission::SourceFile| {
            if file.resolution_stratum == SourceResolutionStratum::Base
                && !build_files.contains(file.path.as_path())
            {
                missing.insert(file.path.clone());
            }
        };
        for choice in self.overrides.iter().chain(defaults) {
            for subject in choice.subject.declarations() {
                require(subject.source());
            }
            require(choice.provider.source());
            for application in [
                choice.subject.application(),
                choice.provider_application.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                for span in application.declaration_spans() {
                    let file = application.sources.get(span.source_id).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "interpreter provider application lost its captured source",
                        )]
                    })?;
                    require(file);
                }
            }
        }
        Ok(missing.into_iter().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty() && self.defaults.is_empty()
    }
}

impl crate::ProductDeclarations {
    /// Resolve the predecessor's executed selections with its original authors
    /// and the exact files and registered packages granted to its product.
    pub fn interpreted_provider_selections(
        &self,
        policy: crate::compile_product::ProductPolicyInputs<'_>,
        subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> Result<InterpretedProviderSelections, Vec<Diagnostic>> {
        let applicable = |target: Option<ProductTarget>| {
            target.is_none_or(|target| target == ProductTarget::Interpreter)
        };
        let selected = policy.executed.iter().any(|event| {
            event.request.operation == declaration_selection::BuildOperation::ProviderSelection
                && applicable(event.target)
        });
        if !selected {
            return Ok(Default::default());
        }
        let mut typed = self.typed.as_ref().clone();
        let (_, admitted) = policy.admit(&mut typed, subject, None)?;
        Ok(InterpretedProviderSelections {
            overrides: capture_providers(
                &typed,
                admitted
                    .providers
                    .iter()
                    .filter(|choice| applicable(choice.target)),
            )?,
            defaults: Vec::new(),
        })
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
