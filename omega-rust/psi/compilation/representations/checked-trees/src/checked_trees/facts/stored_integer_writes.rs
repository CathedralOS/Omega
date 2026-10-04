//! Source-checked representability at physical integer stores.
//!
//! A layout may store fewer bits than the field's semantic type. This row
//! records that checking proved the completed RHS fits that exact destination;
//! it does not change the field type or authorize truncation. Lowering can
//! consume the decision without constructing a receiver proof.

use semantic_vocabulary::IntegerType;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// Distinguish a place update from the creation of one encoded aggregate.
/// Neither operation invents an assignment expression for the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StoredIntegerWriteSite {
    #[default]
    Missing,
    Assignment {
        destination: ExpressionHandle,
    },
    Construction {
        expression: ExpressionHandle,
        statement_index: usize,
    },
    /// Universal decision for this complete outer invocation: every execution
    /// of this leaf write, including distinct nested invocations, fits the row's
    /// physical width. Published only after the entire call scope is admitted;
    /// this is not the identity of one particular nested execution.
    Call {
        statement_index: usize,
        call_ordinal: usize,
        write: arena::Handle<super::mutation::PerformedIntegerWrite>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStoredIntegerWrite {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub site: StoredIntegerWriteSite,
    pub value: ExpressionHandle,
    pub field: SymbolHandle,
    pub stored_type: IntegerType,
}

impl Default for CheckedStoredIntegerWrite {
    fn default() -> Self {
        // Arena's absent row has no source occurrence; its placeholder width
        // grants no decision without valid machine/state/site handles.
        Self {
            machine: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            site: StoredIntegerWriteSite::Missing,
            value: ExpressionHandle::invalid(),
            field: SymbolHandle::invalid(),
            stored_type: IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .expect("u8 has a valid fixed width"),
        }
    }
}
