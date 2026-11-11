/*
The place family of the reader: which `data` declarations are records this
route serves, their fields as dense identities, record literals and member
paths.

Identity is fixed here, once, from symbols. A record type is the
`StructuralTypeId` minted for its `SymbolKind::Data` declaration the first
time a type names it; a field is its position in that declaration and the
`StructuralFieldId` minted with it, found from the field symbol stage 02
recorded on the literal or member. A field or type the typed trees hold only
as a spelling is refused as `Unserved::Spelling`.

The slice served is copy-eligible plain data: every field a builtin integer,
`bool` or another such record. Everything else about a `data` declaration is
refused by name, and later slices add to these functions rather than replace
them: sums and payloads (`type sum data`), data `requires` (`type data
requires`), custody (`type data with custody`), field stores and patterns.
*/

use semantic_vocabulary::{MachineId, StructuralFieldId, StructuralTypeId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{TableMemberExpression, TableStructLiteral};

use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, RecordField, RecordType, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

/// The symbols a record type was resolved from: its declaration and each
/// field, in declaration order, beside the `RecordType` row of the same
/// identity. Only the reader holds symbols; the body holds positions.
pub(super) struct RecordSource {
    declaration: SymbolHandle,
    fields: Vec<SymbolHandle>,
}

impl Program<'_> {
    /// The data declaration a `Named` type's symbol resolves to, when it is
    /// one. A symbol of kind `Data` with no declaration in the typed trees is
    /// not a record this route can read and stays with the scalar route's
    /// refusal.
    pub(super) fn data_declaration(&self, symbol: SymbolHandle) -> Option<&DataDefinition> {
        if !symbol.is_valid() || self.typed.symbols.get(symbol).kind != SymbolKind::Data {
            return None;
        }
        self.typed.data_definition_by_symbol(symbol)
    }

    /// The record type of a data declaration, resolving it the first time.
    pub(super) fn record(&mut self, id: MachineId, symbol: SymbolHandle) -> Checked<ValueType> {
        // ponytail: linear scan over the discovered record types, as for
        // machines; a dense table by declaration position if closures grow.
        for (record, source) in self.record_sources.iter() {
            if source.declaration == symbol {
                return Ok(ValueType::Record(record));
            }
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        if self.resolving.contains(&symbol) {
            return Err(unserved("data that contains itself"));
        }
        let typed = self.typed;
        let data = typed
            .data_definition_by_symbol(symbol)
            .ok_or_else(|| unserved("nominal type"))?;
        let members = typed.data_members(data);
        record_shape(id, data, members)?;
        // Terminal Psi's declaration needs a text identity; the typed trees
        // render it. Without one the declaration cannot be emitted.
        let identity = typed
            .normalized_nominal_type_identity(symbol)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("data without a declaration identity"),
                )
            })?;
        self.resolving.push(symbol);
        let mut fields = Vec::with_capacity(members.len());
        let mut symbols = Vec::with_capacity(members.len());
        for member in members {
            // `record_shape` refused every member that is not a field.
            let field = match member {
                DataMember::Field(field) => field,
                DataMember::Variant(_) => return Err(unserved("sum data")),
            };
            if !field.symbol.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("field without a symbol"),
                ));
            }
            let value_type = self.value_type(id, field.type_reference)?;
            let identity = field.member_identity().spelling();
            if identity.is_empty() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("field without a declaration identity"),
                ));
            }
            fields.push((identity, value_type));
            symbols.push(field.symbol);
        }
        self.resolving.pop();
        // Field identities are minted only now, after every nested record
        // resolved, so each record's fields are consecutive.
        let mut declared = Vec::with_capacity(fields.len());
        for (identity, value_type) in fields {
            self.fields_minted += 1;
            let field = StructuralFieldId::new(self.fields_minted)
                .ok_or_else(|| unserved("field identity overflow"))?;
            declared.push(RecordField {
                id: field,
                identity,
                value_type,
            });
        }
        let record: StructuralTypeId = self.records.push(RecordType {
            identity,
            fields: declared,
        });
        self.record_sources.push(RecordSource {
            declaration: symbol,
            fields: symbols,
        });
        Ok(ValueType::Record(record))
    }

    /// The position of the field `symbol` names in `record`'s declaration.
    fn field_position(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        symbol: SymbolHandle,
        absent: Unserved,
    ) -> Checked<usize> {
        let source =
            terminal_check_tables::row(&self.record_sources, record, id, Construct::Machine)?;
        source
            .fields
            .iter()
            .position(|field| *field == symbol)
            .ok_or_else(|| refuse(id, Construct::Machine, absent))
    }

    fn field_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        field: usize,
    ) -> Checked<ValueType> {
        terminal_check_tables::row(&self.records, record, id, Construct::Machine)?
            .fields
            .get(field)
            .map(|field| field.value_type)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("field position past the record's declaration"),
                )
            })
    }

    /// `receiver.member`: a field of the record the receiver names.
    pub(super) fn member(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        member: &TableMemberExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        if member.case_variant.is_some() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("case payload member"),
            ));
        }
        if !member.member_symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("member without a field symbol"),
            ));
        }
        let base = self.expression(id, slots, member.receiver, None)?;
        let Some(ValueType::Record(record)) = base.result else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("member of a value that is not a record"),
            ));
        };
        let field = self.field_position(
            id,
            record,
            member.member_symbol,
            Unserved::Typing("member is not a field of its receiver's record"),
        )?;
        Ok(Expression {
            result: Some(self.field_type(id, record, field)?),
            kind: ExpressionKind::Field {
                base: Box::new(base),
                field,
            },
            source,
        })
    }

    /// `R { a: x, b: y }`: every field of `R` exactly once, operands kept in
    /// authored order.
    pub(super) fn record_literal(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        literal: &TableStructLiteral,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
        if literal.case_name.is_some() || literal.case_symbol.is_some() {
            return Err(unserved("case literal"));
        }
        if !literal.type_symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("record literal without a type symbol"),
            ));
        }
        if self.data_declaration(literal.type_symbol).is_none() {
            return Err(unserved("literal of a type that is not a data declaration"));
        }
        let result = self.record(id, literal.type_symbol)?;
        let ValueType::Record(record) = result else {
            return Err(typing("record literal of a scalar type"));
        };
        // Stage 03 also records the literal's type as a type reference; it
        // must be this same record, or the literal applies a generic.
        if literal.application_type.is_valid()
            && self.value_type(id, literal.application_type)? != result
        {
            return Err(unserved("record literal of an applied type"));
        }
        let authored = self.typed.expression_table.struct_fields(literal.fields);
        let declared = terminal_check_tables::row(&self.records, record, id, Construct::Machine)?
            .fields
            .len();
        let mut seen = vec![false; declared];
        let mut fields = Vec::with_capacity(authored.len());
        for field in authored {
            if !field.field_symbol.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("record literal field without a symbol"),
                ));
            }
            let position = self.field_position(
                id,
                record,
                field.field_symbol,
                Unserved::Typing("record literal names a field of another type"),
            )?;
            if std::mem::replace(&mut seen[position], true) {
                return Err(typing("record literal names a field twice"));
            }
            let field_type = self.field_type(id, record, position)?;
            let operand = self.expression(id, slots, field.value, field_type.scalar())?;
            if operand.result != Some(field_type) {
                return Err(typing("record literal field type"));
            }
            fields.push((position, operand));
        }
        if seen.contains(&false) {
            return Err(typing("record literal misses a field"));
        }
        Ok(Expression {
            kind: ExpressionKind::Record { fields },
            result: Some(result),
            source,
        })
    }
}

/// Whether a data declaration is a plain record this slice serves. Each
/// feature outside it is refused by name, never ignored.
fn record_shape(id: MachineId, data: &DataDefinition, members: &[DataMember]) -> Checked<()> {
    let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return unserved("data without a checked shape");
    }
    if !data.lifetime_parameters.is_empty()
        || !data.type_parameters.is_empty()
        || data.generic_instance.is_some()
        || !data.conformance_bounds.is_empty()
    {
        return unserved("generic data");
    }
    if data.quotient.is_some() {
        return unserved("quotient data");
    }
    if !data.where_facts.is_empty()
        || data.predicate_subject_symbol.is_valid()
        || data.predicate_subject_type.is_valid()
    {
        return unserved("data requires");
    }
    // Copy eligibility (`places_foundation_lane.md`): "data with no disposal
    // obligation, no linear or affine field, no reference field and no
    // attached `Drop`". ownership.md: "Plain unrestricted primitive-array,
    // scalar-case, and plain record payloads have no disposal obligation."
    if data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
        || data.cleanup_selection.is_some()
    {
        return unserved("data with custody");
    }
    if data.properties.carry.is_some() {
        return unserved("data with a carry policy");
    }
    if data.zero_gated {
        return unserved("zero-gated data");
    }
    if !data.retired_identities.is_empty() {
        return unserved("data with retired field identities");
    }
    if members.is_empty() {
        return unserved("data without fields");
    }
    for member in members {
        match member {
            DataMember::Variant(_) => return unserved("sum data"),
            DataMember::Field(field) => {
                if field.relevance != language_core::BindingRelevance::Relevant {
                    return unserved("data with an erased field");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::{Handle, HandleSpan};
    use typed_trees::data::{DataField, DataProperties, DataVariant};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    /// `data P [copy] { x: ...; }` as the typed trees hold a plain record.
    fn plain() -> (DataDefinition, Vec<DataMember>) {
        let data = DataDefinition {
            properties: DataProperties {
                carry: None,
                multiplicity: language_semantics::Multiplicity::Unrestricted,
            },
            ..DataDefinition::default()
        };
        (data, vec![DataMember::Field(DataField::default())])
    }

    fn detail(data: &DataDefinition, members: &[DataMember]) -> String {
        let refusal = record_shape(machine(), data, members).unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        refusal.detail
    }

    /// A plain `[copy]` record is served; each declaration feature outside
    /// this slice is refused under its own name.
    #[test]
    fn each_data_declaration_outside_the_slice_is_refused_by_name() {
        let (data, members) = plain();
        assert_eq!(record_shape(machine(), &data, &members), Ok(()));
        let cases: [(&str, fn(&mut DataDefinition, &mut Vec<DataMember>)); 9] = [
            ("type data requires", |data, _| {
                data.where_facts = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
            }),
            ("type data with custody", |data, _| {
                data.properties.multiplicity = language_semantics::Multiplicity::Affine;
            }),
            ("type data with custody", |data, _| {
                data.properties.multiplicity = language_semantics::Multiplicity::Linear;
            }),
            ("type generic data", |data, _| {
                data.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
            }),
            ("type generic data", |data, _| {
                data.lifetime_parameters = vec![Default::default()];
            }),
            ("type sum data", |_, members| {
                members.push(DataMember::Variant(DataVariant::default()));
            }),
            ("type data with an erased field", |_, members| {
                members.push(DataMember::Field(DataField {
                    relevance: language_core::BindingRelevance::Erased,
                    ..DataField::default()
                }));
            }),
            ("type data without fields", |_, members| members.clear()),
            ("type data without a checked shape", |data, _| {
                data.supply_mode = language_semantics::DataSupplyMode::BoundaryOpaque;
            }),
        ];
        for (expected, change) in cases {
            let (mut data, mut members) = plain();
            change(&mut data, &mut members);
            assert_eq!(detail(&data, &members), expected);
        }
    }
}
