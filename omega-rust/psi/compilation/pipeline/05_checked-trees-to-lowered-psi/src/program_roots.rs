//! Borrowed source roots of one portable program, not target selections.

/// Extra callable and interface roots retained alongside the default entry.
/// A callable root requests a checked body; an interface root retains its
/// declaration and candidate catalog. Neither grants execution authority.
#[derive(Clone, Copy, Default)]
pub struct TerminalProgramRoots<'a> {
    pub machines: &'a [symbols::SymbolHandle],
    pub boundaries: &'a [symbols::SymbolHandle],
}
