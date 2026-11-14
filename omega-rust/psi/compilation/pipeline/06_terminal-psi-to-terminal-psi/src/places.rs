/*
What a place holds when an operation uses it, answered once for reads, copies
and the initialization judgment by one walk over three things: the stores the
lowered operations make (`log`), the one overlap relation over the segments of
two places (`overlap`), and the lowered graph.

The stores are derived here from the operations and not read from
elaboration's write log, because every goal over a field is stated over the
value its leaf holds: a log that names another value would prove another
goal. The tables' own log, place table and entry content values are checked
against the lowered form before anything is asked of them (`log`).

The walk (`Storage::reaching`). From the point before a use of one scalar
leaf, go back over the graph. The last overlapping store met on a path is the
one that reaches along it. Three things can supply a path:

- a store of the value `v` into exactly the leaf supplies `v`;
- a store into the leaf or a place above it that moves storage whole (a whole
  copy, a record operand of a literal, a record stored into a field) supplies
  what the matching leaf of its source held just before it, and the walk
  continues from there. The exact declared call producing a root supplies
  that leaf's initial logical identity when retained; other overlapping
  stores supply contents without one;
- a path that meets no store before the machine's entry is supplied by the
  place's entry content when its root is a formal ("a valid initialized
  ordinary storage place at entry may supply its exact reaching value
  version", fact_validity.md), and by nothing otherwise.

A local declared without an initializer is one more supplier: a data
local's zero establishment stores a zero constant into each scalar leaf of
the zero value and establishes the first declared case of each sum; a scalar
local's establishment stores its zero constant into its home.

Two judgments read the answer, and so do the goal of a data type's
`requires` clause (`Storage::domain_goal`: what each field the clause names
holds once the owing operation completes) and the clauses a read learns from
its type (`Storage::read_premises`).

The read equation. fact_validity.md: "A primitive read may introduce an
equality between its copied SSA result and an exact reaching stored SSA
value. Every actual predecessor must supply that same value". A read gets
`read == v`, justified by the read operation's own semantics, exactly when
every path supplies the one value `v`. Different values on different paths,
or contents with no value identity, give no equation: the read is still well
formed, it just says nothing. control_flow.md: "A fact from one arm does not
survive the join."

Initialization (`Storage::uses`). Every operation and terminator that reads
storage (a field read, a whole copy, a record operand, a call argument, a
return) gets one verdict: each scalar leaf at or beneath the place it uses
must be supplied with an established value on every path. A path that
supplies nothing is a refusal with reason `Unestablished`. A local declared
without an initializer is zero-constructed (`local-without-initializer`), so
its zeros supply every leaf. Zeroed storage of a gated type is not yet a
value: its declaration opens an invariant window, and a use that observes a
zeroed leaf while it is open is `Unestablished` (`Storage::consumptions`).
dependent_values.md: "If zero satisfies the default domain, the value is
zero-constructible. Otherwise the type is gated: construction or checked
qualification must establish the domain before observation." This is a
structural judgment, not a proposition: it has no obligation row and no
certificate.

Invariant windows (`Storage::consumptions`). The same walk, asked which of
three writes a path meets last: the one that opened the window, a store
that proved the same clause of the same place, or a whole write to the
place. The first leaves it open, the others close it. Each use of storage
that consumes the place's domain and that the window reaches open is a
consumption point, where the clause is stated again over what the place
holds there.

Knowledge names values only. No row here states a proposition about a place,
and `semantic_vocabulary::Proposition`'s structural variants are not used.
*/

use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, InvocationSite, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PsiSemanticId, ScalarTerm, ScalarType, StructuralCaseId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::bindings::{PlaceOrigin, PlaceRow, Segment};
use terminal_check_tables::edges::Actual;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, IdTable, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{
    Block, Operation, OperationKind, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    Terminator, ValueDeclaration,
};

use crate::graph::Graph;
use crate::{StorageUse, refusal};

mod loans;
mod log;

pub use loans::{ConflictingLoan, LoanConflict};
use log::{Logged, Stored};

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

/// A place as its root and the segments from the root down to it. A place
/// the table has not interned (a leaf beneath a place used whole) has a
/// lineage too, which is why the walk works on lineages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lineage {
    root: PlaceId,
    segments: Vec<Segment>,
}

pub(crate) fn lineage(
    machine: MachineId,
    places: &IdTable<ExactPlaceId, PlaceRow>,
    place: ExactPlaceId,
) -> Checked<Lineage> {
    let mut segments = Vec::new();
    let mut at = place;
    for _ in 0..places.len() {
        let row = terminal_check_tables::row(places, at, machine, Construct::Place(at))?;
        match row.origin {
            PlaceOrigin::Root(root) => {
                segments.reverse();
                return Ok(Lineage { root, segments });
            }
            PlaceOrigin::Projection { parent, segment } => {
                segments.push(segment);
                at = parent;
            }
        }
    }
    Err(refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Place(place),
        "a structural place lineage does not reach an owning root",
    ))
}

/// Whether two places may share storage: the one overlap relation.
///
/// A place overlaps itself, every ancestor and every descendant. Two places
/// that part ways below a common ancestor are disjoint only when the two
/// segments they part by are known to select different storage, and the one
/// such pair is two different record fields. Every other pair of sibling
/// segments overlaps: two cases of one sum overlay ("Variant payloads
/// overlay, so different case identities do not prove disjoint storage",
/// structural_access.md), and indexed siblings stay overlapping until the
/// lane that reasons about selector values refines them here. Places under
/// different roots are different storage. Later lanes extend this function
/// (runtime indices, referents); they do not add a second relation.
///
/// It is stated over lineages, not table rows, because the walk also asks it
/// about leaves the place table has not interned.
pub(crate) fn overlap(left: &Lineage, right: &Lineage) -> bool {
    if left.root != right.root {
        return false;
    }
    let parting = left
        .segments
        .iter()
        .zip(&right.segments)
        .find(|(left, right)| left != right);
    match parting {
        // One path is a prefix of the other: the same place, or ancestor and
        // descendant.
        None => true,
        Some((Segment::Field(_), Segment::Field(_)))
        | Some((Segment::FixedIndex(_), Segment::FixedIndex(_))) => false,
        Some((
            Segment::Field(_)
            | Segment::Case(_)
            | Segment::FixedIndex(_)
            | Segment::RuntimeIndex { .. }
            | Segment::Extent,
            _,
        )) => true,
    }
}

/// Whether a field stores a shared byte view, the one view a record holds:
/// structural_access.md, "`StructuralByteViewFieldStore` replaces the
/// descriptor of one shared borrowed-view field"; "Qualified mutable,
/// write-only, and access-unspecified view fields remain unsupported". A
/// predicate-bearing view owes proofs this route does not state yet, so it
/// is not one of these.
///
/// A shared view whose field type carries primitive byte predicates is one
/// too, when the empty view, its zero, satisfies them: a record holding it is
/// then zero-constructible like any other, and each store into the field is
/// judged against the predicates (`Storage::byte_predicate_stores`). One the
/// empty view does not satisfy would make its owner gated, which this route
/// does not judge.
fn is_stored_view(field_type: &StructuralFieldType) -> bool {
    let shared = |carrier: &terminal_psi::ByteSequenceCarrier| {
        matches!(
            carrier,
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }
        )
    };
    match field_type {
        StructuralFieldType::ByteSequence(carrier) => shared(carrier),
        StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => shared(carrier) && predicates.holds_for(&[]),
        _ => false,
    }
}

// A shared view is one established descriptor. Its backing is supplied by
// construction or borrowing; checking a whole value does not project elements
// or invent a fixed extent. Mutable views still need their separate custody.
fn is_shared_view(shape: &StructuralTypeShape) -> bool {
    shape.is_shared_readable_byte_view()
        || matches!(
            shape,
            StructuralTypeShape::ElementView {
                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                ..
            }
        )
}

fn asked_root(asked: Asked<'_>) -> PlaceId {
    asked.place().root
}

/// The initial content value of each place that has one, indexed by
/// `ExactPlaceId - 1`. A derived column over the value table: elaboration
/// mints at most one such value per place.
///
/// The lowered form declares no initial content value, so nothing in it fixes
/// which identity belongs to which leaf. They are minted in the order of
/// the leaves they belong to, and any other order is refused: otherwise two
/// rows of the same type could exchange their leaves and be read as each
/// other's contents. A second value for one place is out of order too: it
/// would be a second identity for one content.
fn initial_contents(machine: MachineId, tables: &MachineTables) -> Checked<Vec<Option<ValueId>>> {
    let mut contents = vec![None; tables.places.len()];
    let mut previous: Option<ExactPlaceId> = None;
    for (value, row) in tables.values.iter() {
        let ValueOrigin::InitialContent { place } = row.origin else {
            continue;
        };
        if previous.is_some_and(|previous| previous >= place) {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Value(value),
                "initial content values are not in the order of the leaves they belong to",
            ));
        }
        previous = Some(place);
        let slot = contents.get_mut(index(place.get())).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Value(value),
                "entry content of a place past the end of this machine",
            )
        })?;
        *slot = Some(value);
    }
    Ok(contents)
}

/// Only invocation formals have invocation-wide initial contents. A call
/// result's fresh contents cannot become a block predicate's entry premise.
/// An unavailable owner remains outside scope; storage validation independently
/// refuses malformed owner tables.
pub(crate) fn input_content(
    lowered: &TerminalMachine,
    tables: &MachineTables,
    place: ExactPlaceId,
) -> bool {
    lineage(lowered.id, &tables.places, place)
        .ok()
        .and_then(|source| {
            lowered
                .structural_places
                .get(index(source.root.get()))
                .filter(|root| root.id == source.root)
        })
        .is_some_and(|root| matches!(root.kind, StructuralPlaceKind::Parameter { .. }))
}

/// What one scalar leaf holds on every path to a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    /// The one value every path supplies.
    Value(ValueId),
    /// Asked of a sum: the one case every path establishes it as.
    Case(StructuralCaseId),
    /// Every path supplies what a call returned: contents with no value
    /// identity that arrived as part of a whole value its callee
    /// established.
    Returned,
    /// Every path supplies contents, but not one same named value or case.
    Unknown,
    /// Some path supplies nothing: no store reaches the use along it and the
    /// place held nothing at entry.
    Unestablished,
    /// Asked of a window: some path arrives from the write that opened it
    /// with nothing since that closed it.
    Open,
    /// Asked of a window: every path closed it, or never passed its opening
    /// write.
    Closed,
}

/// What a place is, by the lowered form's own declarations.
enum Shape<'a> {
    Scalar(ScalarType),
    Typed(StructuralTypeId),
    /// A shared byte view stored in a field: the descriptor is the field's
    /// own contents, with no structural type of its own (structural_access.md,
    /// `StructuralByteViewFieldStore`). It is one leaf, as a view held whole
    /// is (`leaves_of`), and nothing lies beneath it.
    View,
    /// The payload namespace of one case of a sum: a `Case` segment, which
    /// holds no storage of its own and is followed by a payload field.
    Payload(&'a StructuralCaseDeclaration),
}

/// What the walk is asked about a place.
#[derive(Clone, Copy)]
enum Asked<'q> {
    /// What this scalar leaf, or this sum's discriminator, holds.
    Contents(&'q Lineage),
    /// Which case this sum is established as.
    Case(&'q Lineage),
    /// Whether the invariant window that the operation `by` opened on
    /// `place` is still open: dependent_values.md, "Otherwise the write
    /// opens a window on the place ... The domain must be proved again at
    /// the next consumption point". `restored` are the stores that proved
    /// the window's clause of that place where they wrote
    /// (data_and_literals.md: "Construction and every restoring write prove
    /// the owning default-domain clauses.").
    Window {
        place: &'q Lineage,
        by: OperationId,
        restored: &'q [OperationId],
    },
}

impl<'q> Asked<'q> {
    fn place(self) -> &'q Lineage {
        match self {
            Self::Contents(place) | Self::Case(place) | Self::Window { place, .. } => place,
        }
    }

    /// Whether a logged row ends a path of the walk. For contents, any
    /// write to an overlapping place. For a case, a write to the sum or to
    /// a place above it (structural_access.md: "A write to the sum or any
    /// prefix retires these facts"); a store beneath the sum changes a
    /// payload and not the case. For a window, the write that opened it, a
    /// write that replaces the place whole (to it or to a place above it:
    /// the new contents arrived as an established value) and a restoring
    /// store; any other store beneath the place only changes the contents
    /// of a window that stays open.
    fn ended_by(self, row: &Logged) -> bool {
        match self {
            Self::Contents(leaf) => overlap(&row.lineage, leaf),
            Self::Case(sum) => {
                row.lineage.root == sum.root && sum.segments.starts_with(&row.lineage.segments)
            }
            Self::Window {
                place,
                by,
                restored,
            } => {
                overlap(&row.lineage, place)
                    && (place.segments.starts_with(&row.lineage.segments)
                        || row.operation == by
                        || restored.contains(&row.operation))
            }
        }
    }
}

/// One clause of a data type's `requires` on one place of that type: what a
/// `DataDomain` obligation is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
    place: Lineage,
    structural_type: StructuralTypeId,
    /// The clause's position among its type's clauses.
    position: usize,
}

/// One invariant window: a `DataDomain` clause that the store or zero
/// establishment owing it was not shown to keep. The obligation is the
/// window's identity. dependent_values.md, "Invariant windows": "An
/// invariant window is compiler-derived proof debt, not a source `relax`
/// mode, runtime poison flag, or mutable truth bit."
#[derive(Debug, Clone)]
pub(crate) struct Window {
    pub(crate) obligation: ObligationId,
    /// The store or zero establishment that opened it.
    pub(crate) opened_by: OperationId,
    pub(crate) owner: Owner,
    /// Opened by a zero establishment: the storage was zeroed and no valid
    /// value exists yet (dependent_values.md: "initial target-establishment
    /// debt before a valid target value exists"). Otherwise a store broke
    /// the domain of a value that was established.
    pub(crate) initial: bool,
    /// The stores that proved this clause of this place where they wrote.
    pub(crate) restored: Vec<OperationId>,
}

/// One consumption point that an open window reaches, with the window's
/// clause stated over what its place holds there.
pub(crate) struct Consumption {
    /// The window, by position in the list `consumptions` was given.
    pub(crate) window: usize,
    /// The operation, or for a terminator its edge or block.
    pub(crate) at: Construct,
    /// The point the clause is proved at: before the operation, or on the
    /// edge.
    pub(crate) point: ProgramPoint,
    pub(crate) goal: Checked<Proposition>,
}

/// How an operation or terminator touches the storage it names.
#[derive(Clone, Copy)]
enum Touching<'a> {
    /// It passes a whole value on: a whole copy, a record operand, a record
    /// stored into a field, a call's record argument, a return, a transfer
    /// on an edge.
    Whole,
    /// It reads one scalar leaf.
    Leaf,
    /// It tests which case a sum holds.
    Membership,
    /// It dispatches on a sum; the roster is judged with it.
    Dispatch(&'a [terminal_psi::StructuralCaseSuccessorEdge]),
}

/// One operation or terminator that reads storage or names a case payload.
struct Touch<'a> {
    /// The operation, or for a terminator its edge or block.
    at: Construct,
    block: BlockId,
    /// The point before it, as `Graph::locate` orders points.
    up_to: usize,
    /// The places it reads, whole or by one leaf.
    used: Vec<Lineage>,
    /// Those and the place it stores into.
    named: Vec<Lineage>,
    /// The scalar leaves whose clauses it takes as premises: a read's leaf
    /// (`Storage::read_premises`) or a dispatch's bound payload leaves
    /// (`Storage::payload_premises`). Each relies on the domain of every
    /// place it lies in.
    relied: Vec<Lineage>,
    how: Touching<'a>,
    /// Whether it is a use of storage the initialization and payload
    /// judgments see. A call or exit that names no storage is not one.
    judged: bool,
    /// A call, or a terminator that leaves the machine: it may observe, or
    /// it ends the scope of, every place of this machine, so it consumes
    /// every window open there (`Storage::consumes`).
    releases: bool,
}

/// One payload parameter a dispatch binds (`Storage::bound_payloads`).
struct BoundPayload<'a> {
    edge: EdgeId,
    /// The arm's block, which declares the parameter.
    target: BlockId,
    /// The payload leaf of the subject it is bound from.
    leaf: Lineage,
    parameter: &'a ValueDeclaration,
}

/// One machine's storage as the checker reads it: the stores its operations
/// make, located in the graph, the entry contents, and the lowered
/// declarations that say what lies beneath a place. Derived once per
/// machine; the tables do not change beneath it, because discharge only
/// appends knowledge.
pub(crate) struct Storage<'a> {
    machine: MachineId,
    lowered: &'a TerminalMachine,
    module: &'a terminal_psi::TerminalModule,
    types: &'a [StructuralTypeDeclaration],
    graph: &'a Graph,
    initial_contents: Vec<Option<ValueId>>,
    log: Vec<Logged>,
    /// The `requires` clauses of each data type, by its identity
    /// (`Elaboration::domains`).
    domains: &'a IdTable<StructuralTypeId, Vec<Proposition>>,
}

impl<'a> Storage<'a> {
    pub(crate) fn new(
        machine: MachineId,
        lowered: &'a TerminalMachine,
        module: &'a terminal_psi::TerminalModule,
        domains: &'a IdTable<StructuralTypeId, Vec<Proposition>>,
        graph: &'a Graph,
        tables: &MachineTables,
        value_types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Self> {
        // First, because every lineage below is read through the place
        // table's parent links.
        log::places_are_interned(machine, tables)?;
        let mut storage = Self {
            machine,
            lowered,
            module,
            types: &module.structural_types,
            graph,
            initial_contents: initial_contents(machine, tables)?,
            log: Vec::new(),
            domains,
        };
        storage.initial_contents_are_declared(tables)?;
        storage.log = storage.stores(value_types)?;
        storage.log_is_stated(tables)?;
        Ok(storage)
    }

    /// Whether `operation` writes into storage that already holds a value: a
    /// scalar field store or a replacement of a field. A construction and a
    /// zero establishment make a value; they do not write into one.
    pub(crate) fn store(&self, operation: OperationId) -> Checked<bool> {
        Ok(matches!(
            self.operation(operation, Construct::Operation(operation))?
                .kind,
            OperationKind::StructuralScalarFieldStore { .. }
                | OperationKind::WriteOnlyPrimitiveStore { .. }
                | OperationKind::StoreStructuralField { .. }
                | OperationKind::StructuralByteViewFieldStore { .. }
        ))
    }

    pub(crate) fn zero_establishment(&self, operation: OperationId) -> Checked<bool> {
        Ok(matches!(
            self.operation(operation, Construct::Operation(operation))?
                .kind,
            OperationKind::EstablishZeroedStructural {}
        ))
    }

    /// The `requires` clauses of a structural type. The primitive type of a
    /// scalar local's home is declared after the data types and has none.
    fn clauses_of(
        &self,
        structural_type: StructuralTypeId,
        concerns: Construct,
    ) -> Checked<&'a [Proposition]> {
        match self.domains.get(structural_type) {
            Some(clauses) => Ok(clauses),
            None => match self.declaration(structural_type, concerns)?.shape {
                StructuralTypeShape::PrimitiveScalar(_) => Ok(&[]),
                _ => Err(self.foreign(concerns, "a data type has no row of requires clauses")),
            },
        }
    }

    /// The first declared case of the sum a zero value holds at `beneath`
    /// its root, when `beneath` is a path of the zero value: every case it
    /// passes through is the first declared case of its sum.
    /// data_and_literals.md: "The ordinary zero representation of a sum
    /// selects the first declared case and a recursively zeroed payload."
    fn zero_case(
        &self,
        root: PlaceId,
        beneath: &[Segment],
        concerns: Construct,
    ) -> Checked<Option<StructuralCaseId>> {
        let first = |storage: &Self, length: usize| -> Checked<Option<StructuralCaseId>> {
            let prefix = Lineage {
                root,
                segments: beneath[..length].to_vec(),
            };
            let Shape::Typed(sum) = storage.shape(&prefix, concerns)? else {
                return Ok(None);
            };
            Ok(match &storage.declaration(sum, concerns)?.shape {
                StructuralTypeShape::Sum { cases } => cases.first().map(|case| case.id),
                _ => None,
            })
        };
        for (length, segment) in beneath.iter().enumerate() {
            if let Segment::Case(case) = segment
                && first(self, length)? != Some(*case)
            {
                return Ok(None);
            }
        }
        first(self, beneath.len())
    }

    /// The data-typed places of the zero value of `structural_type` at
    /// `at`, parent first, fields in declaration order; a sum contributes
    /// its first case's payload.
    fn zero_typed(
        &self,
        structural_type: StructuralTypeId,
        at: &mut Lineage,
        found: &mut Vec<(Lineage, StructuralTypeId)>,
        concerns: Construct,
    ) -> Checked<()> {
        let fields = match &self.declaration(structural_type, concerns)?.shape {
            StructuralTypeShape::PrimitiveScalar(_) => return Ok(()),
            shape if is_shared_view(shape) => return Ok(()),
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                for index in 0..*length {
                    at.segments.push(Segment::FixedIndex(index));
                    self.zero_typed(*element, at, found, concerns)?;
                    at.segments.pop();
                }
                return Ok(());
            }
            StructuralTypeShape::Record { fields, .. } => {
                found.push((at.clone(), structural_type));
                (None, fields)
            }
            StructuralTypeShape::Sum { cases } => {
                found.push((at.clone(), structural_type));
                let first = cases
                    .first()
                    .ok_or_else(|| self.foreign(concerns, "a sum declares no case"))?;
                (Some(first.id), &first.fields)
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain data")),
        };
        if let Some(case) = fields.0 {
            at.segments.push(Segment::Case(case));
        }
        for field in fields.1 {
            match field.field_type {
                StructuralFieldType::Scalar(_) => {}
                StructuralFieldType::Structural(nested) => {
                    at.segments.push(Segment::Field(field.id));
                    self.zero_typed(nested, at, found, concerns)?;
                    at.segments.pop();
                }
                // The zero of a stored view is the empty shared view: a
                // value of no data type, so it owes no clause.
                ref stored if is_stored_view(stored) => {}
                _ => return Err(self.unsupported(concerns, "a field type outside plain data")),
            }
        }
        if fields.0.is_some() {
            at.segments.pop();
        }
        Ok(())
    }

    /// The data-typed places whose `requires` an operation owes, in the
    /// order `ObligationKind::DataDomain` numbers their clauses.
    ///
    /// - A record or case construction forms one value: its own type.
    /// - A zero establishment forms the zero value: its type and the type
    ///   of every data-typed place of that value, parent first.
    /// - A store into a field changes every value that contains the field:
    ///   the type that declares it and each type above it on the path, from
    ///   the innermost outward. The stored value itself arrives formed: a
    ///   scalar has no clauses and a record was established where it was
    ///   constructed, so its own type owes nothing again.
    fn owed(&self, operation: &Operation) -> Checked<Vec<(Lineage, StructuralTypeId)>> {
        let concerns = Construct::Operation(operation.id);
        let produced = operation.result.as_structural();
        let carrier = match &operation.kind {
            OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishScalarCase { .. }
            | OperationKind::EstablishStructuralCase { .. } => {
                let result = produced
                    .ok_or_else(|| self.foreign(concerns, "a construction produces no place"))?;
                return Ok(vec![(
                    Lineage {
                        root: result.place,
                        segments: Vec::new(),
                    },
                    result.structural_type,
                )]);
            }
            OperationKind::EstablishZeroedStructural {} => {
                let result = produced.ok_or_else(|| {
                    self.foreign(concerns, "a zero establishment produces no place")
                })?;
                let mut found = Vec::new();
                let mut at = Lineage {
                    root: result.place,
                    segments: Vec::new(),
                };
                self.zero_typed(result.structural_type, &mut at, &mut found, concerns)?;
                return Ok(found);
            }
            OperationKind::StructuralScalarFieldStore {
                destination, path, ..
            }
            | OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field: Some(_),
                ..
            }
            | OperationKind::StoreStructuralField {
                destination,
                path,
                field: Some(_),
                ..
            } => self.stored_at(*destination, path, concerns)?,
            OperationKind::WriteOnlyPrimitiveStore {
                destination, path, ..
            }
            | OperationKind::StoreStructuralField {
                destination,
                path,
                field: None,
                ..
            } => {
                // The incoming value arrives formed. Only strict ancestors
                // of the replaced value owe their domains again.
                let mut carrier = self.stored_at(*destination, path, concerns)?;
                if carrier.segments.pop().is_none() {
                    return Ok(Vec::new());
                }
                carrier
            }
            _ => return Ok(Vec::new()),
        };
        let mut owners = Vec::new();
        for length in (0..=carrier.segments.len()).rev() {
            let owner = Lineage {
                root: carrier.root,
                segments: carrier.segments[..length].to_vec(),
            };
            match self.shape(&owner, concerns)? {
                Shape::Typed(structural_type) => owners.push((owner, structural_type)),
                // A case's payload namespace is no value of its own; the
                // sum above it is.
                Shape::Payload(_) => {}
                Shape::Scalar(_) | Shape::View => {
                    return Err(self.foreign(concerns, "a store path crosses a leaf field"));
                }
            }
        }
        Ok(owners)
    }

    /// Checks that every operation's owed clauses each have exactly one
    /// `DataDomain` row: a clause no row names would be dropped in silence.
    pub(crate) fn owed_are_minted(&self, tables: &MachineTables) -> Checked<()> {
        for block in &self.lowered.blocks {
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                let mut owed = 0;
                for (_, structural_type) in self.owed(operation)? {
                    owed += self.clauses_of(structural_type, concerns)?.len();
                }
                // ponytail: one scan of the obligation table per operation;
                // a per-operation column if machines grow.
                let mut named = vec![false; owed];
                for (_, row) in tables.obligations.iter() {
                    let ObligationKind::DataDomain {
                        operation: owner,
                        clause,
                    } = row.kind
                    else {
                        continue;
                    };
                    if owner != operation.id {
                        continue;
                    }
                    let slot = usize::try_from(clause)
                        .ok()
                        .and_then(|clause| named.get_mut(clause))
                        .ok_or_else(|| {
                            self.foreign(
                                concerns,
                                "a data requires obligation names no owed clause",
                            )
                        })?;
                    if std::mem::replace(slot, true) {
                        return Err(
                            self.foreign(concerns, "two obligations name one data requires clause")
                        );
                    }
                }
                if named.contains(&false) {
                    return Err(self.foreign(
                        concerns,
                        "an operation owes a data requires clause no obligation names",
                    ));
                }
            }
        }
        Ok(())
    }

    /// The names of this machine's own entry clause: a scalar formal is its
    /// value, and a field of a record formal is the value that field held
    /// at entry. fact_validity.md: "a valid initialized ordinary storage
    /// place at entry may supply its exact reaching value version; it need
    /// not have a store inside the current body."
    pub(crate) fn at_entry<'s>(&'s self, tables: &'s MachineTables) -> AtEntry<'s, 'a> {
        AtEntry {
            storage: self,
            tables,
        }
    }

    /// The names of a callee's entry clause at one call: a scalar formal is
    /// the scalar actual in its lane, and a field of a record formal is
    /// the one value that field of the record actual holds on every path
    /// to the call. state_contracts.md: "Entry obligations established at
    /// every invocation under exact actual-argument substitution."
    pub(crate) fn at_call<'s>(
        &'s self,
        tables: &'s MachineTables,
        actuals: &[Actual],
        callee_formals: &'s [terminal_psi::StructuralParameterDeclaration],
        site: OperationId,
        concerns: Construct,
    ) -> Checked<AtCall<'s, 'a>> {
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(site))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "a call has no point before it"))?;
        let mut places = Vec::new();
        for actual in actuals {
            if let Actual::Place(place) = actual {
                places.push(lineage(self.machine, &tables.places, *place)?);
            }
        }
        Ok(AtCall {
            storage: self,
            tables,
            scalars: crate::scalar_actuals(actuals),
            places,
            callee_formals,
            block,
            up_to: before,
            concerns,
        })
    }

    /// A normal call guarantee observes the actual returned root after its
    /// producer. Owned input names keep their before-call values; borrowed
    /// completion observations need a separate, current-storage correspondence.
    pub(crate) fn after_call<'s>(
        &'s self,
        tables: &'s MachineTables,
        actuals: &[Actual],
        formals: &'s [terminal_psi::StructuralParameterDeclaration],
        result: Option<&TerminalMachineResult>,
        site: OperationId,
        concerns: Construct,
    ) -> Checked<Returned<'s, 'a, AtCall<'s, 'a>>> {
        let inputs = self.at_call(tables, actuals, formals, site, concerns)?;
        let operation = self.operation(site, concerns)?;
        let structural = match (result, operation.result.as_structural()) {
            (Some(TerminalMachineResult::Structural(declared)), Some(actual)) => {
                if declared.structural_type != actual.structural_type {
                    return Err(self.foreign(
                        concerns,
                        "call result has another structural type than its contract",
                    ));
                }
                Some((
                    declared.place,
                    Lineage {
                        root: actual.place,
                        segments: Vec::new(),
                    },
                ))
            }
            _ => None,
        };
        let scalar = operation.result.as_scalar().map(|actual| {
            let formal = match result {
                Some(TerminalMachineResult::Scalar(declared)) => declared.id,
                _ => ValueId::new(inputs.scalars.len() as u64 + 1)
                    .expect("a scalar result follows its formals"),
            };
            (formal, actual.id)
        });
        Ok(Returned {
            inputs,
            input_formals: formals,
            storage: self,
            tables,
            structural,
            scalar,
            point: ProgramPoint::AfterOperation(site),
            concerns,
        })
    }

    /// A published guarantee must hold of the storage this exact exit returns,
    /// including all writes before the exit. Owned input names denote entry
    /// snapshots. Borrowed postconditions cannot use those stale snapshots.
    pub(crate) fn at_return<'s>(
        &'s self,
        tables: &'s MachineTables,
        exit: EdgeId,
        concerns: Construct,
    ) -> Checked<Returned<'s, 'a, AtEntry<'s, 'a>>> {
        let (block, _) = self.graph.locate(ProgramPoint::Edge(exit))?;
        let (structural, scalar) = match (&self.lowered.result, &self.block(block)?.terminator) {
            (
                TerminalMachineResult::Structural(declared),
                Terminator::ReturnStructural { edge, source, .. },
            ) if *edge == exit => {
                if self.root_type(*source, concerns)? != declared.structural_type {
                    return Err(self.foreign(
                        concerns,
                        "returned storage differs from the declared result type",
                    ));
                }
                (
                    Some((
                        declared.place,
                        Lineage {
                            root: *source,
                            segments: Vec::new(),
                        },
                    )),
                    None,
                )
            }
            (
                TerminalMachineResult::Scalar(declared),
                Terminator::Return {
                    edge,
                    value: actual,
                    ..
                },
            ) if *edge == exit => (None, Some((declared.id, *actual))),
            (TerminalMachineResult::Unit, Terminator::ReturnUnit { edge, .. }) if *edge == exit => {
                (None, None)
            }
            _ => {
                return Err(self.foreign(
                    concerns,
                    "guarantee exit does not return the declared result",
                ));
            }
        };
        Ok(Returned {
            inputs: self.at_entry(tables),
            input_formals: &self.lowered.structural_parameters,
            storage: self,
            tables,
            structural,
            scalar,
            point: ProgramPoint::Edge(exit),
            concerns,
        })
    }

    /// The place and clause one `DataDomain` obligation is about: `clause`
    /// counts through the operation's whole list (`owed`).
    pub(crate) fn owner(
        &self,
        operation: OperationId,
        clause: u32,
        concerns: Construct,
    ) -> Checked<Owner> {
        let found = self.operation(operation, concerns)?;
        let mut remaining = usize::try_from(clause).unwrap_or(usize::MAX);
        for (place, structural_type) in self.owed(found)? {
            let clauses = self.clauses_of(structural_type, concerns)?.len();
            if remaining < clauses {
                return Ok(Owner {
                    place,
                    structural_type,
                    position: remaining,
                });
            }
            remaining -= clauses;
        }
        Err(self.foreign(concerns, "an operation owes no such data requires clause"))
    }

    fn clause(&self, owner: &Owner, concerns: Construct) -> Checked<&'a Proposition> {
        self.clauses_of(owner.structural_type, concerns)?
            .get(owner.position)
            .ok_or_else(|| self.foreign(concerns, "a data type has no such requires clause"))
    }

    /// A clause with `self` replaced by what its place holds at the point
    /// `up_to` of `block`: each field the one value the write log says that
    /// leaf holds there, each case test whether the place is that case
    /// there by structure. A field with no value identity there, or a case
    /// no structure decides, leaves the clause unstated: the refusal is
    /// `Unproved`.
    fn stated(
        &self,
        tables: &MachineTables,
        owner: &Owner,
        block: BlockId,
        up_to: usize,
        concerns: Construct,
    ) -> Checked<Proposition> {
        let mut bindings = Held {
            storage: self,
            tables,
            place: &owner.place,
            block,
            up_to,
            concerns,
        };
        crate::substitute(self.clause(owner, concerns)?, &mut bindings)
            .map_err(|unbound| unbound.refusal(self.machine, concerns))
    }

    /// The goal of one `DataDomain` obligation: the clause with `self`
    /// replaced by what the place holds once the operation completes.
    ///
    /// Each field the clause names becomes the one value the stores say
    /// that leaf holds at the operation's normal continuation: the operand
    /// of a construction, the stored value for the written field, and for
    /// every other field the value that reached the store. Each case test
    /// becomes whether the place is that case there, which a construction
    /// or zero establishment decides by structure. The goal names values
    /// only, and it is proved from the knowledge available before the
    /// operation: structural_access.md, "The obligation binds the
    /// initializer or replacement SSA value, never the old field contents."
    pub(crate) fn domain_goal(
        &self,
        tables: &MachineTables,
        operation: OperationId,
        clause: u32,
        concerns: Construct,
    ) -> Checked<Proposition> {
        let owner = self.owner(operation, clause, concerns)?;
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
        self.stated(tables, &owner, block, after, concerns)
    }

    /// Whether a store writes no storage that a clause reads, so that the
    /// clause is as true after the store as it was before it.
    /// dependent_values.md: "A write known to preserve the default domain
    /// keeps it established." The places a clause reads are the fields and
    /// the sums it names beneath its own value, found by the one
    /// substitution; a clause it cannot follow is not known to be kept.
    pub(crate) fn untouched(
        &self,
        operation: OperationId,
        owner: &Owner,
        concerns: Construct,
    ) -> Checked<bool> {
        let Some(stored) = self.operation_destination(self.operation(operation, concerns)?)? else {
            return Ok(false);
        };
        let mut names = Named {
            place: &owner.place,
            found: Vec::new(),
        };
        match crate::substitute(self.clause(owner, concerns)?, &mut names) {
            // A case test is recorded and left unstated, so that every part
            // of its connective is still followed.
            Ok(_) | Err(crate::Unbound::Unproved(_)) => {}
            Err(crate::Unbound::Unsupported(_)) => return Ok(false),
            Err(crate::Unbound::Refused(refused)) => return Err(refused),
        }
        Ok(names.found.iter().all(|named| !overlap(named, &stored)))
    }

    fn unsupported(&self, concerns: Construct, detail: &str) -> terminal_check_tables::Refusal {
        refusal(RefusalReason::Unsupported, self.machine, concerns, detail)
    }

    fn foreign(&self, concerns: Construct, detail: &str) -> terminal_check_tables::Refusal {
        refusal(
            RefusalReason::ForeignIdentity,
            self.machine,
            concerns,
            detail,
        )
    }

    fn declaration(
        &self,
        id: StructuralTypeId,
        concerns: Construct,
    ) -> Checked<&'a StructuralTypeDeclaration> {
        self.types
            .get(index(id.get()))
            .filter(|declaration| declaration.id == id)
            .ok_or_else(|| self.foreign(concerns, "a structural type is not at its position"))
    }

    fn root_kind(&self, root: PlaceId, concerns: Construct) -> Checked<StructuralPlaceKind> {
        self.lowered
            .structural_places
            .get(index(root.get()))
            .filter(|declaration| declaration.id == root)
            .map(|declaration| declaration.kind)
            .ok_or_else(|| self.foreign(concerns, "a root place is not declared at its position"))
    }

    fn root_type(&self, root: PlaceId, concerns: Construct) -> Checked<StructuralTypeId> {
        match self.root_kind(root, concerns)? {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            }
            | StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } => Ok(structural_type),
            StructuralPlaceKind::Parameter { position } => self
                .lowered
                .structural_parameters
                .iter()
                .find(|parameter| parameter.position == position && parameter.place == root)
                .map(|parameter| parameter.structural_type)
                .ok_or_else(|| self.foreign(concerns, "a formal's place has no declaration")),
            StructuralPlaceKind::Result => match &self.lowered.result {
                TerminalMachineResult::Structural(result) if result.place == root => {
                    Ok(result.structural_type)
                }
                _ => Err(self.foreign(concerns, "a result place on a machine with none")),
            },
            StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::ProviderAttachment { .. }
            | StructuralPlaceKind::TrivialAffineLocal { .. } => {
                Err(self.unsupported(concerns, "a root place kind outside this route"))
            }
        }
    }

    fn field_shape(
        &self,
        fields: &'a [StructuralFieldDeclaration],
        field: semantic_vocabulary::StructuralFieldId,
        concerns: Construct,
    ) -> Checked<Shape<'a>> {
        let declared = fields
            .iter()
            .find(|declared| declared.id == field)
            .ok_or_else(|| self.foreign(concerns, "a field is not of the type it projects"))?;
        match declared.field_type {
            StructuralFieldType::Scalar(scalar_type) => Ok(Shape::Scalar(scalar_type)),
            StructuralFieldType::Structural(nested) => Ok(Shape::Typed(nested)),
            ref stored if is_stored_view(stored) => Ok(Shape::View),
            _ => Err(self.unsupported(concerns, "a field type outside plain data")),
        }
    }

    /// What the place at `lineage` is: resolved segment by segment against
    /// the type the previous segment reached. A `Case` segment must name a
    /// declared case of the exact sum it is applied to.
    fn shape(&self, place: &Lineage, concerns: Construct) -> Checked<Shape<'a>> {
        // A root of a primitive type (a scalar home, a borrowed scalar
        // formal) is itself a scalar leaf.
        let root = self.root_type(place.root, concerns)?;
        let mut at = match &self.declaration(root, concerns)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => Shape::Scalar(*scalar_type),
            _ => Shape::Typed(root),
        };
        for segment in &place.segments {
            at = match (at, segment) {
                (Shape::Scalar(_), _) => {
                    return Err(self.foreign(concerns, "a place lies beneath a scalar field"));
                }
                (Shape::View, Segment::Extent) => Shape::Scalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .expect("u64 is valid"),
                )),
                (Shape::View, _) => {
                    return Err(self.foreign(concerns, "a place lies beneath a stored view"));
                }
                (Shape::Typed(owner), Segment::Extent) => {
                    if !self
                        .declaration(owner, concerns)?
                        .shape
                        .is_borrowed_byte_view()
                    {
                        return Err(self.foreign(concerns, "an extent is not of a byte descriptor"));
                    }
                    Shape::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64 is valid"),
                    ))
                }
                (Shape::Payload(_), Segment::Extent) => {
                    return Err(self.foreign(concerns, "an extent is applied to a case namespace"));
                }
                (Shape::Typed(owner), Segment::Field(field)) => {
                    let StructuralTypeShape::Record { fields, .. } =
                        &self.declaration(owner, concerns)?.shape
                    else {
                        return Err(
                            self.unsupported(concerns, "a field of storage that is no record")
                        );
                    };
                    self.field_shape(fields, *field, concerns)?
                }
                (Shape::Typed(owner), Segment::Case(case)) => {
                    let StructuralTypeShape::Sum { cases } =
                        &self.declaration(owner, concerns)?.shape
                    else {
                        return Err(self.unsupported(concerns, "a case of storage that is no sum"));
                    };
                    Shape::Payload(
                        cases
                            .iter()
                            .find(|declared| declared.id == *case)
                            .ok_or_else(|| {
                                self.foreign(concerns, "a case is not of the sum it is applied to")
                            })?,
                    )
                }
                (Shape::Payload(case), Segment::Field(field)) => {
                    self.field_shape(&case.fields, *field, concerns)?
                }
                (Shape::Payload(_), Segment::Case(_)) => {
                    return Err(self.foreign(concerns, "a case beneath a case"));
                }
                (Shape::Typed(owner), Segment::FixedIndex(_) | Segment::RuntimeIndex { .. }) => {
                    let StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } = self.declaration(owner, concerns)?.shape
                    else {
                        return Err(self
                            .foreign(concerns, "an index is not applied to a closed fixed array"));
                    };
                    if matches!(segment, Segment::FixedIndex(index) if *index >= length) {
                        return Err(
                            self.foreign(concerns, "a literal index exceeds its array extent")
                        );
                    }
                    match self.declaration(element, concerns)?.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar) => Shape::Scalar(scalar),
                        _ => Shape::Typed(element),
                    }
                }
                (Shape::Payload(_), Segment::FixedIndex(_) | Segment::RuntimeIndex { .. }) => {
                    return Err(self.foreign(concerns, "an index is applied to a case namespace"));
                }
            };
        }
        Ok(at)
    }

    /// Every leaf at or beneath `place`, as lineages: each scalar, and each
    /// sum itself, whose discriminator is contents like any other.
    fn leaves(&self, place: &Lineage, concerns: Construct) -> Checked<Vec<Lineage>> {
        let mut found = Vec::new();
        let mut at = place.clone();
        match self.shape(place, concerns)? {
            Shape::Scalar(_) | Shape::View => found.push(at),
            Shape::Typed(structural_type) => {
                self.leaves_of(structural_type, &mut at, &mut found, concerns)?;
            }
            Shape::Payload(case) => {
                self.field_leaves(&case.fields, &mut at, &mut found, concerns)?
            }
        }
        Ok(found)
    }

    fn field_leaves(
        &self,
        fields: &[StructuralFieldDeclaration],
        at: &mut Lineage,
        found: &mut Vec<Lineage>,
        concerns: Construct,
    ) -> Checked<()> {
        for field in fields {
            at.segments.push(Segment::Field(field.id));
            match field.field_type {
                StructuralFieldType::Scalar(_) => found.push(at.clone()),
                StructuralFieldType::Structural(nested) => {
                    self.leaves_of(nested, at, found, concerns)?;
                }
                ref stored if is_stored_view(stored) => found.push(at.clone()),
                _ => return Err(self.unsupported(concerns, "a field type outside plain data")),
            }
            at.segments.pop();
        }
        Ok(())
    }

    fn leaves_of(
        &self,
        structural_type: StructuralTypeId,
        at: &mut Lineage,
        found: &mut Vec<Lineage>,
        concerns: Construct,
    ) -> Checked<()> {
        match &self.declaration(structural_type, concerns)?.shape {
            StructuralTypeShape::PrimitiveScalar(_) => found.push(at.clone()),
            shape if is_shared_view(shape) => {
                found.push(at.clone());
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                for index in 0..*length {
                    at.segments.push(Segment::FixedIndex(index));
                    self.leaves_of(*element, at, found, concerns)?;
                    at.segments.pop();
                }
            }
            StructuralTypeShape::Record { fields, .. } => {
                self.field_leaves(fields, at, found, concerns)?;
            }
            // A sum is established when something was stored into it whole,
            // whatever its case: the sum's own place stands for its
            // discriminator, and the payload leaves of every case lie under
            // it. A sum with no payload has only that one leaf.
            StructuralTypeShape::Sum { cases } => {
                found.push(at.clone());
                for case in cases {
                    at.segments.push(Segment::Case(case.id));
                    self.field_leaves(&case.fields, at, found, concerns)?;
                    at.segments.pop();
                }
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain data")),
        }
        Ok(())
    }

    /// The place a Terminal Psi storage path names beneath a root. Each
    /// field segment is resolved by the lowered form itself against the type
    /// the previous segment reached (`runtime_structural_field`); this
    /// module compares no identity of its own.
    fn stored_at(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        concerns: Construct,
    ) -> Checked<Lineage> {
        let mut segments = Vec::with_capacity(path.len());
        if !path.is_empty() {
            let root_type = self.root_type(root, concerns)?;
            for (position, segment) in path.iter().enumerate() {
                // A case step carries its identity; a field step is resolved
                // by the lowered form against the prefix before it, which
                // may end in the case whose payload holds the field.
                match segment {
                    StructuralPathSegment::Case(case) => {
                        segments.push(Segment::Case(*case));
                        continue;
                    }
                    StructuralPathSegment::FixedIndex(index) => {
                        segments.push(Segment::FixedIndex(*index));
                        continue;
                    }
                    StructuralPathSegment::RuntimeIndex { index, .. } => {
                        segments.push(Segment::RuntimeIndex { selector: *index });
                        continue;
                    }
                    StructuralPathSegment::Field(_) => {}
                    StructuralPathSegment::AllElements
                    | StructuralPathSegment::FixedRange { .. }
                    | StructuralPathSegment::RepresentationView { .. }
                    | StructuralPathSegment::Referent => {
                        return Err(self.unsupported(
                            concerns,
                            "storage projection outside plain records and fixed array elements",
                        ));
                    }
                }
                let field = terminal_semantics::runtime_structural_field(
                    self.types.iter(),
                    root_type,
                    &path[..=position],
                )
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "a storage path that is not a path of fields and cases",
                    )
                })?;
                segments.push(Segment::Field(field.id));
            }
        }
        let place = Lineage { root, segments };
        // Resolving the shape checks each case against the sum it is
        // applied to.
        self.shape(&place, concerns)?;
        Ok(place)
    }

    fn argument(&self, argument: &StructuralArgument, concerns: Construct) -> Checked<Lineage> {
        self.stored_at(argument.place, &argument.path, concerns)
    }

    /// The scalar leaf a read operation observes, or `None` for any other
    /// operation.
    fn read_leaf(&self, operation: &Operation) -> Checked<Option<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        match &operation.kind {
            OperationKind::ByteSequenceLength { source } => {
                let mut leaf = self.argument(source, concerns)?;
                leaf.segments.push(Segment::Extent);
                let Shape::Scalar(scalar_type) = self.shape(&leaf, concerns)? else {
                    return Err(self.foreign(concerns, "an extent has no scalar carrier"));
                };
                if operation
                    .result
                    .scalar_ref()
                    .is_none_or(|result| result.scalar_type != scalar_type)
                {
                    return Err(self.foreign(
                        concerns,
                        "a length result has another scalar carrier than its descriptor extent",
                    ));
                }
                Ok(Some(leaf))
            }
            OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            }
            | OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            } => {
                let mut segments = Vec::with_capacity(path.len() + 1);
                for segment in path {
                    match segment {
                        CanonicalStructuralPathSegment::Field(field) => {
                            segments.push(Segment::Field(*field));
                        }
                        CanonicalStructuralPathSegment::Case(case) => {
                            segments.push(Segment::Case(*case));
                        }
                        CanonicalStructuralPathSegment::FixedIndex(_)
                        | CanonicalStructuralPathSegment::Referent => {
                            return Err(
                                self.unsupported(concerns, "read through an index or a referent")
                            );
                        }
                    }
                }
                segments.push(Segment::Field(*field));
                let leaf = Lineage {
                    root: *source,
                    segments,
                };
                if !matches!(self.shape(&leaf, concerns)?, Shape::Scalar(_)) {
                    return Err(
                        self.foreign(concerns, "a scalar read of storage that is no scalar")
                    );
                }
                Ok(Some(leaf))
            }
            OperationKind::PrimitiveScalarRead { source, path } => {
                self.stored_at(*source, path, concerns).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// The interned row of a lineage, when the place table holds one.
    fn interned(
        &self,
        tables: &MachineTables,
        place: &Lineage,
        concerns: Construct,
    ) -> Checked<Option<ExactPlaceId>> {
        let mut at =
            *terminal_check_tables::row(&tables.roots, place.root, self.machine, concerns)?;
        for segment in &place.segments {
            let row = terminal_check_tables::row(&tables.places, at, self.machine, concerns)?;
            match row
                .children
                .binary_search_by_key(segment, |(segment, _)| *segment)
            {
                Ok(position) => at = row.children[position].1,
                Err(_) => return Ok(None),
            }
        }
        Ok(Some(at))
    }

    fn operation(&self, id: OperationId, concerns: Construct) -> Checked<&'a Operation> {
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(id))?;
        self.lowered
            .blocks
            .get(index(block.get()))
            .zip(after.checked_sub(1))
            .and_then(|(block, position)| block.operations.get(position))
            .filter(|operation| operation.id == id)
            .ok_or_else(|| self.foreign(concerns, "an operation is not where the graph places it"))
    }

    pub(crate) fn safety_goal(
        &self,
        site: terminal_check_tables::obligations::ExecutionSite,
        obligation: ObligationId,
        types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Proposition> {
        use terminal_check_tables::obligations::ExecutionSite;
        let concerns = Construct::Obligation(obligation);
        let projections = match site {
            ExecutionSite::Operation(id) => {
                let operation = self.operation(id, concerns)?;
                if let Some(semantics) =
                    terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, types)
                        .map_err(crate::semantics_refusal(self.machine, id))?
                    && semantics.obligation() == obligation
                {
                    return semantics
                        .canonical_goal()
                        .kernel_proposition()
                        .map_err(crate::semantics_refusal(self.machine, id));
                }
                operation.kind.structural_projections()
            }
            ExecutionSite::Block(id) => self.block(id)?.terminator.storage_uses(),
        };
        let mut found = None;
        for projection in projections {
            for (position, segment) in projection.path.iter().enumerate() {
                let StructuralPathSegment::RuntimeIndex {
                    index,
                    obligation: owned,
                } = segment
                else {
                    continue;
                };
                if *owned != obligation {
                    continue;
                }
                if found.is_some() {
                    return Err(
                        self.foreign(concerns, "two array projections own one index obligation")
                    );
                }
                let root = self.root_type(projection.root, concerns)?;
                let array = terminal_semantics::runtime_structural_path_tip(
                    self.types.iter(),
                    root,
                    &projection.path[..position],
                )
                .ok_or_else(|| {
                    self.foreign(concerns, "an indexed path lost its actual array type")
                })?;
                let StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(extent),
                    ..
                } = array.shape
                else {
                    return Err(self.unsupported(
                        concerns,
                        "index into storage without a closed array extent",
                    ));
                };
                let scalar = types.get(*index).copied().ok_or_else(|| {
                    self.foreign(concerns, "an array selector has no scalar type")
                })?;
                found = Some(
                    terminal_semantics::runtime_index_bound(*index, scalar, extent).ok_or_else(
                        || {
                            self.unsupported(
                                concerns,
                                "array selector without integer bound semantics",
                            )
                        },
                    )?,
                );
            }
        }
        found.ok_or_else(|| {
            self.foreign(
                concerns,
                "operation owns neither this scalar safety goal nor this index bound",
            )
        })
    }

    fn block(&self, id: BlockId) -> Checked<&'a Block> {
        self.lowered
            .blocks
            .get(index(id.get()))
            .filter(|block| block.id == id)
            .ok_or_else(|| self.foreign(Construct::Block(id), "a block is not at its position"))
    }

    /// What one reaching store supplies to the place asked about.
    fn supplied(
        &self,
        tables: &MachineTables,
        mut row_index: usize,
        asked: Asked<'_>,
        budget: &mut usize,
        concerns: Construct,
    ) -> Checked<Reached> {
        let mut weakened = false;
        loop {
            let row = &self.log[row_index];
            // A window asks only which store a path met last: the one that
            // opened it, or one that closed it.
            if let Asked::Window { by, .. } = asked {
                return Ok(if row.operation == by {
                    Reached::Open
                } else {
                    Reached::Closed
                });
            }
            let place = asked.place();
            // Only a store into the place or above it replaces it whole; one
            // beneath it or beside it leaves contents this route cannot name.
            let Some(beneath) = place.segments.strip_prefix(row.lineage.segments.as_slice()) else {
                if matches!(row.stored, Stored::Vacated) {
                    return Ok(Reached::Unestablished);
                }
                // A constructor first establishes its whole case, then its
                // fields at the same operation. Follow earlier stores of that
                // operation before crossing its observation point.
                let earlier =
                    self.log[..row_index]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, previous)| {
                            previous.block == row.block
                                && previous.position == row.position
                                && asked.ended_by(previous)
                        });
                if let Some((index, _)) = earlier {
                    row_index = index;
                    weakened = true;
                    continue;
                }
                let previous =
                    self.reaching(tables, asked, row.block, row.position - 1, budget, concerns)?;
                return Ok(if previous == Reached::Unestablished {
                    previous
                } else {
                    Reached::Unknown
                });
            };
            let reached = match (&row.stored, asked) {
                (Stored::Vacated, Asked::Contents(_)) => Reached::Unestablished,
                (Stored::Vacated, Asked::Case(_)) => Reached::Unknown,
                (Stored::Vacated, Asked::Window { .. }) => {
                    unreachable!("windows are handled above")
                }
                (Stored::Value(value), Asked::Contents(_)) if beneath.is_empty() => {
                    Reached::Value(*value)
                }
                (Stored::Value(_), _) => Reached::Unknown,
                (Stored::Case(case), Asked::Case(_)) if beneath.is_empty() => Reached::Case(*case),
                // The payload leaves of a construction have stores of their
                // own, which come after this one; what is left is the sum's
                // discriminator, or a payload of another case.
                (Stored::Case(_), _) => Reached::Unknown,
                // A zero establishment holds the first declared case of every
                // sum of the zero value.
                (Stored::Zeroed, Asked::Case(_)) => self
                    .zero_case(row.lineage.root, beneath, concerns)?
                    .map_or(Reached::Unknown, Reached::Case),
                (Stored::Zeroed, Asked::Contents(_)) => Reached::Unknown,
                (Stored::Established, Asked::Contents(leaf)) => {
                    let is_producer = matches!(self.root_kind(leaf.root, concerns)?,
                        StructuralPlaceKind::OperationResult { producer, .. }
                            if producer == row.operation && row.lineage.segments.is_empty());
                    if is_producer {
                        self.interned(tables, leaf, concerns)?
                            .and_then(|place| {
                                self.initial_contents.get(index(place.get())).copied()
                            })
                            .flatten()
                            .map_or(Reached::Returned, Reached::Value)
                    } else {
                        Reached::Returned
                    }
                }
                (Stored::Established, Asked::Case(_)) => Reached::Unknown,
                // A window is answered by the row alone, above.
                (Stored::Zeroed | Stored::Established, Asked::Window { .. }) => {
                    return Err(
                        self.foreign(concerns, "a window is followed into a store's contents")
                    );
                }
                (Stored::Moved(source), _) => {
                    let mut source = source.clone();
                    source.segments.extend_from_slice(beneath);
                    let asked = match asked {
                        Asked::Contents(_) => Asked::Contents(&source),
                        Asked::Case(_) => Asked::Case(&source),
                        Asked::Window { .. } => {
                            return Err(self.foreign(concerns, "a window is followed into a copy"));
                        }
                    };
                    // The source as it was just before the store.
                    self.reaching(tables, asked, row.block, row.position - 1, budget, concerns)?
                }
            };
            return Ok(if weakened && reached != Reached::Unestablished {
                Reached::Unknown
            } else {
                reached
            });
        }
    }

    /// What taking an edge from `from` into `into` establishes about the
    /// case of `sum`, when it establishes anything.
    /// structural_access.md: "A dominating membership test or dispatch
    /// supplies that fact."
    ///
    /// - An edge of a `StructuralCase` on exactly this place establishes
    ///   its case. When several of its edges enter the block, as into a
    ///   wildcard arm, no one case is established.
    /// - The true edge of a `Conditional` whose condition is the result of
    ///   a `StructuralCaseMembership` on exactly this place establishes the
    ///   tested case, when the test stands in the branching block and no
    ///   write to the sum or a place above it follows the test there.
    ///
    /// The false edge establishes nothing: "Otherwise the case holds when
    /// every other declared case of that sum is refuted" needs proof about
    /// the other cases and is not decided here.
    fn edge_selects(
        &self,
        tables: &MachineTables,
        sum: &Lineage,
        from: BlockId,
        into: BlockId,
        concerns: Construct,
    ) -> Checked<Option<Reached>> {
        match &self.block(from)?.terminator {
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                if self.stored_at(*source, path, concerns)? != *sum {
                    return Ok(None);
                }
                let mut entering = cases.iter().filter(|edge| edge.target == into);
                Ok(match (entering.next(), entering.next()) {
                    (Some(edge), None) => Some(Reached::Case(edge.case)),
                    (Some(_), Some(_)) => Some(Reached::Unknown),
                    (None, _) => None,
                })
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                if when_true.target != into || when_false.target == into {
                    return Ok(None);
                }
                let origin =
                    terminal_check_tables::row(&tables.values, *condition, self.machine, concerns)?
                        .origin;
                let ValueOrigin::Operation(test) = origin else {
                    return Ok(None);
                };
                let (tested_in, after) = self.graph.locate(ProgramPoint::AfterOperation(test))?;
                if tested_in != from {
                    return Ok(None);
                }
                let OperationKind::StructuralCaseMembership { source, path, case } =
                    &self.operation(test, concerns)?.kind
                else {
                    return Ok(None);
                };
                if self.stored_at(*source, path, concerns)? != *sum {
                    return Ok(None);
                }
                let retired = self.log.iter().any(|row| {
                    row.block == from && row.position > after && Asked::Case(sum).ended_by(row)
                });
                Ok((!retired).then_some(Reached::Case(*case)))
            }
            _ => Ok(None),
        }
    }

    /// What the place asked about holds on every path to the point `up_to`
    /// of `block`: the one reaching walk. It answers what a scalar leaf
    /// holds and which case a sum is established as; the two differ only in
    /// which rows end a path and in what an edge can establish.
    fn reaching(
        &self,
        tables: &MachineTables,
        asked: Asked<'_>,
        block: BlockId,
        up_to: usize,
        budget: &mut usize,
        concerns: Construct,
    ) -> Checked<Reached> {
        // Each traced move continues strictly before its operation, so an
        // acyclic machine ends within one step per operation; a cycle of
        // moves is refused rather than followed for ever.
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| self.unsupported(concerns, "a chain of copies does not end"))?;
        let mut merged: Option<Reached> = None;
        let mut supply = |reached: Reached| {
            merged = Some(match (merged, reached) {
                (None, reached) => reached,
                (Some(Reached::Unestablished), _) | (Some(_), Reached::Unestablished) => {
                    Reached::Unestablished
                }
                // A window open on one arrival is open at the join.
                (Some(Reached::Open), _) | (Some(_), Reached::Open) => Reached::Open,
                (Some(Reached::Closed), Reached::Closed) => Reached::Closed,
                (Some(Reached::Value(known)), Reached::Value(value)) if known == value => {
                    Reached::Value(known)
                }
                (Some(Reached::Case(known)), Reached::Case(case)) if known == case => {
                    Reached::Case(known)
                }
                (Some(Reached::Returned), Reached::Returned) => Reached::Returned,
                (
                    Some(
                        Reached::Value(_)
                        | Reached::Case(_)
                        | Reached::Returned
                        | Reached::Unknown
                        | Reached::Closed,
                    ),
                    Reached::Value(_)
                    | Reached::Case(_)
                    | Reached::Returned
                    | Reached::Unknown
                    | Reached::Closed,
                ) => Reached::Unknown,
            });
        };
        // Each block is walked whole at most once, so a cycle ends the walk.
        let mut walked = vec![false; self.graph.block_count()];
        let mut pending = vec![(block, up_to)];
        while let Some((block, up_to)) = pending.pop() {
            // The last row in this block, at or before the point, that ends
            // the path: the latest position, and in one position the later
            // log entry.
            let last = self
                .log
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.block == block && row.position <= up_to && asked.ended_by(row)
                })
                .fold(None::<(usize, &Logged)>, |best, row| match best {
                    Some(best) if best.1.position > row.1.position => Some(best),
                    Some(_) | None => Some(row),
                });
            if let Some((index, _)) = last {
                supply(self.supplied(tables, index, asked, budget, concerns)?);
                continue;
            }
            if block == self.graph.entry() {
                supply(
                    match (asked, self.root_kind(asked_root(asked), concerns)?) {
                        // A path that never passed the opening write.
                        (Asked::Window { .. }, _) => Reached::Closed,
                        // A formal is initialized storage at entry. Its content
                        // has a value identity when elaboration minted one.
                        (Asked::Contents(leaf), StructuralPlaceKind::Parameter { .. }) => self
                            .interned(tables, leaf, concerns)?
                            .and_then(|place| {
                                self.initial_contents.get(index(place.get())).copied()
                            })
                            .flatten()
                            .map_or(Reached::Unknown, Reached::Value),
                        // Which case a formal holds at entry is not known, and
                        // other storage holds nothing there; that it holds
                        // nothing is the contents question's answer to give.
                        (Asked::Case(_), _) => Reached::Unknown,
                        (
                            Asked::Contents(_),
                            StructuralPlaceKind::BlockParameter { .. }
                            | StructuralPlaceKind::Result
                            | StructuralPlaceKind::OperationResult { .. }
                            | StructuralPlaceKind::ByteSequenceLiteral { .. }
                            | StructuralPlaceKind::ProviderAttachment { .. }
                            | StructuralPlaceKind::TrivialAffineLocal { .. },
                        ) => Reached::Unestablished,
                    },
                );
            }
            let mut asked_of: Vec<BlockId> = Vec::new();
            for predecessor in self.graph.predecessors(block)? {
                // A block that enters by two edges is listed twice.
                if asked_of.contains(predecessor) {
                    continue;
                }
                asked_of.push(*predecessor);
                if let Asked::Case(sum) = asked
                    && let Some(selected) =
                        self.edge_selects(tables, sum, *predecessor, block, concerns)?
                {
                    supply(selected);
                    continue;
                }
                let slot = walked.get_mut(index(predecessor.get())).ok_or_else(|| {
                    self.foreign(
                        Construct::Block(*predecessor),
                        "an edge leaves a block past the end",
                    )
                })?;
                if !std::mem::replace(slot, true) {
                    pending.push((*predecessor, usize::MAX));
                }
            }
        }
        // No path reaches the point at all: nothing is claimed about it.
        Ok(merged.unwrap_or(Reached::Unknown))
    }

    fn budget(&self) -> usize {
        self.lowered
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .sum::<usize>()
            + 1
    }

    /// The equation a read establishes on its normal successor:
    /// `read == v` when `v` is the one value its place holds on every path
    /// to it, else none.
    pub(crate) fn read_equation(
        &self,
        tables: &MachineTables,
        operation: &Operation,
    ) -> Checked<Option<Proposition>> {
        let Some(leaf) = self.read_leaf(operation)? else {
            return Ok(None);
        };
        let concerns = Construct::Operation(operation.id);
        let result = operation
            .result
            .scalar_ref()
            .ok_or_else(|| self.foreign(concerns, "a read has no scalar result"))?;
        // Elaboration interns the place of every read it emits, so an absent
        // row is a table that does not describe the module.
        if self.interned(tables, &leaf, concerns)?.is_none() {
            return Err(self.foreign(
                concerns,
                "a read names a projection the place table does not hold",
            ));
        }
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "a read has no point before it"))?;
        let mut budget = self.budget();
        let Reached::Value(held) = self.reaching(
            tables,
            Asked::Contents(&leaf),
            block,
            before,
            &mut budget,
            concerns,
        )?
        else {
            return Ok(None);
        };
        let held_type =
            terminal_check_tables::row(&tables.values, held, self.machine, concerns)?.scalar_type;
        if held_type != result.scalar_type {
            return Err(self.unsupported(
                concerns,
                "a place holds a value of another type than its read",
            ));
        }
        Ok(Some(Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            ScalarTerm::value(held, held_type),
        )))
    }

    /// What a read learns from the type of the storage it reads: each
    /// `requires` clause of a data type the leaf lies in, stated on the
    /// read's fresh value (`Storage::premises`).
    pub(crate) fn read_premises(
        &self,
        tables: &MachineTables,
        operation: &Operation,
    ) -> Checked<Vec<Proposition>> {
        let Some(leaf) = self.read_leaf(operation)? else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Operation(operation.id);
        let result = operation
            .result
            .scalar_ref()
            .ok_or_else(|| self.foreign(concerns, "a read has no scalar result"))?;
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "a read has no point before it"))?;
        self.premises(
            tables,
            &leaf,
            (result.id, result.scalar_type),
            (block, before),
            concerns,
        )
    }

    /// What one scalar observation of `leaf` learns from the types of the
    /// storage it lies in: each `requires` clause of a data type the leaf
    /// lies in, stated on the observed value `observed`, with every name of
    /// the clause read at the point `at` (a block and a position in it, as
    /// `Graph::locate` orders points). structural_access.md: "A validated
    /// field read or selected-case payload copy instantiates that invariant
    /// on its fresh scalar snapshot; other fields and unselected cases
    /// provide no premise. The snapshot survives later writes".
    ///
    /// A clause is stated only when both hold.
    ///
    /// - What the leaf holds arrived, on every path, inside a value another
    ///   machine established: a formal's entry content, however it was
    ///   copied since, or a leaf of the root a call returned. The clause
    ///   must belong to a type inside that value, on the path from its
    ///   root to the leaf. dependent_values.md: "no
    ///   consumption sees the value outside its required domain", and a
    ///   call and a return each consume the value they pass. A leaf this
    ///   machine wrote gets no premise here: its `DataDomain` obligation
    ///   proved the clause of the written value where it was written, and
    ///   the read equation names that value, so a store that broke the
    ///   clause can never lend it to a later read.
    /// - The clause names that one leaf and no other. A clause that relates
    ///   it to a sibling cannot be stated on one read's value, and a case
    ///   the clause tests must be established at `at` by structure, on the
    ///   place the observation goes through (a copy of what arrived, or
    ///   the place it arrived in): by a construction, a dominating true
    ///   edge of a membership test, or the dispatch edge that entered the
    ///   block.
    ///
    /// A clause that cannot be stated is no premise; nothing is assumed in
    /// its place. A clause that structure alone decides adds nothing and is
    /// left out.
    fn premises(
        &self,
        tables: &MachineTables,
        leaf: &Lineage,
        observed: (ValueId, ScalarType),
        at: (BlockId, usize),
        concerns: Construct,
    ) -> Checked<Vec<Proposition>> {
        let (block, before) = at;
        let (observed, scalar_type) = observed;
        // Where the leaf's content arrived: the leaf of a formal whose
        // entry content it still is, or this same leaf when its root is
        // what a call returned. The types that lend a clause are the ones
        // on that path, inside the value that arrived; a type this machine
        // built around it (a literal holding a formal) lends nothing.
        let mut budget = self.budget();
        let arrived = match self.reaching(
            tables,
            Asked::Contents(leaf),
            block,
            before,
            &mut budget,
            concerns,
        )? {
            Reached::Returned => match self.root_kind(leaf.root, concerns)? {
                // Every path supplies what a call returned, and nothing but
                // the operation that produced the root ever stored into the
                // leaf or a place around it: the root is that call's result
                // and the leaf still lies in the value the callee
                // established.
                StructuralPlaceKind::OperationResult { producer, .. }
                    if self.log.iter().all(|row| {
                        !overlap(&row.lineage, leaf)
                            || (row.lineage.segments.is_empty() && row.operation == producer)
                    }) =>
                {
                    Some(leaf.clone())
                }
                _ => None,
            },
            Reached::Value(held) => {
                match terminal_check_tables::row(&tables.values, held, self.machine, concerns)?
                    .origin
                {
                    ValueOrigin::InitialContent { place } => {
                        Some(lineage(self.machine, &tables.places, place)?)
                    }
                    ValueOrigin::MachineParameter { .. }
                    | ValueOrigin::MachineResult
                    | ValueOrigin::BlockParameter { .. }
                    | ValueOrigin::Operation(_) => None,
                }
            }
            Reached::Case(_)
            | Reached::Unknown
            | Reached::Unestablished
            | Reached::Open
            | Reached::Closed => None,
        };
        let Some(arrived) = arrived else {
            return Ok(Vec::new());
        };
        let mut premises = Vec::new();
        for length in (0..arrived.segments.len()).rev() {
            let owner = Lineage {
                root: arrived.root,
                segments: arrived.segments[..length].to_vec(),
            };
            let Shape::Typed(structural_type) = self.shape(&owner, concerns)? else {
                continue;
            };
            // The same value of that type as the observation reaches it: the
            // prefix of `leaf` as far above it as `owner` is above `arrived`
            // (a copy keeps the path beneath the place it copies). Which
            // case it holds is asked of this place, because a dispatch or a
            // test on the copy establishes it there, and the root the value
            // arrived in may have been written since. A type above the
            // observed storage has no such place.
            let beneath = arrived.segments.len() - length;
            let selected = leaf
                .segments
                .len()
                .checked_sub(beneath)
                .map(|kept| Lineage {
                    root: leaf.root,
                    segments: leaf.segments[..kept].to_vec(),
                });
            for clause in self.clauses_of(structural_type, concerns)? {
                let mut bindings = Observed {
                    held: Held {
                        storage: self,
                        tables,
                        place: &owner,
                        block,
                        up_to: before,
                        concerns,
                    },
                    selected: selected.as_ref(),
                    leaf: &arrived,
                    result: ScalarTerm::value(observed, scalar_type),
                    scalar_type,
                };
                match crate::substitute(clause, &mut bindings) {
                    Ok(Proposition::Truth | Proposition::Falsehood) => {}
                    Ok(premise) => premises.push(premise),
                    Err(crate::Unbound::Unproved(_)) => {}
                    Err(unbound) => return Err(unbound.refusal(self.machine, concerns)),
                }
            }
        }
        Ok(premises)
    }

    /// The payload parameters a dispatch binds, one per payload field of
    /// each edge: the edge, the parameter of its arm's block and the
    /// payload leaf it is bound from. Empty for any other terminator.
    fn bound_payloads(&self, block: &Block) -> Checked<Vec<BoundPayload<'a>>> {
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = &block.terminator
        else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Block(block.id);
        let subject = self.stored_at(*source, path, concerns)?;
        let mut bound = Vec::new();
        for edge in cases {
            let target = self.block(edge.target)?;
            for (position, field) in edge.payload_fields.iter().enumerate() {
                let parameter = target.parameters.get(position).ok_or_else(|| {
                    self.foreign(
                        concerns,
                        "a payload field has no parameter in its arm's block",
                    )
                })?;
                let mut leaf = subject.clone();
                leaf.segments.push(Segment::Case(edge.case));
                leaf.segments.push(Segment::Field(*field));
                bound.push(BoundPayload {
                    edge: edge.edge,
                    target: edge.target,
                    leaf,
                    parameter,
                });
            }
        }
        Ok(bound)
    }

    /// What a dispatch binds on one of its edges: each payload parameter of
    /// the target block equal to the one value its payload leaf holds on
    /// every path to the dispatch, when there is one. The same answer a
    /// payload read would get, for the form in which a dispatch delivers a
    /// payload (control_flow.md: "Selected payload fields become scalar
    /// parameters on that case's edge before cleanup.").
    pub(crate) fn payload_equations(
        &self,
        tables: &MachineTables,
        block: &Block,
    ) -> Checked<Vec<(EdgeId, Proposition)>> {
        let concerns = Construct::Block(block.id);
        let mut equations = Vec::new();
        for bound in self.bound_payloads(block)? {
            let mut budget = self.budget();
            let Reached::Value(held) = self.reaching(
                tables,
                Asked::Contents(&bound.leaf),
                block.id,
                usize::MAX,
                &mut budget,
                concerns,
            )?
            else {
                continue;
            };
            let held_type =
                terminal_check_tables::row(&tables.values, held, self.machine, concerns)?
                    .scalar_type;
            if held_type != bound.parameter.scalar_type {
                return Err(self.unsupported(
                    concerns,
                    "a payload holds a value of another type than its parameter",
                ));
            }
            equations.push((
                bound.edge,
                Proposition::Equal(
                    ScalarTerm::value(bound.parameter.id, bound.parameter.scalar_type),
                    ScalarTerm::value(held, held_type),
                ),
            ));
        }
        Ok(equations)
    }

    /// What a dispatch's payload parameters learn from the types of the
    /// storage they are bound from: the clauses a payload read at the head
    /// of the arm's block would take (`Storage::premises`), stated on the
    /// parameter. structural_access.md: "A validated field read or
    /// selected-case payload copy instantiates that invariant on its fresh
    /// scalar snapshot". Asked at the head of the arm, a case the clause
    /// tests is the one the edge that entered it selected, and no case when
    /// several edges enter it.
    pub(crate) fn payload_premises(
        &self,
        tables: &MachineTables,
        block: &Block,
    ) -> Checked<Vec<(EdgeId, Lineage, Proposition)>> {
        let mut premises = Vec::new();
        for bound in self.bound_payloads(block)? {
            for premise in self.premises(
                tables,
                &bound.leaf,
                (bound.parameter.id, bound.parameter.scalar_type),
                (bound.target, 0),
                Construct::Edge(bound.edge),
            )? {
                premises.push((bound.edge, bound.leaf.clone(), premise));
            }
        }
        Ok(premises)
    }

    /// The places an operation reads whole or by one leaf.
    fn operation_uses(&self, operation: &Operation) -> Checked<Vec<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        if let Some(leaf) = self.read_leaf(operation)? {
            return Ok(vec![leaf]);
        }
        match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
            }
            OperationKind::MoveStructuralValue { source } => {
                Ok(vec![self.argument(source, concerns)?])
            }
            // structural_access.md: "The source must be readable and
            // established on every incoming path."
            OperationKind::StructuralCaseMembership { source, path, .. } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
            }
            kind if kind.constructor_arguments().next().is_some() => kind
                .constructor_arguments()
                .map(|argument| self.argument(argument, concerns))
                .collect(),
            OperationKind::StoreStructuralField { value, .. } => {
                Ok(vec![self.argument(value, concerns)?])
            }
            // The stored descriptor is read whole from its own root.
            OperationKind::StructuralByteViewFieldStore { source, .. } => {
                Ok(vec![self.stored_at(*source, &[], concerns)?])
            }
            OperationKind::StructuralCaseLeafCopy { .. } => {
                Err(self.unsupported(concerns, "a case leaf copy"))
            }
            // A call's record arguments; every other operation reads no
            // storage, by the lowered form's own accessor.
            kind => kind
                .structural_arguments()
                .iter()
                .map(|argument| self.argument(argument, concerns))
                .collect(),
        }
    }

    /// The place an operation stores into, when it is a store.
    fn operation_destination(&self, operation: &Operation) -> Checked<Option<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        let (destination, path, field) = match &operation.kind {
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => (destination, path, Some(*field)),
            OperationKind::StoreStructuralField {
                destination,
                path,
                field,
                ..
            }
            | OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                ..
            } => (destination, path, *field),
            OperationKind::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => (destination, path, None),
            _ => return Ok(None),
        };
        let mut stored = self.stored_at(*destination, path, concerns)?;
        if let Some(field) = field {
            stored.segments.push(Segment::Field(field));
            self.shape(&stored, concerns)?;
        }
        Ok(Some(stored))
    }

    /// The two structural judgments of one operation or terminator, in
    /// order: initialization, then payload access.
    ///
    /// Initialization: every leaf at or beneath each place in `used` is
    /// supplied on every path to the point `up_to` of `block`.
    ///
    /// Payload access: for every `Case` step on the path of a place in
    /// `named`, the sum the step is applied to is established as that case
    /// on every path to the point. A payload overlays the sum's storage, so
    /// naming one is sound only there (structural_access.md: "Each `Case`
    /// step requires source admission to establish the selected case for
    /// the same root and prefix at the read's observation epoch."). The
    /// case comes from a dispatch edge, the true edge of a membership test,
    /// or the construction that every reaching write is.
    fn judged(
        &self,
        tables: &MachineTables,
        used: &[Lineage],
        named: &[Lineage],
        block: BlockId,
        up_to: usize,
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        for place in used {
            for leaf in self.leaves(place, concerns)? {
                let mut budget = self.budget();
                if self.reaching(
                    tables,
                    Asked::Contents(&leaf),
                    block,
                    up_to,
                    &mut budget,
                    concerns,
                )? == Reached::Unestablished
                {
                    return Ok(Err(refusal(
                        RefusalReason::Unestablished,
                        self.machine,
                        concerns,
                        "storage is used on a path on which nothing was stored into it",
                    )));
                }
            }
        }
        for place in named {
            for (length, segment) in place.segments.iter().enumerate() {
                let Segment::Case(case) = segment else {
                    continue;
                };
                let sum = Lineage {
                    root: place.root,
                    segments: place.segments[..length].to_vec(),
                };
                let mut budget = self.budget();
                if self.reaching(
                    tables,
                    Asked::Case(&sum),
                    block,
                    up_to,
                    &mut budget,
                    concerns,
                )? != Reached::Case(*case)
                {
                    return Ok(Err(refusal(
                        RefusalReason::Unestablished,
                        self.machine,
                        concerns,
                        "cannot prove selected case for payload access: a path to this access does not establish the case",
                    )));
                }
            }
        }
        Ok(Ok(()))
    }

    /// Whether a dispatch lists every declared case of the sum it observes
    /// exactly once, each with scalar payload fields of that case matching
    /// its arm block's parameters. A declared case with no edge is the
    /// program's error (patterns.md: "Every dispatch must cover the admitted
    /// subject."); anything else wrong with the roster is a module that does
    /// not describe a dispatch.
    fn roster(
        &self,
        subject: &Lineage,
        edges: &[terminal_psi::StructuralCaseSuccessorEdge],
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        let Shape::Typed(sum) = self.shape(subject, concerns)? else {
            return Err(self.foreign(concerns, "a dispatch on storage that is no sum"));
        };
        let StructuralTypeShape::Sum { cases } = &self.declaration(sum, concerns)?.shape else {
            return Err(self.foreign(concerns, "a dispatch on storage that is no sum"));
        };
        let mut listed = vec![false; cases.len()];
        for edge in edges {
            let position = cases
                .iter()
                .position(|case| case.id == edge.case)
                .ok_or_else(|| self.foreign(concerns, "a dispatch lists a case of another sum"))?;
            if std::mem::replace(&mut listed[position], true) {
                return Err(self.foreign(concerns, "a dispatch lists a case twice"));
            }
            if !edge.structural_arguments.is_empty() {
                return Err(self.unsupported(concerns, "storage passed on a dispatch edge"));
            }
            // An arm that never names the subject again leaves it to its
            // case's edge, which discards it after the payload is bound
            // (control_flow.md: "Selected payload fields become scalar
            // parameters on that case's edge before cleanup."). A discard
            // runs no code and reads nothing, so the place model judges
            // nothing in it, as on any other edge. That the edge's actions
            // are the ones ownership requires, and that none runs a cleanup
            // machine, is the affine judgment's
            // (`terminal_semantics::plain_affine::verify`).
            let parameters = &self.block(edge.target)?.parameters;
            if parameters.len() != edge.payload_fields.len() {
                return Err(self.foreign(
                    concerns,
                    "a dispatch edge and its arm's block disagree on the payload",
                ));
            }
            for (position, (field, parameter)) in
                edge.payload_fields.iter().zip(parameters).enumerate()
            {
                let declared = cases[position_of(cases, edge.case)]
                    .fields
                    .iter()
                    .find(|declared| declared.id == *field);
                let scalar = declared.and_then(|declared| declared.field_type.as_scalar());
                if scalar != Some(parameter.scalar_type)
                    || edge.payload_fields[..position].contains(field)
                {
                    return Err(self.foreign(
                        concerns,
                        "a dispatch edge binds something that is no scalar payload field of its case",
                    ));
                }
            }
        }
        if listed.contains(&false) {
            return Ok(Err(refusal(
                RefusalReason::Unestablished,
                self.machine,
                concerns,
                "a dispatch lists no edge for a declared case",
            )));
        }
        Ok(Ok(()))
    }

    /// One verdict per operation that puts a view into a field whose type
    /// carries primitive byte predicates
    /// (`StructuralFieldType::PredicateByteSequence`, a source field written
    /// `&[u8] in Utf8`): a record or case literal naming the field, a whole
    /// store into it, or a view store into it.
    ///
    /// The predicates are decided by the bytes the view shows, so each is
    /// judged from the bytes its source supplies. byte_views.md: "Checking
    /// rejoins the source's declared predicates or literal contents";
    /// structural_access.md: "Primitive byte predicates replay independently
    /// from exact literal octets". A source is served when it is, whole, the
    /// home of one literal established by the one
    /// `EstablishByteSequenceLiteral` that names it: a shared view never
    /// writes its backing, so those octets are what every later read sees.
    /// Bytes some predicate does not hold of are `Unestablished`, a compile
    /// error of the program: the field's domain was never established for
    /// that value. Any other source is refused as unsupported, never judged
    /// true.
    pub(crate) fn byte_predicate_stores(&self) -> Checked<Vec<StorageUse>> {
        let mut verdicts = Vec::new();
        for block in &self.lowered.blocks {
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                for (predicates, source) in self.predicate_view_sources(operation, concerns)? {
                    let bytes = if source.path.is_empty() {
                        self.literal_bytes(source.place, concerns)?
                    } else {
                        None
                    };
                    let verdict = match bytes {
                        Some(bytes) if predicates.holds_for(bytes) => Ok(()),
                        Some(_) => Err(refusal(
                            RefusalReason::Unestablished,
                            self.machine,
                            concerns,
                            "a byte view is stored into a field of a byte domain, and the bytes \
                             it shows are not in that domain",
                        )),
                        None => Err(self.unsupported(
                            concerns,
                            "a byte view stored into a field of a byte domain from other than a \
                             literal",
                        )),
                    };
                    if let Err(refused) = verdict {
                        verdicts.push(StorageUse {
                            at: concerns,
                            verdict: Err(refused),
                        });
                        break;
                    }
                }
            }
        }
        Ok(verdicts)
    }

    /// The fields of a byte domain an operation stores a view into, each
    /// with the field's predicates and the view's source.
    fn predicate_view_sources(
        &self,
        operation: &'a Operation,
        concerns: Construct,
    ) -> Checked<Vec<(terminal_psi::ByteSequencePredicateSet, StructuralArgument)>> {
        let in_fields = |fields: &[StructuralFieldDeclaration], field| {
            fields
                .iter()
                .find(|declared| declared.id == field)
                .map(|declared| declared.field_type.byte_predicates())
                .ok_or_else(|| self.foreign(concerns, "a field is not of the type it names"))
        };
        let carrier_fields = |destination, path: &[StructuralPathSegment]| {
            let carrier = self.stored_at(destination, path, concerns)?;
            match self.shape(&carrier, concerns)? {
                Shape::Typed(owner) => match &self.declaration(owner, concerns)?.shape {
                    StructuralTypeShape::Record { fields, .. } => Ok(fields.as_slice()),
                    _ => Err(self.foreign(concerns, "a field store into no record")),
                },
                Shape::Payload(case) => Ok(case.fields.as_slice()),
                _ => Err(self.foreign(concerns, "a field store into no record")),
            }
        };
        let mut found = Vec::new();
        let mut initialized = |fields: &[StructuralFieldDeclaration],
                               initializers: &[terminal_psi::RecordFieldInitializer]|
         -> Checked<()> {
            for initializer in initializers {
                if let terminal_psi::RecordFieldValue::Structural(source) = &initializer.value {
                    let predicates = in_fields(fields, initializer.field)?;
                    if !predicates.is_empty() {
                        found.push((predicates, source.clone()));
                    }
                }
            }
            Ok(())
        };
        match &operation.kind {
            OperationKind::EstablishRecord { fields } => {
                let built = self.produced_type(operation, concerns)?;
                let StructuralTypeShape::Record {
                    fields: declared, ..
                } = &self.declaration(built, concerns)?.shape
                else {
                    return Err(self.foreign(concerns, "a record literal of no record type"));
                };
                initialized(declared, fields)?;
            }
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } => {
                let built = self.produced_type(operation, concerns)?;
                let StructuralTypeShape::Sum { cases } = &self.declaration(built, concerns)?.shape
                else {
                    return Err(self.foreign(concerns, "a case literal of no sum type"));
                };
                let case = cases
                    .iter()
                    .find(|case| case.id == *result_case)
                    .ok_or_else(|| self.foreign(concerns, "a case is not of the sum it builds"))?;
                initialized(&case.fields, fields)?;
            }
            OperationKind::StoreStructuralField {
                destination,
                path,
                field: Some(field),
                value,
                ..
            } => {
                let predicates = in_fields(carrier_fields(*destination, path)?, *field)?;
                if !predicates.is_empty() {
                    found.push((predicates, value.clone()));
                }
            }
            OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field: Some(field),
                source,
                ..
            } => {
                let predicates = in_fields(carrier_fields(*destination, path)?, *field)?;
                if !predicates.is_empty() {
                    found.push((
                        predicates,
                        StructuralArgument::whole(
                            *source,
                            terminal_psi::StructuralAccess::SharedBorrow,
                        ),
                    ));
                }
            }
            _ => {}
        }
        Ok(found)
    }

    /// The structural type an operation's result is established as.
    fn produced_type(
        &self,
        operation: &Operation,
        concerns: Construct,
    ) -> Checked<StructuralTypeId> {
        operation
            .result
            .structural()
            .map(|result| result.structural_type)
            .ok_or_else(|| self.foreign(concerns, "a literal with no structural result"))
    }

    /// The octets of the literal whose home is `place`, when `place` is a
    /// literal's home and exactly one operation establishes it.
    fn literal_bytes(&self, place: PlaceId, concerns: Construct) -> Checked<Option<&'a [u8]>> {
        if !matches!(
            self.root_kind(place, concerns)?,
            StructuralPlaceKind::ByteSequenceLiteral { .. }
        ) {
            return Ok(None);
        }
        let mut producers = self.lowered.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::EstablishByteSequenceLiteral {
                        destination, bytes, ..
                    } if *destination == place => Some(bytes.as_slice()),
                    _ => None,
                })
        });
        match (producers.next(), producers.next()) {
            (Some(bytes), None) => Ok(Some(bytes)),
            _ => Ok(None),
        }
    }

    /// Every operation and terminator that reads storage or names a case
    /// payload, in block and operation order, a block's terminator last. A
    /// refusal concerns the module (an identity that does not resolve,
    /// storage outside this route).
    fn touches(&self, tables: &MachineTables) -> Checked<Vec<Touch<'a>>> {
        let mut touches = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let used = self.operation_uses(operation)?;
                let mut named = used.clone();
                named.extend(self.operation_destination(operation)?);
                // A store reads nothing, so it is a use only when its
                // destination lies in a case payload.
                let judged = !used.is_empty()
                    || named.iter().any(|place| {
                        place
                            .segments
                            .iter()
                            .any(|segment| matches!(segment, Segment::Case(_)))
                    });
                // ponytail: one scan of the invocation table per operation;
                // an operation column if machines grow.
                let releases = tables
                    .invocations
                    .values()
                    .any(|invocation| invocation.site == InvocationSite::Operation(operation.id));
                if !judged && !releases {
                    continue;
                }
                let mut relied = Vec::new();
                let how = if let Some(leaf) = self.read_leaf(operation)? {
                    if !self.read_premises(tables, operation)?.is_empty() {
                        relied.push(leaf);
                    }
                    Touching::Leaf
                } else if matches!(
                    operation.kind,
                    OperationKind::StructuralCaseMembership { .. }
                ) {
                    Touching::Membership
                } else {
                    Touching::Whole
                };
                touches.push(Touch {
                    at: Construct::Operation(operation.id),
                    block: block.id,
                    up_to: position,
                    used,
                    named,
                    relied,
                    how,
                    judged,
                    releases,
                });
            }
            let (edge, used) = match &block.terminator {
                // A dispatch reads its subject, and its roster is judged
                // with it. It has one edge per case, so it is named by its
                // block.
                Terminator::StructuralCase {
                    source,
                    path,
                    cases,
                } => {
                    let at = Construct::Block(block.id);
                    let subject = vec![self.stored_at(*source, path, at)?];
                    let mut relied = Vec::new();
                    for (_, leaf, _) in self.payload_premises(tables, block)? {
                        if !relied.contains(&leaf) {
                            relied.push(leaf);
                        }
                    }
                    touches.push(Touch {
                        at,
                        block: block.id,
                        up_to: usize::MAX,
                        named: subject.clone(),
                        used: subject,
                        relied,
                        how: Touching::Dispatch(cases),
                        judged: true,
                        releases: false,
                    });
                    continue;
                }
                Terminator::ReturnStructural { edge, source, .. } => (
                    *edge,
                    vec![Lineage {
                        root: *source,
                        segments: Vec::new(),
                    }],
                ),
                Terminator::InvokeNoNormal {
                    edge,
                    structural_arguments,
                    ..
                } => (
                    *edge,
                    structural_arguments
                        .iter()
                        .map(|argument| self.argument(argument, Construct::Edge(*edge)))
                        .collect::<Checked<Vec<_>>>()?,
                ),
                // An external completion reads its arguments like any
                // transfer and, like a crash, consumes nothing: process
                // exit "may abandon outstanding in-domain obligations,
                // including linear obligations, without cleanup or
                // discharge claims" (process_exit.md, "Abandonment and
                // survivors"), so no window has to be closed before it.
                Terminator::Jump {
                    edge,
                    structural_arguments,
                    ..
                }
                | Terminator::ExternalCompletion {
                    edge,
                    structural_arguments,
                    ..
                } => (
                    *edge,
                    structural_arguments
                        .iter()
                        .map(|argument| self.argument(argument, Construct::Edge(*edge)))
                        .collect::<Checked<Vec<_>>>()?,
                ),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    if !when_true.structural_arguments.is_empty()
                        || !when_false.structural_arguments.is_empty()
                    {
                        return Err(self.unsupported(
                            Construct::Block(block.id),
                            "storage passed on a conditional edge",
                        ));
                    }
                    continue;
                }
                Terminator::Return { edge, .. } | Terminator::ReturnUnit { edge, .. } => {
                    (*edge, Vec::new())
                }
                // A crash may abandon an open window (dependent_values.md:
                // "A crash may abandon an open window but supplies no
                // survivor-safety theorem"): it consumes nothing.
                Terminator::Crash { .. } => continue,
            };
            // A return or a terminal transfer ends the scope of every place
            // of this machine; a jump stays inside it.
            let releases = matches!(
                block.terminator,
                Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::ReturnStructural { .. }
                    | Terminator::InvokeNoNormal { .. }
            );
            let judged = !used.is_empty();
            if !judged && !releases {
                continue;
            }
            touches.push(Touch {
                at: Construct::Edge(edge),
                block: block.id,
                up_to: usize::MAX,
                named: used.clone(),
                used,
                relied: Vec::new(),
                how: Touching::Whole,
                judged,
                releases,
            });
        }
        Ok(touches)
    }

    /// One verdict per operation and terminator that reads storage or names
    /// a case payload. A refusal returned outright concerns the module (an
    /// identity that does not resolve, storage outside this route); a use
    /// that is not established is that use's verdict.
    pub(crate) fn uses(&self, tables: &MachineTables) -> Checked<Vec<StorageUse>> {
        let mut uses = Vec::new();
        for touch in self.touches(tables)? {
            if !touch.judged {
                continue;
            }
            let judge = || {
                self.judged(
                    tables,
                    &touch.used,
                    &touch.named,
                    touch.block,
                    touch.up_to,
                    touch.at,
                )
            };
            let verdict = match (touch.how, touch.used.first()) {
                (Touching::Dispatch(edges), Some(subject)) => {
                    match self.roster(subject, edges, touch.at)? {
                        Ok(()) => judge()?,
                        Err(missing) => Err(missing),
                    }
                }
                (Touching::Dispatch(_), None) => {
                    return Err(self.foreign(touch.at, "a dispatch names no subject"));
                }
                (Touching::Whole | Touching::Leaf | Touching::Membership, _) => judge()?,
            };
            uses.push(StorageUse {
                at: touch.at,
                verdict,
            });
        }
        Ok(uses)
    }

    /// Whether the window is still open at the point before `touch`: on
    /// some path the last write that could close it is the one that opened
    /// it.
    fn open_at(
        &self,
        tables: &MachineTables,
        place: &Lineage,
        by: OperationId,
        restored: &[OperationId],
        touch: &Touch<'_>,
    ) -> Checked<bool> {
        let mut budget = self.budget();
        Ok(self.reaching(
            tables,
            Asked::Window {
                place,
                by,
                restored,
            },
            touch.block,
            touch.up_to,
            &mut budget,
            touch.at,
        )? == Reached::Open)
    }

    /// Whether `window` is open just before `operation`.
    pub(crate) fn open_before(
        &self,
        tables: &MachineTables,
        window: &Window,
        operation: OperationId,
    ) -> Checked<bool> {
        let concerns = Construct::Operation(operation);
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "an operation has no point before it"))?;
        let mut budget = self.budget();
        Ok(self.reaching(
            tables,
            Asked::Window {
                place: &window.owner.place,
                by: window.opened_by,
                restored: &window.restored,
            },
            block,
            before,
            &mut budget,
            concerns,
        )? == Reached::Open)
    }

    /// Whether `touch` is a consumption point of `window` that the window
    /// reaches open. dependent_values.md, "Invariant windows": "The domain
    /// must be proved again at the next consumption point".
    ///
    /// A window a store opened is on a value that was established, and is
    /// consumed where that value is relied on as a member of its domain:
    ///
    /// - "A read relying on the domain or a dependent coupling.": a whole
    ///   read of the place or of a place that contains it (a whole copy, a
    ///   record operand, a record stored into another place), which
    ///   supplies the value under its type; and a field read, or a
    ///   dispatch's bound payload, beneath the place that takes a clause of
    ///   its storage's type as a premise. A field read or a dispatch that
    ///   takes none, and a case test, learn nothing from the domain and
    ///   consume nothing.
    /// - "Borrow creation or a call consuming the affected domain.": a
    ///   call's record argument that is the place or contains it.
    /// - "Transfer, return, scope expiration or loan handback releasing the
    ///   relevant custody or supplying a value under that domain.": a
    ///   record return and a record passed on an edge.
    ///
    /// - "Boundary/capability-carrying calls that may observe or expose the
    ///   affected storage", and scope expiration: every call, and every
    ///   return or terminal transfer, consumes every window open there.
    ///
    /// "A call need not close an unrelated domain window when its complete
    /// access, effect and outcome contracts prove that it cannot observe or
    /// expose the broken coupling ... Unknown access/effects are treated
    /// conservatively." The tables hold no callee summary, so nothing here
    /// proves a call unrelated, and none is treated as one.
    ///
    /// A window a zero establishment opened is on storage that holds no
    /// valid value yet: "A zeroed storage representation can be accessed as
    /// an established value only after its default-domain obligations
    /// hold." Every read under its root that still observes a leaf the
    /// declaration zeroed consumes it, a field read included.
    fn consumes(
        &self,
        tables: &MachineTables,
        touch: &Touch<'_>,
        window: &Window,
    ) -> Checked<bool> {
        let place = &window.owner.place;
        let released = touch.releases && !window.initial;
        // A whole use passes the place on; a leaf it takes a clause of as a
        // premise relies on the domain of every place that leaf lies in. A
        // read, a case test or a dispatch that takes no premise relies on
        // nothing.
        let aimed = released
            || touch.used.iter().any(|used| {
                used.root == place.root
                    && (window.initial
                        || matches!(touch.how, Touching::Whole)
                            && place.segments.starts_with(&used.segments))
            })
            || touch
                .relied
                .iter()
                .any(|leaf| leaf.root == place.root && leaf.segments.starts_with(&place.segments));
        if !aimed || !self.open_at(tables, place, window.opened_by, &window.restored, touch)? {
            return Ok(false);
        }
        if !window.initial {
            return Ok(true);
        }
        for used in &touch.used {
            if used.root != place.root {
                continue;
            }
            // Only leaves of the window's own place: a sibling of a nested
            // place was zeroed by the same declaration but owes another
            // window, or none.
            for leaf in self.leaves(used, touch.at)? {
                if overlap(&leaf, place)
                    && self.open_at(tables, &leaf, window.opened_by, &[], touch)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// The point a clause is proved at for a consumption: on the edge of a
    /// terminator that has one, and otherwise just before the operation or
    /// at the end of the block.
    fn point(&self, touch: &Touch<'_>) -> Checked<ProgramPoint> {
        if let Construct::Edge(edge) = touch.at {
            return Ok(ProgramPoint::Edge(edge));
        }
        let operations = &self.block(touch.block)?.operations;
        Ok(
            match touch
                .up_to
                .min(operations.len())
                .checked_sub(1)
                .and_then(|before| operations.get(before))
            {
                Some(before) => ProgramPoint::AfterOperation(before.id),
                None => ProgramPoint::BlockEntry(touch.block),
            },
        )
    }

    /// Every consumption point each window reaches open, in block and
    /// operation order, with the window's clause stated over what its place
    /// holds there. The clause is proved again at each of them.
    pub(crate) fn consumptions(
        &self,
        tables: &MachineTables,
        windows: &[Window],
    ) -> Checked<Vec<Consumption>> {
        let mut found = Vec::new();
        if windows.is_empty() {
            return Ok(found);
        }
        for touch in self.touches(tables)? {
            for (position, window) in windows.iter().enumerate() {
                if !self.consumes(tables, &touch, window)? {
                    continue;
                }
                found.push(Consumption {
                    window: position,
                    at: touch.at,
                    point: self.point(&touch)?,
                    goal: self.stated(
                        tables,
                        &window.owner,
                        touch.block,
                        touch.up_to,
                        Construct::Obligation(window.obligation),
                    ),
                });
            }
        }
        Ok(found)
    }
}

fn position_of(cases: &[StructuralCaseDeclaration], case: StructuralCaseId) -> usize {
    cases
        .iter()
        .position(|declared| declared.id == case)
        .unwrap_or(usize::MAX)
}

/// The names of a data type's `requires` clause, standing for what the
/// place `place` of that type holds at one point: each field the one value
/// the walk finds there, each case test whether the walk finds that case.
struct Held<'s, 'a> {
    storage: &'s Storage<'a>,
    tables: &'s MachineTables,
    place: &'s Lineage,
    block: BlockId,
    up_to: usize,
    concerns: Construct,
}

/// Extends `place` by the fields and cases a clause's path names.
fn extended(
    mut place: Lineage,
    path: &[CanonicalStructuralPathSegment],
) -> Result<Lineage, crate::Unbound> {
    for segment in path {
        place.segments.push(match segment {
            CanonicalStructuralPathSegment::Field(field) => Segment::Field(*field),
            CanonicalStructuralPathSegment::Case(case) => Segment::Case(*case),
            CanonicalStructuralPathSegment::FixedIndex(_)
            | CanonicalStructuralPathSegment::Referent => {
                return Err(crate::Unbound::Unsupported(
                    "a clause through an index or a referent",
                ));
            }
        });
    }
    Ok(place)
}

impl Held<'_, '_> {
    /// The place a clause's path names beneath the value the clause is
    /// about. The clause's root stands for `self` and for nothing else.
    fn beneath(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        if Some(root) != terminal_check_tables::program::self_place() {
            return Err(crate::Unbound::Unsupported(
                "a data clause names storage other than its own value",
            ));
        }
        extended(self.place.clone(), path)
    }
}

/// Input substitution and returned storage share the existing clause binding
/// interface. Only the declared result root is redirected to the actual result;
/// every field read uses the same checked reaching-write walk as data clauses.
pub(crate) struct Returned<'s, 'a, Inputs> {
    inputs: Inputs,
    input_formals: &'s [terminal_psi::StructuralParameterDeclaration],
    storage: &'s Storage<'a>,
    tables: &'s MachineTables,
    structural: Option<(PlaceId, Lineage)>,
    scalar: Option<(ValueId, ValueId)>,
    point: ProgramPoint,
    concerns: Construct,
}

impl<Inputs: crate::Bindings<Error = crate::Unbound>> Returned<'_, '_, Inputs> {
    fn owned_inputs(&mut self, root: PlaceId) -> Result<&mut Inputs, crate::Unbound> {
        let formal =
            terminal_psi::StructuralParameterDeclaration::find_by_place(self.input_formals, root)
                .ok_or(crate::Unbound::Unsupported(
                "a completion clause names storage that is no formal",
            ))?;
        if formal.access != terminal_psi::StructuralAccess::Owned {
            return Err(crate::Unbound::Unsupported(
                "borrowed completion storage has no post-call observation",
            ));
        }
        Ok(&mut self.inputs)
    }

    fn held(&self) -> Result<Held<'_, '_>, crate::Unbound> {
        let (_, place) = self.structural.as_ref().ok_or(crate::Unbound::Unsupported(
            "no structural result correspondence",
        ))?;
        let (block, up_to) = self
            .storage
            .graph
            .locate(self.point)
            .map_err(crate::Unbound::Refused)?;
        Ok(Held {
            storage: self.storage,
            tables: self.tables,
            place,
            block,
            up_to,
            concerns: self.concerns,
        })
    }
}

impl<Inputs: crate::Bindings<Error = crate::Unbound>> crate::Bindings for Returned<'_, '_, Inputs> {
    type Error = crate::Unbound;
    fn value(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if let Some((formal, actual)) = self.scalar
            && id == formal
        {
            if self.tables.values.get(actual).map(|row| row.scalar_type) != Some(scalar_type) {
                return Err(crate::Unbound::Unsupported(
                    "returned scalar type differs from its contract",
                ));
            }
            return Ok(ScalarTerm::value(actual, scalar_type));
        }
        self.inputs.value(id, scalar_type)
    }
    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if self
            .structural
            .as_ref()
            .is_some_and(|(declared, _)| *declared == root)
        {
            let subject = terminal_check_tables::program::self_place()
                .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
            return self.held()?.field(subject, path, scalar_type);
        }
        self.owned_inputs(root)?.field(root, path, scalar_type)
    }
    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if self
            .structural
            .as_ref()
            .is_some_and(|(declared, _)| *declared == root)
        {
            let subject = terminal_check_tables::program::self_place()
                .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
            return self.held()?.extent(subject, path, scalar_type);
        }
        self.owned_inputs(root)?.extent(root, path, scalar_type)
    }
    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        if self
            .structural
            .as_ref()
            .is_some_and(|(declared, _)| *declared == root)
        {
            let subject = terminal_check_tables::program::self_place()
                .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
            return self.held()?.case(subject, path, case);
        }
        self.owned_inputs(root)?.case(root, path, case)
    }
}

/// See `Storage::at_entry`.
pub(crate) struct AtEntry<'s, 'a> {
    storage: &'s Storage<'a>,
    tables: &'s MachineTables,
}

impl AtEntry<'_, '_> {
    fn observation(
        &self,
        leaf: Lineage,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let storage = self.storage;
        let concerns = Construct::Machine;
        let kind = storage
            .root_kind(leaf.root, concerns)
            .map_err(crate::Unbound::Refused)?;
        if !matches!(kind, StructuralPlaceKind::Parameter { .. }) {
            return Err(crate::Unbound::Unsupported(
                "an entry clause names storage that is no formal",
            ));
        }
        if !matches!(
            storage
                .shape(&leaf, concerns)
                .map_err(crate::Unbound::Refused)?,
            Shape::Scalar(_)
        ) {
            return Err(crate::Unbound::Unsupported(
                "an entry clause names storage that is no scalar field",
            ));
        }
        let held = storage
            .interned(self.tables, &leaf, concerns)
            .map_err(crate::Unbound::Refused)?
            .and_then(|place| storage.initial_contents.get(index(place.get())).copied())
            .flatten()
            .ok_or(crate::Unbound::Unproved(
                "no operation names the field, so its entry content has no value identity",
            ))?;
        let held_type =
            terminal_check_tables::row(&self.tables.values, held, storage.machine, concerns)
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
        if held_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "a formal's field holds a value of another type than its clause term",
            ));
        }
        Ok(ScalarTerm::value(held, scalar_type))
    }
}

impl crate::Bindings for AtEntry<'_, '_> {
    type Error = crate::Unbound;
    fn value(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        Ok(ScalarTerm::value(id, scalar_type))
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.observation(
            extended(
                Lineage {
                    root,
                    segments: Vec::new(),
                },
                path,
            )?,
            scalar_type,
        )
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let mut leaf = extended(
            Lineage {
                root,
                segments: Vec::new(),
            },
            path,
        )?;
        leaf.segments.push(Segment::Extent);
        self.observation(leaf, ScalarType::Integer(scalar_type))
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        Err(crate::Unbound::Unsupported(
            "a case test in an entry clause",
        ))
    }
}

/// See `Storage::at_call`.
pub(crate) struct AtCall<'s, 'a> {
    storage: &'s Storage<'a>,
    tables: &'s MachineTables,
    scalars: Vec<ValueId>,
    /// The record actuals, in the callee's structural lane order.
    places: Vec<Lineage>,
    callee_formals: &'s [terminal_psi::StructuralParameterDeclaration],
    block: BlockId,
    up_to: usize,
    concerns: Construct,
}

impl AtCall<'_, '_> {
    fn actual(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        // The callee's root is one of its record formals; the actual is the
        // place at the same position of the structural lane.
        let actual = self
            .callee_formals
            .iter()
            .find(|parameter| parameter.place == root)
            .and_then(|parameter| usize::try_from(parameter.position).ok())
            .and_then(|position| self.places.get(position))
            .ok_or(crate::Unbound::Unsupported(
                "clause names storage that is no record formal of the callee",
            ))?;
        extended(actual.clone(), path)
    }

    /// Fields and descriptor metadata both name the actual's reaching
    /// content immediately before this call. An earlier observation of a
    /// replaced descriptor cannot supply the callee's requirement.
    fn observation(
        &self,
        leaf: Lineage,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let storage = self.storage;
        if !matches!(
            storage
                .shape(&leaf, self.concerns)
                .map_err(crate::Unbound::Refused)?,
            Shape::Scalar(_)
        ) {
            return Err(crate::Unbound::Unsupported(
                "clause names storage that is no scalar observation of the actual",
            ));
        }
        let mut budget = storage.budget();
        let Reached::Value(held) = storage
            .reaching(
                self.tables,
                Asked::Contents(&leaf),
                self.block,
                self.up_to,
                &mut budget,
                self.concerns,
            )
            .map_err(crate::Unbound::Refused)?
        else {
            return Err(crate::Unbound::Unproved(
                "the actual's observation has no reaching value identity at the call",
            ));
        };
        let held_type =
            terminal_check_tables::row(&self.tables.values, held, storage.machine, self.concerns)
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
        if held_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "an actual's observation holds a value of another type than its clause term",
            ));
        }
        Ok(ScalarTerm::value(held, scalar_type))
    }
}

impl crate::Bindings for AtCall<'_, '_> {
    type Error = crate::Unbound;
    fn value(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        crate::scalar_actual(&self.scalars, self.tables, id, scalar_type)
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.observation(self.actual(root, path)?, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let mut leaf = self.actual(root, path)?;
        leaf.segments.push(Segment::Extent);
        self.observation(leaf, ScalarType::Integer(scalar_type))
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        Err(crate::Unbound::Unsupported(
            "a case test in an entry clause",
        ))
    }
}

impl crate::Bindings for Held<'_, '_> {
    type Error = crate::Unbound;
    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, crate::Unbound> {
        Err(crate::Unbound::Unsupported(
            "a data clause names a value that is no field of its own value",
        ))
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let leaf = self.beneath(root, path)?;
        let storage = self.storage;
        let mut budget = storage.budget();
        let reached = storage
            .reaching(
                self.tables,
                Asked::Contents(&leaf),
                self.block,
                self.up_to,
                &mut budget,
                self.concerns,
            )
            .map_err(crate::Unbound::Refused)?;
        let Reached::Value(held) = reached else {
            return Err(crate::Unbound::Unproved(
                "a field the data requires clause names holds no value identity here",
            ));
        };
        let held_type =
            terminal_check_tables::row(&self.tables.values, held, storage.machine, self.concerns)
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
        if held_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "a field holds a value of another type than its clause term",
            ));
        }
        Ok(ScalarTerm::value(held, scalar_type))
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        let sum = self.beneath(root, path)?;
        let storage = self.storage;
        let mut budget = storage.budget();
        match storage
            .reaching(
                self.tables,
                Asked::Case(&sum),
                self.block,
                self.up_to,
                &mut budget,
                self.concerns,
            )
            .map_err(crate::Unbound::Refused)?
        {
            Reached::Case(held) => Ok(held == case),
            Reached::Value(_)
            | Reached::Returned
            | Reached::Unknown
            | Reached::Unestablished
            | Reached::Open
            | Reached::Closed => Err(crate::Unbound::Unproved(
                "the case the data requires clause tests is not established by structure here",
            )),
        }
    }
}

/// Records the places a data type's `requires` clause reads beneath the
/// value it is about, and changes nothing: a field stays the term it was.
/// A case test is recorded as its sum's place and left unstated, so that the
/// substitution follows every part of the connective around it.
struct Named<'s> {
    place: &'s Lineage,
    found: Vec<Lineage>,
}

impl Named<'_> {
    fn beneath(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(), crate::Unbound> {
        if Some(root) != terminal_check_tables::program::self_place() {
            return Err(crate::Unbound::Unsupported(
                "a data clause names storage other than its own value",
            ));
        }
        self.found.push(extended(self.place.clone(), path)?);
        Ok(())
    }
}

impl crate::Bindings for Named<'_> {
    type Error = crate::Unbound;
    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, crate::Unbound> {
        Err(crate::Unbound::Unsupported(
            "a data clause names a value that is no field of its own value",
        ))
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.beneath(root, path)?;
        match scalar_type {
            ScalarType::Integer(scalar_type) => Ok(ScalarTerm::IntegerField {
                root,
                path: path.to_vec(),
                scalar_type,
            }),
            ScalarType::Boolean => Ok(ScalarTerm::BooleanField {
                root,
                path: path.to_vec(),
            }),
            ScalarType::IeeeFloat(_) => Err(crate::Unbound::Unsupported(
                "a clause term outside the scalar subset",
            )),
        }
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        self.beneath(root, path)?;
        Err(crate::Unbound::Unproved(
            "a case test is recorded, not decided",
        ))
    }
}

/// The names of a data type's `requires` clause as one read observes them:
/// the leaf it read is its fresh result, and every other field is nothing
/// the read can speak for ("other fields and unselected cases provide no
/// premise"). A case test is what the walk establishes at the read, of the
/// place the read observes the value through (`selected`).
struct Observed<'s, 'a> {
    held: Held<'s, 'a>,
    /// The place of the clause's type on the observed path, when the type
    /// lies at or beneath the observed storage's root.
    selected: Option<&'s Lineage>,
    leaf: &'s Lineage,
    result: ScalarTerm,
    scalar_type: ScalarType,
}

impl crate::Bindings for Observed<'_, '_> {
    type Error = crate::Unbound;
    fn value(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.held.value(id, scalar_type)
    }

    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if self.held.beneath(root, path)? != *self.leaf {
            return Err(crate::Unbound::Unproved(
                "a field other than the one read provides no premise",
            ));
        }
        if scalar_type != self.scalar_type {
            return Err(crate::Unbound::Unsupported(
                "a read has another type than its clause term",
            ));
        }
        Ok(self.result.clone())
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, crate::Unbound> {
        let place = self.selected.ok_or(crate::Unbound::Unproved(
            "the clause tests a case of a value above the storage the read observes",
        ))?;
        Held { place, ..self.held }.case(root, path, case)
    }
}
