/*
The place family of lowering: structural roots, projections interned on first
use, the forward write log, record construction, field reads and whole
copies. This is where `MachineTables::places`, `roots` and `writes` are
filled, in lockstep with the Terminal Psi operations that create the storage.

Roots. One Terminal `PlaceId` and one `PlaceRow { origin: Root }` per record
formal, per operation that produces a record (a literal, a whole copy, a call
with a record result) and for a record machine's result place; `roots` maps
the `PlaceId` to its row. A record local has no place of its own: its slot
holds the root its initializer produced. ownership.md: "Parameter,
named-local, and call-result roots retain distinct establishment identities."

Projections. `r.a` is the row `(r, Segment::Field(a))`, found by binary
search of the parent's sorted `children` and pushed only when absent, so a
path names one `ExactPlaceId` however often it is used. ownership.md:
"Resolve every segment against the type reached by the previous segment".

The write log. Establishing a record writes each scalar leaf beneath its
fresh root once, at the point after the establishing operation: a literal
writes `Content::Value(operand)`, a whole copy writes what the source leaf
holds, and a call's record result is one `Content::Overwritten` on the root,
because nothing here knows the callee's values. What a formal holds at entry
is not a write: it is one value per leaf with
`ValueOrigin::EntryContent { place }`, minted the first time the leaf is
read or copied. A value is never invented: a leaf whose content has no value
identity stays `Overwritten`.

What a leaf holds is read back from the log lowering itself wrote (`held`):
the last write on the leaf or on an ancestor. That is exact here because of
how this subset is shaped, not in general: every transition arm completes the
machine, so the statements of a body lie on one spine of blocks, each
dominating the next, and every root a later statement can name was written on
that spine. The checker does not rely on this; it derives reaching writes
from the graph (`lowered-discharge`, `places`). A slice that joins two
different contents of one place (a value `match`, a loop) must replace `held`
with the graph answer rather than extend it.
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IdTable, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
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
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalIdentityIndex, TerminalIdentityTable, TerminalIdentityTableBuilder, Terminator,
};

use super::{Lowering, Value};
use crate::body::{Expression, ExpressionKind, RecordType, ValueType};
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};

/// The module's record types as lowering needs them: the reader's table and,
/// beside it, the interned identity Terminal Psi's declarations and
/// structural paths name each field by.
pub(crate) struct Records<'a> {
    types: &'a IdTable<StructuralTypeId, RecordType>,
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
pub(crate) fn declare<'a>(
    entry: MachineId,
    types: &'a IdTable<StructuralTypeId, RecordType>,
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
    Ok(Declared {
        records: Records {
            types,
            field_identities,
        },
        identities: identities.build(),
        structural_types,
        native_layouts,
    })
}

/// What a slot holds: a scalar slot the value bound to it, a record slot the
/// root place of the value its formal or initializer established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bound {
    Scalar(ValueId),
    Record(PlaceId),
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

    /// What a scalar leaf holds at the point lowering has reached: the last
    /// write on it or on an ancestor, or its entry content when nothing was
    /// written beneath a formal's root. See the module comment for why the
    /// last write is the reaching one in this subset.
    fn held(&mut self, leaf: ExactPlaceId, scalar_type: ScalarType) -> Checked<Content> {
        let mut latest = None;
        let mut place = leaf;
        let root = loop {
            let row = terminal_check_tables::row(
                &self.tables.places,
                place,
                self.id,
                Construct::Place(place),
            )?;
            if let Some(write) = row.writes.last().copied()
                && latest.is_none_or(|(known, _)| write > known)
            {
                latest = Some((write, place));
            }
            match row.origin {
                PlaceOrigin::Root(root) => break root,
                PlaceOrigin::Projection { parent, .. } => place = parent,
            }
        };
        let Some((write, written)) = latest else {
            let (kind, _) =
                terminal_check_tables::row(&self.roots, root, self.id, Construct::Place(leaf))?;
            return match kind {
                StructuralPlaceKind::Parameter { .. } => {
                    self.entry_content(leaf, scalar_type).map(Content::Value)
                }
                _ => Err(self.internal("a place is read before anything was written to it")),
            };
        };
        let content = terminal_check_tables::row(
            &self.tables.writes,
            write,
            self.id,
            Construct::Place(written),
        )?
        .content;
        match content {
            Content::Value(_) if written == leaf => Ok(content),
            Content::Value(_) => Err(self.internal("a scalar value was written to a record")),
            Content::Overwritten => Ok(Content::Overwritten),
            Content::Reference(_) | Content::Vacated => Err(refuse(
                self.id,
                Construct::Place(written),
                Unserved::Expression("place holding a reference or a hole"),
            )),
        }
    }

    /// Writes, leaf by leaf, what `from` holds into `to`: the log entry of a
    /// whole copy or of a record operand moved into its field.
    fn copy_contents(
        &mut self,
        from: ExactPlaceId,
        to: ExactPlaceId,
        value_type: ValueType,
        at: ProgramPoint,
    ) -> Checked<()> {
        match value_type {
            ValueType::Scalar(scalar_type) => {
                let content = self.held(from, scalar_type)?;
                self.write(to, at, content)
            }
            ValueType::Record(record) => {
                for field in &self.record_type(record)?.fields {
                    let from = self.project(from, field.id)?;
                    let to = self.project(to, field.id)?;
                    self.copy_contents(from, to, field.value_type, at)?;
                }
                Ok(())
            }
        }
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
                    Some(Bound::Scalar(_)) => {
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
            let identity = terminal_check_tables::row(
                &self.records.field_identities,
                *owner,
                self.id,
                Construct::Machine,
            )?
            .get(*field)
            .copied()
            .ok_or_else(|| self.internal("a field has no declared identity"))?;
            path.push(StructuralPathSegment::Field(identity));
        }
        let operation = self.operation_identity(expression.source);
        let (root, exact) = self.produce(
            operation,
            at.record,
            OperationKind::StructuralLeafCopy {
                source: at.root,
                path,
            },
        )?;
        self.copy_contents(
            at.place,
            exact,
            ValueType::Record(at.record),
            ProgramPoint::AfterOperation(operation),
        )?;
        Ok(root)
    }

    /// `R { a: x, b: y }`: the operands in authored order, then one
    /// `EstablishRecord` whose roster is in declaration order
    /// (structural_access.md: "a complete roster of declaration-identified
    /// field operands"), then one write per scalar leaf of the fresh root.
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
                Operand::Record(source) => {
                    let source = self.exact_root(source)?;
                    self.copy_contents(source, leaf, field.value_type, at)?;
                }
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
        // Asking what the leaf holds mints its entry content value when it
        // lies beneath a formal and nothing was written to it: the value the
        // checker's read equation names.
        self.held(leaf, scalar_type)?;
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
