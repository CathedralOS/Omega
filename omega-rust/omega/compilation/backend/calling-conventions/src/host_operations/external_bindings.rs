//! External binding rows: how one host operation binds to a foreign symbol.

use crate::plans::BoundaryEntryPlan;
use target::NormalizedForeignLocator;

/// One selected bodyless external leaf retained for target realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBindingRow {
    /// The leaf's target identifier, retained for exact target realization.
    pub target_name: String,
    pub trait_name: String,
    pub method: String,
    /// Exact canonical overload identity. The human method name remains only
    /// readable drift data, including for singleton requirements.
    pub requirement_identity: String,
    /// The attached provider data type that owns the table layout. Empty for
    /// free leaves and required for table-field bindings.
    pub table_type: String,
    /// Canonical source-selected plan for this concrete service method.
    pub boundary_entry_plan: Option<BoundaryEntryPlan>,
    pub binding: ExternalBindingKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingKind {
    Syscall {
        number: i64,
    },
    /// One evaluated, target-normalized foreign locator. Raw foreign bytes are
    /// data; a string pair authored in source never reaches this row.
    Import {
        locator: NormalizedForeignLocator,
    },
    /// Select an existing compiler-known platform lowering. The retained
    /// string is the exact normalized realization-machine overload identity.
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    /// Dispatch by a fn-ptr field of the row's `table_type`; the layout plan
    /// supplies its byte offset.
    VtableField {
        field: String,
    },
    /// Dispatch by fn-ptr FIELD like `VtableField`, but the table pointer is
    /// DISPATCH-ONLY -- never a wire argument (EFI table services take no
    /// This; protocol/COM methods do).
    TableFunction {
        field: String,
    },
}
