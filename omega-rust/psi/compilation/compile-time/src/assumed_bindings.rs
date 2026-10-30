//! Source-admitted, non-callable Product requirement relationships.
//!
//! `product_declarations` admits executed requests against the consuming root.
//! Omega owns runtime compatibility, provider selection and risk acceptance;
//! these declarations grant Build no proposition or callable.

use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContract {
    pub required: SymbolHandle,
    pub supplied: SymbolHandle,
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}
