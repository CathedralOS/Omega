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

/// Declaration rows found by symbol, addressed directly once the owner freezes.
///
/// Every use of a declaration re-finds it from its symbol, so this lookup sits
/// under most structural checks; a binary search or hash probe per use showed
/// up as seconds of CPU. Rows are stably sorted by (index, generation), so a
/// stale generation sharing an index never answers for the live symbol, and
/// duplicate declarations keep their authored order for first-match,
/// last-match and uniqueness readers.
///
/// While the program still mutates, an index can be rebuilt after every
/// rewrite, so a lookup binary-searches the sorted rows and a rebuild costs
/// only the sort. [`SymbolRows::densify`] runs once when the owner seals:
/// symbols are dense arena handles, so `starts[i]..starts[i + 1]` then bounds
/// the rows whose symbol has arena index `i`, and a lookup is two array reads.
/// That table is as long as the largest indexed symbol, which is why mutable
/// phases never build it.
#[derive(Clone)]
pub(in crate::typed_trees) struct SymbolRows<Position> {
    symbols: Vec<SymbolHandle>,
    positions: Vec<Position>,
    starts: Vec<u32>,
}

impl<Position> Default for SymbolRows<Position> {
    fn default() -> Self {
        Self {
            symbols: Vec::new(),
            positions: Vec::new(),
            starts: Vec::new(),
        }
    }
}

impl<Position> SymbolRows<Position> {
    pub(in crate::typed_trees) fn new(mut rows: Vec<(SymbolHandle, Position)>) -> Self {
        rows.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
        let (symbols, positions) = rows.into_iter().unzip();
        Self {
            symbols,
            positions,
            starts: Vec::new(),
        }
    }

    /// Build the per-index start table for a sealed owner.
    pub(in crate::typed_trees) fn densify(&mut self) {
        let Some(last) = self.symbols.last() else {
            return;
        };
        let mut starts = vec![0u32; last.arena_index() as usize + 2];
        for symbol in &self.symbols {
            starts[symbol.arena_index() as usize + 1] += 1;
        }
        for index in 1..starts.len() {
            starts[index] += starts[index - 1];
        }
        self.starts = starts;
    }

    /// Every row carrying exactly `symbol`, in authored order.
    pub(in crate::typed_trees) fn get(&self, symbol: SymbolHandle) -> &[Position] {
        let index = symbol.arena_index();
        let (start, end) = if self.starts.is_empty() {
            (
                self.symbols
                    .partition_point(|row| row.arena_index() < index),
                self.symbols
                    .partition_point(|row| row.arena_index() <= index),
            )
        } else {
            let index = index as usize;
            let (Some(&start), Some(&end)) = (self.starts.get(index), self.starts.get(index + 1))
            else {
                return &[];
            };
            (start as usize, end as usize)
        };
        let same_index = &self.symbols[start..end];
        let generation = symbol.generation();
        let low = start + same_index.partition_point(|row| row.generation() < generation);
        let high = start + same_index.partition_point(|row| row.generation() <= generation);
        &self.positions[low..high]
    }

    /// The first authored row carrying exactly `symbol`.
    pub(in crate::typed_trees) fn first(&self, symbol: SymbolHandle) -> Option<&Position> {
        self.get(symbol).first()
    }
}

/// Every input used by the shared state/value containment index. Keep arena
/// revisions and complete visible root spans separate: neither a shorter root
/// roster nor a changed generation can cancel another input's mutation.
pub(super) type StateDeclarationRevision = (
    u64,
    u64,
    u64,
    u64,
    u64,
    u64,
    arena::HandleSpan<crate::machine::Machine>,
    arena::HandleSpan<crate::proposition::PropositionDefinition>,
);

#[derive(Clone, Default)]
pub(super) struct StateDeclarationIndex {
    locals: SymbolRows<(u32, u32, u32)>,
    /// The first binder narrows candidates; the complete parameter span decides.
    states: SymbolRows<(u32, u32)>,
    /// Flat parameter-arena order includes bodyless and proposition formals.
    parameter_types: SymbolRows<crate::types::TypeReferenceHandle>,
    /// Exact old fallback traversal order, retained beside each declared type.
    value_types: SymbolRows<(usize, crate::types::TypeReferenceHandle)>,
}

#[derive(Clone, Default)]
pub(super) struct DataMemberIndex {
    fields: SymbolRows<(u32, u32, u32)>,
    variants: SymbolRows<(u32, u32)>,
    variant_names: Vec<(Box<str>, (u32, u32))>,
}

impl StateDeclarationIndex {
    pub(super) fn densify(&mut self) {
        self.locals.densify();
        self.states.densify();
        self.parameter_types.densify();
        self.value_types.densify();
    }
}

impl DataMemberIndex {
    pub(super) fn densify(&mut self) {
        self.fields.densify();
        self.variants.densify();
    }
}

impl crate::TypedTrees {
    pub(super) fn with_state_declarations<R>(
        &self,
        run: impl FnOnce(&StateDeclarationIndex) -> R,
    ) -> R {
        let revision = (
            self.tables.machines.revision(),
            self.tables.machine_states.revision(),
            self.tables.state_parameters.revision(),
            self.tables.machine_owned_data.revision(),
            self.statement_table.revision(),
            self.tables.propositions.revision(),
            self.roots.machines,
            self.roots.propositions,
        );
        self.declaration_indexes.state_declarations.with(
            revision,
            || {
                let (mut locals, mut states, mut parameter_types, mut value_types) =
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (_, parameter) in self.tables.state_parameters.iter() {
                    parameter_types.push((parameter.symbol, parameter.type_reference));
                }
                for (machine_position, machine) in self.machines().iter().enumerate() {
                    for owned in self.machine_owned_data(machine) {
                        let order = value_types.len();
                        value_types.push((owned.symbol, (order, owned.type_reference)));
                    }
                    for (state_position, state) in self.machine_states(machine).iter().enumerate() {
                        for parameter in self.state_parameters(state) {
                            let order = value_types.len();
                            value_types.push((parameter.symbol, (order, parameter.type_reference)));
                        }
                        let position = (machine_position as u32, state_position as u32);
                        let first = self
                            .state_parameters(state)
                            .first()
                            .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
                        states.push((first, position));
                        for (statement_position, statement) in self
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .enumerate()
                        {
                            if let StatementNode::LocalData(local) = statement {
                                let order = value_types.len();
                                value_types.push((local.symbol, (order, local.type_reference)));
                                locals.push((
                                    local.symbol,
                                    (position.0, position.1, statement_position as u32),
                                ));
                            }
                        }
                    }
                }
                for proposition in self.propositions() {
                    for parameter in self.proposition_parameters(proposition) {
                        let order = value_types.len();
                        value_types.push((parameter.symbol, (order, parameter.type_reference)));
                    }
                }
                StateDeclarationIndex {
                    locals: SymbolRows::new(locals),
                    states: SymbolRows::new(states),
                    parameter_types: SymbolRows::new(parameter_types),
                    value_types: SymbolRows::new(value_types),
                }
            },
            run,
        )
    }

    /// Named executable value typing's declaration fallback, after its builtin,
    /// mathematical/static binder and verified parent-hint queries. A direct
    /// resolved name first observes the complete flat parameter arena, including
    /// bodyless signatures. The remaining candidates preserve machine-owned data,
    /// then each state's parameters/locals, then proposition parameter order.
    /// This reads declared types only; it establishes no value or storage fact.
    pub fn first_named_value_declared_type(
        &self,
        symbol: SymbolHandle,
        head_symbol: SymbolHandle,
    ) -> Option<crate::types::TypeReferenceHandle> {
        self.with_state_declarations(|index| {
            if symbol.is_valid()
                && symbol == head_symbol
                && let Some(type_reference) = index.parameter_types.first(symbol)
            {
                return Some(*type_reference);
            }
            [symbol, head_symbol]
                .into_iter()
                .filter(|symbol| symbol.is_valid())
                .filter_map(|symbol| index.value_types.first(symbol).copied())
                .min_by_key(|(order, _)| *order)
                .map(|(_, type_reference)| type_reference)
        })
    }

    /// Every local declaration carrying the symbol, in authored traversal order.
    pub fn local_data_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32, u32)> {
        self.with_state_declarations(|index| index.locals.get(symbol).to_vec())
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
            index
                .states
                .get(first)
                .iter()
                .copied()
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

    pub(super) fn with_data_members<R>(&self, run: impl FnOnce(&DataMemberIndex) -> R) -> R {
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
                let (mut fields, mut variants) = (Vec::new(), Vec::new());
                let mut variant_names: Vec<(Box<str>, (u32, u32))> = Vec::new();
                for (data_position, definition) in self.data_definitions().iter().enumerate() {
                    for (member_position, member) in
                        self.data_members(definition).iter().enumerate()
                    {
                        let position = (data_position as u32, member_position as u32);
                        match member {
                            DataMember::Field(field) => {
                                fields.push((field.symbol, (position.0, position.1, u32::MAX)))
                            }
                            DataMember::Variant(variant) => {
                                variants.push((variant.symbol, position));
                                variant_names.push((variant.name.as_str().into(), position));
                                for (payload_position, field) in
                                    self.data_payload_fields(variant).iter().enumerate()
                                {
                                    fields.push((
                                        field.symbol,
                                        (position.0, position.1, payload_position as u32),
                                    ));
                                }
                            }
                        }
                    }
                }
                variant_names.sort_by(|left, right| left.0.cmp(&right.0));
                DataMemberIndex {
                    fields: SymbolRows::new(fields),
                    variants: SymbolRows::new(variants),
                    variant_names,
                }
            },
            run,
        )
    }

    /// Every field, including case payloads; `u32::MAX` denotes a direct member.
    pub fn data_field_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32, u32)> {
        self.with_data_members(|index| index.fields.get(symbol).to_vec())
    }

    /// First case declaration carrying this full symbol identity.
    pub fn data_variant_position_for_symbol(&self, symbol: SymbolHandle) -> Option<(u32, u32)> {
        self.with_data_members(|index| index.variants.first(symbol).copied())
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
    use super::SymbolRows;
    use symbols::SymbolHandle;

    // Generations sharing one arena index and duplicate declarations must each
    // answer exactly, in authored order, including indexes past the table end,
    // through both the sorted (mutable) and the densified (sealed) lookups.
    #[test]
    fn symbol_rows_preserve_generations_and_duplicate_roster_order() {
        let positions = [0, 1, 2, 7, 8];
        let generations = [0, 1, 2, u32::MAX / 2, u32::MAX / 2 + 1, u32::MAX];
        let symbols = positions
            .into_iter()
            .flat_map(|position| {
                generations
                    .into_iter()
                    .map(move |generation| SymbolHandle::from_parts(position, generation))
            })
            .collect::<Vec<_>>();
        let authored = symbols
            .iter()
            .rev()
            .chain(symbols.iter())
            .copied()
            .enumerate()
            .map(|(position, symbol)| (symbol, position))
            .collect::<Vec<_>>();
        let sorted = SymbolRows::new(authored.clone());
        let mut dense = sorted.clone();
        dense.densify();
        for symbol in symbols.into_iter().chain([
            SymbolHandle::from_parts(3, 4),
            SymbolHandle::from_parts(1, 3),
            SymbolHandle::from_parts(9, 0),
            SymbolHandle::from_parts(u32::MAX, 0),
        ]) {
            let expected = authored
                .iter()
                .filter(|(candidate, _)| *candidate == symbol)
                .map(|(_, position)| *position)
                .collect::<Vec<_>>();
            for rows in [&sorted, &dense] {
                assert_eq!(rows.get(symbol), expected);
                assert_eq!(rows.first(symbol), expected.first());
            }
        }
        let mut empty = SymbolRows::<usize>::new(Vec::new());
        empty.densify();
        assert!(empty.get(SymbolHandle::invalid()).is_empty());
        assert!(empty.first(SymbolHandle::invalid()).is_none());
    }
}
