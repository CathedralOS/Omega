//! Declaration lookups delegated to the `TypedTrees`-owned indexes, plus the
//! per-scope `Rc` tables claim-frontier walks hold by reference.

use super::{
    DATA_DEF_POSITIONS_SLOT, TYPE_PARAMETER_MULTIPLICITY_SLOT, scoped_program_map,
};
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
    scoped_program_map(program, &DATA_DEF_POSITIONS_SLOT, |program| {
        program
            .data_definitions()
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.symbol, index))
            .collect()
    })
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
    scoped_program_map(program, &TYPE_PARAMETER_MULTIPLICITY_SLOT, |program| {
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
            .collect()
    })
}

/// The named field of `definition`, when it declares one.
pub(crate) fn data_field_named<'a>(
    program: &'a typed_trees::TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    name: typed_trees::name::Identifier,
) -> Option<&'a typed_trees::data::DataField> {
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(data_field) if data_field.name == name => {
                Some(data_field)
            }
            _ => None,
        })
}

/// The declared type of the one field `symbol` names across every data
/// definition.
pub(crate) fn data_field_type_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if symbol.is_valid() && field.symbol == symbol =>
            {
                Some(field.type_reference)
            }
            _ => None,
        })
}
