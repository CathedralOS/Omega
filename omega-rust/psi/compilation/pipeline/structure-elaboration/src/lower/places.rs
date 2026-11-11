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
- A call's record result is one `Content::Overwritten` on its root: nothing
  here knows the callee's values.
- A local declared without an initializer writes nothing: its home is
  reserved and holds no content until a store reaches it.
- What a formal holds at entry is not a write. It is one value per scalar
  leaf with `ValueOrigin::EntryContent { place }`, minted the first time an
  operation names the leaf or a place above it, whatever is stored there
  later.
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IdTable, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment, WriteRow};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, ExactPlaceId, ProgramPoint, PsiSemanticId};
use terminal_psi::{
    NativeLayoutCatalog, NativeLayoutFieldIdentity, Operation, OperationKind, OperationResult,
    RecordFieldInitializer, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralRecordStorage,
    StructuralStoreDisposition, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalIdentityIndex, TerminalIdentityTable,
    TerminalIdentityTableBuilder, Terminator,
};

use super::{Lowering, Value};
use crate::body::{Body, Expression, ExpressionKind, RecordType, Statement, ValueType};
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
/// Every flag states what a copy-eligible plain record is by structure:
/// `copyable` because its type is unrestricted; `no_code_affine_cleanup`
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
) -> Checked<Declared<'a>> {
    let mut identities = TerminalIdentityTableBuilder::default();
    let mut field_identities = IdTable::default();
    let mut structural_types = Vec::with_capacity(types.len());
    let mut native_layouts = NativeLayoutCatalog::default();
    for (id, record) in types.iter() {
        let mut indexes = Vec::with_capacity(record.fields.len());
        let mut fields = Vec::with_capacity(record.fields.len());
        for field in &record.fields {
            let identity = identities.intern(&field.identity);
            indexes.push(identity);
            let field_type = match field.value_type {
                ValueType::Scalar(scalar_type) => StructuralFieldType::Scalar(scalar_type),
                ValueType::Record(nested) => {
                    // A record holds only records declared before it, so the
                    // table is acyclic and in dependency order.
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
        field_identities.push(indexes);
        // The natural layout row of a record with no authored layout: no
        // policy and no realization. Its nominal native identities are left
        // absent, as the current pipeline leaves any it cannot supply; they
        // serve the native layout review, which this route does not reach.
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
        structural_types.push(StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity: identities.intern(&record.identity),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            shape: StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            },
            scalar_restriction: None,
        });
    }
    let mut primitives: Vec<(ScalarType, StructuralTypeId)> = Vec::new();
    for (_, body) in bodies.iter() {
        for statement in &body.statements {
            let Statement::Declare { slot, home, .. } = statement else {
                continue;
            };
            let Some(ValueType::Scalar(scalar_type)) = body.slots.get(*slot).copied() else {
                continue;
            };
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
        },
        identities: identities.build(),
        structural_types,
        native_layouts,
    })
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
}

/// The storage a record-typed expression names, before anything is copied.
struct Located {
    root: PlaceId,
    place: ExactPlaceId,
    /// The fields from the root to the place, each as the record type that
    /// declares it and its position there.
    path: Vec<(StructuralTypeId, usize)>,
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

fn unrestricted_result(place: PlaceId, record: StructuralTypeId) -> StructuralOperationResult {
    StructuralOperationResult {
        place,
        structural_type: record,
        multiplicity: StructuralMultiplicity::Unrestricted,
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
    fn project(&mut self, parent: ExactPlaceId, field: StructuralFieldId) -> Checked<ExactPlaceId> {
        let segment = Segment::Field(field);
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
            ValueType::Scalar(scalar_type) => self.entry_content(place, scalar_type).map(drop),
            ValueType::Record(record) => {
                for field in &self.record_type(record)?.fields {
                    let leaf = self.project(place, field.id)?;
                    self.entry_leaves(leaf, field.value_type)?;
                }
                Ok(())
            }
        }
    }

    /// The Terminal Psi storage path segment of one field: the interned
    /// identity its declaration carries. It is written into operations,
    /// which is the form Terminal Psi gives them; nothing here compares it.
    fn storage_segment(
        &self,
        owner: StructuralTypeId,
        field: usize,
    ) -> Checked<StructuralPathSegment> {
        terminal_check_tables::row(
            &self.records.field_identities,
            owner,
            self.id,
            Construct::Machine,
        )?
        .get(field)
        .copied()
        .map(StructuralPathSegment::Field)
        .ok_or_else(|| self.internal("a field has no declared identity"))
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
        self.draft()?.operations.push(Operation::unsuspended(
            operation,
            OperationResult::Structural(unrestricted_result(place, record)),
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
                    Some(Bound::Scalar(_) | Bound::Home(_)) => {
                        Err(self.internal("a scalar slot is used as a record"))
                    }
                    None => Err(self.internal("a slot is read before it is bound")),
                }
            }
            ExpressionKind::Field { base, field } => {
                let mut at = self.located(base)?;
                let declared = self
                    .record_type(at.record)?
                    .fields
                    .get(*field)
                    .ok_or_else(|| self.internal("a field is past its record's declaration"))?;
                at.place = self.project(at.place, declared.id)?;
                at.path.push((at.record, *field));
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
            ExpressionKind::Call { callee, arguments } => {
                match self.call(*callee, arguments, expression.result, expression.source)? {
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
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::ShortCircuit { .. } => {
                Err(self.internal("a scalar operation has a record type"))
            }
        }
    }

    /// Evaluates a record-typed expression to a root that holds its value
    /// and that nothing else names. A literal or call is its own fresh root;
    /// a slot or member path is copied whole into one (`StructuralLeafCopy`),
    /// which is what `let b = a;`, passing by value and returning do.
    pub(super) fn record(&mut self, expression: &Expression) -> Checked<PlaceId> {
        let at = self.located(expression)?;
        if at.fresh && at.path.is_empty() {
            return Ok(at.root);
        }
        let mut path = Vec::with_capacity(at.path.len());
        for (owner, field) in &at.path {
            path.push(self.storage_segment(*owner, *field)?);
        }
        self.name_entry(at.root, at.place, ValueType::Record(at.record))?;
        let operation = self.operation_identity(expression.source);
        let (root, exact) = self.produce(
            operation,
            at.record,
            OperationKind::StructuralLeafCopy {
                source: at.root,
                path,
            },
        )?;
        // The copy's source is the operation's own `source` and `path`; the
        // row says only that the fresh root has new contents.
        self.write(
            exact,
            ProgramPoint::AfterOperation(operation),
            Content::Overwritten,
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
        let (root, exact) = self.produce(
            operation,
            record,
            OperationKind::EstablishRecord {
                fields: initializers,
            },
        )?;
        let at = ProgramPoint::AfterOperation(operation);
        for (field, operand) in roster {
            let leaf = self.project(exact, field.id)?;
            match operand {
                Operand::Scalar(value) => self.write(leaf, at, Content::Value(value))?,
                // The operand root is named by the establishment's own
                // roster; the row says only that the field has new contents.
                Operand::Record(_) => self.write(leaf, at, Content::Overwritten)?,
            }
        }
        Ok((root, exact))
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
        let mut path = Vec::with_capacity(at.path.len());
        for (owner, field) in &at.path {
            let declared = self
                .record_type(*owner)?
                .fields
                .get(*field)
                .ok_or_else(|| self.internal("a field is past its record's declaration"))?;
            path.push(CanonicalStructuralPathSegment::Field(declared.id));
        }
        let declared = self
            .record_type(at.record)?
            .fields
            .get(field)
            .ok_or_else(|| self.internal("a field is past its record's declaration"))?;
        let leaf = self.project(at.place, declared.id)?;
        self.name_entry(at.root, leaf, ValueType::Scalar(scalar_type))?;
        let kind = match scalar_type {
            ScalarType::Boolean => OperationKind::BooleanStructuralField {
                source: at.root,
                path,
                field: declared.id,
            },
            ScalarType::Integer(_) => OperationKind::ScalarStructuralField {
                source: at.root,
                path,
                field: declared.id,
            },
            ScalarType::IeeeFloat(_) => {
                return Err(self.internal("a float field reached lowering"));
            }
        };
        let operation = self.operation_identity(source);
        self.emit(operation, Some(scalar_type), kind)
    }

    /// `let x: T;`: reserves the local's storage and stores nothing the
    /// checker may rely on. The operations are the ones the current pipeline
    /// emits, so the module runs: a record home is `EstablishZeroedStructural`
    /// and a scalar home is `EstablishPrimitiveLocal` of a zero constant
    /// (structural_access.md: "`EstablishPrimitiveLocal` consumes an exactly
    /// typed SSA initializer and establishes the referent"). No row is
    /// written for either: state_contracts.md, "Uninitialized storage has no
    /// assumed zero." Whether such a declaration instead establishes the zero
    /// value is `OWNER_QUESTIONS.md`, `local-without-initializer`.
    pub(super) fn declare_local(&mut self, slot: usize, source: SourceOrigin) -> Checked<()> {
        let bound = match self.slot_type(slot)? {
            ValueType::Record(record) => {
                let operation = self.operation_identity(source);
                let (root, _) = self.produce(
                    operation,
                    record,
                    OperationKind::EstablishZeroedStructural {},
                )?;
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
                let zero = self.operation_identity(source);
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
                let Value::Scalar(value) = self.emit(zero, Some(scalar_type), kind)? else {
                    return Err(self.internal("a constant produced no value"));
                };
                let operation = self.operation_identity(source);
                let (root, _) = self.produce(
                    operation,
                    home,
                    OperationKind::EstablishPrimitiveLocal {
                        value,
                        range_obligation: None,
                    },
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
            (Some(Bound::Scalar(_) | Bound::Home(_)), Some(_)) => {
                return Err(self.internal("a field of a scalar slot is stored to"));
            }
            (Some(Bound::Scalar(_) | Bound::Record(_)), None) => {
                return Err(self.internal("a whole store into a slot that is not a home"));
            }
            (None, _) => return Err(self.internal("a slot is stored to before it is bound")),
        };
        let (_, mut record) =
            *terminal_check_tables::row(&self.roots, root, self.id, Construct::Machine)?;
        let mut place = self.exact_root(root)?;
        let (final_field, carrier) = last;
        let mut storage = Vec::with_capacity(carrier.len());
        for position in carrier {
            let declared = self
                .record_type(record)?
                .fields
                .get(*position)
                .ok_or_else(|| self.internal("a field is past its record's declaration"))?;
            storage.push(self.storage_segment(record, *position)?);
            place = self.project(place, declared.id)?;
            record = match declared.value_type {
                ValueType::Record(nested) => nested,
                ValueType::Scalar(_) => {
                    return Err(self.internal("a store path crosses a scalar field"));
                }
            };
        }
        let declared = self
            .record_type(record)?
            .fields
            .get(*final_field)
            .ok_or_else(|| self.internal("a field is past its record's declaration"))?;
        let place = self.project(place, declared.id)?;
        if value.result != Some(declared.value_type) {
            return Err(self.internal("a stored value has another type than its field"));
        }
        match declared.value_type {
            ValueType::Scalar(_) => {
                let stored = self.scalar(value)?;
                let operation = self.operation_identity(source);
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
