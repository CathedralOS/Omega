use omega_core::symbols::SymbolHandle;

use super::RangeFacts;

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn field_length(
        &self,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<usize> {
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

    pub(in crate::checks::ranges) fn local_length(
        &self,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<usize> {
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

    pub(in crate::checks::ranges) fn local_integer(
        &self,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<i64> {
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

    pub(in crate::checks::ranges) fn field_integer(
        &self,
        symbol: SymbolHandle,
        name: Option<&str>,
    ) -> Option<i64> {
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

    pub(in crate::checks::ranges) fn define_local(
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

    pub(in crate::checks::ranges) fn assign_local(
        &mut self,
        symbol: SymbolHandle,
        name: Option<&str>,
        length: Option<usize>,
        integer: Option<i64>,
    ) {
        self.forget_local(symbol, name);
        self.define_local(symbol, name.unwrap_or_default().to_owned(), length, integer);
    }

    pub(in crate::checks::ranges) fn define_field_integer(
        &mut self,
        symbol: SymbolHandle,
        name: impl Into<String>,
        integer: i64,
    ) {
        self.integer_fields.push((symbol, name.into(), integer));
    }

    pub(in crate::checks::ranges) fn assign_field_integer(
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

    pub(in crate::checks::ranges) fn forget_field_integers(&mut self) {
        self.integer_fields.clear();
    }

    fn forget_local(&mut self, symbol: SymbolHandle, name: Option<&str>) {
        self.locals
            .retain(|(local, local_name, _)| !value_fact_matches(*local, local_name, symbol, name));
        self.integer_locals
            .retain(|(local, local_name, _)| !value_fact_matches(*local, local_name, symbol, name));
    }

    fn forget_field_integer(&mut self, symbol: SymbolHandle, name: Option<&str>) {
        self.integer_fields
            .retain(|(field, field_name, _)| !value_fact_matches(*field, field_name, symbol, name));
    }
}

fn value_fact_matches(
    candidate_symbol: SymbolHandle,
    candidate_name: &str,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> bool {
    candidate_symbol == symbol || name.is_some_and(|name| name == candidate_name)
}
