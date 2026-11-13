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
- A zero establishment zeroes its fresh root. The operation has no operand,
  so the zero of a scalar leaf has a value identity only through a constant:
  elaboration emits one zero constant per scalar leaf of the zero value
  directly before the establishment, in leaf order, and that is where it is
  looked for. A leaf whose slot holds anything else is zero with no value
  identity.
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
- an entry content value must be of a scalar leaf of a formal, with that
  leaf's declared type (`Storage::entry_contents_are_of_formals`).
*/

use semantic_vocabulary::{
    BlockId, IdTable, IntegerValue, MachineId, OperationId, PsiSemanticId, ScalarType,
    StructuralCaseId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::{Checked, Construct, ExactPlaceId, MachineTables, ProgramPoint};
use terminal_psi::{
    Block, Operation, OperationKind, RecordFieldValue, StructuralAccess, StructuralFieldType,
    StructuralTypeShape,
};

use super::{Lineage, Shape, Storage, index, lineage};
use crate::refusal;
use terminal_check_tables::RefusalReason;

/// What one store put into its place.
pub(super) enum Stored {
    /// The exact region transferred its value to another owner.
    Vacated,
    /// Exactly this scalar value.
    Value(ValueId),
    /// What this other place held just before the store, leaf for leaf.
    Moved(Lineage),
    /// A sum established whole as this case. Its payload operands are
    /// stores of their own, after this one.
    Case(StructuralCaseId),
    /// The zero value, whole. The scalar leaves that have a zero constant
    /// are stores of their own, after this one.
    Zeroed,
    /// What a call returned, or left in storage this machine lent it
    /// exclusively: contents its callee established, with no value identity
    /// in this machine.
    Established,
}

impl Stored {
    /// The same store as a write-log row can say it. `Content` has no form
    /// for a source place, a case or a call, so those are all "new contents".
    fn content(&self) -> Content {
        match self {
            Self::Vacated => Content::Vacated,
            Self::Value(value) => Content::Value(*value),
            Self::Moved(_) | Self::Case(_) | Self::Zeroed | Self::Established => {
                Content::Overwritten
            }
        }
    }
}

/// One store, located in the graph.
pub(super) struct Logged {
    pub(super) lineage: Lineage,
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
    let mut writes: Vec<Vec<terminal_check_tables::WriteId>> =
        vec![Vec::new(); tables.places.len()];
    for (write, row) in tables.writes.iter() {
        writes
            .get_mut(index(row.place.get()))
            .ok_or_else(|| {
                foreign(
                    machine,
                    Construct::Place(row.place),
                    "a write row names a place past the end of this machine",
                )
            })?
            .push(write);
    }
    for ((place, row), written) in tables.places.iter().zip(&writes) {
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
        if row.writes != *written {
            return Err(foreign(
                machine,
                concerns,
                "a place's writes are not the rows of the log that name it",
            ));
        }
    }
    Ok(())
}

/// The result of `operation` when it is a constant zero of `scalar_type`.
fn zero_constant(operation: &Operation, scalar_type: ScalarType) -> Option<ValueId> {
    let result = operation.result.as_scalar()?;
    let zero = match operation.kind {
        OperationKind::BooleanConstant { value } => !value,
        OperationKind::IntegerConstant { value } => {
            matches!(value, IntegerValue::Signed(0) | IntegerValue::Unsigned(0))
        }
        _ => false,
    };
    (zero && result.scalar_type == scalar_type).then_some(result.id)
}

impl Storage<'_> {
    /// Every store of the machine, in block and operation order.
    pub(super) fn stores(
        &self,
        value_types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Vec<Logged>> {
        let mut log = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                for (lineage, stored) in self.stored_by(block, position, value_types)? {
                    log.push(Logged {
                        lineage,
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

    /// What the operation at `position` of `block` stores, in the order its
    /// stores take effect: a whole store before the stores beneath it.
    fn stored_by(
        &self,
        block: &Block,
        position: usize,
        value_types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Vec<(Lineage, Stored)>> {
        let operation = &block.operations[position];
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
        match &operation.kind {
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
                rows.insert(0, (root, Stored::Case(*result_case)));
            }
            OperationKind::EstablishZeroedStructural {} => {
                let mut at = fresh()?;
                let zeroed = produced.map(|result| result.structural_type);
                let mut leaves = Vec::new();
                if let Some(zeroed) = zeroed {
                    self.zero_scalars(zeroed, &mut at, &mut leaves, concerns)?;
                }
                rows.push((at, Stored::Zeroed));
                // One constant per leaf, directly before the establishment.
                let constants = position
                    .checked_sub(leaves.len())
                    .map(|first| &block.operations[first..position]);
                for (slot, (leaf, scalar_type)) in leaves.into_iter().enumerate() {
                    let zero = constants
                        .and_then(|constants| zero_constant(&constants[slot], scalar_type));
                    if let Some(zero) = zero {
                        rows.push((leaf, Stored::Value(zero)));
                    }
                }
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                rows.push((
                    fresh()?,
                    Stored::Moved(self.stored_at(*source, path, concerns)?),
                ));
            }
            OperationKind::MoveStructuralValue { source } => {
                let source = self.argument(source, concerns)?;
                rows.push((fresh()?, Stored::Moved(source.clone())));
                rows.push((source, Stored::Vacated));
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
                field: Some(field),
                value,
                ..
            } => {
                let carrier = self.stored_at(*destination, path, concerns)?;
                rows.push((
                    beneath(&carrier, &[Segment::Field(*field)])?,
                    Stored::Moved(self.argument(value, concerns)?),
                ));
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field: None,
                value,
                ..
            } => {
                rows.push((
                    self.stored_at(*destination, path, concerns)?,
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
            OperationKind::EstablishPrimitiveLocal { value, .. } => {
                rows.push((fresh()?, Stored::Value(*value)));
            }
            // The operations that only observe storage.
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::ScalarStructuralField { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | OperationKind::SelectMachine { .. } => {}
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
                for argument in kind.structural_arguments() {
                    match argument.access {
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                            rows.push((self.argument(argument, concerns)?, Stored::Established));
                        }
                        StructuralAccess::Owned | StructuralAccess::SharedBorrow => {}
                    }
                }
            }
            // An operation over scalar values alone, by Terminal semantics'
            // own word. Anything else may store somewhere this derivation
            // does not know of.
            _ => {
                if produced.is_some()
                    || crate::result_equation(self.machine, operation, value_types)?.is_none()
                {
                    return Err(
                        self.unsupported(concerns, "an operation whose stores are not derived")
                    );
                }
            }
        }
        Ok(rows)
    }

    /// The scalar leaves of the zero value of `structural_type` at `at`,
    /// with their types, fields in declaration order; a sum contributes the
    /// payload of its first declared case.
    fn zero_scalars(
        &self,
        structural_type: StructuralTypeId,
        at: &mut Lineage,
        found: &mut Vec<(Lineage, ScalarType)>,
        concerns: Construct,
    ) -> Checked<()> {
        let (case, fields) = match &self.declaration(structural_type, concerns)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                found.push((at.clone(), *scalar_type));
                return Ok(());
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                for index in 0..*length {
                    at.segments.push(Segment::FixedIndex(index));
                    self.zero_scalars(*element, at, found, concerns)?;
                    at.segments.pop();
                }
                return Ok(());
            }
            StructuralTypeShape::Record { fields, .. } => (None, fields),
            StructuralTypeShape::Sum { cases } => {
                let first = cases
                    .first()
                    .ok_or_else(|| self.foreign(concerns, "a sum declares no case"))?;
                (Some(first.id), &first.fields)
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain data")),
        };
        if let Some(case) = case {
            at.segments.push(Segment::Case(case));
        }
        for field in fields {
            at.segments.push(Segment::Field(field.id));
            match field.field_type {
                StructuralFieldType::Scalar(scalar_type) => found.push((at.clone(), scalar_type)),
                StructuralFieldType::Structural(nested) => {
                    self.zero_scalars(nested, at, found, concerns)?;
                }
                // The empty shared view has no scalar leaf.
                ref stored if super::is_stored_view(stored) => {}
                _ => return Err(self.unsupported(concerns, "a field type outside plain data")),
            }
            at.segments.pop();
        }
        if case.is_some() {
            at.segments.pop();
        }
        Ok(())
    }

    /// Refuses a write log that is not, operation by operation and in
    /// order, the stores derived from the lowered form.
    pub(super) fn log_is_stated(&self, tables: &MachineTables) -> Checked<()> {
        // The log's rows at each operation, by `OperationId - 1`, in log
        // order.
        let mut stated: Vec<Vec<(Lineage, Content)>> = Vec::new();
        for (_, row) in tables.writes.iter() {
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
            let concerns = Construct::Operation(store.operation);
            let slot = index(store.operation.get());
            let row = stated
                .get(slot)
                .and_then(|rows| rows.get(matched[slot]))
                .ok_or_else(|| self.foreign(concerns, "an operation's store has no write row"))?;
            if row.0 != store.lineage || row.1 != store.stored.content() {
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

    /// Refuses an entry content value that is not of a scalar leaf of a
    /// formal, typed as that leaf is declared. Only a formal holds anything
    /// at entry, and the walk hands such a value to every read no store
    /// reaches.
    pub(super) fn entry_contents_are_of_formals(&self, tables: &MachineTables) -> Checked<()> {
        for (slot, value) in self.entry_contents.iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let concerns = Construct::Value(*value);
            let place = ExactPlaceId::new(slot as u64 + 1)
                .ok_or_else(|| self.foreign(concerns, "a place identity out of range"))?;
            let leaf = lineage(self.machine, &tables.places, place)?;
            let declared = match self.root_kind(leaf.root, concerns)? {
                StructuralPlaceKind::Parameter { .. } => match self.shape(&leaf, concerns)? {
                    Shape::Scalar(scalar_type) => Some(scalar_type),
                    Shape::Typed(_) | Shape::View | Shape::Payload(_) => None,
                },
                _ => None,
            };
            let stated = tables.values.get(*value).map(|row| row.scalar_type);
            if declared.is_none() || declared != stated {
                return Err(self.foreign(
                    concerns,
                    "an entry content value of storage that is no scalar leaf of a formal",
                ));
            }
        }
        Ok(())
    }
}
