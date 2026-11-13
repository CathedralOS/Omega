// Dispatch table extraction for table-field foreign bindings
// (`ForeignBinding::VtableField` and `ForeignBinding::TableFunction`). The
// record holding the function-address field is borrowed by the leaf's first
// parameter, which must be a readable reference to a data declaration. Freezing
// `TableFieldStorage` while the selected leaf still owns Source keeps plan
// derivation, layout realization, and package review aligned on the exact
// record application and `addr` field declaration.

use crate::provider_planning::{entry_state, type_reference_node};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::DataDefinition,
    machine::Machine,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchTable {
    table_type: TypeReferenceHandle,
    declaration: SymbolHandle,
}

impl DispatchTable {
    pub fn table_type(&self) -> TypeReferenceHandle {
        self.table_type
    }

    pub fn declaration(&self) -> SymbolHandle {
        self.declaration
    }

    pub fn definition<'typed>(&self, typed: &'typed TypedTrees) -> &'typed DataDefinition {
        typed
            .data_definition_by_symbol(self.declaration)
            .expect("dispatch table declaration was resolved when the table was read")
    }

    pub fn path(&self, typed: &TypedTrees) -> String {
        if typed.symbols.symbol_module(self.declaration).is_valid() {
            typed.symbols.display_path(self.declaration, "::")
        } else {
            self.definition(typed).name.as_str().to_owned()
        }
    }
}

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

pub fn field_storage(
    typed: &TypedTrees,
    machine: &Machine,
    field_name: &str,
) -> Result<effects::provider_plan::TableFieldStorage, String> {
    use typed_trees::data::DataMember;
    use typed_trees::types::PrimitiveType;
    let table = dispatch_table(typed, machine)?;
    let field = typed
        .data_members(table.definition(typed))
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field)
                if !field.relevance.is_erased() && field.name.as_str() == field_name =>
            {
                Some(field)
            }
            _ => None,
        })
        .ok_or_else(|| format!("no runtime field `{field_name}` in the dispatch table"))?;
    if typed.primitive_type_reference(field.type_reference) != Some(PrimitiveType::Addr) {
        return Err("table dispatch field must have type addr".into());
    }
    let application = typed.storage_type_identity(
        typed_trees::type_identity::TypeIdentityRequest::ordinary(table.table_type()),
    );
    Ok(effects::provider_plan::TableFieldStorage {
        table_application: typed
            .type_identity_text(application)
            .ok_or("dispatch table lost its exact storage application")?,
        field_declaration: typed.normalized_hermetic_symbol_identity(field.symbol)?,
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
