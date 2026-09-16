use crate::{SymbolHandle, SymbolSpan};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolPath {
    pub root: SymbolHandle,
    pub members: SymbolSpan,
}
