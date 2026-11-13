/*
The place family of lowering: structural roots, projections interned on first
use, the forward write log, record construction, field reads, stores, whole
copies and locals declared without an initializer. This is where
`MachineTables::places`, `roots` and `writes` are filled, in lockstep with
the Terminal Psi operations that create or change the storage.

Roots. One Terminal `PlaceId` and one `PlaceRow { origin: Root }` per record
formal, per operation that produces storage (a literal, a whole copy, a call
with a record result, the home of a local declared without an initializer)
and for a record machine's result place; `roots` maps the `PlaceId` to its
row. A record local has no place of its own: its slot holds the root its
initializer, or the last whole value assigned to it, produced.
ownership.md: "Parameter, named-local, and call-result roots retain distinct
establishment identities."

Projections. `r.a` is the row `(r, Segment::Field(a))`, found by binary
search of the parent's sorted `children` and pushed only when absent, so a
path names one `ExactPlaceId` however often it is used. ownership.md:
"Resolve every segment against the type reached by the previous segment".

The write log. Each row says what one operation stored and where, and
nothing else: lowering never asks what a place holds.

- A literal writes `Content::Value(operand)` on each scalar field of its
  fresh root, and a scalar store writes it on the exact leaf it names.
- A whole copy writes one `Content::Overwritten` on its fresh root, and a
  record moved into a field (a record operand of a literal, a record stored
  into a field) writes one on that field's place. `Content` has no way to
  say "what that other place held", so the row states only that the place
  has new contents; the operation at the row's point names the source, and
  the checker resolves each leaf through it with the same walk it uses for
  a read (`lowered-discharge`, `places`).
- A case literal writes one `Content::Overwritten` on its fresh root, then
  `Content::Value(operand)` on each scalar payload leaf and one
  `Content::Overwritten` on each data payload field. No row can say which
  case the root was established as: `Content` has no such form, so the
  checker reads it from the establishing operation at the root's row, as it
  reads a copy's source.
- A call's record result is one `Content::Overwritten` on its root: nothing
  here knows the callee's values.
- A data local declared without an initializer writes one
  `Content::Overwritten` on its fresh root and a constant zero on each
  scalar leaf of the zero value. A scalar local declared that way writes
  the zero constant its home is established with on that home
  (`local-without-initializer`: a builtin scalar admits zero, so the zero is
  an established value).
- What a formal holds at entry is not a write. It is one value per scalar
  leaf with `ValueOrigin::EntryContent { place }`, minted the first time an
  operation names the leaf or a place above it, whatever is stored there
  later.
*/

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, IdTable, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment, WriteRow};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::{Lender, ObligationKind};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, ProgramPoint, PsiSemanticId, Refusal, RefusalReason,
};
use terminal_psi::{
    NativeLayoutCatalog, NativeLayoutFieldIdentity, Operation, OperationKind, OperationResult,
    RecordFieldInitializer, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralCaseSuccessorEdge, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralRecordStorage, StructuralStoreDisposition, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalIdentityIndex, TerminalIdentityTable,
    TerminalIdentityTableBuilder, Terminator,
};

use super::{Lowering, Value};
use crate::body::{
    Body, Expression, ExpressionKind, RecordField, RecordType, Signature, Statement, Target,
    ValueType,
};
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};

/// The module's record types as lowering needs them: the reader's table and,
/// beside it, the interned identity Terminal Psi's declarations and
/// structural paths name each field by.
pub(crate) struct Records<'a> {
    types: &'a IdTable<StructuralTypeId, RecordType>,
    /// The primitive structural type of each scalar type some local is homed
    /// in, declared after the record types. A handful at most, searched by
    /// the scalar type itself.
    primitives: Vec<(ScalarType, StructuralTypeId)>,
    /// `field_identities[record][position]`: the field's index in the
    /// module's identity table. It is written into `StructuralLeafCopy`
    /// paths, which is the form Terminal Psi gives them; nothing compares it.
    field_identities: IdTable<StructuralTypeId, Vec<TerminalIdentityIndex>>,
    /// `payload_identities[sum][case][position]`: the same for a payload
    /// field.
    payload_identities: IdTable<StructuralTypeId, Vec<Vec<TerminalIdentityIndex>>>,
}

/// What a module with these record types must declare: Terminal Psi's
/// structural type table, the identity table its rows index, and the native
/// layout catalog, which the decoder requires to list every record.
pub(crate) struct Declared<'a> {
    pub records: Records<'a>,
    pub identities: TerminalIdentityTable,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub native_layouts: NativeLayoutCatalog,
}

/// The Terminal Psi declarations of the record types, in identity order.
///
/// Copyability follows the declaration's multiplicity. These plain records
/// have no nominal hook or conserved claim, so represented contents need no
/// executable disposal; affine values still retain their move/discard custody.
///
/// A scalar local declared without an initializer is homed in a place of
/// primitive structural type, so each scalar type such a local has is
/// declared too, after the records, as the current pipeline declares it:
/// copyable, with no cleanup and no claim.
pub(crate) fn declare<'a, 'b>(
    entry: MachineId,
    types: &'a IdTable<StructuralTypeId, RecordType>,
    bodies: &'b IdTable<MachineId, Body>,
    signatures: impl Iterator<Item = &'b Signature>,
) -> Checked<Declared<'a>> {
    let mut identities = TerminalIdentityTableBuilder::default();
    let mut field_identities = IdTable::default();
    let mut structural_types = Vec::with_capacity(types.len());
    let mut native_layouts = NativeLayoutCatalog::default();
    let mut payload_identities = IdTable::default();
    for (id, record) in types.iter() {
        if record.multiplicity == StructuralMultiplicity::Linear {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Type("linear record claims"),
            ));
        }
        let mut declare_fields = |declared: &[RecordField]| {
            let mut indexes = Vec::with_capacity(declared.len());
            let mut fields = Vec::with_capacity(declared.len());
            for field in declared {
                let identity = identities.intern(&field.identity);
                indexes.push(identity);
                let field_type = match field.value_type {
                    // The reader refuses a data type with such a field.
                    ValueType::Rigid(_) => {
                        return Err(refuse(
                            entry,
                            Construct::Machine,
                            Unserved::Type("generic data"),
                        ));
                    }
                    ValueType::Scalar(scalar_type) => StructuralFieldType::Scalar(scalar_type),
                    ValueType::Record(nested) => {
                        // A data type holds only types declared before it,
                        // so the table is acyclic and in dependency order.
                        if nested.get() >= id.get() {
                            return Err(refuse(
                                entry,
                                Construct::Machine,
                                Unserved::Type("record field of a later record type"),
                            ));
                        }
                        StructuralFieldType::Structural(nested)
                    }
                };
                fields.push(StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: field.id,
                    identity,
                    relevance: language_core::BindingRelevance::Relevant,
                    field_type,
                    storage: None,
                });
            }
            Ok((indexes, fields))
        };
        let (indexes, fields) = declare_fields(&record.fields)?;
        let mut case_indexes = Vec::with_capacity(record.cases.len());
        let mut cases = Vec::with_capacity(record.cases.len());
        for case in &record.cases {
            let (indexes, fields) = declare_fields(&case.fields)?;
            case_indexes.push(indexes);
            cases.push((case, fields));
        }
        field_identities.push(indexes);
        payload_identities.push(case_indexes);
        let shape = if cases.is_empty() {
            // The natural layout row of a record with no authored layout:
            // no policy and no realization. Its nominal native identities
            // are left absent, as the current pipeline leaves any it cannot
            // supply; they serve the native layout review, which this route
            // does not reach. The decoder asks for a row per record and for
            // none per sum.
            native_layouts.insert(
                id,
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                fields.iter().map(|field| NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: TerminalIdentityIndex::ABSENT,
                }),
                [],
            );
            StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            }
        } else if fields.is_empty() {
            StructuralTypeShape::Sum {
                cases: cases
                    .into_iter()
                    .map(|(case, fields)| StructuralCaseDeclaration {
                        id: case.id,
                        identity: identities.intern(&case.identity),
                        fields,
                    })
                    .collect(),
            }
        } else {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Type("mixed data"),
            ));
        };
        structural_types.push(StructuralTypeDeclaration {
            requires: record.requires.clone(),
            quotient_carrier: None,
            id,
            identity: identities.intern(&record.identity),
            owns_linear_claim: false,
            copyable: record.multiplicity == StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            shape,
            scalar_restriction: None,
        });
    }
    // Every scalar place a body names besides its records: the home of a
    // scalar local declared without an initializer or lent to a call, and
    // the place of a borrowed scalar formal. Each needs its primitive type
    // declared once, with the text identity the typed trees gave it.
    let mut homes: Vec<(ScalarType, Option<&String>)> = Vec::new();
    for signature in signatures {
        for borrowed in &signature.borrowed {
            if let Some(ValueType::Scalar(scalar_type)) =
                signature.parameters.get(borrowed.position).copied()
            {
                homes.push((scalar_type, borrowed.home.as_ref()));
            }
        }
    }
    for (_, body) in bodies.iter() {
        for statement in &body.statements {
            statement_homes(statement, &mut homes);
            let Statement::Declare { slot, home, .. } = statement else {
                continue;
            };
            let Some(ValueType::Scalar(scalar_type)) = body.slots.get(*slot).copied() else {
                continue;
            };
            homes.push((scalar_type, home.as_ref()));
        }
    }
    let mut primitives: Vec<(ScalarType, StructuralTypeId)> = Vec::new();
    for (scalar_type, home) in homes {
        {
            if primitives.iter().any(|(known, _)| *known == scalar_type) {
                continue;
            }
            let Some(home) = home else {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Spelling("scalar home without a type identity"),
                ));
            };
            let id = StructuralTypeId::new(structural_types.len() as u64 + 1).ok_or_else(|| {
                refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Type("structural type identity overflow"),
                )
            })?;
            structural_types.push(StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id,
                identity: identities.intern(home),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
                scalar_restriction: None,
            });
            primitives.push((scalar_type, id));
        }
    }
    Ok(Declared {
        records: Records {
            types,
            primitives,
            field_identities,
            payload_identities,
        },
        identities: identities.build(),
        structural_types,
        native_layouts,
    })
}

/// The scalar homes the loans in `statement` give SSA locals, with the text
/// identity of each home's type (`ExpressionKind::Borrow::home`).
fn statement_homes<'b>(
    statement: &'b Statement,
    homes: &mut Vec<(ScalarType, Option<&'b String>)>,
) {
    match statement {
        Statement::Bind { value, .. }
        | Statement::Evaluate(value)
        | Statement::Store { value, .. } => expression_homes(value, homes),
        Statement::Declare { .. } | Statement::Witness { .. } => {}
        Statement::Dispatch {
            subject,
            arms,
            default,
            ..
        } => {
            expression_homes(subject, homes);
            for (_, arm, _) in arms {
                target_homes(arm, homes);
            }
            if let Some((arm, _)) = default {
                target_homes(arm, homes);
            }
        }
        Statement::Arm { guard, target, .. } => {
            if let Some(guard) = guard {
                expression_homes(guard, homes);
            }
            target_homes(target, homes);
        }
    }
}

fn target_homes<'b>(target: &'b Target, homes: &mut Vec<(ScalarType, Option<&'b String>)>) {
    if let Target::Value(value) = target {
        expression_homes(value, homes);
    }
}

fn expression_homes<'b>(
    expression: &'b Expression,
    homes: &mut Vec<(ScalarType, Option<&'b String>)>,
) {
    match &expression.kind {
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::MachineValue { .. }
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Borrow { place, home, .. } => {
            if let (Some(home), Some(ValueType::Scalar(scalar_type))) = (home, place.result) {
                homes.push((scalar_type, Some(home)));
            }
            expression_homes(place, homes);
        }
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        } => expression_homes(operand, homes),
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, operand) in fields {
                expression_homes(operand, homes);
            }
        }
        ExpressionKind::Match {
            subject,
            arms,
            default,
        } => {
            expression_homes(subject, homes);
            for (_, arm) in arms {
                expression_homes(arm, homes);
            }
            if let Some(arm) = default {
                expression_homes(arm, homes);
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression_homes(left, homes);
            expression_homes(right, homes);
        }
        ExpressionKind::Call { callee, arguments } => {
            if let crate::body::CallTarget::Selected { callable, .. } = callee {
                expression_homes(callable, homes);
            }
            for argument in arguments {
                expression_homes(argument, homes);
            }
        }
    }
}

/// The Terminal access of a structural argument or parameter that lends
/// with `access`; `None` for write-only access, which no slice lends yet.
pub(crate) fn borrowed_access(access: language_core::ReferenceAccess) -> Option<StructuralAccess> {
    match access {
        language_core::ReferenceAccess::Shared => Some(StructuralAccess::SharedBorrow),
        language_core::ReferenceAccess::Mutable => Some(StructuralAccess::MutableBorrow),
        language_core::ReferenceAccess::WriteOnly => None,
    }
}

impl Records<'_> {
    pub(super) fn multiplicity(
        &self,
        record: StructuralTypeId,
        machine: MachineId,
    ) -> Checked<StructuralMultiplicity> {
        if let Some(record) = self.types.get(record) {
            return Ok(record.multiplicity);
        }
        if self
            .primitives
            .iter()
            .any(|(_, primitive)| *primitive == record)
        {
            return Ok(StructuralMultiplicity::Unrestricted);
        }
        Err(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine,
            concerns: Construct::Machine,
            detail: String::from("a structural place has no declared multiplicity"),
        })
    }

    /// The primitive structural type `declare` declared for `scalar_type`.
    pub(super) fn primitive(
        &self,
        scalar_type: ScalarType,
        machine: MachineId,
    ) -> Checked<StructuralTypeId> {
        self.primitives
            .iter()
            .find(|(known, _)| *known == scalar_type)
            .map(|(_, home)| *home)
            .ok_or_else(|| Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine,
                concerns: Construct::Machine,
                detail: String::from("a scalar place's primitive type is not declared"),
            })
    }
}

/// What a slot holds: a scalar slot the value bound to it, a record slot the
/// root place of the value its formal, initializer or last whole assignment
/// established, and a scalar local declared without an initializer the root
/// place that is its home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bound {
    Scalar(ValueId),
    Record(PlaceId),
    Home(PlaceId),
    /// An `[erased]` formal or local: the term it names. Nothing evaluates
    /// it and no runtime expression may read it.
    Erased(Witness),
}

/// The term an erased binding names: a value of this machine or a literal,
/// the forms `ScalarTerm` states with no formation condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Witness {
    Value(ValueId, ScalarType),
    Integer(
        semantic_vocabulary::IntegerType,
        semantic_vocabulary::IntegerValue,
    ),
    Boolean(bool),
}

impl Witness {
    pub(super) fn term(self) -> semantic_vocabulary::ScalarTerm {
        use semantic_vocabulary::ScalarTerm;
        match self {
            Self::Value(id, scalar_type) => ScalarTerm::Value { id, scalar_type },
            Self::Integer(scalar_type, value) => ScalarTerm::Integer { scalar_type, value },
            Self::Boolean(value) => ScalarTerm::Boolean(value),
        }
    }
}

/// One step of a path beneath a root: a field of the record `owner`, or the
/// payload field of one case of the sum `owner`. A payload step is two
/// segments of the place, `Case` then `Field`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Field {
        owner: StructuralTypeId,
        field: usize,
    },
    Payload {
        owner: StructuralTypeId,
        case: usize,
        field: usize,
    },
}

/// One dispatch arm the open block lies in. For a case arm: the sum place
/// the dispatch selected on, the case, and for each payload field in
/// declaration order the block parameter it arrived as, `None` for a data
/// payload field, which has no value identity. The wildcard arm has no case.
/// control_flow.md: "Selected payload fields become scalar parameters on
/// that case's edge before cleanup."
#[derive(Debug, Clone)]
pub(super) struct Chosen {
    place: ExactPlaceId,
    case: Option<usize>,
    parameters: Vec<Option<ValueId>>,
}

/// The storage a record-typed expression names, before anything is copied.
struct Located {
    root: PlaceId,
    place: ExactPlaceId,
    /// The steps from the root to the place.
    path: Vec<Step>,
    /// The record type of the place itself.
    record: StructuralTypeId,
    /// The root was produced by this expression's own operation, so no slot
    /// names it and it can be used as the value without a copy.
    fresh: bool,
}

/// An evaluated operand of a record literal.
#[derive(Clone, Copy)]
enum Operand {
    Scalar(ValueId),
    Record(PlaceId),
}

fn structural_result(
    place: PlaceId,
    record: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
) -> StructuralOperationResult {
    StructuralOperationResult {
        place,
        structural_type: record,
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    }
}

impl<'a> Lowering<'a> {
    pub(super) fn record_type(&self, record: StructuralTypeId) -> Checked<&'a RecordType> {
        terminal_check_tables::row(self.records.types, record, self.id, Construct::Machine)
    }

    /// Mints a root place: its Terminal `PlaceId`, its row and its `roots`
    /// entry, all three by `push` so they stay the same integer.
    pub(super) fn root(
        &mut self,
        kind: StructuralPlaceKind,
        record: StructuralTypeId,
    ) -> Checked<(PlaceId, ExactPlaceId)> {
        let place: PlaceId = self.roots.push((kind, record));
        let exact = self.tables.places.push(PlaceRow {
            origin: PlaceOrigin::Root(place),
            children: Vec::new(),
            writes: Vec::new(),
        });
        if self.tables.roots.push(exact) != place {
            return Err(self.internal("a root place and its table row disagree"));
        }
        Ok((place, exact))
    }

    pub(super) fn exact_root(&self, place: PlaceId) -> Checked<ExactPlaceId> {
        terminal_check_tables::row(&self.tables.roots, place, self.id, Construct::Machine).copied()
    }

    fn place_row(&mut self, place: ExactPlaceId) -> Checked<&mut PlaceRow> {
        let id = self.id;
        self.tables
            .places
            .get_mut(place)
            .ok_or(terminal_check_tables::Refusal {
                reason: terminal_check_tables::RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Place(place),
                detail: String::from("place is past the end of this machine"),
            })
    }

    /// The projection of `parent` by `field`, interned: one row per
    /// `(parent, field)` pair, found by binary search of the parent's sorted
    /// children.
    fn project(&mut self, parent: ExactPlaceId, segment: Segment) -> Checked<ExactPlaceId> {
        let found = self
            .place_row(parent)?
            .children
            .binary_search_by_key(&segment, |(segment, _)| *segment);
        match found {
            Ok(position) => Ok(self.place_row(parent)?.children[position].1),
            Err(position) => {
                let child = self.tables.places.push(PlaceRow {
                    origin: PlaceOrigin::Projection { parent, segment },
                    children: Vec::new(),
                    writes: Vec::new(),
                });
                self.place_row(parent)?
                    .children
                    .insert(position, (segment, child));
                Ok(child)
            }
        }
    }

    fn write(&mut self, place: ExactPlaceId, at: ProgramPoint, content: Content) -> Checked<()> {
        let write = self.tables.writes.push(WriteRow { place, at, content });
        self.place_row(place)?.writes.push(write);
        Ok(())
    }

    /// The value a formal's scalar leaf holds at entry, minted the first
    /// time it is asked for. fact_validity.md: "a valid initialized ordinary
    /// storage place at entry may supply its exact reaching value version; it
    /// need not have a store inside the current body."
    fn entry_content(&mut self, leaf: ExactPlaceId, scalar_type: ScalarType) -> Checked<ValueId> {
        let index = usize::try_from(leaf.get() - 1)
            .map_err(|_| self.internal("place identity exceeds the address space"))?;
        if self.entry_contents.len() <= index {
            self.entry_contents.resize(index + 1, None);
        }
        if let Some(value) = self.entry_contents[index] {
            return Ok(value);
        }
        let value = self.value(scalar_type, ValueOrigin::EntryContent { place: leaf });
        self.entry_contents[index] = Some(value);
        Ok(value)
    }

    /// Mints the entry content of every scalar leaf at or beneath `place`
    /// when `root` is a formal: an operation is about to name that storage,
    /// and what it held at entry is the value the checker's walk ends at on
    /// every path with no store. Other roots hold nothing at entry.
    fn name_entry(
        &mut self,
        root: PlaceId,
        place: ExactPlaceId,
        value_type: ValueType,
    ) -> Checked<()> {
        let (kind, _) =
            *terminal_check_tables::row(&self.roots, root, self.id, Construct::Place(place))?;
        match kind {
            StructuralPlaceKind::Parameter { .. } => self.entry_leaves(place, value_type),
            StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::Result
            | StructuralPlaceKind::OperationResult { .. }
            | StructuralPlaceKind::ByteSequenceLiteral { .. }
            | StructuralPlaceKind::ProviderAttachment { .. }
            | StructuralPlaceKind::TrivialAffineLocal { .. } => Ok(()),
        }
    }

    fn entry_leaves(&mut self, place: ExactPlaceId, value_type: ValueType) -> Checked<()> {
        match value_type {
            ValueType::Rigid(_) => Err(self.internal("a field of a rigid type")),
            ValueType::Scalar(scalar_type) => self.entry_content(place, scalar_type).map(drop),
            ValueType::Record(record) => {
                let data = self.record_type(record)?;
                for field in &data.fields {
                    let leaf = self.project(place, Segment::Field(field.id))?;
                    self.entry_leaves(leaf, field.value_type)?;
                }
                // Which case a formal holds is not known, so every case's
                // payload leaves get their entry content; the checker lets
                // a read reach one only where that case is established.
                for case in &data.cases {
                    let payload = self.project(place, Segment::Case(case.id))?;
                    for field in &case.fields {
                        let leaf = self.project(payload, Segment::Field(field.id))?;
                        self.entry_leaves(leaf, field.value_type)?;
                    }
                }
                Ok(())
            }
        }
    }

    /// The declared field or payload field a step selects.
    fn declared(&self, step: Step) -> Checked<&'a RecordField> {
        match step {
            Step::Field { owner, field } => self.record_type(owner)?.fields.get(field),
            Step::Payload { owner, case, field } => self
                .record_type(owner)?
                .cases
                .get(case)
                .and_then(|case| case.fields.get(field)),
        }
        .ok_or_else(|| self.internal("a field is past its declaration"))
    }

    fn case_of(&self, owner: StructuralTypeId, case: usize) -> Checked<&'a crate::body::CaseType> {
        self.record_type(owner)?
            .cases
            .get(case)
            .ok_or_else(|| self.internal("a case is past its sum's declaration"))
    }

    /// The place one step beneath `parent`, interned.
    fn step(&mut self, parent: ExactPlaceId, step: Step) -> Checked<ExactPlaceId> {
        let field = self.declared(step)?.id;
        let parent = match step {
            Step::Field { .. } => parent,
            Step::Payload { owner, case, .. } => {
                let case = self.case_of(owner, case)?.id;
                self.project(parent, Segment::Case(case))?
            }
        };
        self.project(parent, Segment::Field(field))
    }

    /// The Terminal Psi storage path of `steps`: each field by the interned
    /// identity its declaration carries, each case by its identity. It is
    /// written into operations, which is the form Terminal Psi gives them;
    /// nothing here compares it.
    fn storage_path(&self, steps: &[Step]) -> Checked<Vec<StructuralPathSegment>> {
        let mut path = Vec::with_capacity(steps.len());
        for step in steps {
            let identity = match *step {
                Step::Field { owner, field } => terminal_check_tables::row(
                    &self.records.field_identities,
                    owner,
                    self.id,
                    Construct::Machine,
                )?
                .get(field),
                Step::Payload { owner, case, field } => {
                    path.push(StructuralPathSegment::Case(self.case_of(owner, case)?.id));
                    terminal_check_tables::row(
                        &self.records.payload_identities,
                        owner,
                        self.id,
                        Construct::Machine,
                    )?
                    .get(case)
                    .and_then(|fields| fields.get(field))
                }
            };
            path.push(StructuralPathSegment::Field(*identity.ok_or_else(
                || self.internal("a field has no declared identity"),
            )?));
        }
        Ok(path)
    }

    /// Emits an operation whose result is a fresh record root.
    pub(super) fn produce(
        &mut self,
        operation: OperationId,
        record: StructuralTypeId,
        kind: OperationKind,
    ) -> Checked<(PlaceId, ExactPlaceId)> {
        let (place, exact) = self.root(
            StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: record,
            },
            record,
        )?;
        let multiplicity = self.records.multiplicity(record, self.id)?;
        self.draft()?.operations.push(Operation::unsuspended(
            operation,
            OperationResult::Structural(structural_result(place, record, multiplicity)),
            kind,
        ));
        self.point = Some(ProgramPoint::AfterOperation(operation));
        Ok((place, exact))
    }

    /// A call's record result holds values this machine cannot name.
    pub(super) fn unknown_contents(
        &mut self,
        root: ExactPlaceId,
        operation: OperationId,
    ) -> Checked<()> {
        self.write(
            root,
            ProgramPoint::AfterOperation(operation),
            Content::Overwritten,
        )
    }

    fn located(&mut self, expression: &Expression) -> Checked<Located> {
        let Some(ValueType::Record(record)) = expression.result else {
            return Err(self.internal("a scalar expression is used as a record"));
        };
        match &expression.kind {
            ExpressionKind::Slot(slot) => {
                let bound = *self.slot_mut(*slot)?;
                match bound {
                    Some(Bound::Record(root)) => Ok(Located {
                        root,
                        place: self.exact_root(root)?,
                        path: Vec::new(),
                        record,
                        fresh: false,
                    }),
                    Some(Bound::Scalar(_) | Bound::Home(_) | Bound::Erased(_)) => {
                        Err(self.internal("a scalar slot is used as a record"))
                    }
                    None => Err(self.internal("a slot is read before it is bound")),
                }
            }
            ExpressionKind::Field { base, field } => {
                let mut at = self.located(base)?;
                let step = Step::Field {
                    owner: at.record,
                    field: *field,
                };
                at.place = self.step(at.place, step)?;
                at.path.push(step);
                at.record = record;
                Ok(at)
            }
            ExpressionKind::Payload { base, case, field } => {
                let mut at = self.located(base)?;
                let step = Step::Payload {
                    owner: at.record,
                    case: *case,
                    field: *field,
                };
                at.place = self.step(at.place, step)?;
                at.path.push(step);
                at.record = record;
                Ok(at)
            }
            ExpressionKind::Record { fields } => {
                let (root, place) = self.establish(record, fields, expression.source)?;
                Ok(Located {
                    root,
                    place,
                    path: Vec::new(),
                    record,
                    fresh: true,
                })
            }
            ExpressionKind::Case { case, fields } => {
                let (root, place) =
                    self.establish_case(record, *case, fields, expression.source)?;
                Ok(Located {
                    root,
                    place,
                    path: Vec::new(),
                    record,
                    fresh: true,
                })
            }
            ExpressionKind::Call { callee, arguments } => {
                match self.call(callee, arguments, expression.result, expression.source)? {
                    Value::Record(root) => Ok(Located {
                        root,
                        place: self.exact_root(root)?,
                        path: Vec::new(),
                        record,
                        fresh: true,
                    }),
                    Value::Scalar(_) | Value::Unit => {
                        Err(self.internal("a record call produced no record"))
                    }
                }
            }
            ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::MachineValue { .. }
            | ExpressionKind::IsCase { .. }
            | ExpressionKind::Match { .. }
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::Cast(_)
            | ExpressionKind::ShortCircuit { .. } => {
                Err(self.internal("a scalar operation has a record type"))
            }
            ExpressionKind::Borrow { .. } => {
                Err(self.internal("a loan outside a call's arguments"))
            }
        }
    }

    /// Evaluates a record-typed expression to a root that holds its value
    /// and that nothing else names. A literal or call is its own fresh root;
    /// an unrestricted value is copied, and an affine whole value transfers
    /// its custody. Passing or returning a value uses that same operation.
    pub(super) fn record(&mut self, expression: &Expression) -> Checked<PlaceId> {
        let at = self.located(expression)?;
        if at.fresh && at.path.is_empty() {
            return Ok(at.root);
        }
        let path = self.storage_path(&at.path)?;
        self.name_entry(at.root, at.place, ValueType::Record(at.record))?;
        let multiplicity = self.record_type(at.record)?.multiplicity;
        let kind = match multiplicity {
            StructuralMultiplicity::Unrestricted => OperationKind::StructuralLeafCopy {
                source: at.root,
                path,
            },
            StructuralMultiplicity::Affine if path.is_empty() => {
                OperationKind::MoveStructuralValue {
                    source: StructuralArgument::whole(at.root, StructuralAccess::Owned),
                }
            }
            StructuralMultiplicity::Affine => {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Type("partial affine extraction"),
                ));
            }
            StructuralMultiplicity::Linear => {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Type("linear record claims"),
                ));
            }
        };
        let operation = self.operation_identity(expression.source);
        let (root, exact) = self.produce(operation, at.record, kind)?;
        // Value transport retains its exact source relationship in the
        // operation; this row records the destination's new contents.
        self.write(
            exact,
            ProgramPoint::AfterOperation(operation),
            Content::Overwritten,
        )?;
        Ok(root)
    }

    /// The place a loan lends, as the call passes it: the lent place's root
    /// and storage path with the loan's access, the place's interned row,
    /// and where its authority comes from. loans.md: "Loan formation
    /// captures a place, not a live selector expression."
    ///
    /// The place is a slot or a field path beneath one. A scalar local that
    /// is an SSA value (`needs_home`) first gets a home holding its current
    /// value, the operation the current pipeline emits for a local it lends
    /// (structural_access.md: "`EstablishPrimitiveLocal` consumes an exactly
    /// typed SSA initializer and establishes the referent"); the slot names
    /// that home from then on, so the reads after the call observe what the
    /// callee left there.
    pub(super) fn lend(
        &mut self,
        place: &Expression,
        access: language_core::ReferenceAccess,
        needs_home: bool,
    ) -> Checked<(StructuralArgument, ExactPlaceId, Lender)> {
        let lent_access =
            borrowed_access(access).ok_or_else(|| self.internal("a write-only loan"))?;
        let value_type = place
            .result
            .ok_or_else(|| self.internal("a loan of a Unit value"))?;
        let (root, steps, exact) = match (&place.kind, value_type) {
            (ExpressionKind::Slot(slot), ValueType::Scalar(scalar_type)) => {
                let bound = *self.slot_mut(*slot)?;
                let home = match bound {
                    Some(Bound::Home(home)) => home,
                    Some(Bound::Scalar(_)) if needs_home && self.joining > 0 => {
                        return Err(refuse(
                            self.id,
                            self.current.map_or(Construct::Machine, Construct::Block),
                            Unserved::Call("loan of a value local on some paths only"),
                        ));
                    }
                    Some(Bound::Scalar(value)) if needs_home => {
                        let home = self.establish_home(value, scalar_type, place.source)?;
                        *self.slot_mut(*slot)? = Some(Bound::Home(home));
                        home
                    }
                    _ => return Err(self.internal("a lent scalar slot holds no storage")),
                };
                (home, Vec::new(), self.exact_root(home)?)
            }
            (ExpressionKind::Field { base, field }, ValueType::Scalar(_)) => {
                let at = self.located(base)?;
                let step = Step::Field {
                    owner: at.record,
                    field: *field,
                };
                let leaf = self.step(at.place, step)?;
                let mut steps = at.path;
                steps.push(step);
                (at.root, steps, leaf)
            }
            (_, ValueType::Record(_)) => {
                let at = self.located(place)?;
                if at.fresh {
                    return Err(self.internal("a loan of a value no slot names"));
                }
                (at.root, at.path, at.place)
            }
            _ => return Err(self.internal("a loan of something that is not a place")),
        };
        if steps
            .iter()
            .any(|step| matches!(step, Step::Payload { .. }))
        {
            return Err(self.internal("a loan beneath a case payload"));
        }
        self.name_entry(root, exact, value_type)?;
        let lender = if self.borrowed_roots.contains(&root) {
            Lender::Incoming
        } else {
            Lender::Owner
        };
        Ok((
            StructuralArgument {
                place: root,
                path: self.storage_path(&steps)?,
                access: lent_access,
            },
            exact,
            lender,
        ))
    }

    /// A home for an SSA scalar holding `value`: one `EstablishPrimitiveLocal`
    /// and the row that says the home holds that value.
    fn establish_home(
        &mut self,
        value: ValueId,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<PlaceId> {
        let home = self.records.primitive(scalar_type, self.id)?;
        let operation = self.operation_identity(source);
        let (root, exact) = self.produce(
            operation,
            home,
            OperationKind::EstablishPrimitiveLocal {
                value,
                range_obligation: None,
            },
        )?;
        self.write(
            exact,
            ProgramPoint::AfterOperation(operation),
            Content::Value(value),
        )?;
        Ok(root)
    }

    /// `R { a: x, b: y }`: the operands in authored order, then one
    /// `EstablishRecord` whose roster is in declaration order
    /// (structural_access.md: "a complete roster of declaration-identified
    /// field operands"), then one write per field of the fresh root.
    fn establish(
        &mut self,
        record: StructuralTypeId,
        fields: &[(usize, Expression)],
        source: SourceOrigin,
    ) -> Checked<(PlaceId, ExactPlaceId)> {
        let declared = &self.record_type(record)?.fields;
        let mut operands: Vec<Option<Operand>> = vec![None; declared.len()];
        for (position, operand) in fields {
            let field = declared
                .get(*position)
                .ok_or_else(|| self.internal("a literal names a field past its record"))?;
            let evaluated = match field.value_type {
                ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                ValueType::Scalar(_) => Operand::Scalar(self.scalar(operand)?),
                ValueType::Record(_) => Operand::Record(self.record(operand)?),
            };
            if operands[*position].replace(evaluated).is_some() {
                return Err(self.internal("a literal names a field twice"));
            }
        }
        let mut roster = Vec::with_capacity(declared.len());
        let mut initializers = Vec::with_capacity(declared.len());
        for (field, operand) in declared.iter().zip(operands) {
            let operand = operand.ok_or_else(|| self.internal("a literal misses a field"))?;
            roster.push((field, operand));
            initializers.push(RecordFieldInitializer {
                field: field.id,
                value: match operand {
                    // A plain scalar field has no declared range, so it
                    // carries no obligation (structural_access.md:
                    // "unbounded fields must not carry one").
                    Operand::Scalar(value) => RecordFieldValue::Scalar {
                        value,
                        range_obligation: None,
                    },
                    Operand::Record(root) => RecordFieldValue::Structural(
                        StructuralArgument::whole(root, StructuralAccess::Owned),
                    ),
                },
            });
        }
        let operation = self.operation_identity(source);
        self.owe_domains(operation, self.record_type(record)?.requires.len(), source)?;
        let (root, exact) = self.produce(
            operation,
            record,
            OperationKind::EstablishRecord {
                fields: initializers,
            },
        )?;
        let at = ProgramPoint::AfterOperation(operation);
        for (field, operand) in roster {
            let leaf = self.project(exact, Segment::Field(field.id))?;
            match operand {
                Operand::Scalar(value) => self.write(leaf, at, Content::Value(value))?,
                // The operand root is named by the establishment's own
                // roster; the row says only that the field has new contents.
                Operand::Record(_) => self.write(leaf, at, Content::Overwritten)?,
            }
        }
        Ok((root, exact))
    }

    /// `S::C { p: x }`: the payload operands in authored order, then one
    /// establishment of the sum as that case (expressions.md: "Record or
    /// case literal | Authored field-expression order."). The operation is
    /// the one the current pipeline emits: `EstablishScalarCase` with no
    /// field for a case without payload, `EstablishStructuralCase` with the
    /// payload roster in declaration order otherwise. A plain payload field
    /// has no declared range, so it carries no obligation.
    ///
    /// The root gets one row for its new contents, then one per payload
    /// field. Which case it now holds is the operation's `result_case`.
    fn establish_case(
        &mut self,
        record: StructuralTypeId,
        case: usize,
        fields: &[(usize, Expression)],
        source: SourceOrigin,
    ) -> Checked<(PlaceId, ExactPlaceId)> {
        let selected = self.case_of(record, case)?;
        let declared = &selected.fields;
        let mut operands: Vec<Option<Operand>> = vec![None; declared.len()];
        for (position, operand) in fields {
            let field = declared
                .get(*position)
                .ok_or_else(|| self.internal("a literal names a field past its case"))?;
            let evaluated = match field.value_type {
                ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                ValueType::Scalar(_) => Operand::Scalar(self.scalar(operand)?),
                ValueType::Record(_) => Operand::Record(self.record(operand)?),
            };
            if operands[*position].replace(evaluated).is_some() {
                return Err(self.internal("a literal names a field twice"));
            }
        }
        let mut roster = Vec::with_capacity(declared.len());
        let mut initializers = Vec::with_capacity(declared.len());
        for (field, operand) in declared.iter().zip(operands) {
            let operand = operand.ok_or_else(|| self.internal("a literal misses a field"))?;
            roster.push((field, operand));
            initializers.push(RecordFieldInitializer {
                field: field.id,
                value: match operand {
                    Operand::Scalar(value) => RecordFieldValue::Scalar {
                        value,
                        range_obligation: None,
                    },
                    Operand::Record(root) => RecordFieldValue::Structural(
                        StructuralArgument::whole(root, StructuralAccess::Owned),
                    ),
                },
            });
        }
        let kind = if initializers.is_empty() {
            OperationKind::EstablishScalarCase {
                result_case: selected.id,
                fields: Vec::new(),
            }
        } else {
            OperationKind::EstablishStructuralCase {
                result_case: selected.id,
                fields: initializers,
            }
        };
        let operation = self.operation_identity(source);
        self.owe_domains(operation, self.record_type(record)?.requires.len(), source)?;
        let (root, exact) = self.produce(operation, record, kind)?;
        let at = ProgramPoint::AfterOperation(operation);
        self.write(exact, at, Content::Overwritten)?;
        if !roster.is_empty() {
            let payload = self.project(exact, Segment::Case(selected.id))?;
            for (field, operand) in roster {
                let leaf = self.project(payload, Segment::Field(field.id))?;
                match operand {
                    Operand::Scalar(value) => self.write(leaf, at, Content::Value(value))?,
                    Operand::Record(_) => self.write(leaf, at, Content::Overwritten)?,
                }
            }
        }
        Ok((root, exact))
    }

    /// `subject in S::C`: one `StructuralCaseMembership` on the sum the
    /// subject names, in place. structural_access.md: "An empty path
    /// observes the whole root. Relevant record fields ... select the
    /// original backing without copying or borrowing a child."
    pub(super) fn case_membership(
        &mut self,
        subject: &Expression,
        case: usize,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let at = self.located(subject)?;
        let case = self.case_of(at.record, case)?.id;
        let path = self.storage_path(&at.path)?;
        self.name_entry(at.root, at.place, ValueType::Record(at.record))?;
        let operation = self.operation_identity(source);
        self.emit(
            operation,
            Some(ScalarType::Boolean),
            OperationKind::StructuralCaseMembership {
                source: at.root,
                path,
                case,
            },
        )
    }

    /// Ends the open block in the `StructuralCase` that observes the sum
    /// `subject` names, and returns the block of each arm in `arms` order
    /// and of the wildcard arm. control_flow.md: "The complete declared case
    /// roster determines its successors independently of case count".
    ///
    /// The roster is built from the declaration's case table, one edge per
    /// declared case in declaration order: to the arm that names the case,
    /// else to the wildcard arm. A case with neither is a compile error of
    /// the program (patterns.md: "Every dispatch must cover the admitted
    /// subject. Missing runtime coverage is a compile error, not an
    /// implicit trap or fallthrough."), reported as `Unestablished` at the
    /// block. A wildcard that no case reaches would never be lowered, so it
    /// is refused rather than dropped.
    ///
    /// Each scalar payload field of an arm's case is one parameter of the
    /// arm's block, in declaration order.
    #[allow(clippy::type_complexity)]
    pub(super) fn select(
        &mut self,
        subject: &Expression,
        arms: &[usize],
        wildcard: bool,
        source: SourceOrigin,
    ) -> Checked<(Vec<(BlockId, Chosen)>, Option<(BlockId, Chosen)>)> {
        let at = self.located(subject)?;
        let data = self.record_type(at.record)?;
        if data.cases.is_empty() {
            return Err(self.internal("a dispatch on storage that is no sum"));
        }
        let path = self.storage_path(&at.path)?;
        self.name_entry(at.root, at.place, ValueType::Record(at.record))?;
        let mut blocks = Vec::with_capacity(arms.len());
        let mut payloads = Vec::with_capacity(arms.len());
        for case in arms {
            let selected = self.case_of(at.record, *case)?;
            let block = self.block(source);
            let mut parameters = Vec::with_capacity(selected.fields.len());
            let mut payload_fields = Vec::new();
            for field in &selected.fields {
                parameters.push(match field.value_type {
                    ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                    ValueType::Scalar(scalar_type) => {
                        let value = self.value(
                            scalar_type,
                            ValueOrigin::BlockParameter {
                                block,
                                position: self.position(payload_fields.len())?,
                            },
                        );
                        self.parameter(block, value, scalar_type)?;
                        payload_fields.push(field.id);
                        Some(value)
                    }
                    ValueType::Record(_) => None,
                });
            }
            blocks.push((
                block,
                Chosen {
                    place: at.place,
                    case: Some(*case),
                    parameters,
                },
            ));
            payloads.push(payload_fields);
        }
        let mut rest: Option<(BlockId, Chosen)> = None;
        let mut edges = Vec::with_capacity(data.cases.len());
        for (position, case) in data.cases.iter().enumerate() {
            let (target, payload_fields) = match arms.iter().position(|arm| *arm == position) {
                Some(arm) => (blocks[arm].0, payloads[arm].clone()),
                None if wildcard => {
                    let block = match &rest {
                        Some((block, _)) => *block,
                        None => {
                            let block = self.block(source);
                            rest = Some((
                                block,
                                Chosen {
                                    place: at.place,
                                    case: None,
                                    parameters: Vec::new(),
                                },
                            ));
                            block
                        }
                    };
                    (block, Vec::new())
                }
                None => {
                    return Err(Refusal {
                        reason: RefusalReason::Unestablished,
                        machine: self.id,
                        concerns: self.current.map_or(Construct::Machine, Construct::Block),
                        detail: String::from("a dispatch selects no arm for a declared case"),
                    });
                }
            };
            let edge = self.edge(Destination::Block(target), source)?;
            edges.push(StructuralCaseSuccessorEdge {
                edge,
                target,
                case: case.id,
                payload_fields,
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            });
        }
        if wildcard && rest.is_none() {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Transition("wildcard arm that no case reaches"),
            ));
        }
        self.terminate(Terminator::StructuralCase {
            source: at.root,
            path,
            cases: edges,
        })?;
        Ok((blocks, rest))
    }

    /// The scalar payload field `field` of case `case` of the sum `base`
    /// names. Inside the arm of a dispatch on that same place and case it
    /// is the arm block's parameter. Anywhere else it is a read whose
    /// carrier ends in the `Case` step (structural_access.md: "One `Case`
    /// step may end the carrier when the preceding carrier steps reach a
    /// sum. The final field is then a relevant scalar payload field of that
    /// exact declared case."), and whether the case is selected there is
    /// the checker's judgment.
    pub(super) fn read_payload(
        &mut self,
        base: &Expression,
        case: usize,
        field: usize,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let at = self.located(base)?;
        let bound = self
            .selected
            .iter()
            .rev()
            .find(|chosen| chosen.place == at.place && chosen.case == Some(case))
            .and_then(|chosen| chosen.parameters.get(field).copied().flatten());
        if let Some(value) = bound {
            return Ok(Value::Scalar(value));
        }
        // The one `Case` step a read's carrier may hold is its last.
        if at
            .path
            .iter()
            .any(|step| matches!(step, Step::Payload { .. }))
        {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("payload read beneath another case payload"),
            ));
        }
        let selected = self.case_of(at.record, case)?;
        let declared = selected
            .fields
            .get(field)
            .ok_or_else(|| self.internal("a field is past its case's declaration"))?;
        let mut path = self.canonical_path(&at.path)?;
        path.push(CanonicalStructuralPathSegment::Case(selected.id));
        let leaf = self.step(
            at.place,
            Step::Payload {
                owner: at.record,
                case,
                field,
            },
        )?;
        self.name_entry(at.root, leaf, ValueType::Scalar(scalar_type))?;
        self.read_scalar(at.root, path, declared, scalar_type, source)
    }

    fn canonical_path(&self, steps: &[Step]) -> Checked<Vec<CanonicalStructuralPathSegment>> {
        let mut path = Vec::with_capacity(steps.len());
        for step in steps {
            if let Step::Payload { owner, case, .. } = *step {
                path.push(CanonicalStructuralPathSegment::Case(
                    self.case_of(owner, case)?.id,
                ));
            }
            path.push(CanonicalStructuralPathSegment::Field(
                self.declared(*step)?.id,
            ));
        }
        Ok(path)
    }

    fn read_scalar(
        &mut self,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        declared: &RecordField,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let kind = match scalar_type {
            ScalarType::Boolean => OperationKind::BooleanStructuralField {
                source: root,
                path,
                field: declared.id,
            },
            ScalarType::Integer(_) => OperationKind::ScalarStructuralField {
                source: root,
                path,
                field: declared.id,
            },
            ScalarType::IeeeFloat(_) => {
                return Err(self.internal("a float field reached lowering"));
            }
            ScalarType::Callable(_) => {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Expression("callable field without a retained contract"),
                ));
            }
        };
        let operation = self.operation_identity(source);
        self.emit(operation, Some(scalar_type), kind)
    }

    /// `base.field` of a scalar field: one read operation whose result is a
    /// fresh value. structural_access.md: "`ScalarStructuralField` and
    /// `BooleanStructuralField` retain the whole source place, an ordered
    /// canonical carrier path, and the final declaration-local field ID. The
    /// carrier path excludes the final field".
    pub(super) fn read_field(
        &mut self,
        base: &Expression,
        field: usize,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let at = self.located(base)?;
        // A read through a payload names its `Case` step inside the
        // carrier, where the specification allows it only as the last step.
        if at
            .path
            .iter()
            .any(|step| matches!(step, Step::Payload { .. }))
        {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("field read beneath a case payload"),
            ));
        }
        let path = self.canonical_path(&at.path)?;
        let step = Step::Field {
            owner: at.record,
            field,
        };
        let declared = self.declared(step)?;
        let leaf = self.step(at.place, step)?;
        self.name_entry(at.root, leaf, ValueType::Scalar(scalar_type))?;
        self.read_scalar(at.root, path, declared, scalar_type, source)
    }

    /// `let x: T;`. The operations are the ones the current pipeline emits,
    /// so the module runs: a data home is `EstablishZeroedStructural` and a
    /// scalar home is `EstablishPrimitiveLocal` of a zero constant
    /// (structural_access.md: "`EstablishPrimitiveLocal` consumes an exactly
    /// typed SSA initializer and establishes the referent").
    ///
    /// A data local is logged as holding the zero value: one row for the
    /// root's new contents, then on each scalar leaf of the zero value a
    /// constant zero of the leaf's type. The zero value of a sum is its
    /// first declared case with a zeroed payload (data_and_literals.md:
    /// "The ordinary zero representation of a sum selects the first declared
    /// case and a recursively zeroed payload."), so only that case's payload
    /// leaves get a row. The current pipeline emits no constant for a data
    /// home; the constants here are what gives each leaf's content a value
    /// identity the module contains, as its own scalar home's constant does.
    /// The establishment owes every `requires` clause of the type and of
    /// each data-typed place of the zero value; whether they hold decides
    /// whether the storage is an established value (`lowered-discharge`).
    ///
    /// A scalar local is zero-constructed the same way
    /// (`local-without-initializer`): its home is logged as holding the
    /// zero constant it is established with. dependent_values.md: "If zero
    /// satisfies the default domain, the value is zero-constructible.
    /// Otherwise the type is gated: construction or checked qualification
    /// must establish the domain before observation." Every scalar type
    /// this route reads (`i8`..`u64`, `bool`) admits zero and a constrained
    /// scalar type is refused by the reader, so a scalar home owes no
    /// domain clause and is never gated.
    pub(super) fn declare_local(&mut self, slot: usize, source: SourceOrigin) -> Checked<()> {
        let bound = match self.slot_type(slot)? {
            ValueType::Rigid(_) => {
                return Err(self.internal("a local of a rigid type is declared"));
            }
            ValueType::Record(record) => {
                // The zero of each scalar leaf, as a constant the module
                // holds, so the row that says what the leaf holds names a
                // value. The constants come first: the establishment's
                // domain obligations are stated over them.
                let mut zeros = Vec::new();
                let mut owed = 0;
                self.zero_leaves(record, &mut Vec::new(), &mut zeros, &mut owed, source)?;
                let operation = self.operation_identity(source);
                self.owe_domains(operation, owed, source)?;
                let (root, exact) = self.produce(
                    operation,
                    record,
                    OperationKind::EstablishZeroedStructural {},
                )?;
                let at = ProgramPoint::AfterOperation(operation);
                self.write(exact, at, Content::Overwritten)?;
                for (segments, zero) in zeros {
                    let mut leaf = exact;
                    for segment in segments {
                        leaf = self.project(leaf, segment)?;
                    }
                    self.write(leaf, at, Content::Value(zero))?;
                }
                Bound::Record(root)
            }
            ValueType::Scalar(scalar_type) => {
                let home = self
                    .records
                    .primitives
                    .iter()
                    .find(|(known, _)| *known == scalar_type)
                    .map(|(_, home)| *home)
                    .ok_or_else(|| self.internal("a homed scalar type is not declared"))?;
                let value = self.zero(scalar_type, source)?;
                let operation = self.operation_identity(source);
                let (root, exact) = self.produce(
                    operation,
                    home,
                    OperationKind::EstablishPrimitiveLocal {
                        value,
                        range_obligation: None,
                    },
                )?;
                // The home holds the zero it was established with: a local
                // declared without an initializer is zero-constructed, and a
                // builtin scalar admits zero, so nothing is owed here.
                self.write(
                    exact,
                    ProgramPoint::AfterOperation(operation),
                    Content::Value(value),
                )?;
                Bound::Home(root)
            }
        };
        *self.slot_mut(slot)? = Some(bound);
        Ok(())
    }

    /// A fresh read of a scalar local's home. structural_access.md:
    /// "`PrimitiveScalarRead` produces a fresh scalar observation of an
    /// established local ... A read is not equality with the initializer or
    /// a previous read: intervening stores and calls can change storage."
    pub(super) fn read_home(
        &mut self,
        home: PlaceId,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // A borrowed scalar formal's place holds the caller's value at entry.
        let place = self.exact_root(home)?;
        self.name_entry(home, place, ValueType::Scalar(scalar_type))?;
        let operation = self.operation_identity(source);
        self.emit(
            operation,
            Some(scalar_type),
            OperationKind::PrimitiveScalarRead {
                source: home,
                path: Vec::new(),
            },
        )
    }

    /// `slot.path = value;`: the value, then one store into exactly the
    /// place the path selects, and one row saying what that place now holds.
    /// Sibling places get no row, so they keep their contents; no earlier
    /// row is edited, so what was known about the old value stays known.
    ///
    /// A scalar field is `StructuralScalarFieldStore`, whose path is the
    /// carrier and whose field is the leaf (structural_access.md: "An empty
    /// carrier path denotes a common field directly on the root"); a plain
    /// field has no declared range, so it carries no obligation ("unbounded
    /// fields must not carry one"). A record field is the ordinary
    /// structural replacement, `StoreStructuralField`, of a root that holds
    /// the already constructed value ("A whole record assignment constructs
    /// its operands before replacing the destination through the ordinary
    /// structural replacement operation ... Root, field, and indexed
    /// destinations do not change construction semantics or decompose the
    /// assignment into member stores"). A homed scalar local is
    /// `WriteOnlyPrimitiveStore` with an empty path.
    pub(super) fn store(
        &mut self,
        slot: usize,
        path: &[usize],
        value: &Expression,
        source: SourceOrigin,
    ) -> Checked<()> {
        let bound = *self.slot_mut(slot)?;
        let (root, last) = match (bound, path.split_last()) {
            (Some(Bound::Home(home)), None) => {
                let stored = self.scalar(value)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    None,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination: home,
                        path: Vec::new(),
                        value: stored,
                        range_obligation: None,
                    },
                )?;
                let place = self.exact_root(home)?;
                return self.write(
                    place,
                    ProgramPoint::AfterOperation(operation),
                    Content::Value(stored),
                );
            }
            (Some(Bound::Record(root)), Some(last)) => (root, last),
            (Some(Bound::Scalar(_) | Bound::Home(_) | Bound::Erased(_)), Some(_)) => {
                return Err(self.internal("a field of a scalar slot is stored to"));
            }
            (Some(Bound::Scalar(_) | Bound::Record(_) | Bound::Erased(_)), None) => {
                return Err(self.internal("a whole store into a slot that is not a home"));
            }
            (None, _) => return Err(self.internal("a slot is stored to before it is bound")),
        };
        let (_, mut record) =
            *terminal_check_tables::row(&self.roots, root, self.id, Construct::Machine)?;
        let mut place = self.exact_root(root)?;
        let (final_field, carrier) = last;
        let mut steps = Vec::with_capacity(carrier.len());
        for position in carrier {
            let step = Step::Field {
                owner: record,
                field: *position,
            };
            let declared = self.declared(step)?;
            steps.push(step);
            place = self.step(place, step)?;
            record = match declared.value_type {
                ValueType::Record(nested) => nested,
                ValueType::Scalar(_) | ValueType::Rigid(_) => {
                    return Err(self.internal("a store path crosses a scalar field"));
                }
            };
        }
        let storage = self.storage_path(&steps)?;
        let last = Step::Field {
            owner: record,
            field: *final_field,
        };
        // Every type that owns a step of the written path owes its clauses:
        // the type of the field's own record and each type that contains it.
        let mut owed = self.record_type(record)?.requires.len();
        for step in &steps {
            let Step::Field { owner, .. } = step else {
                return Err(self.internal("a store path crosses a case payload"));
            };
            owed += self.record_type(*owner)?.requires.len();
        }
        let declared = self.declared(last)?;
        let place = self.step(place, last)?;
        if value.result != Some(declared.value_type) {
            return Err(self.internal("a stored value has another type than its field"));
        }
        match declared.value_type {
            ValueType::Rigid(_) => Err(self.internal("a field of a rigid type")),
            ValueType::Scalar(_) => {
                let stored = self.scalar(value)?;
                let operation = self.operation_identity(source);
                self.owe_domains(operation, owed, source)?;
                self.emit(
                    operation,
                    None,
                    OperationKind::StructuralScalarFieldStore {
                        destination: root,
                        path: storage,
                        field: declared.id,
                        value: stored,
                        range_obligation: None,
                    },
                )?;
                self.write(
                    place,
                    ProgramPoint::AfterOperation(operation),
                    Content::Value(stored),
                )
            }
            ValueType::Record(_) => {
                let constructed = self.record(value)?;
                let operation = self.operation_identity(source);
                self.owe_domains(operation, owed, source)?;
                self.emit(
                    operation,
                    None,
                    OperationKind::StoreStructuralField {
                        disposition: StructuralStoreDisposition::Vacant,
                        destination: root,
                        path: storage,
                        field: Some(declared.id),
                        value: StructuralArgument::whole(constructed, StructuralAccess::Owned),
                    },
                )?;
                // The operation names the root the value came from; the row
                // says only that the field has new contents.
                self.write(
                    place,
                    ProgramPoint::AfterOperation(operation),
                    Content::Overwritten,
                )
            }
        }
    }

    /// A constant zero of a scalar type.
    fn zero(&mut self, scalar_type: ScalarType, source: SourceOrigin) -> Checked<ValueId> {
        let kind = match scalar_type {
            ScalarType::Boolean => OperationKind::BooleanConstant { value: false },
            ScalarType::Integer(integer) => OperationKind::IntegerConstant {
                value: IntegerValue::for_sign(integer.sign(), 0)
                    .map_err(|_| self.internal("zero is not an integer value"))?,
            },
            ScalarType::IeeeFloat(_) => {
                return Err(self.internal("a float local reached lowering"));
            }
            ScalarType::Callable(_) => {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Expression("zero-initialized callable"),
                ));
            }
        };
        let operation = self.operation_identity(source);
        match self.emit(operation, Some(scalar_type), kind)? {
            Value::Scalar(value) => Ok(value),
            Value::Record(_) | Value::Unit => Err(self.internal("a constant produced no value")),
        }
    }

    /// Emits the zero constant of every scalar leaf of the zero value of
    /// `record`, each with its path beneath the value, and counts the
    /// `requires` clauses the zero value owes: the type's own, then those of
    /// each data-typed field in declaration order, parent first. A sum's
    /// zero value is its first declared case, so only that case's payload
    /// is walked.
    fn zero_leaves(
        &mut self,
        record: StructuralTypeId,
        prefix: &mut Vec<Segment>,
        zeros: &mut Vec<(Vec<Segment>, ValueId)>,
        owed: &mut usize,
        source: SourceOrigin,
    ) -> Checked<()> {
        let data = self.record_type(record)?;
        *owed += data.requires.len();
        let (fields, case) = match data.cases.first() {
            Some(first) => (&first.fields, Some(first.id)),
            None => (&data.fields, None),
        };
        if let Some(case) = case {
            prefix.push(Segment::Case(case));
        }
        for field in fields {
            prefix.push(Segment::Field(field.id));
            match field.value_type {
                ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                ValueType::Scalar(scalar_type) => {
                    let zero = self.zero(scalar_type, source)?;
                    zeros.push((prefix.clone(), zero));
                }
                ValueType::Record(nested) => {
                    self.zero_leaves(nested, prefix, zeros, owed, source)?;
                }
            }
            prefix.pop();
        }
        if case.is_some() {
            prefix.pop();
        }
        Ok(())
    }

    /// Mints the `DataDomain` obligations of `operation`, one per clause it
    /// owes, at the point before it. data_and_literals.md: "Construction and
    /// every restoring write prove the owning default-domain clauses."
    fn owe_domains(
        &mut self,
        operation: OperationId,
        clauses: usize,
        source: SourceOrigin,
    ) -> Checked<()> {
        let before = self.here()?;
        for clause in 0..clauses {
            let clause = self.position(clause)?;
            self.obligation(
                ObligationKind::DataDomain { operation, clause },
                before,
                source,
            );
        }
        Ok(())
    }

    /// Ends the block after a call that consumed record arguments and
    /// retires their homes on the edge into its continuation. ownership.md:
    /// "A dying temporary's normal continuation owns its remaining cleanup.
    /// ... a continuing body uses an explicit continuation edge." and "The
    /// checked dying-owner partition publishes `RetireHome` after its
    /// complete ordered region actions, including an empty complement".
    pub(super) fn retire_homes(&mut self, homes: &[PlaceId], source: SourceOrigin) -> Checked<()> {
        let continuation = self.block(SourceOrigin::Unpositioned(Unpositioned::ArgumentRetirement));
        let edge = self.edge(Destination::Block(continuation), source)?;
        self.terminate(Terminator::Jump {
            edge,
            target: continuation,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: homes
                .iter()
                .copied()
                .map(TerminalAffineCleanupAction::RetireHome)
                .collect(),
        })?;
        self.enter(continuation);
        Ok(())
    }
}
