//! Shared declaration indexes and lookup answers for one checked program.
//! Frozen-program indexes take precedence over the enclosing build-scope memo.

use super::{DATA_DEF_LOOKUP_SLOT, DATA_DEF_POSITIONS_SLOT, TYPE_PARAMETER_MULTIPLICITY_SLOT};
use std::collections::HashMap;

/// The declaration-table answer for a symbol: absent, exactly one
/// definition at a position, or a duplicate (first position retained —
/// `find`-style consumers still read it).
#[derive(Clone, Copy)]
pub(crate) enum DataDefinitionLookup {
    Missing,
    Unique(u32),
    Duplicate(u32),
}

impl DataDefinitionLookup {
    pub(crate) fn first_position(self) -> Option<u32> {
        match self {
            Self::Unique(position) | Self::Duplicate(position) => Some(position),
            Self::Missing => None,
        }
    }
}

/// Every data-definition symbol's lookup answer, from one pass over the
/// declaration table.
pub(crate) struct DataDefinitionIndex(
    symbols::SymbolKeyMap<symbols::SymbolHandle, DataDefinitionLookup>,
);

impl DataDefinitionIndex {
    fn of(program: &typed_trees::TypedTrees) -> Self {
        let mut lookups = symbols::SymbolKeyMap::default();
        for (position, definition) in program.data_definitions().iter().enumerate() {
            let position = u32::try_from(position).expect("data definition position overflow");
            lookups
                .entry(definition.symbol)
                .and_modify(|lookup| {
                    if let DataDefinitionLookup::Unique(first) = *lookup {
                        *lookup = DataDefinitionLookup::Duplicate(first);
                    }
                })
                .or_insert(DataDefinitionLookup::Unique(position));
        }
        Self(lookups)
    }

    fn lookup(&self, symbol: symbols::SymbolHandle) -> DataDefinitionLookup {
        self.0
            .get(&symbol)
            .copied()
            .unwrap_or(DataDefinitionLookup::Missing)
    }
}

/// The data-definition table's answer for a symbol, memoized per
/// (program, symbol) inside the scope — the `.find`/`.filter` over the
/// declaration slice otherwise re-scans the whole table at every
/// classification site, and the uniqueness answer some callers require is
/// recorded with the same walk.
pub(crate) fn memoized_data_definition_lookup(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> DataDefinitionLookup {
    // A frozen check program answers every symbol from one table pass.
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        return memos
            .data_definitions
            .get_or_init(|| std::sync::Arc::new(DataDefinitionIndex::of(program)))
            .lookup(symbol);
    }
    enum SlotState {
        NoScope,
        Hit(DataDefinitionLookup),
        Miss,
        ForeignProgram,
    }
    let state = DATA_DEF_LOOKUP_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    map.get(&symbol)
                        .copied()
                        .map_or(SlotState::Miss, SlotState::Hit)
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(lookup) = state {
        return lookup;
    }
    let mut matches = program
        .data_definitions()
        .iter()
        .enumerate()
        .filter(|(_, definition)| definition.symbol == symbol)
        .map(|(index, _)| u32::try_from(index).expect("data definition position overflow"));
    let lookup = match (matches.next(), matches.next()) {
        (None, _) => DataDefinitionLookup::Missing,
        (Some(first), None) => DataDefinitionLookup::Unique(first),
        (Some(first), Some(_)) => DataDefinitionLookup::Duplicate(first),
    };
    if matches!(state, SlotState::Miss) {
        DATA_DEF_LOOKUP_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                match scope {
                    Some((owner, map)) if std::ptr::eq(*owner, program) => {
                        map.insert(symbol, lookup);
                    }
                    slot_none @ None => {
                        *slot_none = Some((program, HashMap::from([(symbol, lookup)])));
                    }
                    Some(_) => {}
                }
            }
        });
    }
    lookup
}

/// Where the first data field carrying a symbol sits: a record field of a
/// definition, or a payload field of one of its cases.
#[derive(Clone, Copy)]
enum DataFieldPosition {
    Member {
        definition: u32,
        member: u32,
    },
    Payload {
        definition: u32,
        member: u32,
        field: u32,
    },
}

/// Every data field symbol's first position, in the definition, member and
/// payload order a whole-table `find_map` visits them.
pub(crate) struct DataFieldIndex(symbols::SymbolKeyMap<symbols::SymbolHandle, DataFieldPosition>);

impl DataFieldIndex {
    pub(crate) fn of(program: &typed_trees::TypedTrees) -> Self {
        let position = |index: usize| u32::try_from(index).expect("data field position overflow");
        let mut fields = symbols::SymbolKeyMap::default();
        for (definition, data) in program.data_definitions().iter().enumerate() {
            for (member, entry) in program.data_members(data).iter().enumerate() {
                match entry {
                    typed_trees::data::DataMember::Field(field) => {
                        fields
                            .entry(field.symbol)
                            .or_insert(DataFieldPosition::Member {
                                definition: position(definition),
                                member: position(member),
                            });
                    }
                    typed_trees::data::DataMember::Variant(variant) => {
                        for (field, payload) in
                            program.data_payload_fields(variant).iter().enumerate()
                        {
                            fields
                                .entry(payload.symbol)
                                .or_insert(DataFieldPosition::Payload {
                                    definition: position(definition),
                                    member: position(member),
                                    field: position(field),
                                });
                        }
                    }
                }
            }
        }
        Self(fields)
    }

    pub(crate) fn field<'program>(
        &self,
        program: &'program typed_trees::TypedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'program typed_trees::data::DataField> {
        let (definition, member, payload) = match *self.0.get(&symbol)? {
            DataFieldPosition::Member { definition, member } => (definition, member, None),
            DataFieldPosition::Payload {
                definition,
                member,
                field,
            } => (definition, member, Some(field)),
        };
        let data = program.data_definitions().get(definition as usize)?;
        match (program.data_members(data).get(member as usize)?, payload) {
            (typed_trees::data::DataMember::Field(field), None) => Some(field),
            (typed_trees::data::DataMember::Variant(variant), Some(field)) => {
                program.data_payload_fields(variant).get(field as usize)
            }
            _ => None,
        }
    }
}

/// A data definition by symbol, resolved through the build-scope memo when
/// one is open — the declaration slice is otherwise re-scanned per query.
/// `find`-style callers read the first match; callers that require uniqueness
/// should use [`memoized_data_definition_lookup`] directly.
pub(crate) fn data_definition_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    memoized_data_definition_lookup(program, symbol)
        .first_position()
        .and_then(|position| program.data_definitions().get(position as usize))
}

/// The symbol -> data-definition position table, built once per scoped
/// program and shared by `Rc`: the claim-frontier walk built this table per
/// missed type reference, rescans of the declaration slice dominated its
/// cost.
pub(crate) fn memoized_data_definition_positions(
    program: &typed_trees::TypedTrees,
) -> std::rc::Rc<HashMap<symbols::SymbolHandle, usize>> {
    enum SlotState {
        NoScope,
        Hit(std::rc::Rc<HashMap<symbols::SymbolHandle, usize>>),
        Miss,
        ForeignProgram,
    }
    let state = DATA_DEF_POSITIONS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(std::rc::Rc::clone(map))
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(map) = state {
        return map;
    }
    let map = std::rc::Rc::new(
        program
            .data_definitions()
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.symbol, index))
            .collect::<HashMap<symbols::SymbolHandle, usize>>(),
    );
    if matches!(state, SlotState::Miss) {
        DATA_DEF_POSITIONS_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, map.clone()));
            }
        });
    }
    map
}

/// The symbol -> (multiplicity, kind) table over the type-parameter
/// declarations, built once per scoped program and shared by `Rc` for the
/// same reason as the data-definition positions.
pub(crate) fn memoized_type_parameter_multiplicities(
    program: &typed_trees::TypedTrees,
) -> std::rc::Rc<
    HashMap<
        symbols::SymbolHandle,
        (
            language_semantics::Multiplicity,
            typed_trees::data::TypeParameterKind,
        ),
    >,
> {
    enum SlotState {
        NoScope,
        Hit(
            std::rc::Rc<
                HashMap<
                    symbols::SymbolHandle,
                    (
                        language_semantics::Multiplicity,
                        typed_trees::data::TypeParameterKind,
                    ),
                >,
            >,
        ),
        Miss,
        ForeignProgram,
    }
    let state = TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if std::ptr::eq(*owner, program) {
                    SlotState::Hit(std::rc::Rc::clone(map))
                } else {
                    SlotState::ForeignProgram
                }
            }
        }
    });
    if let SlotState::Hit(map) = state {
        return map;
    }
    let map = std::rc::Rc::new(
        program
            .data_type_parameters
            .iter()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(_, parameter)| {
                (
                    parameter.symbol,
                    (parameter.bounds.multiplicity, parameter.kind.clone()),
                )
            })
            .collect::<HashMap<
                symbols::SymbolHandle,
                (
                    language_semantics::Multiplicity,
                    typed_trees::data::TypeParameterKind,
                ),
            >>(),
    );
    if matches!(state, SlotState::Miss) {
        TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(scope) = &mut *slot
            {
                *scope = Some((program, map.clone()));
            }
        });
    }
    map
}
