pub use source_symbol_table::source_symbols::*;
use source_text::SourceSpan;
use symbols::SymbolHandle;

pub fn capture_build_selection_origin(
    symbols: &SymbolTable,
    activation: SymbolHandle,
    author: SymbolHandle,
    occurrence: SourceSpan,
    event_ordinal: Option<u64>,
) -> Result<crate::declaration_selection::BuildSelectionOrigin, &'static str> {
    let activation = retain_declaration(symbols, activation)?;
    let author = retain_declaration(symbols, author)?;
    if author.source().source_id != occurrence.source_id {
        return Err("Build selection occurrence does not belong to its lexical author");
    }
    crate::declaration_selection::BuildSelectionOrigin::from_retained_parts(
        activation,
        author,
        occurrence.span,
        event_ordinal,
    )
}
