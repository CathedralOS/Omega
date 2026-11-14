/*
The place family of lowering: structural roots, projections interned on first
use, the forward write log, record construction, field reads, stores, whole
copies, affine transfers and local storage homes. This is where
`MachineTables::places`, `roots` and `writes` are filled, in lockstep with
the Terminal Psi operations that create or change the storage.

Roots. One Terminal `PlaceId` and one `PlaceRow { origin: Root }` per record
formal, per operation that produces storage (a literal, a whole copy, a call
with a record result, the home of a local declared without an initializer)
and for a record machine's result place; `roots` maps the `PlaceId` to its
row. An affine named local reserves a distinct home and receives its
initializer through a checked whole store. A continuing transfer retains
an explicit continuation even when whole transport consumed its source home.
Projected moves leave a home whose residuals and retirement belong to its
normal continuation. A copy local can
retain its initializer's unrestricted root. ownership.md: "Parameter,
named-local, and call-result roots retain distinct establishment identities."

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
  a read (`terminal-psi-to-terminal-psi`, `places`).
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

use crate::body::Qualification;
use semantic_vocabulary::{
    BlockId, IdTable, IntegerValue, MachineId, OperationId, PlaceId, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment, WriteRow};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::{ExecutionSite, Lender, ObligationKind};
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
use terminal_semantics::CollectionExtent;

use super::{Lowering, Value};
use crate::body::{
    Body, Expression, ExpressionKind, RecordField, RecordType, Statement, StorageShape, Target,
    ValueType,
};
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};

/// The module's record types as lowering needs them: the reader's table and,
/// beside it, the interned identity Terminal Psi's declarations and
/// structural paths name each field by.
pub(crate) struct Records<'a> {
    types: &'a IdTable<StructuralTypeId, RecordType>,
    /// Scalar homes retain their exact type identity and qualification. An
    /// array element of the same scalar width need not be the same home type.
    primitives: Vec<(ValueType, String, StructuralTypeId)>,
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
/// Copy permission follows the declaration; affine plain data is never
/// promoted to copyable because its fields happen to be scalars.
/// `no_code_affine_cleanup`
/// because its "represented owned contents require no executable disposal"
/// and `descriptor_cleanup` because it is "a closed value containing only
/// owned storage and reference descriptors, with no nominal cleanup hook or
/// conserved claim" (`wiki/spec/terminal-psi/structural_access.md`); it owns
/// no linear claim and authors no layout.
///
/// A scalar local declared without an initializer is homed in a place of
/// primitive structural type, so each scalar type such a local has is
/// declared too, after the records, as the current pipeline declares it:
/// copyable, with no cleanup and no claim.
pub(crate) fn declare<'a>(
    entry: MachineId,
    types: &'a IdTable<StructuralTypeId, RecordType>,
    bodies: &IdTable<MachineId, Body>,
    signatures: crate::body::Signatures<'_>,
    mut identities: TerminalIdentityTableBuilder,
) -> Checked<Declared<'a>> {
    let mut field_identities = IdTable::default();
    let mut structural_types = Vec::with_capacity(types.len());
    let mut native_layouts = NativeLayoutCatalog::default();
    let mut payload_identities = IdTable::default();
    for (id, record) in types.iter() {
        let mut declare_fields = |declared: &[RecordField]| {
            let mut indexes = Vec::with_capacity(declared.len());
            let mut fields = Vec::with_capacity(declared.len());
            for field in declared {
                let identity = identities.intern(&field.identity);
                indexes.push(identity);
                let mut inline_carrier = None;
                let field_type = match field.value_type {
                    // The reader refuses a data type with such a field.
                    ValueType::Rigid(_) => {
                        return Err(refuse(
                            entry,
                            Construct::Machine,
                            Unserved::Type("generic data"),
                        ));
                    }
                    ValueType::Scalar(scalar_type, _) => StructuralFieldType::Scalar(scalar_type),
                    ValueType::Record(nested, _, _) => {
                        // A data type holds only types declared before it,
                        // so the table is acyclic and in dependency order.
                        if nested.get() >= id.get() {
                            return Err(refuse(
                                entry,
                                Construct::Machine,
                                Unserved::Type("record field of a later record type"),
                            ));
                        }
                        // A stored byte view is a byte field of its record,
                        // not a nested value of a view type
                        // (structural_access.md, "Store vocabulary": the
                        // field is replaced by `StructuralByteViewFieldStore`
                        // and "Zero-initialized storage holds the empty
                        // shared view"). Only the shared view is storable:
                        // "Qualified mutable, write-only, and
                        // access-unspecified view fields remain unsupported".
                        match types.get(nested).map(|row| &row.shape) {
                            Some(crate::StorageShape::ByteSequence(carrier))
                                if is_stored_view(*carrier) =>
                            {
                                // The view type the field was read as stays
                                // the carrier a copy of the descriptor has:
                                // it names the region the view is valid for.
                                inline_carrier = Some(nested);
                                // A view in a domain keeps its primitive
                                // predicates on the field's type, where every
                                // store into it is judged against them.
                                if field.byte_predicates.is_empty() {
                                    StructuralFieldType::ByteSequence(*carrier)
                                } else {
                                    StructuralFieldType::PredicateByteSequence {
                                        carrier: *carrier,
                                        predicates: field.byte_predicates,
                                    }
                                }
                            }
                            Some(crate::StorageShape::ByteSequence(_)) => {
                                return Err(refuse(
                                    entry,
                                    Construct::Machine,
                                    Unserved::Type("stored view that is not a shared byte view"),
                                ));
                            }
                            _ => StructuralFieldType::Structural(nested),
                        }
                    }
                };
                fields.push(StructuralFieldDeclaration {
                    inline_carrier,
                    id: field.id,
                    identity,
                    relevance: language_core::BindingRelevance::Relevant,
                    field_type,
                    storage: None,
                });
            }
            Ok((indexes, fields))
        };
        let (indexes, fields) = declare_fields(record.fields())?;
        let mut case_indexes = Vec::with_capacity(record.cases().len());
        let mut cases = Vec::with_capacity(record.cases().len());
        for case in record.cases() {
            let (indexes, fields) = declare_fields(&case.fields)?;
            case_indexes.push(indexes);
            cases.push((case, fields));
        }
        field_identities.push(indexes);
        payload_identities.push(case_indexes);
        let shape = match &record.shape {
            crate::StorageShape::Record(_) => {
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
            }
            crate::StorageShape::Sum(_) => StructuralTypeShape::Sum {
                cases: cases
                    .into_iter()
                    .map(|(case, fields)| StructuralCaseDeclaration {
                        id: case.id,
                        identity: identities.intern(&case.identity),
                        fields,
                    })
                    .collect(),
            },
            crate::StorageShape::FixedArray { element, length } => {
                if element.get() >= id.get() || length.literal().is_none() {
                    return Err(refuse(
                        entry,
                        Construct::Machine,
                        Unserved::Type("array with an unresolved element or extent"),
                    ));
                }
                StructuralTypeShape::FixedArray {
                    element: *element,
                    length: *length,
                }
            }
            crate::StorageShape::PrimitiveScalar { scalar_type, .. } => {
                StructuralTypeShape::PrimitiveScalar(*scalar_type)
            }
            crate::StorageShape::ByteSequence(carrier) => {
                StructuralTypeShape::ByteSequence(*carrier)
            }
            crate::StorageShape::ElementView { element, access } => {
                if element.get() >= id.get() {
                    return Err(refuse(
                        entry,
                        Construct::Machine,
                        Unserved::Type("sequence with an unresolved element"),
                    ));
                }
                StructuralTypeShape::ElementView {
                    element: *element,
                    access: *access,
                }
            }
        };
        structural_types.push(StructuralTypeDeclaration {
            requires: record.requires.clone(),
            quotient_carrier: None,
            id,
            identity: identities.intern(&record.identity),
            owns_linear_claim: false,
            copyable: record.copyable,
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
    let mut homes: Vec<(ValueType, Option<&String>)> = Vec::new();
    for signature in signatures
        .machines
        .values()
        .chain(signatures.boundaries.values())
    {
        for borrowed in &signature.borrowed {
            if let Some(value_type @ ValueType::Scalar(..)) =
                signature.parameters.get(borrowed.position).copied()
            {
                homes.push((value_type, borrowed.home.as_ref()));
            }
        }
    }
    for (_, body) in bodies.iter() {
        for statement in &body.statements {
            statement_homes(statement, &mut homes);
            let Statement::Declare { slot, home, .. } = statement else {
                continue;
            };
            let Some(value_type @ ValueType::Scalar(..)) = body.slots.get(*slot).copied() else {
                continue;
            };
            homes.push((value_type, home.as_ref()));
        }
    }
    let mut primitives: Vec<(ValueType, String, StructuralTypeId)> = types
        .iter()
        .filter_map(|(id, row)| match row.shape {
            crate::StorageShape::PrimitiveScalar {
                scalar_type,
                policy,
            } => Some((
                policy.map_or(
                    ValueType::Scalar(scalar_type, Qualification::NONE),
                    |policy| ValueType::Scalar(scalar_type, Qualification::of_policy(policy)),
                ),
                row.identity.clone(),
                id,
            )),
            _ => None,
        })
        .collect();
    for (value_type, home) in homes {
        let Some(home) = home else {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Spelling("scalar home without a type identity"),
            ));
        };
        let scalar_type = value_type.scalar().ok_or_else(|| {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Type("a primitive home without a scalar type"),
            )
        })?;
        let mut matching = primitives
            .iter()
            .filter(|(_, identity, _)| identity == home);
        if let Some((known, _, _)) = matching.next() {
            if matching.next().is_some() || *known != value_type {
                return Err(refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Type("a scalar home identity has conflicting declarations"),
                ));
            }
            continue;
        }
        let id = declare_scalar_type(&mut structural_types, identities.intern(home), scalar_type)
            .map_err(|reason| refuse(entry, Construct::Machine, Unserved::Type(reason)))?;
        primitives.push((value_type, home.clone(), id));
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

/// The one view a record field stores: a shared borrowed byte view.
fn is_stored_view(carrier: terminal_psi::ByteSequenceCarrier) -> bool {
    carrier
        == terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }
}

// Calling storage and scalar homes name the same primitive structural type.
// Publish it here once; a matching identity with another shape is a broken
// correspondence, never permission to relabel that existing declaration.
pub(crate) fn declare_scalar_type(
    types: &mut Vec<StructuralTypeDeclaration>,
    identity: TerminalIdentityIndex,
    scalar_type: ScalarType,
) -> Result<StructuralTypeId, &'static str> {
    if identity.is_absent() {
        return Err("primitive structural type has no exact identity");
    }
    let mut matching = types
        .iter()
        .filter(|declared| declared.identity == identity);
    if let Some(declared) = matching.next() {
        if matching.next().is_some()
            || declared.shape != StructuralTypeShape::PrimitiveScalar(scalar_type)
        {
            return Err("primitive structural type has ambiguous or different storage");
        }
        return Ok(declared.id);
    }
    append_scalar_type(types, identity, scalar_type)
}

fn append_scalar_type(
    types: &mut Vec<StructuralTypeDeclaration>,
    identity: TerminalIdentityIndex,
    scalar_type: ScalarType,
) -> Result<StructuralTypeId, &'static str> {
    let ordinal = u64::try_from(types.len())
        .ok()
        .and_then(|length| length.checked_add(1))
        .ok_or("structural type identity overflow")?;
    let id = StructuralTypeId::new(ordinal).ok_or("structural type identity overflow")?;
    types.push(StructuralTypeDeclaration {
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        scalar_restriction: None,
    });
    Ok(id)
}

/// The scalar homes the loans in `statement` give SSA locals, with the text
/// identity of each home's type (`ExpressionKind::Borrow::home`).
fn statement_homes<'b>(statement: &'b Statement, homes: &mut Vec<(ValueType, Option<&'b String>)>) {
    match statement {
        Statement::Bind { value, .. } | Statement::Evaluate(value) => {
            expression_homes(value, homes)
        }
        Statement::Store {
            destination, value, ..
        } => {
            expression_homes(destination, homes);
            expression_homes(value, homes);
        }
        Statement::Declare { .. }
        | Statement::Witness { .. }
        | Statement::CaptureReference { .. } => {}
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

fn target_homes<'b>(target: &'b Target, homes: &mut Vec<(ValueType, Option<&'b String>)>) {
    if let Target::Value(value) = target {
        expression_homes(value, homes);
    }
}

fn expression_homes<'b>(
    expression: &'b Expression,
    homes: &mut Vec<(ValueType, Option<&'b String>)>,
) {
    match &expression.kind {
        ExpressionKind::SavedSubject(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Borrow { place, home, .. } => {
            if let (Some(home), Some(value_type @ ValueType::Scalar(..))) = (home, place.result) {
                homes.push((value_type, Some(home)));
            }
            expression_homes(place, homes);
        }
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Establish { operand, .. }
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::CollectionMeasure { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        } => expression_homes(operand, homes),
        ExpressionKind::Array { elements } => {
            for element in elements {
                expression_homes(element, homes);
            }
        }
        ExpressionKind::Index { base, index } => {
            expression_homes(base, homes);
            expression_homes(index, homes);
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, operand) in fields {
                expression_homes(operand, homes);
            }
        }
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
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
        ExpressionKind::Call { arguments, .. } => {
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
        if self
            .primitives
            .iter()
            .any(|(_, _, primitive)| *primitive == record)
        {
            return Ok(StructuralMultiplicity::Unrestricted);
        }
        let record = terminal_check_tables::row(self.types, record, machine, Construct::Machine)?;
        Ok(if record.copyable {
            StructuralMultiplicity::Unrestricted
        } else {
            StructuralMultiplicity::Affine
        })
    }
    /// Resolve the exact scalar home already published for this source type.
    pub(super) fn primitive(
        &self,
        value_type: ValueType,
        identity: Option<&str>,
        machine: MachineId,
    ) -> Checked<StructuralTypeId> {
        let mut matching = self
            .primitives
            .iter()
            .filter(|(known, home, _)| *known == value_type && Some(home.as_str()) == identity);
        let found = matching.next();
        if let Some((_, _, home)) = found
            && matching.next().is_none()
        {
            return Ok(*home);
        }
        Err(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine,
            concerns: Construct::Machine,
            detail: String::from("a scalar place's exact primitive type is not uniquely declared"),
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
pub(super) enum Step {
    Index {
        owner: StructuralTypeId,
        selector: ValueId,
        fixed: Option<u64>,
    },
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
    capture: Option<(
        arena::Handle<typed_trees::expression::CompletedValueCapture>,
        Located,
    )>,
    place: ExactPlaceId,
    case: Option<usize>,
    parameters: Vec<Option<ValueId>>,
}

/// The storage a record-typed expression names, before anything is copied.
#[derive(Debug, Clone)]
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

fn owned_result(
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

    /// Whether a field of this type is a byte field of its record, holding
    /// a shared view's descriptor (`declare`).
    fn is_stored_view(&self, record: StructuralTypeId) -> Checked<bool> {
        Ok(matches!(self.record_type(record)?.shape,
            StorageShape::ByteSequence(carrier) if is_stored_view(carrier)))
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
            ValueType::Scalar(scalar_type, _) => self.entry_content(place, scalar_type).map(drop),
            ValueType::Record(record, _, _) => {
                let data = self.record_type(record)?;
                if matches!(data.shape, StorageShape::FixedArray { .. }) {
                    return Ok(());
                }
                if let StorageShape::PrimitiveScalar { scalar_type, .. } = data.shape {
                    return self.entry_content(place, scalar_type).map(drop);
                }
                if matches!(
                    data.shape,
                    StorageShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        ),
                    })
                ) {
                    let extent = self.project(place, Segment::Extent)?;
                    let scalar_type = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64 is valid"),
                    );
                    return self.entry_content(extent, scalar_type).map(drop);
                }
                for field in data.fields() {
                    let leaf = self.project(place, Segment::Field(field.id))?;
                    self.entry_leaves(leaf, field.value_type)?;
                }
                // Which case a formal holds is not known, so every case's
                // payload leaves get their entry content; the checker lets
                // a read reach one only where that case is established.
                for case in data.cases() {
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
            Step::Index { .. } => None,
            Step::Field { owner, field } => self.record_type(owner)?.fields().get(field),
            Step::Payload { owner, case, field } => self
                .record_type(owner)?
                .cases()
                .get(case)
                .and_then(|case| case.fields.get(field)),
        }
        .ok_or_else(|| self.internal("a field is past its declaration"))
    }

    fn case_of(&self, owner: StructuralTypeId, case: usize) -> Checked<&'a crate::body::CaseType> {
        self.record_type(owner)?
            .cases()
            .get(case)
            .ok_or_else(|| self.internal("a case is past its sum's declaration"))
    }

    /// The place one step beneath `parent`, interned.
    fn step(&mut self, parent: ExactPlaceId, step: Step) -> Checked<ExactPlaceId> {
        if let Step::Index {
            selector, fixed, ..
        } = step
        {
            return self.project(
                parent,
                match fixed {
                    Some(index) => Segment::FixedIndex(index),
                    None => Segment::RuntimeIndex { selector },
                },
            );
        }
        let field = self.declared(step)?.id;
        let parent = match step {
            Step::Index { .. } => return Err(self.internal("an index was not projected")),
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
    pub(super) fn storage_path(
        &mut self,
        steps: &[Step],
        site: ExecutionSite,
        source: SourceOrigin,
    ) -> Checked<Vec<StructuralPathSegment>> {
        let mut path = Vec::with_capacity(steps.len());
        for step in steps {
            let identity = match *step {
                Step::Index {
                    owner,
                    selector,
                    fixed,
                } => {
                    let (_, CollectionExtent::Fixed(length)) = self.array_element(owner)? else {
                        return Err(
                            self.internal("sequence projection needs its actual current length")
                        );
                    };
                    if fixed.is_some_and(|index| index >= length) {
                        return Err(
                            self.internal("a captured fixed index exceeds its array extent")
                        );
                    }
                    let segment = match fixed {
                        Some(index) => StructuralPathSegment::FixedIndex(index),
                        None => {
                            let before = self.here()?;
                            let obligation = self.obligation(
                                ObligationKind::OperationSafety { site },
                                before,
                                source,
                            );
                            StructuralPathSegment::RuntimeIndex {
                                index: selector,
                                obligation,
                            }
                        }
                    };
                    path.push(segment);
                    continue;
                }
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

    fn array_element(
        &self,
        array: StructuralTypeId,
    ) -> Checked<(StructuralTypeId, CollectionExtent)> {
        match self.record_type(array)?.shape {
            StorageShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => Ok((element, CollectionExtent::Fixed(length))),
            StorageShape::ElementView { element, .. } => {
                Ok((element, CollectionExtent::StoredLength))
            }
            _ => Err(self.internal("a collection has no element/extent declaration")),
        }
    }

    fn storage_value_type(&self, storage: StructuralTypeId) -> Checked<ValueType> {
        Ok(match self.record_type(storage)?.shape {
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy: None,
            } => ValueType::Scalar(scalar_type, Qualification::NONE),
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy: Some(policy),
            } => ValueType::Scalar(scalar_type, Qualification::of_policy(policy)),
            _ => ValueType::record(storage),
        })
    }

    fn index_step(&mut self, owner: StructuralTypeId, expression: &Expression) -> Checked<Step> {
        let (_, CollectionExtent::Fixed(length)) = self.array_element(owner)? else {
            return Err(self.internal("sequence index needs its actual current length"));
        };
        let fixed = match expression.kind {
            ExpressionKind::Integer(IntegerValue::Unsigned(value)) => Some(u64::try_from(value)),
            ExpressionKind::Integer(IntegerValue::Signed(value)) => Some(u64::try_from(value)),
            _ => None,
        };
        let fixed = match fixed {
            Some(Ok(index)) if index < length => Some(index),
            Some(_) => {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Typing("array index outside its declared extent"),
                ));
            }
            None => None,
        };
        let selector = self.scalar(expression)?;
        Ok(Step::Index {
            owner,
            selector,
            fixed,
        })
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
            OperationResult::Structural(owned_result(place, record, multiplicity)),
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
        let Some(ValueType::Record(record, _, _)) = expression.result else {
            return Err(self.internal("a scalar expression is used as a record"));
        };
        match &expression.kind {
            ExpressionKind::SavedSubject(capture) => {
                let mut at = self
                    .selected
                    .iter()
                    .rev()
                    .filter_map(|chosen| chosen.capture.as_ref())
                    .find(|(id, _)| id == capture)
                    .map(|(_, place)| place.clone())
                    .ok_or_else(|| self.internal("a saved subject is outside its match capture"))?;
                if at.record != record {
                    return Err(self.internal("a saved subject has another type than its capture"));
                }
                at.fresh = false;
                Ok(at)
            }
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
            ExpressionKind::Index { base, index } => {
                let mut at = self.located(base)?;
                let (element, _) = self.array_element(at.record)?;
                if element != record {
                    return Err(self.internal("an indexed value has another element storage type"));
                }
                let step = self.index_step(at.record, index)?;
                at.place = self.step(at.place, step)?;
                at.path.push(step);
                at.record = record;
                Ok(at)
            }
            ExpressionKind::Array { elements } => {
                let (root, place) = self.establish_array(record, elements, expression.source)?;
                Ok(Located {
                    root,
                    place,
                    path: Vec::new(),
                    record,
                    fresh: true,
                })
            }
            ExpressionKind::ByteLiteral(bytes) => {
                if !matches!(self.record_type(record)?.shape,
                    crate::StorageShape::ByteSequence(carrier) if carrier.is_shared_readable_view())
                {
                    return Err(self.internal("a byte literal requires a shared byte view"));
                }
                let operation = self.operation_identity(expression.source);
                let (root, place) = self.root(
                    StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: self.position(self.roots.len())?,
                        structural_type: record,
                    },
                    record,
                )?;
                self.emit(
                    operation,
                    None,
                    OperationKind::EstablishByteSequenceLiteral {
                        destination: root,
                        bytes: bytes.clone(),
                        qualifications: Vec::new(),
                    },
                )?;
                self.write(
                    place,
                    ProgramPoint::AfterOperation(operation),
                    Content::Overwritten,
                )?;
                Ok(Located {
                    root,
                    place,
                    path: Vec::new(),
                    record,
                    fresh: true,
                })
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
            ExpressionKind::Call {
                callee,
                required_contract,
                source_call,
                arguments,
                designated,
            } => {
                match self.call(
                    *callee,
                    *required_contract,
                    arguments,
                    *designated,
                    expression.result,
                    (expression.source, *source_call),
                )? {
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
            | ExpressionKind::IsCase { .. }
            | ExpressionKind::CollectionMeasure { .. }
            | ExpressionKind::Match { .. }
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::Cast(_)
            | ExpressionKind::ShortCircuit { .. } => {
                Err(self.internal("a scalar operation has a record type"))
            }
            // The operand's value under another set of domains: the same
            // storage, and nothing is computed (domains.md: "removing `D`
            // with a cast changes no bits or geometry").
            ExpressionKind::Requalify(operand) => self.located(operand),
            ExpressionKind::Establish { .. } => {
                Err(self.internal("a cast that proves predicates of a data value"))
            }
            ExpressionKind::Borrow { .. } => {
                Err(self.internal("a loan outside a call's arguments"))
            }
        }
    }

    /// Evaluates a record-typed expression to a root that holds its value
    /// and that nothing else names. A literal or call is its own fresh root;
    /// a slot or member path is copied only with declared copy permission.
    /// Otherwise `MoveStructuralValue` vacates exactly that region; its
    /// continuation owns any remaining cleanup and home retirement.
    pub(super) fn record(&mut self, expression: &Expression) -> Checked<PlaceId> {
        let at = self.located(expression)?;
        if at.fresh && at.path.is_empty() {
            return Ok(at.root);
        }
        let moving = !self.record_type(at.record)?.copyable;
        if moving && self.borrowed_roots.contains(&at.root) {
            return Err(refuse(
                self.id,
                Construct::Machine,
                Unserved::Expression("owned extraction from borrowed storage"),
            ));
        }
        self.name_entry(at.root, at.place, ValueType::record(at.record))?;
        let operation = self.operation_identity(expression.source);
        let path = self.storage_path(
            &at.path,
            ExecutionSite::Operation(operation),
            expression.source,
        )?;
        let (root, exact) = self.produce(
            operation,
            at.record,
            if moving {
                OperationKind::MoveStructuralValue {
                    source: StructuralArgument {
                        place: at.root,
                        path,
                        access: StructuralAccess::Owned,
                    },
                }
            } else {
                OperationKind::StructuralLeafCopy {
                    source: at.root,
                    path,
                }
            },
        )?;
        // The copy's source is the operation's own `source` and `path`; the
        // row says only that the fresh root has new contents.
        self.write(
            exact,
            ProgramPoint::AfterOperation(operation),
            Content::Overwritten,
        )?;
        if moving {
            self.write(
                at.place,
                ProgramPoint::AfterOperation(operation),
                Content::Vacated,
            )?;
            if at.path.is_empty() {
                self.retire_homes(&[], expression.source)?;
            }
        }
        Ok(root)
    }

    /// A named affine local has its own home: the initializer transfers into
    /// it rather than becoming another name for a call result or parameter.
    /// Capture all operands before the continuation can dispose the previous
    /// binding. The admitted plain slice stores no persistent reference
    /// descriptors, so rebinding has no retained loan of the previous home.
    pub(super) fn bind_record(&mut self, expression: &Expression) -> Checked<PlaceId> {
        let Some(ValueType::Record(record, _, _)) = expression.result else {
            return Err(self.internal("a scalar initializer is used as a record"));
        };
        let value = self.record(expression)?;
        if self.record_type(record)?.copyable {
            return Ok(value);
        }
        self.retire_homes(&[], expression.source)?;
        let reserve = self.operation_identity(expression.source);
        let (home, exact) =
            self.produce(reserve, record, OperationKind::EstablishStructuralLocal {})?;
        self.write(
            exact,
            ProgramPoint::AfterOperation(reserve),
            Content::Vacated,
        )?;
        let store = self.operation_identity(expression.source);
        self.emit(
            store,
            None,
            OperationKind::StoreStructuralField {
                disposition: StructuralStoreDisposition::Vacant,
                destination: home,
                path: Vec::new(),
                field: None,
                value: StructuralArgument::whole(value, StructuralAccess::Owned),
            },
        )?;
        self.write(
            exact,
            ProgramPoint::AfterOperation(store),
            Content::Overwritten,
        )?;
        self.retire_homes(&[value], expression.source)?;
        Ok(home)
    }

    /// The place a loan lends, as the call passes it: the lent place's root
    /// and storage path with the loan's access, the place's interned row,
    /// and where its authority comes from. loans.md: "Loan formation
    /// captures a place, not a live selector expression."
    ///
    /// Case and field steps stay on the loan, so checking still requires
    /// the selected case to be active. A scalar local that
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
        home_identity: Option<&str>,
    ) -> Checked<((PlaceId, Vec<Step>, StructuralAccess), ExactPlaceId, Lender)> {
        let lent_access =
            borrowed_access(access).ok_or_else(|| self.internal("a write-only loan"))?;
        let value_type = place
            .result
            .ok_or_else(|| self.internal("a loan of a Unit value"))?;
        let shared_view = match value_type {
            ValueType::Record(record, _, _) => match &self.record_type(record)?.shape {
                StorageShape::ByteSequence(carrier) => carrier.is_shared_readable_view(),
                StorageShape::ElementView {
                    access: Some(StructuralAccess::SharedBorrow),
                    ..
                } => true,
                _ => false,
            },
            _ => false,
        };
        let (root, steps, exact, actual) =
            if shared_view && access == language_core::ReferenceAccess::Shared {
                let at = self.located(place)?;
                (at.root, at.path, at.place, ValueType::record(at.record))
            } else {
                self.capture_place(place, home_identity)?
            };
        if actual.erased() != value_type.erased() {
            return Err(self.internal("a loan has another type than its captured place"));
        }
        self.name_entry(root, exact, value_type)?;
        let lender = if self.borrowed_roots.contains(&root) {
            Lender::Incoming
        } else {
            Lender::Owner
        };
        Ok(((root, steps, lent_access), exact, lender))
    }

    /// Capture an existing place using the same expression and declared
    /// storage types for a loan or assignment. Only index operands evaluate;
    /// a saved subject already holds its captured selectors.
    fn capture_place(
        &mut self,
        expression: &Expression,
        home_identity: Option<&str>,
    ) -> Checked<(PlaceId, Vec<Step>, ExactPlaceId, ValueType)> {
        let captured = match &expression.kind {
            ExpressionKind::Slot(slot) => {
                let value_type = self.slot_type(*slot)?;
                let bound = *self.slot_mut(*slot)?;
                let root = match bound {
                    Some(Bound::Home(root) | Bound::Record(root)) => root,
                    Some(Bound::Scalar(_)) if home_identity.is_some() && self.joining > 0 => {
                        return Err(refuse(
                            self.id,
                            self.current.map_or(Construct::Machine, Construct::Block),
                            Unserved::Call("loan of a value local on some paths only"),
                        ));
                    }
                    Some(Bound::Scalar(value)) if home_identity.is_some() => {
                        let root = self.establish_home(
                            value,
                            value_type,
                            home_identity,
                            expression.source,
                        )?;
                        *self.slot_mut(*slot)? = Some(Bound::Home(root));
                        root
                    }
                    _ => return Err(self.internal("a captured slot holds no storage")),
                };
                (root, Vec::new(), self.exact_root(root)?, value_type)
            }
            ExpressionKind::Field { base, field } => {
                let (root, mut steps, parent, ValueType::Record(owner, _, _)) =
                    self.capture_place(base, None)?
                else {
                    return Err(self.internal("a field place crosses scalar storage"));
                };
                let step = Step::Field {
                    owner,
                    field: *field,
                };
                let value_type = self.declared(step)?.value_type;
                let exact = self.step(parent, step)?;
                steps.push(step);
                (root, steps, exact, value_type)
            }
            ExpressionKind::Index { base, index } => {
                let (root, mut steps, parent, ValueType::Record(owner, _, _)) =
                    self.capture_place(base, None)?
                else {
                    return Err(self.internal("an indexed place crosses scalar storage"));
                };
                let (element, _) = self.array_element(owner)?;
                let value_type = self.storage_value_type(element)?;
                let step = self.index_step(owner, index)?;
                let exact = self.step(parent, step)?;
                steps.push(step);
                (root, steps, exact, value_type)
            }
            ExpressionKind::Payload { base, case, field } => {
                let (root, mut steps, parent, ValueType::Record(owner, _, _)) =
                    self.capture_place(base, None)?
                else {
                    return Err(self.internal("a payload place crosses scalar storage"));
                };
                let step = Step::Payload {
                    owner,
                    case: *case,
                    field: *field,
                };
                let value_type = self.declared(step)?.value_type;
                let exact = self.step(parent, step)?;
                steps.push(step);
                (root, steps, exact, value_type)
            }
            ExpressionKind::SavedSubject(_) => {
                let at = self.located(expression)?;
                if at.fresh {
                    return Err(self.internal("a capture names newly produced storage"));
                }
                (at.root, at.path, at.place, ValueType::record(at.record))
            }
            _ => return Err(self.internal("a destination does not name existing storage")),
        };
        if expression
            .result
            .is_some_and(|declared| declared.erased() != captured.3.erased())
        {
            return Err(self.internal("a captured place has another declared type"));
        }
        Ok(captured)
    }

    /// A home for an SSA scalar holding `value`: one `EstablishPrimitiveLocal`
    /// and the row that says the home holds that value.
    fn establish_home(
        &mut self,
        value: ValueId,
        value_type: ValueType,
        identity: Option<&str>,
        source: SourceOrigin,
    ) -> Checked<PlaceId> {
        let home = self.records.primitive(value_type, identity, self.id)?;
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
        let declared = self.record_type(record)?.fields();
        let mut operands: Vec<Option<Operand>> = vec![None; declared.len()];
        for (position, operand) in fields {
            let field = declared
                .get(*position)
                .ok_or_else(|| self.internal("a literal names a field past its record"))?;
            let evaluated = match field.value_type {
                ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                ValueType::Scalar(..) => Operand::Scalar(self.scalar(operand)?),
                ValueType::Record(_, _, _) => Operand::Record(self.record(operand)?),
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

    fn establish_array(
        &mut self,
        array: StructuralTypeId,
        elements: &[Expression],
        source: SourceOrigin,
    ) -> Checked<(PlaceId, ExactPlaceId)> {
        let (element_type, length) = self.array_element(array)?;
        if let CollectionExtent::Fixed(length) = length
            && u64::try_from(elements.len()).ok() != Some(length)
        {
            return Err(self.internal("an array literal has another extent than its type"));
        }
        let value_type = self.storage_value_type(element_type)?;
        let mut operands = Vec::with_capacity(elements.len());
        for element in elements {
            if element.result.map(ValueType::erased) != Some(value_type.erased()) {
                return Err(self.internal("an array literal element has another type"));
            }
            operands.push(match value_type {
                ValueType::Scalar(..) => {
                    let value = self.scalar(element)?;
                    if length == CollectionExtent::StoredLength {
                        let identity = self.record_type(element_type)?.identity.as_str();
                        Operand::Record(self.establish_home(
                            value,
                            value_type,
                            Some(identity),
                            source,
                        )?)
                    } else {
                        Operand::Scalar(value)
                    }
                }
                ValueType::Record(_, _, _) => Operand::Record(self.record(element)?),
                ValueType::Rigid(_) => return Err(self.internal("a rigid array element")),
            });
        }
        let kind =
            match value_type {
                _ if length == CollectionExtent::StoredLength => {
                    OperationKind::EstablishStructuralArray {
                        elements: operands
                            .iter()
                            .map(|operand| match operand {
                                Operand::Record(root) => {
                                    Ok(StructuralArgument::whole(*root, StructuralAccess::Owned))
                                }
                                _ => Err(self
                                    .internal("a sequence element lost its exact structural home")),
                            })
                            .collect::<Checked<_>>()?,
                    }
                }
                ValueType::Scalar(..) => OperationKind::EstablishScalarArray {
                    elements: operands
                        .iter()
                        .map(|operand| match operand {
                            Operand::Scalar(value) => Ok(*value),
                            _ => Err(self.internal("a non-scalar array element")),
                        })
                        .collect::<Checked<_>>()?,
                },
                ValueType::Record(_, _, _) => OperationKind::EstablishStructuralArray {
                    elements: operands
                        .iter()
                        .map(|operand| match operand {
                            Operand::Record(root) => {
                                Ok(StructuralArgument::whole(*root, StructuralAccess::Owned))
                            }
                            _ => Err(self.internal("a non-structural array element")),
                        })
                        .collect::<Checked<_>>()?,
                },
                ValueType::Rigid(_) => return Err(self.internal("a rigid array element")),
            };
        let operation = self.operation_identity(source);
        let (root, exact) = self.produce(operation, array, kind)?;
        self.write(
            exact,
            ProgramPoint::AfterOperation(operation),
            Content::Overwritten,
        )?;
        // A stored-length collection establishes its descriptor. The actual
        // element operands remain on the constructor operation; they are not
        // fixed-index source places beneath that descriptor.
        if length == CollectionExtent::StoredLength {
            return Ok((root, exact));
        }
        for (index, operand) in operands.into_iter().enumerate() {
            let index = u64::try_from(index)
                .map_err(|_| self.internal("an array element index exceeds u64"))?;
            let leaf = self.project(exact, Segment::FixedIndex(index))?;
            self.write(
                leaf,
                ProgramPoint::AfterOperation(operation),
                match operand {
                    Operand::Scalar(value) => Content::Value(value),
                    Operand::Record(_) => Content::Overwritten,
                },
            )?;
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
                ValueType::Scalar(..) => Operand::Scalar(self.scalar(operand)?),
                ValueType::Record(_, _, _) => Operand::Record(self.record(operand)?),
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
        self.name_entry(at.root, at.place, ValueType::record(at.record))?;
        let operation = self.operation_identity(source);
        let path = self.storage_path(&at.path, ExecutionSite::Operation(operation), source)?;
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
        capture: Option<arena::Handle<typed_trees::expression::CompletedValueCapture>>,
    ) -> Checked<(Vec<(BlockId, Chosen)>, Option<(BlockId, Chosen)>)> {
        let at = self.located(subject)?;
        let data = self.record_type(at.record)?;
        if data.cases().is_empty() {
            return Err(self.internal("a dispatch on storage that is no sum"));
        }
        let path = self.storage_path(&at.path, ExecutionSite::Block(self.current()?), source)?;
        self.name_entry(at.root, at.place, ValueType::record(at.record))?;
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
                    ValueType::Scalar(scalar_type, _) => {
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
                    ValueType::Record(_, _, _) => None,
                });
            }
            blocks.push((
                block,
                Chosen {
                    capture: capture.map(|id| (id, at.clone())),
                    place: at.place,
                    case: Some(*case),
                    parameters,
                },
            ));
            payloads.push(payload_fields);
        }
        let mut rest: Option<(BlockId, Chosen)> = None;
        let mut edges = Vec::with_capacity(data.cases().len());
        for (position, case) in data.cases().iter().enumerate() {
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
                                    capture: capture.map(|id| (id, at.clone())),
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
        let step = Step::Payload {
            owner: at.record,
            case,
            field,
        };
        let leaf = self.step(at.place, step)?;
        let mut steps = at.path;
        steps.push(step);
        self.read_projected_scalar(at.root, leaf, &steps, scalar_type, source)
    }

    fn read_projected_scalar(
        &mut self,
        root: PlaceId,
        leaf: ExactPlaceId,
        steps: &[Step],
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        self.name_entry(
            root,
            leaf,
            ValueType::Scalar(scalar_type, Qualification::NONE),
        )?;
        let operation = self.operation_identity(source);
        let path = self.storage_path(steps, ExecutionSite::Operation(operation), source)?;
        self.emit(
            operation,
            Some(scalar_type),
            OperationKind::PrimitiveScalarRead { source: root, path },
        )
    }

    pub(super) fn read_field(
        &mut self,
        base: &Expression,
        field: usize,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let at = self.located(base)?;
        let step = Step::Field {
            owner: at.record,
            field,
        };
        let leaf = self.step(at.place, step)?;
        let mut steps = at.path;
        steps.push(step);
        self.read_projected_scalar(at.root, leaf, &steps, scalar_type, source)
    }

    pub(super) fn read_index(
        &mut self,
        base: &Expression,
        index: &Expression,
        scalar_type: ScalarType,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let at = self.located(base)?;
        let step = self.index_step(at.record, index)?;
        let leaf = self.step(at.place, step)?;
        let mut steps = at.path;
        steps.push(step);
        self.read_projected_scalar(at.root, leaf, &steps, scalar_type, source)
    }

    pub(super) fn measure_collection(
        &mut self,
        base: &Expression,
        measure: typed_trees::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Value> {
        if measure != typed_trees::expression::CollectionMeasure::Length {
            return Err(self.internal("a collection measure has no canonical operation"));
        }
        // Locating evaluates a call or constructor base exactly once. Naming
        // the metadata beneath its actual descriptor lets the same write
        // history distinguish the initial extent from a replacement's.
        let at = self.located(base)?;
        if !matches!(
            self.record_type(at.record)?.shape,
            StorageShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
            })
        ) {
            return Err(self.internal("a length observation has no byte descriptor"));
        }
        let scalar_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        self.name_entry(at.root, at.place, ValueType::record(at.record))?;
        self.project(at.place, Segment::Extent)?;
        let operation = self.operation_identity(source);
        let path = self.storage_path(&at.path, ExecutionSite::Operation(operation), source)?;
        self.emit(
            operation,
            Some(scalar_type),
            OperationKind::ByteSequenceLength {
                source: StructuralArgument {
                    place: at.root,
                    path,
                    access: StructuralAccess::SharedBorrow,
                },
            },
        )
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
    /// whether the storage is an established value (`terminal-psi-to-terminal-psi`).
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
    pub(super) fn declare_local(
        &mut self,
        slot: usize,
        identity: Option<&str>,
        source: SourceOrigin,
    ) -> Checked<()> {
        let bound = match self.slot_type(slot)? {
            ValueType::Rigid(_) => {
                return Err(self.internal("a local of a rigid type is declared"));
            }
            ValueType::Record(record, _, _) => {
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
            ValueType::Scalar(scalar_type, _) => {
                let home = self
                    .records
                    .primitive(self.slot_type(slot)?, identity, self.id)?;
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
        self.name_entry(
            home,
            place,
            ValueType::Scalar(scalar_type, Qualification::NONE),
        )?;
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
    /// assignment into member stores"). A stored byte view is a byte field,
    /// replaced by `StructuralByteViewFieldStore` from the root that holds
    /// the source descriptor ("No length, capacity or bytes are retained:
    /// the slot takes the source's backing and extent"). A homed scalar
    /// local is `WriteOnlyPrimitiveStore` with an empty path.
    pub(super) fn store(
        &mut self,
        destination: &Expression,
        value: &Expression,
        source: SourceOrigin,
    ) -> Checked<()> {
        // Capturing visits the destination before the right-hand side and
        // retains each selector's exact SSA value in the existing Step roster.
        let (root, steps, place, expected) = self.capture_place(destination, None)?;
        if steps.is_empty() && matches!(expected, ValueType::Record(_, _, _)) {
            return Err(self.internal("a whole record assignment must rebind its local"));
        }
        let mut owed = 0;
        for step in &steps {
            let owner = match step {
                Step::Field { owner, .. }
                | Step::Payload { owner, .. }
                | Step::Index { owner, .. } => *owner,
            };
            owed += self.record_type(owner)?.requires.len();
        }
        if value.result.map(ValueType::erased) != Some(expected.erased()) {
            return Err(self.internal("a replacement has another type than its destination"));
        }
        match expected {
            ValueType::Rigid(_) => Err(self.internal("a store into rigid storage")),
            ValueType::Scalar(..) => {
                let stored = self.scalar(value)?;
                let operation = self.operation_identity(source);
                self.owe_domains(operation, owed, source)?;
                let kind = if let Some(Step::Field { .. }) = steps.last() {
                    let field = self
                        .declared(
                            *steps
                                .last()
                                .ok_or_else(|| self.internal("a field store lost its step"))?,
                        )?
                        .id;
                    let storage = self.storage_path(
                        &steps[..steps.len() - 1],
                        ExecutionSite::Operation(operation),
                        source,
                    )?;
                    OperationKind::StructuralScalarFieldStore {
                        destination: root,
                        path: storage,
                        field,
                        value: stored,
                        range_obligation: None,
                    }
                } else {
                    let storage =
                        self.storage_path(&steps, ExecutionSite::Operation(operation), source)?;
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination: root,
                        path: storage,
                        value: stored,
                        range_obligation: None,
                    }
                };
                self.emit(operation, None, kind)?;
                self.write(
                    place,
                    ProgramPoint::AfterOperation(operation),
                    Content::Value(stored),
                )
            }
            ValueType::Record(stored, _, _) => {
                let constructed = self.record(value)?;
                let operation = self.operation_identity(source);
                self.owe_domains(operation, owed, source)?;
                let mut storage =
                    self.storage_path(&steps, ExecutionSite::Operation(operation), source)?;
                let view_field = match steps.last() {
                    Some(step @ (Step::Field { .. } | Step::Payload { .. }))
                        if self.is_stored_view(stored)? =>
                    {
                        Some(self.declared(*step)?.id)
                    }
                    _ => None,
                };
                let kind = match view_field {
                    Some(field) => {
                        // The carrier is the path without the field itself.
                        storage.pop();
                        OperationKind::StructuralByteViewFieldStore {
                            destination: root,
                            path: storage,
                            field: Some(field),
                            source: constructed,
                            predicate_obligation: None,
                        }
                    }
                    None => OperationKind::StoreStructuralField {
                        disposition: StructuralStoreDisposition::ReplaceDiscardable,
                        destination: root,
                        path: storage,
                        field: None,
                        value: StructuralArgument::whole(constructed, StructuralAccess::Owned),
                    },
                };
                self.emit(operation, None, kind)?;
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
        match data.shape {
            StorageShape::PrimitiveScalar { scalar_type, .. } => {
                let zero = self.zero(scalar_type, source)?;
                zeros.push((prefix.clone(), zero));
                return Ok(());
            }
            StorageShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                for index in 0..length {
                    prefix.push(Segment::FixedIndex(index));
                    self.zero_leaves(element, prefix, zeros, owed, source)?;
                    prefix.pop();
                }
                return Ok(());
            }
            StorageShape::FixedArray { .. } => {
                return Err(self.internal("zero array without a closed extent"));
            }
            StorageShape::ByteSequence(_) => {
                return Err(
                    self.internal("byte view initialization requires actual readable backing")
                );
            }
            StorageShape::ElementView {
                access: None | Some(StructuralAccess::SharedBorrow),
                ..
            } => {
                // EstablishZeroedStructural retains an actual empty immutable
                // descriptor through the canonical zero-view owner.
                return Ok(());
            }
            StorageShape::ElementView { .. } => {
                return Err(self.internal("zero mutable sequence requires actual backing"));
            }
            StorageShape::Record(_) | StorageShape::Sum(_) => {}
        }
        let (fields, case) = match data.cases().first() {
            Some(first) => (first.fields.as_slice(), Some(first.id)),
            None => (data.fields(), None),
        };
        if let Some(case) = case {
            prefix.push(Segment::Case(case));
        }
        for field in fields {
            prefix.push(Segment::Field(field.id));
            match field.value_type {
                ValueType::Rigid(_) => return Err(self.internal("a field of a rigid type")),
                ValueType::Scalar(scalar_type, _) => {
                    let zero = self.zero(scalar_type, source)?;
                    zeros.push((prefix.clone(), zero));
                }
                // A stored view's zero is the empty shared view: no scalar
                // leaf and no clause (structural_access.md: "Zero-initialized
                // storage holds the empty shared view, which borrows
                // nothing").
                ValueType::Record(nested, _, _) if self.is_stored_view(nested)? => {}
                ValueType::Record(nested, _, _) => {
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

    /// Retains a normal continuation after record argument staging. Copy
    /// temporaries still need explicit retirement; the ownership plan replaces
    /// affine actions, omitting a second retirement when whole transport has
    /// already consumed the source home. ownership.md:
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Borrowed, CrashCeiling, Policy, Signature, Termination};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

    #[test]
    fn scalar_homes_keep_their_identity_beside_qualified_array_elements() {
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let plain = ValueType::Scalar(scalar, Qualification::NONE);
        let wrapping = ValueType::Scalar(scalar, Qualification::of_policy(Policy::Wrapping));
        let machine = MachineId::new(1).unwrap();
        let mut types = IdTable::default();
        let wrapping_id = types.push(RecordType {
            copyable: true,
            identity: "wrapping-u32".into(),
            shape: StorageShape::PrimitiveScalar {
                scalar_type: scalar,
                policy: Some(Policy::Wrapping),
            },
            requires: Vec::new(),
        });
        types.push(RecordType {
            copyable: true,
            identity: "wrapping-elements".into(),
            shape: StorageShape::FixedArray {
                element: wrapping_id,
                length: terminal_psi::ArrayExtent::Literal(2),
            },
            requires: Vec::new(),
        });
        let source = SourceOrigin::Unpositioned(Unpositioned::SpanAbsent);
        let mut signatures = IdTable::default();
        signatures.push(Signature {
            parameters: vec![plain],
            erased: Vec::new(),
            borrowed: vec![Borrowed {
                position: 0,
                access: language_core::ReferenceAccess::Shared,
                home: Some("plain-u32".into()),
            }],
            result: Some(plain),
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: crate::Ceiling::Inferred,
            invokes: crate::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
        let mut bodies = IdTable::default();
        bodies.push(Body {
            slots: vec![plain, wrapping, plain, plain],
            statements: vec![
                Statement::Declare {
                    slot: 1,
                    home: Some("wrapping-u32".into()),
                    source,
                },
                Statement::Declare {
                    slot: 2,
                    home: Some("plain-alias".into()),
                    source,
                },
                Statement::Bind {
                    slot: 3,
                    value: Expression {
                        kind: ExpressionKind::Integer(IntegerValue::Unsigned(0)),
                        result: Some(plain),
                        source,
                    },
                },
                Statement::Evaluate(Expression {
                    kind: ExpressionKind::Call {
                        callee: semantic_vocabulary::TargetMachineCallable::Machine(
                            MachineId::new(2).unwrap(),
                        ),
                        arguments: vec![Expression {
                            kind: ExpressionKind::Borrow {
                                access: language_core::ReferenceAccess::Shared,
                                place: Box::new(Expression {
                                    kind: ExpressionKind::Slot(3),
                                    result: Some(plain),
                                    source,
                                }),
                                home: Some("plain-u32".into()),
                            },
                            result: Some(plain),
                            source,
                        }],
                        designated: None,
            required_contract: None,
            source_call: None,
                    },
                    result: Some(plain),
                    source,
                }),
                Statement::Arm {
                    guard: None,
                    target: Target::Value(Expression {
                        kind: ExpressionKind::Slot(0),
                        result: Some(plain),
                        source,
                    }),
                    source,
                },
            ],
        });
        signatures.push(signatures.get(machine).unwrap().clone());
        bodies.push(Body {
            slots: vec![plain],
            statements: vec![Statement::Arm {
                guard: None,
                target: Target::Value(Expression {
                    kind: ExpressionKind::Slot(0),
                    result: Some(plain),
                    source,
                }),
                source,
            }],
        });
        let declared = declare(
            machine,
            &types,
            &bodies,
            crate::body::Signatures {
                declarations: &IdTable::default(),
            declaration_aliases: None,
                machines: &signatures,
                boundaries: &IdTable::default(),
                completions: &IdTable::default(),
            },
            TerminalIdentityTableBuilder::default(),
        )
        .unwrap();
        let records = &declared.records;
        let plain_id = records
            .primitive(plain, Some("plain-u32"), machine)
            .unwrap();
        let alias_id = records
            .primitive(plain, Some("plain-alias"), machine)
            .unwrap();
        assert_ne!(plain_id, wrapping_id);
        assert_ne!(plain_id, alias_id);
        assert_eq!(
            records
                .primitive(wrapping, Some("wrapping-u32"), machine)
                .unwrap(),
            wrapping_id
        );
        assert!(
            records
                .primitive(plain, Some("wrapping-u32"), machine)
                .is_err()
        );
        assert!(records.primitive(plain, None, machine).is_err());
        for (id, identity) in [
            (plain_id, "plain-u32"),
            (alias_id, "plain-alias"),
            (wrapping_id, "wrapping-u32"),
        ] {
            let row = declared
                .structural_types
                .iter()
                .find(|row| row.id == id)
                .unwrap();
            assert_eq!(declared.identities.text(row.identity), Some(identity));
            assert_eq!(row.shape, StructuralTypeShape::PrimitiveScalar(scalar));
        }
        let elaboration = crate::lower_bodies(machine, &bodies, &signatures, &types).unwrap();
        let lowered = &elaboration.module.machines[0];
        assert_eq!(lowered.structural_parameters[0].structural_type, plain_id);
        let homes: Vec<_> = lowered
            .operations()
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishPrimitiveLocal { .. }
                )
            })
            .map(|operation| operation.result.as_structural().unwrap().structural_type)
            .collect();
        assert_eq!(homes, [wrapping_id, alias_id, plain_id]);
    }
}
