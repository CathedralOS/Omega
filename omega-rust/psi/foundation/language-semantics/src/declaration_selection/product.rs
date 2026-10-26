//! Source-owned product selections, independent of an execution engine.
//!
//! Descriptions name declarations in the admitted product context. Execution
//! engines expose only opaque issued values and retain these payloads privately.
//! A completed binding carries its lexical request directly, never an AST
//! statement handle to reinterpret later. Authored paths remain locators until
//! the product join resolves them under that source's authority.
//! These are in-process source selections, not serialized Terminal identities.

/// Inert designated operands retained under their authored lexical scope.
/// They are not executable arguments or authority to publish a product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RootBindingRequest {
    pub source_span: source::SourceSpan,
    pub slot: String,
    pub implementation: RootBindingOperand,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RootBindingOperand {
    AuthoredPath(String),
    #[default]
    Description,
}

/// One product-entry description issued by the compiler-owned
/// `Build.product.entry` query during build evaluation. The evaluator keeps
/// this semantic payload in a private side table; the `ProductEntryRef` value
/// handed to evaluated code carries only an opaque index into it, so source
/// cannot read, convert, or fabricate the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductEntry {
    /// Exact product machine the description selected in its query's lexical
    /// package scope. Final admission rejoins this symbol; it never reselects
    /// by spelling.
    pub machine_symbol: symbols::SymbolHandle,
    /// The selected machine's authored name, retained for diagnostics only.
    pub machine_name: String,
    /// The root slot spelling the description was selected for. A later
    /// `Build::bind` must spell the same slot path.
    pub slot: String,
}

/// One product-type-schema description issued by the compiler-owned
/// `Build.product.schema` query during build evaluation. Same marker
/// discipline as `DescribedProductEntry`: the evaluator keeps this payload
/// in a private side table and the `ProductTypeSchema` value handed to
/// evaluated code carries only an opaque index into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductSchema {
    /// Exact product data declaration the description selected in its
    /// query's lexical package scope.
    pub schema_symbol: symbols::SymbolHandle,
    /// The selected declaration's canonical path, retained for `path()`
    /// inspection and diagnostics.
    pub canonical_path: String,
}

/// One product-provider description issued by the compiler-owned
/// `Build.product.provider` query during build evaluation. Same marker
/// discipline as `DescribedProductSchema`: the evaluator keeps this payload
/// in a private side table and the `ProductProviderRef` value handed to
/// evaluated code carries only an opaque index into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedProductProvider {
    /// Exact provider data declaration the description selected in its
    /// query's lexical package scope. A provider declaration is the nominal
    /// data type owning at least one `satisfies` machine.
    pub provider_symbol: symbols::SymbolHandle,
    /// The selected declaration's canonical path, retained for `path()`
    /// inspection and diagnostics.
    pub canonical_path: String,
}

/// The two source-language selection channels have different authority:
/// a path resolves under the binding author's scope, while an issued description
/// keeps the exact selection made under its original query author's scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootBindingSelection {
    AuthoredPath(String),
    Described(DescribedProductEntry),
}

/// One operation actually executed against the original Build receiver.
/// The emitter retains the admitted occurrence's lexical source; possession of
/// this record alone grants no package visibility or executable entry authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedRootBinding {
    pub source_span: source::SourceSpan,
    pub slot: String,
    pub implementation: RootBindingSelection,
}

/// Evaluated composition belongs to the executed request, never a source
/// expression recognizer. Omission explicitly means fused composition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderCompositionMode {
    #[default]
    Fused,
    Independent,
}

/// A closed declaration path under its own authored lexical/package scope.
/// It is inert description data until Product's scoped declaration resolver
/// establishes the required role. It carries no old source-arena handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationOperand {
    pub path: Vec<String>,
    pub source_span: source::SourceSpan,
}

/// An operation observed on the original Build activation. The producer
/// records the whole actual request so later consumers never reconstruct an
/// executed helper specialization from source text or backing expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedProductProviderSelection {
    pub source_span: source::SourceSpan,
    pub selecting_machine: source::SourceSpan,
    pub subject: ProductDeclarationOperand,
    pub provider: ProductDeclarationOperand,
    pub composition_mode: ProviderCompositionMode,
}
