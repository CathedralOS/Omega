//! Source-checked representability at physical integer stores.
//!
//! A layout may store fewer bits than the field's semantic type. This row
//! records that checking proved the completed RHS fits that exact destination;
//! it does not change the field type or authorize truncation. Lowering can
//! consume the decision without constructing a receiver proof.

use semantic_vocabulary::IntegerType;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStoredIntegerWrite {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub destination: ExpressionHandle,
    pub value: ExpressionHandle,
    pub field: SymbolHandle,
    pub stored_type: IntegerType,
}

impl Default for CheckedStoredIntegerWrite {
    fn default() -> Self {
        // Arena's absent row has no source occurrence; its placeholder width
        // grants no decision without valid machine/state/destination handles.
        Self {
            machine: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            destination: ExpressionHandle::invalid(),
            value: ExpressionHandle::invalid(),
            field: SymbolHandle::invalid(),
            stored_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .expect("u8 has a valid fixed width"),
        }
    }
}
