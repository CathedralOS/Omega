//! Scoped source roots and normalization selected by Build. The source owner
//! prepares and judges this roster before a product can publish its code.

use diagnostics::Diagnostic;
use std::sync::Arc;

/// The source choices bound after preparation to its actual returned owner.
/// Rejudging a grown roster reuses these inputs; native consumers read only
/// the canonical contracts those source choices formed in the module.
#[derive(Debug, Clone, Default)]
pub struct SourceNormalizationInputs {
    pub calling: SourceCallingInputs,
    pub providers: Vec<crate::TerminalProviderSelectionInput>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceCallingInputs {
    pub applications: Vec<crate::source_state::trait_definition::TraitApplicationEnvironment>,
    pub entries: Vec<SourceProgramEntryInput>,
}

#[derive(Debug, Clone)]
pub struct SourceProgramEntryInput {
    pub target: String,
    pub application: crate::source_state::trait_definition::TraitApplicationEnvironment,
    pub physical_source_digest: [u8; 32],
    pub implementation: symbols::SymbolHandle,
}

pub struct SourceCallingContext<'source> {
    source: &'source crate::source_state::TypedTrees,
    applications: &'source [crate::source_state::trait_definition::TraitApplicationEnvironment],
    entries: Vec<crate::TerminalProgramEntryContractSelection<'source>>,
}

impl SourceCallingInputs {
    pub fn borrow<'source>(
        &'source self,
        source: &'source crate::source_state::TypedTrees,
    ) -> SourceCallingContext<'source> {
        SourceCallingContext {
            source,
            applications: &self.applications,
            entries: self
                .entries
                .iter()
                .map(|entry| crate::TerminalProgramEntryContractSelection {
                    target: &entry.target,
                    application: &entry.application,
                    physical_source_digest: entry.physical_source_digest,
                    implementation: entry.implementation,
                })
                .collect(),
        }
    }
}

impl SourceCallingContext<'_> {
    pub fn selection(&self) -> Option<crate::TerminalCallingPolicySelections<'_>> {
        (!self.entries.is_empty() || !self.applications.is_empty()).then_some(
            crate::TerminalCallingPolicySelections {
                source: self.source,
                applications: self.applications,
                entries: &self.entries,
            },
        )
    }
}

impl SourceNormalizationInputs {
    pub fn providers<'source>(
        &'source self,
        source: &'source crate::source_state::TypedTrees,
    ) -> Option<crate::TerminalProviderSelections<'source>> {
        (!self.providers.is_empty()).then_some(crate::TerminalProviderSelections {
            source,
            choices: &self.providers,
        })
    }
    fn is_empty(&self) -> bool {
        self.providers.is_empty()
            && self.calling.applications.is_empty()
            && self.calling.entries.is_empty()
    }
}

/// What a Product compilation asks the lowered-form checker to judge: the
/// roots the evaluated Build states (`crate::TerminalSourceCompilation`
/// holds the same fields by reference). It is kept beside the judgment it
/// produced because provider settlement extends it: the supply chosen for a
/// requirement the accepted module reaches is a further root of the same
/// compilation, known only after the first judgment.
#[derive(Debug, Clone, Default)]
pub struct SourceCompilationRoster {
    /// Canonical declaration descriptors required by this product.
    pub declaration_metadata: Vec<symbols::SymbolHandle>,
    /// The Build's bound program entries, which drive deployment demand.
    pub entries: Vec<symbols::SymbolHandle>,
    /// An inspection sets the artifact entry without binding it for deployment.
    /// Its Build entries remain among the selected declarations.
    pub inspection_root: Option<symbols::SymbolHandle>,
    /// The package whose declarations the check claims; `None` for a source
    /// checked outside a package.
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Layout-scope entries, native binding declarations and chosen opaque
    /// representations the Build names.
    pub selected: Vec<symbols::SymbolHandle>,
    /// One visible-continuation obligation per executed Build binding.
    pub bindings: Vec<crate::source_state::callable_signature_shapes::BoundCallableShape>,
    /// The providers chosen to supply boundary slots: the Build's own
    /// selections first, then what settlement chose for reached requirements.
    pub supplies: Vec<crate::TerminalSourceSupply>,
    pub normalization: Arc<SourceNormalizationInputs>,
}

impl SourceCompilationRoster {
    /// Borrow the full roster for source preparation and judgment alike.
    pub fn preparation_compilation(
        &self,
    ) -> Result<crate::TerminalSourceCompilation<'_>, Vec<Diagnostic>> {
        if !self.normalization.is_empty() {
            return Err(vec![Diagnostic::error(
                "source choices must bind to the actual prepared owner before judgment",
            )]);
        }
        Ok(crate::TerminalSourceCompilation {
            declaration_metadata: &self.declaration_metadata,
            entries: self
                .inspection_root
                .as_ref()
                .map_or(self.entries.as_slice(), std::slice::from_ref),
            subjects: std::slice::from_ref(&self.subject),
            purpose: source_provenance::DependencyScope::Product,
            selected: &self.selected,
            bindings: &self.bindings,
            supplies: &self.supplies,
            calling: None,
            providers: None,
        })
    }

    /// The lowered-form checker's judgment of this roster over `typed`: the
    /// closure of the roots, the check form of each generic machine the
    /// subject declares, and the refinement witness of each implementation
    /// row and chosen supply. Its verdict is the only verdict of a product
    /// compilation: it accepts only when every part does, a roster it does
    /// not accept produces no module, and a missing publication never
    /// selects another stage's module.
    pub fn judge(
        &self,
        typed: Arc<crate::source_state::FrozenTypedTrees>,
    ) -> Result<Arc<crate::AcceptedTerminalSource>, Vec<Diagnostic>> {
        let calling = self.normalization.calling.borrow(&typed);
        let accepted = crate::AcceptedTerminalSource::judge(
            Arc::clone(&typed),
            crate::TerminalSourceCompilation {
                declaration_metadata: &self.declaration_metadata,
                entries: &self.entries,
                subjects: std::slice::from_ref(&self.subject),
                purpose: source_provenance::DependencyScope::Product,
                selected: &self.selected,
                bindings: &self.bindings,
                supplies: &self.supplies,
                calling: calling.selection(),
                providers: self.normalization.providers(&typed),
            },
            terminal_codec::TerminalPublication::Portable,
        )?;
        Ok(Arc::new(accepted))
    }
}
