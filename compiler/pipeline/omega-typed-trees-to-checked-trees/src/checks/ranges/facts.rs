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

#[derive(Clone)]
pub(super) struct RangeFacts<'field> {
    fields: &'field [(SymbolHandle, String, usize)],
    integer_fields: Vec<(SymbolHandle, String, i64)>,
    locals: Vec<(SymbolHandle, String, usize)>,
    integer_locals: Vec<(SymbolHandle, String, i64)>,
    proven_indexes: Vec<(String, String)>,
    proven_orderings: Vec<(String, String)>,
    proven_range_bounds: Vec<(String, String)>,
    minimum_lengths: Vec<(String, i64)>,
}

impl<'field> RangeFacts<'field> {
    pub(super) fn new(fields: &'field [(SymbolHandle, String, usize)]) -> Self {
        Self {
            fields,
            integer_fields: Vec::new(),
            locals: Vec::new(),
            integer_locals: Vec::new(),
            proven_indexes: Vec::new(),
            proven_orderings: Vec::new(),
            proven_range_bounds: Vec::new(),
            minimum_lengths: Vec::new(),
        }
    }

    pub(super) fn field_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if symbol.is_valid() {
            if let Some(length) = self
                .fields
                .iter()
                .find_map(|(field, _, length)| (*field == symbol).then_some(*length))
            {
                return Some(length);
            }
        }

        self.fields.iter().find_map(|(_, field_name, length)| {
            name.is_some_and(|name| name == field_name)
                .then_some(*length)
        })
    }

    pub(super) fn local_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if symbol.is_valid() {
            if let Some(length) = self
                .locals
                .iter()
                .rev()
                .find_map(|(local, _, length)| (*local == symbol).then_some(*length))
            {
                return Some(length);
            }
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
        if symbol.is_valid() {
            if let Some(value) = self
                .integer_locals
                .iter()
                .rev()
                .find_map(|(local, _, value)| (*local == symbol).then_some(*value))
            {
                return Some(value);
            }
        }

        self.integer_locals
            .iter()
            .rev()
            .find_map(|(_, local_name, value)| {
                name.is_some_and(|name| name == local_name)
                    .then_some(*value)
            })
    }

    pub(super) fn field_integer(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<i64> {
        if symbol.is_valid() {
            if let Some(value) = self
                .integer_fields
                .iter()
                .rev()
                .find_map(|(field, _, value)| (*field == symbol).then_some(*value))
            {
                return Some(value);
            }
        }

        self.integer_fields
            .iter()
            .rev()
            .find_map(|(_, field_name, value)| {
                name.is_some_and(|name| name == field_name)
                    .then_some(*value)
            })
    }

    pub(super) fn define_local(
        &mut self,
        symbol: SymbolHandle,
        name: impl Into<String>,
        length: Option<usize>,
        integer: Option<i64>,
    ) {
        let name = name.into();
        if let Some(length) = length {
            self.locals.push((symbol, name.clone(), length));
        }
        if let Some(value) = integer {
            self.integer_locals.push((symbol, name, value));
        }
    }

    pub(super) fn assign_local(
        &mut self,
        symbol: SymbolHandle,
        name: Option<&str>,
        length: Option<usize>,
        integer: Option<i64>,
    ) {
        self.forget_local(symbol, name);
        self.define_local(symbol, name.unwrap_or_default().to_owned(), length, integer);
    }

    pub(super) fn define_field_integer(
        &mut self,
        symbol: SymbolHandle,
        name: impl Into<String>,
        integer: i64,
    ) {
        self.integer_fields.push((symbol, name.into(), integer));
    }

    pub(super) fn assign_field_integer(
        &mut self,
        symbol: SymbolHandle,
        name: Option<&str>,
        integer: Option<i64>,
    ) {
        self.forget_field_integer(symbol, name);
        if let Some(integer) = integer {
            self.define_field_integer(symbol, name.unwrap_or_default().to_owned(), integer);
        }
    }

    pub(super) fn forget_field_integers(&mut self) {
        self.integer_fields.clear();
    }

    pub(super) fn alias_collection(&mut self, original: &str, alias: &str) {
        if original == alias {
            return;
        }

        for (_, index) in self
            .proven_indexes
            .clone()
            .into_iter()
            .filter(|(collection, _)| collection == original)
        {
            self.prove_index(alias.to_owned(), index);
        }
        for (_, bound) in self
            .proven_range_bounds
            .clone()
            .into_iter()
            .filter(|(collection, _)| collection == original)
        {
            self.prove_range_bound(alias.to_owned(), bound);
        }
        if let Some(minimum_length) = self.minimum_length(original) {
            self.prove_minimum_length(alias.to_owned(), minimum_length);
        }
    }

    pub(super) fn alias_index(&mut self, original: &str, alias: &str) {
        if original == alias {
            return;
        }

        for (collection, _) in self
            .proven_indexes
            .clone()
            .into_iter()
            .filter(|(_, index)| index == original)
        {
            self.prove_index(collection, alias.to_owned());
        }
        for (collection, _) in self
            .proven_range_bounds
            .clone()
            .into_iter()
            .filter(|(_, bound)| bound == original)
        {
            self.prove_range_bound(collection, alias.to_owned());
        }
        for (lower, upper) in self
            .proven_orderings
            .clone()
            .into_iter()
            .filter(|(lower, upper)| lower == original || upper == original)
        {
            let lower = if lower == original {
                alias.to_owned()
            } else {
                lower
            };
            let upper = if upper == original {
                alias.to_owned()
            } else {
                upper
            };
            self.prove_at_most(lower, upper);
        }
    }

    pub(super) fn prove_index(&mut self, collection: String, index: String) {
        if !self
            .proven_indexes
            .iter()
            .any(|(known_collection, known_index)| {
                known_collection == &collection && known_index == &index
            })
        {
            self.proven_indexes.push((collection, index));
        }
    }

    pub(super) fn index_is_proven(&self, collection: &str, index: &str) -> bool {
        self.proven_indexes
            .iter()
            .any(|(known_collection, known_index)| {
                known_collection == collection && known_index == index
            })
    }

    pub(super) fn index_value_is_proven(&self, collection: &str, index: i64) -> bool {
        index >= 0
            && self
                .minimum_length(collection)
                .is_some_and(|length| index < length)
    }

    pub(super) fn prove_at_most(&mut self, lower: String, upper: String) {
        if !self
            .proven_orderings
            .iter()
            .any(|(known_lower, known_upper)| known_lower == &lower && known_upper == &upper)
        {
            self.proven_orderings.push((lower, upper));
        }
    }

    pub(super) fn at_most_is_proven(&self, lower: &str, upper: &str) -> bool {
        self.proven_orderings
            .iter()
            .any(|(known_lower, known_upper)| known_lower == lower && known_upper == upper)
    }

    pub(super) fn prove_range_bound(&mut self, collection: String, bound: String) {
        if !self
            .proven_range_bounds
            .iter()
            .any(|(known_collection, known_bound)| {
                known_collection == &collection && known_bound == &bound
            })
        {
            self.proven_range_bounds.push((collection, bound));
        }
    }

    pub(super) fn range_bound_is_proven(&self, collection: &str, bound: &str) -> bool {
        self.proven_range_bounds
            .iter()
            .any(|(known_collection, known_bound)| {
                known_collection == collection && known_bound == bound
            })
    }

    pub(super) fn range_bound_value_is_proven(&self, collection: &str, bound: i64) -> bool {
        bound >= 0
            && self
                .minimum_length(collection)
                .is_some_and(|length| bound <= length)
    }

    pub(super) fn prove_minimum_length(&mut self, collection: String, minimum_length: i64) {
        if minimum_length <= 0 {
            return;
        }

        if let Some((_, known_minimum)) = self
            .minimum_lengths
            .iter_mut()
            .find(|(known_collection, _)| known_collection == &collection)
        {
            *known_minimum = (*known_minimum).max(minimum_length);
            return;
        }

        self.minimum_lengths.push((collection, minimum_length));
    }

    fn forget_local(&mut self, symbol: SymbolHandle, name: Option<&str>) {
        self.locals
            .retain(|(local, local_name, _)| !local_matches(*local, local_name, symbol, name));
        self.integer_locals
            .retain(|(local, local_name, _)| !local_matches(*local, local_name, symbol, name));
    }

    fn forget_field_integer(&mut self, symbol: SymbolHandle, name: Option<&str>) {
        self.integer_fields
            .retain(|(field, field_name, _)| !local_matches(*field, field_name, symbol, name));
    }

    fn minimum_length(&self, collection: &str) -> Option<i64> {
        self.minimum_lengths
            .iter()
            .find_map(|(known_collection, minimum_length)| {
                (known_collection == collection).then_some(*minimum_length)
            })
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
