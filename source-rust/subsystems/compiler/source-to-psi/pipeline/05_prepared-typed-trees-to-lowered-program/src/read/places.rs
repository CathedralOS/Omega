use crate::read::refuse_at;
/*
Read data declarations, literals, field paths, case tests and complete record
patterns. Resolved symbols identify each type, field and case; a spelling alone
cannot select one.

Values retain their declared copy, affine or linear ownership, selected cleanup,
references and lifetime arguments. Field types substitute the actual lifetime
arguments of their containing value. A complete owned pattern ends its record
wrapper and leaves its components subject to their own ownership and cleanup.
Ending a wrapper with selected cleanup requires the declaring package's authority
or the exact selected consumer.

Data predicates use the same clause reader as machine contracts. Field stores
are read in `statements`, and case dispatch is read in `transitions`.
*/

use crate::body::{Qualification, StorageShape};
use crate::source_state::data::{DataDefinition, DataField, DataMember};
use crate::source_state::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, TableDestructurePattern,
    TableMemberExpression, TableStructLiteral,
};
use crate::source_state::name::Identifier;
use crate::source_state::types::TypeReferenceNode;
use semantic_vocabulary::{
    MachineId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};

use super::authority::{Access, Opened};
use super::contracts::{ClauseScope, DataSubject, ResultTerm};
use super::{Program, Slot};
use crate::body::{
    CaseSelector, CaseType, Expression, ExpressionKind, RecordField, RecordType, ValueType,
};
use crate::canonical::{self, Repeated};
use crate::{SourceOrigin, Unserved};

/// Different checking machines own different const-input storage rows.
/// Their labels must be unique; type equality still uses source applications.
/// The refusal of a structural type identity that holds no record row.
fn missing_record(machine: MachineId) -> terminal_check_tables::Refusal {
    refuse_at(
        machine,
        Unserved::Type("structural type identity has no record row"),
    )
}

pub(super) fn storage_identity_for_inputs(identity: String, owner: Option<MachineId>) -> String {
    match owner {
        Some(owner) => format!("{identity} (checking machine {})", owner.get()),
        None => identity,
    }
}

/// The symbols a data type was resolved from: its declaration, each field
/// and each case with its payload fields, in declaration order, beside the
/// `RecordType` row of the same identity. Only the reader holds symbols; the
/// body holds positions.
#[cfg_attr(test, derive(Default))]
pub(super) struct RecordSource {
    /// Symbolic array lengths name inputs of this checking machine. Closed
    /// storage has no such owner and may be shared across machine readings.
    pub(super) constant_input_owner: Option<MachineId>,
    pub(super) declaration: SymbolHandle,
    pub(super) application: Option<crate::source_state::SourceTypeApplication>,
    pub(super) cleanup: Option<crate::source_state::domain::CleanupSelection>,
    /// The declaration whose lifetime binders a byte view's region is one
    /// of, when the typed trees retained no owner for that occurrence: the
    /// data declaration the view is a field of. Invalid everywhere else.
    pub(super) region_owner: SymbolHandle,
    pub(super) fields: Vec<SymbolHandle>,
    pub(super) cases: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
    /// The symbol of each `[erased]` field, in `RecordType::erased` order.
    pub(super) erased: Vec<SymbolHandle>,
}

impl RecordSource {
    /// Every field symbol of a record, its erased fields included: what a
    /// record pattern accounts for.
    fn record_members(&self) -> Vec<SymbolHandle> {
        self.fields.iter().chain(&self.erased).copied().collect()
    }
}

/// What `declared_members` reads: the fields and their symbols, then the cases
/// (identity and payload fields) and their symbols with their payloads', then
/// the erased fields, which have no storage and are read apart.
type Members = (
    Vec<(String, ValueType)>,
    Vec<SymbolHandle>,
    Vec<(String, Vec<(String, ValueType)>)>,
    Vec<(SymbolHandle, Vec<SymbolHandle>)>,
    Vec<DeclaredErased>,
);

/// One `[erased]` field as `declared_members` reads it: its identity, the
/// text of its type's identity, its case, its position among the authored
/// members of its record or payload, and its symbol.
pub(super) struct DeclaredErased {
    identity: String,
    type_identity: String,
    value_type: ValueType,
    evidence: bool,
    case: Option<usize>,
    position: usize,
    symbol: SymbolHandle,
}

/// What a member's field symbol names in its receiver's data type.
pub(super) enum Member {
    Field(usize),
    Payload { case: usize, field: usize },
}

impl Program<'_> {
    /// A structural type and its source share the identity used for field and case lookups.
    pub(super) fn push_record_row(
        &mut self,
        record: RecordType,
        source: RecordSource,
    ) -> StructuralTypeId {
        let id = self.records.push(record);
        self.record_sources.push(source);
        id
    }

    /// The storage row at `record`, with `machine` anchoring a missing-row refusal.
    pub(super) fn record_type(
        &self,
        machine: MachineId,
        record: StructuralTypeId,
    ) -> Checked<&crate::body::RecordType> {
        self.records
            .get(record)
            .ok_or_else(|| missing_record(machine))
    }

    /// The source declaration and application pushed beside `record`.
    pub(super) fn record_source(
        &self,
        machine: MachineId,
        record: StructuralTypeId,
    ) -> Checked<&RecordSource> {
        self.record_sources
            .get(record)
            .ok_or_else(|| missing_record(machine))
    }

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
                refuse_at(
                    id,
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
        reference: crate::source_state::types::TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let typed = self.typed;
        let semantic = super::source_records::semantic_reference(typed, reference, id)?;
        // The canonical layout links to a natural semantic schema. Discover
        // that same record first, with its own identity and field namespace.
        if semantic != reference {
            self.record_at(id, semantic)?;
        }
        let data = typed
            .data_application_definition(semantic)
            .or_else(|| typed.closed_data_definition(semantic))
            .ok_or_else(|| {
                refuse_at(
                    id,
                    Unserved::Type("record without its exact retained application"),
                )
            })?;
        // A quotient application is a declaration of its own, its carrier,
        // relation and conformance closed at its arguments (`quotients`).
        if data.quotient.is_some() && data.generic_instance.is_some() {
            return self.record_definition(id, data, None);
        }
        let application = self.source_record_application(data, reference, id)?;
        let regions = self.occurrence_regions(id, data, semantic)?;
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
        reference: crate::source_state::types::TypeReferenceHandle,
    ) -> Checked<crate::body::Regions> {
        use crate::source_state::type_identity::lifetime_parameter_identity;
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
                    let parameter = lifetime_parameter_identity(typed, owner, argument.as_str());
                    // An authored tag its owner has no binder for is the
                    // program's error, as for a reference
                    // (`reference_lifetime_declared`).
                    if parameter.is_none() && !argument.as_str().is_empty() {
                        return Err(crate::reject(
                            id,
                            SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                            undeclared_lifetime_rule(argument.as_str()),
                        ));
                    }
                    parameter
                }
                _ => None,
            };
            let parameter = parameter.ok_or_else(|| {
                refuse_at(
                    id,
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

    /// Cache the input context with the shape that uses it. A child was formed
    /// before its parent, so its saved owner avoids walking the whole graph
    /// each time a nominal type is read. This metadata grants no access.
    pub(super) fn storage_constant_input_owner(
        &self,
        machine: MachineId,
        shape: &StorageShape,
    ) -> Checked<Option<MachineId>> {
        let mut owner = match shape {
            StorageShape::FixedArray {
                length: terminal_psi::ArrayExtent::Parameter { owner, .. },
                ..
            } => Some(*owner),
            _ => None,
        };
        let mut include = |child| -> Checked<()> {
            let child_owner = self.source(machine, child)?.constant_input_owner;
            match (owner, child_owner) {
                (Some(left), Some(right)) if left != right => Err(refuse_at(
                    machine,
                    Unserved::Type("storage combines constants from different checking machines"),
                )),
                (None, child) => {
                    owner = child;
                    Ok(())
                }
                _ => Ok(()),
            }
        };
        match shape {
            StorageShape::Reference { referent, .. } => include(*referent)?,
            StorageShape::ElementView { element, .. }
            | StorageShape::FixedArray { element, .. } => include(*element)?,
            StorageShape::Record(fields) => {
                for field in fields {
                    if let ValueType::Record(child, ..) = field.value_type {
                        include(child)?;
                    }
                }
            }
            StorageShape::Sum(cases) => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let ValueType::Record(child, ..) = field.value_type {
                        include(child)?;
                    }
                }
            }
            StorageShape::Opaque
            | StorageShape::TypeParameter(_)
            | StorageShape::ByteSequence(_)
            | StorageShape::AtomicCell(_)
            | StorageShape::PrimitiveScalar { .. } => {}
        }
        Ok(owner)
    }

    fn record_definition(
        &mut self,
        id: MachineId,
        data: &DataDefinition,
        application: Option<crate::source_state::SourceTypeApplication>,
    ) -> Checked<ValueType> {
        let typed = self.typed;
        let symbol = data.symbol;
        self.served(id, symbol)?;
        let same =
            |owner: SymbolHandle, retained: &Option<crate::source_state::SourceTypeApplication>| {
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
                            // An argument-free declaration is its natural
                            // occurrence, never another selected storage form.
                            && typed.normalized_nominal_type_identity(symbol).is_some_and(|identity| {
                                identity == typed.normalized_type_identity(application.reference)
                            })
                        }
                    }
            };
        // A quotient of mathematical data is a type of the mathematical
        // signature, with no members to read (`quotients`).
        if self.mathematical && data.quotient.is_some() && self.is_mathematical_data(data) {
            return self.quotient_record(id, symbol, data);
        }
        if data.quotient.is_some() {
            return self.executable_quotient_record(id, symbol, data);
        }
        // Data indexed by values is one type of the signature at every
        // application (`families`).
        if self.mathematical && self.is_family(data) {
            return self.family_record(id, data);
        }
        let members = typed.data_members(data);
        // Admission precedes cache reuse: descriptor-only records cannot
        // become executable body types through a shared storage row.
        record_shape_for(id, data, application.as_ref(), self.type_purpose, members).map_err(
            |mut refusal| {
                // The caller may not yet have a machine body to locate this
                // refusal. Keep the declaration whose shape was actually read.
                if refusal.source_span.is_none() {
                    refusal.source_span = typed
                        .symbols
                        .symbol_provenance_source_span(symbol)
                        .filter(|span| *span != source_text::SourceSpan::default());
                }
                refusal
            },
        )?;
        let known = self.record_sources.iter().find_map(|(record, source)| {
            (source.constant_input_owner.is_none_or(|owner| owner == id)
                && same(source.declaration, &source.application))
            .then_some(record)
        });
        if let Some(record) = known {
            // A field of data that contains itself names the same
            // mathematical type; there is no storage to admit again.
            if self.self_containing.binary_search(&record).is_ok() {
                return Ok(ValueType::record(record));
            }
            // A description cannot authorize a later value use, including
            // use of a plain outer record containing an address or linear child.
            if self.type_purpose == super::storage_metadata::TypePurpose::Value
                && self.value_records.binary_search(&record).is_err()
            {
                if self
                    .resolving
                    .iter()
                    .any(|(owner, retained)| same(*owner, retained))
                {
                    return Err(refuse_at(
                        id,
                        Unserved::Type("data application that contains itself"),
                    ));
                }
                record_shape_for(
                    id,
                    data,
                    application.as_ref(),
                    self.type_purpose,
                    typed.data_members(data),
                )?;
                self.resolving.push((symbol, application.clone()));
                let enclosing = std::mem::replace(&mut self.lifetime_owner, symbol);
                let read = (|| {
                    if let Some(application) = &application {
                        for (_, argument) in &application.substitutions {
                            self.value_type(id, *argument)?;
                        }
                    }
                    self.declared_members(id, typed.data_members(data))
                })();
                self.lifetime_owner = enclosing;
                self.resolving.pop();
                read?;
                if let Err(position) = self.value_records.binary_search(&record) {
                    self.value_records.insert(position, record);
                }
            }
            if let Some(application) = &application {
                self.record_application(id, record, application.reference)?;
            }
            return Ok(ValueType::record(record));
        }
        let unserved = |what| refuse_at(id, Unserved::Type(what));
        if self
            .resolving
            .iter()
            .any(|(owner, retained)| same(*owner, retained))
        {
            return Err(unserved("data application that contains itself"));
        }
        lifetime_binders(id, &data.lifetime_parameters, self.declared_at(data.symbol))?;
        // Terminal Psi's declaration needs a text identity; the typed trees
        // render it. Without one the declaration cannot be emitted.
        let identity = match &application {
            Some(application) => Some(application.type_identity(typed)),
            None => typed.normalized_nominal_type_identity(symbol),
        }
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| unserved("data without a complete application identity"))?;
        // Data that contains itself is a mathematical type. Its record is
        // reserved before the members are read, so the field that names the
        // declaration resolves to it.
        let reserved = if self.mathematical && self.contains_itself(data) {
            // An application of generic data (`Seq<u64>`) is the generic
            // declaration at its type arguments: the signature declares the
            // template once and this application in terms of it
            // (`signature::generic_data`). Only type arguments are read so.
            if application.as_ref().is_some_and(|application| {
                !application.substitutions.is_empty() && !self.type_arguments_only(data)
            }) || !data.type_parameters.is_empty()
                || !data.requires_facts.is_empty()
                || data.cleanup_selection.is_some()
                || data.supply_mode == language_core::DataSupplyMode::BoundaryOpaque
            {
                return Err(unserved(
                    "applied, constrained or boundary data that contains itself",
                ));
            }
            let record_type = RecordType {
                multiplicity: crate::body::source_multiplicity(data.properties.multiplicity),
                cleanup: None,
                identity: identity.clone(),
                shape: crate::StorageShape::Sum(Vec::new()),
                requires: Vec::new(),
                authored_layout: false,
                erased: Vec::new(),
            };
            let source = RecordSource {
                constant_input_owner: None,
                declaration: symbol,
                cleanup: None,
                application: application.clone(),
                region_owner: SymbolHandle::invalid(),
                fields: Vec::new(),
                cases: Vec::new(),
                erased: Vec::new(),
            };
            let record = self.push_record_row(record_type, source);
            if let Err(position) = self.self_containing.binary_search(&record) {
                self.self_containing.insert(position, record);
            }
            Some(record)
        } else {
            None
        };
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
                let template = typed
                    .unique_data_definition_by_symbol(symbol)
                    .ok_or_else(|| unserved("applied data lost its template"))?;
                for (parameter, argument) in typed
                    .data_type_parameters(template)
                    .iter()
                    .zip(&application.substitutions)
                {
                    if matches!(
                        parameter.kind,
                        crate::source_state::data::TypeParameterKind::Type
                    ) {
                        self.value_type(id, argument.1)?;
                    }
                    // source_application checks each const against its carrier;
                    // a const is not itself a runtime value type to read here.
                }
            }
            // The supplied types are admitted above; the members are read as
            // fields of this declaration, which state their lifetimes.
            self.elision = super::regions::Elision::Members;
            self.declared_members(id, members)
        })();
        self.lifetime_owner = enclosing;
        self.elision = elision;
        self.resolving.pop();
        let members = read.map_err(|mut refusal| {
            refusal.detail.push_str(&format!(
                " while reading the record application `{identity}`"
            ));
            refusal
        })?;
        let (fields, field_symbols, cases, case_symbols, erased) = members;
        // Field identities are minted in authored order, erased fields
        // among the rest, since Terminal Psi orders a declaration's field
        // rows by them. `erased` is in that order already: the record's
        // fields, then each case's payload.
        let mut erased = erased.into_iter().peekable();
        let mut erased_fields = Vec::new();
        let mut erased_symbols = Vec::new();
        let mut mint = |fields: Vec<(String, ValueType)>, case: Option<usize>| {
            let mut declared = Vec::with_capacity(fields.len());
            let mut fields = fields.into_iter();
            for position in 0.. {
                self.fields_minted += 1;
                let field = StructuralFieldId::new(self.fields_minted)
                    .ok_or_else(|| unserved("field identity overflow"))?;
                if let Some(next) =
                    erased.next_if(|next| next.case == case && next.position == position)
                {
                    erased_fields.push(crate::body::ErasedField {
                        id: field,
                        identity: next.identity,
                        type_identity: next.type_identity,
                        value_type: next.value_type,
                        evidence: next.evidence,
                        case: next.case,
                        position: next.position,
                    });
                    erased_symbols.push(next.symbol);
                    continue;
                }
                let Some((identity, value_type)) = fields.next() else {
                    self.fields_minted -= 1;
                    break;
                };
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
        let declared = mint(fields, None)?;
        let mut declared_cases = Vec::with_capacity(cases.len());
        for (position, (identity, payload)) in cases.into_iter().enumerate() {
            declared_cases.push((identity, mint(payload, Some(position))?));
        }
        if erased.next().is_some() {
            return Err(unserved("erased field outside its record's authored order"));
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
        if let Some(record) = reserved {
            if !declared.is_empty() || case_types.is_empty() {
                return Err(unserved("data that contains itself outside a case"));
            }
            if !erased_fields.is_empty() {
                return Err(unserved("data that contains itself with an erased field"));
            }
            self.records
                .get_mut(record)
                .ok_or_else(|| unserved("record identity"))?
                .shape = crate::StorageShape::Sum(case_types);
            let source = self
                .record_sources
                .get_mut(record)
                .ok_or_else(|| unserved("record identity"))?;
            source.fields = field_symbols;
            source.cases = case_symbols;
            self.admitted_declarations.include(symbol);
            if let Err(position) = self.value_records.binary_search(&record) {
                self.value_records.insert(position, record);
            }
            return Ok(ValueType::record(record));
        }
        let copyable = data.properties.multiplicity == language_core::Multiplicity::Unrestricted;
        if copyable
            && !declared
                .iter()
                .chain(case_types.iter().flat_map(|case| &case.fields))
                .all(|field| copy_field_is_unrestricted(field.value_type, &self.records))
        {
            return Err(unserved("copy data with an owned field"));
        }
        let shape = if data.supply_mode == language_core::DataSupplyMode::BoundaryOpaque {
            crate::StorageShape::Opaque
        } else if case_types.is_empty() {
            crate::StorageShape::Record(declared)
        } else {
            crate::StorageShape::Sum(case_types)
        };
        // An argument that is this reading's own `const` or value input
        // (`Index<Limit>` while `Limit` is rigid) makes the requires name that
        // input, so the record belongs to this checking machine like an array
        // sized by one.
        let (bound, bound_owner) = self.application_values(id, symbol, application.as_ref())?;
        let storage_owner = self.storage_constant_input_owner(id, &shape)?;
        let constant_input_owner = match (storage_owner, bound_owner) {
            (Some(left), Some(right)) if left != right => {
                return Err(unserved(
                    "storage combines constants from different checking machines",
                ));
            }
            (left, right) => left.or(right),
        };
        let identity = storage_identity_for_inputs(identity, constant_input_owner);
        let record_type = RecordType {
            multiplicity: crate::body::source_multiplicity(data.properties.multiplicity),
            cleanup: None,
            identity,
            shape,
            requires: Vec::new(),
            authored_layout: application.as_ref().is_some_and(|application| {
                typed
                    .layout_storage_arguments(application.reference)
                    .is_some()
            }),
            erased: erased_fields,
        };
        let source = RecordSource {
            constant_input_owner,
            declaration: symbol,
            cleanup: data.cleanup_selection,
            application: application.clone(),
            region_owner: SymbolHandle::invalid(),
            fields: field_symbols,
            cases: case_symbols,
            erased: erased_symbols,
        };
        let record = self.push_record_row(record_type, source);
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
            bound: &bound,
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in a data clause"),
            formal_fields: false,
            logical: None,
            subject: Some(DataSubject {
                value_type: ValueType::record(record),
                symbol: data.predicate_subject_symbol,
                root,
                path: &[],
                bare_fields: true,
            }),
        };
        let facts = super::span(
            |span| typed.proof_facts.span(span),
            data.requires_facts,
            id,
            "data requires span",
        )?;
        // The `requires` name the template's binders. Under an application
        // each stands for its argument, so `T == i32` in `Value<T>` read as
        // `Value<i32>` compares `i32` with `i32`. The arguments are written
        // in the application's own source frame.
        let clause_frame = application.as_ref().map(|application| {
            std::sync::Arc::new(crate::source_state::SourceTypeScope {
                parent: Some(application.scope.clone()),
                substitutions: application.substitutions.clone(),
                rigid_parameters: Vec::new(),
            })
        });
        let outer_frame =
            clause_frame.map(|frame| std::mem::replace(&mut self.source_scope, frame));
        let read = facts
            .iter()
            .map(|fact| {
                let fact = self.fact(id, fact)?;
                self.clause(id, &scope, fact)
            })
            .collect::<Checked<Vec<_>>>();
        if let Some(outer) = outer_frame {
            self.source_scope = outer;
        }
        let mut requires = read?;
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
        let cases: Vec<(StructuralCaseId, Vec<(StructuralFieldId, ValueType)>)> = declared
            .cases()
            .iter()
            .map(|case| {
                (
                    case.id,
                    case.fields
                        .iter()
                        .map(|field| (field.id, field.value_type))
                        .collect(),
                )
            })
            .collect();
        for (field, value_type) in fields {
            let path = [semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                field,
            )];
            requires.extend(self.field_predicates(id, &path, value_type)?);
        }
        // A payload's domain holds only of a value in its case
        // (data_and_literals.md: "case-dependent projections require
        // established case knowledge"). It is owed as the clause an author
        // would write in the declaration's `requires`: the value is in that
        // case and its payload meets the predicates, or it is in another
        // case. Construction proves it and a case test assumes it, like any
        // other data clause.
        for (position, (case, fields)) in cases.iter().enumerate() {
            let mut owed = Vec::new();
            for &(field, value_type) in fields {
                let path = [
                    semantic_vocabulary::CanonicalStructuralPathSegment::Case(*case),
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(field),
                ];
                owed.extend(self.field_predicates(id, &path, value_type)?);
            }
            if owed.is_empty() {
                continue;
            }
            let in_case = |case| semantic_vocabulary::Proposition::StructuralCaseMembership {
                subject: semantic_vocabulary::StructuralCaseSubject::new(root, Vec::new()),
                case,
            };
            owed.push(in_case(*case));
            let mut disjuncts = vec![semantic_vocabulary::Proposition::Conjunction(
                canonical::clauses(owed, Repeated::Refuse, id)?,
            )];
            disjuncts.extend(
                cases
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != position)
                    .map(|(_, (other, _))| in_case(*other)),
            );
            requires.push(match disjuncts.len() {
                1 => disjuncts.remove(0),
                _ => semantic_vocabulary::Proposition::Disjunction(canonical::clauses(
                    disjuncts,
                    Repeated::Refuse,
                    id,
                )?),
            });
        }
        self.records
            .get_mut(record)
            .ok_or_else(|| unserved("record identity"))?
            .requires = requires;
        self.admitted_declarations.include(symbol);
        if self.type_purpose == super::storage_metadata::TypePurpose::Value
            && let Err(position) = self.value_records.binary_search(&record)
        {
            self.value_records.insert(position, record);
        }
        if let Some(selection) = data.cleanup_selection {
            if data.generic_instance.is_some() {
                return Err(unserved("nominal cleanup on an applied data type"));
            }
            let executable = self.ordinary_application_target(id, selection.application)?;
            self.pending_cleanup.push((record, executable));
        }
        Ok(ValueType::record(record))
    }

    /// A selected consumer is an ordinary discovered callee. Drain new types
    /// once, after signatures have their identities, including types that a
    /// selected consumer's own signature discovers.
    pub(crate) fn finish_cleanup_selections(&mut self) -> Checked<()> {
        while let Some((record, executable)) = self.pending_cleanup.pop() {
            let diagnostic = self.next_identity()?;
            let definition = self.required_machine(diagnostic, executable, "selected cleanup")?;
            if crate::is_generic_template(self.typed, definition) {
                return Err(refuse_at(
                    diagnostic,
                    Unserved::Declaration("selected cleanup needs a closed ordinary application"),
                ));
            }
            let machine = self.machine(executable)?;
            let declaration = self
                .records
                .get_mut(record)
                .ok_or_else(|| refuse_at(diagnostic, Unserved::Type("cleanup carrier identity")))?;
            declaration.cleanup = Some(crate::body::RecordCleanup {
                declaration_identity: declaration.identity.clone(),
                machine,
            });
        }
        Ok(())
    }

    /// The fields and cases of a data declaration, each with its symbols,
    /// in declaration order. Fields and cases are read first and identities
    /// minted after, so a nested data type resolves, and takes its
    /// identities, before its owner and each type's fields are consecutive.
    fn declared_members(
        &mut self,
        id: MachineId,
        members: arena::RootRows<'_, DataMember>,
    ) -> Checked<Members> {
        let typed = self.typed;
        let mut fields = Vec::new();
        let mut field_symbols = Vec::new();
        let mut cases = Vec::new();
        let mut case_symbols = Vec::new();
        let mut erased = Vec::new();
        let mut field_position = 0;
        for member in members {
            match member {
                DataMember::Field(field) if is_erased(field) => {
                    erased.push(self.declared_erased(id, field, None, field_position)?);
                    field_position += 1;
                }
                DataMember::Field(field) => {
                    fields.push(self.declared_field(id, field)?);
                    field_symbols.push(field.symbol);
                    field_position += 1;
                }
                DataMember::Variant(variant) => {
                    if !variant.symbol.is_valid() {
                        return Err(refuse_at(id, Unserved::Spelling("case without a symbol")));
                    }
                    let identity = variant.member_identity().spelling();
                    if identity.is_empty() {
                        return Err(refuse_at(
                            id,
                            Unserved::Spelling("case without a declaration identity"),
                        ));
                    }
                    let mut payload = Vec::new();
                    let mut payload_symbols = Vec::new();
                    for (position, field) in typed.data_payload_fields(variant).iter().enumerate() {
                        if is_erased(field) {
                            erased.push(self.declared_erased(
                                id,
                                field,
                                Some(cases.len()),
                                position,
                            )?);
                            continue;
                        }
                        payload.push(self.declared_field(id, field)?);
                        payload_symbols.push(field.symbol);
                    }
                    cases.push((identity, payload));
                    case_symbols.push((variant.symbol, payload_symbols));
                }
            }
        }
        Ok((fields, field_symbols, cases, case_symbols, erased))
    }

    /// One `[erased]` field or payload field. Its type is read as an erased
    /// binding's is (`erased_local`): this slice serves a plain scalar,
    /// whose term the literal supplies; anything else is refused by name.
    fn declared_erased(
        &mut self,
        id: MachineId,
        field: &DataField,
        case: Option<usize>,
        position: usize,
    ) -> Checked<DeclaredErased> {
        let unserved = |what| refuse_at(id, Unserved::Type(what));
        if !field.symbol.is_valid() {
            return Err(refuse_at(id, Unserved::Spelling("field without a symbol")));
        }
        // Evidence of indexed data has no layout and no term here: a
        // literal's evidence and every use of it are checked where the
        // mathematical signature reads evidence (`families::evidence_uses`).
        // Only a record's own field is read there.
        let evidence = self.names_indexed_data(field.type_reference);
        if evidence && case.is_some() {
            return Err(unserved(
                "data with an erased payload field of data indexed by values",
            ));
        }
        let value_type = if evidence {
            super::families::evidence_witness()
        } else {
            self.value_type(id, field.type_reference)?
        };
        if !matches!(value_type, ValueType::Scalar(_, Qualification::NONE)) {
            return Err(unserved(
                "data with an erased field that is not a plain scalar",
            ));
        }
        let identity = field.member_identity().spelling();
        if identity.is_empty() {
            return Err(refuse_at(
                id,
                Unserved::Spelling("field without a declaration identity"),
            ));
        }
        let type_identity = self
            .typed
            .type_identity_text(self.typed.normalized_type_identity(field.type_reference))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unserved("erased field without a type identity"))?;
        Ok(DeclaredErased {
            identity,
            type_identity,
            value_type,
            evidence,
            case,
            position,
            symbol: field.symbol,
        })
    }

    /// One declared field or payload field: its text identity for Terminal
    /// Psi's declaration and its type.
    fn declared_field(&mut self, id: MachineId, field: &DataField) -> Checked<(String, ValueType)> {
        if !field.symbol.is_valid() {
            return Err(refuse_at(id, Unserved::Spelling("field without a symbol")));
        }
        // A field that names lifetime-parameterised data without its
        // lifetime arguments, directly or behind a reference, is refused
        // where its regions are asked for (`regions::Elision::Members`).
        let value_type = self.value_type(id, field.type_reference)?;
        // A field needs storage even when its surrounding signature is
        // logical. Its exact source binder supplies the unknown carrier.
        let value_type = if let ValueType::Rigid(rigid) = value_type {
            let supplied = self
                .source_scope
                .supplied_type_reference(self.typed, field.type_reference)
                .map_err(|reason| refuse_at(id, Unserved::Type(reason)))?;
            self.parameter_storage(id, rigid, supplied.reference)?
        } else {
            value_type
        };
        let identity = field.member_identity().spelling();
        if identity.is_empty() {
            return Err(refuse_at(
                id,
                Unserved::Spelling("field without a declaration identity"),
            ));
        }
        Ok((identity, value_type))
    }

    /*
        The value each `const` or value parameter of an applied declaration
        stands for. `data Index<const Limit: u32> requires value < Limit`
        applied as `Index<5>` owes `value < 5`: its `requires` names the
        template's `Limit`, and the application supplies the argument
        (generics.md, "Value binders and const requirements": "Constructing
        an instance owes the coupling"). A type parameter binds no value.
        An argument may be a binder this reading holds rigid, `Index<N>` in
        `machine f<const N: u32>`; it stands for that machine's input, which
        is returned as the owner of the values.
    */
    fn application_values(
        &self,
        id: MachineId,
        declaration: SymbolHandle,
        application: Option<&crate::source_state::SourceTypeApplication>,
    ) -> Checked<(
        Vec<(SymbolHandle, semantic_vocabulary::ScalarTerm)>,
        Option<MachineId>,
    )> {
        use crate::source_state::data::TypeParameterKind;
        use crate::source_state::types::TypeReferenceNode;
        let Some(application) = application else {
            return Ok((Vec::new(), None));
        };
        let template = self
            .typed
            .unique_data_definition_by_symbol(declaration)
            .ok_or_else(|| refuse_at(id, Unserved::Typing("applied data lost its template")))?;
        let mut bound = Vec::new();
        let mut owner = None;
        for parameter in self.typed.data_type_parameters(template) {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                continue;
            };
            let Some((_, argument)) = application
                .substitutions
                .iter()
                .find(|(binder, _)| *binder == parameter.symbol)
            else {
                return Err(refuse_at(
                    id,
                    Unserved::Typing("applied data without an argument for its value parameter"),
                ));
            };
            let scalar_type = self.scalar_type(id, type_reference)?;
            if let TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(*argument)
                && self
                    .const_scope
                    .iter()
                    .any(|input| input.symbol == *symbol && input.owner == id)
            {
                owner = Some(id);
            }
            bound.push((
                parameter.symbol,
                self.bound_scalar_value(id, scalar_type, *argument, true)?,
            ));
        }
        Ok((bound, owner))
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
            .ok_or_else(|| refuse_at(id, absent))
    }

    /// The position in `RecordType::erased` of the erased field `symbol`
    /// names in `record`, if it names one.
    pub(super) fn erased_member(
        &self,
        id: MachineId,
        record: StructuralTypeId,
        symbol: SymbolHandle,
    ) -> Checked<Option<usize>> {
        Ok(self
            .source(id, record)?
            .erased
            .iter()
            .position(|erased| symbol.is_valid() && *erased == symbol))
    }

    /// What the field symbol of a member names in `record`: a field, or a
    /// payload field of one of its cases. Each field symbol is one
    /// declaration, so at most one answer exists. An erased field has no
    /// storage to name: the readers that can use one ask `erased_member`
    /// first, and every other read of one is refused here.
    pub(super) fn member_of(
        &self,
        id: MachineId,
        record: Opened,
        symbol: SymbolHandle,
    ) -> Checked<Member> {
        if self.erased_member(id, record.record(), symbol)?.is_some() {
            return Err(refuse_at(
                id,
                Unserved::Expression("read of an erased field of data"),
            ));
        }
        let source = self.source(id, record.record())?;
        if let Some(field) = source.fields.iter().position(|field| *field == symbol) {
            return Ok(Member::Field(field));
        }
        for (case, (_, payload)) in source.cases.iter().enumerate() {
            if let Some(field) = payload.iter().position(|field| *field == symbol) {
                return Ok(Member::Payload { case, field });
            }
        }
        Err(refuse_at(
            id,
            Unserved::Typing("member is not a field of its receiver's data type"),
        ))
    }

    /// Intrinsic equality compares tags only on a payload-free sum: no
    /// record fields and no case payloads (data_and_literals.md, "Primitive
    /// and payload-free-sum equality is intrinsic"; conformances.md,
    /// "payload-bearing sums require an explicit named conformance"). Psi
    /// 03 replaces every `==` it can type on any other data with its selected
    /// `Equatable` call, so one still here was never selected: refuse it
    /// rather than silently read the tag alone.
    pub(super) fn require_intrinsic_tag_equality(
        &self,
        id: MachineId,
        record: Opened,
    ) -> Checked<()> {
        let source = self.source(id, record.record())?;
        if source.fields.is_empty()
            && !source.cases.is_empty()
            && source.cases.iter().all(|(_, payload)| payload.is_empty())
        {
            return Ok(());
        }
        Err(refuse_at(
            id,
            Unserved::Typing("`==` on data with no selected Equatable comparison"),
        ))
    }

    /// The position of the case `symbol` names among `record`'s cases.
    /// structural_access.md: "Its case identity must belong to the exact
    /// selected sum type; matching case spellings in another type supply no
    /// authority." The answer comes from the case symbol alone.
    pub(super) fn case_position(
        &self,
        id: MachineId,
        record: Opened,
        symbol: SymbolHandle,
    ) -> Checked<usize> {
        self.source(id, record.record())?
            .cases
            .iter()
            .position(|(case, _)| *case == symbol)
            .ok_or_else(|| {
                refuse_at(
                    id,
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
        record: Opened,
        regions: crate::body::Regions,
        field: usize,
    ) -> Checked<ValueType> {
        let record = record.record();
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
                refuse_at(
                    id,
                    Unserved::Typing("field position past the record's declaration"),
                )
            })
    }

    fn payload_type(
        &mut self,
        id: MachineId,
        record: Opened,
        regions: crate::body::Regions,
        case: usize,
        field: usize,
    ) -> Checked<ValueType> {
        let record = record.record();
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
                refuse_at(
                    id,
                    Unserved::Typing("payload position past the case's declaration"),
                )
            })
    }

    /// `let { a, b as _ } = subject;`: the marker local of a record pattern.
    /// It binds no field and ends an owned noncopy wrapper. The bound fields are the
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
        local: &crate::source_state::statement::TableLocalData,
    ) -> Checked<Option<crate::body::Statement>> {
        let refused = |what| refuse_at(id, what);
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
        let source = terminal_check_tables::row(
            &self.record_sources,
            record.record(),
            id,
            Construct::Machine,
        )?;
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
        // Typing has replaced a `..` by one waived row for each field it
        // covers, so a `let` pattern's roster names every field.
        record_roster_coverage(id, &source.record_members(), roster, false)?;
        let nominal = self
            .typed
            .unique_data_definition_by_symbol(source.declaration)
            .is_some_and(|definition| definition.cleanup_selection.is_some());
        let owned = match place.kind {
            ExpressionKind::Slot(slot) => {
                slots.get(slot).is_some_and(|slot| slot.borrowed.is_none())
            }
            _ => false,
        };
        let noncopy = !self.data_type(id, record.record())?.is_copyable();
        if owned && (nominal || noncopy) {
            // The consuming event retains the exact qualified operand. Its
            // independent content remains an obligation of the custody checker.
            Ok(Some(crate::body::Statement::Decompose { subject }))
        } else {
            Ok(None)
        }
    }

    /// A dispatch arm whose pattern decomposes the record `record`:
    /// `Pair { first as _, second } -> ...`, or the positional
    /// `(_, second) -> ...` whose rows typing already gave their fields. A
    /// record has one shape, so the pattern selects nothing: it is served
    /// when it cannot fail, and the arm then takes every value like `_`.
    /// Stage 01 has replaced each use of a binding by a read of its field.
    ///
    /// `wiki/spec/language/patterns.md`, "Data and case decomposition":
    /// "Named patterns account for every field once; positional patterns have
    /// the corresponding exact arity. An explicit `..` may waive the
    /// remaining components."
    pub(super) fn record_arm(
        &self,
        id: MachineId,
        record: Opened,
        pattern: &TableDestructurePattern,
        at: SourceOrigin,
    ) -> Checked<()> {
        let unserved = |what| refuse_at(id, Unserved::Transition(what));
        if pattern.application_type.is_valid() {
            return Err(unserved("pattern of an applied type"));
        }
        if !pattern.proof_selectors.is_empty() {
            return Err(unserved("pattern with proof selectors"));
        }
        if pattern.guard.is_valid() {
            return Err(unserved("record arm with a guard"));
        }
        let source = self.source(id, record.record())?;
        if pattern.head.is_valid() {
            let named = match self.typed.expression_table.expression(pattern.head) {
                ExpressionNode::Name(path) if !path.static_application.is_valid() => path.symbol,
                _ => return Err(unserved("record pattern head that is not a name")),
            };
            if named != source.declaration {
                return Err(crate::reject(
                    id,
                    at,
                    format!(
                        "this pattern names `{}`, which is not the record its subject holds: a \
                         record pattern names its subject's own type",
                        self.typed.expression_table.display_name(pattern.head)
                    ),
                ));
            }
        }
        let roster = self.typed.expression_table.match_fields(pattern.fields);
        for row in roster {
            if row.component != language_core::PatternComponent::Fields {
                return Err(unserved("pattern that binds its case"));
            }
            if matches!(row.action, MatchFieldAction::Equal(_)) {
                return Err(unserved("record pattern that tests a field"));
            }
        }
        record_roster_coverage(id, &source.record_members(), roster, pattern.has_rest)
    }

    /// A member the typed trees hold only as a spelling. It is an error
    /// where the receiver is a record whose declaration has no field of
    /// that name: expressions.md, "Dot access remains data-field projection;
    /// it does not select or invoke a machine", and state_contracts.md,
    /// "Syntax, name resolution, types and declaration validity are checked
    /// as usual". Every other such member (of a sum, of a value this route
    /// does not read) stays a refusal.
    fn unselected_member(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        member: &TableMemberExpression,
        source: SourceOrigin,
    ) -> terminal_check_tables::Refusal {
        let refused = refuse_at(id, Unserved::Spelling("member without a field symbol"));
        let name = member.member.as_str();
        if name.is_empty() || member.case_variant.is_some() {
            return refused;
        }
        let Ok(base) = self.expression(id, slots, member.receiver, None) else {
            return refused;
        };
        let held = match base.result {
            Some(value) => match self.reference_referent(id, value) {
                Ok(Some((referent, _))) => Some(referent),
                Ok(None) => Some(value),
                Err(_) => None,
            },
            None => None,
        };
        let Some(ValueType::Record(record, _, _)) = held else {
            return refused;
        };
        let Some(data) = self
            .record_sources
            .get(record)
            .and_then(|row| self.data_declaration(row.declaration))
        else {
            return refused;
        };
        let mut fields = 0;
        for declared in self.typed.data_members(data) {
            match declared {
                DataMember::Field(field) if field.name.as_str() == name => return refused,
                DataMember::Field(_) => fields += 1,
                // A member of a sum may name a payload field of a case.
                DataMember::Variant(_) => return refused,
            }
        }
        if fields == 0 {
            return refused;
        }
        // The typed trees give the member no span of its own in an
        // assignment's target; the receiver is the nearest authored place.
        let at = match source {
            SourceOrigin::Unpositioned(_) => base.source,
            authored => authored,
        };
        crate::reject(
            id,
            at,
            format!(
                "data `{}` has no field `{name}`: dot access is data-field projection, and \
                 the name after the dot must be a field its declaration has",
                data.name.as_str()
            ),
        )
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
        access: Access,
    ) -> Checked<Expression> {
        if let crate::source_state::expression::MemberSelection::CollectionMeasure(measure) =
            member.selection
        {
            return self.collection_measure(id, slots, member.receiver, measure, source);
        }
        if !member.selection.field_symbol().is_valid() {
            return Err(self.unselected_member(id, slots, member, source));
        }
        // A read of `items[i].field` copies only the field, so the indexed
        // element is read as a projection base rather than as a whole value.
        let base = match self.node(id, member.receiver)? {
            crate::source_state::expression::ExpressionNode::Indexed(indexed)
                if access == Access::Read =>
            {
                let element = self.indexed(
                    id,
                    slots,
                    indexed,
                    member.receiver,
                    None,
                    access,
                    super::arrays::ElementRead::Projected,
                )?;
                self.complete_formed_operator(id, member.receiver, &element)?;
                element
            }
            _ => self.place(id, slots, member.receiver, None, access)?,
        };
        let Some(record) = self.opened_place(id, &base, access)? else {
            return Err(refuse_at(
                id,
                Unserved::Typing("member of a value that is not a record"),
            ));
        };
        // An erased field holds no executable value (contracts.md,
        // "Explicit erased bindings": it "cannot determine runtime data or
        // control"), so a runtime read of one is refused as a read of any
        // erased binding is.
        if self
            .erased_member(id, record.record(), member.selection.field_symbol())?
            .is_some()
        {
            return Err(crate::reject(
                id,
                source,
                "runtime use of an erased binding",
            ));
        }
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let base = Box::new(base);
        Ok(
            match self.member_of(id, record, member.selection.field_symbol())? {
                Member::Field(field) => Expression {
                    call_context: None,
                    result: Some(self.field_type(id, record, regions, field)?),
                    kind: ExpressionKind::Field { base, field },
                    source,
                },
                Member::Payload { case, field } => Expression {
                    call_context: None,
                    result: Some(self.payload_type(id, record, regions, case, field)?),
                    kind: ExpressionKind::Payload { base, case, field },
                    source,
                },
            },
        )
    }

    /// Read authored record or selected-case fields in their written order.
    /// A final `..` supplies checked zero values for missing runtime fields;
    /// without it the literal must supply every runtime field.
    pub(super) fn record_literal(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        expression: ExpressionHandle,
        literal: &TableStructLiteral,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let expected = self.expected.take();
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
        let typing = |what| refuse_at(id, Unserved::Typing(what));
        let spelling = |what| refuse_at(id, Unserved::Spelling(what));
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
                .map_err(spelling)?
                .map_or_else(
                    crate::source_state::types::TypeReferenceHandle::invalid,
                    |constant| constant.declared_type,
                ),
        ];
        let result = match applications
            .iter()
            .copied()
            .find(|reference| reference.is_valid())
        {
            Some(reference) => {
                // A layout storage application holds its semantic record's
                // fields; the literal names that record and builds the
                // storage representation.
                let semantic =
                    super::source_records::semantic_reference(self.typed, reference, id)?;
                let selected = self
                    .typed
                    .data_application_definition(semantic)
                    .or_else(|| self.typed.closed_data_definition(semantic))
                    .ok_or_else(|| {
                        unserved("literal without its exact retained record application")
                    })?;
                if self
                    .typed
                    .data_constructor_definition(semantic, literal.type_symbol)
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
        let ValueType::Record(record, Qualification::NONE, _) = result else {
            return Err(typing("record literal of a scalar or a qualified type"));
        };
        if matches!(
            self.data_type(id, record)?.shape,
            crate::StorageShape::Opaque
        ) {
            return Err(typing(
                "opaque literal requires its checked representation and supplied contents",
            ));
        }
        let result = literal_value_type(result, expected);
        let regions = result.regions();
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
        // The literal constructs this carrier, so it names its fields and
        // cases without asking any domain (`authority::Opened::constructing`).
        let constructed = Opened::constructing(record);
        let case = match case_symbol {
            Some(symbol) => Some(self.case_position(id, constructed, symbol)?),
            None => None,
        };
        let substitution = self.field_regions(id, record, regions)?;
        let data = self.data_type(id, record)?;
        let declared_fields = match case {
            Some(case) => data
                .cases()
                .get(case)
                .ok_or_else(|| typing("case position past the sum's declaration"))?
                .fields
                .as_slice(),
            None if data.cases().is_empty() => data.fields(),
            None => return Err(typing("literal of a sum without a case")),
        };
        let names: Vec<String> = declared_fields
            .iter()
            .map(|field| field.identity.clone())
            .collect();
        let declared: Vec<ValueType> = declared_fields
            .iter()
            .map(|field| field.value_type)
            .collect();
        let erased: Vec<usize> = data
            .erased
            .iter()
            .enumerate()
            .filter(|(_, erased)| erased.case == case)
            .map(|(position, _)| position)
            .collect();
        let declared: Vec<ValueType> = declared
            .into_iter()
            .map(|field| self.substitute(field, &substitution))
            .collect();
        let authored = self.typed.expression_table.struct_fields(literal.fields);
        let mut seen = vec![false; declared.len()];
        let mut erased_seen = vec![false; erased.len()];
        let mut fields = Vec::with_capacity(declared.len());
        let mut erased_terms = Vec::with_capacity(erased.len());
        for field in authored {
            if !field.field_symbol.is_valid() {
                return Err(spelling("record literal field without a symbol"));
            }
            // contracts.md, "Explicit erased bindings": "Construction
            // supplies the erased term". The literal's term for an erased
            // field is read as an erased local's initializer is, and is no
            // runtime operand: the record has no storage for it.
            if let Some(position) = self.erased_member(id, record, field.field_symbol)? {
                let slot = erased
                    .iter()
                    .position(|candidate| *candidate == position)
                    .ok_or_else(|| typing("case literal names a field of another case"))?;
                if std::mem::replace(&mut erased_seen[slot], true) {
                    return Err(typing("record literal names a field twice"));
                }
                if let Some(erased_term) =
                    self.erased_field_term(id, slots, record, position, field.value)?
                {
                    erased_terms.push(erased_term);
                }
                continue;
            }
            let position = match case {
                None => self.field_position(
                    id,
                    record,
                    field.field_symbol,
                    Unserved::Typing("record literal names a field of another type"),
                )?,
                Some(case) => match self.member_of(id, constructed, field.field_symbol)? {
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
            if !self.agrees(operand.result, Some(field_type)) {
                return Err(self.mistyped(
                    id,
                    operand.source,
                    super::rejections::Destination::Field,
                    Some(field_type),
                    operand.result,
                    "record literal field type",
                ));
            }
            fields.push((position, operand));
        }
        // No zero value stands in for an erased term: "there is no general
        // implicit inhabitance or default-value search".
        if erased_seen.contains(&false) {
            return Err(if literal.zero_omitted_fields {
                unserved("record literal that leaves an erased field to `..`")
            } else {
                typing("record literal misses a field")
            });
        }
        if seen.contains(&false) {
            if !literal.zero_omitted_fields {
                return Err(typing("record literal misses a field"));
            }
            // Authored operands stay in their original evaluation order. Each
            // missing operand is an ordinary zero value, and the final record
            // or selected case still owes its complete default domain.
            for (position, (present, field_type)) in seen.iter().zip(&declared).enumerate() {
                if !present {
                    let zero = self.zero_value(id, *field_type, Some(&names[position]), source)?;
                    fields.push((position, zero));
                }
            }
        }
        Ok(Expression {
            call_context: None,
            kind: match case {
                None => ExpressionKind::Record {
                    fields,
                    erased: erased_terms,
                },
                Some(case) => ExpressionKind::Case {
                    case,
                    fields,
                    erased: erased_terms,
                },
            },
            result: Some(result),
            source,
        })
    }

    /// The term a literal supplies for the erased field at `position` of
    /// `record`, read by the erased-term reader of an erased local's
    /// initializer: a literal, a binding or a denotational call, whose
    /// formation that reader checks. Scalar erased fields keep their term for
    /// clause checking while still contributing no runtime storage. Evidence
    /// is not read here: the mathematical signature checks that a literal
    /// supplies evidence at the field's index (`families::evidence_uses`).
    fn erased_field_term(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        record: StructuralTypeId,
        position: usize,
        value: crate::source_state::expression::ExpressionHandle,
    ) -> Checked<Option<(StructuralFieldId, Expression)>> {
        let erased = self
            .data_type(id, record)?
            .erased
            .get(position)
            .ok_or_else(|| refuse_at(id, Unserved::Typing("erased field position")))?;
        if erased.evidence {
            return Ok(None);
        }
        let field_id = erased.id;
        let value_type = erased.value_type;
        let term = self.erased_term(id, slots, value, value_type.scalar())?;
        if term.result != Some(value_type) {
            return Err(refuse_at(id, Unserved::Typing("record literal field type")));
        }
        Ok(Some((field_id, term)))
    }

    /// Construct zero using the existing initialization logic, then check each
    /// declared qualification as an explicit cast would. Zero bits alone do not
    /// prove domain membership, issuer permission, or required backing storage.
    /// `omitted` names the field of a literal that `..` zeroes, for the
    /// diagnostic when its zero cannot exist (`zero_established`).
    pub(super) fn zero_value(
        &mut self,
        id: MachineId,
        result: ValueType,
        omitted: Option<&str>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let plain = result
            .qualified(crate::body::Qualification::NONE)
            .ok_or_else(|| refuse_at(id, Unserved::Expression("zero of a rigid type")))?;
        // Ordinary field construction retains each field's qualification
        // evidence. An opaque zero operation cannot establish issuer permissions.
        let kind = match plain {
            ValueType::Record(record, _, regions) => {
                let selected = match &self.data_type(id, record)?.shape {
                    StorageShape::Record(fields) => Some((None, fields.clone())),
                    StorageShape::Sum(cases) => {
                        cases.first().map(|case| (Some(0), case.fields.clone()))
                    }
                    _ => None,
                };
                match selected {
                    Some((case, declared)) => {
                        let substitution = self.field_regions(id, record, regions)?;
                        let mut fields = Vec::with_capacity(declared.len());
                        for (position, field) in declared.into_iter().enumerate() {
                            let field_type = self.substitute(field.value_type, &substitution);
                            fields.push((
                                position,
                                self.zero_value(id, field_type, omitted, source)?,
                            ));
                        }
                        match case {
                            None => ExpressionKind::Record {
                                fields,
                                erased: Vec::new(),
                            },
                            Some(case) => ExpressionKind::Case {
                                case,
                                fields,
                                erased: Vec::new(),
                            },
                        }
                    }
                    None => {
                        self.zero_established(id, plain, omitted, source)?;
                        ExpressionKind::Zero
                    }
                }
            }
            _ => {
                self.zero_established(id, plain, omitted, source)?;
                ExpressionKind::Zero
            }
        };
        let mut operand = Expression {
            call_context: None,
            kind,
            result: Some(plain),
            source,
        };
        let mut qualification = crate::body::Qualification::NONE;
        for domain in result.qualification().domains.members() {
            qualification.domains = qualification.domains.with(domain).ok_or_else(|| {
                refuse_at(
                    id,
                    Unserved::Expression("zero qualification set is too large"),
                )
            })?;
            let qualified = plain
                .qualified(qualification)
                .expect("plain value has a carrier");
            operand = self.add_domain(id, operand, qualified, domain, source)?;
        }
        if operand.result != Some(result) {
            operand = Expression {
                call_context: None,
                kind: ExpressionKind::Requalify(Box::new(operand)),
                result: Some(result),
                source,
            };
        }
        Ok(operand)
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
            ExpressionNode::MachineValue(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::Block(_)
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
    /// the subject's own sum type. Equality has its own value-comparison
    /// admission; membership does not ask for Equatable.
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
            return Err(refuse_at(
                id,
                Unserved::Typing("case test of a value that is not a sum"),
            ));
        };
        let case = self.case_position(id, record, case)?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::IsCase {
                subject: Box::new(subject),
                case,
            },
            result: Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
            source,
        })
    }

    /// Preserve the checked projected-equality condition as an ordinary
    /// expression. It will execute only after lowering selects this case.
    pub(super) fn case_selector(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        record: Opened,
        pattern: &TableDestructurePattern,
        subject: &Expression,
        at: SourceOrigin,
    ) -> Checked<CaseSelector> {
        let case = self.case_pattern(id, record, pattern, at)?;
        let condition = self.expression(
            id,
            slots,
            pattern.condition,
            Some(ValueType::plain(ScalarType::Boolean)),
        )?;
        let tests_fields = self
            .typed
            .expression_table
            .match_fields(pattern.fields)
            .iter()
            .any(|field| matches!(field.action, MatchFieldAction::Equal(_)));
        if !tests_fields
            && !super::expressions::same_computation(
                &condition,
                &super::expressions::case_condition(subject, case),
            )
        {
            return Err(refuse_at(
                id,
                Unserved::Transition("case pattern whose condition is not its case test"),
            ));
        }
        Ok(CaseSelector {
            case,
            condition: tests_fields.then(|| Box::new(condition)),
        })
    }

    /// The case a destructuring pattern selects in the sum `record`, after
    /// checking its nominal head and exact payload roster. A field may bind,
    /// waive, or test its projected value. The condition is read separately
    /// by `case_selector`; a binding has no node of its own because stage 01
    /// replaces each use of it by the payload member it names. The
    /// pattern must name every payload field unless it ends in `..`;
    /// frozen typed trees retain the roster, not a coverage verdict, so
    /// this reader checks that rule before lowering the binding.
    ///
    /// A pattern that lists payload fields and names one twice, or misses
    /// one without `..`, is wrong in every program and rejected at `at`:
    /// "Named patterns account for every field once" (patterns.md, "Data and
    /// case decomposition"). A pattern that lists none may be a case named as
    /// a value, which this reader does not tell apart, and stays refused.
    pub(super) fn case_pattern(
        &self,
        id: MachineId,
        record: Opened,
        pattern: &TableDestructurePattern,
        at: SourceOrigin,
    ) -> Checked<usize> {
        let unserved = |what| refuse_at(id, Unserved::Transition(what));
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
        let payload = self.source(id, record.record())?.cases[case].1.len();
        let mut seen = vec![false; payload];
        // The case's erased payload fields are payload fields a pattern
        // accounts for too, but only by waiving: there is nothing to bind.
        let erased: Vec<usize> = self
            .data_type(id, record.record())?
            .erased
            .iter()
            .enumerate()
            .filter(|(_, erased)| erased.case == Some(case))
            .map(|(position, _)| position)
            .collect();
        let mut erased_seen = vec![false; erased.len()];
        let listed = self.typed.expression_table.match_fields(pattern.fields);
        for field in listed {
            match field.component {
                language_core::PatternComponent::Fields => {}
                language_core::PatternComponent::Case => {
                    return Err(unserved("pattern that binds its case"));
                }
            }
            match &field.action {
                MatchFieldAction::Bind(_)
                | MatchFieldAction::Waive
                | MatchFieldAction::Equal(_) => {}
            }
            if !field.field_symbol.is_valid() {
                return Err(refuse_at(
                    id,
                    Unserved::Spelling("pattern field without a symbol"),
                ));
            }
            if let Some(position) = self.erased_member(id, record.record(), field.field_symbol)?
                && let Some(slot) = erased.iter().position(|candidate| *candidate == position)
                && matches!(field.action, MatchFieldAction::Waive)
            {
                if std::mem::replace(&mut erased_seen[slot], true) {
                    return Err(crate::reject(
                        id,
                        at,
                        format!(
                            "this pattern names the payload field `{}` twice: a case pattern \
                             names each payload field once",
                            field.member.as_str()
                        ),
                    ));
                }
                continue;
            }
            let position = match self.member_of(id, record, field.field_symbol)? {
                Member::Payload { case: owner, field } if owner == case => field,
                Member::Payload { .. } | Member::Field(_) => {
                    return Err(refuse_at(
                        id,
                        Unserved::Typing("pattern names a field of another case"),
                    ));
                }
            };
            if std::mem::replace(&mut seen[position], true) {
                return Err(crate::reject(
                    id,
                    at,
                    format!(
                        "this pattern names the payload field `{}` twice: a case pattern names \
                         each payload field once",
                        field.member.as_str()
                    ),
                ));
            }
        }
        if !pattern.has_rest && (seen.contains(&false) || erased_seen.contains(&false)) {
            if listed.is_empty() {
                return Err(refuse_at(
                    id,
                    Unserved::Typing("pattern misses a payload field"),
                ));
            }
            return Err(crate::reject(
                id,
                at,
                "this pattern leaves a payload field out without `..`: a case pattern mentions \
                 every payload field, or ends in `..` to waive the rest",
            ));
        }
        Ok(case)
    }
}

/// Whether the rows of a record pattern name each declared field at most
/// once, and all of them unless the pattern ends in `..` (`waives_rest`).
/// Coverage is read from the rows' field symbols and the record's own; no
/// spelling is compared.
fn record_roster_coverage(
    id: MachineId,
    fields: &[SymbolHandle],
    roster: &[crate::source_state::expression::TableMatchField],
    waives_rest: bool,
) -> Checked<()> {
    let refused = |what| refuse_at(id, what);
    let mut declared = fields.to_vec();
    declared.sort_unstable_by_key(|field| (field.generation(), field.arena_index()));
    if declared.windows(2).any(|fields| fields[0] == fields[1]) {
        return Err(refused(Unserved::Spelling(
            "record pattern over ambiguous field declarations",
        )));
    }
    let mut used = vec![false; declared.len()];
    for row in roster {
        let field = row.field_symbol;
        let position = field
            .is_valid()
            .then(|| {
                declared
                    .binary_search_by_key(&(field.generation(), field.arena_index()), |candidate| {
                        (candidate.generation(), candidate.arena_index())
                    })
                    .ok()
            })
            .flatten()
            .ok_or_else(|| refused(Unserved::Spelling("record pattern field without a symbol")))?;
        if std::mem::replace(&mut used[position], true) {
            return Err(refused(Unserved::Typing("record pattern repeats a field")));
        }
    }
    if !waives_rest && used.iter().any(|used| !used) {
        return Err(refused(Unserved::Statement(
            "record pattern that omits a field",
        )));
    }
    Ok(())
}

/// Whether a field or payload field is `[erased]`.
fn is_erased(field: &DataField) -> bool {
    field.relevance == language_core::BindingRelevance::Erased
}

/// Whether a data declaration is a plain record or a plain sum this slice
/// serves. Each feature outside it is refused by name, never ignored.
#[cfg(test)]
fn record_shape(
    id: MachineId,
    data: &DataDefinition,
    application: Option<&crate::source_state::SourceTypeApplication>,
    members: &[DataMember],
) -> Checked<()> {
    record_shape_for(
        id,
        data,
        application,
        super::storage_metadata::TypePurpose::Value,
        members.into(),
    )
}

fn record_shape_for(
    id: MachineId,
    data: &DataDefinition,
    application: Option<&crate::source_state::SourceTypeApplication>,
    _purpose: super::storage_metadata::TypePurpose,
    members: arena::RootRows<'_, DataMember>,
) -> Checked<()> {
    let unserved = |what| Err(refuse_at(id, Unserved::Type(what)));
    if data.supply_mode == language_core::DataSupplyMode::BoundaryOpaque && !members.is_empty() {
        return unserved("opaque data with declared members");
    }
    if data.supply_mode == language_core::DataSupplyMode::BoundaryOpaque {
        if !data.lifetime_parameters.is_empty() {
            return unserved("opaque data without its retained lifetime obligations");
        }
        if !data.requires_facts.is_empty() {
            return unserved("opaque data without its checked value predicates");
        }
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
    // Multiplicity describes the carrier. The ordinary ownership walk emits
    // claim transport; Psi 06 independently reconstructs whether it is valid.
    // `zero_gated` is not read: whether the zero value satisfies the
    // declaration's `requires` is this route's own proof question, asked
    // where a local of the type is declared without an initializer.
    if !data.retired_identities.is_empty() {
        return unserved("data with retired field identities");
    }
    // Data with no member is a record whose roster is empty (`data Token
    // {}`): a value of it is established, copied or moved, passed and
    // dropped like any other record, and holds nothing to read.
    // An `[erased]` field is a member with no storage (`declared_erased`).
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
            }
            DataMember::Field(_) => fields = true,
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

/// A fresh literal takes its destination's regions when its carrier and
/// qualification agree. Its fields still have to form at those regions.
/// Without a matching destination, use the literal's own type.
pub(super) fn literal_value_type(written: ValueType, expected: Option<ValueType>) -> ValueType {
    expected
        .filter(|destination| destination.erased() == written.erased())
        .unwrap_or(written)
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
/// (`crate::source_state::type_identity::lifetime_parameter_identity`), so two
/// binders with one tag would be one parameter to every lookup. A use that
/// names an undeclared tag is rejected where its type is read
/// (`lifetime_application`, `reference_lifetime_declared`).
pub(super) fn lifetime_binders(
    id: MachineId,
    binders: &[Identifier],
    at: SourceOrigin,
) -> Checked<()> {
    for (position, binder) in binders.iter().enumerate() {
        if binder.as_str().is_empty() {
            return Err(refuse_at(
                id,
                Unserved::Spelling("lifetime parameter without a tag"),
            ));
        }
        // Wrong in every program, not a form left unread: a rejection.
        if binders[..position]
            .iter()
            .any(|earlier| earlier.as_str() == binder.as_str())
        {
            return Err(crate::reject(
                id,
                at,
                format!(
                    "this declaration states the lifetime `'{}` twice: duplicate lifetime \
                     binders reject",
                    binder.as_str().trim_start_matches('\'')
                ),
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
    let typing = |what| Err(refuse_at(id, Unserved::Typing(what)));
    if arguments.len() != declared {
        return typing("lifetime argument count differs from the data declaration's");
    }
    if !retained.is_empty() && retained.len() != arguments.len() {
        return typing("lifetime argument its owner does not declare");
    }
    for (slot, argument) in arguments.iter().enumerate() {
        let tag = argument.as_str();
        if tag.is_empty() {
            return typing("lifetime argument its owner does not declare");
        }
        match retained.get(slot) {
            Some(RetainedLifetime::Resolved(parameter)) => {
                if *parameter != Some(tag) {
                    return typing("lifetime argument its owner does not declare");
                }
            }
            // An authored tag the owner has no binder for: the program is
            // wrong, as for a reference (`reference_lifetime_declared`).
            Some(RetainedLifetime::Open) | None => {
                if !declares(tag) {
                    return Err(crate::reject(
                        id,
                        SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                        undeclared_lifetime_rule(tag),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// `@'a T` where a type is read: the tag a reference writes must be a binder
/// its owner declares. lifetimes.md: "Duplicate binders and undeclared tags
/// reject." A tag no binder of the owner answers is the author's error in
/// every program, so it is rejected here rather than left to the identity
/// reader beneath, which would only find the region unresolved and call the
/// reference's type unsupported.
///
/// `owner` is the occurrence's lifetime owner, as the readers pass it to the
/// type identity they compute next. A lifetime the typed trees resolved for
/// this occurrence (`operand_lifetimes`) is declared by construction; an
/// elided lifetime or an empty tag is judged by the readers as before.
pub(super) fn reference_lifetime_declared(
    typed: &crate::source_state::TypedTrees,
    id: MachineId,
    reference: crate::source_state::types::TypeReferenceHandle,
    owner: SymbolHandle,
) -> Checked<()> {
    let table = &typed.type_reference_table;
    let TypeReferenceNode::Reference {
        lifetime: Some(tag),
        ..
    } = table.type_reference(reference)
    else {
        return Ok(());
    };
    let resolved = table
        .operand_lifetimes(reference)
        .first()
        .is_some_and(|region| region.declaration.is_valid());
    if resolved
        || tag.as_str().is_empty()
        || !owner.is_valid()
        || crate::source_state::type_identity::lifetime_parameter_identity(
            typed,
            owner,
            tag.as_str(),
        )
        .is_some()
    {
        return Ok(());
    }
    Err(crate::reject(
        id,
        SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
        undeclared_lifetime_rule(tag.as_str()),
    ))
}

/// The rule an undeclared lifetime tag breaks, as a rejection states it.
pub(super) fn undeclared_lifetime_rule(tag: &str) -> String {
    format!(
        "this type names the lifetime `'{}`, which its declaration does not declare: \
         undeclared lifetime tags reject",
        tag.trim_start_matches('\'')
    )
}

// Copy permission must reach every nested value. Scalar fields alone do not
// grant it to their affine owner, and a missing record identity grants nothing.
fn copy_field_is_unrestricted(
    field: ValueType,
    records: &arena::IdTable<StructuralTypeId, RecordType>,
) -> bool {
    match field {
        // A policy changes no payload: the integer copies as its carrier does.
        ValueType::Scalar(..) => true,
        ValueType::Record(record, _, _) => records
            .get(record)
            .is_some_and(|record| record.is_copyable()),
        ValueType::Rigid(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::data::{DataField, DataProperties, DataVariant};
    use arena::{Handle, HandleSpan};

    fn machine() -> MachineId {
        MachineId::new(1).unwrap()
    }

    #[test]
    fn copy_permission_does_not_hide_an_owned_nested_record() {
        let mut records = arena::IdTable::default();
        let owned = records.push(RecordType {
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            cleanup: None,
            identity: "Owned".into(),
            shape: crate::StorageShape::Record(Vec::new()),
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        assert!(!copy_field_is_unrestricted(
            ValueType::record(owned),
            &records
        ));
        records.get_mut(owned).unwrap().multiplicity =
            terminal_psi::StructuralMultiplicity::Unrestricted;
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
                multiplicity: language_core::Multiplicity::Unrestricted,
            },
            ..DataDefinition::default()
        };
        (data, vec![DataMember::Field(DataField::default())])
    }

    fn detail(data: &DataDefinition, members: &[DataMember]) -> String {
        let refusal = record_shape(machine(), data, None, members).unwrap_err();
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
        assert_eq!(record_shape(machine(), &data, None, &members), Ok(()));
        let mut affine = data.clone();
        affine.properties.multiplicity = language_core::Multiplicity::Affine;
        assert_eq!(record_shape(machine(), &affine, None, &members), Ok(()));
        // Lifetime parameters select nothing a value is made of, so the
        // shape is served with them; a type parameter beside them is not.
        let mut borrowing = data.clone();
        borrowing.lifetime_parameters = vec![Identifier::generated("buffer")];
        assert_eq!(record_shape(machine(), &borrowing, None, &members), Ok(()));
        borrowing.type_parameters = HandleSpan::from_parts(Handle::from_parts(1, 1), 1);
        assert_eq!(detail(&borrowing, &members), "type generic data");
        // A declaration-level `requires` is no longer refused here: its
        // clauses are read by the clause reader. A case-level one still is.
        let cases: [(&str, fn(&mut DataDefinition, &mut Vec<DataMember>)); 5] = [
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
            ("type opaque data with declared members", |data, _| {
                data.supply_mode = language_core::DataSupplyMode::BoundaryOpaque;
            }),
        ];
        for (expected, change) in cases {
            let (mut data, mut members) = plain();
            change(&mut data, &mut members);
            assert_eq!(detail(&data, &members), expected);
        }
        // Data with no member at all is a record with an empty roster.
        let (data, _) = plain();
        assert_eq!(record_shape(machine(), &data, None, &[]), Ok(()));
        let mut opaque = data.clone();
        opaque.supply_mode = language_core::DataSupplyMode::BoundaryOpaque;
        assert_eq!(record_shape(machine(), &opaque, None, &[]), Ok(()));
        // A sum alone, with or without payload, is served.
        let sum = vec![
            DataMember::Variant(DataVariant::default()),
            DataMember::Variant(DataVariant::default()),
        ];
        assert_eq!(record_shape(machine(), &data, None, &sum), Ok(()));
        // An erased field is a member with no storage, so its shape is
        // served; `declared_erased` reads what it holds.
        let (data, mut members) = plain();
        members.push(DataMember::Field(DataField {
            relevance: language_core::BindingRelevance::Erased,
            ..DataField::default()
        }));
        assert_eq!(record_shape(machine(), &data, None, &members), Ok(()));
    }

    fn tags(tags: &[&str]) -> Vec<Identifier> {
        tags.iter().copied().map(Identifier::generated).collect()
    }

    /// `data Pair<'a, 'b>` is served; a binder repeated is rejected
    /// (lifetimes.md: "Duplicate binders and undeclared tags reject"), and
    /// one the typed trees hold without a tag is refused.
    #[test]
    fn a_data_declaration_states_each_lifetime_binder_once() {
        let nowhere = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        assert_eq!(lifetime_binders(machine(), &[], nowhere), Ok(()));
        assert_eq!(
            lifetime_binders(machine(), &tags(&["a", "b"]), nowhere),
            Ok(())
        );
        let repeated = lifetime_binders(machine(), &tags(&["a", "b", "a"]), nowhere).unwrap_err();
        assert_eq!(
            repeated.reason,
            terminal_check_tables::RefusalReason::Violation
        );
        assert_eq!(repeated.source_span, None);
        assert_eq!(
            repeated.detail,
            "this declaration states the lifetime `'a` twice: duplicate lifetime binders reject"
        );
        assert_eq!(
            lifetime_binders(machine(), &[Identifier::default()], nowhere)
                .unwrap_err()
                .detail,
            "spelling lifetime parameter without a tag"
        );
    }

    /// `Inner<'a>` is served where the owner of its occurrence declares
    /// `'a` and `Inner` declares one lifetime. Too few or too many
    /// arguments are refused, and an argument the owner does not declare is
    /// rejected; a machine is read under no owner, so there a lifetime the
    /// typed trees did not resolve is undeclared. A lifetime they did resolve must be
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
        // An authored tag the owner does not declare is the program's error.
        assert_eq!(
            detail(1, &["b"], &[], &["a"]),
            Err(undeclared_lifetime_rule("b"))
        );
        assert_eq!(
            detail(1, &["a"], &[], &[]),
            Err(undeclared_lifetime_rule("a"))
        );
        assert_eq!(
            lifetime_application(machine(), 1, &tags(&["b"]), &[], |_| false)
                .unwrap_err()
                .reason,
            terminal_check_tables::RefusalReason::Violation
        );
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
