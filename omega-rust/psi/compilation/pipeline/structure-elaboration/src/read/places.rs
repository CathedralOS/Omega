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
A declaration may state lifetime parameters, `data Outer<'a>`, and a field
may apply such data at them, `inner: Inner<'a>`: they are erased regions, so
the declaration is one record row (`lifetime_binders`, `lifetime_application`),
and the regions a value of it is at belong to its value type (`regions`): a
field read, a literal or a store sees the field types at them
(`field_type`). A field may store a byte view, `&'a [u8]`, whose type carries
the region it names (`bytes`); a reference to anything else in a
field is still refused (`type reference`).
A declaration's `requires` is read into `RecordType::requires` by the clause
reader the machine contracts use (`contracts`), over `self` and its fields;
a `requires` on a case is refused (`type data requires`). Everything else
about a `data` declaration is refused by name, and later slices add to these
functions rather than replace them: custody (`type data with custody`),
common fields beside cases (`type mixed data`). Stores into a record's fields are read in
`statements`, dispatch over a sum in `transitions`.
*/

use crate::body::Qualification;
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
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

use super::authority::Access;
use super::contracts::{ClauseScope, DataSubject, MathematicalTerms, ResultTerm};
use super::{Program, Slot};
use crate::body::{CaseType, Expression, ExpressionKind, RecordField, RecordType, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

/// The symbols a data type was resolved from: its declaration, each field
/// and each case with its payload fields, in declaration order, beside the
/// `RecordType` row of the same identity. Only the reader holds symbols; the
/// body holds positions.
pub(super) struct RecordSource {
    pub(super) declaration: SymbolHandle,
    pub(super) application: Option<typed_trees::SourceTypeApplication>,
    /// The declaration whose lifetime binders a byte view's region is one
    /// of, when the typed trees retained no owner for that occurrence: the
    /// data declaration the view is a field of. Invalid everywhere else.
    pub(super) region_owner: SymbolHandle,
    pub(super) fields: Vec<SymbolHandle>,
    pub(super) cases: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
}

/// What `declared_members` reads: the fields and their symbols, then the cases
/// (identity and payload fields) and their symbols with their payloads'.
type Members = (
    Vec<(String, ValueType)>,
    Vec<SymbolHandle>,
    Vec<(String, Vec<(String, ValueType)>)>,
    Vec<(SymbolHandle, Vec<SymbolHandle>)>,
);

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

    /// A declaration root with no explicit application. Generic declarations
    /// need their actual retained application, never a guessed empty tuple.
    pub(super) fn record(&mut self, id: MachineId, symbol: SymbolHandle) -> Checked<ValueType> {
        let typed = self.typed;
        let data = typed
            .unique_data_definition_by_symbol(symbol)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Type("nominal type without a unique declaration"),
                )
            })?;
        if let Some(reference) = data.generic_instance {
            return self.record_at(id, reference);
        }
        // Named with no application, the declaration is at its own regions:
        // a value of it holds views of whatever regions its declaration's
        // parameters stand for (`regions`).
        let elided = self.elided_regions(id, data.symbol)?;
        let regions = self.intern_regions(elided);
        self.record_definition(id, data, None)
            .map(|value_type| with_regions(value_type, regions))
    }

    pub(super) fn record_at(
        &mut self,
        id: MachineId,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let typed = self.typed;
        let data = typed.closed_data_definition(reference).ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Type("record without its exact closed application"),
            )
        })?;
        let application = super::source_records::source_application(typed, data, reference, id)?;
        let regions = self.occurrence_regions(id, data, reference)?;
        self.record_definition(id, data, Some(application))
            .map(|value_type| with_regions(value_type, regions))
    }

    /// The regions a type occurrence applies `data` at. `Message<'a>` names
    /// `'a`: the parameter the typed trees resolved that argument to, else
    /// the one its tag names in the occurrence's lifetime owner
    /// (lifetimes.md: a named data application must "use binders declared in
    /// the lexical owner"). `Message` with its lifetimes elided is at the
    /// declaration's own regions.
    fn occurrence_regions(
        &mut self,
        id: MachineId,
        data: &DataDefinition,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Checked<crate::body::Regions> {
        use typed_trees::type_identity::lifetime_parameter_identity;
        if data.lifetime_parameters.is_empty() {
            return Ok(crate::body::Regions::NONE);
        }
        let typed = self.typed;
        let table = &typed.type_reference_table;
        let arguments: &[Identifier] = match table.type_reference(reference) {
            TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments,
            _ => &[],
        };
        if arguments.is_empty() {
            let elided = self.elided_regions(id, data.symbol)?;
            return Ok(self.intern_regions(elided));
        }
        let retained = table.operand_lifetimes(reference);
        let owner = match table.operand_lifetime_owner(reference) {
            owner if owner.is_valid() => owner,
            _ => self.lifetime_owner,
        };
        let mut tuple = Vec::with_capacity(arguments.len());
        for (slot, argument) in arguments.iter().enumerate() {
            let parameter = match retained.get(slot) {
                Some(region) if region.declaration.is_valid() => Some(*region),
                _ if owner.is_valid() => {
                    lifetime_parameter_identity(typed, owner, argument.as_str())
                }
                _ => None,
            };
            let parameter = parameter.ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("lifetime argument its owner does not declare"),
                )
            })?;
            tuple.push(super::regions::Region::Parameter(parameter));
        }
        Ok(self.intern_regions(tuple))
    }

    /// The declaration a record row was read from; invalid for a row with
    /// none of its own (a view, an array, a literal).
    pub(crate) fn source_declaration(
        &self,
        id: MachineId,
        record: StructuralTypeId,
    ) -> Checked<SymbolHandle> {
        Ok(self.source(id, record)?.declaration)
    }

    fn record_definition(
        &mut self,
        id: MachineId,
        data: &DataDefinition,
        application: Option<typed_trees::SourceTypeApplication>,
    ) -> Checked<ValueType> {
        let typed = self.typed;
        let symbol = data.symbol;
        self.served(id, symbol)?;
        let same = |owner: SymbolHandle, retained: &Option<typed_trees::SourceTypeApplication>| {
            owner == symbol
                && match (retained, &application) {
                    (Some(retained), Some(application)) => {
                        super::source_records::same_application(typed, retained, application)
                    }
                    (None, None) => true,
                    (None, Some(application)) | (Some(application), None) => {
                        data.generic_instance.is_none()
                            && data.type_parameters.is_empty()
                            && application.substitutions.is_empty()
                    }
                }
        };
        let known = self.record_sources.iter().find_map(|(record, source)| {
            same(source.declaration, &source.application).then_some(record)
        });
        if let Some(record) = known {
            if let Some(application) = &application {
                self.record_application(id, record, application.reference)?;
            }
            return Ok(ValueType::record(record));
        }
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        if self
            .resolving
            .iter()
            .any(|(owner, retained)| same(*owner, retained))
        {
            return Err(unserved("data application that contains itself"));
        }
        let members = typed.data_members(data);
        record_shape(id, data, application.as_ref(), members, |variant| {
            typed.data_payload_fields(variant)
        })?;
        lifetime_binders(id, &data.lifetime_parameters)?;
        // Terminal Psi's declaration needs a text identity; the typed trees
        // render it. Without one the declaration cannot be emitted.
        let identity = match &application {
            Some(application) => Some(typed.normalized_type_identity(application.reference)),
            None => typed.normalized_nominal_type_identity(symbol),
        }
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| unserved("data without a complete application identity"))?;
        self.resolving.push((symbol, application.clone()));
        // Field types are read with this declaration as the owner of the
        // lifetimes they name: lifetimes.md, a named data application must
        // "use binders declared in the lexical owner". A nested declaration
        // is read under its own, so the enclosing owner is put back whether
        // or not the members were served.
        let enclosing = std::mem::replace(&mut self.lifetime_owner, symbol);
        let elision = std::mem::replace(&mut self.elision, super::regions::Elision::Declaration);
        let read = (|| {
            // A closed metadata row is not argument admission. Each supplied
            // type must itself be a type this same checked route can serve.
            if let Some(application) = &application {
                for (_, argument) in &application.substitutions {
                    self.value_type(id, *argument)?;
                }
            }
            self.declared_members(id, members)
        })();
        self.lifetime_owner = enclosing;
        self.elision = elision;
        self.resolving.pop();
        let (fields, field_symbols, cases, case_symbols) = read?;
        let mut mint = |fields: Vec<(String, ValueType)>| {
            let mut declared = Vec::with_capacity(fields.len());
            for (identity, value_type) in fields {
                self.fields_minted += 1;
                let field = StructuralFieldId::new(self.fields_minted)
                    .ok_or_else(|| unserved("field identity overflow"))?;
                let byte_predicates = self.view_predicates(id, value_type)?;
                declared.push(RecordField {
                    id: field,
                    identity,
                    value_type,
                    byte_predicates,
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
            shape: if case_types.is_empty() {
                crate::StorageShape::Record(declared)
            } else {
                crate::StorageShape::Sum(case_types)
            },
            requires: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            declaration: symbol,
            application: application.clone(),
            region_owner: SymbolHandle::invalid(),
            fields: field_symbols,
            cases: case_symbols,
        });
        if application.is_none() && data.predicate_subject_type.is_valid() {
            self.record_application(id, record, data.predicate_subject_type)?;
        }
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
            mathematical: MathematicalTerms::Refused("mathematical comparison in a data requires"),
            subject: Some(DataSubject {
                value_type: ValueType::record(record),
                symbol: data.predicate_subject_symbol,
                root,
                path: &[],
                bare_fields: true,
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
        // dependent_values.md: "A data declaration's field qualifications
        // and `requires` clauses define its default domain". A field in a
        // domain with predicates owes them as clauses of this type, over
        // the field (`predicates`).
        let declared = self
            .records
            .get(record)
            .ok_or_else(|| unserved("record identity"))?;
        let fields: Vec<_> = declared
            .fields()
            .iter()
            .map(|field| (field.id, field.value_type))
            .collect();
        let payloads: Vec<ValueType> = declared
            .cases()
            .iter()
            .flat_map(|case| case.fields.iter().map(|field| field.value_type))
            .collect();
        for (field, value_type) in fields {
            let path = [semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                field,
            )];
            requires.extend(self.field_predicates(id, &path, value_type)?);
        }
        // A payload's clause holds only of a value in its case, and no
        // clause here is conditional on the case yet.
        for value_type in payloads {
            if value_type.qualification().domains.members().any(|domain| {
                typed
                    .domain_definition(domain)
                    .is_some_and(super::authority::has_predicates)
            }) {
                return Err(unserved("case payload in a domain with predicates"));
            }
        }
        self.records
            .get_mut(record)
            .ok_or_else(|| unserved("record identity"))?
            .requires = requires;
        Ok(ValueType::record(record))
    }

    /// The fields and cases of a data declaration, each with its symbols,
    /// in declaration order. Fields and cases are read first and identities
    /// minted after, so a nested data type resolves, and takes its
    /// identities, before its owner and each type's fields are consecutive.
    fn declared_members(&mut self, id: MachineId, members: &[DataMember]) -> Checked<Members> {
        let typed = self.typed;
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
        Ok((fields, field_symbols, cases, case_symbols))
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
        // A field that names lifetime-parameterised data bare leaves out
        // which of its owner's lifetimes the nested views live for.
        // lifetimes.md gives elision to a signature ("For an elided output,
        // one unambiguous reference input supplies the relation") and says
        // of a named data application that it must "match the declaration's
        // lifetime arity", so a field states its arguments.
        if field.type_reference.is_valid()
            && let TypeReferenceNode::Named { symbol, .. } = self
                .typed
                .type_reference_table
                .type_reference(field.type_reference)
            && self
                .data_declaration(*symbol)
                .is_some_and(|data| !data.lifetime_parameters.is_empty())
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("data field that elides a lifetime argument"),
            ));
        }
        let value_type = self.qualified_view_type(id, field.type_reference)?;
        // lifetimes.md: "A lifetime relates a borrowed result or stored view
        // to storage that remains valid for its use." The one view a field
        // may store is a byte view, a `Record` here, and its type is keyed
        // by the region it names under this declaration (`byte_view_type`):
        // a view of any other region is another type and is not a value of
        // the field. A slice that adds a field type storing a view some
        // other way adds its lifetime accounting with the arm it adds here.
        match value_type {
            ValueType::Scalar(..) | ValueType::Record(_, _, _) | ValueType::Rigid(_) => {}
        }
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

    /// A field's type as a value of `record` at `regions` sees it: the
    /// row's declared type with the declaration's own lifetimes replaced by
    /// the value's (`regions::field_regions`).
    fn field_type(
        &mut self,
        id: MachineId,
        record: StructuralTypeId,
        regions: crate::body::Regions,
        field: usize,
    ) -> Checked<ValueType> {
        let declared = self.declared_field_type(id, record, field)?;
        let substitution = self.field_regions(id, record, regions)?;
        Ok(self.substitute(declared, &substitution))
    }

    fn declared_field_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        field: usize,
    ) -> Checked<ValueType> {
        self.data_type(id, record)?
            .fields()
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
        &mut self,
        id: MachineId,
        record: StructuralTypeId,
        regions: crate::body::Regions,
        case: usize,
        field: usize,
    ) -> Checked<ValueType> {
        let declared = self.declared_payload_type(id, record, case, field)?;
        let substitution = self.field_regions(id, record, regions)?;
        Ok(self.substitute(declared, &substitution))
    }

    fn declared_payload_type(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        case: usize,
        field: usize,
    ) -> Checked<ValueType> {
        self.data_type(id, record)?
            .cases()
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
        let Some(record) = self.opened(id, subject.result, Access::Read)? else {
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
        let Some(record) = self.opened(id, base.result, Access::Read)? else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("member of a value that is not a record"),
            ));
        };
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let base = Box::new(base);
        Ok(match self.member_of(id, record, member.member_symbol)? {
            Member::Field(field) => Expression {
                result: Some(self.field_type(id, record, regions, field)?),
                kind: ExpressionKind::Field { base, field },
                source,
            },
            Member::Payload { case, field } => Expression {
                result: Some(self.payload_type(id, record, regions, case, field)?),
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
        expression: ExpressionHandle,
        literal: &TableStructLiteral,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let expected = self.expected.take();
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
        let applications = [
            literal.application_type,
            self.typed
                .expression_table
                .constructed_value_type_reference(expression),
            self.typed
                .expression_const_declaration(expression)
                .map_or_else(
                    typed_trees::types::TypeReferenceHandle::invalid,
                    |constant| constant.declared_type,
                ),
        ];
        let result = match applications
            .iter()
            .copied()
            .find(|reference| reference.is_valid())
        {
            Some(reference) => {
                let selected = self
                    .typed
                    .closed_data_definition(reference)
                    .ok_or_else(|| unserved("literal without its closed record application"))?;
                if self
                    .typed
                    .closed_data_constructor_definition(reference, literal.type_symbol)
                    != Some(selected)
                {
                    return Err(typing("literal application changed its declared owner"));
                }
                self.record_at(id, reference)?
            }
            None => self.record(id, literal.type_symbol)?,
        };
        // A literal constructs the bare carrier: "Constructing a fresh valid
        // carrier precedes establishing its qualification" (domains.md).
        let ValueType::Record(record, Qualification::NONE, written) = result else {
            return Err(typing("record literal of a scalar or a qualified type"));
        };
        // A literal is a fresh value, so the regions it is at are the ones
        // the place it stands for asks (an argument, an initializer, a
        // field), whatever lifetime spelling typing attached to its type: a
        // literal passed to `from_source<'storage>(source: Source<'storage>)`
        // carries that callee's `'storage`, which names no region the caller
        // holds. What it holds is then judged against its field types at
        // those regions, so a view of another region is still refused. With
        // nothing expected it is at the regions its own type names.
        let regions = match expected {
            Some(ValueType::Record(row, Qualification::NONE, regions)) if row == record => regions,
            _ => written,
        };
        let result = with_regions(result, regions);
        for reference in applications {
            if reference.is_valid() {
                self.record_application(id, record, reference)?;
            }
        }
        // Stage 03 also records the literal's type as a type reference; it
        // must be this same record, or the literal applies a generic.
        if literal.application_type.is_valid()
            && self.value_type(id, literal.application_type)?.erased() != result.erased()
        {
            return Err(unserved("record literal of an applied type"));
        }
        let case = match case_symbol {
            Some(symbol) => Some(self.case_position(id, record, symbol)?),
            None => None,
        };
        let substitution = self.field_regions(id, record, regions)?;
        let data = self.data_type(id, record)?;
        let declared: Vec<ValueType> = match case {
            Some(case) => data
                .cases()
                .get(case)
                .ok_or_else(|| typing("case position past the sum's declaration"))?
                .fields
                .iter()
                .map(|field| field.value_type)
                .collect(),
            None if data.cases().is_empty() => {
                data.fields().iter().map(|field| field.value_type).collect()
            }
            None => return Err(typing("literal of a sum without a case")),
        };
        let declared: Vec<ValueType> = declared
            .into_iter()
            .map(|field| self.substitute(field, &substitution))
            .collect();
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
            let operand = self.expression(id, slots, field.value, Some(field_type))?;
            if !self.agrees(operand.result, field_type) {
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
        let is_case = |symbol: SymbolHandle| {
            symbol.is_valid() && self.typed.symbols.get(symbol).kind == SymbolKind::Variant
        };
        let symbol = match self.typed.expression_table.expression(handle) {
            // A path names the declaration its last segment resolves to
            // (`symbol`). `head_symbol` is its first segment's: the sum in
            // `Packet::Alpha`, which is how a data `requires` holds the case
            // of `self in Packet::Alpha`. The head is asked only when the
            // last segment's symbol is no case, for a path resolved by its
            // head alone.
            ExpressionNode::Name(path) if !path.static_application.is_valid() => {
                if is_case(path.symbol) {
                    path.symbol
                } else {
                    path.head_symbol
                }
            }
            ExpressionNode::Name(_) => return None,
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
        is_case(symbol).then_some(symbol)
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
        let Some(record) = self.opened(id, subject.result, Access::Read)? else {
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
            result: Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
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
    /// deleted checker enforced that law in validation, after the typed
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
    application: Option<&typed_trees::SourceTypeApplication>,
    members: &'m [DataMember],
    payload: impl Fn(&'m typed_trees::data::DataVariant) -> &'m [DataField],
) -> Checked<()> {
    let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return unserved("data without a checked shape");
    }
    // Lifetime parameters are not generic data: they are erased regions that
    // select no type, field or layout (`lifetime_binders`).
    if !data.type_parameters.is_empty()
        || data.generic_instance.is_some() && application.is_none()
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
    // Data with no member is a record whose roster is empty (`data Token
    // {}`): a value of it is established, copied or moved, passed and
    // dropped like any other record, and holds nothing to read.
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

/// `value_type` at `regions`, when it is a record type; a record row is the
/// same whatever regions a value of it holds (`regions`).
pub(super) fn with_regions(value_type: ValueType, regions: crate::body::Regions) -> ValueType {
    match value_type {
        ValueType::Record(record, qualification, _) => {
            ValueType::Record(record, qualification, regions)
        }
        other => other,
    }
}

/// The lifetime binders of a data declaration, `data Message<'buffer>`.
///
/// A lifetime parameter selects nothing a value is made of.
/// `wiki/spec/language/lifetimes.md`: "Lifetime arguments are erased regions,
/// separate from runtime generic arity, layout and monomorphization." So a
/// declaration with lifetime parameters is one record row, whatever
/// lifetimes a use applies it at, and is read like plain data. The same
/// sentence goes on: "Erasure does not remove semantic lifetime relations or
/// loan obligations." Those are kept in the value type instead: the regions
/// a value is at (`regions`), which its stored views' types are read at.
///
/// What is checked is the declaration itself: "Duplicate binders and
/// undeclared tags reject." The typed trees name a lifetime parameter by its
/// declaration and position, found from its tag
/// (`typed_trees::type_identity::lifetime_parameter_identity`), so two
/// binders with one tag would be one parameter to every lookup. A use that
/// names an undeclared tag is refused where its type is read
/// (`lifetime_application`).
pub(super) fn lifetime_binders(id: MachineId, binders: &[Identifier]) -> Checked<()> {
    for (position, binder) in binders.iter().enumerate() {
        if binder.as_str().is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("lifetime parameter without a tag"),
            ));
        }
        if binders[..position]
            .iter()
            .any(|earlier| earlier.as_str() == binder.as_str())
        {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("declaration that states a lifetime twice"),
            ));
        }
    }
    Ok(())
}

/// What the typed trees retained for one lifetime argument of a type
/// occurrence (`TypeReferenceTable::operand_lifetimes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RetainedLifetime<'t> {
    /// Nothing resolved: the argument is looked up in its owner.
    Open,
    /// The lifetime parameter this argument was resolved to, by the tag
    /// that parameter is declared with; `None` when the declaration it
    /// names no longer states such a parameter.
    Resolved(Option<&'t str>),
}

/// `Inner<'a>` where a type is read: the lifetime arguments a named data
/// application supplies, against the number of binders that data declares
/// and the owner of the lifetimes the application names. lifetimes.md:
/// "Named data applications supply lifetime arguments before
/// type/const/machine arguments. They must match the declaration's lifetime
/// arity and use binders declared in the lexical owner."
///
/// The owner is the typed trees' answer, not a scope rebuilt here:
/// `retained` is what they resolved for each argument of this occurrence,
/// empty when they resolved none, and `declares` is whether the occurrence's
/// owner (the declaration the typed trees retained for it, else the data
/// declaration whose member is being read) declares a tag. A literal's type
/// is such a retained occurrence: stage 03 gives `Inner { .. }` inside
/// `Outer { inner: .. }` the field's declared type `Inner<'a>` and retains
/// `Outer` as the owner of `'a`. The judgment is the one the current
/// checker makes in validation (`contains_operand_lifetime`), which is
/// being removed, so it is made here.
pub(super) fn lifetime_application(
    id: MachineId,
    declared: usize,
    arguments: &[Identifier],
    retained: &[RetainedLifetime<'_>],
    declares: impl Fn(&str) -> bool,
) -> Checked<()> {
    let typing = |what| Err(refuse(id, Construct::Machine, Unserved::Typing(what)));
    if arguments.len() != declared {
        return typing("lifetime argument count differs from the data declaration's");
    }
    if !retained.is_empty() && retained.len() != arguments.len() {
        return typing("lifetime argument its owner does not declare");
    }
    for (slot, argument) in arguments.iter().enumerate() {
        let tag = argument.as_str();
        let declared = !tag.is_empty()
            && match retained.get(slot) {
                Some(RetainedLifetime::Resolved(parameter)) => *parameter == Some(tag),
                Some(RetainedLifetime::Open) | None => declares(tag),
            };
        if !declared {
            return typing("lifetime argument its owner does not declare");
        }
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
        // A policy changes no payload: the integer copies as its carrier does.
        ValueType::Scalar(..) => true,
        ValueType::Record(record, _, _) => {
            records.get(record).is_some_and(|record| record.copyable)
        }
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
            shape: crate::StorageShape::Record(Vec::new()),
            requires: Vec::new(),
        });
        assert!(!copy_field_is_unrestricted(
            ValueType::record(owned),
            &records
        ));
        records.get_mut(owned).unwrap().copyable = true;
        assert!(copy_field_is_unrestricted(
            ValueType::record(owned),
            &records
        ));
        assert!(!copy_field_is_unrestricted(
            ValueType::record(StructuralTypeId::new(2).unwrap()),
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
        let refusal = record_shape(machine(), data, None, members, |_| &[]).unwrap_err();
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
        assert_eq!(
            record_shape(machine(), &data, None, &members, |_| &[]),
            Ok(())
        );
        let mut affine = data.clone();
        affine.properties.multiplicity = language_semantics::Multiplicity::Affine;
        assert_eq!(
            record_shape(machine(), &affine, None, &members, |_| &[]),
            Ok(())
        );
        // Lifetime parameters select nothing a value is made of, so the
        // shape is served with them; a type parameter beside them is not.
        let mut borrowing = data.clone();
        borrowing.lifetime_parameters = vec![Identifier::generated("buffer")];
        assert_eq!(
            record_shape(machine(), &borrowing, None, &members, |_| &[]),
            Ok(())
        );
        borrowing.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
        assert_eq!(detail(&borrowing, &members), "type generic data");
        // A declaration-level `requires` is no longer refused here: its
        // clauses are read by the clause reader. A case-level one still is.
        let cases: [(&str, fn(&mut DataDefinition, &mut Vec<DataMember>)); 7] = [
            ("type data with custody", |data, _| {
                data.properties.multiplicity = language_semantics::Multiplicity::Linear;
            }),
            ("type generic data", |data, _| {
                data.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
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
            ("type data without a checked shape", |data, _| {
                data.supply_mode = language_semantics::DataSupplyMode::BoundaryOpaque;
            }),
        ];
        for (expected, change) in cases {
            let (mut data, mut members) = plain();
            change(&mut data, &mut members);
            assert_eq!(detail(&data, &members), expected);
        }
        // Data with no member at all is a record with an empty roster.
        let (data, _) = plain();
        assert_eq!(record_shape(machine(), &data, None, &[], |_| &[]), Ok(()));
        // A sum alone, with or without payload, is served.
        let sum = vec![
            DataMember::Variant(DataVariant::default()),
            DataMember::Variant(DataVariant::default()),
        ];
        assert_eq!(record_shape(machine(), &data, None, &sum, |_| &[]), Ok(()));
        let erased = [DataField {
            relevance: language_core::BindingRelevance::Erased,
            ..DataField::default()
        }];
        assert_eq!(
            record_shape(machine(), &data, None, &sum, |_| &erased)
                .unwrap_err()
                .detail,
            "type data with an erased field"
        );
    }

    fn tags(tags: &[&str]) -> Vec<Identifier> {
        tags.iter().copied().map(Identifier::generated).collect()
    }

    /// `data Pair<'a, 'b>` is served; a binder repeated, or one the typed
    /// trees hold without a tag, is refused. lifetimes.md: "Duplicate
    /// binders and undeclared tags reject."
    #[test]
    fn a_data_declaration_states_each_lifetime_binder_once() {
        assert_eq!(lifetime_binders(machine(), &[]), Ok(()));
        assert_eq!(lifetime_binders(machine(), &tags(&["a", "b"])), Ok(()));
        assert_eq!(
            lifetime_binders(machine(), &tags(&["a", "b", "a"]))
                .unwrap_err()
                .detail,
            "typing declaration that states a lifetime twice"
        );
        assert_eq!(
            lifetime_binders(machine(), &[Identifier::default()])
                .unwrap_err()
                .detail,
            "spelling lifetime parameter without a tag"
        );
    }

    /// `Inner<'a>` is served where the owner of its occurrence declares
    /// `'a` and `Inner` declares one lifetime. Too few or too many
    /// arguments, and an argument the owner does not declare, are refused;
    /// a machine is read under no owner, so there a lifetime the typed trees
    /// did not resolve is undeclared. A lifetime they did resolve must be
    /// the parameter the argument names, whatever the owner declares.
    #[test]
    fn a_lifetime_application_matches_its_declaration_and_its_owner() {
        use RetainedLifetime::{Open, Resolved};
        let detail = |declared: usize,
                      arguments: &[&str],
                      retained: &[RetainedLifetime<'_>],
                      owner: &[&str]| {
            lifetime_application(machine(), declared, &tags(arguments), retained, |tag| {
                owner.contains(&tag)
            })
            .map_err(|refusal| refusal.detail)
        };
        assert_eq!(detail(1, &["a"], &[], &["a"]), Ok(()));
        assert_eq!(detail(1, &["b"], &[], &["a", "b"]), Ok(()));
        assert_eq!(detail(2, &["a", "a"], &[Open, Open], &["a"]), Ok(()));
        assert_eq!(detail(0, &[], &[], &[]), Ok(()));
        let arity =
            Err("typing lifetime argument count differs from the data declaration's".into());
        assert_eq!(detail(1, &[], &[], &["a"]), arity);
        assert_eq!(detail(1, &["a", "a"], &[], &["a"]), arity);
        assert_eq!(detail(0, &["a"], &[], &["a"]), arity);
        let undeclared = Err("typing lifetime argument its owner does not declare".into());
        assert_eq!(detail(1, &["b"], &[], &["a"]), undeclared);
        assert_eq!(detail(1, &["a"], &[], &[]), undeclared);
        assert_eq!(detail(1, &[""], &[], &[""]), undeclared);
        // What the typed trees resolved decides, not the owner's tags.
        assert_eq!(detail(1, &["a"], &[Resolved(Some("a"))], &[]), Ok(()));
        assert_eq!(
            detail(1, &["a"], &[Resolved(Some("b"))], &["a"]),
            undeclared
        );
        assert_eq!(detail(1, &["a"], &[Resolved(None)], &["a"]), undeclared);
        assert_eq!(detail(2, &["a", "a"], &[Open], &["a"]), undeclared);
    }
}
