//! Authored containment relationships, indexed beside their typed arenas.
//!
//! Structural readers previously copied machine/data indexes into a persistent
//! thread-local roster and hashed parameter lists to rediscover their owner.
//! Machines and data definitions use their existing indexes. These rows cover
//! only the remaining state/local and member/payload containment relationships.
//! Stable full-handle ordering preserves duplicate declarations and their source
//! order. Parameter ownership is selected by exact symbols, never a fingerprint.
//! Arena revisions and the visible root span invalidate derived rows while
//! preparation mutates; frozen checking shares the same immutable declarations.

use super::roster_marker;
use crate::{data::DataMember, signature::StateParameter, statement::StatementNode};
use symbols::SymbolHandle;

fn symbol_key(symbol: SymbolHandle) -> u64 {
    // Packing preserves lexicographic (index, generation) order while each
    // binary-search step compares one integer. This is an ordering key, not
    // a fingerprint: neither half of the generational identity is discarded.
    (u64::from(symbol.arena_index()) << 32) | u64::from(symbol.generation())
}

pub(super) fn sort_symbol_rows<Position>(rows: &mut [(SymbolHandle, Position)]) {
    rows.sort_by_key(|(symbol, _)| symbol_key(*symbol));
}

/// First-match readers need only the lower bound. Finding the entire duplicate
/// range made every machine lookup search twice during structural checking.
/// Keep the full-range query below for readers that must detect ambiguity.
pub(super) fn first_symbol_row<Position>(
    rows: &[(SymbolHandle, Position)],
    symbol: SymbolHandle,
) -> Option<&(SymbolHandle, Position)> {
    let key = symbol_key(symbol);
    let start = rows.partition_point(|(symbol, _)| symbol_key(*symbol) < key);
    rows.get(start)
        .filter(|(candidate, _)| *candidate == symbol)
}

pub(super) fn symbol_rows<Position>(
    rows: &[(SymbolHandle, Position)],
    symbol: SymbolHandle,
) -> &[(SymbolHandle, Position)] {
    let key = symbol_key(symbol);
    let start = rows.partition_point(|(symbol, _)| symbol_key(*symbol) < key);
    let end = rows.partition_point(|(symbol, _)| symbol_key(*symbol) <= key);
    &rows[start..end]
}

#[derive(Clone, Default)]
pub(super) struct StateDeclarationIndex {
    locals: Vec<(SymbolHandle, (u32, u32, u32))>,
    /// The first binder narrows candidates; the complete parameter span decides.
    states: Vec<(SymbolHandle, (u32, u32))>,
}

#[derive(Clone, Default)]
pub(super) struct DataMemberIndex {
    fields: Vec<(SymbolHandle, (u32, u32, u32))>,
    variants: Vec<(SymbolHandle, (u32, u32))>,
    variant_names: Vec<(Box<str>, (u32, u32))>,
}

impl crate::TypedTrees {
    fn with_state_declarations<R>(&self, run: impl FnOnce(&StateDeclarationIndex) -> R) -> R {
        let revision = self
            .tables
            .machines
            .revision()
            .wrapping_add(self.tables.machine_states.revision())
            .wrapping_add(self.tables.state_parameters.revision())
            .wrapping_add(self.statement_table.revision())
            .wrapping_add(roster_marker(self.roots.machines));
        self.declaration_indexes.state_declarations.with(
            revision,
            || {
                let mut index = StateDeclarationIndex::default();
                for (machine_position, machine) in self.machines().iter().enumerate() {
                    for (state_position, state) in self.machine_states(machine).iter().enumerate() {
                        let position = (machine_position as u32, state_position as u32);
                        let first = self
                            .state_parameters(state)
                            .first()
                            .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
                        index.states.push((first, position));
                        for (statement_position, statement) in self
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .enumerate()
                        {
                            if let StatementNode::LocalData(local) = statement {
                                index.locals.push((
                                    local.symbol,
                                    (position.0, position.1, statement_position as u32),
                                ));
                            }
                        }
                    }
                }
                sort_symbol_rows(&mut index.locals);
                sort_symbol_rows(&mut index.states);
                index
            },
            run,
        )
    }

    /// Every local declaration carrying the symbol, in authored traversal order.
    pub fn local_data_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32, u32)> {
        self.with_state_declarations(|index| {
            symbol_rows(&index.locals, symbol)
                .iter()
                .map(|(_, position)| *position)
                .collect()
        })
    }

    /// The first state with this exact binder sequence. Empty spans and forged
    /// duplicate symbols retain the same first-match rule as a source roster.
    pub fn state_position_for_parameter_symbols(
        &self,
        parameters: &[StateParameter],
    ) -> Option<(u32, u32)> {
        let first = parameters
            .first()
            .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
        self.with_state_declarations(|index| {
            symbol_rows(&index.states, first)
                .iter()
                .map(|(_, position)| *position)
                .find(|&(machine_position, state_position)| {
                    let machine = &self.machines()[machine_position as usize];
                    let state = &self.machine_states(machine)[state_position as usize];
                    let authored = self.state_parameters(state);
                    authored.len() == parameters.len()
                        && authored
                            .iter()
                            .zip(parameters)
                            .all(|(left, right)| left.symbol == right.symbol)
                })
        })
    }

    fn with_data_members<R>(&self, run: impl FnOnce(&DataMemberIndex) -> R) -> R {
        let revision = self
            .tables
            .data_definitions
            .revision()
            .wrapping_add(self.tables.data_members.revision())
            .wrapping_add(self.tables.data_payload_fields.revision())
            .wrapping_add(roster_marker(self.roots.data_definitions));
        self.declaration_indexes.data_members.with(
            revision,
            || {
                let mut index = DataMemberIndex::default();
                for (data_position, definition) in self.data_definitions().iter().enumerate() {
                    for (member_position, member) in
                        self.data_members(definition).iter().enumerate()
                    {
                        let position = (data_position as u32, member_position as u32);
                        match member {
                            DataMember::Field(field) => index
                                .fields
                                .push((field.symbol, (position.0, position.1, u32::MAX))),
                            DataMember::Variant(variant) => {
                                index.variants.push((variant.symbol, position));
                                index
                                    .variant_names
                                    .push((variant.name.as_str().into(), position));
                                for (payload_position, field) in
                                    self.data_payload_fields(variant).iter().enumerate()
                                {
                                    index.fields.push((
                                        field.symbol,
                                        (position.0, position.1, payload_position as u32),
                                    ));
                                }
                            }
                        }
                    }
                }
                sort_symbol_rows(&mut index.fields);
                sort_symbol_rows(&mut index.variants);
                index
                    .variant_names
                    .sort_by(|left, right| left.0.cmp(&right.0));
                index
            },
            run,
        )
    }

    /// Every field, including case payloads; `u32::MAX` denotes a direct member.
    pub fn data_field_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32, u32)> {
        self.with_data_members(|index| {
            symbol_rows(&index.fields, symbol)
                .iter()
                .map(|(_, position)| *position)
                .collect()
        })
    }

    /// First case declaration carrying this full symbol identity.
    pub fn data_variant_position_for_symbol(&self, symbol: SymbolHandle) -> Option<(u32, u32)> {
        self.with_data_members(|index| {
            first_symbol_row(&index.variants, symbol).map(|(_, position)| *position)
        })
    }

    /// Spelling narrows case candidates only; the caller must also match the
    /// resolved payload field identity before accepting a source member path.
    pub fn data_variant_positions_for_name(&self, name: &str) -> Vec<(u32, u32)> {
        self.with_data_members(|index| {
            let start = index
                .variant_names
                .partition_point(|(candidate, _)| candidate.as_ref() < name);
            let end = index
                .variant_names
                .partition_point(|(candidate, _)| candidate.as_ref() <= name);
            index.variant_names[start..end]
                .iter()
                .map(|(_, position)| *position)
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{first_symbol_row, sort_symbol_rows, symbol_key, symbol_rows};
    use symbols::SymbolHandle;

    #[test]
    fn symbol_order_preserves_generations_and_duplicate_roster_order() {
        let boundaries = [0, 1, 2, u32::MAX / 2, u32::MAX / 2 + 1, u32::MAX];
        let symbols = boundaries
            .into_iter()
            .flat_map(|position| {
                boundaries
                    .into_iter()
                    .map(move |generation| SymbolHandle::from_parts(position, generation))
            })
            .collect::<Vec<_>>();
        for left in &symbols {
            for right in &symbols {
                assert_eq!(
                    symbol_key(*left).cmp(&symbol_key(*right)),
                    (left.arena_index(), left.generation())
                        .cmp(&(right.arena_index(), right.generation()))
                );
            }
        }

        let mut rows = symbols
            .iter()
            .rev()
            .chain(symbols.iter())
            .copied()
            .enumerate()
            .map(|(position, symbol)| (symbol, position))
            .collect::<Vec<_>>();
        let authored = rows.clone();
        sort_symbol_rows(&mut rows);
        for symbol in symbols.into_iter().chain([
            SymbolHandle::from_parts(3, 4),
            SymbolHandle::from_parts(1, 3),
        ]) {
            let expected = authored
                .iter()
                .copied()
                .filter(|(candidate, _)| *candidate == symbol)
                .collect::<Vec<_>>();
            assert_eq!(symbol_rows(&rows, symbol), expected);
            assert_eq!(first_symbol_row(&rows, symbol), expected.first());
        }
        let empty: [(SymbolHandle, usize); 0] = [];
        assert!(symbol_rows(&empty, SymbolHandle::invalid()).is_empty());
        assert!(first_symbol_row(&empty, SymbolHandle::invalid()).is_none());
    }
}
