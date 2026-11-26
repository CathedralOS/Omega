/*
What each operation stores, derived from the lowered form, and the checks
that elaboration's tables say the same.

Every goal over a field is stated over the value its leaf holds: a read
equation, a data type's `requires` clause, a requirement over a field of a
record actual. If the answer came from elaboration's write log, one row
logged with another value would prove another goal and nothing would notice.
So the walk reads the stores from the operations themselves
(`Storage::stores`), which state the same thing:

- A record literal stores each operand into its field of the fresh root: a
  scalar operand as that value, a record operand as what its source held.
- A case literal establishes its fresh root as `result_case`, then stores
  each payload operand the same way.
- A zero establishment writes its fresh root once. Each checked read derives
  the typed zero for its selected leaf from that store.
- A whole copy stores what its source held into its fresh root, and a record
  stored into a field stores what its source held into that field.
- A scalar store stores its value into exactly the leaf it names, and a view
  store puts what its source descriptor held into exactly the field it names.
- A call's record result holds what the callee returned, and a place the
  call borrows exclusively holds what the callee left there.
- A scalar local declared without an initializer is zero-constructed
  (`local-without-initializer`): `EstablishPrimitiveLocal` stores its
  operand into the fresh home.

An operation that is none of these, reads no storage and has no scalar
semantics is refused as `Unsupported`: it may store somewhere, and an
operation left out here would be a store nothing reaches.

Elaboration's tables stay honest records of the same facts, so they are
compared and a difference is a `ForeignIdentity` refusal, which `judge`
returns as `Err`:

- the write log must list, per operation and in order, exactly the stores
  derived here (`Storage::log_is_stated`);
- the place table must hold one place per path, since a place identity
  stands for its path wherever a table names one
  (`places_are_interned`);
- a storage snapshot must name a scalar leaf at its owner's entry or the
  normal return of its exact establishing call, with the leaf's type
  (`Storage::initial_contents_are_declared`).
*/

use arena::{IdTable, NumericId};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, StructuralCaseId, ValueId};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::{Checked, Construct, MachineTables, ProgramPoint};
use terminal_psi::{
    Operation, OperationKind, RecordFieldValue, StructuralAccess, StructuralTypeShape,
};

use super::{Backing, Lineage, Shape, Storage, StorageAddresses, index, lineage};
use crate::refusal;
use terminal_check_tables::RefusalReason;

/// What one store put into its place.
pub(super) enum Stored {
    /// The exact region transferred its value to another owner.
    Vacated,
    /// Exactly this scalar value.
    Value(ValueId),
    /// An erased scalar field term recorded in `MachineTables::erased_field_terms`.
    ErasedScalar(usize),
    /// What this other place held just before the store, leaf for leaf.
    Moved(Lineage),
    /// A sum established whole as this case. Its payload operands are
    /// stores of their own, after this one.
    Case(StructuralCaseId),
    /// The authenticated zero value of the whole result.
    Zeroed,
    /// What a call returned, or left in storage this machine lent it
    /// exclusively. Only this call may supply the returned leaf identities;
    /// an earlier snapshot never names the contents after it.
    Established,
    /// A call may write through contents whose backing cannot be enumerated.
    /// This invalidates possible aliases; it establishes no replacement value.
    UnknownPayloadWrite {
        receiving: Backing,
        contents: terminal_semantics::descriptor_results::UnknownDescriptorContents,
    },
}

impl Stored {
    /// The same store as a write-log row can say it. `Content` has no form
    /// for a source place, a case or a call, so those are all "new contents".
    fn content(&self) -> Content {
        match self {
            Self::Vacated => Content::Vacated,
            Self::Value(value) => Content::Value(*value),
            Self::ErasedScalar(_)
            | Self::Moved(_)
            | Self::Case(_)
            | Self::Zeroed
            | Self::Established
            | Self::UnknownPayloadWrite { .. } => Content::Overwritten,
        }
    }
}

/// One store, located in the graph.
pub(super) struct Logged {
    /// The logical destination is retained for permission checks after the
    /// reaching-write owner resolves its separately captured backing.
    pub(super) destination: Lineage,
    pub(super) backing: Backing,
    pub(super) block: BlockId,
    /// The normal continuation of its operation, as `Graph::locate` orders
    /// points: one past the operation's index in the block.
    pub(super) position: usize,
    pub(super) operation: OperationId,
    pub(super) stored: Stored,
}

fn foreign(
    machine: MachineId,
    concerns: Construct,
    detail: &str,
) -> terminal_check_tables::Refusal {
    refusal(RefusalReason::ForeignIdentity, machine, concerns, detail)
}

/// Refuses a place table that is not an interning of paths: one row per
/// root and per (parent, segment) pair, each listed by its parent, and each
/// row's `writes` column the rows of the log that name it.
///
/// A table names a place by its identity (an invocation's record actual, the
/// place of an entry content value, a write row), and the walk reads that
/// identity as a path. Two paths that share a row, or a row whose parent
/// link says another path than the one that leads to it, would let one
/// place's contents answer for another's.
pub(super) fn places_are_interned(machine: MachineId, tables: &MachineTables) -> Checked<()> {
    for (root, exact) in tables.roots.iter() {
        let own = tables
            .places
            .get(*exact)
            .is_some_and(|row| row.origin == PlaceOrigin::Root(root));
        if !own {
            return Err(foreign(
                machine,
                Construct::Place(*exact),
                "a root place's row is not its own",
            ));
        }
    }
    for row in tables.writes.values() {
        if tables.places.get(row.place).is_none() {
            return Err(foreign(
                machine,
                Construct::Place(row.place),
                "a write row names a place past the end of this machine",
            ));
        }
    }
    for (place, row) in tables.places.iter() {
        let concerns = Construct::Place(place);
        let listed = match row.origin {
            PlaceOrigin::Root(root) => tables.roots.get(root) == Some(&place),
            // A parent is minted before its projections, so every parent
            // chain ends at a root.
            PlaceOrigin::Projection { parent, segment } => {
                parent < place
                    && tables.places.get(parent).is_some_and(|parent| {
                        parent
                            .children
                            .binary_search_by_key(&segment, |(segment, _)| *segment)
                            .is_ok_and(|position| parent.children[position].1 == place)
                    })
            }
        };
        if !listed {
            return Err(foreign(
                machine,
                concerns,
                "a place row is not the projection its parent lists",
            ));
        }
        if !row.children.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            return Err(foreign(
                machine,
                concerns,
                "a place's projections are not sorted, one per segment",
            ));
        }
        for (segment, child) in &row.children {
            let projects = tables.places.get(*child).is_some_and(|child| {
                child.origin
                    == PlaceOrigin::Projection {
                        parent: place,
                        segment: *segment,
                    }
            });
            if !projects {
                return Err(foreign(
                    machine,
                    concerns,
                    "a place row is not the projection its parent lists",
                ));
            }
        }
    }
    Ok(())
}

impl<'a> Storage<'a> {
    /// Every store of the machine, in block and operation order.
    pub(super) fn stores(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        tables: &MachineTables,
        value_types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Vec<Logged>> {
        let mut log = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                for (lineage, stored) in
                    self.stored_by(references, tables, operation, value_types)?
                {
                    log.push(Logged {
                        destination: lineage.clone(),
                        backing: Backing::direct(lineage),
                        block: block.id,
                        position: position + 1,
                        operation: operation.id,
                        stored,
                    });
                }
            }
        }
        Ok(log)
    }

    /// The stores an operation makes, with each whole store before its fields.
    fn stored_by(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        tables: &MachineTables,
        operation: &Operation,
        value_types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Vec<(Lineage, Stored)>> {
        let concerns = Construct::Operation(operation.id);
        let produced = operation.result.as_structural();
        let fresh = || {
            produced
                .map(|result| Lineage {
                    root: result.place,
                    segments: Vec::new(),
                })
                .ok_or_else(|| {
                    self.foreign(concerns, "an operation that makes storage has no place")
                })
        };
        let beneath = |place: &Lineage, steps: &[Segment]| -> Checked<Lineage> {
            let mut leaf = place.clone();
            leaf.segments.extend_from_slice(steps);
            // Resolving the shape checks each step against its type.
            self.shape(&leaf, concerns)?;
            Ok(leaf)
        };
        let operand = |value: &RecordFieldValue| -> Checked<Stored> {
            Ok(match value {
                RecordFieldValue::Scalar { value, .. } => Stored::Value(*value),
                RecordFieldValue::Structural(argument) => {
                    Stored::Moved(self.argument(argument, concerns)?)
                }
            })
        };
        let mut rows = Vec::new();
        if let Some(source) = self.moved_source(operation)? {
            rows.push((fresh()?, Stored::Moved(source.clone())));
            rows.push((source, Stored::Vacated));
            return Ok(rows);
        }
        match &operation.kind {
            OperationKind::AtomicAccess {
                place,
                path,
                event,
                resident_obligation,
            } => {
                let leaf = self.stored_at(*place, path, concerns)?;
                let Shape::Atomic(cell) = self.shape(&leaf, concerns)? else {
                    return Err(self.foreign(
                        concerns,
                        "atomic event does not name dedicated atomic storage",
                    ));
                };
                if !cell.is_supported() || cell.bounds().is_some() || resident_obligation.is_some()
                {
                    return Err(self.unsupported(
                        concerns,
                        "atomic resident requires its checked establishment obligation",
                    ));
                }
                if !event.ordering_is_legal() {
                    return Err(self.foreign(concerns, "atomic event has an illegal ordering"));
                }
                if event
                    .operands()
                    .iter()
                    .any(|operand| value_types.get(*operand) != Some(&cell.scalar_type()))
                {
                    return Err(
                        self.foreign(concerns, "atomic operand differs from its resident type")
                    );
                }
                match operation.result {
                    terminal_psi::OperationResult::Scalar(result)
                        if event.observes_resident()
                            && result.scalar_type == cell.scalar_type() => {}
                    terminal_psi::OperationResult::Unit if !event.observes_resident() => {}
                    _ => return Err(self.foreign(concerns, "atomic result differs from its event")),
                }
                if matches!(
                    event,
                    terminal_psi::AtomicAccessEvent::CompareExchangeOnce { .. }
                ) {
                    return Err(self.unsupported(concerns, "single-attempt atomic outcome custody"));
                }
                if !matches!(
                    event,
                    terminal_psi::AtomicAccessEvent::Load { .. }
                        | terminal_psi::AtomicAccessEvent::Store { .. }
                ) && !matches!(cell.scalar_type(), ScalarType::Integer(_))
                {
                    return Err(
                        self.foreign(concerns, "atomic update requires an integer resident")
                    );
                }
                if event.modifies_resident() {
                    rows.push((leaf, Stored::Established));
                }
            }
            OperationKind::EstablishStructuralLocal {} => {
                rows.push((fresh()?, Stored::Vacated));
            }
            // Literal bytes establish the explicitly named home. Unlike a
            // record constructor, this operation has no structural result.
            OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
                rows.push((
                    self.stored_at(*destination, &[], concerns)?,
                    Stored::Established,
                ));
            }
            OperationKind::EstablishScalarArray { elements } => {
                let root = fresh()?;
                rows.push((root.clone(), Stored::Established));
                for (index, value) in elements.iter().enumerate() {
                    let index = u64::try_from(index)
                        .map_err(|_| self.foreign(concerns, "array element index exceeds u64"))?;
                    rows.push((
                        beneath(&root, &[Segment::FixedIndex(index)])?,
                        Stored::Value(*value),
                    ));
                }
            }
            OperationKind::EstablishStructuralArray { elements } => {
                let root = fresh()?;
                rows.push((root.clone(), Stored::Established));
                let shape = &self
                    .declaration(self.root_type(root.root, concerns)?, concerns)?
                    .shape;
                if matches!(shape, StructuralTypeShape::ElementView { .. }) {
                    if !super::is_shared_view(shape) {
                        return Err(self.unsupported(concerns, "mutable sequence construction"));
                    }
                    // The operation's actual element arguments are required
                    // established by operation_uses. Its result is a descriptor,
                    // not inline storage with fixed-index child places.
                    return Ok(rows);
                }
                for (index, value) in elements.iter().enumerate() {
                    let index = u64::try_from(index)
                        .map_err(|_| self.foreign(concerns, "array element index exceeds u64"))?;
                    rows.push((
                        beneath(&root, &[Segment::FixedIndex(index)])?,
                        Stored::Moved(self.argument(value, concerns)?),
                    ));
                }
            }
            OperationKind::EstablishRecord { fields } => {
                let root = fresh()?;
                for field in fields {
                    rows.push((
                        beneath(&root, &[Segment::Field(field.field)])?,
                        operand(&field.value)?,
                    ));
                }
                for (index, erased) in tables.erased_field_terms.iter().enumerate() {
                    if erased.operation == operation.id {
                        rows.push((
                            beneath(&root, &[Segment::Field(erased.field)])?,
                            Stored::ErasedScalar(index),
                        ));
                    }
                }
            }
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            } => {
                let root = fresh()?;
                for field in fields {
                    rows.push((
                        beneath(
                            &root,
                            &[Segment::Case(*result_case), Segment::Field(field.field)],
                        )?,
                        Stored::Value(field.value),
                    ));
                }
                for (index, erased) in tables.erased_field_terms.iter().enumerate() {
                    if erased.operation == operation.id {
                        rows.push((
                            beneath(
                                &root,
                                &[Segment::Case(*result_case), Segment::Field(erased.field)],
                            )?,
                            Stored::ErasedScalar(index),
                        ));
                    }
                }
                rows.insert(0, (root, Stored::Case(*result_case)));
            }
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } => {
                let root = fresh()?;
                for field in fields {
                    rows.push((
                        beneath(
                            &root,
                            &[Segment::Case(*result_case), Segment::Field(field.field)],
                        )?,
                        operand(&field.value)?,
                    ));
                }
                for (index, erased) in tables.erased_field_terms.iter().enumerate() {
                    if erased.operation == operation.id {
                        rows.push((
                            beneath(
                                &root,
                                &[Segment::Case(*result_case), Segment::Field(erased.field)],
                            )?,
                            Stored::ErasedScalar(index),
                        ));
                    }
                }
                rows.insert(0, (root, Stored::Case(*result_case)));
            }
            OperationKind::EstablishZeroedStructural {} => {
                let at = fresh()?;
                let result = produced.ok_or_else(|| {
                    self.foreign(concerns, "zero establishment has no structural result")
                })?;
                // prepare_replay already checks this exact immutable module's
                // zero eligibility and qualifications through the shared
                // zeroed-declaration graph in scalar_qualifications::validate.
                // Retain fresh-result authorization here without a second
                // whole declaration walk for every tiny establishment.
                if !matches!(self.root_kind(at.root, concerns)?, semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)
                    || !matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    || !result.claims.is_empty()
                    || !result.qualification_establishments.is_empty()
                {
                    return Err(
                        self.foreign(concerns, "zero establishment has an invalid result shape")
                    );
                }
                rows.push((at, Stored::Zeroed));
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                rows.push((
                    fresh()?,
                    Stored::Moved(self.stored_at(*source, path, concerns)?),
                ));
            }
            OperationKind::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                ..
            } => {
                rows.push((
                    self.stored_at(*destination, path, concerns)?,
                    Stored::Value(*value),
                ));
            }
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } => {
                let carrier = self.stored_at(*destination, path, concerns)?;
                rows.push((
                    beneath(&carrier, &[Segment::Field(*field)])?,
                    Stored::Value(*value),
                ));
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => {
                rows.push((
                    self.projected_storage(*destination, path, *field, concerns)?,
                    Stored::Moved(self.argument(value, concerns)?),
                ));
            }
            // The field takes the source's descriptor (structural_access.md:
            // "The field then denotes the source's exact backing and
            // extent"), so it holds what the source held. The whole-home
            // form (`field: None`) is not derived and stays refused below.
            OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field: Some(field),
                source,
                ..
            } => {
                let carrier = self.stored_at(*destination, path, concerns)?;
                let stored = beneath(&carrier, &[Segment::Field(*field)])?;
                if !matches!(self.shape(&stored, concerns)?, Shape::View) {
                    return Err(
                        self.foreign(concerns, "a view store into a field that stores none")
                    );
                }
                rows.push((
                    stored,
                    Stored::Moved(self.stored_at(*source, &[], concerns)?),
                ));
            }
            OperationKind::ByteSequenceSubslice { .. }
            | OperationKind::ElementViewSubslice { .. } => {
                self.subslice(references, operation)?;
                rows.push((fresh()?, Stored::Established));
            }
            OperationKind::EstablishReference { .. }
            | OperationKind::EstablishElementView { .. }
            | OperationKind::EstablishMutableElementView { .. } => {
                rows.push((fresh()?, Stored::Established));
            }
            OperationKind::ReleaseReference { source, path } => {
                rows.push((
                    self.released_reference(*source, path, concerns)?,
                    Stored::Vacated,
                ));
            }
            OperationKind::EstablishPrimitiveLocal { value, .. } => {
                rows.push((fresh()?, Stored::Value(*value)));
            }
            OperationKind::ByteSequenceWrite { destination, .. } => {
                self.writable_byte_view(operation, *destination)?;
                // Payload writes are tracked by descriptor backing analysis.
                // The descriptor itself retains its extent and initialization.
            }
            // A checked decomposition ends the wrapper role, not its bytes.
            // Shared affine replay authenticates the complete component roster.
            OperationKind::DecomposeStructuralValue { .. } => {}
            OperationKind::PackedIntegerBinary { operation: primitive, left, right } => {
                let result = produced.ok_or_else(|| self.foreign(concerns, "packed arithmetic has no owned array result"))?;
                let expected = result.structural_type;
                if terminal_semantics::packed_integer_array_type(self.types.iter(), expected, *primitive).is_none()
                    || !result.is_unrestricted_plain()
                    || !result.additional_input_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                {
                    return Err(self.foreign(concerns, "packed arithmetic has another result shape or authority"));
                }
                for argument in [left, right] {
                    let place = self.argument(argument, concerns)?;
                    if argument.access != StructuralAccess::SharedBorrow || self.projection_type(&place, concerns) != Some(expected) {
                        return Err(self.foreign(concerns, "packed arithmetic requires shared arrays of the result type"));
                    }
                }
                rows.push((fresh()?, Stored::Established));
            }
            OperationKind::PackedIntegerUnary { operation: primitive, source } => {
                let result = produced.ok_or_else(|| self.foreign(concerns, "packed unary operation has no owned array result"))?;
                let expected = result.structural_type;
                if terminal_semantics::packed_integer_unary_array_type(
                    self.types.iter(),
                    expected,
                    *primitive,
                    terminal_semantics::PackedArrayRole::Result,
                ).is_none()
                    || !result.is_unrestricted_plain()
                    || !result.additional_input_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                {
                    return Err(self.foreign(concerns, "packed unary operation has another result shape or authority"));
                }
                let place = self.argument(source, concerns)?;
                let Some(input_type) = self.projection_type(&place, concerns) else {
                    return Err(self.foreign(concerns, "packed unary operation requires a shared input array"));
                };
                if source.access != StructuralAccess::SharedBorrow
                    || terminal_semantics::packed_integer_unary_array_type(
                        self.types.iter(),
                        input_type,
                        *primitive,
                        terminal_semantics::PackedArrayRole::Input,
                    ).is_none()
                {
                    return Err(self.foreign(concerns, "packed unary operation requires a shared array of the declared input type"));
                }
                rows.push((fresh()?, Stored::Established));
            }
            // These operations observe their exact operands and store nothing;
            // readability and same-view measurements remain independently checked.
            OperationKind::ByteSequenceEqual { .. }
            | OperationKind::ByteSequencePredicate { .. } => self.observed_byte_views(operation)?,
            // A fence orders observations without replacing any stored value.
            OperationKind::AtomicFence { .. } => {}
            // The operations that only observe storage.
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::StorageAddress { .. }
            | OperationKind::StorageAlignment { .. }
            | OperationKind::ByteSequenceLength { .. }
            | OperationKind::ElementViewLength { .. }
            | OperationKind::ByteSequenceRead { .. }
            | OperationKind::ElementViewRead { .. }
            | OperationKind::StructuralByteSequenceFieldLength { .. }
            | OperationKind::StructuralByteSequenceFieldRead { .. }
            | OperationKind::ScalarStructuralField { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | OperationKind::SelectMachine { .. }
            | OperationKind::ReadScalarApplication { .. } => {}
            kind if kind.module_invocation().is_some()
                || kind.callable_invocation().is_some()
                || matches!(kind, OperationKind::BoundaryCall { .. }) =>
            {
                if produced.is_some() {
                    rows.push((fresh()?, Stored::Established));
                }
                // An exclusive loan lets the callee store anything beneath
                // the lent place; a shared one lets it store nothing.
                // loans.md: "Borrowed arguments continue to denote the
                // original storage".
                if let Some(regions) = terminal_semantics::borrowed_storage::call_write_regions(
                    self.module,
                    self.lowered,
                    operation,
                )
                .map_err(|detail| self.foreign(concerns, &detail))?
                {
                    for region in regions {
                        let root = region.root.ok_or_else(|| {
                            self.foreign(concerns, "a known call write region has no root")
                        })?;
                        let path = region.written_storage_path();
                        rows.push((self.stored_at(root, &path, concerns)?, Stored::Established));
                    }
                    return Ok(rows);
                }
                for argument in kind.structural_arguments() {
                    match argument.access {
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                            rows.push((self.argument(argument, concerns)?, Stored::Established));
                        }
                        StructuralAccess::Owned | StructuralAccess::SharedBorrow => {}
                    }
                }
                // The callee can also store through the `@mutable` references
                // an argument carries. Each such referent is a write of the
                // call, after every argument's own row, in the order
                // lowering writes them (`publish_write_frames`).
                for argument in kind.structural_arguments() {
                    if !matches!(
                        argument.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::Owned
                    ) {
                        continue;
                    }
                    let lent = self.argument(argument, concerns)?;
                    let Shape::Typed(structural_type) = self.shape(&lent, concerns)? else {
                        continue;
                    };
                    let referents = terminal_semantics::borrowed_storage::carried_exclusive_referents(
                        self.types,
                        structural_type,
                        argument.access,
                    )
                    .ok_or_else(|| {
                        self.unsupported(
                            concerns,
                            "a call argument carries references whose referents have no exact paths",
                        )
                    })?;
                    for referent in referents {
                        rows.push((beneath(&lent, &referent)?, Stored::Established));
                    }
                }
            }
            // An operation over scalar values alone, by Terminal semantics'
            // own word. Anything else may store somewhere this derivation
            // does not know of.
            _ => {
                if produced.is_some()
                    || crate::result_equation(self.module, self.lowered, operation, value_types)?
                        .is_none()
                {
                    return Err(
                        self.unsupported(concerns, "an operation whose stores are not derived")
                    );
                }
            }
        }
        Ok(rows)
    }

    /// The authenticated store log is immutable once captured addresses and
    /// byte-payload effects have been installed. Preserve the log's order,
    /// including multiple writes at the same point.
    pub(super) fn index_reaching_writes(&mut self) {
        let block_count = self.graph.block_count();
        self.writes_by_block.clear();
        self.writes_by_block.resize_with(block_count, Vec::new);
        self.block_log_ranges.clear();
        self.block_log_ranges.resize(block_count, 0..0);
        for (log_index, row) in self.log.iter().enumerate() {
            let slot = index(row.block.get());
            if let Some(range) = self.block_log_ranges.get_mut(slot) {
                if range.start == range.end {
                    *range = log_index..(log_index + 1);
                } else {
                    range.end = log_index + 1;
                }
            }
            let Some(block_roots) = self.writes_by_block.get_mut(slot) else {
                continue;
            };
            let mut previous = None;
            for address in 0..row.backing.address_count() {
                let root = row.backing.address(address).root;
                if previous.replace(root) != Some(root) {
                    match block_roots.binary_search_by_key(&root, |(id, _)| *id) {
                        Ok(pos) => block_roots[pos].1.push(log_index),
                        Err(pos) => block_roots.insert(pos, (root, vec![log_index])),
                    }
                }
            }
        }
    }

    /// Writes in `block` at or before `up_to` that share a physical root with
    /// `watched`, yielded in reverse authored order (latest position and latest
    /// log entry first).
    pub(super) fn reaching_writes_rev(
        &self,
        block: BlockId,
        up_to: usize,
        watched: &Backing,
    ) -> impl Iterator<Item = (usize, &Logged)> {
        let slot = index(block.get());
        let root = watched.address(0).root;
        let (selected, all) = if watched.address(watched.address_count() - 1).root == root {
            let rows = self
                .writes_by_block
                .get(slot)
                .and_then(|block_roots| {
                    if block_roots.len() <= 8 {
                        block_roots
                            .iter()
                            .find(|(id, _)| *id == root)
                            .map(|(_, indices)| indices.as_slice())
                    } else {
                        block_roots
                            .binary_search_by_key(&root, |(id, _)| *id)
                            .ok()
                            .map(|pos| block_roots[pos].1.as_slice())
                    }
                })
                .unwrap_or(&[]);
            let end = if up_to == usize::MAX {
                rows.len()
            } else {
                rows.partition_point(|&log_index| self.log[log_index].position <= up_to)
            };
            (&rows[..end], 0..0)
        } else {
            // A query with several possible roots still considers every write
            // in the block, once, in reverse authored order.
            let range = self.block_log_ranges.get(slot).cloned().unwrap_or(0..0);
            let end = if up_to == usize::MAX {
                range.end
            } else {
                range.start + self.log[range.clone()].partition_point(|row| row.position <= up_to)
            };
            (&[][..], range.start..end)
        };
        selected
            .iter()
            .rev()
            .copied()
            .chain(all.rev())
            .map(|log_index| (log_index, &self.log[log_index]))
    }

    /// Refuses a write log that is not, operation by operation and in
    /// order, the stores derived from the lowered form.
    pub(super) fn log_is_stated(&self, tables: &MachineTables) -> Checked<()> {
        // The log's rows at each operation, by `OperationId - 1`, in log
        // order.
        let mut stated: Vec<Vec<(Lineage, Content)>> = Vec::new();
        for (_, row) in tables.writes.iter() {
            if matches!(row.at, ProgramPoint::AfterCleanup { .. }) {
                continue;
            }
            let ProgramPoint::AfterOperation(operation) = row.at else {
                return Err(self.foreign(
                    Construct::Place(row.place),
                    "a write row is not at the normal continuation of an operation",
                ));
            };
            let slot = index(operation.get());
            if slot >= self.graph.operation_count() {
                return Err(self.foreign(
                    Construct::Operation(operation),
                    "a write row states a store its operation does not make",
                ));
            }
            if stated.len() <= slot {
                stated.resize_with(slot + 1, Vec::new);
            }
            stated[slot].push((
                lineage(self.machine, &tables.places, row.place)?,
                row.content,
            ));
        }
        let mut matched = vec![0; stated.len()];
        for store in &self.log {
            if matches!(store.stored, Stored::ErasedScalar(_)) {
                continue;
            }
            let concerns = Construct::Operation(store.operation);
            let slot = index(store.operation.get());
            let row = stated
                .get(slot)
                .and_then(|rows| rows.get(matched[slot]))
                .ok_or_else(|| self.foreign(concerns, "an operation's store has no write row"))?;
            if row.0 != store.destination || row.1 != store.stored.content() {
                return Err(self.foreign(concerns, "a write row is not what its operation stores"));
            }
            matched[slot] += 1;
        }
        for (slot, (rows, matched)) in stated.iter().zip(&matched).enumerate() {
            if rows.len() != *matched {
                let concerns = OperationId::new(slot as u64 + 1)
                    .map_or(Construct::Machine, Construct::Operation);
                return Err(self.foreign(
                    concerns,
                    "a write row states a store its operation does not make",
                ));
            }
        }
        Ok(())
    }
}
