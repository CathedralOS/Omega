/*
The place family of the reader: which `data` declarations are records and
sums this route serves, their fields, cases and payload fields as dense
identities, record and case literals, member paths and case tests.

Identity is fixed here, once, from symbols. A data type is the
`StructuralTypeId` minted for its `SymbolKind::Data` declaration the first
time a type names it; a field is its position in that declaration and the
`StructuralFieldId` minted with it, found from the field symbol stage 02
recorded on the literal or member; a case is its position among the
declaration's cases and the `StructuralCaseId` minted with it, found from the
case symbol on a literal, a pattern head or the right operand of a case test.
A field, case or type the typed trees hold only as a spelling is refused as
`Unserved::Spelling`. A member's `case_variant` is a spelling and is never
read: a payload member is the payload field its symbol names.

The slice serves plain data with explicit copy permission or ordinary affine
custody: fields and payloads are builtin integers, `bool` or other such data.
Affine values transfer once and publish normal-edge disposal of their remaining
regions. Nominal cleanup hooks, linear claims, reference-bearing data and
extraction through borrowed storage remain outside this slice.
A declaration's `requires` is read into `RecordType::requires` by the clause
reader the machine contracts use (`contracts`), over `self` and its fields;
a `requires` on a case is refused (`type data requires`). Everything else
about a `data` declaration is refused by name, and later slices add to these
functions rather than replace them: custody (`type data with custody`),
common fields beside cases (`type mixed data`). Stores into a record's fields are read in
`statements`, dispatch over a sum in `transitions`.
*/

use semantic_vocabulary::{
    MachineId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, TableDestructurePattern,
    TableMemberExpression, TableStructLiteral,
};

use super::contracts::{ClauseScope, DataSubject, ResultTerm};
use super::{Program, Slot};
use crate::body::{CaseType, Expression, ExpressionKind, RecordField, RecordType, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

/// The symbols a data type was resolved from: its declaration, each field
/// and each case with its payload fields, in declaration order, beside the
/// `RecordType` row of the same identity. Only the reader holds symbols; the
/// body holds positions.
pub(super) struct RecordSource {
    pub(super) declaration: SymbolHandle,
    pub(super) fields: Vec<SymbolHandle>,
    pub(super) cases: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
}

/// What a member's field symbol names in its receiver's data type.
pub(super) enum Member {
    Field(usize),
    Payload { case: usize, field: usize },
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
        record_shape(id, data, members, |variant| {
            typed.data_payload_fields(variant)
        })?;
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
        // Fields and cases are read first and identities minted after, so a
        // nested data type resolves, and takes its identities, before its
        // owner and each type's fields are consecutive.
        let mut fields = Vec::new();
        let mut field_symbols = Vec::new();
        let mut cases = Vec::new();
        let mut case_symbols = Vec::new();
        for member in members {
            match member {
                DataMember::Field(field) => {
                    fields.push(self.declared_field(id, field)?);
                    field_symbols.push(field.symbol);
                }
                DataMember::Variant(variant) => {
                    if !variant.symbol.is_valid() {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Spelling("case without a symbol"),
                        ));
                    }
                    let identity = variant.member_identity().spelling();
                    if identity.is_empty() {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Spelling("case without a declaration identity"),
                        ));
                    }
                    let mut payload = Vec::new();
                    let mut payload_symbols = Vec::new();
                    for field in typed.data_payload_fields(variant) {
                        payload.push(self.declared_field(id, field)?);
                        payload_symbols.push(field.symbol);
                    }
                    cases.push((identity, payload));
                    case_symbols.push((variant.symbol, payload_symbols));
                }
            }
        }
        self.resolving.pop();
        let mut mint = |fields: Vec<(String, ValueType)>| {
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
            Ok(declared)
        };
        let declared = mint(fields)?;
        let mut declared_cases = Vec::with_capacity(cases.len());
        for (identity, payload) in cases {
            declared_cases.push((identity, mint(payload)?));
        }
        let mut case_types = Vec::with_capacity(declared_cases.len());
        for (identity, fields) in declared_cases {
            self.cases_minted += 1;
            let case = StructuralCaseId::new(self.cases_minted)
                .ok_or_else(|| unserved("case identity overflow"))?;
            case_types.push(CaseType {
                id: case,
                identity,
                fields,
            });
        }
        let copyable =
            data.properties.multiplicity == language_semantics::Multiplicity::Unrestricted;
        if copyable
            && !declared
                .iter()
                .chain(case_types.iter().flat_map(|case| &case.fields))
                .all(|field| copy_field_is_unrestricted(field.value_type, &self.records))
        {
            return Err(unserved("copy data with an owned field"));
        }
        let record: StructuralTypeId = self.records.push(RecordType {
            copyable,
            identity,
            fields: declared,
            cases: case_types,
            requires: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            declaration: symbol,
            fields: field_symbols,
            cases: case_symbols,
        });
        // The declaration's `requires`, read once the fields and cases it
        // names have their identities, by the clause reader the machine
        // contracts use.
        let root = crate::body::self_place().ok_or_else(|| unserved("self place identity"))?;
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in a data clause"),
            formal_fields: false,
            subject: Some(DataSubject {
                record,
                symbol: data.predicate_subject_symbol,
                root,
            }),
        };
        let facts = super::span(
            &typed.proof_facts,
            data.requires_facts,
            id,
            "data requires span",
        )?;
        let mut requires = Vec::with_capacity(facts.len());
        for fact in facts {
            requires.push(self.clause(id, &scope, self.fact(id, fact)?)?);
        }
        self.records
            .get_mut(record)
            .ok_or_else(|| unserved("record identity"))?
            .requires = requires;
        Ok(ValueType::Record(record))
    }

    /// One declared field or payload field: its text identity for Terminal
    /// Psi's declaration and its type.
    fn declared_field(&mut self, id: MachineId, field: &DataField) -> Checked<(String, ValueType)> {
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
        Ok((identity, value_type))
    }

    fn source(&self, id: MachineId, record: StructuralTypeId) -> Checked<&RecordSource> {
        terminal_check_tables::row(&self.record_sources, record, id, Construct::Machine)
    }

    pub(super) fn data_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
    ) -> Checked<&RecordType> {
        terminal_check_tables::row(&self.records, record, id, Construct::Machine)
    }

    /// The position of the field `symbol` names in `record`'s declaration.
    fn field_position(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        symbol: SymbolHandle,
        absent: Unserved,
    ) -> Checked<usize> {
        self.source(id, record)?
            .fields
            .iter()
            .position(|field| *field == symbol)
            .ok_or_else(|| refuse(id, Construct::Machine, absent))
    }

    /// What the field symbol of a member names in `record`: a field, or a
    /// payload field of one of its cases. Each field symbol is one
    /// declaration, so at most one answer exists.
    pub(super) fn member_of(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        symbol: SymbolHandle,
    ) -> Checked<Member> {
        let source = self.source(id, record)?;
        if let Some(field) = source.fields.iter().position(|field| *field == symbol) {
            return Ok(Member::Field(field));
        }
        for (case, (_, payload)) in source.cases.iter().enumerate() {
            if let Some(field) = payload.iter().position(|field| *field == symbol) {
                return Ok(Member::Payload { case, field });
            }
        }
        Err(refuse(
            id,
            Construct::Machine,
            Unserved::Typing("member is not a field of its receiver's data type"),
        ))
    }

    /// The position of the case `symbol` names among `record`'s cases.
    /// structural_access.md: "Its case identity must belong to the exact
    /// selected sum type; matching case spellings in another type supply no
    /// authority." The answer comes from the case symbol alone.
    pub(super) fn case_position(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        symbol: SymbolHandle,
    ) -> Checked<usize> {
        self.source(id, record)?
            .cases
            .iter()
            .position(|(case, _)| *case == symbol)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("case of another type than its subject's"),
                )
            })
    }

    fn field_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        field: usize,
    ) -> Checked<ValueType> {
        self.data_type(id, record)?
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

    fn payload_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        case: usize,
        field: usize,
    ) -> Checked<ValueType> {
        self.data_type(id, record)?
            .cases
            .get(case)
            .and_then(|case| case.fields.get(field))
            .map(|field| field.value_type)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("payload position past the case's declaration"),
                )
            })
    }

    /// `let { a, b as _ } = subject;`: the marker local of a record pattern.
    /// It binds nothing and lowers to nothing. The bound fields are the
    /// ordinary locals that follow it, each a field read of the subject; the
    /// marker only says which fields the author named, and the pattern is
    /// served when those are every field of the subject's record.
    /// `wiki/spec/language/patterns.md`: "A pattern that can fail rejects; a
    /// binding introduces no hidden branch, trap, fallback or runtime
    /// assertion." Coverage is read from the roster's field symbols and the
    /// record's own; no spelling is compared.
    pub(super) fn record_pattern(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        local: &typed_trees::statement::TableLocalData,
    ) -> Checked<()> {
        let refused = |what| refuse(id, Construct::Machine, what);
        if !local.pattern_subject.is_valid() {
            return Err(refused(Unserved::Spelling(
                "record pattern without a resolved subject",
            )));
        }
        let subject = self.expression(id, slots, local.initial_value, None)?;
        let mut place = &subject;
        while let ExpressionKind::Field { base, .. } = &place.kind {
            place = base;
        }
        if !matches!(place.kind, ExpressionKind::Slot(_)) {
            return Err(refused(Unserved::Statement(
                "record pattern over a value that is not a place",
            )));
        }
        let Some(ValueType::Record(record)) = subject.result else {
            return Err(refused(Unserved::Typing(
                "record pattern over a value that is not a record",
            )));
        };
        let source =
            terminal_check_tables::row(&self.record_sources, record, id, Construct::Machine)?;
        if source.declaration != local.pattern_subject {
            return Err(refused(Unserved::Typing(
                "record pattern resolved against another record",
            )));
        }
        if !source.cases.is_empty() {
            return Err(refused(Unserved::Typing("record pattern over a sum")));
        }
        let roster = self
            .typed
            .expression_table
            .match_fields(local.pattern_fields);
        if roster
            .iter()
            .any(|row| !row.field_symbol.is_valid() || !source.fields.contains(&row.field_symbol))
        {
            return Err(refused(Unserved::Spelling(
                "record pattern field without a symbol",
            )));
        }
        if source
            .fields
            .iter()
            .any(|field| !roster.iter().any(|row| row.field_symbol == *field))
        {
            return Err(refused(Unserved::Statement(
                "record pattern that omits a field",
            )));
        }
        Ok(())
    }

    /// `receiver.member`: a field of the record, or a payload field of a
    /// case of the sum, that the receiver names. Which of the two is read
    /// from the member's field symbol; a member the typed trees hold only as
    /// a spelling is refused. A directly authored payload member (`s.at`)
    /// carries its payload field's symbol when exactly one case declares
    /// that field, as stage 03 records it; whether the sum holds that case
    /// where it is read is judged on the lowered form.
    pub(super) fn member(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        member: &TableMemberExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
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
        let base = Box::new(base);
        Ok(match self.member_of(id, record, member.member_symbol)? {
            Member::Field(field) => Expression {
                result: Some(self.field_type(id, record, field)?),
                kind: ExpressionKind::Field { base, field },
                source,
            },
            Member::Payload { case, field } => Expression {
                result: Some(self.payload_type(id, record, case, field)?),
                kind: ExpressionKind::Payload { base, case, field },
                source,
            },
        })
    }

    /// `R { a: x, b: y }`: every field of `R` exactly once, operands kept in
    /// authored order. `S::C { p: x }`, `S::C {}` and `S::C`: every payload
    /// field of the case exactly once, the same way.
    pub(super) fn record_literal(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        literal: &TableStructLiteral,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
        let spelling = |what| refuse(id, Construct::Machine, Unserved::Spelling(what));
        // A case literal names its case by symbol; one the typed trees hold
        // only as a name is a spelling.
        let case_symbol = match (literal.case_symbol, &literal.case_name) {
            (Some(symbol), _) if symbol.is_valid() => Some(symbol),
            (Some(_), _) | (None, Some(_)) => {
                return Err(spelling("case literal without a case symbol"));
            }
            (None, None) => None,
        };
        if !literal.type_symbol.is_valid() {
            return Err(spelling("record literal without a type symbol"));
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
        let case = match case_symbol {
            Some(symbol) => Some(self.case_position(id, record, symbol)?),
            None => None,
        };
        let data = self.data_type(id, record)?;
        let declared: Vec<ValueType> = match case {
            Some(case) => data
                .cases
                .get(case)
                .ok_or_else(|| typing("case position past the sum's declaration"))?
                .fields
                .iter()
                .map(|field| field.value_type)
                .collect(),
            None if data.cases.is_empty() => {
                data.fields.iter().map(|field| field.value_type).collect()
            }
            None => return Err(typing("literal of a sum without a case")),
        };
        let authored = self.typed.expression_table.struct_fields(literal.fields);
        let mut seen = vec![false; declared.len()];
        let mut fields = Vec::with_capacity(authored.len());
        for field in authored {
            if !field.field_symbol.is_valid() {
                return Err(spelling("record literal field without a symbol"));
            }
            let position = match case {
                None => self.field_position(
                    id,
                    record,
                    field.field_symbol,
                    Unserved::Typing("record literal names a field of another type"),
                )?,
                Some(case) => match self.member_of(id, record, field.field_symbol)? {
                    Member::Payload { case: owner, field } if owner == case => field,
                    Member::Payload { .. } | Member::Field(_) => {
                        return Err(typing("case literal names a field of another case"));
                    }
                },
            };
            if std::mem::replace(&mut seen[position], true) {
                return Err(typing("record literal names a field twice"));
            }
            let field_type = declared[position];
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
            kind: match case {
                None => ExpressionKind::Record { fields },
                Some(case) => ExpressionKind::Case { case, fields },
            },
            result: Some(result),
            source,
        })
    }

    /// The case an expression names when it stands where a case is
    /// expected: a name path that resolves to a case symbol, or a case
    /// literal with no payload operand. Stage 01 writes the right operand of
    /// `subject in S::C` and a pattern head as the former and a payload-free
    /// `S::C` value as the latter.
    pub(super) fn named_case(&self, handle: ExpressionHandle) -> Option<SymbolHandle> {
        if !handle.is_valid() {
            return None;
        }
        let symbol = match self.typed.expression_table.expression(handle) {
            ExpressionNode::Name(path) => path.symbol,
            ExpressionNode::StructLiteral(literal) => {
                if !literal.fields.is_empty() {
                    return None;
                }
                literal.case_symbol?
            }
            ExpressionNode::Indexed(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::SavedSubject(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Atomic(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Call(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::String(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::ZeroValue(_) => return None,
        };
        (symbol.is_valid() && self.typed.symbols.get(symbol).kind == SymbolKind::Variant)
            .then_some(symbol)
    }

    /// `subject in S::C`: the subject, read once, and the case's position in
    /// the subject's own sum type.
    pub(super) fn case_test(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        subject: ExpressionHandle,
        case: SymbolHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let subject = self.expression(id, slots, subject, None)?;
        let Some(ValueType::Record(record)) = subject.result else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("case test of a value that is not a sum"),
            ));
        };
        let case = self.case_position(id, record, case)?;
        Ok(Expression {
            kind: ExpressionKind::IsCase {
                subject: Box::new(subject),
                case,
            },
            result: Some(ValueType::Scalar(ScalarType::Boolean)),
            source,
        })
    }

    /// The case a destructuring pattern selects in the sum `record`, after
    /// checking that the pattern is one this slice serves: a case head and
    /// payload fields that are each bound or waived. patterns.md: "A pattern
    /// that can fail rejects; a binding introduces no hidden branch, trap,
    /// fallback or runtime assertion." A binding has no node of its own:
    /// stage 01 replaces each use of it by the payload member it names. The
    /// pattern must name every payload field unless it ends in `..`; the
    /// current checker enforces that law in validation, after the typed
    /// trees this route reads are frozen, so it is enforced here too.
    pub(super) fn case_pattern(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        pattern: &TableDestructurePattern,
    ) -> Checked<usize> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Transition(what));
        if pattern.application_type.is_valid() {
            return Err(unserved("pattern of an applied type"));
        }
        if !pattern.proof_selectors.is_empty() {
            return Err(unserved("pattern with proof selectors"));
        }
        if pattern.guard.is_valid() {
            return Err(unserved("case arm with a guard"));
        }
        let Some(symbol) = self.named_case(pattern.head) else {
            return Err(unserved("pattern head that is not a case"));
        };
        let case = self.case_position(id, record, symbol)?;
        let payload = self.source(id, record)?.cases[case].1.len();
        let mut seen = vec![false; payload];
        for field in self.typed.expression_table.match_fields(pattern.fields) {
            match field.component {
                language_core::PatternComponent::Fields => {}
                language_core::PatternComponent::Case => {
                    return Err(unserved("pattern that binds its case"));
                }
            }
            match &field.action {
                MatchFieldAction::Bind(_) | MatchFieldAction::Waive => {}
                MatchFieldAction::Equal(_) => {
                    return Err(unserved("pattern field tested against a value"));
                }
            }
            if !field.field_symbol.is_valid() {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Spelling("pattern field without a symbol"),
                ));
            }
            let position = match self.member_of(id, record, field.field_symbol)? {
                Member::Payload { case: owner, field } if owner == case => field,
                Member::Payload { .. } | Member::Field(_) => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Typing("pattern names a field of another case"),
                    ));
                }
            };
            if std::mem::replace(&mut seen[position], true) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("pattern names a field twice"),
                ));
            }
        }
        if !pattern.has_rest && seen.contains(&false) {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("pattern misses a payload field"),
            ));
        }
        Ok(case)
    }
}

/// Whether a data declaration is a plain record or a plain sum this slice
/// serves. Each feature outside it is refused by name, never ignored.
fn record_shape<'m>(
    id: MachineId,
    data: &DataDefinition,
    members: &'m [DataMember],
    payload: impl Fn(&'m typed_trees::data::DataVariant) -> &'m [DataField],
) -> Checked<()> {
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
    // Plain affine values need explicit moves and no-code disposal, even when
    // every field is copyable. Linear claims and selected cleanup machines
    // require their own disposition contracts and remain outside this reader.
    if data.properties.multiplicity == language_semantics::Multiplicity::Linear
        || data.cleanup_selection.is_some()
    {
        return unserved("data with custody");
    }
    if data.properties.carry.is_some() {
        return unserved("data with a carry policy");
    }
    // `zero_gated` is not read: whether the zero value satisfies the
    // declaration's `requires` is this route's own proof question, asked
    // where a local of the type is declared without an initializer.
    if !data.retired_identities.is_empty() {
        return unserved("data with retired field identities");
    }
    if members.is_empty() {
        return unserved("data without fields");
    }
    let relevant = |field: &DataField| field.relevance == language_core::BindingRelevance::Relevant;
    let mut fields = false;
    let mut cases = false;
    for member in members {
        match member {
            DataMember::Variant(variant) => {
                cases = true;
                if !variant.requires_facts.is_empty() {
                    return unserved("data requires");
                }
                if !variant.retired_payload_identities.is_empty() {
                    return unserved("data with retired field identities");
                }
                if !payload(variant).iter().all(relevant) {
                    return unserved("data with an erased field");
                }
            }
            DataMember::Field(field) => {
                fields = true;
                if !relevant(field) {
                    return unserved("data with an erased field");
                }
            }
        }
    }
    // Common fields beside cases: a case literal names no common field
    // (`Token::Mark { kind: 2 }`), so what they hold after construction
    // is a rule of its own that no slice of this lane lowers yet.
    if fields && cases {
        return unserved("mixed data");
    }
    Ok(())
}

// Copy permission must reach every nested value. Scalar fields alone do not
// grant it to their affine owner, and a missing record identity grants nothing.
fn copy_field_is_unrestricted(
    field: ValueType,
    records: &semantic_vocabulary::IdTable<StructuralTypeId, RecordType>,
) -> bool {
    match field {
        ValueType::Scalar(_) => true,
        ValueType::Record(record) => records.get(record).is_some_and(|record| record.copyable),
        ValueType::Rigid(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::{Handle, HandleSpan};
    use typed_trees::data::{DataField, DataProperties, DataVariant};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    #[test]
    fn copy_permission_does_not_hide_an_owned_nested_record() {
        let mut records = semantic_vocabulary::IdTable::default();
        let owned = records.push(RecordType {
            copyable: false,
            identity: "Owned".into(),
            fields: Vec::new(),
            cases: Vec::new(),
            requires: Vec::new(),
        });
        assert!(!copy_field_is_unrestricted(
            ValueType::Record(owned),
            &records
        ));
        records.get_mut(owned).unwrap().copyable = true;
        assert!(copy_field_is_unrestricted(
            ValueType::Record(owned),
            &records
        ));
        assert!(!copy_field_is_unrestricted(
            ValueType::Record(StructuralTypeId::new(2).unwrap()),
            &records,
        ));
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
        let refusal = record_shape(machine(), data, members, |_| &[]).unwrap_err();
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
        assert_eq!(record_shape(machine(), &data, &members, |_| &[]), Ok(()));
        let mut affine = data.clone();
        affine.properties.multiplicity = language_semantics::Multiplicity::Affine;
        assert_eq!(record_shape(machine(), &affine, &members, |_| &[]), Ok(()));
        // A declaration-level `requires` is no longer refused here: its
        // clauses are read by the clause reader. A case-level one still is.
        let cases: [(&str, fn(&mut DataDefinition, &mut Vec<DataMember>)); 9] = [
            ("type data with custody", |data, _| {
                data.properties.multiplicity = language_semantics::Multiplicity::Linear;
            }),
            ("type generic data", |data, _| {
                data.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
            }),
            ("type generic data", |data, _| {
                data.lifetime_parameters = vec![Default::default()];
            }),
            // Cases beside a field: a sum alone is served, the mix is not.
            ("type mixed data", |_, members| {
                members.push(DataMember::Variant(DataVariant::default()));
            }),
            ("type data requires", |_, members| {
                *members = vec![DataMember::Variant(DataVariant {
                    requires_facts: HandleSpan::from_parts(Handle::from_parts(1, 1), 1),
                    ..DataVariant::default()
                })];
            }),
            ("type data with retired field identities", |_, members| {
                *members = vec![DataMember::Variant(DataVariant {
                    retired_payload_identities: vec![2],
                    ..DataVariant::default()
                })];
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
        // A sum alone, with or without payload, is served.
        let (data, _) = plain();
        let sum = vec![
            DataMember::Variant(DataVariant::default()),
            DataMember::Variant(DataVariant::default()),
        ];
        assert_eq!(record_shape(machine(), &data, &sum, |_| &[]), Ok(()));
        let erased = [DataField {
            relevance: language_core::BindingRelevance::Erased,
            ..DataField::default()
        }];
        assert_eq!(
            record_shape(machine(), &data, &sum, |_| &erased)
                .unwrap_err()
                .detail,
            "type data with an erased field"
        );
    }
}
