use omega_core::symbols::{SymbolHandle, SymbolTable};

pub(super) struct MachineScope<'program> {
    pub(super) symbol: SymbolHandle,
    pub(super) attached_data: Option<&'program omega_symbol_resolved_trees::name::DiagnosticName>,
    pub(super) contains: &'program [omega_symbol_resolved_trees::machine::ContainedObject],
    pub(super) inherited_data_members:
        Option<&'program [omega_symbol_resolved_trees::data::DataMember]>,
    pub(super) owned_data: &'program [omega_symbol_resolved_trees::machine::OwnedData],
}

impl MachineScope<'_> {
    pub(super) fn field_type_reference(
        &self,
        symbols: &SymbolTable,
        field_symbol: SymbolHandle,
    ) -> Option<&omega_symbol_resolved_trees::types::TypeReference> {
        if let Some(data_members) = self.inherited_data_members {
            for member in data_members {
                let omega_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                    continue;
                };
                if field.symbol == field_symbol
                    || (field_symbol.is_valid()
                        && field.name.as_str() == symbols.name(field_symbol))
                {
                    return Some(&field.type_reference);
                }
            }
        }

        self.owned_data
            .iter()
            .find(|owned_data| owned_data.symbol == field_symbol)
            .map(|owned_data| &owned_data.type_reference)
    }
}
