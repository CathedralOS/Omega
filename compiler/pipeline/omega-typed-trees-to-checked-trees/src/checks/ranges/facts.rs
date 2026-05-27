use omega_core::symbols::SymbolHandle;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn fixed_array_field_lengths(
    program: &omega_typed_trees::TypedTrees,
) -> Vec<(SymbolHandle, String, usize)> {
    let mut fields = Vec::new();
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let Some(length) = fixed_array_type_length(program, field.type_reference) else {
                continue;
            };
            fields.push((field.symbol, field.name.to_string(), length));
        }
    }
    fields
}

pub(super) fn fixed_array_type_length(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray { length, .. } => Some(*length),
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => fixed_array_type_length(program, *referee),
        TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

pub(super) struct SliceLengthFacts<'field> {
    fields: &'field [(SymbolHandle, String, usize)],
    pub(super) locals: Vec<(SymbolHandle, String, usize)>,
    pub(super) integer_locals: Vec<(SymbolHandle, String, i64)>,
}

impl<'field> SliceLengthFacts<'field> {
    pub(super) fn new(fields: &'field [(SymbolHandle, String, usize)]) -> Self {
        Self {
            fields,
            locals: Vec::new(),
            integer_locals: Vec::new(),
        }
    }

    pub(super) fn field_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if let Some(length) = self
            .fields
            .iter()
            .find_map(|(field, _, length)| (*field == symbol).then_some(*length))
        {
            return Some(length);
        }

        self.fields.iter().find_map(|(_, field_name, length)| {
            name.is_some_and(|name| name == field_name)
                .then_some(*length)
        })
    }

    pub(super) fn local_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if let Some(length) = self
            .locals
            .iter()
            .rev()
            .find_map(|(local, _, length)| (*local == symbol).then_some(*length))
        {
            return Some(length);
        }

        self.locals
            .iter()
            .rev()
            .find_map(|(_, local_name, length)| {
                name.is_some_and(|name| name == local_name)
                    .then_some(*length)
            })
    }

    pub(super) fn local_integer(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<i64> {
        if let Some(value) = self
            .integer_locals
            .iter()
            .rev()
            .find_map(|(local, _, value)| (*local == symbol).then_some(*value))
        {
            return Some(value);
        }

        self.integer_locals
            .iter()
            .rev()
            .find_map(|(_, local_name, value)| {
                name.is_some_and(|name| name == local_name)
                    .then_some(*value)
            })
    }

    pub(super) fn forget_local(&mut self, symbol: SymbolHandle, name: Option<&str>) {
        self.locals
            .retain(|(local, local_name, _)| !local_matches(*local, local_name, symbol, name));
        self.integer_locals
            .retain(|(local, local_name, _)| !local_matches(*local, local_name, symbol, name));
    }
}

fn local_matches(
    candidate_symbol: SymbolHandle,
    candidate_name: &str,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> bool {
    candidate_symbol == symbol || name.is_some_and(|name| name == candidate_name)
}
