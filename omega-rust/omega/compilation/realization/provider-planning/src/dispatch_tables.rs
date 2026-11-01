//! The dispatch table of a table-field binding comes from its leaf's signature.
//!
//! `ForeignBinding::VtableField` and `ForeignBinding::TableFunction` name a
//! function-address field. The record holding that field is the record the
//! leaf's first parameter borrows: the caller passes it explicitly, and the
//! foreign binding plan records that declaration. A provider holds no state,
//! so neither the provider's name nor a data declaration qualifying the leaf
//! supplies the table. Plan derivation, layout realization and package review
//! all read the table here, so they cannot disagree about which record owns
//! the field.

use crate::provider_planning::{entry_state, type_reference_node};
use checked_trees::typed_trees::{
    TypedTrees,
    data::DataDefinition,
    machine::Machine,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use symbols::SymbolHandle;

/// The record a table leaf dispatches through, read from its first parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchTable {
    /// The borrowed record type, with refinements removed.
    pub table_type: TypeReferenceHandle,
    /// The record's data declaration symbol.
    pub declaration: SymbolHandle,
}

impl DispatchTable {
    /// The record's data declaration.
    pub fn definition<'typed>(&self, typed: &'typed TypedTrees) -> &'typed DataDefinition {
        typed
            .data_definition_by_symbol(self.declaration)
            .expect("dispatch table declaration was resolved when the table was read")
    }

    /// The table's declaration path, exported by the rule data declarations
    /// use: a live module names its full path; a source-free tree retains the
    /// complete path in the authored name.
    pub fn path(&self, typed: &TypedTrees) -> String {
        if typed.symbols.symbol_module(self.declaration).is_valid() {
            typed.symbols.display_path(self.declaration, "::")
        } else {
            self.definition(typed).name.as_str().to_owned()
        }
    }
}

/// Read the dispatch table from the leaf's first parameter: a readable borrow
/// of a data declaration. A write-only borrow cannot supply the table the call
/// reads its function address from.
pub fn dispatch_table(typed: &TypedTrees, machine: &Machine) -> Result<DispatchTable, String> {
    let entry = entry_state(typed, machine).ok_or("table leaf has no entry signature")?;
    let first = typed
        .state_parameters(entry)
        .first()
        .ok_or("table leaf requires its dispatch table as its first parameter")?;
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = type_reference_node(typed, unrefined(typed, first.type_reference))
    else {
        return Err("table leaf's first parameter must borrow its dispatch table".into());
    };
    if *access == language_semantics::ReferenceAccess::WriteOnly {
        return Err("table dispatch requires a readable table borrow".into());
    }
    let table_type = unrefined(typed, *referee);
    let declaration = type_reference_node(typed, table_type)
        .as_named_symbol()
        .filter(|symbol| typed.data_definition_by_symbol(*symbol).is_some())
        .ok_or("table leaf's first parameter does not borrow a data declaration")?;
    Ok(DispatchTable {
        table_type,
        declaration,
    })
}

fn unrefined(typed: &TypedTrees, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        type_reference_node(typed, reference)
    {
        reference = *base_type;
    }
    reference
}
