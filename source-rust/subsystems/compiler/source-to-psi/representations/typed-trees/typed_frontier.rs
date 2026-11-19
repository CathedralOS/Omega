/// The Product program as Psi 04 left it, with the entries this Build bound.
#[derive(Debug, Clone)]
pub struct TypedFrontier {
    /// The actual frozen typed source returned by Psi 04 preparation.
    pub typed: crate::source_state::FrozenTypedTrees,
    /// Actual constant invocation custody retained with this source owner.
    pub source_completion_accounting: crate::compile_time::SourceCompletionAccounting,
    /// The Build's bound program roots, in the order the checker received
    /// them. Empty when the Build binds no entry.
    pub entries: Vec<symbols::SymbolHandle>,
    /// The package whose declarations this check claims. `None` for a source
    /// checked outside a package. The structural judgment needs this subject
    /// because the trees also hold declarations of dependency packages.
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// The declarations the Build's selections name: selected providers and
    /// what each provides, layout-scope entries and chosen opaque
    /// representations. The check claims them and their closure even when no
    /// entry reaches them.
    pub selected: Vec<symbols::SymbolHandle>,
    /// The distinct visible-continuation obligations of the executed bindings.
    pub bindings: Vec<crate::source_state::callable_signature_shapes::BoundCallableShape>,
    pub supplies: Vec<crate::TerminalSourceSupply>,
    pub normalization: std::sync::Arc<crate::SourceNormalizationInputs>,
}

impl TypedFrontier {
    pub fn provider_selections(&self) -> Option<crate::TerminalProviderSelections<'_>> {
        self.normalization.providers(&self.typed)
    }
}
