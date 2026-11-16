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
    Block, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

use crate::graph::Graph;
use crate::{StorageUse, refusal};

mod byte_payloads;
mod byte_views;
mod cleanup;
mod contents;
mod loans;
mod log;

pub(crate) use loans::reference_graph;
pub use loans::{ConflictingLoan, LoanConflict};
use log::{Logged, Stored};
use std::collections::BTreeMap;
use terminal_semantics::descriptor_results::DescriptorAnalysis;

fn index(raw: u64) -> usize {
    usize::try_from(raw - 1).unwrap_or(usize::MAX)
}

/// A place as its root and the segments from the root down to it. A place
/// the table has not interned (a leaf beneath a place used whole) has a
/// lineage too, which is why the walk works on lineages.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Lineage {
    root: PlaceId,
    segments: Vec<Segment>,
}

/// Descriptor storage and the payload it permits access to are different
/// places. Initializing or replacing a descriptor does not write payload bytes.
enum StorageEffect {
    Storage(Lineage),
    Payload(Lineage),
}

impl Lineage {
    fn descriptor_location(
        &self,
    ) -> Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation> {
        use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
        DescriptorLocation::captured(self.root, &self.segments)
    }
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
    terminal_semantics::storage::place_paths::may_overlap(&left.segments, &right.segments)
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
/// judged against the predicates (`Storage::byte_predicate_establishments`). One the
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

/// Logical values indexed by exact place at its owner's entry and by
/// (call, place) on return. The producer lists snapshots in (place, boundary) order, so
/// duplicate or exchanged identities cannot change which leaf a clause names.
struct Contents {
    entry: Vec<Option<ValueId>>,
    calls: Vec<Vec<(ExactPlaceId, ValueId)>>,
}

fn initial_contents(
    machine: MachineId,
    tables: &MachineTables,
    operations: usize,
) -> Checked<Contents> {
    let mut contents = Contents {
        entry: vec![None; tables.places.len()],
        calls: vec![Vec::new(); operations],
    };
    let mut previous = None;
    for (value, row) in tables.values.iter() {
        let ValueOrigin::StorageContent { place, after } = row.origin else {
            continue;
        };
        let key = (place, after);
        if previous.is_some_and(|previous| previous >= key) {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Value(value),
                "storage content values are not in the order of their leaves and call boundaries",
            ));
        }
        previous = Some(key);
        let entry = contents.entry.get_mut(index(place.get())).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Value(value),
                "content of a place past the end of this machine",
            )
        })?;
        if let Some(call) = after {
            let returned = contents.calls.get_mut(index(call.get())).ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    machine,
                    Construct::Value(value),
                    "storage content names a call past the end of this machine",
                )
            })?;
            returned.push((place, value));
        } else {
            *entry = Some(value);
        }
    }
    Ok(contents)
}

/// The declaration says which entry binds the snapshot. Joins apply that
/// entry's scope; storage validation independently checks the owner and leaf.
pub(crate) fn content_root_kind(
    lowered: &TerminalMachine,
    tables: &MachineTables,
    place: ExactPlaceId,
) -> Option<StructuralPlaceKind> {
    content_root(lowered, tables, place).map(|root| root.kind)
}

pub(crate) fn content_root<'a>(
    lowered: &'a TerminalMachine,
    tables: &MachineTables,
    place: ExactPlaceId,
) -> Option<&'a terminal_psi::StructuralPlaceDeclaration> {
    lineage(lowered.id, &tables.places, place)
        .ok()
        .and_then(|source| {
            lowered
                .structural_places
                .get(index(source.root.get()))
                .filter(|root| root.id == source.root)
        })
}

/// What one scalar leaf holds on every path to a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    /// No real arrival supplies this equation yet. A cycle of copies has
    /// no contents of its own.
    Cycle,
    /// The one value every path supplies.
    Value(ValueId),
    /// An extent defined by this exact reaching descriptor producer.
    Extent(OperationId),
    /// Asked of a sum: the one case every path establishes it as.
    Case(StructuralCaseId),
    /// Every path supplies part of an established value without naming one
    /// scalar identity. The depth counts ancestors the leaf still shares
    /// with that value. A transfer can retain the field's own type evidence
    /// without lending it to a record that now encloses it.
    Established(usize),
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

impl Reached {
    fn within(self, depth: usize) -> Self {
        match self {
            Self::Established(established) => Self::Established(established.min(depth)),
            reached => reached,
        }
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Cycle, supply) | (supply, Self::Cycle) => supply,
            (Self::Unestablished, _) | (_, Self::Unestablished) => Self::Unestablished,
            (Self::Open, _) | (_, Self::Open) => Self::Open,
            (Self::Established(left), Self::Established(right)) => {
                Self::Established(left.min(right))
            }
            (left, right) if left == right => left,
            _ => Self::Unknown,
        }
    }

    fn without_identity(self) -> Self {
        match self {
            Self::Cycle | Self::Unestablished => self,
            _ => Self::Unknown,
        }
    }
}

/// A read or block entry observes a value without writing or restoring it.
struct Observation {
    point: ProgramPoint,
    position: usize,
    value: ValueId,
}

/// Local to one requested byte predicate; declarations provide its dense IDs.
enum BytePredicateState {
    Unseen,
    Visiting,
    Resolved(ScalarTerm),
}

/// A storage trace must retain which block entries it crossed. A descriptor
/// saved on an earlier loop iteration cannot use today's endpoint SSA values.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QueryKind {
    Contents,
    Case,
}

/// What reaches one equation: a real store, entry, or case selection, or
/// the result of another equation before a copy or block argument transfer.
/// How many enclosing path segments were established together at a call or
/// invocation boundary on every arriving path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundaryDepth {
    Cycle,
    Established(usize),
    Local,
}

impl BoundaryDepth {
    fn within(self, limit: usize) -> Self {
        match self {
            Self::Established(depth) => Self::Established(depth.min(limit)),
            other => other,
        }
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Cycle, supply) | (supply, Self::Cycle) => supply,
            (Self::Established(left), Self::Established(right)) => {
                Self::Established(left.min(right))
            }
            (Self::Local, _) | (_, Self::Local) => Self::Local,
        }
    }
}

#[derive(Clone, Copy)]
enum Supply {
    Direct(Reached),
    Boundary {
        value: ValueId,
        depth: usize,
    },
    Transfer {
        source: usize,
        depth: usize,
        loses_identity: bool,
    },
}

impl Supply {
    fn transfer(source: usize) -> Self {
        Self::Transfer {
            source,
            depth: usize::MAX,
            loses_identity: false,
        }
    }

    fn within(self, limit: usize) -> Self {
        match self {
            Self::Direct(reached) => Self::Direct(reached.within(limit)),
            Self::Boundary { value, depth } => Self::Boundary {
                value,
                depth: depth.min(limit),
            },
            Self::Transfer {
                source,
                depth,
                loses_identity,
            } => Self::Transfer {
                source,
                depth: depth.min(limit),
                loses_identity,
            },
        }
    }

    fn without_identity(self) -> Self {
        match self {
            Self::Direct(reached) => Self::Direct(reached.without_identity()),
            Self::Boundary { .. } => Self::Direct(Reached::Unknown),
            Self::Transfer { source, depth, .. } => Self::Transfer {
                source,
                depth,
                loses_identity: true,
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Transfer {
    destination: usize,
    depth: usize,
    loses_identity: bool,
}

#[derive(Clone, Copy)]
struct EntryFallback {
    upstream: usize,
    destination: usize,
    value: ValueId,
}

struct ReachingEquation {
    seed: Reached,
    seed_boundary: BoundaryDepth,
    direct: Reached,
    boundary: BoundaryDepth,
    transfers: Vec<Transfer>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Observations {
    None,
    Boundary,
    All,
}

pub(super) struct Reaching {
    keys: BTreeMap<(BlockId, usize, Lineage, QueryKind), usize>,
    equations: Vec<ReachingEquation>,
    fallbacks: Vec<EntryFallback>,
    boundary: BoundaryDepth,
    initialization_only: bool,
    observations: Observations,
    remaining: usize,
    crossed_entries: Vec<bool>,
    used: Option<ProgramPoint>,
    #[cfg(test)]
    scanned_rows: usize,
}

impl Reaching {
    fn add_equation(&mut self) -> usize {
        let address = self.equations.len();
        self.equations.push(ReachingEquation {
            seed: Reached::Cycle,
            seed_boundary: BoundaryDepth::Cycle,
            direct: Reached::Cycle,
            boundary: BoundaryDepth::Cycle,
            transfers: Vec::new(),
        });
        address
    }

    fn add_supply(&mut self, destination: usize, supply: Supply) {
        match supply {
            Supply::Direct(reached) => {
                let boundary = match reached {
                    Reached::Cycle => BoundaryDepth::Cycle,
                    Reached::Established(depth) if !self.initialization_only => {
                        BoundaryDepth::Established(depth)
                    }
                    _ => BoundaryDepth::Local,
                };
                let equation = &mut self.equations[destination];
                equation.seed = equation.seed.merge(if self.initialization_only {
                    reached.without_identity()
                } else {
                    reached
                });
                equation.seed_boundary = equation.seed_boundary.merge(boundary);
            }
            Supply::Boundary { value, depth } => {
                let equation = &mut self.equations[destination];
                if self.initialization_only {
                    equation.seed = equation.seed.merge(Reached::Unknown);
                    equation.seed_boundary = equation.seed_boundary.merge(BoundaryDepth::Local);
                } else {
                    equation.seed = equation.seed.merge(Reached::Value(value));
                    equation.seed_boundary = equation
                        .seed_boundary
                        .merge(BoundaryDepth::Established(depth));
                }
            }
            Supply::Transfer {
                source,
                depth,
                loses_identity,
            } => {
                self.equations[source].transfers.push(Transfer {
                    destination,
                    depth,
                    loses_identity,
                });
            }
        }
    }

    fn solve(&mut self, root: usize) -> Reached {
        let mut pending = Vec::new();
        let mut queued = vec![false; self.equations.len()];
        for (address, row) in self.equations.iter_mut().enumerate() {
            row.direct = row.seed;
            row.boundary = row.seed_boundary;
            if row.direct != Reached::Cycle || row.boundary != BoundaryDepth::Cycle {
                queued[address] = true;
                pending.push(address);
            }
        }
        // Copies have no independent contents. Propagate real arrivals until
        // every equation agrees with its predecessors. A cycle with no real
        // supply remains empty, including one with projected copies.
        while let Some(source) = pending.pop() {
            queued[source] = false;
            let previous = self.equations[source].direct;
            let previous_boundary = self.equations[source].boundary;
            for position in 0..self.equations[source].transfers.len() {
                let transfer = self.equations[source].transfers[position];
                let (reached, boundary) = if transfer.loses_identity {
                    (previous.without_identity(), BoundaryDepth::Local)
                } else {
                    (
                        previous.within(transfer.depth),
                        previous_boundary.within(transfer.depth),
                    )
                };
                let destination = &mut self.equations[transfer.destination];
                let merged = destination.direct.merge(reached);
                let merged_boundary = destination.boundary.merge(boundary);
                if merged != destination.direct || merged_boundary != destination.boundary {
                    destination.direct = merged;
                    destination.boundary = merged_boundary;
                    if !std::mem::replace(&mut queued[transfer.destination], true) {
                        pending.push(transfer.destination);
                    }
                }
            }
        }
        self.equations[root].direct
    }
}

/// What a place is, by the lowered form's own declarations.
enum Shape<'a> {
    Scalar(ScalarType),
    Atomic(semantic_vocabulary::AtomicCellType),
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

    fn at<'p>(self, place: &'p Lineage) -> Asked<'p>
    where
        'q: 'p,
    {
        match self {
            Self::Contents(_) => Asked::Contents(place),
            Self::Case(_) => Asked::Case(place),
            Self::Window { by, restored, .. } => Asked::Window {
                place,
                by,
                restored,
            },
        }
    }

    /// Whether a logged row ends a path of the walk. For contents, any
    /// write to an overlapping place. For a case, any write that may
    /// overlap the sum except one beneath it: a write to the sum or to a
    /// place above it (structural_access.md: "A write to the sum or any
    /// prefix retires these facts"), and a write that may reach it through
    /// another path, such as a store to `items[j]` while the case of
    /// `items[0]` is known (a store crossing a runtime element "overlaps
    /// every element of that array"). A store beneath the sum changes a
    /// payload and not the case. Both questions use the one `overlap`
    /// relation, so a refinement of it reaches case facts too. For a window, the write that opened it, a
    /// write that replaces the place whole (to it or to a place above it:
    /// the new contents arrived as an established value) and a restoring
    /// store; any other store beneath the place only changes the contents
    /// of a window that stays open.
    fn ended_by(self, row: &Logged) -> bool {
        match self {
            Self::Contents(leaf) => overlap(&row.lineage, leaf),
            Self::Case(sum) => {
                overlap(&row.lineage, sum)
                    && !(row.lineage.segments.len() > sum.segments.len()
                        && row.lineage.segments.starts_with(&sum.segments))
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
    /// It observes an actual value or case payload whose declaration proves
    /// that no initialized value can inhabit it; no transfer occurs.
    Empty {
        source: PlaceId,
        path: &'a [StructuralPathSegment],
    },
}

/// One operation or terminator that reads storage or names a case payload.
struct Touch<'a> {
    cleanup: Option<(EdgeId, u32)>,
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
    float_meanings: std::cell::OnceCell<
        Result<terminal_semantics::float_meaning_sources::Sources, &'static str>,
    >,
    types: &'a [StructuralTypeDeclaration],
    graph: &'a Graph,
    initial_contents: Vec<Option<ValueId>>,
    returned_contents: Vec<Vec<(ExactPlaceId, ValueId)>>,
    /// Bound actual operations and structural edge bindings once per machine.
    reaching_budget: usize,
    log: Vec<Logged>,
    cleanup_writes: Vec<crate::owned_cleanup::Write>,
    observations: Vec<BTreeMap<Lineage, Vec<Observation>>>,
    /// The `requires` clauses of each data type, by its identity
    /// (`Elaboration::domains`).
    domains: &'a IdTable<StructuralTypeId, Vec<Proposition>>,
}

impl<'a> Storage<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        machine: MachineId,
        lowered: &'a TerminalMachine,
        module: &'a terminal_psi::TerminalModule,
        domains: &'a IdTable<StructuralTypeId, Vec<Proposition>>,
        graph: &'a Graph,
        tables: &MachineTables,
        value_types: &IdTable<ValueId, ScalarType>,
        references: &mut DescriptorAnalysis<'a>,
    ) -> Checked<Self> {
        // First, because every lineage below is read through the place
        // table's parent links.
        log::places_are_interned(machine, tables)?;
        let reaching_budget = lowered
            .blocks
            .iter()
            .map(|block| {
                block.operations.len()
                    + block
                        .terminator
                        .successor_block_arguments()
                        .iter()
                        .map(|arguments| arguments.len())
                        .sum::<usize>()
            })
            .sum::<usize>()
            + 1;
        let contents = initial_contents(machine, tables, graph.operation_count())?;
        let mut storage = Self {
            machine,
            lowered,
            module,
            float_meanings: std::cell::OnceCell::new(),
            types: &module.structural_types,
            graph,
            initial_contents: contents.entry,
            returned_contents: contents.calls,
            reaching_budget,
            log: Vec::new(),
            cleanup_writes: Vec::new(),
            observations: (0..graph.block_count()).map(|_| BTreeMap::new()).collect(),
            domains,
        };
        // Entry observations occupy position zero. Install them before the
        // operation loop appends reads in position order for binary search.
        storage.initial_contents_are_declared(references, tables)?;
        storage.log = storage.stores(references, value_types)?;
        storage.cleanup_writes = crate::owned_cleanup::writes(module, lowered, references)?;
        storage.log_is_stated(tables)?;
        storage.cleanup_log_is_stated(tables)?;
        // Compare the authored log first. Then every write uses the same
        // captured backing relation as reads and loan compatibility.
        let relocated = storage
            .log
            .iter()
            .map(|row| {
                let concerns = Construct::Operation(row.operation);
                let before = row
                    .position
                    .checked_sub(1)
                    .ok_or_else(|| storage.foreign(concerns, "a write has no formation point"))?;
                let destination =
                    storage.backing(references, &row.lineage, row.block, before, concerns)?;
                let source = if let Stored::Moved(source) = &row.stored {
                    Some(storage.backing(references, source, row.block, before, concerns)?)
                } else {
                    None
                };
                Ok((destination, source))
            })
            .collect::<Checked<Vec<_>>>()?;
        for (row, (destination, source)) in storage.log.iter_mut().zip(relocated) {
            row.lineage = destination;
            if let Some(source) = source {
                row.stored = Stored::Moved(source);
            }
        }
        storage.byte_payload_writes(references)?;
        for block in &lowered.blocks {
            for (before, operation) in block.operations.iter().enumerate() {
                let Some(leaf) = storage.read_leaf(operation)? else {
                    continue;
                };
                let concerns = Construct::Operation(operation.id);
                let leaf = storage.backing(references, &leaf, block.id, before, concerns)?;
                let value = operation
                    .result
                    .scalar_ref()
                    .ok_or_else(|| {
                        storage.foreign(concerns, "a storage observation has no scalar result")
                    })?
                    .id;
                storage.observations[index(block.id.get())]
                    .entry(leaf)
                    .or_default()
                    .push(Observation {
                        point: ProgramPoint::AfterOperation(operation.id),
                        position: before + 1,
                        value,
                    });
            }
        }
        Ok(storage)
    }

    fn validate_float_meaning_source(&self, source: &ScalarTerm) -> Result<(), crate::Unbound> {
        let sources = self
            .float_meanings
            .get_or_init(|| {
                if self.lowered.id != self.machine {
                    return Err("float meaning source owner differs from the checked machine");
                }
                terminal_semantics::float_meaning_sources::Sources::for_machine(self.lowered)
            })
            .as_ref()
            .map_err(|detail| crate::Unbound::Unsupported(detail))?;
        sources
            .source(source)
            .map(|_| ())
            .map_err(crate::Unbound::Unsupported)
    }

    /// Resolve a permission crossing at its actual use. Descriptor identity
    /// is not backing identity, and a later reassignment must not retarget an
    /// earlier captured borrow. Ambiguous or missing correspondence refuses.
    fn backing(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        place: &Lineage,
        block: BlockId,
        before: usize,
        concerns: Construct,
    ) -> Checked<Lineage> {
        let borrowed_arrival =
            matches!(
                self.root_kind(place.root, concerns)?,
                StructuralPlaceKind::BlockParameter { .. }
            ) && terminal_semantics::structural_argument_source(self.lowered, place.root, 0)
                .is_some_and(|parameter| {
                    parameter.access != terminal_psi::StructuralAccess::Owned
                        && !terminal_semantics::descriptor_results::is_descriptor(
                            self.types.iter(),
                            parameter.structural_type,
                        )
                });
        if !borrowed_arrival && !place.segments.contains(&Segment::Referent) {
            return Ok(place.clone());
        }
        let length = self
            .lowered
            .blocks
            .iter()
            .find(|row| row.id == block)
            .ok_or_else(|| self.foreign(concerns, "reference use has no block"))?
            .operations
            .len();
        let before = u32::try_from(before.min(length)).map_err(|_| {
            self.foreign(
                concerns,
                "reference use position exceeds its descriptor graph",
            )
        })?;
        // A runtime selector or an extent names a part of storage, not a
        // different backing: `main.nums[j]` through `main: &mut Main` is
        // element `j` of whatever `main.nums` designates here. The selector
        // is an immutable value captured when the place was formed (loans.md:
        // "Loan formation captures a place, not a live selector
        // expression"), so resolve the storage prefix before the first such
        // step at this use and keep the rest as written. Crossing a
        // permission beneath a runtime selector would need the backing of a
        // descriptor the replay cannot name, so that still refuses.
        let split = place
            .segments
            .iter()
            .position(|segment| matches!(segment, Segment::RuntimeIndex { .. } | Segment::Extent))
            .unwrap_or(place.segments.len());
        let (prefix, suffix) = place.segments.split_at(split);
        let unresolved = || {
            self.unsupported(
                concerns,
                "reference backing with an unresolved selector or extent",
            )
        };
        if suffix.contains(&Segment::Referent) {
            return Err(unresolved());
        }
        let location = Lineage {
            root: place.root,
            segments: prefix.to_vec(),
        }
        .descriptor_location()
        .ok_or_else(unresolved)?;
        let captured = references
            .storage_projection(self.machine, (block, before), &location)
            .ok_or_else(|| {
                self.unsupported(
                    concerns,
                    terminal_semantics::descriptor_results::INEXACT_REFERENCE_BACKING,
                )
            })?;
        let terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument {
            source: root,
            projection,
        } = captured
        else {
            return Err(unresolved());
        };
        let path = projection.captured_storage_path().ok_or_else(unresolved)?;
        let mut resolved = self.stored_at(root, &path, concerns)?;
        resolved.segments.extend_from_slice(suffix);
        self.shape(&resolved, concerns)?;
        Ok(resolved)
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
            StructuralTypeShape::AtomicCell(cell) if cell.bounds().is_none() => return Ok(()),
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
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Parameter { .. },
            } => {
                let element = self.declaration(*element, concerns)?;
                if element.requires.is_empty()
                    && element.scalar_restriction.is_none()
                    && matches!(element.shape, StructuralTypeShape::PrimitiveScalar(_))
                {
                    return Ok(());
                }
                return Err(self.unsupported(
                    concerns,
                    "symbolic array zero requires an ungated scalar element",
                ));
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
                StructuralFieldType::Scalar(_) | StructuralFieldType::Atomic(_) => {}
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
                Shape::Scalar(_) | Shape::Atomic(_) | Shape::View => {
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
    pub(crate) fn at_entry<'s>(
        &'s self,
        references: &'s mut DescriptorAnalysis<'a>,
        tables: &'s MachineTables,
    ) -> AtEntry<'s, 'a> {
        AtEntry {
            storage: self,
            references,
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
        references: &'s mut DescriptorAnalysis<'a>,
        tables: &'s MachineTables,
        actuals: &[Actual],
        callee_formals: &'s [terminal_psi::StructuralParameterDeclaration],
        site: InvocationSite,
        concerns: Construct,
    ) -> Checked<AtCall<'s, 'a>> {
        let (block, before) = match site {
            InvocationSite::Operation(operation) => {
                let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
                let before = after
                    .checked_sub(1)
                    .ok_or_else(|| self.foreign(concerns, "a call has no point before it"))?;
                (block, before)
            }
            InvocationSite::Cleanup { .. } => {
                return Err(self.unsupported(
                    concerns,
                    "cleanup call requires its exact owner-prestate adapter",
                ));
            }
            InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge) => {
                let (block, _) = self.graph.locate(ProgramPoint::Edge(edge))?;
                (block, self.block(block)?.operations.len())
            }
        };
        let mut places = Vec::new();
        for actual in actuals {
            if let Actual::Place(place) = actual {
                places.push(lineage(self.machine, &tables.places, *place)?);
            }
        }
        Ok(AtCall {
            storage: self,
            references,
            tables,
            scalars: crate::scalar_actuals(actuals),
            places,
            callee_formals,
            block,
            up_to: before,
            concerns,
        })
    }

    pub(crate) fn arrival(
        &self,
        edge: EdgeId,
        concerns: Construct,
    ) -> Checked<(BlockId, &'a [ValueId], &'a [StructuralArgument])> {
        let (source, _) = self.graph.locate(ProgramPoint::Edge(edge))?;
        let block = self.block(source)?;
        match &block.terminator {
            Terminator::Jump {
                edge: found,
                target,
                arguments,
                structural_arguments,
                ..
            } if *found == edge => Ok((*target, arguments, structural_arguments)),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let successor = [when_true, when_false]
                    .into_iter()
                    .find(|successor| successor.edge == edge)
                    .ok_or_else(|| {
                        self.foreign(
                            concerns,
                            "arrival edge is absent from its source terminator",
                        )
                    })?;
                Ok((
                    successor.target,
                    &successor.arguments,
                    &successor.structural_arguments,
                ))
            }
            _ => Err(self.foreign(concerns, "arrival has no runtime argument telescope")),
        }
    }

    pub(crate) fn at_join<'s>(
        &'s self,
        references: &'s mut DescriptorAnalysis<'a>,
        tables: &'s MachineTables,
        formals: &'s [terminal_psi::StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
        renaming: &'s [(ValueId, ValueId)],
        point: ProgramPoint,
        concerns: Construct,
    ) -> Checked<AtJoin<'s, 'a>> {
        if formals.len() != arguments.len() {
            return Err(self.foreign(concerns, "join structural arguments differ from its header"));
        }
        let (block, up_to) = self.graph.locate(point)?;
        let places = arguments
            .iter()
            .map(|argument| self.argument(argument, concerns))
            .collect::<Checked<Vec<_>>>()?;
        Ok(AtJoin {
            inputs: AtCall {
                storage: self,
                references,
                tables,
                scalars: Vec::new(),
                places,
                callee_formals: formals,
                block,
                up_to,
                concerns,
            },
            renaming,
        })
    }

    /// A normal call guarantee observes the actual returned root after its
    /// producer. Owned input names keep their before-call values; borrowed
    /// reads keep the captured actual place and observe it after this call.
    pub(crate) fn after_call<'s>(
        &'s self,
        references: &'s mut DescriptorAnalysis<'a>,
        tables: &'s MachineTables,
        actuals: &[Actual],
        (formals, result): (
            &'s [terminal_psi::StructuralParameterDeclaration],
            Option<&TerminalMachineResult>,
        ),
        site: OperationId,
        concerns: Construct,
    ) -> Checked<Returned<'s, 'a, AtCall<'s, 'a>>> {
        let inputs = self.at_call(
            references,
            tables,
            actuals,
            formals,
            InvocationSite::Operation(site),
            concerns,
        )?;
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
        references: &'s mut DescriptorAnalysis<'a>,
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
            inputs: self.at_entry(references, tables),
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        owner: &Owner,
        block: BlockId,
        up_to: usize,
        point: ProgramPoint,
        concerns: Construct,
    ) -> Checked<Proposition> {
        let mut bindings = Held {
            point: Some(point),
            storage: self,
            references,
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        operation: OperationId,
        clause: u32,
        concerns: Construct,
    ) -> Checked<Proposition> {
        let owner = self.owner(operation, clause, concerns)?;
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
        self.stated(
            references,
            tables,
            &owner,
            block,
            after,
            ProgramPoint::AfterOperation(operation),
            concerns,
        )
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
        let Some(stored) = self.field_destination(self.operation(operation, concerns)?)? else {
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
            StructuralPlaceKind::BlockParameter { block, position } => self
                .block(block)?
                .structural_parameters
                .get(usize::try_from(position).unwrap_or(usize::MAX))
                .filter(|parameter| parameter.position == position && parameter.place == root)
                .map(|parameter| parameter.structural_type)
                .ok_or_else(|| self.foreign(concerns, "a block formal's place has no declaration")),
            StructuralPlaceKind::ProviderAttachment { .. }
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
            ref field if field.atomic_cell_type().is_some() => Ok(Shape::Atomic(
                field.atomic_cell_type().expect("checked atomic field"),
            )),
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
            StructuralTypeShape::AtomicCell(cell) => Shape::Atomic(*cell),
            StructuralTypeShape::PrimitiveScalar(scalar_type) => Shape::Scalar(*scalar_type),
            _ => Shape::Typed(root),
        };
        for segment in &place.segments {
            at = match (at, segment) {
                (
                    Shape::Typed(_) | Shape::Scalar(_),
                    Segment::RepresentationView {
                        structural_type,
                        stored_scalar,
                    },
                ) => {
                    let view = terminal_semantics::representation_view_declaration(
                        self.module.structural_types.iter(),
                        *structural_type,
                        *stored_scalar,
                    )
                    .ok_or_else(|| {
                        self.foreign(
                            concerns,
                            "a storage view has no matching presentation and codec",
                        )
                    })?;
                    match view.shape {
                        StructuralTypeShape::AtomicCell(cell) => Shape::Atomic(cell),
                        StructuralTypeShape::PrimitiveScalar(scalar) => Shape::Scalar(scalar),
                        _ => Shape::Typed(view.id),
                    }
                }
                (Shape::Payload(_), Segment::RepresentationView { .. }) => {
                    return Err(
                        self.foreign(concerns, "a storage view is applied to a case namespace")
                    );
                }
                (Shape::Atomic(_), _) => {
                    return Err(self.foreign(concerns, "a place lies beneath an atomic cell"));
                }
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
                (Shape::Typed(owner), Segment::Referent) => {
                    let StructuralTypeShape::Reference { referent, .. } =
                        self.declaration(owner, concerns)?.shape
                    else {
                        return Err(self.foreign(
                            concerns,
                            "a referent crossing without a reference descriptor",
                        ));
                    };
                    match self.declaration(referent, concerns)?.shape {
                        StructuralTypeShape::AtomicCell(cell) => Shape::Atomic(cell),
                        StructuralTypeShape::PrimitiveScalar(scalar) => Shape::Scalar(scalar),
                        _ => Shape::Typed(referent),
                    }
                }
                (Shape::Payload(_), Segment::Referent) => {
                    return Err(self.foreign(concerns, "a referent crossing on a case namespace"));
                }
                (Shape::Typed(owner), Segment::Extent) => {
                    let shape = &self.declaration(owner, concerns)?.shape;
                    if !shape.is_borrowed_byte_view()
                        && !matches!(
                            shape,
                            StructuralTypeShape::ElementView {
                                access: None
                                    | Some(
                                        StructuralAccess::SharedBorrow
                                            | StructuralAccess::MutableBorrow
                                    ),
                                ..
                            } | StructuralTypeShape::FixedArray {
                                length: terminal_psi::ArrayExtent::Literal(_),
                                ..
                            }
                        )
                    {
                        return Err(self.foreign(
                            concerns,
                            "an extent requires a readable descriptor or a closed array",
                        ));
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
                    let element = match self.declaration(owner, concerns)?.shape {
                        StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } => {
                            if matches!(segment, Segment::FixedIndex(index) if *index >= length) {
                                return Err(self.foreign(
                                    concerns,
                                    "a literal index exceeds its array extent",
                                ));
                            }
                            element
                        }
                        StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Parameter { .. },
                        } if matches!(segment, Segment::RuntimeIndex { .. }) => element,
                        StructuralTypeShape::ElementView { element, .. }
                            if matches!(segment, Segment::RuntimeIndex { .. }) =>
                        {
                            element
                        }
                        _ => {
                            return Err(self.foreign(
                                concerns,
                                "an index needs a closed array or a live view bound",
                            ));
                        }
                    };
                    match self.declaration(element, concerns)?.shape {
                        StructuralTypeShape::AtomicCell(cell) => Shape::Atomic(cell),
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
            Shape::Atomic(_) | Shape::Scalar(_) | Shape::View => found.push(at),
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
                StructuralFieldType::Atomic(_)
                | StructuralFieldType::AtomicBoundedInteger(_)
                | StructuralFieldType::Scalar(_) => found.push(at.clone()),
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
            // References and opaque values each occupy one established
            // place. Neither grants owned access to hidden backing storage.
            StructuralTypeShape::AtomicCell(_)
            | StructuralTypeShape::Opaque
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::Reference { .. } => {
                found.push(at.clone());
            }
            // Availability concerns the descriptor, not ownership of its
            // backing. An exclusive descriptor is readable too; copying it
            // still requires the separate unrestricted-copy judgment.
            shape if shape.is_view_descriptor() => found.push(at.clone()),
            // A symbolic array can be established whole without enumerating
            // N elements. Every selected read still owes its own index bound.
            StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Parameter { .. },
                ..
            } => {
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
                    StructuralPathSegment::Referent => {
                        segments.push(Segment::Referent);
                        continue;
                    }
                    StructuralPathSegment::RepresentationView {
                        structural_type,
                        stored_scalar,
                    } => {
                        // The view keeps the same backing and changes how
                        // later fields are resolved. Retain it for the shared
                        // shape and overlap judgments, rather than erasing it.
                        segments.push(Segment::RepresentationView {
                            structural_type: *structural_type,
                            stored_scalar: *stored_scalar,
                        });
                        continue;
                    }
                    StructuralPathSegment::Field(_) => {}
                    StructuralPathSegment::AllElements
                    | StructuralPathSegment::FixedRange { .. } => {
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
        let leaf = match &operation.kind {
            OperationKind::ByteSequenceLength { source }
            | OperationKind::ElementViewLength { source } => Some(self.argument(source, concerns)?),
            OperationKind::StructuralByteSequenceFieldLength {
                source,
                path,
                field,
            } => Some(self.argument(
                &self.byte_field_argument(*source, path, *field, concerns)?,
                concerns,
            )?),
            _ => None,
        };
        if let Some(leaf) = leaf {
            return self.descriptor_length_leaf(operation, leaf).map(Some);
        }
        match &operation.kind {
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
                let mut leaf = extended(
                    Lineage {
                        root: *source,
                        segments: Vec::new(),
                    },
                    path,
                )
                .map_err(|unbound| unbound.refusal(self.machine, concerns))?;
                leaf.segments.push(Segment::Field(*field));
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

    /// Whole and stored descriptors expose the same extent leaf. Keeping
    /// one observation rule lets guards and later indexed reads agree on it.
    fn descriptor_length_leaf(&self, operation: &Operation, mut leaf: Lineage) -> Checked<Lineage> {
        let concerns = Construct::Operation(operation.id);
        let element_count = matches!(operation.kind, OperationKind::ElementViewLength { .. });
        let descriptor = match self.shape(&leaf, concerns)? {
            Shape::View => !element_count,
            Shape::Typed(owner) => {
                let shape = &self.declaration(owner, concerns)?.shape;
                if element_count {
                    matches!(shape, StructuralTypeShape::ElementView { .. })
                } else {
                    shape.is_borrowed_byte_view()
                }
            }
            _ => false,
        };
        if !descriptor {
            let reason = if element_count {
                "an element length operation requires an element descriptor"
            } else {
                "a byte length operation requires a byte descriptor"
            };
            return Err(self.foreign(concerns, reason));
        }
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
        Ok(leaf)
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
                if let OperationKind::ByteSequenceSubslice {
                    obligation: owned, ..
                }
                | OperationKind::ElementViewSubslice {
                    obligation: owned, ..
                } = &operation.kind
                {
                    if *owned != obligation {
                        return Err(self.foreign(concerns, "a subslice owns another obligation"));
                    }
                    return self.subslice_goal(operation, types);
                }
                if let OperationKind::ByteSequenceRead {
                    obligation: owned, ..
                }
                | OperationKind::ByteSequenceWrite {
                    obligation: owned, ..
                }
                | OperationKind::StructuralByteSequenceFieldRead {
                    obligation: owned, ..
                } = &operation.kind
                {
                    if *owned != obligation {
                        return Err(
                            self.foreign(concerns, "a byte operation owns another obligation")
                        );
                    }
                    return self.collection_index_goal(operation, types);
                }
                if matches!(operation.kind, OperationKind::ElementViewRead {
                    obligation: owned, ..
                } if owned == obligation)
                {
                    return self.collection_index_goal(operation, types);
                }
                if let OperationKind::EstablishElementView {
                    region:
                        terminal_psi::ElementViewRegion::WholeRepresentation {
                            obligation: owned, ..
                        },
                    ..
                } = &operation.kind
                {
                    if *owned != obligation {
                        return Err(self.foreign(concerns, "a re-view owns another obligation"));
                    }
                    return self.representation_count_goal(operation);
                }
                operation.kind.structural_projections()
            }
            ExecutionSite::Block(id) => self.block(id)?.terminator.storage_uses(),
        };
        let preceding = match site {
            ExecutionSite::Operation(id) => {
                let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(id))?;
                let before = after.checked_sub(1).ok_or_else(|| {
                    self.foreign(concerns, "an indexed operation has no position")
                })?;
                &self.block(block)?.operations[..before]
            }
            ExecutionSite::Block(id) => self.block(id)?.operations.as_slice(),
        };
        let mut found = None;
        for projection in projections {
            if !projection.path.iter().any(|step| {
                step.runtime_index()
                    .is_some_and(|(_, owned)| owned == obligation)
            }) {
                continue;
            }
            let root = self.root_type(projection.root, concerns)?;
            let mut duplicate = false;
            terminal_semantics::visit_runtime_index_goals(
                self.types.iter(),
                |id| self.declaration(id, concerns).ok(),
                root,
                projection.path,
                types,
                |prefix| {
                    terminal_semantics::preceding_element_view_length(
                        preceding,
                        projection.root,
                        prefix,
                    )
                },
                |owned, goal| {
                    if owned == obligation {
                        duplicate |= found.replace(goal).is_some();
                    }
                },
            )
            .map_err(|error| {
                use terminal_semantics::RuntimeIndexGoalError;
                match error {
                    RuntimeIndexGoalError::InvalidPath => {
                        self.foreign(concerns, "an indexed path lost its actual array type")
                    }
                    RuntimeIndexGoalError::OpenExtent => self
                        .unsupported(concerns, "index into storage without a closed array extent"),
                    RuntimeIndexGoalError::MissingExtent => self.unsupported(
                        concerns,
                        "index into a view without its current descriptor length",
                    ),
                    RuntimeIndexGoalError::MissingSelector => {
                        self.foreign(concerns, "an array selector has no scalar type")
                    }
                    RuntimeIndexGoalError::UnsupportedSelector => {
                        self.unsupported(concerns, "array selector without integer bound semantics")
                    }
                }
            })?;
            if duplicate {
                return Err(
                    self.foreign(concerns, "two array projections own one index obligation")
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

    /// Reads and writes prove their bound against the exact descriptor
    /// measured immediately before use. A write additionally needs exclusive
    /// backing access; neither an equal count nor another view supplies it.
    fn collection_index_goal(
        &self,
        operation: &Operation,
        types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<Proposition> {
        let concerns = Construct::Operation(operation.id);
        let byte = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .expect("u8 is valid"),
        );
        let element_read = matches!(operation.kind, OperationKind::ElementViewRead { .. });
        let (source, index, length) = match &operation.kind {
            OperationKind::ByteSequenceRead {
                source,
                index,
                length,
                ..
            } => {
                self.readable_byte_view(operation, *source, &[])?;
                if operation.result.scalar_ref().is_none_or(|result| {
                    result.scalar_type != byte || result.qualifications != Default::default()
                }) {
                    return Err(self.foreign(concerns, "a byte read must produce a plain u8"));
                }
                (
                    StructuralArgument::whole(
                        *source,
                        terminal_psi::StructuralAccess::SharedBorrow,
                    ),
                    *index,
                    *length,
                )
            }
            OperationKind::StructuralByteSequenceFieldRead {
                source,
                path,
                field,
                index,
                length,
                ..
            } => {
                let source = self.byte_field_argument(*source, path, *field, concerns)?;
                self.readable_byte_view(operation, source.place, &source.path)?;
                if operation.result.scalar_ref().is_none_or(|result| {
                    result.scalar_type != byte || result.qualifications != Default::default()
                }) {
                    return Err(self.foreign(concerns, "a byte read must produce a plain u8"));
                }
                (source, *index, *length)
            }
            OperationKind::ByteSequenceWrite {
                destination,
                index,
                value,
                length,
                ..
            } => {
                self.writable_byte_view(operation, *destination)?;
                if !matches!(operation.result, terminal_psi::OperationResult::Unit)
                    || types.get(*value).copied() != Some(byte)
                {
                    return Err(
                        self.foreign(concerns, "a byte store requires a u8 value and Unit result")
                    );
                }
                (
                    StructuralArgument::whole(
                        *destination,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    ),
                    *index,
                    *length,
                )
            }
            OperationKind::ElementViewRead {
                source,
                index,
                length,
                path,
                ..
            } => {
                let root = self.root_type(source.place, concerns)?;
                let Some(terminal_psi::StructuralTypeShape::ElementView { element, .. }) =
                    terminal_semantics::runtime_structural_path_tip(
                        self.types.iter(),
                        root,
                        &source.path,
                    )
                    .map(|declaration| &declaration.shape)
                else {
                    return Err(self.foreign(concerns, "an element read needs a declared view"));
                };
                if operation.result.scalar_ref().is_none_or(|result| {
                    terminal_semantics::element_view_read_scalar(self.types, *element, path)
                        != Some(result.scalar_type)
                }) {
                    return Err(
                        self.foreign(concerns, "an element read needs a copyable scalar leaf")
                    );
                }
                if source.access != terminal_psi::StructuralAccess::SharedBorrow
                    || terminal_semantics::structural_projection_authority(
                        self.lowered,
                        self.types,
                        source,
                    )
                    .is_none_or(|available| !source.access.admits_referent(available))
                {
                    return Err(self.foreign(concerns, "an element read needs a readable view"));
                }
                (source.clone(), *index, *length)
            }
            _ => return Err(self.foreign(concerns, "byte bounds requested for another operation")),
        };
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let mut expected = self.argument(&source, concerns)?;
        expected.segments.push(Segment::Extent);
        let operations = &self.block(block)?.operations;
        let measurement = after
            .checked_sub(2)
            .and_then(|position| operations.get(position));
        let measured = match measurement {
            Some(measurement) => {
                self.read_leaf(measurement)?.as_ref() == Some(&expected)
                    && measurement
                        .result
                        .scalar_ref()
                        .is_some_and(|result| result.id == length)
            }
            None => false,
        };
        if !measured {
            return Err(self.unsupported(
                concerns,
                if element_read {
                    "an element read whose length is not its view's measurement directly before it"
                } else {
                    "a byte operation whose length is not its view's measurement directly before it"
                },
            ));
        }
        let count = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        if types.get(index).copied() != Some(count) || types.get(length).copied() != Some(count) {
            return Err(self.foreign(
                concerns,
                if element_read {
                    "an element read's index or length is not a u64"
                } else {
                    "a byte operation's index or length is not a u64"
                },
            ));
        }
        terminal_semantics::operation_safety_goal(operation, types)
            .map_err(crate::semantics_refusal(self.machine, operation.id))?
            .map(|(_, goal)| goal)
            .ok_or_else(|| self.foreign(concerns, "a byte operation has no canonical bounds goal"))
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        mut row_index: usize,
        asked: Asked<'_>,
        budget: &mut Reaching,
        concerns: Construct,
    ) -> Checked<Supply> {
        let mut weakened = false;
        loop {
            let row = &self.log[row_index];
            // A window asks only which store a path met last: the one that
            // opened it, or one that closed it.
            if let Asked::Window { by, .. } = asked {
                return Ok(Supply::Direct(if row.operation == by {
                    Reached::Open
                } else {
                    Reached::Closed
                }));
            }
            let place = asked.place();
            // Only a store into the place or above it replaces it whole; one
            // beneath it or beside it leaves contents this route cannot name.
            let Some(beneath) = place.segments.strip_prefix(row.lineage.segments.as_slice()) else {
                if matches!(row.stored, Stored::Vacated) {
                    return Ok(Supply::Direct(Reached::Unestablished));
                }
                // A constructor first establishes its whole case, then its
                // fields at the same operation. Follow earlier stores of that
                // operation before crossing its observation point.
                let earlier = self.log[..row_index]
                    .iter()
                    .enumerate()
                    .rev()
                    .take_while(|(_, previous)| {
                        previous.block == row.block && previous.position == row.position
                    })
                    .find(|(_, previous)| asked.ended_by(previous));
                if let Some((index, _)) = earlier {
                    row_index = index;
                    weakened = true;
                    continue;
                }
                return Ok(self
                    .reaching_supply(
                        references,
                        tables,
                        asked,
                        row.block,
                        row.position - 1,
                        budget,
                        concerns,
                    )?
                    .without_identity());
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
                (Stored::Established, Asked::Contents(_))
                    if beneath == [Segment::Extent]
                        && matches!(
                            self.operation(row.operation, concerns)?.kind,
                            OperationKind::ByteSequenceSubslice { .. }
                                | OperationKind::ElementViewSubslice { .. }
                                | OperationKind::EstablishByteSequenceLiteral { .. }
                                | OperationKind::EstablishElementView { .. }
                                | OperationKind::EstablishMutableElementView { .. }
                                | OperationKind::EstablishStructuralArray { .. }
                        ) =>
                {
                    Reached::Extent(row.operation)
                }
                (Stored::Established, Asked::Contents(leaf)) => {
                    let named = (budget.observations != Observations::None)
                        .then(|| self.interned(tables, leaf, concerns))
                        .transpose()?
                        .flatten()
                        .and_then(|place| {
                            self.returned_contents
                                .get(index(row.operation.get()))
                                .and_then(|values| {
                                    values
                                        .binary_search_by_key(&place, |(place, _)| *place)
                                        .ok()
                                        .map(|found| values[found].1)
                                })
                        });
                    if let Some(named) = named {
                        let supply = Supply::Boundary {
                            value: named,
                            depth: beneath.len(),
                        };
                        return Ok(if weakened {
                            supply.without_identity()
                        } else {
                            supply
                        });
                    } else if row.lineage.segments.is_empty()
                        && matches!(self.root_kind(leaf.root, concerns)?,
                            StructuralPlaceKind::OperationResult { producer, .. }
                                if producer == row.operation)
                        && self
                            .operation(row.operation, concerns)?
                            .kind
                            .declared_invocation()
                            .is_some()
                    {
                        Reached::Established(beneath.len())
                    } else {
                        Reached::Unknown
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
                    let supply = self
                        .reaching_supply(
                            references,
                            tables,
                            asked,
                            row.block,
                            row.position - 1,
                            budget,
                            concerns,
                        )?
                        .within(beneath.len());
                    return Ok(if weakened {
                        supply.without_identity()
                    } else {
                        supply
                    });
                }
            };
            return Ok(Supply::Direct(if weakened {
                reached.without_identity()
            } else {
                reached
            }));
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
    /// A membership whose path crosses a runtime element reads "some
    /// element": it "names no canonical sum and keys no case fact"
    /// (structural_access.md, case observation), so its true edge
    /// establishes nothing either.
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
                if self.stored_at(*source, path, concerns)? != *sum
                    || sum
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, Segment::RuntimeIndex { .. }))
                {
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
    /// A helper can change a slice's bytes while keeping its original window.
    /// Preserve a length fact only after proving the same descriptor survives
    /// every normal return through each overlapping loan.
    fn call_preserves_extent(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        asked: Asked<'_>,
        row: &Logged,
        concerns: Construct,
    ) -> Checked<bool> {
        let (Stored::Established, Asked::Contents(leaf)) = (&row.stored, asked) else {
            return Ok(false);
        };
        let Some((Segment::Extent, parent)) = leaf.segments.split_last() else {
            return Ok(false);
        };
        let descriptor = Lineage {
            root: leaf.root,
            segments: parent.to_vec(),
        };
        let operation = self.operation(row.operation, concerns)?;
        if crate::call_contracts::operation_inputs(operation).is_none() {
            return Ok(false);
        }
        let (block, after) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let position = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "call observation has no operation position"))?;
        let at = (
            block,
            u32::try_from(position)
                .map_err(|_| self.foreign(concerns, "call observation position exceeds u32"))?,
        );
        let watched = self.backing(references, &descriptor, block, position, concerns)?;
        let mut covered = false;
        for (parameter, argument) in operation.kind.structural_arguments().iter().enumerate() {
            if !matches!(
                argument.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                continue;
            }
            let actual = self.argument(argument, concerns)?;
            let actual = self.backing(references, &actual, block, position, concerns)?;
            if !overlap(&watched, &actual) {
                continue;
            }
            if watched.root != actual.root || !watched.segments.starts_with(&actual.segments) {
                return Ok(false);
            }
            let relative = Lineage {
                root: watched.root,
                segments: watched.segments[actual.segments.len()..].to_vec(),
            };
            let Some(relative) = relative.descriptor_location() else {
                return Ok(false);
            };
            if !references
                .call_preserves_descriptor_storage(self.machine, at, parameter, &relative.path)
                .map_err(|detail| self.unsupported(concerns, detail))?
            {
                return Ok(false);
            }
            covered = true;
        }
        Ok(covered)
    }

    /// Whether `block` is the machine entry or belongs to a control-flow
    /// cycle: at such a block incoming facts do not dominate across backedges,
    /// so head contracts and input inference name the block's entry snapshots.
    fn cuts_incoming_facts(&self, block: BlockId) -> Checked<bool> {
        Ok(block == self.graph.entry() || self.graph.in_cycle(block)?)
    }

    /// Whether a window on a block formal is closed at a point outside the
    /// formal's scope. A block formal names storage only in its own block and
    /// the blocks that block dominates; an edge that passes it on renames it
    /// to the receiving block's formal. Walk forward from the formal's block:
    /// an edge that passes the formal on whole is a whole use that consumes
    /// the window, so the clause is proved there (dependent_values.md,
    /// "Invariant windows"), and the receiving formal's binding establishes a
    /// whole value, so that path ends; an edge into a dominated block keeps
    /// the formal in scope and the walk continues. When the opening write
    /// lies in the walked scope, the asked point does not, and no edge leaves
    /// the scope without passing the formal, every path from the opening
    /// write to the point is closed: a path that never enters the scope never
    /// passed the opening write. Any other shape, including a dispatch arm
    /// that binds the formal, keeps the ordinary walk, which refuses a
    /// backing it cannot name.
    fn window_handed_off(
        &self,
        place: &Lineage,
        opened_by: OperationId,
        (block, up_to): (BlockId, usize),
        used: Option<ProgramPoint>,
        concerns: Construct,
    ) -> Checked<bool> {
        let StructuralPlaceKind::BlockParameter { block: owner, .. } =
            self.root_kind(place.root, concerns)?
        else {
            return Ok(false);
        };
        let used = match used {
            Some(used) => used,
            None => {
                let operations = &self.block(block)?.operations;
                match up_to.min(operations.len()).checked_sub(1) {
                    Some(last) => ProgramPoint::AfterOperation(operations[last].id),
                    None => ProgramPoint::BlockEntry(block),
                }
            }
        };
        let scope = ProgramPoint::BlockEntry(owner);
        if self.graph.dominates(scope, used)? {
            return Ok(false);
        }
        let (opened_in, _) = self.graph.locate(ProgramPoint::AfterOperation(opened_by))?;
        let mut walked = vec![false; self.graph.block_count()];
        let mut pending = vec![owner];
        while let Some(current) = pending.pop() {
            let slot = walked
                .get_mut(index(current.get()))
                .ok_or_else(|| self.foreign(concerns, "a block is outside its machine"))?;
            if std::mem::replace(slot, true) {
                continue;
            }
            let terminator = &self.block(current)?.terminator;
            let successors = terminator.successor_edges();
            let bindings = terminator.successor_block_arguments();
            if successors.len() != bindings.len() {
                return Err(self.foreign(concerns, "an edge has no complete binding roster"));
            }
            // Only a jump's arguments are a whole use that consumes the
            // window (`touches`); a dispatch arm's binding is not.
            let consumes = matches!(terminator, Terminator::Jump { .. });
            for ((edge, into), arguments) in successors.into_iter().zip(bindings) {
                let mut passed = false;
                for argument in arguments.iter().filter(|_| consumes) {
                    let argument = self.argument(argument, Construct::Edge(edge))?;
                    passed |= argument.root == place.root
                        && place.segments.starts_with(&argument.segments);
                }
                if passed {
                    continue;
                }
                if into == owner
                    || !self
                        .graph
                        .dominates(scope, ProgramPoint::BlockEntry(into))?
                {
                    return Ok(false);
                }
                pending.push(into);
            }
        }
        Ok(walked.get(index(opened_in.get())).copied().unwrap_or(false))
    }

    fn reaching(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        asked: Asked<'_>,
        (block, up_to): (BlockId, usize),
        trace: &mut Reaching,
        concerns: Construct,
    ) -> Checked<Reached> {
        if let Asked::Window { place, by, .. } = asked
            && self.window_handed_off(place, by, (block, up_to), trace.used, concerns)?
        {
            return Ok(Reached::Closed);
        }
        let root =
            self.reaching_supply(references, tables, asked, block, up_to, trace, concerns)?;
        let Supply::Transfer { source, depth, .. } = root else {
            unreachable!("a query is an equation")
        };
        for equation in &mut trace.equations {
            // Entry crossings discovered anywhere in the query retire reads
            // from an earlier iteration, even if we saw the read first.
            if let Reached::Value(value) = equation.seed
                && let Some(owner) = self.observation_block(tables, value, concerns)?
                && trace.crossed_entries[index(owner.get())]
            {
                equation.seed = Reached::Unknown;
                equation.seed_boundary = BoundaryDepth::Local;
            }
        }
        let mut solved = trace.solve(source);
        for position in (0..trace.fallbacks.len()).rev() {
            let fallback = trace.fallbacks[position];
            let arrival = match trace.equations[fallback.upstream].direct {
                Reached::Unknown | Reached::Established(_) => Reached::Value(fallback.value),
                Reached::Extent(producer) if !self.current_extent(producer, trace)? => {
                    Reached::Value(fallback.value)
                }
                other => other,
            };
            trace.equations[fallback.destination].seed =
                trace.equations[fallback.destination].seed.merge(arrival);
            trace.equations[fallback.destination].seed_boundary = trace.equations
                [fallback.destination]
                .seed_boundary
                .merge(trace.equations[fallback.upstream].boundary);
            solved = trace.solve(source);
        }
        trace.boundary = trace.equations[source].boundary.within(depth);
        Ok(match solved.within(depth) {
            Reached::Cycle if matches!(asked, Asked::Window { .. }) => Reached::Unknown,
            Reached::Cycle => Reached::Unestablished,
            reached => reached,
        })
    }

    fn reaching_supply(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        asked: Asked<'_>,
        block: BlockId,
        up_to: usize,
        budget: &mut Reaching,
        concerns: Construct,
    ) -> Checked<Supply> {
        let original = asked.place();
        let resolved = self.backing(references, original, block, up_to, concerns)?;
        let asked = asked.at(&resolved);
        let key = match asked {
            Asked::Contents(_) => Some((block, up_to, resolved.clone(), QueryKind::Contents)),
            Asked::Case(_) => Some((block, up_to, resolved.clone(), QueryKind::Case)),
            Asked::Window { .. } => None,
        };
        let projection = if original == &resolved { usize::MAX } else { 0 };
        if let Some(key) = &key
            && let Some(&address) = budget.keys.get(key)
        {
            return Ok(Supply::transfer(address).within(projection));
        }
        let address = budget.add_equation();
        if let Some(key) = key {
            budget.keys.insert(key, address);
        }
        // A real cleanup epoch can end this observation before operation
        // stores do. Feed that result into the same equation graph so shared
        // copy traces and initialization still use the ordinary solver.
        if up_to >= self.block(block)?.operations.len()
            && let Some(point) = budget.used
            && let Some(changed) = self.cleanup_at_point(asked, block, point)?
        {
            budget.add_supply(address, Supply::Direct(changed));
            return Ok(Supply::transfer(address).within(projection));
        }
        // The limit still bounds walks whose projected source keeps changing.
        // Alternative arrivals start with the same remaining depth.
        let remaining = budget.remaining;
        let discovered = (|| {
            budget.remaining = budget
                .remaining
                .checked_sub(1)
                .ok_or_else(|| self.unsupported(concerns, "a chain of copies does not end"))?;
            if budget.used.is_none() {
                let operations = &self.block(block)?.operations;
                let count = up_to.min(operations.len());
                budget.used = Some(if count == 0 {
                    ProgramPoint::BlockEntry(block)
                } else {
                    ProgramPoint::AfterOperation(operations[count - 1].id)
                });
            }
            // Each block is walked whole at most once, so a cycle ends the walk.
            let mut walked = vec![false; self.graph.block_count()];
            let mut pending = vec![(block, address, up_to)];
            while let Some((block, mut destination, up_to)) = pending.pop() {
                // The last row in this block, at or before the point, that ends
                // the path: the latest position, and in one position the later
                // log entry.
                let mut last: Option<(usize, &Logged)> = None;
                // Search this block before following its predecessors. stores emits
                // Graph-checked block identity order, and byte_payload_writes
                // sorts appended effects into that order. Keep the original
                // forward check order and row identities within the block.
                let begin = self.log.partition_point(|row| row.block < block);
                let end = self.log.partition_point(|row| row.block <= block);
                for index in begin..end {
                    let row = &self.log[index];
                    #[cfg(test)]
                    {
                        budget.scanned_rows += 1;
                    }
                    if row.position > up_to
                        || !asked.ended_by(row)
                        || self.call_preserves_extent(references, asked, row, concerns)?
                    {
                        continue;
                    }
                    if last.is_none_or(|(_, previous)| previous.position <= row.position) {
                        last = Some((index, row));
                    }
                }
                if !budget.initialization_only
                    && budget.observations == Observations::All
                    && let Asked::Contents(leaf) = asked
                    && let Some(observations) = self.observations[index(block.get())].get(leaf)
                {
                    let end =
                        observations.partition_point(|observation| observation.position <= up_to);
                    if let Some(observation) = end.checked_sub(1).map(|last| &observations[last])
                        && !matches!(observation.point, ProgramPoint::BlockEntry(_))
                        && last.is_none_or(|(_, row)| row.position < observation.position)
                        && !budget.crossed_entries[index(block.get())]
                        && self.graph.dominates(
                            observation.point,
                            budget.used.ok_or_else(|| {
                                self.foreign(concerns, "a storage trace has no use point")
                            })?,
                        )?
                    {
                        budget.add_supply(
                            destination,
                            Supply::Direct(Reached::Value(observation.value)),
                        );
                        continue;
                    }
                }
                if let Some((index, _)) = last {
                    let supply =
                        self.supplied(references, tables, index, asked, budget, concerns)?;
                    budget.add_supply(destination, supply);
                    continue;
                }
                if !budget.initialization_only
                    && budget.observations == Observations::All
                    && let Asked::Contents(leaf) = asked
                    && !budget.crossed_entries[index(block.get())]
                    && let Some(observation) = self.observations[index(block.get())]
                        .get(leaf)
                        .and_then(|rows| rows.first())
                        .filter(|row| matches!(row.point, ProgramPoint::BlockEntry(_)))
                    && self.graph.dominates(
                        observation.point,
                        budget.used.ok_or_else(|| {
                            self.foreign(concerns, "a storage trace has no use point")
                        })?,
                    )?
                {
                    // A header with a head contract or cycle cuts incoming
                    // facts except its head contract, which names the entry
                    // snapshot, so the snapshot is the value later proofs use.
                    if self.cuts_incoming_facts(block)? {
                        budget.add_supply(
                            destination,
                            Supply::Direct(Reached::Value(observation.value)),
                        );
                        continue;
                    }
                    // Elsewhere, walk incoming arrivals first so uninitialized
                    // paths, single upstream values and current slice
                    // endpoints survive; name the entry snapshot only when
                    // initialized arrivals share no upstream identity.
                    let upstream = budget.add_equation();
                    budget.fallbacks.push(EntryFallback {
                        upstream,
                        destination,
                        value: observation.value,
                    });
                    destination = upstream;
                }
                // A block formal is a new owner of the selected edge's actual
                // value. Follow that exact region before the edge consumes it;
                // do not invent initial contents or pick one predecessor's value.
                if let StructuralPlaceKind::BlockParameter {
                    block: owner,
                    position,
                } = self.root_kind(asked_root(asked), concerns)?
                    && owner == block
                {
                    let target = self.block(block)?;
                    let position = usize::try_from(position).map_err(|_| {
                        self.foreign(concerns, "a block formal position exceeds usize")
                    })?;
                    let parameter =
                        target.structural_parameters.get(position).ok_or_else(|| {
                            self.foreign(concerns, "a block formal is outside its telescope")
                        })?;
                    if parameter.place != asked_root(asked)
                        || usize::try_from(parameter.position).ok() != Some(position)
                    {
                        return Err(self.foreign(concerns, "a block formal has another owner"));
                    }
                    // A transfer establishes a whole value, as a whole store
                    // closes any window on the destination. A window on its
                    // source is owed at the edge's ordinary consumption point.
                    if matches!(asked, Asked::Window { .. }) {
                        budget.add_supply(destination, Supply::Direct(Reached::Closed));
                        continue;
                    }
                    budget.crossed_entries[index(block.get())] = true;
                    let mut predecessors = self.graph.predecessors(block)?.to_vec();
                    predecessors.sort_unstable();
                    predecessors.dedup();
                    let mut found = false;
                    for predecessor in predecessors {
                        let terminator = &self.block(predecessor)?.terminator;
                        let successors = terminator.successor_edges();
                        let bindings = terminator.successor_block_arguments();
                        if successors.len() != bindings.len() {
                            return Err(
                                self.foreign(concerns, "an edge has no complete binding roster")
                            );
                        }
                        for ((edge, into), arguments) in successors.into_iter().zip(bindings) {
                            if into != block {
                                continue;
                            }
                            found = true;
                            if arguments.len() != target.structural_parameters.len() {
                                return Err(self.foreign(
                                    Construct::Edge(edge),
                                    "structural edge arguments differ from the target telescope",
                                ));
                            }
                            let argument = &arguments[position];
                            if !terminal_psi::structural_access_covers(
                                argument.access,
                                parameter.access,
                            ) {
                                return Err(self.foreign(
                                    Construct::Edge(edge),
                                    "a structural edge argument has insufficient access",
                                ));
                            }
                            let mut source = self.argument(argument, Construct::Edge(edge))?;
                            if !matches!(self.shape(&source, Construct::Edge(edge))?,
                            Shape::Typed(actual) if actual == parameter.structural_type)
                            {
                                return Err(self.foreign(
                                    Construct::Edge(edge),
                                    "a structural edge argument has another type",
                                ));
                            }
                            let beneath = &asked.place().segments;
                            source.segments.extend_from_slice(beneath);
                            let asked = match asked {
                                Asked::Contents(_) => Asked::Contents(&source),
                                Asked::Case(_) => Asked::Case(&source),
                                Asked::Window { .. } => {
                                    unreachable!("windows close at the binding")
                                }
                            };
                            let supply = self
                                .reaching_supply(
                                    references,
                                    tables,
                                    asked,
                                    predecessor,
                                    usize::MAX,
                                    budget,
                                    concerns,
                                )?
                                .within(beneath.len());
                            budget.add_supply(destination, supply);
                        }
                    }
                    if !found {
                        return Err(
                            self.foreign(concerns, "a block formal has no incoming binding")
                        );
                    }
                    continue;
                }
                if block == self.graph.entry() {
                    let supply = match (asked, self.root_kind(asked_root(asked), concerns)?) {
                        // A path that never passed the opening write.
                        (Asked::Window { .. }, _) => Supply::Direct(Reached::Closed),
                        // A formal arrives with its declared type established.
                        // A selected field need not name one entry value,
                        // especially when its index changes in a ranked loop.
                        (Asked::Contents(leaf), StructuralPlaceKind::Parameter { .. }) => {
                            let depth = leaf.segments.len();
                            if budget.observations == Observations::None {
                                Supply::Direct(Reached::Established(depth))
                            } else {
                                self.interned(tables, leaf, concerns)?
                                    .and_then(|place| {
                                        self.initial_contents.get(index(place.get())).copied()
                                    })
                                    .flatten()
                                    .map_or(Supply::Direct(Reached::Established(depth)), |value| {
                                        Supply::Boundary { value, depth }
                                    })
                            }
                        }
                        // Which case a formal holds at entry is not known, and
                        // other storage holds nothing there; that it holds
                        // nothing is the contents question's answer to give.
                        (Asked::Case(_), _) => Supply::Direct(Reached::Unknown),
                        (
                            Asked::Contents(_),
                            StructuralPlaceKind::BlockParameter { .. }
                            | StructuralPlaceKind::Result
                            | StructuralPlaceKind::OperationResult { .. }
                            | StructuralPlaceKind::ByteSequenceLiteral { .. }
                            | StructuralPlaceKind::ProviderAttachment { .. }
                            | StructuralPlaceKind::TrivialAffineLocal { .. },
                        ) => Supply::Direct(Reached::Unestablished),
                    };
                    budget.add_supply(destination, supply);
                }
                budget.crossed_entries[index(block.get())] = true;
                let mut asked_of: Vec<BlockId> = Vec::new();
                for predecessor in self.graph.predecessors(block)? {
                    // A block that enters by two edges is listed twice.
                    if asked_of.contains(predecessor) {
                        continue;
                    }
                    asked_of.push(*predecessor);
                    if let Some(changed) = self.cleanup_arrival(asked, *predecessor, block)? {
                        budget.add_supply(destination, Supply::Direct(changed));
                        continue;
                    }
                    if let Asked::Case(sum) = asked
                        && let Some(selected) =
                            self.edge_selects(tables, sum, *predecessor, block, concerns)?
                    {
                        budget.add_supply(destination, Supply::Direct(selected));
                        continue;
                    }
                    let slot = walked.get_mut(index(predecessor.get())).ok_or_else(|| {
                        self.foreign(
                            Construct::Block(*predecessor),
                            "an edge leaves a block past the end",
                        )
                    })?;
                    if !std::mem::replace(slot, true) {
                        pending.push((*predecessor, destination, usize::MAX));
                    }
                }
            }
            Ok(())
        })();
        budget.remaining = remaining;
        discovered?;
        Ok(Supply::transfer(address).within(projection))
    }

    fn budget(&self) -> Reaching {
        Reaching {
            keys: BTreeMap::new(),
            equations: Vec::new(),
            fallbacks: Vec::new(),
            boundary: BoundaryDepth::Cycle,
            initialization_only: false,
            observations: Observations::None,
            remaining: self.reaching_budget,
            crossed_entries: vec![false; self.graph.block_count()],
            used: None,
            #[cfg(test)]
            scanned_rows: 0,
        }
    }

    fn provenance_budget(&self) -> Reaching {
        Reaching {
            observations: Observations::Boundary,
            ..self.budget()
        }
    }

    pub(super) fn snapshot_budget(&self) -> Reaching {
        Reaching {
            observations: Observations::All,
            ..self.budget()
        }
    }

    fn initialization_budget(&self) -> Reaching {
        // Reads supply snapshots rather than initialize storage. Suppress
        // their observations and project away identity from the same solver.
        Reaching {
            initialization_only: true,
            ..self.budget()
        }
    }

    fn current_extent(&self, producer: OperationId, trace: &Reaching) -> Checked<bool> {
        let (owner, _) = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
        Ok(!trace.crossed_entries[index(owner.get())])
    }

    /// The equation a read establishes on its normal successor:
    /// `read == v` when `v` is the one value its place holds on every path
    /// to it, else none.
    pub(crate) fn read_equation(
        &self,
        references: &mut DescriptorAnalysis<'a>,
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
        let mut budget = self.snapshot_budget();
        let reached = self.reaching(
            references,
            tables,
            Asked::Contents(&leaf),
            (block, before),
            &mut budget,
            concerns,
        )?;
        if let Reached::Extent(producer) = reached {
            return self.extent_equation(references, tables, operation, producer, &budget);
        }
        let Reached::Value(held) = reached else {
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
        // Opaque code is transported by the typed storage operation, not
        // by mathematical equality or a proof address literal.
        if matches!(held_type, ScalarType::Callable(_)) {
            return Ok(None);
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
        references: &mut DescriptorAnalysis<'a>,
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
            references,
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
    ///   it to a sibling cannot be stated on one read's value; it is known
    ///   instead of the values the record arrived with
    ///   (`Storage::entry_type_premises`), which the read equation reaches
    ///   while no store intervenes. A case
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        leaf: &Lineage,
        observed: (ValueId, ScalarType),
        at: (BlockId, usize),
        concerns: Construct,
    ) -> Checked<Vec<Proposition>> {
        let (block, before) = at;
        let (observed, scalar_type) = observed;
        // Follow the storage to the value that was established. Named
        // contents retain their original path. With no single name, the
        // trace retains only the part of the current path established on
        // every arrival, inside the value supplied by another machine.
        let mut budget = self.provenance_budget();
        let reached = self.reaching(
            references,
            tables,
            Asked::Contents(leaf),
            (block, before),
            &mut budget,
            concerns,
        )?;
        let shared = match budget.boundary {
            BoundaryDepth::Established(depth) => Some(depth),
            BoundaryDepth::Cycle | BoundaryDepth::Local => None,
        };
        let arrived = match reached {
            Reached::Established(depth) => leaf
                .segments
                .len()
                .checked_sub(depth)
                .map(|start| (leaf.clone(), start, depth)),
            Reached::Unknown => shared.and_then(|depth| {
                leaf.segments
                    .len()
                    .checked_sub(depth)
                    .map(|start| (leaf.clone(), start, depth))
            }),
            Reached::Value(held) => {
                match terminal_check_tables::row(&tables.values, held, self.machine, concerns)?
                    .origin
                {
                    ValueOrigin::StorageContent { place, after } => {
                        let arrived = lineage(self.machine, &tables.places, place)?;
                        let start = match after {
                            None if matches!(
                                self.root_kind(arrived.root, concerns)?,
                                StructuralPlaceKind::Parameter { .. }
                            ) =>
                            {
                                Some(0)
                            }
                            None => None,
                            Some(call) => Some(
                                self.call_content_boundary(call, &arrived, concerns)?
                                    .segments
                                    .len(),
                            ),
                        };
                        start
                            .zip(shared)
                            .map(|(start, shared)| (arrived, start, shared))
                    }
                    ValueOrigin::MachineParameter { .. }
                    | ValueOrigin::MachineResult
                    | ValueOrigin::BlockParameter { .. }
                    | ValueOrigin::Operation(_) => None,
                }
            }
            Reached::Case(_)
            | Reached::Extent(_)
            | Reached::Unestablished
            | Reached::Cycle
            | Reached::Open
            | Reached::Closed => None,
        };
        let Some((arrived, start, shared)) = arrived else {
            return Ok(Vec::new());
        };
        let mut premises = Vec::new();
        for length in (start..arrived.segments.len()).rev() {
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
            let selected = (beneath <= shared)
                .then(|| leaf.segments.len().checked_sub(beneath))
                .flatten()
                .map(|kept| Lineage {
                    root: leaf.root,
                    segments: leaf.segments[..kept].to_vec(),
                });
            for clause in self.clauses_of(structural_type, concerns)? {
                let mut bindings = Observed {
                    held: Held {
                        point: None,
                        storage: self,
                        references,
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        block: &Block,
    ) -> Checked<Vec<(EdgeId, Proposition)>> {
        let concerns = Construct::Block(block.id);
        let mut equations = Vec::new();
        for bound in self.bound_payloads(block)? {
            let mut budget = self.snapshot_budget();
            let Reached::Value(held) = self.reaching(
                references,
                tables,
                Asked::Contents(&bound.leaf),
                (block.id, usize::MAX),
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
            if matches!(held_type, ScalarType::Callable(_)) {
                continue;
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        block: &Block,
    ) -> Checked<Vec<(EdgeId, Lineage, Proposition)>> {
        let mut premises = Vec::new();
        for bound in self.bound_payloads(block)? {
            for premise in self.premises(
                references,
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
        if let Some(source) = self.moved_source(operation)? {
            return Ok(vec![source]);
        }
        match &operation.kind {
            OperationKind::AtomicAccess { place, path, .. } => {
                Ok(vec![self.stored_at(*place, path, concerns)?])
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
            }
            OperationKind::DecomposeStructuralValue { subject: source }
            | OperationKind::EstablishReference { source, .. } => {
                Ok(vec![self.argument(source, concerns)?])
            }
            OperationKind::ReleaseReference { source, path } => {
                Ok(vec![self.stored_at(*source, path, concerns)?])
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
            // A byte is read through the view's descriptor, so the whole
            // descriptor is read where its byte is.
            OperationKind::ByteSequenceRead { source, .. }
            | OperationKind::ByteSequenceWrite {
                destination: source,
                ..
            } => Ok(vec![self.stored_at(*source, &[], concerns)?]),
            OperationKind::StructuralByteSequenceFieldRead {
                source,
                path,
                field,
                ..
            } => Ok(vec![self.argument(
                &self.byte_field_argument(*source, path, *field, concerns)?,
                concerns,
            )?]),
            // Each compared view is read whole through its own descriptor.
            OperationKind::ByteSequenceEqual { left, right } => [left, right]
                .into_iter()
                .map(|operand| {
                    self.argument(&self.byte_operand_argument(operand, concerns)?, concerns)
                })
                .collect(),
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

    /// Both owned transport and an exclusive storage window read exactly the
    /// region they vacate. The same source drives the read check and write log.
    fn moved_source(&self, operation: &Operation) -> Checked<Option<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        match &operation.kind {
            OperationKind::MoveStructuralValue { source } => {
                self.argument(source, concerns).map(Some)
            }
            OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } => self
                .projected_storage(*source, path, *field, concerns)
                .map(Some),
            _ => Ok(None),
        }
    }

    fn projected_storage(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
        concerns: Construct,
    ) -> Checked<Lineage> {
        let mut selected = self.stored_at(root, path, concerns)?;
        if let Some(field) = field {
            selected.segments.push(Segment::Field(field));
            self.shape(&selected, concerns)?;
        }
        Ok(selected)
    }

    /// The explicit field destination, used to state that field's invariant.
    /// This is not an inventory of call effects or owned consumption.
    fn field_destination(&self, operation: &Operation) -> Checked<Option<Lineage>> {
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
        self.projected_storage(*destination, path, field, concerns)
            .map(Some)
    }

    /// Every storage change uses the already derived write log, including
    /// call writes and scalar homes. Owned operands also relinquish their
    /// storage; a live shared loan forbids that move just as it forbids a write.
    fn operation_changes(&self, operation: &Operation) -> Checked<Vec<StorageEffect>> {
        let concerns = Construct::Operation(operation.id);
        let payloads = self.call_payload_arguments(operation, true)?;
        let mut changes: Vec<_> = self
            .log
            .iter()
            .filter(|row| row.operation == operation.id)
            .map(|row| {
                if payloads.iter().any(|payload| {
                    row.captured.root == payload.root
                        && row.captured.segments.starts_with(&payload.segments)
                }) {
                    StorageEffect::Payload(row.captured.clone())
                } else {
                    StorageEffect::Storage(row.captured.clone())
                }
            })
            .collect();
        if let OperationKind::ByteSequenceWrite { destination, .. } = &operation.kind {
            // The payload changes even though the descriptor has no store
            // row. Retained loans protect that backing against this write.
            changes.push(StorageEffect::Payload(self.stored_at(
                *destination,
                &[],
                concerns,
            )?));
        }
        for argument in operation
            .kind
            .constructor_arguments()
            .chain(operation.kind.structural_arguments())
            .filter(|argument| argument.access == terminal_psi::StructuralAccess::Owned)
        {
            changes.push(StorageEffect::Storage(self.argument(argument, concerns)?));
        }
        if let OperationKind::DecomposeStructuralValue { subject } = &operation.kind {
            changes.push(StorageEffect::Storage(self.argument(subject, concerns)?));
        }
        if let OperationKind::StoreStructuralField { value, .. } = &operation.kind
            && value.access == terminal_psi::StructuralAccess::Owned
        {
            changes.push(StorageEffect::Storage(self.argument(value, concerns)?));
        }
        Ok(changes)
    }

    /// What a block's terminator reads and changes, judged against retained
    /// loans as an operation's effects are. A dispatch or an empty-domain
    /// observation reads its subject; an owned transfer on an edge or a
    /// return relinquishes its storage; a borrowed argument is read, and an
    /// exclusive one may also be changed by the callee. Cleanup actions are
    /// judged on their own rosters.
    fn terminator_effects(&self, block: &Block) -> Checked<(Vec<Lineage>, Vec<Lineage>)> {
        let mut reads = Vec::new();
        let mut changes = Vec::new();
        let mut transfer = |argument: &StructuralArgument, concerns: Construct| -> Checked<()> {
            let place = self.argument(argument, concerns)?;
            match argument.access {
                StructuralAccess::Owned => changes.push(place),
                StructuralAccess::SharedBorrow => reads.push(place),
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                    reads.push(place.clone());
                    changes.push(place);
                }
            }
            Ok(())
        };
        match &block.terminator {
            Terminator::StructuralCase { source, path, .. } => {
                reads.push(self.stored_at(*source, path, Construct::Block(block.id))?);
            }
            Terminator::EliminateEmpty { edge, source, path } => {
                reads.push(self.stored_at(*source, path, Construct::Edge(*edge))?);
            }
            Terminator::ReturnStructural { edge, source, .. } => transfer(
                &StructuralArgument::whole(*source, StructuralAccess::Owned),
                Construct::Edge(*edge),
            )?,
            Terminator::Jump {
                edge,
                structural_arguments,
                ..
            }
            | Terminator::InvokeNoNormal {
                edge,
                structural_arguments,
                ..
            }
            | Terminator::ExternalCompletion {
                edge,
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    transfer(argument, Construct::Edge(*edge))?;
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    for argument in &successor.structural_arguments {
                        transfer(argument, Construct::Edge(successor.edge))?;
                    }
                }
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => {}
        }
        Ok((reads, changes))
    }

    fn call_payload_arguments(&self, operation: &Operation, writes: bool) -> Checked<Vec<Lineage>> {
        let concerns = Construct::Operation(operation.id);
        let Some(call) = operation.declared_invocation() else {
            return Ok(Vec::new());
        };
        let formals = self
            .module
            .invocation_structural_parameters(call.callee, call.required_contract)
            .ok_or_else(|| self.foreign(concerns, "call effects have no exact retained formals"))?;
        if formals.len() != call.structural_arguments.len() {
            return Err(self.foreign(concerns, "call effects differ from their retained formals"));
        }
        let mut payloads = Vec::new();
        for (argument, formal) in call.structural_arguments.iter().zip(formals) {
            if argument.access != formal.access {
                return Err(self.foreign(
                    concerns,
                    "call effect access differs from its retained formal",
                ));
            }
            let touches = match argument.access {
                terminal_psi::StructuralAccess::SharedBorrow => !writes,
                terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow => true,
                terminal_psi::StructuralAccess::Owned => false,
            };
            if touches
                && matches!(
                    self.borrowed_resource(formal),
                    loans::BorrowedResource::ViewPayload
                )
            {
                payloads.push(self.argument(argument, concerns)?);
            }
        }
        Ok(payloads)
    }

    fn operation_read_effects(&self, operation: &Operation) -> Checked<Vec<StorageEffect>> {
        let payloads = self.call_payload_arguments(operation, false)?;
        let indexed = matches!(
            operation.kind,
            OperationKind::ByteSequenceRead { .. }
                | OperationKind::StructuralByteSequenceFieldRead { .. }
        );
        Ok(self
            .operation_uses(operation)?
            .into_iter()
            .map(|place| {
                if indexed || payloads.contains(&place) {
                    StorageEffect::Payload(place)
                } else {
                    StorageEffect::Storage(place)
                }
            })
            .collect())
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
    #[allow(clippy::too_many_arguments)]
    fn judged(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        used: &[Lineage],
        named: &[Lineage],
        (block, up_to): (BlockId, usize),
        point: ProgramPoint,
        concerns: Construct,
    ) -> Checked<Checked<()>> {
        for place in used {
            for leaf in self.leaves(place, concerns)? {
                let mut budget = self.initialization_budget();
                budget.used = Some(point);
                if self.reaching(
                    references,
                    tables,
                    Asked::Contents(&leaf),
                    (block, up_to),
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
                budget.used = Some(point);
                if self.reaching(
                    references,
                    tables,
                    Asked::Case(&sum),
                    (block, up_to),
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

    /// Judge primitive byte-domain establishment from the exact source.
    /// Cast edges, copies and moves owe their new qualifications at their
    /// producer, even when nothing later consumes the result. A field store owes the field's
    /// predicates, independently of its storage and loan obligations.
    /// Exact literal bytes, declared qualified inputs and supported descriptor
    /// producers supply predicates through the shared source-free owner.
    pub(crate) fn byte_predicate_establishments(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Checked<Vec<StorageUse>> {
        let mut verdicts = Vec::new();
        let changes =
            terminal_semantics::structural_qualifications::byte_qualification_change_judgments(
                module,
                self.lowered,
            )
            .map_err(|error| self.unsupported(Construct::Machine, error.detail))?;
        for (edge, established) in changes {
            if !established {
                let concerns = Construct::Edge(edge);
                verdicts.push(StorageUse {
                    at: concerns,
                    verdict: Err(refusal(
                        RefusalReason::Unestablished,
                        self.machine,
                        concerns,
                        "a byte transfer's source does not establish its primitive byte-domain \
                             qualifications",
                    )),
                });
            }
        }
        for block in &self.lowered.blocks {
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                let establishment = self.byte_literal_establishment(module, operation).and_then(
                    |()| match terminal_semantics::byte_transfer_qualifications_hold(
                        module,
                        self.lowered,
                        operation,
                    ) {
                        Ok(None | Some(true)) => Ok(()),
                        Ok(Some(false)) => Err(refusal(
                            RefusalReason::Unestablished,
                            self.machine,
                            concerns,
                            "a byte transfer's source does not establish its primitive byte-domain \
                                 qualifications",
                        )),
                        Err(reason) => Err(self.unsupported(concerns, reason)),
                    },
                );
                if let Err(refused) = establishment {
                    verdicts.push(StorageUse {
                        at: concerns,
                        verdict: Err(refused),
                    });
                    continue;
                }
                for (predicates, source) in self.predicate_view_sources(operation, concerns)? {
                    let supplied = terminal_semantics::byte_sequence_argument_predicates(
                        module,
                        self.lowered,
                        &source,
                    );
                    let verdict = match supplied {
                        Some(supplied) if supplied.implies(predicates) => Ok(()),
                        Some(_) => Err(refusal(
                            RefusalReason::Unestablished,
                            self.machine,
                            concerns,
                            "a byte view is stored into a field of a byte domain, and the bytes \
                             it shows are not in that domain",
                        )),
                        None => Err(self.unsupported(
                            concerns,
                            &format!(
                                "a byte view stored into a field of a byte domain has no \
                                 reconstructed source predicates: {source:?}",
                            ),
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

    fn byte_literal_establishment(
        &self,
        module: &terminal_psi::TerminalModule,
        operation: &Operation,
    ) -> Checked<()> {
        let OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes,
            qualifications,
        } = &operation.kind
        else {
            return Ok(());
        };
        let concerns = Construct::Operation(operation.id);
        let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = self.root_kind(*destination, concerns)?
        else {
            return Err(self.foreign(concerns, "a byte literal has no declared literal root"));
        };
        if terminal_semantics::byte_qualification_predicates_in(
            module.structural_domains.iter(),
            structural_type,
            qualifications,
        )
        .is_none()
        {
            return Err(self.unsupported(
                concerns,
                &format!(
                    "byte literal qualifications do not name exact pure primitive byte domains: \
                     carrier {structural_type:?}, domains {qualifications:?}",
                ),
            ));
        }
        if !terminal_semantics::byte_literal_qualifications_hold(
            &module.structural_domains,
            structural_type,
            qualifications,
            bytes,
        ) {
            return Err(refusal(
                RefusalReason::Unestablished,
                self.machine,
                concerns,
                "a byte literal's contents do not establish its primitive byte-domain qualifications",
            ));
        }
        Ok(())
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

    /// Resolve the predicate from the exact immutable descriptor source.
    /// Shared inputs remain symbolic entry requirements; a produced view may
    /// use only reconstructed source facts or its authenticated callee contract.
    fn byte_source(
        &self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        actual: &Lineage,
        concerns: Construct,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let mut memo = IdTable::from_rows(
            self.lowered
                .structural_places
                .iter()
                .map(|place| (place.id, BytePredicateState::Unseen)),
        )
        .map_err(|_| {
            crate::Unbound::Unsupported("byte source places are not dense in declaration order")
        })?;
        self.byte_source_with(predicate, actual, concerns, &mut memo)
    }

    fn byte_source_with(
        &self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        actual: &Lineage,
        concerns: Construct,
        memo: &mut IdTable<PlaceId, BytePredicateState>,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if !actual.segments.is_empty() {
            return Err(crate::Unbound::Unsupported(
                "a byte predicate of a view inside other storage",
            ));
        }
        let state = memo
            .get_mut(actual.root)
            .ok_or(crate::Unbound::Unsupported(
                "a byte source names no declared place",
            ))?;
        match state {
            BytePredicateState::Resolved(result) => return Ok(result.clone()),
            BytePredicateState::Visiting => {
                return Err(crate::Unbound::Unsupported(
                    "a byte predicate of a view copied in a cycle",
                ));
            }
            BytePredicateState::Unseen => *state = BytePredicateState::Visiting,
        }
        let refused = crate::Unbound::Refused;
        let place = actual.root;
        let result = match self.root_kind(place, concerns).map_err(refused)? {
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                let bytes = self
                    .literal_bytes(place, concerns)
                    .map_err(refused)?
                    .ok_or(crate::Unbound::Unsupported(
                        "a byte literal has no exact establishment",
                    ))?;
                ScalarTerm::Boolean(predicate.holds_for(bytes))
            }
            StructuralPlaceKind::Parameter { .. } => {
                let shared = self
                    .lowered
                    .structural_parameter(place)
                    .is_some_and(|formal| {
                        formal.access == StructuralAccess::SharedBorrow
                            && self
                                .module
                                .structural_type(formal.structural_type)
                                .is_some_and(|ty| ty.shape.is_shared_readable_byte_view())
                    });
                if !shared {
                    return Err(crate::Unbound::Unsupported(
                        "a byte predicate of an input that is not an immutable shared byte view",
                    ));
                }
                ScalarTerm::byte_sequence_predicate(predicate, place, Vec::new())
            }
            StructuralPlaceKind::BlockParameter { .. } => {
                let formal = terminal_semantics::arrival_parameter(self.lowered, place).ok_or(
                    crate::Unbound::Unsupported("a byte arrival has no exact formal"),
                )?;
                if !self
                    .module
                    .structural_type(formal.structural_type)
                    .is_some_and(|ty| ty.shape.is_shared_readable_byte_view())
                {
                    return Err(crate::Unbound::Unsupported(
                        "a byte arrival is not a shared view",
                    ));
                }
                // This is a goal term, not an established fact. Every incoming
                // edge proves the exact formal's predicates, including backedges.
                ScalarTerm::byte_sequence_predicate(predicate, place, Vec::new())
            }
            StructuralPlaceKind::OperationResult { producer, .. } => {
                let operation = self.operation(producer, concerns).map_err(refused)?;
                let source = match &operation.kind {
                    OperationKind::MoveStructuralValue { source } if source.path.is_empty() => {
                        Some(source.place)
                    }
                    OperationKind::StructuralLeafCopy { source, path } if path.is_empty() => {
                        Some(*source)
                    }
                    _ => None,
                };
                if let Some(source) = source {
                    self.byte_source_with(
                        predicate,
                        &Lineage {
                            root: source,
                            segments: Vec::new(),
                        },
                        concerns,
                        memo,
                    )?
                } else if matches!(
                    operation.kind,
                    terminal_psi::call_structural_pair_pattern!(..)
                        | OperationKind::ByteSequenceSubslice { .. }
                ) {
                    let mut required = terminal_psi::ByteSequencePredicateSet::EMPTY;
                    required.insert(predicate);
                    if terminal_semantics::byte_sequence_source_predicates(
                        self.module,
                        self.lowered,
                        place,
                    )
                    .is_some_and(|facts| facts.implies(required))
                    {
                        ScalarTerm::Boolean(true)
                    } else if matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                        && self
                            .module
                            .structural_type(self.root_type(place, concerns).map_err(refused)?)
                            .is_some_and(|ty| ty.shape.is_shared_readable_byte_view())
                    {
                        // The operation contributes only a conditional premise
                        // from its exact source; no annotation proves this term.
                        ScalarTerm::byte_sequence_predicate(predicate, place, Vec::new())
                    } else {
                        return Err(crate::Unbound::Unsupported(
                            "a byte result does not establish this predicate",
                        ));
                    }
                } else {
                    return Err(crate::Unbound::Unsupported(
                        "a byte result has no admitted predicate source",
                    ));
                }
            }
            _ => {
                return Err(crate::Unbound::Unsupported(
                    "a byte predicate has no established source",
                ));
            }
        };
        *memo.get_mut(place).ok_or(crate::Unbound::Unsupported(
            "a byte source lost its declared place",
        ))? = BytePredicateState::Resolved(result.clone());
        Ok(result)
    }

    /// A shared subslice inherits predicates that hold for every window of
    /// its source. Name the actual producer's result and keep the source
    /// condition in the proof so each loop arrival must establish it.
    pub(crate) fn subslice_predicate_premises(
        &self,
        operation: &Operation,
    ) -> Checked<Vec<Proposition>> {
        use semantic_vocabulary::ByteSequencePredicate;

        let OperationKind::ByteSequenceSubslice { source, .. } = &operation.kind else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Operation(operation.id);
        let result = operation
            .result
            .structural()
            .ok_or_else(|| self.foreign(concerns, "a byte subslice has no structural result"))?;
        if !self
            .declaration(result.structural_type, concerns)?
            .shape
            .is_shared_readable_byte_view()
        {
            return Ok(Vec::new());
        }
        let source = self.argument(source, concerns)?;
        let mut known = Vec::new();
        for predicate in ByteSequencePredicate::ALL {
            if !predicate.is_subslice_preserving() {
                continue;
            }
            let term = match self.byte_source(predicate, &source, concerns) {
                Ok(term) => term,
                Err(crate::Unbound::Refused(refusal)) => return Err(refusal),
                Err(_) => continue,
            };
            for inherited in ByteSequencePredicate::ALL {
                if predicate.implies(inherited) {
                    let destination =
                        ScalarTerm::byte_sequence_predicate(inherited, result.place, Vec::new());
                    known.push(Proposition::Implication {
                        premise: Box::new(Proposition::Equal(
                            term.clone(),
                            ScalarTerm::Boolean(true),
                        )),
                        conclusion: Box::new(Proposition::Equal(
                            destination,
                            ScalarTerm::Boolean(true),
                        )),
                    });
                }
            }
        }
        Ok(known)
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
    fn touches(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
    ) -> Checked<Vec<Touch<'a>>> {
        let mut touches = Vec::new();
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let used = self.operation_uses(operation)?;
                let mut named = used.clone();
                named.extend(self.field_destination(operation)?);
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
                    if !self
                        .read_premises(references, tables, operation)?
                        .is_empty()
                    {
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
                    cleanup: None,
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
            for (edge, actions) in block.terminator.cleanup_rosters() {
                for (ordinal, action) in actions.iter().enumerate() {
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) = action
                    {
                        let used = vec![self.argument(&cleanup.subject, Construct::Edge(edge))?];
                        touches.push(Touch {
                            cleanup: Some((edge, crate::position(ordinal, self.machine)?)),
                            at: Construct::Edge(edge),
                            block: block.id,
                            up_to: block.operations.len(),
                            named: used.clone(),
                            used,
                            relied: Vec::new(),
                            how: Touching::Whole,
                            judged: true,
                            releases: true,
                        });
                    }
                }
            }
            let (edge, used) = match &block.terminator {
                Terminator::EliminateEmpty { edge, source, path } => {
                    let at = Construct::Edge(*edge);
                    let subject = vec![self.stored_at(*source, path, at)?];
                    touches.push(Touch {
                        cleanup: None,
                        at,
                        block: block.id,
                        up_to: usize::MAX,
                        named: subject.clone(),
                        used: subject,
                        relied: Vec::new(),
                        how: Touching::Empty {
                            source: *source,
                            path,
                        },
                        judged: true,
                        releases: false,
                    });
                    continue;
                }
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
                    for (_, leaf, _) in self.payload_premises(references, tables, block)? {
                        if !relied.contains(&leaf) {
                            relied.push(leaf);
                        }
                    }
                    touches.push(Touch {
                        cleanup: None,
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
                cleanup: None,
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
    pub(crate) fn uses(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        empty_contracts: &terminal_semantics::inhabitation::TerminalContracts<'_>,
        empty_types: &terminal_semantics::inhabitation::Inhabitation,
    ) -> Checked<Vec<StorageUse>> {
        let mut uses = Vec::new();
        for touch in self.touches(references, tables)? {
            if !touch.judged {
                continue;
            }
            let mut judge = || {
                self.judged(
                    references,
                    tables,
                    &touch.used,
                    &touch.named,
                    (touch.block, touch.up_to),
                    self.point(&touch)?,
                    touch.at,
                )
            };
            let verdict = match (touch.how, touch.used.first()) {
                (Touching::Empty { source, path }, _) => {
                    let root_type = self.root_type(source, touch.at)?;
                    if self
                        .lowered
                        .structural_parameters
                        .iter()
                        .chain(self.lowered.block_structural_parameters())
                        .any(|parameter| {
                            parameter.place == source
                                && parameter.access
                                    == terminal_psi::StructuralAccess::WriteOnlyBorrow
                        })
                    {
                        Err(refusal(
                            RefusalReason::Unestablished,
                            self.machine,
                            touch.at,
                            "an empty observation cannot read write-only storage",
                        ))
                    } else if empty_types.declared_path_status(empty_contracts, root_type, path)
                        != terminal_semantics::inhabitation::Status::ProvedEmpty
                    {
                        Err(refusal(
                            RefusalReason::Unestablished,
                            self.machine,
                            touch.at,
                            "an empty observation has no proof that its actual subject is empty",
                        ))
                    } else {
                        judge()?
                    }
                }
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        place: &Lineage,
        by: OperationId,
        restored: &[OperationId],
        touch: &Touch<'_>,
    ) -> Checked<bool> {
        let mut budget = self.budget();
        budget.used = Some(self.point(touch)?);
        Ok(self.reaching(
            references,
            tables,
            Asked::Window {
                place,
                by,
                restored,
            },
            (touch.block, touch.up_to),
            &mut budget,
            touch.at,
        )? == Reached::Open)
    }

    /// Whether `window` is open just before `operation`.
    pub(crate) fn open_before(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        window: &Window,
        operation: OperationId,
    ) -> Checked<bool> {
        let concerns = Construct::Operation(operation);
        let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
        let before = after
            .checked_sub(1)
            .ok_or_else(|| self.foreign(concerns, "an operation has no point before it"))?;
        let operations = &self.block(block)?.operations;
        let mut budget = self.budget();
        budget.used = Some(
            before
                .checked_sub(1)
                .map_or(ProgramPoint::BlockEntry(block), |previous| {
                    ProgramPoint::AfterOperation(operations[previous].id)
                }),
        );
        Ok(self.reaching(
            references,
            tables,
            Asked::Window {
                place: &window.owner.place,
                by: window.opened_by,
                restored: &window.restored,
            },
            (block, before),
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
        references: &mut DescriptorAnalysis<'a>,
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
        if !aimed
            || !self.open_at(
                references,
                tables,
                place,
                window.opened_by,
                &window.restored,
                touch,
            )?
        {
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
                    && self.open_at(references, tables, &leaf, window.opened_by, &[], touch)?
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
        if let Some((edge, action_ordinal)) = touch.cleanup {
            return Ok(ProgramPoint::BeforeCleanup {
                edge,
                action_ordinal,
            });
        }
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
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        windows: &[Window],
    ) -> Checked<Vec<Consumption>> {
        let mut found = Vec::new();
        if windows.is_empty() {
            return Ok(found);
        }
        for touch in self.touches(references, tables)? {
            for (position, window) in windows.iter().enumerate() {
                if !self.consumes(references, tables, &touch, window)? {
                    continue;
                }
                found.push(Consumption {
                    window: position,
                    at: touch.at,
                    point: self.point(&touch)?,
                    goal: self.stated(
                        references,
                        tables,
                        &window.owner,
                        touch.block,
                        touch.up_to,
                        self.point(&touch)?,
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
    point: Option<ProgramPoint>,
    storage: &'s Storage<'a>,
    references: &'s mut DescriptorAnalysis<'a>,
    tables: &'s MachineTables,
    place: &'s Lineage,
    block: BlockId,
    up_to: usize,
    concerns: Construct,
}

/// Extend a clause's exact path. The storage walk independently checks each
/// field, case, index and referent step against the type reached by the
/// previous segment, so a clause through a stored reference names what the
/// reference actually lends there.
fn extended(
    mut place: Lineage,
    path: &[CanonicalStructuralPathSegment],
) -> Result<Lineage, crate::Unbound> {
    for segment in path {
        place.segments.push(match segment {
            CanonicalStructuralPathSegment::Field(field) => Segment::Field(*field),
            CanonicalStructuralPathSegment::Case(case) => Segment::Case(*case),
            CanonicalStructuralPathSegment::FixedIndex(position) => Segment::FixedIndex(*position),
            CanonicalStructuralPathSegment::Referent => Segment::Referent,
        });
    }
    Ok(place)
}

impl<'a> Held<'_, 'a> {
    fn at_place<'p>(&'p mut self, place: &'p Lineage) -> Held<'p, 'a> {
        Held {
            point: self.point,
            storage: self.storage,
            references: self.references,
            tables: self.tables,
            place,
            block: self.block,
            up_to: self.up_to,
            concerns: self.concerns,
        }
    }

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
/// interface. The result uses its actual returned storage, and borrowed inputs
/// use their captured storage at completion. Owned inputs and explicit entry
/// reads keep the input observation. All use the same checked storage walk.
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

/// Keep the actual place selected when the invocation's arguments were
/// evaluated. A completion read changes its observation point, never the
/// argument or the reference identity it follows.
pub(crate) trait InputPlaces<'a>: crate::Bindings<Error = crate::Unbound> {
    fn references(&mut self) -> &mut DescriptorAnalysis<'a>;
    fn input_place(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound>;
}

impl<'s, 'a, Inputs: InputPlaces<'a>> Returned<'s, 'a, Inputs> {
    fn formal_access(
        &self,
        root: PlaceId,
    ) -> Result<terminal_psi::StructuralAccess, crate::Unbound> {
        terminal_psi::StructuralParameterDeclaration::find_by_place(self.input_formals, root)
            .map(|formal| formal.access)
            .ok_or(crate::Unbound::Unsupported(
                "a completion clause names storage that is no formal",
            ))
    }

    fn owned_inputs(&mut self, root: PlaceId) -> Result<&mut Inputs, crate::Unbound> {
        if self.formal_access(root)? != terminal_psi::StructuralAccess::Owned {
            return Err(crate::Unbound::Unsupported(
                "borrowed completion storage has no post-call observation",
            ));
        }
        Ok(&mut self.inputs)
    }

    fn held(&mut self) -> Result<Held<'_, 'a>, crate::Unbound> {
        let (_, place) = self.structural.as_ref().ok_or(crate::Unbound::Unsupported(
            "no structural result correspondence",
        ))?;
        let (block, up_to) = self
            .storage
            .graph
            .locate(self.point)
            .map_err(crate::Unbound::Refused)?;
        Ok(Held {
            point: Some(self.point),
            storage: self.storage,
            references: self.inputs.references(),
            tables: self.tables,
            place,
            block,
            up_to,
            concerns: self.concerns,
        })
    }

    fn held_at<'p>(&'p mut self, place: &'p Lineage) -> Result<Held<'p, 'a>, crate::Unbound> {
        let (block, up_to) = self
            .storage
            .graph
            .locate(self.point)
            .map_err(crate::Unbound::Refused)?;
        Ok(Held {
            point: Some(self.point),
            storage: self.storage,
            references: self.inputs.references(),
            tables: self.tables,
            place,
            block,
            up_to,
            concerns: self.concerns,
        })
    }
}

impl<'a, Inputs: InputPlaces<'a>> crate::Bindings for Returned<'_, 'a, Inputs> {
    type Error = crate::Unbound;
    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Self::Error> {
        self.storage.validate_float_meaning_source(source)
    }

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
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if self
            .structural
            .as_ref()
            .is_some_and(|(declared, _)| *declared == root)
        {
            if version == semantic_vocabulary::ContentPlaceVersion::Entry {
                return Err(crate::Unbound::Unsupported(
                    "a result has no invocation-entry contents",
                ));
            }
            let subject = terminal_check_tables::program::self_place()
                .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
            return self.held()?.field(version, subject, path, scalar_type);
        }
        let access = self.formal_access(root)?;
        if !terminal_psi::structural_access_covers(
            access,
            terminal_psi::StructuralAccess::SharedBorrow,
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause reads a write-only input",
            ));
        }
        if version == semantic_vocabulary::ContentPlaceVersion::Entry
            || access == terminal_psi::StructuralAccess::Owned
        {
            return self.inputs.field(version, root, path, scalar_type);
        }
        let actual = self.inputs.input_place(root, path)?;
        let subject = terminal_check_tables::program::self_place()
            .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
        self.held_at(&actual)?.field(
            semantic_vocabulary::ContentPlaceVersion::Current,
            subject,
            &[],
            scalar_type,
        )
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
        if self.formal_access(root)? == terminal_psi::StructuralAccess::Owned {
            return self.inputs.extent(root, path, scalar_type);
        }
        let actual = self.inputs.input_place(root, path)?;
        self.held_at(&actual)?.extent_of(&actual, scalar_type)
    }
    /// A shared input view shows at completion the bytes it showed at entry:
    /// nothing writes through a shared borrow.
    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, crate::Unbound> {
        if self
            .structural
            .as_ref()
            .is_some_and(|(declared, _)| *declared == root)
        {
            let subject = terminal_check_tables::program::self_place()
                .ok_or(crate::Unbound::Unsupported("data subject identity"))?;
            return self.held()?.byte_predicate(predicate, subject, path);
        }
        if self.formal_access(root)? != terminal_psi::StructuralAccess::SharedBorrow {
            return Err(crate::Unbound::Unsupported(
                "a byte predicate at completion of other than a shared input view",
            ));
        }
        self.inputs.byte_predicate(predicate, root, path)
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
    references: &'s mut DescriptorAnalysis<'a>,
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
        _: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.observation(self.input_place(root, path)?, scalar_type)
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

    /// The machine's own shared input view keeps its entry bytes, so the
    /// predicate of them stays the term over that input.
    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.input_place(root, path)?;
        Ok(ScalarTerm::byte_sequence_predicate(
            predicate,
            root,
            path.to_vec(),
        ))
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

impl<'a> InputPlaces<'a> for AtEntry<'_, 'a> {
    fn references(&mut self) -> &mut DescriptorAnalysis<'a> {
        self.references
    }

    fn input_place(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        let formal = terminal_psi::StructuralParameterDeclaration::find_by_place(
            &self.storage.lowered.structural_parameters,
            root,
        )
        .ok_or(crate::Unbound::Unsupported(
            "a clause names storage that is no input",
        ))?;
        if !terminal_psi::structural_access_covers(
            formal.access,
            terminal_psi::StructuralAccess::SharedBorrow,
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause reads a write-only input",
            ));
        }
        extended(
            Lineage {
                root,
                segments: Vec::new(),
            },
            path,
        )
    }
}

/// See `Storage::at_call`.
pub(crate) struct AtCall<'s, 'a> {
    storage: &'s Storage<'a>,
    references: &'s mut DescriptorAnalysis<'a>,
    tables: &'s MachineTables,
    scalars: Vec<ValueId>,
    /// The record actuals, in the callee's structural lane order.
    places: Vec<Lineage>,
    callee_formals: &'s [terminal_psi::StructuralParameterDeclaration],
    block: BlockId,
    up_to: usize,
    concerns: Construct,
}

impl<'a> AtCall<'_, 'a> {
    fn actual(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        // The callee's root is one of its record formals; the actual is the
        // place at the same position of the structural lane.
        let formal = self
            .callee_formals
            .iter()
            .find(|parameter| parameter.place == root)
            .ok_or(crate::Unbound::Unsupported(
                "clause names storage that is no record formal of the callee",
            ))?;
        extended(self.readable_actual(formal)?.clone(), path)
    }

    fn readable_actual(
        &self,
        formal: &terminal_psi::StructuralParameterDeclaration,
    ) -> Result<&Lineage, crate::Unbound> {
        if !terminal_psi::structural_access_covers(
            formal.access,
            terminal_psi::StructuralAccess::SharedBorrow,
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause reads a write-only input",
            ));
        }
        usize::try_from(formal.position)
            .ok()
            .and_then(|position| self.places.get(position))
            .ok_or(crate::Unbound::Unsupported(
                "clause names storage that is no record formal of the callee",
            ))
    }

    pub(crate) fn observations(&mut self) -> CallObservations<'_, 'a> {
        let mut readable = Vec::new();
        for (position, formal) in self.callee_formals.iter().enumerate() {
            let Ok(actual) = self.readable_actual(formal) else {
                continue;
            };
            let mut place = actual.clone();
            for _ in 0..=self.storage.lowered.structural_places.len() {
                let source = self.transfer_source(&place);
                readable.push((place, position));
                let Some(source) = source else {
                    break;
                };
                place = source;
            }
        }
        readable.sort_unstable();
        CallObservations {
            storage: self.storage,
            references: self.references,
            tables: self.tables,
            callee_formals: self.callee_formals,
            block: self.block,
            up_to: self.up_to,
            concerns: self.concerns,
            readable,
            places: &self.places,
            extents: None,
            projections: None,
            terms: Vec::new(),
        }
    }

    /// Find the source named by the operation that established this actual.
    /// This only nominates a formal path. Its current contents are still
    /// checked at the actual, so copies do not carry facts across overwrites.
    fn transfer_source(&self, place: &Lineage) -> Option<Lineage> {
        let storage = self.storage;
        let StructuralPlaceKind::OperationResult { producer, .. } =
            storage.root_kind(place.root, self.concerns).ok()?
        else {
            return None;
        };
        let point = storage
            .graph
            .locate(ProgramPoint::AfterOperation(producer))
            .ok()?;
        let start = storage
            .log
            .partition_point(|row| (row.block, row.position) < point);
        storage.log[start..]
            .iter()
            .take_while(|row| (row.block, row.position) == point)
            .find_map(|row| {
                if row.lineage.root != place.root || !row.lineage.segments.is_empty() {
                    return None;
                }
                let Stored::Moved(source) = &row.stored else {
                    return None;
                };
                let mut source = source.clone();
                source.segments.extend_from_slice(&place.segments);
                (source != *place).then_some(source)
            })
    }

    fn held_at<'p>(&'p mut self, place: &'p Lineage) -> Held<'p, 'a> {
        Held {
            point: None,
            storage: self.storage,
            references: self.references,
            tables: self.tables,
            place,
            block: self.block,
            up_to: self.up_to,
            concerns: self.concerns,
        }
    }
}

/// Inference may encounter the same value in several facts. Its binding is
/// fixed for this call point, including observations that have no formal name.
pub(crate) struct CallObservations<'s, 'a> {
    storage: &'s Storage<'a>,
    references: &'s mut DescriptorAnalysis<'a>,
    tables: &'s MachineTables,
    callee_formals: &'s [terminal_psi::StructuralParameterDeclaration],
    block: BlockId,
    up_to: usize,
    concerns: Construct,
    readable: Vec<(Lineage, usize)>,
    places: &'s [Lineage],
    extents: Option<Vec<(Lineage, usize, ValueId)>>,
    // Keep unvisited and unbound distinct. Only named observations keep a term.
    projections: Option<IdTable<ValueId, Option<Option<usize>>>>,
    terms: Vec<ScalarTerm>,
}

impl<'a> CallObservations<'_, 'a> {
    fn readable_actual(
        &self,
        formal: &terminal_psi::StructuralParameterDeclaration,
    ) -> Result<&Lineage, crate::Unbound> {
        if !terminal_psi::structural_access_covers(
            formal.access,
            terminal_psi::StructuralAccess::SharedBorrow,
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause reads a write-only input",
            ));
        }
        usize::try_from(formal.position)
            .ok()
            .and_then(|position| self.places.get(position))
            .ok_or(crate::Unbound::Unsupported(
                "clause names storage that is no record formal of the callee",
            ))
    }

    fn observed_place(&mut self, origin: ValueOrigin) -> Option<Lineage> {
        match origin {
            ValueOrigin::StorageContent { place, .. } => {
                lineage(self.storage.machine, &self.tables.places, place).ok()
            }
            ValueOrigin::Operation(operation) => {
                let operation = self.storage.operation(operation, self.concerns).ok()?;
                let leaf = self.storage.read_leaf(operation).ok()??;
                let (block, after) = self
                    .storage
                    .graph
                    .locate(ProgramPoint::AfterOperation(operation.id))
                    .ok()?;
                self.storage
                    .backing(
                        self.references,
                        &leaf,
                        block,
                        after.checked_sub(1)?,
                        self.concerns,
                    )
                    .ok()
            }
            _ => None,
        }
    }

    fn held_at<'p>(&'p mut self, place: &'p Lineage) -> Held<'p, 'a> {
        Held {
            point: None,
            storage: self.storage,
            references: self.references,
            tables: self.tables,
            place,
            block: self.block,
            up_to: self.up_to,
            concerns: self.concerns,
        }
    }

    pub(crate) fn formal_observation(
        &mut self,
        value: ValueId,
        scalar_type: ScalarType,
        same: impl FnOnce(ScalarTerm, ScalarTerm) -> bool,
    ) -> Option<ScalarTerm> {
        if self.readable.is_empty() || matches!(scalar_type, ScalarType::Callable(_)) {
            return None;
        }
        let row = self.tables.values.get(value)?;
        if row.scalar_type != scalar_type {
            return None;
        }
        if self.projections.is_none() {
            self.projections = Some(self.tables.values.map(|_, _| None));
        }
        if let Some(projection) = *self.projections.as_ref()?.get(value)? {
            return projection.map(|index| self.terms[index].clone());
        }
        let origin = row.origin;
        let term = self.bind_observation(value, scalar_type, origin, same);
        let projection = term.as_ref().map(|term| {
            let index = self.terms.len();
            self.terms.push(term.clone());
            index
        });
        *self.projections.as_mut()?.get_mut(value)? = Some(projection);
        term
    }

    fn bind_observation(
        &mut self,
        value: ValueId,
        scalar_type: ScalarType,
        origin: ValueOrigin,
        same: impl FnOnce(ScalarTerm, ScalarTerm) -> bool,
    ) -> Option<ScalarTerm> {
        let recorded = self.observed_place(origin)?;
        let (formal, path, held) =
            if let Some((formal, path)) = formal_observation_path(&self.readable, &recorded) {
                let declaration = &self.callee_formals[formal];
                let mut actual =
                    extended(self.readable_actual(declaration).ok()?.clone(), &path).ok()?;
                if matches!(recorded.segments.last(), Some(Segment::Extent)) {
                    actual.segments.push(Segment::Extent);
                }
                let held = self
                    .held_at(&actual)
                    .observation(&actual, scalar_type)
                    .ok()?;
                (formal, path, held)
            } else if let (Some(Segment::Extent), ScalarType::Integer(integer)) =
                (recorded.segments.last(), scalar_type)
            {
                // Reborrowed descriptors must carry the reaching extent value.
                if self.extents.is_none() {
                    let mut extents = Vec::new();
                    let formals = self.callee_formals;
                    for (formal, declaration) in formals.iter().enumerate() {
                        let Some(actual) = self.readable_actual(declaration).ok().cloned() else {
                            continue;
                        };
                        let Some(ScalarTerm::Value { id, .. }) =
                            self.held_at(&actual).extent_of(&actual, integer).ok()
                        else {
                            continue;
                        };
                        let Some(origin) = self.tables.values.get(id).map(|row| row.origin) else {
                            continue;
                        };
                        let Some(source) = self.observed_place(origin) else {
                            continue;
                        };
                        if matches!(source.segments.last(), Some(Segment::Extent)) {
                            extents.push((source, formal, id));
                        }
                    }
                    extents.sort_unstable();
                    self.extents = Some(extents);
                }
                let extents = self.extents.as_ref()?;
                let position = extents.partition_point(|(place, _, _)| place < &recorded);
                let (place, formal, held) = extents.get(position)?;
                if *place != recorded {
                    return None;
                }
                (*formal, Vec::new(), ScalarTerm::value(*held, scalar_type))
            } else {
                return None;
            };
        if !same(held, ScalarTerm::value(value, scalar_type)) {
            return None;
        }
        let root = self.callee_formals[formal].place;
        let version = semantic_vocabulary::ContentPlaceVersion::Entry;
        Some(match (recorded.segments.last(), scalar_type) {
            (_, ScalarType::Callable(_)) => return None,
            (Some(Segment::Extent), ScalarType::Integer(scalar_type)) => ScalarTerm::ViewExtent {
                root,
                path,
                scalar_type,
            },
            (Some(Segment::Extent), _) => return None,
            (_, ScalarType::Boolean) => ScalarTerm::BooleanField {
                version,
                root,
                path,
            },
            (_, ScalarType::Integer(scalar_type)) => ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            },
            (_, ScalarType::IeeeFloat(format)) => ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            },
        })
    }
}

/// Only exact prefixes of this observation can name it. If several actuals
/// match, keep the first formal in declaration order, even across prefixes.
fn formal_observation_path(
    readable: &[(impl std::borrow::Borrow<Lineage>, usize)],
    recorded: &Lineage,
) -> Option<(usize, Vec<CanonicalStructuralPathSegment>)> {
    // A length is observed on the descriptor. Its formal term names that
    // descriptor, so the final extent marker is not a path projection.
    let subject = match recorded.segments.split_last() {
        Some((Segment::Extent, descriptor)) => descriptor,
        _ => &recorded.segments,
    };
    let segment = |segment: &Segment| match segment {
        Segment::Field(field) => Some(CanonicalStructuralPathSegment::Field(*field)),
        Segment::Case(case) => Some(CanonicalStructuralPathSegment::Case(*case)),
        Segment::FixedIndex(index) => Some(CanonicalStructuralPathSegment::FixedIndex(*index)),
        Segment::Referent => Some(CanonicalStructuralPathSegment::Referent),
        _ => None,
    };
    // Any segment unavailable in a contract must lie inside the actual.
    let shortest = subject
        .iter()
        .rposition(|step| segment(step).is_none())
        .map_or(0, |position| position + 1);
    let (formal, prefix) = (shortest..=subject.len())
        .filter_map(|length| {
            let prefix = (recorded.root, &subject[..length]);
            let first = readable.partition_point(|(actual, _)| {
                let actual = actual.borrow();
                (actual.root, actual.segments.as_slice()) < prefix
            });
            let (actual, formal) = readable.get(first)?;
            let actual = actual.borrow();
            ((actual.root, actual.segments.as_slice()) == prefix).then_some((*formal, length))
        })
        .min()?;
    let path = subject[prefix..]
        .iter()
        .map(segment)
        .collect::<Option<Vec<_>>>()?;
    Some((formal, path))
}

impl crate::Bindings for AtCall<'_, '_> {
    type Error = crate::Unbound;
    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Self::Error> {
        self.storage.validate_float_meaning_source(source)
    }

    fn value(
        &mut self,
        id: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        crate::scalar_actual(&self.scalars, self.tables, id, scalar_type)
    }

    fn field(
        &mut self,
        _: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let actual = self.actual(root, path)?;
        self.held_at(&actual).observation(&actual, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let actual = self.actual(root, path)?;
        self.held_at(&actual).extent_of(&actual, scalar_type)
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, crate::Unbound> {
        let actual = self.actual(root, path)?;
        self.storage.byte_source(predicate, &actual, self.concerns)
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

/// Block contracts use the same captured storage reads as call contracts. Their
/// scalar names are canonical IDs in this machine, rather than a callee lane.
pub(crate) struct AtJoin<'s, 'a> {
    inputs: AtCall<'s, 'a>,
    renaming: &'s [(ValueId, ValueId)],
}

impl<'a> AtJoin<'_, 'a> {
    pub(crate) fn observations(&mut self) -> CallObservations<'_, 'a> {
        self.inputs.observations()
    }
}

impl crate::Bindings for AtJoin<'_, '_> {
    type Error = crate::Unbound;

    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Self::Error> {
        self.inputs.storage.validate_float_meaning_source(source)
    }

    fn value(
        &mut self,
        value: ValueId,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let value = self
            .renaming
            .iter()
            .find(|(from, _)| *from == value)
            .map_or(value, |(_, to)| *to);
        let actual = terminal_check_tables::row(
            &self.inputs.tables.values,
            value,
            self.inputs.storage.machine,
            self.inputs.concerns,
        )
        .map_err(crate::Unbound::Refused)?;
        if actual.scalar_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "join argument has another scalar carrier",
            ));
        }
        if let ValueOrigin::StorageContent { place, after: None } = actual.origin {
            let observed = lineage(
                self.inputs.storage.machine,
                &self.inputs.tables.places,
                place,
            )
            .map_err(crate::Unbound::Refused)?;
            if let Some(formal) = terminal_psi::StructuralParameterDeclaration::find_by_place(
                self.inputs.callee_formals,
                observed.root,
            ) {
                // Each incoming edge proves the destination predicate about
                // its own actual. It may not use the destination's snapshot
                // as if that binding already existed on the edge.
                let mut leaf = self.inputs.readable_actual(formal)?.clone();
                if let Some((Segment::Extent, parent)) = observed.segments.split_last()
                    && let ScalarType::Integer(integer) = scalar_type
                {
                    leaf.segments.extend_from_slice(parent);
                    return self.inputs.held_at(&leaf).extent_of(&leaf, integer);
                }
                leaf.segments.extend_from_slice(&observed.segments);
                return self.inputs.held_at(&leaf).observation(&leaf, scalar_type);
            }
        }
        Ok(ScalarTerm::value(value, scalar_type))
    }

    fn field(
        &mut self,
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        if self
            .inputs
            .callee_formals
            .iter()
            .any(|formal| formal.place == root)
        {
            return crate::Bindings::field(&mut self.inputs, version, root, path, scalar_type);
        }
        // A subject outside the destination header keeps its existing
        // storage version. It is not a callee input to reread at this edge.
        Ok(match scalar_type {
            ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                version,
                root,
                path: path.to_vec(),
                scalar_type,
            },
            ScalarType::Boolean => ScalarTerm::BooleanField {
                version,
                root,
                path: path.to_vec(),
            },
            ScalarType::IeeeFloat(_) => {
                return Err(crate::Unbound::Unsupported(
                    "join field has no scalar observation",
                ));
            }
            ScalarType::Callable(_) => {
                return Err(crate::Unbound::Unsupported(
                    "an opaque callable value is not a mathematical clause term",
                ));
            }
        })
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        if self
            .inputs
            .callee_formals
            .iter()
            .any(|formal| formal.place == root)
        {
            crate::Bindings::extent(&mut self.inputs, root, path, scalar_type)
        } else {
            Ok(ScalarTerm::ViewExtent {
                root,
                path: path.to_vec(),
                scalar_type,
            })
        }
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, Self::Error> {
        if self
            .inputs
            .callee_formals
            .iter()
            .any(|formal| formal.place == root)
        {
            crate::Bindings::byte_predicate(&mut self.inputs, predicate, root, path)
        } else {
            Ok(ScalarTerm::byte_sequence_predicate(
                predicate,
                root,
                path.to_vec(),
            ))
        }
    }

    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        let actual = self.inputs.actual(root, path)?;
        crate::Bindings::case(
            &mut self.inputs.held_at(&actual),
            terminal_check_tables::program::self_place().ok_or(crate::Unbound::Unsupported(
                "data clause has no self coordinate",
            ))?,
            &[],
            case,
        )
    }

    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Self::Error> {
        if self
            .inputs
            .callee_formals
            .iter()
            .any(|formal| formal.place == root)
        {
            self.case(root, path, case).map(|holds| {
                if holds {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                }
            })
        } else {
            Ok(Proposition::StructuralCaseMembership {
                subject: semantic_vocabulary::StructuralCaseSubject::new(root, path.to_vec()),
                case,
            })
        }
    }
}

impl<'a> InputPlaces<'a> for AtCall<'_, 'a> {
    fn references(&mut self) -> &mut DescriptorAnalysis<'a> {
        self.references
    }

    fn input_place(
        &self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<Lineage, crate::Unbound> {
        self.actual(root, path)
    }
}

impl Held<'_, '_> {
    /// Scalar fields and descriptor lengths both use the contents reaching
    /// this observation point. A descriptor producer can supply its exact
    /// endpoint term without a new runtime measurement.
    fn observation(
        &mut self,
        leaf: &Lineage,
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        let storage = self.storage;
        if !matches!(
            storage
                .shape(leaf, self.concerns)
                .map_err(crate::Unbound::Refused)?,
            Shape::Scalar(declared) if declared == scalar_type
        ) {
            return Err(crate::Unbound::Unsupported(
                "a clause names storage with another scalar type",
            ));
        }
        let mut budget = storage.snapshot_budget();
        budget.used = self.point;
        let reached = storage
            .reaching(
                self.references,
                self.tables,
                Asked::Contents(leaf),
                (self.block, self.up_to),
                &mut budget,
                self.concerns,
            )
            .map_err(crate::Unbound::Refused)?;
        if let Reached::Extent(producer) = reached {
            let block = storage.block(self.block).map_err(crate::Unbound::Refused)?;
            let point = self
                .up_to
                .min(block.operations.len())
                .checked_sub(1)
                .map_or(ProgramPoint::BlockEntry(self.block), |previous| {
                    ProgramPoint::AfterOperation(block.operations[previous].id)
                });
            return storage
                .extent_contents(
                    self.references,
                    self.tables,
                    producer,
                    point,
                    scalar_type,
                    &budget,
                )
                .map_err(crate::Unbound::Refused)?
                .ok_or(crate::Unbound::Unproved(
                    "the descriptor's extent has no current endpoint correspondence",
                ));
        }
        let Reached::Value(held) = reached else {
            return Err(crate::Unbound::Unproved(
                "the clause's observation has no reaching value identity",
            ));
        };
        let held_type =
            terminal_check_tables::row(&self.tables.values, held, storage.machine, self.concerns)
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
        if held_type != scalar_type {
            return Err(crate::Unbound::Unsupported(
                "a clause's observation holds a value of another type than its term",
            ));
        }
        Ok(ScalarTerm::value(held, scalar_type))
    }

    fn extent_of(
        &mut self,
        place: &Lineage,
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        // A fixed array lends its declared extent. This is a type fact,
        // independent of its contents; loan and initialization checks still
        // decide whether the actual may supply the view.
        if let Shape::Typed(array) = self
            .storage
            .shape(place, self.concerns)
            .map_err(crate::Unbound::Refused)?
            && let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = self
                .storage
                .declaration(array, self.concerns)
                .map_err(crate::Unbound::Refused)?
                .shape
        {
            if scalar_type
                != semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .expect("u64 is valid")
            {
                return Err(crate::Unbound::Unsupported(
                    "an array view extent that is not a u64",
                ));
            }
            return ScalarTerm::integer(
                scalar_type,
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)),
            )
            .map_err(|error| {
                crate::Unbound::Refused(self.storage.foreign(
                    self.concerns,
                    &format!("invalid fixed-array extent: {error:?}"),
                ))
            });
        }
        let mut leaf = place.clone();
        leaf.segments.push(Segment::Extent);
        self.observation(&leaf, ScalarType::Integer(scalar_type))
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
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if version != semantic_vocabulary::ContentPlaceVersion::Current {
            return Err(crate::Unbound::Unsupported(
                "a data predicate has no invocation-entry contents",
            ));
        }
        self.observation(&self.beneath(root, path)?, scalar_type)
    }

    fn extent(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.extent_of(&self.beneath(root, path)?, scalar_type)
    }

    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<ScalarTerm, crate::Unbound> {
        let actual = self.beneath(root, path)?;
        self.storage.byte_source(predicate, &actual, self.concerns)
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
        budget.used = self.point;
        match storage
            .reaching(
                self.references,
                self.tables,
                Asked::Case(&sum),
                (self.block, self.up_to),
                &mut budget,
                self.concerns,
            )
            .map_err(crate::Unbound::Refused)?
        {
            Reached::Case(held) => Ok(held == case),
            Reached::Value(_)
            | Reached::Extent(_)
            | Reached::Established(_)
            | Reached::Unknown
            | Reached::Unestablished
            | Reached::Cycle
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
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if version != semantic_vocabulary::ContentPlaceVersion::Current {
            return Err(crate::Unbound::Unsupported(
                "a data predicate has no invocation-entry contents",
            ));
        }
        self.beneath(root, path)?;
        match scalar_type {
            ScalarType::Integer(scalar_type) => Ok(ScalarTerm::IntegerField {
                version,
                root,
                path: path.to_vec(),
                scalar_type,
            }),
            ScalarType::Boolean => Ok(ScalarTerm::BooleanField {
                version,
                root,
                path: path.to_vec(),
            }),
            ScalarType::IeeeFloat(_) => Err(crate::Unbound::Unsupported(
                "a clause term outside the scalar subset",
            )),
            ScalarType::Callable(_) => Err(crate::Unbound::Unsupported(
                "an opaque callable value is not a mathematical clause term",
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
        version: semantic_vocabulary::ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if version != semantic_vocabulary::ContentPlaceVersion::Current {
            return Err(crate::Unbound::Unsupported(
                "a data predicate has no invocation-entry contents",
            ));
        }
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
        self.held.at_place(place).case(root, path, case)
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use semantic_vocabulary::StructuralFieldId;

    #[test]
    fn call_observation_keeps_the_first_readable_formal_across_exact_prefixes() {
        let root = PlaceId::new(1).unwrap();
        let field = StructuralFieldId::new(2).unwrap();
        let sibling = StructuralFieldId::new(3).unwrap();
        let at = |root, segments| Lineage { root, segments };
        let whole = at(root, vec![]);
        let narrowed = at(root, vec![Segment::Field(field)]);
        let disjoint = at(root, vec![Segment::Field(sibling)]);
        let foreign = at(PlaceId::new(2).unwrap(), vec![]);
        let recorded = at(root, vec![Segment::Field(field), Segment::FixedIndex(4)]);

        let mut readable = [
            (&disjoint, 0),
            (&foreign, 1),
            (&whole, 4),
            (&narrowed, 3),
            (&narrowed, 2),
        ];
        readable.sort_unstable();
        assert_eq!(
            formal_observation_path(&readable, &recorded),
            Some((2, vec![CanonicalStructuralPathSegment::FixedIndex(4)]))
        );
        let readable = [(&whole, 1), (&narrowed, 2)];
        assert_eq!(
            formal_observation_path(&readable, &recorded),
            Some((
                1,
                vec![
                    CanonicalStructuralPathSegment::Field(field),
                    CanonicalStructuralPathSegment::FixedIndex(4)
                ]
            ))
        );
        assert_eq!(
            formal_observation_path(&[(&disjoint, 0), (&foreign, 1)], &recorded),
            None
        );
    }

    #[test]
    fn a_runtime_index_can_name_an_actual_but_cannot_enter_a_formal_path() {
        let root = PlaceId::new(1).unwrap();
        let selector = ValueId::new(1).unwrap();
        let whole = Lineage {
            root,
            segments: Vec::new(),
        };
        let actual = Lineage {
            root,
            segments: vec![Segment::RuntimeIndex { selector }],
        };
        let mut recorded = actual.clone();
        recorded.segments.push(Segment::FixedIndex(4));

        assert_eq!(formal_observation_path(&[(&whole, 0)], &recorded), None);
        assert_eq!(
            formal_observation_path(&[(&whole, 0), (&actual, 1)], &recorded),
            Some((1, vec![CanonicalStructuralPathSegment::FixedIndex(4)]))
        );
    }

    #[test]
    fn a_copy_trace_scans_its_blocks_and_preserves_every_arrival() {
        let mut source = String::from("data Point { x: u32; y: u32; }\n");
        let count = 12;
        for step in 0..count {
            let next = step + 1;
            source.push_str(&format!(
                "machine hop{step}(p: Point, choose: bool) -> u32 {{\n\
                 transition choose {{ true -> left{step}(p, choose) false -> right{step}(p, choose) }}\n\
                 }}\n\
                 machine left{step}(p: Point, choose: bool) -> u32 {{ hop{next}(p, choose) }}\n\
                 machine right{step}(p: Point, choose: bool) -> u32 {{ hop{next}(p, choose) }}\n"
            ));
        }
        source.push_str(&format!("machine hop{count}(p: Point, choose: bool) -> u32 {{ transition choose {{ true -> p.x false -> hop0(p, choose) }} }}\n"));
        let typed = crate::tests::rankings::typed_with_ranking_catalog(&source);
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "hop0")
            .unwrap()
            .symbol;
        let prepared = typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program;
        let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) = prepared
        else {
            panic!("reconvergence must prepare");
        };
        let input = prepared
            .closure
            .unwrap_or_else(|error| panic!("{}", error.detail));
        let module = &input.module;
        let machine = module
            .machines
            .iter()
            .find(|machine| {
                machine.operations().any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::ScalarStructuralField { .. }
                            | OperationKind::PrimitiveScalarRead { .. }
                    )
                })
            })
            .unwrap();
        let tables = input.tables.machines.get(machine.id).unwrap();
        let graph = Graph::new(machine.id, machine, tables).unwrap();
        let types = tables.values.map(|_, row| row.scalar_type);
        let descriptor_graph = reference_graph(module, machine.id).unwrap();
        let mut references = DescriptorAnalysis::with_borrowed_storage_slots(
            &module.structural_types,
            &descriptor_graph,
        );
        let mut storage = Storage::new(
            machine.id,
            machine,
            module,
            &input.domains,
            &graph,
            tables,
            &types,
            &mut references,
        )
        .unwrap();
        let operation = machine
            .operations()
            .find(|operation| storage.read_leaf(operation).unwrap().is_some())
            .expect("final field read");
        let leaf = storage.read_leaf(operation).unwrap().unwrap();
        let (block, after) = graph
            .locate(ProgramPoint::AfterOperation(operation.id))
            .unwrap();
        let mut trace = storage.budget();
        let reached = storage
            .reaching(
                &mut references,
                tables,
                Asked::Contents(&leaf),
                (block, after - 1),
                &mut trace,
                Construct::Operation(operation.id),
            )
            .unwrap();
        assert_eq!(reached, Reached::Established(1));
        let row_budget = storage.log.len() * graph.block_count();
        assert!(
            trace.scanned_rows <= row_budget,
            "exact contents revisited reconverging copy equations: {} rows, budget {row_budget}",
            trace.scanned_rows,
        );

        // Initialization must check every incoming value, but reconverging
        // copies must not multiply the work with every alternative path.
        let mut initialization = storage.initialization_budget();
        let initialized = storage
            .reaching(
                &mut references,
                tables,
                Asked::Contents(&leaf),
                (block, after - 1),
                &mut initialization,
                Construct::Operation(operation.id),
            )
            .unwrap();
        assert!(!matches!(
            initialized,
            Reached::Unestablished | Reached::Cycle
        ));
        eprintln!(
            "COPY_TRACE full_rows={} initialization_rows={} polynomial_row_budget={row_budget}",
            trace.scanned_rows, initialization.scanned_rows,
        );
        assert!(
            initialization.scanned_rows <= row_budget,
            "initialization revisited reconverging copy equations: {} rows, budget {row_budget}",
            initialization.scanned_rows,
        );

        let row_index = storage
            .log
            .iter()
            .position(|row| {
                row.block != graph.entry()
                    && row.lineage.segments.is_empty()
                    && matches!(&row.stored, Stored::Moved(source) if source.segments.is_empty())
            })
            .expect("a branch copies its formal");
        let destination = storage.log[row_index].lineage.clone();
        let source = match &storage.log[row_index].stored {
            Stored::Moved(source) => source.clone(),
            _ => unreachable!(),
        };
        // One arrival transfers just the leaf. Its initial contents are the
        // same, but it carries no evidence for a newly enclosing record.
        storage.log[row_index]
            .lineage
            .segments
            .extend_from_slice(&leaf.segments);
        if let Stored::Moved(source) = &mut storage.log[row_index].stored {
            source.segments.extend_from_slice(&leaf.segments);
        }
        for initialization_only in [false, true] {
            let mut shallower = if initialization_only {
                storage.initialization_budget()
            } else {
                storage.budget()
            };
            let reached = storage
                .reaching(
                    &mut references,
                    tables,
                    Asked::Contents(&leaf),
                    (block, after - 1),
                    &mut shallower,
                    Construct::Operation(operation.id),
                )
                .unwrap();
            if initialization_only {
                assert!(!matches!(reached, Reached::Unestablished | Reached::Cycle));
            } else {
                assert_eq!(reached, Reached::Established(0));
            }
        }
        storage.log[row_index].lineage = destination;

        // That same arrival now supplies no contents. A valid sibling arrival
        // cannot establish the joined value on its behalf.
        storage.log[row_index].stored = Stored::Vacated;
        for mut missing in [storage.budget(), storage.initialization_budget()] {
            assert_eq!(
                storage
                    .reaching(
                        &mut references,
                        tables,
                        Asked::Contents(&leaf),
                        (block, after - 1),
                        &mut missing,
                        Construct::Operation(operation.id)
                    )
                    .unwrap(),
                Reached::Unestablished
            );
        }
        storage.log[row_index].stored = Stored::Moved(source);

        // Disconnect the invocation's incoming edges from the equation graph.
        // Keep their identities so each cleanup still names its actual source.
        // The remaining backedges can carry contents but cannot initialize them.
        let mut unseeded = tables.clone();
        for (_, edge) in unseeded.edges.iter_mut() {
            if edge.source == graph.entry()
                && matches!(
                    edge.destination,
                    terminal_check_tables::edges::Destination::Block(_)
                )
            {
                edge.destination = terminal_check_tables::edges::Destination::Return;
            }
        }
        let cyclic_graph = Graph::new(machine.id, machine, &unseeded).unwrap();
        storage.graph = &cyclic_graph;
        for mut cycle in [storage.budget(), storage.initialization_budget()] {
            assert_eq!(
                storage
                    .reaching(
                        &mut references,
                        &unseeded,
                        Asked::Contents(&leaf),
                        (block, after - 1),
                        &mut cycle,
                        Construct::Operation(operation.id)
                    )
                    .unwrap(),
                Reached::Unestablished
            );
        }
    }

    #[test]
    fn a_saved_slice_cannot_use_a_later_iterations_endpoints() {
        let source = r#"
machine walk<'a>(bytes: &'a [u8], saved: &'a [u8], count: u64, first: bool) {
    let fresh: &'a [u8] = bytes[0..count];
    transition first {
        true -> inspect(bytes, fresh, count, fresh)
        false -> inspect(bytes, saved, count, fresh)
    }
}
machine inspect<'a>(bytes: &'a [u8], saved: &'a [u8], count: u64, fresh: &'a [u8]) {
    let current: u64 = fresh.len;
    let previous: u64 = saved.len;
    transition count > 1 {
        true -> walk(bytes, saved, count - 1, false)
        false -> done()
    }
    machine done() {}
}
machine entry() {
    let bytes: [u8; 3] = [11, 22, 33];
    walk(&bytes[..], &bytes[..], 3, true);
}
"#;
        let typed = crate::tests::rankings::typed_with_ranking_catalog(source);
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "entry")
            .unwrap()
            .symbol;
        let prepared = typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program;
        let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) = prepared
        else {
            panic!("the saved-slice program must prepare");
        };
        let input = prepared
            .closure
            .unwrap_or_else(|refusal| panic!("saved-slice preparation: {}", refusal.detail));
        let module = &input.module;
        let machine = module
            .machines
            .iter()
            .find(|machine| {
                machine.operations().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
            })
            .unwrap();
        let producer = machine
            .operations()
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
            .unwrap()
            .id;
        let tables = input.tables.machines.get(machine.id).unwrap();
        let graph = Graph::new(machine.id, machine, tables).unwrap();
        let types = tables.values.map(|_, row| row.scalar_type);
        let descriptor_graph = reference_graph(module, machine.id).unwrap();
        let mut references = DescriptorAnalysis::with_borrowed_storage_slots(
            &module.structural_types,
            &descriptor_graph,
        );
        let storage = Storage::new(
            machine.id,
            machine,
            module,
            &input.domains,
            &graph,
            tables,
            &types,
            &mut references,
        )
        .unwrap();
        let mut current = 0;
        let mut saved = 0;
        for operation in machine.operations() {
            if !matches!(operation.kind, OperationKind::ByteSequenceLength { .. }) {
                continue;
            }
            let point = ProgramPoint::AfterOperation(operation.id);
            if !graph
                .dominates(ProgramPoint::AfterOperation(producer), point)
                .unwrap()
            {
                continue;
            }
            let leaf = storage.read_leaf(operation).unwrap().unwrap();
            let (block, after) = graph.locate(point).unwrap();
            let mut trace = storage.budget();
            let reached = storage
                .reaching(
                    &mut references,
                    tables,
                    Asked::Contents(&leaf),
                    (block, after - 1),
                    &mut trace,
                    Construct::Operation(operation.id),
                )
                .unwrap();
            let extent = storage
                .extent_contents(
                    &mut references,
                    tables,
                    producer,
                    point,
                    operation.result.scalar_ref().unwrap().scalar_type,
                    &trace,
                )
                .unwrap();
            if storage.current_extent(producer, &trace).unwrap()
                && reached == Reached::Extent(producer)
            {
                assert!(
                    extent.is_some(),
                    "the current slice keeps its actual endpoints"
                );
                current += 1;
            } else if !storage.current_extent(producer, &trace).unwrap() {
                assert!(
                    extent.is_none(),
                    "the saved slice must not borrow today's endpoint values"
                );
                saved += 1;
            }
        }
        assert!(current > 0, "the current descriptor must be exercised");
        assert!(
            saved > 0,
            "the descriptor captured before the backedge must be exercised"
        );
    }

    #[test]
    fn block_entry_snapshots_keep_arrivals_sound_across_vacated_single_and_merged_paths() {
        let prepare_storage = |source: &str| {
            let typed = crate::tests::rankings::typed_with_ranking_catalog(source);
            let root = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "entry")
                .unwrap()
                .symbol;
            let prepared = typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[root],
                    subjects: &[],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program;
            let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) =
                prepared
            else {
                panic!("the block-entry snapshot test must prepare");
            };
            prepared
                .closure
                .unwrap_or_else(|error| panic!("{}", error.detail))
        };

        // Acyclic match join: merged distinct arrivals fall back to the
        // block-entry snapshot, a single upstream value survives, and an
        // uninitialized path still fails closed.
        let acyclic = prepare_storage(
            r#"
data Point { x: u32; y: u32; }
domain Point::Small requires self.x < 100u32;
machine entry(choose: bool, v: u32) -> u32 requires v < 50u32 {
    let raw: Point = match choose {
        true -> Point { x: v, y: 0u32 }
        false -> Point { x: 7u32, y: 0u32 }
    };
    let p: Point in Small = raw as +Small;
    let plain: Point = p as -Small;
    plain.x
}
"#,
        );
        {
            let module = &acyclic.module;
            let machine = &module.machines[0];
            let tables = acyclic.tables.machines.get(machine.id).unwrap();
            let graph = Graph::new(machine.id, machine, tables).unwrap();
            let types = tables.values.map(|_, row| row.scalar_type);
            let descriptor_graph = reference_graph(module, machine.id).unwrap();
            let mut references = DescriptorAnalysis::with_borrowed_storage_slots(
                &module.structural_types,
                &descriptor_graph,
            );
            let mut storage = Storage::new(
                machine.id,
                machine,
                module,
                &acyclic.domains,
                &graph,
                tables,
                &types,
                &mut references,
            )
            .unwrap();
            let operation = machine
                .operations()
                .find(|operation| storage.read_leaf(operation).unwrap().is_some())
                .expect("join reads p.x");
            let leaf = storage.read_leaf(operation).unwrap().unwrap();
            let (block, after) = graph
                .locate(ProgramPoint::AfterOperation(operation.id))
                .unwrap();
            assert!(!graph.in_cycle(block).unwrap());

            let mut snapshot = storage.snapshot_budget();
            let reached = storage
                .reaching(
                    &mut references,
                    tables,
                    Asked::Contents(&leaf),
                    (block, after - 1),
                    &mut snapshot,
                    Construct::Operation(operation.id),
                )
                .unwrap();
            let Reached::Value(entry_value) = reached else {
                panic!(
                    "merged initialized arrivals must expose the block-entry snapshot: {reached:?}"
                );
            };
            assert!(matches!(
                tables.values.get(entry_value).unwrap().origin,
                ValueOrigin::StorageContent { after: None, .. }
            ));
            assert!(
                storage
                    .read_premises(&mut references, tables, operation)
                    .unwrap()
                    .is_empty(),
                "locally constructed values arriving at a block parameter are not inter-machine boundary contents"
            );

            let x_writes: Vec<usize> = storage
                .log
                .iter()
                .enumerate()
                .filter_map(|(pos, row)| {
                    (row.lineage.segments == leaf.segments
                        && matches!(row.stored, Stored::Value(_)))
                    .then_some(pos)
                })
                .collect();
            assert_eq!(x_writes.len(), 2, "both branches initialize Point.x");
            let Stored::Value(shared_value) = storage.log[x_writes[0]].stored else {
                unreachable!()
            };
            let saved_second = std::mem::replace(
                &mut storage.log[x_writes[1]].stored,
                Stored::Value(shared_value),
            );
            let mut single = storage.snapshot_budget();
            assert_eq!(
                storage
                    .reaching(
                        &mut references,
                        tables,
                        Asked::Contents(&leaf),
                        (block, after - 1),
                        &mut single,
                        Construct::Operation(operation.id),
                    )
                    .unwrap(),
                Reached::Value(shared_value)
            );

            storage.log[x_writes[1]].stored = Stored::Vacated;
            for mut trace in [
                storage.snapshot_budget(),
                storage.provenance_budget(),
                storage.budget(),
                storage.initialization_budget(),
            ] {
                assert_eq!(
                    storage
                        .reaching(
                            &mut references,
                            tables,
                            Asked::Contents(&leaf),
                            (block, after - 1),
                            &mut trace,
                            Construct::Operation(operation.id),
                        )
                        .unwrap(),
                    Reached::Unestablished
                );
            }
            storage.log[x_writes[1]].stored = saved_second;
        }

        // Cyclic head: even with a single upstream write, a cycle member uses
        // its entry snapshot so inferred and declared head contracts apply,
        // while initialization checks still refuse a vacated arrival.
        let cyclic = prepare_storage(
            r#"
data Point requires self.x < 100u32 { x: u32; y: u32; }
machine entry(choose: bool, v: u32) -> u32 requires v < 50u32 {
    transition { _ -> block left(choose, v) }
    machine left(choose: bool, v: u32) -> u32 {
        transition { _ -> block step(Point { x: v, y: 0u32 }, choose, v) }
    }
    machine step(p: Point, choose: bool, v: u32) -> u32 {
        let found: u32 = p.x;
        transition choose {
            true -> found
            false -> block left(choose, v)
        }
    }
}
"#,
        );
        {
            let module = &cyclic.module;
            let machine = module
                .machines
                .iter()
                .find(|machine| {
                    machine.operations().any(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::ScalarStructuralField { .. }
                                | OperationKind::PrimitiveScalarRead { .. }
                        )
                    })
                })
                .unwrap();
            let tables = cyclic.tables.machines.get(machine.id).unwrap();
            let graph = Graph::new(machine.id, machine, tables).unwrap();
            let types = tables.values.map(|_, row| row.scalar_type);
            let descriptor_graph = reference_graph(module, machine.id).unwrap();
            let mut references = DescriptorAnalysis::with_borrowed_storage_slots(
                &module.structural_types,
                &descriptor_graph,
            );
            let mut storage = Storage::new(
                machine.id,
                machine,
                module,
                &cyclic.domains,
                &graph,
                tables,
                &types,
                &mut references,
            )
            .unwrap();
            let operation = machine
                .operations()
                .find(|operation| storage.read_leaf(operation).unwrap().is_some())
                .expect("step reads p.x");
            let leaf = storage.read_leaf(operation).unwrap().unwrap();
            let (block, after) = graph
                .locate(ProgramPoint::AfterOperation(operation.id))
                .unwrap();
            assert!(graph.in_cycle(block).unwrap());
            let mut snapshot = storage.snapshot_budget();
            let Reached::Value(entry_value) = storage
                .reaching(
                    &mut references,
                    tables,
                    Asked::Contents(&leaf),
                    (block, after - 1),
                    &mut snapshot,
                    Construct::Operation(operation.id),
                )
                .unwrap()
            else {
                panic!("a cyclic head must expose its block-entry snapshot");
            };
            assert!(matches!(
                tables.values.get(entry_value).unwrap().origin,
                ValueOrigin::StorageContent { after: None, .. }
            ));

            let x_write = storage
                .log
                .iter()
                .position(|row| {
                    row.lineage.segments == leaf.segments && matches!(row.stored, Stored::Value(_))
                })
                .unwrap();
            storage.log[x_write].stored = Stored::Vacated;
            let mut init = storage.initialization_budget();
            assert_eq!(
                storage
                    .reaching(
                        &mut references,
                        tables,
                        Asked::Contents(&leaf),
                        (block, after - 1),
                        &mut init,
                        Construct::Operation(operation.id),
                    )
                    .unwrap(),
                Reached::Unestablished
            );
        }
    }
}
