//! Compiler-owned `size_of<T>()` and `align_of<T>()` query names.
//!
//! These are core names, not keywords: a call whose target resolved to an
//! authored declaration stays an ordinary call, and only a receiverless,
//! unresolved spelling with exactly one static type argument names the
//! request. A portable query representation must precede selected-layout
//! evaluation. Until that path exists, source lowering reports unimplemented;
//! native layout must not rewrite typed source into host-dependent literals.

/// Which layout extent a `size_of`/`align_of` query asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum DataLayoutQueryKind {
    /// `size_of<T>()`: the fixed complete extent in bytes, including padding.
    Size,
    /// `align_of<T>()`: the validated alignment in bytes; a known alignment
    /// may remain available even when the plan's encoded size is dynamic.
    Alignment,
}

impl DataLayoutQueryKind {
    /// The compiler-known query name, or `None` for an ordinary call target.
    /// Callers must still require an unresolved target symbol and no receiver
    /// before treating a spelling match as a layout query.
    pub fn from_target_name(name: &str) -> Option<Self> {
        match name {
            "size_of" => Some(Self::Size),
            "align_of" => Some(Self::Alignment),
            _ => None,
        }
    }

    /// The authored spelling, for diagnostics that name the rejected query.
    pub fn target_name(self) -> &'static str {
        match self {
            Self::Size => "size_of",
            Self::Alignment => "align_of",
        }
    }
}

/// Whether a call shape is a data-layout query candidate: the compiler-known
/// spelling on a receiverless target that did not resolve to a declaration.
/// Shapes failing this predicate stay on the ordinary call path.
pub fn is_data_layout_query_spelling(
    target: &str,
    target_symbol_valid: bool,
    receiver_valid: bool,
) -> bool {
    !target_symbol_valid
        && !receiver_valid
        && DataLayoutQueryKind::from_target_name(target).is_some()
}
