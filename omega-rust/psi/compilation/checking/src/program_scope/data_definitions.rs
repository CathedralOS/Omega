//! Declaration lookups delegated to the `TypedTrees`-owned indexes, plus the
//! per-scope `Rc` tables claim-frontier walks hold by reference.

use super::{DATA_DEF_POSITIONS_SLOT, TYPE_PARAMETER_MULTIPLICITY_SLOT, program_key};
use std::collections::HashMap;

/// The declaration-table answer for a symbol: absent, exactly one
/// definition at a position, or a duplicate.
#[derive(Clone, Copy)]
pub(crate) enum DataDefinitionLookup {
    Missing,
    Unique(u32),
    Duplicate,
}

/// The data-definition table's answer for `symbol`. The program's owned
/// declaration index keeps every position per symbol across mutations, so
/// the answer — including the negative and duplicate cases — is a single
/// index query rather than a re-scan of the whole declaration slice.
pub(crate) fn memoized_data_definition_lookup(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> DataDefinitionLookup {
    program.with_data_definition_index(|index| match index.positions_for_symbol(symbol) {
        [] => DataDefinitionLookup::Missing,
        [first] => DataDefinitionLookup::Unique(*first),
        [_, ..] => DataDefinitionLookup::Duplicate,
    })
}

/// A data definition by symbol, resolved through the program's owned
/// declaration index — `find`-style callers read the first match; callers
/// that require uniqueness should use [`memoized_data_definition_lookup`]
/// directly.
pub(crate) fn data_definition_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.data_definition_by_symbol(symbol)
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
/// payload order a whole-table `find_map` visits them. Built once inside
/// the frozen-program scope's identity-checked memos.
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

/// The symbol -> data-definition position table, built once per scoped
/// program and shared by `Rc`: the claim-frontier walk built this table per
/// missed type reference, rescans of the declaration slice dominated its
/// cost. The slot keys on the program's identity plus its structural
/// fingerprint — a replacement program can never inherit the table, and any
/// declaration mutation reopens it.
pub(crate) fn memoized_data_definition_positions(
    program: &typed_trees::TypedTrees,
) -> std::rc::Rc<HashMap<symbols::SymbolHandle, usize>> {
    enum SlotState {
        NoScope,
        Hit(std::rc::Rc<HashMap<symbols::SymbolHandle, usize>>),
        Miss,
        ForeignProgram,
    }
    let key = program_key(program);
    let state = DATA_DEF_POSITIONS_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if *owner == key {
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
                *scope = Some((key, map.clone()));
            }
        });
    }
    map
}

/// The symbol -> (multiplicity, kind) table over the type-parameter
/// declarations, built once per scoped program and shared by `Rc` for the
/// same reason as the data-definition positions. Keyed like that slot: the
/// program's identity plus its structural fingerprint.
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
    let key = program_key(program);
    let state = TYPE_PARAMETER_MULTIPLICITY_SLOT.with(|cell| {
        let cell = cell.borrow();
        match cell.as_ref() {
            None => SlotState::NoScope,
            Some(None) => SlotState::Miss,
            Some(Some((owner, map))) => {
                if *owner == key {
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
                *scope = Some((key, map.clone()));
            }
        });
    }
    map
}
