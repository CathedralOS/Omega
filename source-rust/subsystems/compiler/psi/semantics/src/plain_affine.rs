//! Ownership of plain affine storage, independently of its scalar contents.
//!
//! A value exists even when it has no scalar leaf (an empty case, for example).
//! Moves therefore change an exact region's availability, while scalar reaching
//! definitions remain a separate analysis. The producer uses the resulting
//! edge schedule; a checker reconstructs it from the same unsealed operations.

mod claims;
pub use claims::{ClaimLocation, ClaimsPlan, publish_contract_claims};
mod fences;

pub use fences::CallFence;

use arena::IdSet;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, EdgeId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralStoreDisposition,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Site {
    Operation(OperationId),
    Edge(EdgeId),
}

#[derive(Debug, Clone)]
pub struct Unavailable {
    pub site: Site,
    pub place: PlaceId,
    pub reason: UnavailableReason,
}

/// Why the storage at `Unavailable::place` cannot be used at its site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    /// The site observes storage whose value was transferred or consumed.
    Use,
    /// The site returns custody of a borrowed region while an owned move
    /// still leaves part of it vacant. This includes descriptor release and
    /// cleanup as well as normal handback of incoming storage.
    Unrestored,
    /// The site calls code whose contract does not rule out observing or
    /// exposing the incomplete owner while an owned move leaves borrowed
    /// storage vacant (`fences`). `place` is the storage holding the vacancy.
    FencedCall(CallFence),
}

impl Unavailable {
    /// The ownership walk knows why the storage is unavailable at its site;
    /// every consumer reports that reason.
    pub const fn message(&self) -> &'static str {
        match self.reason {
            UnavailableReason::Use => {
                "owned storage is used after its value was transferred or consumed"
            }
            UnavailableReason::Unrestored => {
                "an owned value was moved out of borrowed storage, and the place does not \
                 hold a value again where the machine returns the borrow: a replacement \
                 must be stored into the vacant place on every path that leaves the machine"
            }
            UnavailableReason::FencedCall(fence) => fence.message(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    /// A supported ownership check found unavailable storage at an exact site.
    Unavailable(Unavailable),
    /// A linear value is still live where a normal edge ends its storage, or
    /// is live on only some of the paths that meet at a join. ownership.md:
    /// "Every normal path accounts for each live linear obligation by
    /// transfer or an authorized terminal disposition", and "At an actual
    /// reconvergence, predecessor paths must agree on the live ownership
    /// frontier". This is the program's error, never a limit of the check.
    Unconsumed {
        /// The storage root that still holds the linear value.
        place: PlaceId,
        at_join: bool,
        site: Option<Site>,
    },
    Unsupported {
        detail: &'static str,
        site: Option<Site>,
    },
    Malformed {
        detail: &'static str,
        site: Option<Site>,
    },
}

impl Error {
    fn unsupported(detail: &'static str) -> Self {
        Self::Unsupported { detail, site: None }
    }

    fn malformed(detail: &'static str) -> Self {
        Self::Malformed { detail, site: None }
    }

    /// Preserve the actual operation/edge that failed, so the stage boundary
    /// can use its existing source correspondence rather than blaming a whole
    /// machine or guessing a neighboring source construct.
    fn at(self, at: Site) -> Self {
        match self {
            Self::Unavailable(unavailable) => Self::Unavailable(unavailable),
            Self::Unconsumed {
                place,
                at_join,
                site,
            } => Self::Unconsumed {
                place,
                at_join,
                site: site.or(Some(at)),
            },
            Self::Unsupported { detail, site } => Self::Unsupported {
                detail,
                site: site.or(Some(at)),
            },
            Self::Malformed { detail, site } => Self::Malformed {
                detail,
                site: site.or(Some(at)),
            },
        }
    }

    pub const fn site(&self) -> Option<Site> {
        match self {
            Self::Unavailable(unavailable) => Some(unavailable.site),
            Self::Unconsumed { site, .. }
            | Self::Unsupported { site, .. }
            | Self::Malformed { site, .. } => *site,
        }
    }

    /// What every consumer reports for an unconsumed linear value.
    pub const fn unconsumed_message(at_join: bool) -> &'static str {
        if at_join {
            "a linear value is still held on one path that reaches this join but not on \
             another: every path that meets here must have passed on or ended the same \
             linear values"
        } else {
            "a linear value is still held when this path leaves its storage, and nothing \
             consumes it: every path must pass it on, return it or end it by its declared \
             disposition, and neither `_ =` nor leaving scope can discard it"
        }
    }
}

#[derive(Debug, Default)]
pub struct Plan {
    pub affine_roots: IdSet<PlaceId>,
    /// Homes whose cleanup is reconstructed from the ownership walk. This
    /// includes copyable descriptors whose explicit release vacates the root.
    pub scheduled_roots: IdSet<PlaceId>,
    /// Owned copyable storage with trivial cleanup may retire its binding.
    /// It owns no affine contents to discard.
    pub copyable_owned_roots: IdSet<PlaceId>,
    pub cleanup: Vec<(EdgeId, Vec<TerminalAffineCleanupAction>)>,
    /// Exact receiving regions determine whether a store fills a vacancy or
    /// replaces intact discardable contents. Sorted by operation identity.
    pub stores: Vec<(OperationId, StructuralStoreDisposition)>,
    pub unavailable: Vec<Unavailable>,
    pub reference_ends: Vec<ReferenceEnd>,
    /// Fully restored borrowed names that the settled ownership walk ends
    /// on this edge. The loan checker still accounts for their resources.
    pub borrowed_header_ends: Vec<(EdgeId, PlaceId)>,
    pub claims: Option<ClaimsPlan>,
    /// The other machines whose blocks the walk read: the members of the
    /// descriptor graph built from this machine
    /// (`terminal_borrowed_storage_graph_from`), sorted. Empty when the walk
    /// needed no graph. Of any other machine the walk reads only its
    /// parameters, result and contract (a call's callee, an owned cleanup's
    /// target), never its blocks.
    pub graphed_machines: Vec<semantic_vocabulary::MachineId>,
}

#[derive(Debug)]
pub struct ReferenceEnd {
    pub at: crate::descriptor_lifetimes::DescriptorPosition,
    pub source: StructuralArgument,
    pub structural_type: StructuralTypeId,
}

/// `base` describes children not overridden in the trie. A partially moved
/// ancestor stays unavailable whole while disjoint established siblings remain
/// readable. Cached child counts make each mutation proportional to path depth,
/// rather than scanning every earlier move on every later observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Value,
    Components,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Region {
    mode: Mode,
    partition_count: usize,
    base: bool,
    whole: bool,
    unavailable_children: usize,
    children: Vec<(Segment, Region)>,
}

impl Region {
    fn new(available: bool) -> Self {
        Self {
            mode: Mode::Value,
            partition_count: 0,
            base: available,
            whole: available,
            unavailable_children: 0,
            children: Vec::new(),
        }
    }

    fn available(&self, path: &[Segment]) -> bool {
        let Some((head, tail)) = path.split_first() else {
            return self.whole;
        };
        match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(position) => self.children[position].1.available(tail),
            Err(_) => self.base,
        }
    }

    fn selected(&self, path: &[Segment]) -> Option<&Region> {
        let Some((head, tail)) = path.split_first() else {
            return Some(self);
        };
        match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(position) => self.children[position].1.selected(tail),
            Err(_) => None,
        }
    }

    /// Moving a value or one of its whole ancestors carries its qualifications.
    /// Vacating individual descendants cannot retire a qualification on their parent.
    fn moved_whole(&self, path: &[Segment]) -> bool {
        let Some((head, tail)) = path.split_first() else {
            return !self.base && self.children.is_empty();
        };
        match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(index) => self.children[index].1.moved_whole(tail),
            Err(_) => !self.base,
        }
    }

    fn view(&self, path: &[Segment]) -> Region {
        let Some((head, tail)) = path.split_first() else {
            return self.clone();
        };
        match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(position) => self.children[position].1.view(tail),
            Err(_) => Region::new(self.base),
        }
    }

    fn set(&mut self, path: &[Segment], available: bool) {
        let Some((head, tail)) = path.split_first() else {
            *self = Self::new(available);
            return;
        };
        let position = match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(position) => position,
            Err(position) => {
                if available == self.base {
                    return;
                }
                self.children
                    .insert(position, (*head, Self::new(self.base)));
                self.unavailable_children += usize::from(!self.base);
                position
            }
        };
        let child = &mut self.children[position].1;
        let was_unavailable = !child.whole;
        let old_partitions = child.partition_count;
        child.set(tail, available);
        self.partition_count = self.partition_count + child.partition_count - old_partitions;
        let now_unavailable = !child.whole;
        self.unavailable_children =
            self.unavailable_children + usize::from(now_unavailable) - usize::from(was_unavailable);
        if child.mode == Mode::Value && child.base == self.base && child.children.is_empty() {
            self.unavailable_children -= usize::from(!child.whole);
            self.children.remove(position);
        }
        self.whole = self.mode == Mode::Value && self.base && self.unavailable_children == 0;
    }

    fn partition(&mut self, path: &[Segment]) {
        let Some((head, tail)) = path.split_first() else {
            if self.mode != Mode::Components {
                self.partition_count += 1;
            }
            self.mode = Mode::Components;
            self.whole = false;
            return;
        };
        let position = match self
            .children
            .binary_search_by_key(head, |(segment, _)| *segment)
        {
            Ok(position) => position,
            Err(position) => {
                self.children
                    .insert(position, (*head, Self::new(self.base)));
                self.unavailable_children += usize::from(!self.base);
                position
            }
        };
        let child = &mut self.children[position].1;
        let before = !child.whole;
        let old_partitions = child.partition_count;
        child.partition(tail);
        self.partition_count = self.partition_count + child.partition_count - old_partitions;
        self.unavailable_children =
            self.unavailable_children + usize::from(!child.whole) - usize::from(before);
        self.whole = self.mode == Mode::Value && self.base && self.unavailable_children == 0;
    }

    fn partitions(&self, prefix: &mut Vec<Segment>, output: &mut Vec<Vec<Segment>>) {
        if self.mode == Mode::Components {
            output.push(prefix.clone());
        }
        for (segment, child) in &self.children {
            if child.partition_count == 0 {
                continue;
            }
            prefix.push(*segment);
            child.partitions(prefix, output);
            prefix.pop();
        }
    }

    fn contains_partition(&self, path: &[Segment]) -> bool {
        let Some((head, tail)) = path.split_first() else {
            return self.partition_count != 0;
        };
        self.children
            .binary_search_by_key(head, |(segment, _)| *segment)
            .is_ok_and(|position| self.children[position].1.contains_partition(tail))
    }

    fn same_partitions(&self, other: &Region) -> bool {
        if self.partition_count != other.partition_count {
            return false;
        }
        if self.partition_count == 0 {
            return true;
        }
        let mut mine = Vec::new();
        let mut theirs = Vec::new();
        self.partitions(&mut Vec::new(), &mut mine);
        other.partitions(&mut Vec::new(), &mut theirs);
        mine == theirs
    }

    /// What both regions hold available: every part either has vacated is
    /// vacant.
    fn intersection(&self, other: &Region) -> Region {
        let mut both = self.clone();
        let mut vacated = Vec::new();
        other.vacancies(&mut Vec::new(), &mut vacated);
        for path in vacated {
            both.set(&path, false);
        }
        both
    }

    fn vacancies(&self, prefix: &mut Vec<Segment>, output: &mut Vec<Vec<Segment>>) {
        if !self.base {
            output.push(prefix.clone());
            return;
        }
        for (segment, child) in &self.children {
            if child.whole {
                continue;
            }
            prefix.push(*segment);
            child.vacancies(prefix, output);
            prefix.pop();
        }
    }

    fn has_rebuilt_hole(&self) -> bool {
        !self.base && !self.children.is_empty()
            || self
                .children
                .iter()
                .any(|(_, child)| child.has_rebuilt_hole())
    }
}

/// Whole-value restrictions stay with their wrapper. Nested restrictions stay
/// with the exact fields they qualify when a containing record is opened.
#[derive(Clone, Debug, Default)]
enum Decomposition {
    #[default]
    Plain,
    QualifiedFields(std::sync::Arc<std::collections::BTreeSet<Vec<Segment>>>),
    QualifiedRoot,
}

impl Decomposition {
    /// Keep the whole restriction when a projection cannot be resolved to an
    /// inline storage region (for example, membership of every array element).
    fn protect(&mut self, path: Option<Vec<Segment>>) {
        let Some(path) = path.filter(|path| !path.is_empty()) else {
            *self = Self::QualifiedRoot;
            return;
        };
        match self {
            Self::Plain => {
                *self = Self::QualifiedFields(std::sync::Arc::new([path].into_iter().collect()));
            }
            Self::QualifiedFields(paths) => {
                std::sync::Arc::make_mut(paths).insert(path);
            }
            Self::QualifiedRoot => {}
        }
    }

    fn requires_conservation_at(&self, path: &[Segment]) -> bool {
        match self {
            Self::Plain => false,
            Self::QualifiedFields(fields) => fields.iter().any(|field| path.starts_with(field)),
            Self::QualifiedRoot => true,
        }
    }
}

#[derive(Debug, Clone)]
struct Root {
    structural_type: StructuralTypeId,
    owned: bool,
    /// A reference-bearing home participates in the availability walk even
    /// when it holds an unrestricted value. Its contract still determines
    /// which disposal the home owes.
    affine: bool,
    multiplicity: terminal_psi::StructuralMultiplicity,
    /// The root's own contract is nonlinear, unqualified and claim-free.
    /// This is a static storage restriction, never authority to use a loan.
    whole_restoration: bool,
    /// Decomposition may end a linear wrapper, but cannot settle qualifications
    /// or independent content custody attached to its storage.
    decomposition: Decomposition,
    parameter: bool,
    /// A machine or block header lent with `@mutable`. A block's incoming edges
    /// authenticate the requested access; renaming storage grants none.
    exclusive: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct State {
    regions: Vec<Option<Region>>,
    // Establishment order of the currently allocated owned homes. Normal-edge
    // disposal walks this frontier, not every root ever produced by the body.
    active: Vec<PlaceId>,
    claims: claims::ClaimsState,
}

impl State {
    /// The frontier two arrivals at one block agree on. A root one arrival
    /// no longer owns (its home was consumed by a whole transfer, or it was
    /// disposed of) is not owned at the join. Of a root both still own, the
    /// join owns the part both own: a part one arrival moved out is vacant
    /// there, and the arrival that still holds it discards it on its edge
    /// and keeps the home (`Analysis::walk`).
    fn meet(&self, other: &State) -> Result<State, Error> {
        if self.regions.len() != other.regions.len() {
            return Err(Error::malformed("two arrivals track different places"));
        }
        let regions = self
            .regions
            .iter()
            .zip(&other.regions)
            .map(|pair| match pair {
                (Some(mine), Some(theirs)) => {
                    if !mine.same_partitions(theirs) {
                        return Err(Error::unsupported(
                            "arrivals disagree on the nominal component partition",
                        ));
                    }
                    Ok(Some(mine.intersection(theirs)))
                }
                _ => Ok(None),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let owned = |active: &[PlaceId]| -> Result<Vec<PlaceId>, Error> {
            let mut kept = Vec::new();
            for place in active {
                if regions[index(place.get())?].is_some() {
                    kept.push(*place);
                }
            }
            Ok(kept)
        };
        let active = owned(&self.active)?;
        if active != owned(&other.active)? {
            return Err(Error::unsupported(
                "owned homes established in different orders at a reconvergence",
            ));
        }
        Ok(State {
            regions,
            active,
            claims: self.claims.meet(&other.claims)?,
        })
    }
}

/// One normal edge out of a walked block.
struct Arrival {
    edge: EdgeId,
    /// `None` for an edge that leaves the machine.
    target: Option<semantic_vocabulary::BlockId>,
    outgoing: State,
    actions: Vec<TerminalAffineCleanupAction>,
    borrowed_header_ends: Vec<PlaceId>,
}

struct Residence {
    path: Vec<Segment>,
    live: std::collections::BTreeSet<crate::descriptor_lifetimes::DescriptorPosition>,
}

struct Components<'a> {
    common: &'a [terminal_psi::StructuralFieldDeclaration],
    case: Option<&'a terminal_psi::StructuralCaseDeclaration>,
}
impl<'a> Components<'a> {
    fn fields(
        &self,
    ) -> impl DoubleEndedIterator<
        Item = (
            Option<semantic_vocabulary::StructuralCaseId>,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
    > + '_ {
        self.common.iter().map(|field| (None, field)).chain(
            self.case
                .into_iter()
                .flat_map(|case| case.fields.iter().map(move |field| (Some(case.id), field))),
        )
    }
}

type BackingPath = (PlaceId, Vec<StructuralPathSegment>);

enum WindowBacking {
    Exact(BackingPath),
    Alternatives(Vec<BackingPath>),
}

impl WindowBacking {
    fn targets(&self) -> &[BackingPath] {
        match self {
            Self::Exact(target) => std::slice::from_ref(target),
            Self::Alternatives(targets) => targets,
        }
    }
}

struct Analysis<'a> {
    module: Option<&'a terminal_psi::TerminalModule>,
    /// The call fences of `module`; `None` exactly when `module` is.
    fences: Option<&'a fences::CallFences<'a>>,
    types: &'a [StructuralTypeDeclaration],
    dispositions: Vec<Option<bool>>,
    roots: Vec<Option<Root>>,
    borrowed_headers: Vec<PlaceId>,
    unavailable: Vec<Unavailable>,
    unavailable_roots: Vec<Option<Site>>,
    stores: Vec<(OperationId, StructuralStoreDisposition)>,
    verify_stores: bool,
    references: Option<crate::descriptor_results::DescriptorAnalysis<'a>>,
    body: Option<&'a TerminalMachine>,
    /// Residence demand comes from actual descriptor dispositions. It only
    /// keeps captured homes allocated; `State::regions` remains the single
    /// owner of exact initialization and restoration availability.
    residences: Vec<Vec<Residence>>,
    position: Option<crate::descriptor_lifetimes::DescriptorPosition>,
    successor: Option<(usize, EdgeId)>,
    reference_candidates: Option<&'a crate::reference_ends::Candidates>,
    reference_ends: Vec<ReferenceEnd>,
    claim_flow: Option<claims::Flow<'a>>,
}

/// The inline array whose established contents a retained runtime selector
/// needs. It covers any element the operation could select, so it grants no
/// exact extraction or vacant initialization.
pub(crate) fn runtime_array_region(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<Segment>> {
    let indexed = path
        .iter()
        .position(|segment| segment.runtime_index().is_some())?;
    let (array, indexed_type) =
        crate::canonical_static_projection(types.iter(), root, &path[..indexed])?;
    // An element view's elements are outside the storage that holds the
    // view. Only a fixed inline array's region covers its elements.
    (matches!(indexed_type.shape, StructuralTypeShape::FixedArray { .. })
        && matches!(
            crate::runtime_read_projection_tip(types.iter(), root, path),
            Some(crate::Position::Declared(_) | crate::Position::ScalarField(_))
        ))
    .then_some(array)
}

fn index(raw: u64) -> Result<usize, Error> {
    usize::try_from(raw)
        .ok()
        .and_then(|raw| raw.checked_sub(1))
        .ok_or(Error::malformed("zero or oversized dense identity"))
}

fn storage_root(used: terminal_psi::StructuralStorageUse<'_>) -> Option<PlaceId> {
    use terminal_psi::StructuralStorageUse;
    match used {
        StructuralStorageUse::Place(root)
        | StructuralStorageUse::Canonical { root, .. }
        | StructuralStorageUse::ObservedField { root, .. } => Some(root),
        StructuralStorageUse::Projection(projection) => Some(projection.root),
        StructuralStorageUse::Element { descriptor, .. } => Some(descriptor.root),
        StructuralStorageUse::Type(_) => None,
    }
}

// Cleanup is output of this analysis, never an observation that keeps its
// own root alive. Only actual outgoing values and case selection are inputs.
fn terminator_roots(terminator: &terminal_psi::Terminator) -> Vec<PlaceId> {
    use terminal_psi::Terminator as T;
    match terminator {
        T::PendingNormalCompletion { storage, .. } => storage.clone(),
        T::StructuralCase { source, .. }
        | T::EliminateEmpty { source, .. }
        | T::ReturnStructural { source, .. } => vec![*source],
        T::Jump {
            structural_arguments,
            ..
        }
        | T::InvokeNoNormal {
            structural_arguments,
            ..
        }
        | T::ExternalCompletion {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        T::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .structural_arguments
            .iter()
            .chain(&when_false.structural_arguments)
            .map(|argument| argument.place)
            .collect(),
        T::Return { .. } | T::ReturnUnit { .. } | T::Crash { .. } => Vec::new(),
    }
}

/// A carrier's use also keeps its original storage alive. Descriptor homes
/// and their backing are different owners: disposing the latter just because
/// its authored name is dead invalidates a still-live reference.
fn reference_backing_uses(
    types: &[StructuralTypeDeclaration],
    graph: &crate::descriptor_results::DescriptorMachine,
    references: &mut crate::descriptor_results::DescriptorAnalysis<'_>,
    candidates: Option<&crate::reference_ends::Candidates>,
) -> Result<
    std::collections::BTreeMap<crate::descriptor_lifetimes::DescriptorPosition, Vec<PlaceId>>,
    Error,
> {
    use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPathSegment};
    use crate::descriptor_results::{
        DescriptorCallOrigin, DescriptorSlotKind, DescriptorSourceOrigin, FlowFeasibility,
    };
    let mut uses = std::collections::BTreeMap::new();
    for (at, step) in &graph.flow {
        let mut inputs: Vec<_> = step.observed_inputs().cloned().collect();
        for candidate in candidates
            .and_then(|candidates| candidates.get(at))
            .into_iter()
            .flatten()
        {
            let argument = &candidate.argument;
            let ty = *graph
                .place_types
                .get(&argument.place)
                .ok_or(Error::malformed(
                    "reference disposition lost its source type",
                ))?;
            inputs.push(
                crate::descriptor_lifetimes::descriptor_location(
                    types,
                    argument.place,
                    ty,
                    &argument.path,
                    None,
                )
                .ok_or(Error::unsupported(
                    "reference disposition has no exact source path",
                ))?,
            );
        }
        for input in inputs {
            let inventory = DescriptorSlotKind::BorrowedStorage
                .entry_inventory(types, graph, input.root)
                .ok_or(Error::unsupported(
                    "reference observation has no owned contents inventory",
                ))?;
            for unknown in inventory.unknown_contents {
                let mut carrier = unknown.location;
                if carrier.path.last() == Some(&DescriptorPathSegment::UnknownContents) {
                    carrier.path.pop();
                }
                if input.contains(&carrier) || carrier.contains(&input) {
                    // An unknown packet has no address we can resolve. Keep
                    // its entire carrier live; this supplies no access to its
                    // hidden referents and does not authorize their release.
                    uses.entry(*at).or_insert_with(Vec::new).push(input.root);
                }
            }
            'slots: for mut slot in inventory.slots {
                if !input.contains(&slot) && !slot.contains(&input) {
                    continue;
                }
                for (position, segment) in slot.path.iter().enumerate() {
                    let DescriptorPathSegment::Case(case) = segment else {
                        continue;
                    };
                    let predicate = crate::descriptor_results::DescriptorCasePredicate {
                        subject: DescriptorLocation {
                            root: slot.root,
                            path: slot.path[..position].to_vec(),
                        }
                        .into(),
                        case: *case,
                    };
                    if references.case_feasibility(graph.id, *at, &predicate)
                        == FlowFeasibility::ProvedImpossible
                    {
                        continue 'slots;
                    }
                }
                let roots = uses.entry(*at).or_insert_with(Vec::new);
                if let Some(mut origins) = references.symbolic_source_origins(graph.id, *at, &slot)
                {
                    // The descriptor may name provider storage. Its declared input
                    // loans still keep each actual lender alive until this use.
                    while let Some(origin) = origins.pop() {
                        match origin {
                            DescriptorSourceOrigin::Provider(provider) => {
                                origins.extend(
                                    provider
                                        .lifetime_dependencies
                                        .into_iter()
                                        .map(|debt| debt.origin),
                                );
                            }
                            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                                source,
                                ..
                            }) => roots.push(source),
                            DescriptorSourceOrigin::IncomingSlot(source) => roots.push(source.root),
                            DescriptorSourceOrigin::IncomingContents { query, .. } => {
                                roots.push(query.root)
                            }
                            DescriptorSourceOrigin::Backing(
                                DescriptorCallOrigin::AccessFormation { machine, place, .. },
                            ) if machine == graph.id => roots.push(place),
                            DescriptorSourceOrigin::Backing(_) => {}
                        }
                    }
                    continue;
                }
                slot.path.push(DescriptorPathSegment::Referent);
                if let Some(source) = references.location(graph.id, *at, &slot) {
                    if !graph.place_types.contains_key(&source.root) {
                        return Err(Error::malformed("reference backing is outside its machine"));
                    }
                    roots.push(source.root);
                    continue;
                }
                // Liveness must keep every address the reference can select.
                // The loan checker separately judges access to its referent.
                let family = references
                    .captured_storage_family(graph.id, *at, &slot)
                    .map_err(|_| {
                        Error::unsupported("reference observation has no exact live backing")
                    })?;
                for origin in family {
                    let crate::descriptor_results::DescriptorCallOrigin::Argument {
                        source, ..
                    } = origin
                    else {
                        return Err(Error::unsupported(
                            "reference observation has no exact live backing",
                        ));
                    };
                    roots.push(source);
                }
            }
        }
    }
    for roots in uses.values_mut() {
        roots.sort_unstable();
        roots.dedup();
    }
    Ok(uses)
}

fn live_entries(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    reference_candidates: Option<&crate::reference_ends::Candidates>,
    reference_backing: &std::collections::BTreeMap<
        crate::descriptor_lifetimes::DescriptorPosition,
        Vec<PlaceId>,
    >,
) -> Result<Vec<IdSet<PlaceId>>, Error> {
    let valid = |place: PlaceId| -> Result<PlaceId, Error> {
        if index(place.get())? < machine.structural_places.len() {
            Ok(place)
        } else {
            Err(Error::malformed("live place is outside the machine"))
        }
    };
    let mut header_backing = std::collections::BTreeMap::new();
    for block in &machine.blocks {
        for parameter in &block.structural_parameters {
            if parameter.access == StructuralAccess::Owned
                || crate::descriptor_results::is_descriptor(types.iter(), parameter.structural_type)
            {
                continue;
            }
            header_backing.insert(parameter.place, (block.id, None));
        }
    }
    // A borrowed header observes its captured backing in any block that
    // uses it. Add those dependencies at each use, so preceding definitions
    // still stop the backward walk at their establishment points.
    let mut predecessors = None;
    let mut observe = |before: &mut IdSet<PlaceId>, place| -> Result<(), Error> {
        before.insert(valid(place)?);
        if let Some((block, backing)) = header_backing.get_mut(&place) {
            let roots = backing.get_or_insert_with(|| {
                let incoming = predecessors.get_or_insert_with(|| {
                    crate::storage::arrival_storage::StoragePredecessors::new(machine)
                });
                let names = incoming.storage_names(*block, place);
                let mut roots: Vec<_> = names.captured_roots().collect();
                roots.sort_unstable();
                roots.dedup();
                roots
            });
            for &root in roots.iter() {
                before.insert(valid(root)?);
            }
        }
        Ok(())
    };
    let mut live = vec![IdSet::new(); machine.blocks.len()];
    loop {
        let mut changed = false;
        for block in machine.blocks.iter().rev() {
            let mut before = IdSet::new();
            for target in block.terminator.successor_targets() {
                before.union_with(
                    live.get(index(target.get())?)
                        .ok_or(Error::malformed("edge target is outside the machine"))?,
                );
            }
            for root in terminator_roots(&block.terminator) {
                observe(&mut before, root)?;
            }
            let end = u32::try_from(block.operations.len())
                .map_err(|_| Error::malformed("reference observation position overflows"))?;
            for &root in reference_backing
                .get(&(block.id, end))
                .into_iter()
                .flatten()
            {
                observe(&mut before, root)?;
            }
            for (position, operation) in block.operations.iter().enumerate().rev() {
                if let Some(result) = operation.result.as_structural() {
                    before.remove(valid(result.place)?);
                }
                for used in operation
                    .kind
                    .storage_uses()
                    .into_iter()
                    .filter_map(|(storage, _)| storage_root(storage))
                {
                    observe(&mut before, used)?;
                }
                // A planned release is a real future use of its owned
                // descriptor home. Keep that home until the planner can emit
                // it; otherwise edge cleanup could close a parent while its
                // child still runs. The final verifier reads emitted uses.
                let position = u32::try_from(position)
                    .map_err(|_| Error::malformed("reference end position overflows"))?;
                for &root in reference_backing
                    .get(&(block.id, position))
                    .into_iter()
                    .flatten()
                {
                    observe(&mut before, root)?;
                }
                for candidate in reference_candidates
                    .and_then(|candidates| candidates.get(&(block.id, position)))
                    .into_iter()
                    .flatten()
                {
                    observe(&mut before, candidate.argument.place)?;
                }
            }
            // A borrowed header receives an already captured address. Its
            // backing must survive the incoming edge's cleanup even when
            // the destination never reads its formal.
            for parameter in &block.structural_parameters {
                if parameter.access != StructuralAccess::Owned {
                    observe(&mut before, parameter.place)?;
                }
                before.remove(valid(parameter.place)?);
            }
            let old = live
                .get_mut(index(block.id.get())?)
                .ok_or(Error::malformed("block is outside the machine"))?;
            if *old != before {
                *old = before;
                changed = true;
            }
        }
        if !changed {
            return Ok(live);
        }
    }
}

/// Future uses of a non-owning header, independently of loans held in its contents.
/// Each block retains the last actual use, rather than a set at every operation.
pub struct BorrowedAddressLiveness {
    headers: IdSet<PlaceId>,
    blocks: Vec<Option<BorrowedAddressUses>>,
}

struct BorrowedAddressUses {
    end: u32,
    last_uses: Vec<(PlaceId, u32)>,
}

impl BorrowedAddressLiveness {
    pub fn is_address_header(&self, place: PlaceId) -> bool {
        self.headers.contains(place)
    }

    pub fn is_live(
        &self,
        place: PlaceId,
        at: crate::descriptor_lifetimes::DescriptorPosition,
    ) -> Option<bool> {
        let block = self.blocks.get(index(at.0.get()).ok()?)?.as_ref()?;
        if at.1 > block.end {
            return None;
        }
        Some(
            block
                .last_uses
                .binary_search_by_key(&place, |(place, _)| *place)
                .is_ok_and(|position| at.1 <= block.last_uses[position].1),
        )
    }
}

pub fn borrowed_address_liveness(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
) -> Result<BorrowedAddressLiveness, Error> {
    let mut headers = IdSet::new();
    let mut header_places = Vec::new();
    for block in &machine.blocks {
        for parameter in &block.structural_parameters {
            if parameter.access == StructuralAccess::Owned {
                continue;
            }
            let place = machine
                .structural_places
                .get(index(parameter.place.get())?)
                .ok_or(Error::malformed(
                    "borrowed address header is outside the place domain",
                ))?;
            if place.id != parameter.place
                || !matches!(place.kind,
                semantic_vocabulary::StructuralPlaceKind::BlockParameter { block: owner, position }
                if owner == block.id && position == parameter.position)
            {
                return Err(Error::malformed(
                    "borrowed address header has another block owner",
                ));
            }
            crate::structural_declaration(types.iter(), parameter.structural_type).ok_or(
                Error::malformed("borrowed address header lost its declared carrier"),
            )?;
            if crate::descriptor_results::DescriptorSlotKind::BorrowedStorage
                .borrows_inline_storage(types, parameter.place, parameter.structural_type)
                && headers.insert(parameter.place)
            {
                header_places.push(parameter.place);
            }
        }
    }
    let mut result = BorrowedAddressLiveness {
        headers,
        blocks: (0..machine.blocks.len()).map(|_| None).collect(),
    };
    if header_places.is_empty() {
        return Ok(result);
    }
    let backing = Default::default();
    let live = live_entries(machine, types, None, &backing)?;
    for block in &machine.blocks {
        let end = u32::try_from(block.operations.len())
            .map_err(|_| Error::malformed("reference observation position overflows"))?;
        let mut last_uses = Vec::new();
        let mut retain = |place, at| {
            if result.headers.contains(place) {
                last_uses.push((place, at));
            }
        };
        for target in block.terminator.successor_targets() {
            let target_live = live
                .get(index(target.get())?)
                .ok_or(Error::malformed("edge target is outside the machine"))?;
            for &place in &header_places {
                if target_live.contains(place) {
                    retain(place, end);
                }
            }
        }
        for place in terminator_roots(&block.terminator) {
            retain(place, end);
        }
        for (position, operation) in block.operations.iter().enumerate() {
            // A block parameter cannot be defined again by an operation.
            if operation
                .result
                .as_structural()
                .is_some_and(|result_place| result.headers.contains(result_place.place))
            {
                return Err(Error::malformed(
                    "borrowed address header is redefined inside its block",
                ));
            }
            let position = u32::try_from(position)
                .map_err(|_| Error::malformed("reference observation position overflows"))?;
            for place in operation
                .kind
                .storage_uses()
                .into_iter()
                .filter_map(|(storage, _)| storage_root(storage))
            {
                retain(place, position);
            }
        }
        last_uses.sort_unstable_by_key(|(place, position)| (*place, std::cmp::Reverse(*position)));
        last_uses.dedup_by_key(|(place, _)| *place);
        let row = result
            .blocks
            .get_mut(index(block.id.get())?)
            .ok_or(Error::malformed("block is outside the machine"))?;
        if row.is_some() {
            return Err(Error::malformed("borrowed address block is repeated"));
        }
        *row = Some(BorrowedAddressUses { end, last_uses });
    }
    Ok(result)
}

/// Plans only no-code affine storage, including each receiving region's store
/// disposition. The producer installs these dispositions before verification.
/// Selected owned consumers require the module-aware entry point. Conserved
/// claims and borrowed extraction retain their independent contracts.
pub fn plan(machine: &TerminalMachine, types: &[StructuralTypeDeclaration]) -> Result<Plan, Error> {
    analyze(machine, types, false, None, None, None)
}

/// An affine walk's descriptor graph has no nominal calls. Its rows can share
/// a module's summaries only with a table built from this same slice, which
/// `ModuleSummaries` checks by address.
static NO_NOMINAL_CALLS: [terminal_psi::generic_calls::NominalCallArgument; 0] = [];

/// The callee summaries that every affine walk over one module replays. Each
/// walk builds its machine's borrowed-storage graph without nominal calls,
/// so the walks of all the module's machines, on any worker, can share one
/// table: a walk takes a callee's summary from it only where its own replay
/// would have produced the same one (`ModuleSummaries`). Build the table
/// from the exact module the walks read; a walk over another module, even an
/// equal copy, replays alone.
pub struct AffineSummaries<'m> {
    summaries: crate::descriptor_results::ModuleSummaries<'m>,
    fences: fences::CallFences<'m>,
    type_permissions: Result<(), Error>,
}

impl<'m> AffineSummaries<'m> {
    pub fn new(module: &'m terminal_psi::TerminalModule) -> Self {
        Self {
            summaries: crate::descriptor_results::ModuleSummaries::borrowed_storage(
                module,
                &NO_NOMINAL_CALLS,
            ),
            fences: fences::CallFences::new(module),
            type_permissions: validate_type_permissions(&module.types.structural_types),
        }
    }

    /// Whether some walk has completed and published `machine`'s summary.
    pub fn holds(&self, machine: semantic_vocabulary::MachineId) -> bool {
        self.summaries.get(machine).is_some()
    }

    /// Let a fresh analysis over rows built from this table's module without
    /// nominal calls take and publish summaries here. `ModuleSummaries`
    /// refuses rows it cannot serve, and that analysis then replays alone.
    pub(crate) fn share_with<'a>(
        &'a self,
        analysis: &mut crate::descriptor_results::DescriptorAnalysis<'a>,
    ) {
        analysis.share_summaries(&self.summaries, &NO_NOMINAL_CALLS);
    }
}

/// The affine walks of one machine in one module. Every walk that needs
/// conditional storage reads the same borrowed-storage graph, built from
/// this machine (`terminal_borrowed_storage_graph_from`); the first walk
/// builds it and later walks reuse it. With an `AffineSummaries` table the
/// walks also take callee summaries other walks of the module completed.
/// Each answer is the one a walk alone would give.
pub struct MachineAffine<'s> {
    module: &'s terminal_psi::TerminalModule,
    machine: &'s TerminalMachine,
    summaries: Option<&'s AffineSummaries<'s>>,
    graph: std::cell::OnceCell<
        Result<
            Vec<crate::descriptor_results::DescriptorMachine>,
            crate::descriptor_results::DescriptorGraphError,
        >,
    >,
    /// Used only without a module table built from this exact module.
    fences: fences::CallFences<'s>,
}

impl<'s> MachineAffine<'s> {
    pub fn new(
        module: &'s terminal_psi::TerminalModule,
        identity: semantic_vocabulary::MachineId,
        summaries: Option<&'s AffineSummaries<'s>>,
    ) -> Result<Self, Error> {
        Ok(Self {
            module,
            machine: module_machine(module, identity)?,
            summaries,
            graph: std::cell::OnceCell::new(),
            fences: fences::CallFences::new(module),
        })
    }

    /// The module table's call fences when it was built from this module,
    /// so every walk over the module shares one; otherwise this walk's own.
    fn fences(&self) -> &fences::CallFences<'s> {
        match self.summaries {
            Some(summaries) if std::ptr::eq(summaries.fences.module(), self.module) => {
                &summaries.fences
            }
            _ => &self.fences,
        }
    }

    fn graph(
        &self,
    ) -> Result<
        &[crate::descriptor_results::DescriptorMachine],
        crate::descriptor_results::DescriptorGraphError,
    > {
        match self.graph.get_or_init(|| {
            crate::descriptor_results::terminal_borrowed_storage_graph_from(
                self.module,
                [self.machine.id],
                &[],
            )
        }) {
            Ok(graph) => Ok(graph),
            Err(failure) => Err(*failure),
        }
    }

    /// A fresh replay over this machine's graph, taking and publishing
    /// callee summaries through the module's table when there is one.
    fn references<'r>(
        &'r self,
        graph: &'r [crate::descriptor_results::DescriptorMachine],
    ) -> crate::descriptor_results::DescriptorAnalysis<'r> {
        let mut references =
            crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &self.module.types.structural_types,
                graph,
            );
        if let Some(summaries) = self.summaries {
            summaries.share_with(&mut references);
        }
        references
    }

    fn analyze(
        &self,
        bound_output: bool,
        reference_candidates: Option<&crate::reference_ends::Candidates>,
        claim_mode: Option<claims::Mode>,
    ) -> Result<Plan, Error> {
        analyze(
            self.machine,
            &self.module.types.structural_types,
            bound_output,
            Some(self),
            reference_candidates,
            claim_mode,
        )
    }

    /// Resolve destructive reference paths with the same descriptor graph as
    /// storage and loan checking. The detached value never owns its lender's home.
    pub fn plan(&self) -> Result<Plan, Error> {
        self.analyze(false, None, None)
    }

    /// Resolve dead descriptor candidates against the same exact ownership
    /// frontier that supplies residual cleanup. Moved or already disposed
    /// leaves are not released again, and borrowed descriptor storage never
    /// grants disposition.
    pub fn plan_reference_ends(
        &self,
        candidates: &crate::reference_ends::Candidates,
    ) -> Result<Plan, Error> {
        self.analyze(false, Some(candidates), None)
    }

    /// The receiving checker reconstructs both the exact backing and its schedule.
    pub fn verify(&self) -> Result<Plan, Error> {
        let has_owned_cleanup = self
            .machine
            .blocks
            .iter()
            .flat_map(|block| block.terminator.cleanup_rosters())
            .any(|(_, actions)| {
                actions
                    .iter()
                    .any(|action| matches!(action, TerminalAffineCleanupAction::InvokeOwned(_)))
            });
        verify_schedule(
            self.machine,
            self.analyze(
                true,
                None,
                has_owned_cleanup.then_some(claims::Mode::Verify),
            )?,
        )
    }

    /// Emit logical claim rows from the same ownership walk that plans storage.
    pub fn plan_claims(&self) -> Result<ClaimsPlan, Error> {
        self.analyze(false, None, Some(claims::Mode::Publish))?
            .claims
            .ok_or(Error::malformed("claim producer lost its plan"))
    }

    /// Receiving retains authenticated declaration names and reconstructs
    /// every move/call/return without taking the supplied transfer rows as
    /// evidence.
    pub fn verify_claims(&self) -> Result<ClaimsPlan, Error> {
        let plan = self.analyze(true, None, Some(claims::Mode::Verify))?;
        // Finish reconstructing the claim rows before reporting storage debt:
        // malformed transport must still refuse as malformed, while an exact
        // unavailable place is an ordinary ownership error, not missing support.
        if let Some(unavailable) = plan.unavailable.into_iter().next() {
            return Err(Error::Unavailable(unavailable));
        }
        plan.claims
            .ok_or(Error::malformed("claim receiver lost its plan"))
    }
}

/// `MachineAffine::plan` for one walk.
pub fn plan_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<Plan, Error> {
    MachineAffine::new(module, identity, None)?.plan()
}

/// `MachineAffine::plan_reference_ends` for one walk.
pub fn plan_reference_ends(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
    candidates: &crate::reference_ends::Candidates,
) -> Result<Plan, Error> {
    MachineAffine::new(module, identity, None)?.plan_reference_ends(candidates)
}

/// Reconstructs the canonical schedule without trusting the supplied actions.
/// The producer's actual output bounds residual enumeration before allocation.
/// Exact equality then checks root/path/type identity, maximality and order.
pub fn verify(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
) -> Result<Plan, Error> {
    verify_schedule(machine, analyze(machine, types, true, None, None, None)?)
}

/// `MachineAffine::verify` for one walk.
pub fn verify_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<Plan, Error> {
    MachineAffine::new(module, identity, None)?.verify()
}

/// `MachineAffine::plan_claims` for one walk.
pub fn plan_claims_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<ClaimsPlan, Error> {
    MachineAffine::new(module, identity, None)?.plan_claims()
}

/// `MachineAffine::verify_claims` for one walk.
pub fn verify_claims_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<ClaimsPlan, Error> {
    MachineAffine::new(module, identity, None)?.verify_claims()
}

fn module_machine(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<&TerminalMachine, Error> {
    module
        .machines
        .get(index(identity.get())?)
        .filter(|machine| machine.id == identity)
        .ok_or(Error::malformed(
            "a window's machine is outside its module catalog",
        ))
}

fn verify_schedule(machine: &TerminalMachine, plan: Plan) -> Result<Plan, Error> {
    for block in &machine.blocks {
        for (edge, actions) in block.terminator.cleanup_rosters() {
            for action in actions {
                if machine
                    .structural_places
                    .get(index(action.place().get())?)
                    .is_none_or(|place| place.id != action.place())
                {
                    return Err(Error::malformed("cleanup root is outside the machine"));
                }
                // Retirement can end a copyable binding. Whole-root discard
                // consumes an affine obligation from the schedule below.
                if !plan.affine_roots.contains(action.place())
                    && (!matches!(action, TerminalAffineCleanupAction::RetireHome(_))
                        || !plan.copyable_owned_roots.contains(action.place()))
                {
                    return Err(Error::unsupported(
                        "disposal of storage the affine schedule does not own",
                    )
                    .at(Site::Edge(edge)));
                }
            }
            let actual = actions
                .iter()
                .filter(|action| plan.scheduled_roots.contains(action.place()));
            let expected = plan
                .cleanup
                .binary_search_by_key(&edge, |(edge, _)| *edge)
                .ok()
                .map(|position| plan.cleanup[position].1.as_slice())
                .unwrap_or(&[]);
            if !actual
                .zip(expected.iter())
                .all(|(actual, expected)| match (actual, expected) {
                    (
                        TerminalAffineCleanupAction::InvokeOwned(actual),
                        TerminalAffineCleanupAction::InvokeOwned(expected),
                    ) => {
                        // The claim walk checks each transfer against live custody.
                        // The call checker checks the requirement obligations.
                        actual.subject == expected.subject
                            && actual.structural_type == expected.structural_type
                            && actual.cleanup_machine == expected.cleanup_machine
                    }
                    _ => actual == expected,
                })
                || actions
                    .iter()
                    .filter(|action| plan.scheduled_roots.contains(action.place()))
                    .count()
                    != expected.len()
            {
                return Err(Error::malformed(
                    "normal edge affine disposal differs from its canonical schedule",
                ));
            }
        }
    }
    Ok(plan)
}

/// The multiplicity supplied by a structural type and its access mode.
/// A fresh permission-producing operation can require its own affine result.
pub fn declared_multiplicity(
    declaration: &StructuralTypeDeclaration,
    access: StructuralAccess,
) -> terminal_psi::StructuralMultiplicity {
    use terminal_psi::StructuralMultiplicity;
    if access != StructuralAccess::Owned {
        StructuralMultiplicity::Unrestricted
    } else {
        declaration.multiplicity
    }
}

/// The custody a block binding may carry from an actual fresh permission.
/// This classifies the requested binding; every incoming carrier still needs
/// `validate_carried_permissions` before that custody can justify a discard.
pub(crate) fn required_parameter_multiplicity(
    kind: semantic_vocabulary::StructuralPlaceKind,
    access: StructuralAccess,
    multiplicity: terminal_psi::StructuralMultiplicity,
    declaration: &StructuralTypeDeclaration,
) -> Option<terminal_psi::StructuralMultiplicity> {
    (matches!(
        kind,
        semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
    ) && access == StructuralAccess::Owned
        && multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && declaration.is_copyable()
        && matches!(
            declaration.shape,
            StructuralTypeShape::Reference {
                access: StructuralAccess::SharedBorrow,
                ..
            }
        ))
    .then_some(terminal_psi::StructuralMultiplicity::Affine)
}

/// Every incoming edge transfers the exact owned affine carrier that its
/// destination binding requests. A copyable source or a borrowed argument
/// cannot manufacture that occurrence ownership.
pub(crate) fn validate_carried_permissions(
    machine: &TerminalMachine,
    carried: &[PlaceId],
    source_contract: impl Fn(
        PlaceId,
    ) -> Option<(
        StructuralTypeId,
        terminal_psi::StructuralMultiplicity,
        StructuralAccess,
    )>,
) -> Result<(), &'static str> {
    if carried.is_empty() {
        return Ok(());
    }
    if carried.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("carried permission bindings are not strictly ordered");
    }
    for block in &machine.blocks {
        let edges = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if edges.len() != arguments.len() {
            return Err("a structural successor has no complete argument roster");
        }
        for ((_, target), arguments) in edges.into_iter().zip(arguments) {
            let target = machine
                .blocks
                .binary_search_by_key(&target, |block| block.id)
                .ok()
                .map(|index| &machine.blocks[index])
                .ok_or("a structural successor names an absent block")?;
            if arguments.len() != target.structural_parameters.len() {
                return Err("a structural successor has no complete argument roster");
            }
            for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
                if carried.binary_search(&parameter.place).is_err() {
                    continue;
                }
                let affine = argument.path.is_empty()
                    && argument.access == StructuralAccess::Owned
                    && source_contract(argument.place).is_some_and(
                        |(structural_type, multiplicity, access)| {
                            structural_type == parameter.structural_type
                                && multiplicity == terminal_psi::StructuralMultiplicity::Affine
                                && access == StructuralAccess::Owned
                        },
                    );
                if !affine {
                    return Err("an affine permission parameter receives other than its carrier");
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn required_result_multiplicity(
    operation: &terminal_psi::Operation,
) -> Option<terminal_psi::StructuralMultiplicity> {
    crate::StructuralEffectSemanticRow::for_tag(crate::OperationSemanticTag::for_operation(
        &operation.kind,
    ))
    .and_then(|row| row.schema().required_result_multiplicity())
}

fn prepare_disposition(
    types: &[StructuralTypeDeclaration],
    module: Option<&terminal_psi::TerminalModule>,
    ty: StructuralTypeId,
    rows: &mut [Option<bool>],
    visiting: &mut [bool],
) -> Result<bool, Error> {
    let raw = index(ty.get())?;
    if let Some(answer) = rows.get(raw).copied().flatten() {
        return Ok(answer);
    }
    if *visiting
        .get(raw)
        .ok_or(Error::malformed("disposition type is outside the catalog"))?
    {
        return Ok(false);
    }
    visiting[raw] = true;
    let declaration = &types[raw];
    // A consuming pattern may end this wrapper while leaving its children
    // live. Prepare their dispositions even when this wrapper is linear.
    for (_, field) in declaration.shape.fields_with_case() {
        if !field.relevance.is_erased()
            && let Some(child) = field.field_type.as_structural()
        {
            prepare_disposition(types, module, child, rows, visiting)?;
        }
    }
    if let terminal_psi::StructuralTypeShape::FixedArray { element, .. } = declaration.shape {
        prepare_disposition(types, module, element, rows, visiting)?;
    }
    let supported = if declaration.is_copyable() && declaration.nominal_cleanup.is_some() {
        false
    } else if let Some(selection) = &declaration.nominal_cleanup {
        let module = module.ok_or(Error::unsupported(
            "nominal disposition requires its exact module",
        ))?;
        let cleanup = terminal_psi::OwnedCleanupInvocation {
            subject: StructuralArgument::whole(
                PlaceId::new(1).ok_or(Error::malformed("zero disposition subject"))?,
                StructuralAccess::Owned,
            ),
            structural_type: ty,
            cleanup_machine: selection.cleanup_machine,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        crate::nominal_cleanup::owned_target(types.iter(), module.machines.iter(), &cleanup)
            .map_err(Error::unsupported)?;
        true
    } else if declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || declaration.owns_linear_claim
    {
        false
    } else if matches!(declaration.shape, StructuralTypeShape::TypeParameter) {
        // A `[copy]` binder permits discarding every value of its type
        // (ownership.md: "Copy and discard"). Each specialization still checks
        // its concrete type, so this grants nothing the actual type lacks.
        declaration.is_copyable()
    } else if declaration.no_code_affine_cleanup && declaration.descriptor_cleanup {
        true
    } else if let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape {
        let mut complete = true;
        let mut nontrivial_child = false;
        for field in fields {
            if field.relevance.is_erased() {
                complete = false;
                break;
            }
            if let Some(child) = field.field_type.as_structural() {
                let child_decl = types
                    .get(index(child.get())?)
                    .ok_or(Error::malformed("disposition child is outside the catalog"))?;
                nontrivial_child |= child_decl.nominal_cleanup.is_some()
                    || !child_decl.no_code_affine_cleanup
                    || !child_decl.descriptor_cleanup;
                complete &= prepare_disposition(types, module, child, rows, visiting)?;
            }
        }
        complete && nontrivial_child
    } else {
        false
    };
    visiting[raw] = false;
    rows[raw] = Some(supported);
    Ok(supported)
}

fn validate_type_permissions(types: &[StructuralTypeDeclaration]) -> Result<(), Error> {
    // A parent's permission cannot erase the custody of its represented
    // children. Check direct dependencies once; no recursive rescanning is
    // needed because every declaration in the dense catalog is visited.
    for (slot, declaration) in types.iter().enumerate() {
        if index(declaration.id.get())? != slot {
            return Err(Error::malformed("types are not in dense identity order"));
        }
        let field_children = declaration
            .shape
            .direct_fields()
            .unwrap_or(&[])
            .iter()
            .chain(
                declaration
                    .shape
                    .sum_cases()
                    .unwrap_or(&[])
                    .iter()
                    .flat_map(|case| &case.fields),
            )
            .filter_map(|field| match field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            });
        let element_child = match declaration.shape {
            terminal_psi::StructuralTypeShape::FixedArray { element, .. } => Some(element),
            _ => None,
        };
        for child in field_children.chain(element_child) {
            let child = types
                .get(index(child.get())?)
                .ok_or(Error::malformed("nested type is outside the catalog"))?;
            if declaration.is_copyable() && !child.is_copyable()
                || declaration.no_code_affine_cleanup
                    && (!child.no_code_affine_cleanup
                        || child.multiplicity == terminal_psi::StructuralMultiplicity::Linear)
                || declaration.descriptor_cleanup && !child.descriptor_cleanup
            {
                return Err(Error::malformed(
                    "parent type permissions erase a nested ownership obligation",
                ));
            }
        }
    }
    Ok(())
}

fn analyze(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    bound_output: bool,
    walks: Option<&MachineAffine<'_>>,
    reference_candidates: Option<&crate::reference_ends::Candidates>,
    claim_mode: Option<claims::Mode>,
) -> Result<Plan, Error> {
    use semantic_vocabulary::StructuralPlaceKind;
    let module = walks.map(|walks| walks.module);
    use terminal_psi::TerminalMachineResult;
    // A requirement has no activation to walk, but its signature still owns
    // exact claim rows. Prepare or verify those without inventing a body.
    if machine.blocks.is_empty() {
        let claims = claim_mode
            .map(|mode| {
                let (flow, _) = claims::Flow::new(machine, types, mode)?;
                flow.finish(machine)
            })
            .transpose()?;
        return Ok(Plan {
            claims,
            ..Plan::default()
        });
    }
    if let Some(summaries) = walks.and_then(|walks| {
        walks
            .summaries
            .filter(|summaries| std::ptr::eq(summaries.fences.module(), walks.module))
    }) {
        summaries.type_permissions.clone()?;
    } else {
        validate_type_permissions(types)?;
    }
    let mut contracts = vec![None; machine.structural_places.len()];
    // Read static whole-origin restrictions once with the custody declarations.
    // Actual loan authority and the current value remain separate judgments.
    let mut whole_restoration = vec![true; machine.structural_places.len()];
    let mut qualified = vec![Decomposition::Plain; machine.structural_places.len()];
    let mut protect = |place: PlaceId, path: Option<Vec<Segment>>| -> Result<(), Error> {
        qualified
            .get_mut(index(place.get())?)
            .ok_or(Error::malformed(
                "decomposition contract is outside the machine",
            ))?
            .protect(path);
        Ok(())
    };
    let mut exclude_whole = |place: PlaceId| -> Result<(), Error> {
        *whole_restoration
            .get_mut(index(place.get())?)
            .ok_or(Error::malformed(
                "whole-origin contract is outside the machine",
            ))? = false;
        Ok(())
    };
    let mut insert_contract = |place: PlaceId,
                               structural_type,
                               multiplicity,
                               access,
                               required_result|
     -> Result<(), Error> {
        let slot = contracts
            .get_mut(index(place.get())?)
            .ok_or(Error::malformed(
                "custody declaration is outside the machine",
            ))?;
        if slot
            .replace((structural_type, multiplicity, access, required_result))
            .is_some()
        {
            return Err(Error::malformed("a place has two custody declarations"));
        }
        Ok(())
    };
    // A block parameter may carry a fresh permission carrier on, keeping
    // the custody its producer gave it: a shared reference is affine while
    // it owns its loan although its type permits copies
    // (`required_result_multiplicity`). Every arrival must then supply such
    // a carrier whole (`validate_carried_permissions`), so the marking is never an
    // arbitrary one.
    let mut carried = Vec::new();
    for parameter in machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
    {
        if parameter.access != StructuralAccess::Owned
            || !parameter.is_unqualified_root()
            || !parameter.additional_input_qualifications.is_empty()
        {
            exclude_whole(parameter.place)?;
        }
        if parameter.access != StructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || !parameter.additional_input_qualifications.is_empty()
        {
            protect(parameter.place, None)?;
        } else {
            for row in &parameter.projected_qualifications {
                let path = crate::canonical_inline_region(types.iter(), parameter.structural_type, &row.path);
                protect(parameter.place, path)?;
            }
        }
        let required = machine
            .structural_places
            .binary_search_by_key(&parameter.place, |place| place.id)
            .ok()
            .map(|position| &machine.structural_places[position])
            .zip(
                types
                    .get(index(parameter.structural_type.get())?)
                    .filter(|declaration| declaration.id == parameter.structural_type),
            )
            .and_then(|(place, declaration)| {
                required_parameter_multiplicity(
                    place.kind,
                    parameter.access,
                    parameter.multiplicity,
                    declaration,
                )
            });
        if required.is_some() {
            carried.push(parameter.place);
        }
        insert_contract(
            parameter.place,
            parameter.structural_type,
            parameter.multiplicity,
            parameter.access,
            required,
        )?;
    }
    // Copyable storage joins the region walk when a disposition affects it.
    // Borrowed names without a release keep their existing arrival semantics.
    let mut descriptor_releases = IdSet::new();
    if let Some(candidates) = reference_candidates {
        for candidate in candidates.values().flatten() {
            descriptor_releases.insert(candidate.argument.place);
        }
    }
    let mut has_borrowed_windows = false;
    for operation in machine.operations() {
        if let terminal_psi::OperationKind::ReleaseReference { source, .. } = operation.kind {
            descriptor_releases.insert(source);
        }
        has_borrowed_windows |= matches!(
            operation.kind,
            terminal_psi::OperationKind::MoveStructuralField { .. }
        );
        if let Some(result) = operation.result.as_structural() {
            if !result.qualifications.is_empty()
                || !result.additional_input_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
            {
                protect(result.place, None)?;
            } else {
                for row in &result.projected_qualifications {
                    let path = crate::canonical_inline_region(types.iter(), result.structural_type, &row.path);
                    protect(result.place, path)?;
                }
            }
            if !(result.is_affine_plain() || result.is_unrestricted_plain())
                || !result.additional_input_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
            {
                exclude_whole(result.place)?;
            }
            let required_result = required_result_multiplicity(operation);
            if required_result.is_some() {
                // An operation-owned permission is distinct from the type's
                // later copy permission. Bind that custody to its actual
                // producer and reference declaration, never an arbitrary
                // affine marking on otherwise copyable storage.
                let place = machine.structural_places.get(index(result.place.get())?);
                let declaration = types.get(index(result.structural_type.get())?);
                if !place.is_some_and(|place| matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, structural_type }
                        if producer == operation.id && structural_type == result.structural_type))
                    || !declaration.is_some_and(|declaration| matches!(declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference { access, .. }
                            if access != StructuralAccess::Owned
                                && (!declaration.is_copyable() || access == StructuralAccess::SharedBorrow)))
                {
                    return Err(Error::malformed(
                        "permission result has no exact reference producer declaration",
                    ));
                }
            }
            insert_contract(
                result.place,
                result.structural_type,
                result.multiplicity,
                StructuralAccess::Owned,
                required_result,
            )?;
        }
        if let terminal_psi::OperationKind::EstablishByteSequenceLiteral {
            destination,
            qualifications,
            ..
        } = &operation.kind
            && !qualifications.is_empty()
        {
            exclude_whole(*destination)?;
            protect(*destination, None)?;
        }
    }
    for claim in machine
        .entry_claims
        .iter()
        .chain(&machine.block_entry_claims)
    {
        exclude_whole(claim.input)?;
    }
    for claim in &machine.content_entry_claims {
        exclude_whole(claim.input.root)?;
        let path = walks.and_then(|walks| {
            let ty = machine.structural_parameter(claim.input.root)?.structural_type;
            let authored = crate::content_transformations::structural_path(
                types.iter(), &walks.module.identities, ty, &claim.input.segments,
            )?;
            crate::canonical_inline_region(types.iter(), ty, &authored)
        });
        protect(claim.input.root, path)?;
    }
    if let TerminalMachineResult::Structural(result) = &machine.result {
        insert_contract(
            result.place,
            result.structural_type,
            result.multiplicity,
            StructuralAccess::Owned,
            None,
        )?;
    }
    carried.sort_unstable();
    validate_carried_permissions(machine, &carried, |place| {
        contracts
            .get(index(place.get()).ok()?)
            .copied()
            .flatten()
            .map(|(ty, multiplicity, access, _)| (ty, multiplicity, access))
    })
    .map_err(Error::malformed)?;
    // An empty-domain observation needs an actual initialized value even
    // when it has no scalar leaves. Follow its producers and copies through
    // the same region transfer, without converting copies into affine moves.
    let observe_copyable = machine.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            terminal_psi::Terminator::EliminateEmpty { .. }
        )
    });
    let mut roots = Vec::with_capacity(machine.structural_places.len());
    let mut copyable_owned_roots = IdSet::new();
    for place in &machine.structural_places {
        if index(place.id.get())? != roots.len() {
            return Err(Error::malformed("places are not in dense identity order"));
        }
        let mut exclusive = false;
        let (ty, owned, parameter) = match place.kind {
            StructuralPlaceKind::Parameter { .. } => {
                let (ty, _, access, _) = contracts[roots.len()]
                    .ok_or(Error::malformed("parameter place has no signature"))?;
                exclusive = access == StructuralAccess::MutableBorrow;
                (ty, access == StructuralAccess::Owned, true)
            }
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            }
            | StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            }
            | StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } => (structural_type, true, false),
            StructuralPlaceKind::Result => {
                let TerminalMachineResult::Structural(result) = &machine.result else {
                    return Err(Error::malformed("result place has no structural signature"));
                };
                (result.structural_type, false, false)
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                let parameter = machine
                    .blocks
                    .get(index(block.get())?)
                    .filter(|candidate| candidate.id == block)
                    .and_then(|block| {
                        usize::try_from(position)
                            .ok()
                            .and_then(|position| block.structural_parameters.get(position))
                    })
                    .filter(|parameter| parameter.place == place.id)
                    .ok_or(Error::malformed(
                        "block parameter place has no exact destination header",
                    ))?;
                // A block binding changes the name of the incoming storage,
                // not its owner. Borrowed referents stay outside cleanup.
                exclusive = parameter.access == StructuralAccess::MutableBorrow;
                (
                    parameter.structural_type,
                    parameter.access == StructuralAccess::Owned,
                    false,
                )
            }
            StructuralPlaceKind::ProviderAttachment { .. } => {
                return Err(Error::unsupported("affine provider storage"));
            }
        };
        let declaration = types
            .get(index(ty.get())?)
            .filter(|declaration| declaration.id == ty)
            .ok_or(Error::malformed(
                "structural type is outside its identity table",
            ))?;
        if matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
            && (!declaration.shape.is_shared_readable_byte_view()
                || !declaration.is_copyable()
                || declaration.owns_linear_claim)
        {
            return Err(Error::malformed(
                "literal backing requires a claim-free shared byte declaration",
            ));
        }
        if matches!(place.kind, StructuralPlaceKind::OperationResult { .. })
            && contracts[roots.len()].is_none()
        {
            return Err(Error::malformed(
                "operation-result place has no producing operation",
            ));
        }
        let required_result = contracts[roots.len()].and_then(|(_, _, _, required)| required);
        if let Some((declared_type, multiplicity, access, required)) = contracts[roots.len()] {
            let expected = required.unwrap_or_else(|| declared_multiplicity(declaration, access));
            if declared_type != ty || multiplicity != expected {
                return Err(Error::malformed(
                    "place custody disagrees with its exact type declaration",
                ));
            }
        }
        if contracts[roots.len()].is_none() {
            let access = StructuralAccess::Owned;
            contracts[roots.len()] =
                Some((ty, declared_multiplicity(declaration, access), access, None));
        }
        // Fresh permission ownership also owes a disposition; copyability
        // only permits later copies, not forgetting this producing root.
        let tracked = !declaration.is_copyable()
            || declaration.nominal_cleanup.is_some()
            || terminal_psi::structural_type_contains_reference_by(
                |id| terminal_psi::find_structural_type(types, id),
                ty,
            )
            || required_result.is_some_and(|multiplicity| {
                multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            });

        if owned
            && !tracked
            && declaration.is_copyable()
            && declaration.no_code_affine_cleanup
            && declaration.descriptor_cleanup
        {
            copyable_owned_roots.insert(place.id);
        }
        let (_, multiplicity, _, _) = contracts[roots.len()]
            .ok_or(Error::malformed("tracked storage has no custody contract"))?;
        // A shared descriptor may be copied, but releasing one copy
        // still vacates its own storage region.
        let descriptor_region = owned && descriptor_releases.contains(place.id);
        roots.push(
            (tracked || observe_copyable || descriptor_region).then_some(Root {
                structural_type: ty,
                owned,
                affine: tracked,
                multiplicity,
                whole_restoration: owned && whole_restoration[roots.len()],
                decomposition: if owned { std::mem::take(&mut qualified[roots.len()]) } else { Decomposition::QualifiedRoot },
                parameter,
                exclusive,
            }),
        );
    }
    // Prepare disposition facts for the actual affine type family once,
    // before transfer replay. Consumers read the completed dense rows; they
    // neither rediscover child cleanup nor maintain a second query history.
    let mut dispositions = vec![None; types.len()];
    let mut active_dispositions = vec![false; types.len()];
    for root in roots.iter().flatten().filter(|root| root.affine) {
        prepare_disposition(
            types,
            module,
            root.structural_type,
            &mut dispositions,
            &mut active_dispositions,
        )?;
    }
    // Access is independent of whether the referent has affine contents.
    // Check every successor telescope before the copyable-storage fast path.
    for block in &machine.blocks {
        let successors = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if successors.len() != arguments.len() {
            return Err(Error::malformed(
                "successor bindings have no complete edge roster",
            ));
        }
        for ((edge, target), arguments) in successors.into_iter().zip(arguments) {
            validate_successor(machine, types, &contracts, target, arguments)
                .map_err(|error| error.at(Site::Edge(edge)))?;
        }
    }
    // Copy permission does not erase an explicitly destructive window.
    if roots.iter().all(Option::is_none) && !has_borrowed_windows && claim_mode.is_none() {
        return Ok(Plan {
            copyable_owned_roots,
            ..Plan::default()
        });
    }
    let mut affine_roots = IdSet::new();
    let mut scheduled_roots = IdSet::new();
    for (slot, root) in roots.iter().enumerate() {
        let Some(root) = root else { continue };
        let place =
            PlaceId::new(slot as u64 + 1).ok_or(Error::malformed("place identity overflow"))?;
        if root.affine {
            affine_roots.insert(place);
        }
        if root.affine || copyable_owned_roots.contains(place) {
            scheduled_roots.insert(place);
        }
    }
    let regions = roots
        .iter()
        .map(|root| {
            root.as_ref()
                .filter(|root| root.parameter)
                .map(|_| Region::new(true))
        })
        .collect();
    let active = roots
        .iter()
        .enumerate()
        .filter(|(_, root)| {
            root.as_ref()
                .is_some_and(|root| root.parameter && root.owned)
        })
        .map(|(slot, _)| {
            PlaceId::new(slot as u64 + 1).ok_or(Error::malformed("place identity overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (claim_flow, initial_claims) = match claim_mode {
        Some(mode) => {
            let (flow, state) = claims::Flow::new(machine, types, mode)?;
            (Some(flow), state)
        }
        None => (None, claims::ClaimsState::default()),
    };
    let initial = State {
        regions,
        active,
        claims: initial_claims,
    };
    let first = index(machine.entry.get())?;
    if first >= machine.blocks.len() {
        return Err(Error::malformed("entry block is outside the machine"));
    }
    let needs_backing = roots.iter().flatten().any(|root| {
        terminal_psi::structural_type_contains_reference_by(
            |id| types.iter().find(|declaration| declaration.id == id),
            root.structural_type,
        )
    }) || machine.operations().any(|operation| {
        matches!(
            &operation.kind,
            terminal_psi::OperationKind::MoveStructuralField { path, .. }
                if path.contains(&StructuralPathSegment::Referent)
        )
    });
    // The active case determines which payload needs cleanup, even when
    // the type contains no references. Ordinary type-wide cleanup needs no
    // additional flow. Captured homes and reference backing remain separate.
    let decomposes = machine.operations().any(|operation| {
        matches!(
            operation.kind,
            terminal_psi::OperationKind::DecomposeStructuralValue { .. }
        )
    });
    let keeps_borrowed_storage = needs_backing
        || module.is_some()
            && roots.iter().flatten().any(|root| {
                terminal_psi::structural_type_has_borrowed_storage_by(
                    |id| types.iter().find(|declaration| declaration.id == id),
                    root.structural_type,
                )
            });
    let mut needs_storage_flow = keeps_borrowed_storage;
    if !needs_storage_flow && module.is_some() {
        for (root, place) in roots.iter().zip(&machine.structural_places) {
            let Some(root) = root.as_ref().filter(|root| root.owned && root.affine) else {
                continue;
            };
            if (decomposes || dispositions[index(root.structural_type.get())?] != Some(true))
                && crate::descriptor_results::DescriptorSlotKind::References.tracks_returned_value(
                    types,
                    place.id,
                    root.structural_type,
                )
            {
                needs_storage_flow = true;
                break;
            }
        }
    }
    let (reference_graph, mut references) = if needs_storage_flow {
        let walks = walks.ok_or(Error::unsupported(
            "conditional storage analysis requires its exact module",
        ))?;
        // A reference can borrow an element through a slice. Its residence
        // follows both the slice's backing and the resulting reference.
        let graph = match walks.graph() {
            Ok(graph) => graph,
            Err(failure) => {
                return Err(Error::Unsupported {
                    detail: failure
                        .detail()
                        .unwrap_or("reference storage has no complete closure"),
                    site: (failure.machine() == machine.id)
                        .then(|| failure.operation(walks.module))
                        .flatten()
                        .map(Site::Operation),
                });
            }
        };
        (graph, Some(walks.references(graph)))
    } else {
        (&[][..], None)
    };
    let mut graphed_machines = reference_graph
        .iter()
        .map(|graph| graph.id)
        .filter(|id| *id != machine.id)
        .collect::<Vec<_>>();
    graphed_machines.sort_unstable();
    let backing = if keeps_borrowed_storage {
        let references = references.as_mut().ok_or(Error::malformed(
            "reference backing lost its storage analysis",
        ))?;
        let graph = reference_graph
            .iter()
            .find(|graph| graph.id == machine.id)
            .ok_or(Error::malformed("reference storage lost its machine graph"))?;
        reference_backing_uses(types, graph, references, reference_candidates)?
    } else {
        Default::default()
    };
    let live = live_entries(machine, types, reference_candidates, &backing)?;
    let borrowed_headers = machine
        .block_structural_parameters()
        .filter(|parameter| parameter.access != StructuralAccess::Owned)
        .map(|parameter| parameter.place)
        .collect();
    let mut analysis = Analysis {
        module,
        fences: walks.map(MachineAffine::fences),
        types,
        dispositions,
        roots,
        borrowed_headers,
        unavailable: Vec::new(),
        unavailable_roots: Vec::new(),
        stores: Vec::new(),
        verify_stores: bound_output,
        references,
        body: Some(machine),
        residences: Vec::new(),
        position: None,
        successor: None,
        reference_candidates,
        reference_ends: Vec::new(),
        claim_flow,
    };
    if needs_backing {
        analysis.prepare_residences(
            module.ok_or(Error::malformed("a reference window has no module"))?,
        )?;
    }
    // Each block is entered with the frontier every arrival agrees on
    // (ownership.md, "Paths and control flow": predecessor paths must agree
    // on the live ownership frontier). A value one path transferred whole
    // and another still owns is not owned at the join: the path that still
    // owns it disposes of it on its edge, and a later use is a use after
    // the value was transferred. The entering frontiers only shrink, so the
    // walk reaches a fixpoint; loops are re-walked until their head agrees
    // with its back edges. Nothing the walk reports is kept until then.
    let mut entering: Vec<Option<State>> = vec![None; machine.blocks.len()];
    entering[first] = Some(initial);
    let mut pending = vec![machine.entry];
    while let Some(block_id) = pending.pop() {
        let slot = index(block_id.get())?;
        let state = entering[slot]
            .clone()
            .ok_or(Error::malformed("block has no incoming frontier"))?;
        analysis.unavailable_roots = vec![None; analysis.roots.len()];
        for arrival in analysis.walk(machine, block_id, state, &live, bound_output, None)? {
            let Some(target) = arrival.target else {
                continue;
            };
            let destination = index(target.get())?;
            let merged = match &entering[destination] {
                None => arrival.outgoing,
                Some(earlier) => earlier.meet(&arrival.outgoing)?,
            };
            if entering[destination].as_ref() != Some(&merged) {
                entering[destination] = Some(merged);
                pending.push(target);
            }
        }
    }
    analysis.unavailable.clear();
    analysis.reference_ends.clear();
    analysis.unavailable_roots = vec![None; analysis.roots.len()];
    let mut cleanup = Vec::new();
    let mut borrowed_header_ends = Vec::new();
    for block in &machine.blocks {
        let Some(state) = entering[index(block.id.get())?].clone() else {
            if let Some(flow) = &mut analysis.claim_flow {
                flow.unreachable_block(block);
            }
            continue;
        };
        for arrival in analysis.walk(
            machine,
            block.id,
            state,
            &live,
            bound_output,
            Some(&entering),
        )? {
            if let Some(target) = arrival.target
                && entering[index(target.get())?].as_ref() != Some(&arrival.outgoing)
            {
                return Err(Error::malformed(
                    "an edge leaves a frontier its successor does not enter with",
                )
                .at(Site::Edge(arrival.edge)));
            }
            if !arrival.actions.is_empty() {
                cleanup.push((arrival.edge, arrival.actions));
            }
            borrowed_header_ends.extend(
                arrival
                    .borrowed_header_ends
                    .into_iter()
                    .map(|place| (arrival.edge, place)),
            );
        }
    }
    cleanup.sort_by_key(|(edge, _)| *edge);
    borrowed_header_ends.sort_unstable();
    analysis.stores.sort_by_key(|(operation, _)| *operation);
    Ok(Plan {
        affine_roots,
        scheduled_roots,
        copyable_owned_roots,
        cleanup,
        stores: analysis.stores,
        unavailable: analysis.unavailable,
        reference_ends: analysis.reference_ends,
        borrowed_header_ends,
        claims: analysis
            .claim_flow
            .map(|flow| flow.finish(machine))
            .transpose()?,
        graphed_machines,
    })
}

/// A successor may attenuate an existing permission, never invent one from
/// the destination's desired header. Custody and referent access are separate:
/// owning a shared descriptor still grants only its retained shared access.
fn validate_successor(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    contracts: &[Option<(
        StructuralTypeId,
        terminal_psi::StructuralMultiplicity,
        StructuralAccess,
        Option<terminal_psi::StructuralMultiplicity>,
    )>],
    target: semantic_vocabulary::BlockId,
    arguments: &[StructuralArgument],
) -> Result<(), Error> {
    let target = machine
        .blocks
        .get(index(target.get())?)
        .filter(|block| block.id == target)
        .ok_or(Error::malformed("successor target is outside the machine"))?;
    if target.structural_parameters.len() != arguments.len() {
        return Err(Error::malformed(
            "structural successor arguments differ from the target telescope",
        ));
    }
    for (position, (argument, parameter)) in arguments
        .iter()
        .zip(&target.structural_parameters)
        .enumerate()
    {
        if usize::try_from(parameter.position).ok() != Some(position)
            || !terminal_psi::structural_access_covers(argument.access, parameter.access)
        {
            return Err(Error::malformed(
                "structural successor parameter has no exact access or position",
            ));
        }
        let (source, multiplicity, access, _) = contracts
            .get(index(argument.place.get())?)
            .copied()
            .flatten()
            .ok_or(Error::malformed(
                "structural successor argument has no actual source declaration",
            ))?;
        if !crate::successor_argument_carrier_matches(types, source, argument, parameter) {
            return Err(Error::malformed(
                "structural successor argument differs from its parameter type",
            ));
        }
        if parameter.access != StructuralAccess::Owned {
            let requested = if argument.access == StructuralAccess::Owned {
                if access != StructuralAccess::Owned {
                    return Err(Error::malformed(
                        "borrowed successor cannot claim ownership of its actual source",
                    ));
                }
                parameter.access
            } else {
                argument.access
            };
            if !crate::reference_source_projection_accessible(
                types.iter(),
                source,
                multiplicity,
                access,
                &argument.path,
                requested,
            ) {
                return Err(Error::malformed(
                    "borrowed successor requests access unavailable from its actual source",
                ));
            }
        }
    }
    Ok(())
}

impl<'a> Analysis<'a> {
    fn prepare_residences(&mut self, module: &terminal_psi::TerminalModule) -> Result<(), Error> {
        use crate::descriptor_lifetimes::{
            DescriptorLocation, descriptor_residence_live_positions,
        };
        let body = self
            .body
            .ok_or(Error::malformed("a residence has no machine"))?;
        let mut rules = crate::descriptor_results::terminal_residence_rules(module, body).ok_or(
            Error::unsupported("captured storage has no complete residence rules"),
        )?;
        let flow = self
            .references
            .as_mut()
            .ok_or(Error::malformed("a residence has no reference analysis"))?
            .residence_flow(body.id, &mut rules)
            .map_err(|failure| Error::Unsupported {
                detail: failure
                    .detail()
                    .unwrap_or("captured storage has no complete residence flow"),
                site: (failure.machine() == body.id)
                    .then(|| failure.operation(module))
                    .flatten()
                    .map(Site::Operation),
            })?;
        let mut births = vec![None; self.roots.len()];
        let mut dispositions = std::collections::BTreeSet::new();
        for parameter in &body.structural_parameters {
            births[index(parameter.place.get())?] = Some(((body.entry, 0), true));
        }
        for block in &body.blocks {
            for parameter in &block.structural_parameters {
                births[index(parameter.place.get())?] = Some(((block.id, 0), true));
            }
            if matches!(
                block.terminator,
                terminal_psi::Terminator::ReturnStructural { .. }
            ) {
                dispositions.insert((
                    block.id,
                    u32::try_from(block.operations.len())
                        .map_err(|_| Error::malformed("residence terminal position overflows"))?,
                ));
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if matches!(
                    operation.kind,
                    terminal_psi::OperationKind::ReleaseReference { .. }
                ) {
                    dispositions.insert((
                        block.id,
                        u32::try_from(ordinal).map_err(|_| {
                            Error::malformed("residence release position overflows")
                        })?,
                    ));
                }
                if let Some(result) = operation.result.as_structural() {
                    births[index(result.place.get())?] = Some((
                        (
                            block.id,
                            u32::try_from(ordinal)
                                .map_err(|_| Error::malformed("residence position overflows"))?,
                        ),
                        false,
                    ));
                }
            }
        }
        let mut captured = rules
            .captures
            .iter()
            .map(|(_, source, _)| source.clone())
            .collect::<Vec<_>>();
        captured.sort();
        captured.dedup();
        let mut residences = (0..self.roots.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for source in captured {
            let Some(root) = self.root(source.root)?.filter(|root| root.owned) else {
                continue;
            };
            let raw = index(source.root.get())?;
            let (birth, at_entry) = births[raw].ok_or(Error::unsupported(
                "a captured home has no exact establishment",
            ))?;
            let path = crate::descriptor_lifetimes::descriptor_path(
                self.types,
                root.structural_type,
                &source.path,
            )
            .and_then(|path| {
                crate::canonical_inline_region(self.types.iter(), root.structural_type, &path)
            })
            .ok_or(Error::unsupported(
                "a captured home has no exact inline region",
            ))?;
            let live = descriptor_residence_live_positions(
                &flow,
                &rules,
                birth,
                &source,
                at_entry,
                |at, _: &DescriptorLocation| dispositions.contains(&at),
            );
            residences[raw].push(Residence { path, live });
        }
        self.residences = residences;
        Ok(())
    }

    /// The storage holding an open borrowed-storage window at the current
    /// position, if any: a home captured by a live reference whose captured
    /// region is not available, or a lent affine parameter that is not whole.
    /// Every fence on an open window asks this one question.
    fn open_window(&self, state: &State) -> Result<Option<PlaceId>, Error> {
        let raw = self
            .residences
            .iter()
            .enumerate()
            .find(|(raw, captures)| {
                captures.iter().any(|capture| {
                    self.position.is_some_and(|at| capture.live.contains(&at))
                        && state.regions[*raw]
                            .as_ref()
                            .is_none_or(|region| !region.available(&capture.path))
                })
            })
            .map(|(raw, _)| raw)
            .or_else(|| {
                self.roots.iter().enumerate().find_map(|(raw, root)| {
                    (root
                        .as_ref()
                        .is_some_and(|root| root.parameter && !root.owned && root.affine)
                        && state.regions[raw]
                            .as_ref()
                            .is_some_and(|region| !region.available(&[])))
                    .then_some(raw)
                })
            });
        raw.map(|raw| {
            u64::try_from(raw)
                .ok()
                .and_then(|raw| raw.checked_add(1))
                .and_then(PlaceId::new)
                .ok_or(Error::malformed("place position is outside its identity"))
        })
        .transpose()
    }

    /// A call made while a window is open, unless the callee's contract
    /// rules out observing the incomplete owner (`fences`). The arguments
    /// are judged by the ordinary use check of the call itself. Without the
    /// module the callee's contract is unknown, and the call is not served.
    fn fenced(
        &self,
        state: &State,
        reach: fences::Reach,
        site: Site,
    ) -> Result<Option<Unavailable>, Error> {
        if matches!(reach, fences::Reach::Plain) {
            return Ok(None);
        }
        let Some(place) = self.open_window(state)? else {
            return Ok(None);
        };
        let fences = || {
            self.fences.ok_or(
                Error::unsupported(
                    "a call while an owned value is out of borrowed storage needs its exact module",
                )
                .at(site),
            )
        };
        let fence = match reach {
            fences::Reach::Plain => None,
            fences::Reach::Calls(callee) => fences()?.machine(callee),
            fences::Reach::Boundary(boundary) => fences()?.boundary(boundary),
            fences::Reach::Fenced(fence) => Some(fence),
        };
        Ok(fence.map(|fence| Unavailable {
            site,
            place,
            reason: UnavailableReason::FencedCall(fence),
        }))
    }

    /// Returning a reference returns the captured region, rather than just
    /// disposing a pointer-sized carrier. Availability is read from the same
    /// frontier that the extraction and receiving store update.
    fn restore_reference(
        &mut self,
        state: &State,
        source: PlaceId,
        path: &[StructuralPathSegment],
        site: Site,
    ) -> Result<(), Error> {
        if self.references.is_none() {
            return Ok(());
        }
        let mut referent = path.to_vec();
        referent.push(StructuralPathSegment::Referent);
        let backings = self.backings(state, source, &referent, site)?;
        for (root, captured) in backings.targets() {
            if self.root(*root)?.is_none() {
                continue;
            }
            let (captured, inline) = self.observed(*root, captured)?;
            // Indirect storage cannot hold a tracked vacancy. Its loan
            // still has to pass the reference checker's restoration rules.
            if inline
                && !state.regions[index(root.get())?]
                    .as_ref()
                    .is_some_and(|region| region.available(&captured))
            {
                self.unavailable.push(Unavailable {
                    site,
                    place: *root,
                    reason: UnavailableReason::Unrestored,
                });
            }
        }
        Ok(())
    }

    fn end_references(
        &mut self,
        state: &mut State,
        at: crate::descriptor_lifetimes::DescriptorPosition,
        site: Site,
    ) -> Result<(), Error> {
        let Some(candidates) = self
            .reference_candidates
            .and_then(|candidates| candidates.get(&at))
        else {
            return Ok(());
        };
        for candidate in candidates {
            let argument = &candidate.argument;
            let Some(root) = self.root(argument.place)?.filter(|root| root.owned) else {
                continue;
            };
            let structural_type = root.structural_type;
            let path = self.canonical(argument.place, &argument.path)?;
            if !state.regions[index(argument.place.get())?]
                .as_ref()
                .is_some_and(|region| region.available(&path))
            {
                continue;
            }
            // An intact nominal wrapper still owes its complete selected
            // consumer. Ending one dead descriptor would create an unauthorized
            // partial value before that ordinary owned call can receive it.
            if !self.nominal_ancestors_partitioned(state, argument.place, &path)? {
                continue;
            }
            if candidate.unproved_case.is_some() {
                return Err(Error::unsupported(
                    "last-use reference disposition needs an established active case",
                ));
            }
            self.restore_reference_leaves(state, argument.place, &argument.path, site)?;
            self.vacate(state, argument.place, &path)?;
            self.reference_ends.push(ReferenceEnd {
                at,
                source: argument.clone(),
                structural_type,
            });
        }
        Ok(())
    }

    fn restore_reference_leaves(
        &mut self,
        state: &State,
        source: PlaceId,
        path: &[StructuralPathSegment],
        site: Site,
    ) -> Result<(), Error> {
        if self.references.is_none() {
            return Ok(());
        }
        let body = self
            .body
            .ok_or(Error::malformed("reference closure has no machine"))?;
        let root_type = body
            .structural_place_type(source)
            .ok_or(Error::malformed("reference closure has no carrier type"))?;
        let selected = match crate::runtime_read_projection_tip(self.types.iter(), root_type, path)
        {
            Some(crate::Position::Declared(ty)) => ty,
            Some(crate::Position::ScalarField(_) | crate::Position::ByteElement) => return Ok(()),
            None => {
                return Err(Error::unsupported(
                    "reference closure has no selected carrier type",
                ));
            }
        };
        let inventory = crate::ReferenceInventory::for_restoration(self.types, body, source, path, selected).ok_or(
            Error::unsupported("reference closure has no complete contents inventory"),
        )?;
        for unknown in inventory.unknown_contents {
            let mut carrier = path.to_vec();
            carrier.extend(unknown.path);
            let carrier = self.canonical(source, &carrier)?;
            // Hidden references cannot be opened through this carrier. Keep
            // the entire value established when handing it on; a whole move
            // does not claim that its referents were individually released.
            if !state.regions[index(source.get())?]
                .as_ref()
                .is_some_and(|region| region.available(&carrier))
            {
                self.unavailable.push(Unavailable {
                    site,
                    place: source,
                    reason: UnavailableReason::Unrestored,
                });
            }
        }
        for leaf in inventory.references {
            let declaration = self
                .types
                .get(index(leaf.structural_type.get())?)
                .filter(|declaration| declaration.id == leaf.structural_type)
                .ok_or(Error::malformed(
                    "reference closure lost the leaf declaration",
                ))?;
            // Only a mutable loan can leave a hole. Shared byte recasts
            // retain the lender's storage through loan checking, without
            // turning their alternate presentation into an owned path.
            if !matches!(
                declaration.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::MutableBorrow,
                    ..
                }
            ) {
                continue;
            }
            let mut exact = path.to_vec();
            exact.extend(leaf.path);
            self.restore_reference(state, source, &exact, site)?;
        }
        Ok(())
    }

    /// Walks one block from `state` and returns its normal edges, each with
    /// the frontier it carries and the disposals it performs. An unread root
    /// is disposed of only when its remaining value permits implicit cleanup;
    /// other obligations remain live across internal edges. With
    /// `entering`, the fixpoint's frontiers, a root the successor reads but
    /// does not own is disposed of too: another arrival transferred it.
    fn walk(
        &mut self,
        machine: &TerminalMachine,
        block_id: semantic_vocabulary::BlockId,
        mut state: State,
        live: &[IdSet<PlaceId>],
        bound_output: bool,
        entering: Option<&[Option<State>]>,
    ) -> Result<Vec<Arrival>, Error> {
        use terminal_psi::Terminator;
        self.successor = None;
        let block = machine
            .blocks
            .get(index(block_id.get())?)
            .filter(|block| block.id == block_id)
            .ok_or(Error::malformed("block is not at its identity"))?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let at = (
                block.id,
                u32::try_from(ordinal)
                    .map_err(|_| Error::malformed("operation position overflows"))?,
            );
            self.position = Some(at);
            let site = Site::Operation(operation.id);
            self.end_references(&mut state, at, site)
                .map_err(|error| error.at(site))?;
            // "Suspension must retain the exclusive loan, exact hole,
            // removed-value custody, and restoration obligation without
            // exposing the incomplete owner. Its resume and cancellation
            // paths must satisfy their ordinary carry and cleanup contracts;
            // otherwise suspension with the window open rejects." A
            // suspension plan states none of that yet, so a crossing reached
            // with lent storage partly vacant is not served.
            if operation.suspension_crossing.is_some() && self.open_window(&state)?.is_some() {
                return Err(Error::unsupported(
                    "a possibly suspending call while an owned value is out of borrowed storage",
                )
                .at(site));
            }
            // Judged on the frontier the call starts from, reported after
            // the call's own argument uses, which name the storage directly.
            let fenced = self.fenced(&state, fences::operation(&operation.kind), site)?;
            self.operation(&mut state, operation, entering.is_some())
                .map_err(|error| error.at(site))?;
            self.unavailable.extend(fenced);
        }
        let at = (
            block.id,
            u32::try_from(block.operations.len())
                .map_err(|_| Error::malformed("terminal position overflows"))?,
        );
        self.position = Some(at);
        if let Terminator::PendingNormalCompletion { call, storage, .. } = &block.terminator {
            let site = Site::Operation(*call);
            // Earlier ownership preparation runs before crossings are assigned.
            // The final checker requires the crossing on this exact selected call.
            if block
                .operations
                .last()
                .is_none_or(|op| op.id != *call || op.declared_invocation().is_none())
            {
                return Err(
                    Error::malformed("the open tail lost its checked suspending call").at(site),
                );
            }
            if state.claims.live().next().is_some() {
                return Err(
                    Error::unsupported("an open Build tail with a live logical claim").at(site),
                );
            }
            for &place in storage {
                self.read(&state, place, &[], site)?;
            }
            // A retained owner will be checked again by the successor. Never
            // forget a linear or nontrivially disposed home at a missing exit.
            for &place in &state.active {
                if storage.binary_search(&place).is_ok() {
                    continue;
                }
                let root = self.root(place)?.ok_or_else(|| {
                    Error::malformed("a pending Build suffix has an undeclared active home")
                        .at(site)
                })?;
                if root.owned {
                    let ty = terminal_psi::find_structural_type(self.types, root.structural_type)
                        .ok_or_else(|| {
                        Error::malformed("a pending Build home has no declared type").at(site)
                    })?;
                    if root.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                        || ty.nominal_cleanup.is_some()
                        || !ty.no_code_affine_cleanup
                    {
                        return Err(Error::unsupported(
                            "a pending Build suffix must retain an outstanding owned home",
                        )
                        .at(site));
                    }
                }
            }
            return Ok(Vec::new());
        }
        let site = Site::Edge(
            block
                .terminator
                .edges()
                .next()
                .ok_or(Error::malformed("terminator has no edge"))?,
        );
        let fenced = self.fenced(&state, fences::terminator(&block.terminator), site)?;
        self.unavailable.extend(fenced);
        if let Some(flow) = &mut self.claim_flow {
            flow.terminal(&mut state.claims, &block.terminator, entering.is_some())
                .map_err(|error| error.at(site))?;
        }
        // End dead descriptors before a successor borrows their lenders.
        // The flow observes every edge argument here, so a descriptor
        // transferred or used on any successor stays available.
        if matches!(
            block.terminator,
            Terminator::Jump { .. }
                | Terminator::Conditional { .. }
                | Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::StructuralCase { .. }
                | Terminator::EliminateEmpty { .. }
        ) {
            self.end_references(&mut state, at, site)?;
        }
        match &block.terminator {
            Terminator::PendingNormalCompletion { .. } => unreachable!("handled before selecting an edge"),
            Terminator::StructuralCase { source, path, .. } => {
                self.argument(
                    &mut state,
                    &StructuralArgument {
                        place: *source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    site,
                    false,
                )?;
            }
            Terminator::EliminateEmpty { source, path, .. } => {
                let observed = crate::inhabitation::observed_storage_path(path);
                self.argument(
                    &mut state,
                    &StructuralArgument {
                        place: *source,
                        path: observed.to_vec(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    site,
                    false,
                )?;
            }
            Terminator::ReturnStructural { source, .. } => {
                self.argument(
                    &mut state,
                    &StructuralArgument::whole(*source, StructuralAccess::Owned),
                    site,
                    true,
                )?;
            }
            Terminator::InvokeNoNormal {
                structural_arguments,
                ..
            }
            | Terminator::ExternalCompletion {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    self.restore_reference_leaves(&state, argument.place, &argument.path, site)?;
                    self.argument(&mut state, argument, site, true)?;
                }
            }
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => {}
        }
        if matches!(
            block.terminator,
            Terminator::Crash { .. }
                | Terminator::EliminateEmpty { .. }
                | Terminator::InvokeNoNormal { .. }
                | Terminator::ExternalCompletion { .. }
        ) {
            return Ok(Vec::new());
        }
        // Edge IDs are machine-wide and untrusted here. Index only the
        // actual local successors, never allocate up to a serialized raw ID.
        let successors = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        if successors.len() != arguments.len() {
            return Err(Error::malformed(
                "successor bindings have no complete edge roster",
            ));
        }
        let mut targets = successors
            .into_iter()
            .zip(arguments)
            .map(|((edge, target), arguments)| (edge, target, arguments))
            .collect::<Vec<_>>();
        targets.sort_by_key(|(edge, _, _)| *edge);
        if targets.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::malformed("successor edge has two binding rosters"));
        }
        let rosters = block.terminator.cleanup_rosters().collect::<Vec<_>>();
        let mut arrivals = Vec::with_capacity(rosters.len());
        for (ordinal, (edge, supplied)) in rosters.iter().copied().enumerate() {
            let mut residual_budget = if bound_output {
                supplied
                    .iter()
                    .filter(|action| {
                        matches!(
                            action,
                            TerminalAffineCleanupAction::DiscardResidual(_)
                                | TerminalAffineCleanupAction::InvokeOwned(_)
                        )
                    })
                    .count()
            } else {
                usize::MAX
            };
            let mut outgoing = if ordinal + 1 == rosters.len() {
                std::mem::take(&mut state)
            } else {
                state.clone()
            };
            if let Some(flow) = &self.claim_flow {
                flow.select_case(&mut outgoing.claims, &block.terminator, edge)?;
            }
            let successor = targets
                .binary_search_by_key(&edge, |(edge, _, _)| *edge)
                .ok()
                .map(|position| targets[position]);
            self.successor = successor.map(|_| (ordinal, edge));
            let target = successor.map(|(_, target, _)| target);
            if let Some((_, target, arguments)) = successor {
                self.capture_successor(machine, &mut outgoing, edge, target, arguments)
                    .map_err(|error| error.at(Site::Edge(edge)))?;
            }
            let needed = target
                .map(|target| index(target.get()).map(|index| &live[index]))
                .transpose()?;
            let owned_there = match (entering, target) {
                (Some(entering), Some(target)) => Some(
                    entering
                        .get(index(target.get())?)
                        .and_then(Option::as_ref)
                        .ok_or(Error::malformed("edge target has no entering frontier"))?,
                ),
                _ => None,
            };
            // ownership.md: "Every return or terminal transfer from the
            // region owing restoration ... or restoration of the borrow to
            // its parent must close the window." An edge with no target
            // leaves the machine, where every loan it was lent ends, so
            // lent storage with a part still vacant there is owed back
            // incomplete. "one repaired branch does not repair another":
            // the frontier of a join is what every arrival holds.
            if target.is_none() {
                for (raw, root) in self.roots.iter().enumerate() {
                    let lent = root
                        .as_ref()
                        .is_some_and(|root| root.parameter && !root.owned && root.affine);
                    if lent
                        && outgoing.regions[raw]
                            .as_ref()
                            .is_some_and(|region| !region.available(&[]))
                    {
                        let place = u64::try_from(raw)
                            .ok()
                            .and_then(|raw| raw.checked_add(1))
                            .and_then(PlaceId::new)
                            .ok_or(Error::malformed("place position is outside its identity"))?;
                        self.unavailable.push(Unavailable {
                            site: Site::Edge(edge),
                            place,
                            reason: UnavailableReason::Unrestored,
                        });
                    }
                }
            }
            // A borrowed header names captured storage; it does not own a
            // value that needs cleanup. End a dead name before the next header
            // is bound, leaving its backing and all loan checks untouched.
            // A surviving use still needs a name established on every arrival.
            let mut borrowed_header_ends = Vec::new();
            for place in &self.borrowed_headers {
                let raw = index(place.get())?;
                let unused = !needed.is_some_and(|needed| needed.contains(*place));
                let forgotten =
                    unused || owned_there.is_some_and(|there| there.regions[raw].is_none());
                if forgotten && let Some(region) = &outgoing.regions[raw] {
                    if !region.available(&[]) {
                        return Err(Error::unsupported(
                            "borrowed block storage still owes restoration at its last use",
                        )
                        .at(Site::Edge(edge)));
                    }
                    if unused && target.is_some() && bound_output && entering.is_some() {
                        borrowed_header_ends.push(*place);
                    }
                    outgoing.regions[raw] = None;
                }
            }
            // "At reconvergence, predecessors must agree on the exact live
            // ownership and open restoration obligations." An owned root an
            // arrival holds more of is made equal on its edge by discarding
            // the difference; lent storage is not this machine's to discard,
            // so an arrival that differs from the settled frontier of its
            // target in lent storage is outside what this schedule serves.
            if let Some(there) = owned_there {
                for (raw, root) in self.roots.iter().enumerate() {
                    if root
                        .as_ref()
                        .is_some_and(|root| root.parameter && !root.owned && root.affine)
                        && outgoing.regions[raw] != there.regions[raw]
                    {
                        return Err(Error::unsupported(
                            "arrivals that disagree on a window open in borrowed storage",
                        )
                        .at(Site::Edge(edge)));
                    }
                }
            }
            if let (Some(there), Some(target)) = (owned_there, target) {
                for (raw, captures) in self.residences.iter().enumerate() {
                    for capture in captures
                        .iter()
                        .filter(|capture| capture.live.contains(&(target, 0)))
                    {
                        let held = outgoing.regions[raw]
                            .as_ref()
                            .map(|region| region.view(&capture.path));
                        let joined = there.regions[raw]
                            .as_ref()
                            .map(|region| region.view(&capture.path));
                        if held != joined {
                            return Err(Error::unsupported(
                                "arrivals that disagree on a window held by a live reference",
                            )
                            .at(Site::Edge(edge)));
                        }
                    }
                }
            }
            let mut explicit_retirements = IdSet::new();
            for action in supplied {
                if let TerminalAffineCleanupAction::RetireHome(place) = action
                    && self
                        .root(*place)?
                        .is_some_and(|root| root.owned && !root.affine)
                {
                    explicit_retirements.insert(*place);
                }
            }
            let mut actions = Vec::new();
            let mut retained = Vec::new();
            for place in std::mem::take(&mut outgoing.active).into_iter().rev() {
                let raw = index(place.get())?;
                if let Some(region) = &outgoing.regions[raw] {
                    let root = self
                        .root(place)?
                        .ok_or(Error::malformed("allocated home has no ownership row"))?;
                    if !root.affine {
                        // A copied descriptor still keeps its own loan alive.
                        // Retire its dead home at its establishment position,
                        // before an older backing owner can be disposed.
                        let explicit = explicit_retirements.contains(place);
                        let retired_at_join = !bound_output
                            && owned_there.is_some_and(|there| there.regions[raw].is_none())
                            && root.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted;
                        if explicit || retired_at_join {
                            actions.push(TerminalAffineCleanupAction::RetireHome(place));
                            outgoing.regions[raw] = None;
                        } else {
                            retained.push(place);
                        }
                        continue;
                    }
                    let pinned = target.is_some_and(|target| {
                        self.residences.get(raw).is_some_and(|regions| {
                            regions
                                .iter()
                                .any(|region| region.live.contains(&(target, 0)))
                        })
                    });
                    let read_there = pinned || needed.is_some_and(|needed| needed.contains(place));
                    let there = owned_there.map(|there| there.regions[raw].as_ref());
                    let still_owed = !read_there
                        && target.is_some()
                        && self
                            .needs_explicit_disposition(place, region)
                            .map_err(|error| error.at(Site::Edge(edge)))?;
                    if (read_there || still_owed) && there.is_none_or(|there| there.is_some()) {
                        if let Some(Some(there)) = there
                            && there != region
                        {
                            let discards = self
                                .equalize(place, region, there, residual_budget)
                                .map_err(|error| error.at(Site::Edge(edge)))?;
                            for action in &discards {
                                self.restore_reference_leaves(
                                    &outgoing,
                                    action.place(),
                                    action.path(),
                                    Site::Edge(edge),
                                )?;
                            }
                            residual_budget = residual_budget.checked_sub(discards.len()).ok_or(
                                Error::malformed("affine residual partition exceeds its output"),
                            )?;
                            actions.extend(discards);
                            outgoing.regions[raw] = Some(there.clone());
                        }
                        retained.push(place);
                    } else {
                        let disposal = self
                            .dispose(place, region, residual_budget)
                            .map_err(|error| error.at(Site::Edge(edge)))?;
                        for action in &disposal {
                            if !matches!(action, TerminalAffineCleanupAction::RetireHome(_)) {
                                self.restore_reference_leaves(
                                    &outgoing,
                                    action.place(),
                                    action.path(),
                                    Site::Edge(edge),
                                )?;
                            }
                        }
                        residual_budget -= disposal
                            .iter()
                            .filter(|action| {
                                matches!(
                                    action,
                                    TerminalAffineCleanupAction::DiscardResidual(_)
                                        | TerminalAffineCleanupAction::InvokeOwned(_)
                                )
                            })
                            .count();
                        actions.extend(disposal);
                        outgoing.regions[raw] = None;
                    }
                }
            }
            retained.reverse();
            outgoing.active = retained;
            // Owned cleanup on this edge calls its cleanup machine while
            // any window the edge carries is still open.
            for action in &actions {
                if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                    let fenced = self.fenced(
                        &outgoing,
                        fences::Reach::Calls(cleanup.cleanup_machine),
                        Site::Edge(edge),
                    )?;
                    self.unavailable.extend(fenced);
                }
            }
            if let Some(flow) = &mut self.claim_flow {
                flow.cleanup(&mut outgoing.claims, edge, supplied, entering.is_some())
                    .map_err(|error| error.at(Site::Edge(edge)))?;
                if target.is_none() {
                    outgoing
                        .claims
                        .require_empty_return()
                        .map_err(|error| error.at(Site::Edge(edge)))?;
                }
            }
            if let Some(target) = target {
                self.establish_successor(machine, &mut outgoing, target)
                    .map_err(|error| error.at(Site::Edge(edge)))?;
            }
            arrivals.push(Arrival {
                edge,
                target,
                outgoing,
                actions,
                borrowed_header_ends,
            });
        }
        Ok(arrivals)
    }

    /// Capture every selected successor operand before disposing the old
    /// frontier. Binding destination names happens only after that cleanup,
    /// including when an edge reuses its source names or swaps two owners.
    fn capture_successor(
        &mut self,
        machine: &TerminalMachine,
        state: &mut State,
        edge: EdgeId,
        target: semantic_vocabulary::BlockId,
        arguments: &[StructuralArgument],
    ) -> Result<(), Error> {
        if let Some(flow) = &mut self.claim_flow {
            flow.successor(&mut state.claims, machine, target, arguments)?;
        }
        let target = machine
            .blocks
            .get(index(target.get())?)
            .filter(|block| block.id == target)
            .ok_or(Error::malformed("successor target is outside the machine"))?;
        for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
            self.argument(
                state,
                argument,
                Site::Edge(edge),
                parameter.access == StructuralAccess::Owned,
            )?;
        }
        Ok(())
    }

    fn establish_successor(
        &self,
        machine: &TerminalMachine,
        state: &mut State,
        target: semantic_vocabulary::BlockId,
    ) -> Result<(), Error> {
        let target = machine
            .blocks
            .get(index(target.get())?)
            .filter(|block| block.id == target)
            .ok_or(Error::malformed("successor target is outside the machine"))?;
        // No surviving obligation may be overwritten by a destination name.
        // Check the whole telescope before installing any of its values.
        for parameter in &target.structural_parameters {
            if self
                .root(parameter.place)?
                .is_some_and(|root| root.owned && root.affine)
                && state.regions[index(parameter.place.get())?].is_some()
            {
                return Err(Error::malformed(
                    "structural successor overwrites an allocated owner",
                ));
            }
        }
        for parameter in &target.structural_parameters {
            self.establish(state, parameter.place, true)?;
        }
        Ok(())
    }

    fn establish(&self, state: &mut State, place: PlaceId, initialized: bool) -> Result<(), Error> {
        let Some(root) = self.root(place)? else {
            return Ok(());
        };
        let region = &mut state.regions[index(place.get())?];
        if root.owned && region.is_none() {
            state.active.push(place);
        }
        *region = Some(Region::new(initialized));
        Ok(())
    }

    fn storage_use(
        &mut self,
        state: &State,
        used: terminal_psi::StructuralStorageUse<'_>,
        site: Site,
    ) -> Result<(), Error> {
        use terminal_psi::StructuralStorageUse;
        match used {
            StructuralStorageUse::Place(root) => self.read(state, root, &[], site),
            // An element observation retains its descriptor, just like a
            // length read. The descriptor owns no elements; bounds and backing
            // authority are checked by their separate storage and loan judgments.
            StructuralStorageUse::Projection(projection)
            | StructuralStorageUse::Element {
                descriptor: projection,
                ..
            } => {
                if self.root(projection.root)?.is_none() {
                    return Ok(());
                }
                // Field operands retain a final selector outside their common
                // source path. Reading the parent instead would require moved
                // siblings to be live and would lose this exact observation.
                let selected = projection
                    .field
                    .map(|field| {
                        let root = self
                            .root(projection.root)?
                            .ok_or(Error::malformed("tracked projection lost its root"))?;
                        let field = crate::projected_field_declaration(
                            self.types.iter(),
                            root.structural_type,
                            projection.path,
                            field,
                        )
                        .ok_or(Error::malformed(
                            "storage projection lost its exact declared field",
                        ))?;
                        let mut path = projection.path.to_vec();
                        path.push(StructuralPathSegment::Field(field.identity));
                        Ok::<_, Error>(path)
                    })
                    .transpose()?;
                let (path, _) = self.observed(
                    projection.root,
                    selected.as_deref().unwrap_or(projection.path),
                )?;
                self.read(state, projection.root, &path, site)
            }
            StructuralStorageUse::Canonical { root, path }
            | StructuralStorageUse::ObservedField { root, path, .. } => {
                self.read(state, root, path, site)
            }
            StructuralStorageUse::Type(_) => Ok(()),
        }
    }

    fn operation(
        &mut self,
        state: &mut State,
        operation: &terminal_psi::Operation,
        settled: bool,
    ) -> Result<(), Error> {
        use terminal_psi::OperationKind as Op;
        let site = Site::Operation(operation.id);
        if let Op::DecomposeStructuralValue { subject } = &operation.kind {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(Error::malformed(
                    "nominal decomposition has a non-Unit result",
                ));
            }
            let module = self.module.ok_or(Error::unsupported(
                "nominal decomposition requires its exact module",
            ))?;
            let current = self
                .body
                .ok_or(Error::malformed("decomposition has no machine"))?
                .id;
            crate::nominal_cleanup::decomposition(module, current, subject)
                .map_err(Error::unsupported)?;
            let root = self.root(subject.place)?.ok_or(Error::unsupported(
                "decomposition requires tracked owned storage",
            ))?;
            if !root.owned || !root.affine {
                return Err(Error::unsupported(
                    "decomposition requires owned affine custody",
                ));
            }
            let path = self.canonical(subject.place, &subject.path)?;
            if root.decomposition.requires_conservation_at(&path) {
                let machine = self.body.ok_or(Error::malformed("decomposition has no machine"))?;
                crate::content_transformations::partition::require_decomposition_laws(module, machine, subject)
                    .map_err(Error::unsupported)?;
                if self.claim_flow.is_some() && !state.claims.original_content_at(machine, subject) {
                    return Err(Error::unsupported("qualified decomposition lost its original content claim"));
                }
            }
            // Independent claims and qualifications are not a wrapper's disposition.
            // The bounded record operation serves no implicit claim settlement.
            if !self.nominal_ancestors_partitioned(state, subject.place, &path)? {
                return Err(Error::unsupported(
                    "decomposition crosses an intact nominal wrapper",
                ));
            }
            let region = state.regions[index(subject.place.get())?]
                .as_mut()
                .ok_or(Error::malformed("decomposition source home has ended"))?;
            if !region.available(&path) {
                return Err(Error::unsupported(
                    "decomposition source is not a complete live value",
                ));
            }
            region.partition(&path);
        }
        if let Some(flow) = &mut self.claim_flow {
            flow.operation(
                &mut state.claims,
                operation,
                self.types,
                &self.roots,
                settled,
            )?;
        }
        if matches!(operation.kind, Op::DecomposeStructuralValue { .. }) {
            return Ok(());
        }
        for argument in terminal_psi::call_structural_arguments(&operation.kind) {
            self.restore_reference_leaves(state, argument.place, &argument.path, site)?;
        }
        if let Op::StructuralLeafCopy { source, path } = &operation.kind
            && let Some(root) = self.root(*source)?
            && crate::canonical_static_projection(self.types.iter(), root.structural_type, path)
                .is_some_and(|(_, declaration)| !declaration.is_copyable())
        {
            return Err(Error::malformed("a leaf copy duplicates an affine region"));
        }
        match &operation.kind {
            Op::MoveStructuralField {
                source,
                path,
                field,
            } => {
                if path.contains(&StructuralPathSegment::Referent) {
                    let declared = self
                        .body
                        .and_then(|body| body.structural_place_type(*source))
                        .ok_or(Error::unsupported(
                            "a borrowed extraction has no reference carrier type",
                        ))?;
                    for (position, segment) in path.iter().enumerate() {
                        if *segment == StructuralPathSegment::Referent
                            && !crate::runtime_structural_path_tip(
                                self.types.iter(),
                                declared,
                                &path[..position],
                            )
                            .is_some_and(|row| {
                                matches!(
                                    row.shape,
                                    terminal_psi::StructuralTypeShape::Reference {
                                        access: StructuralAccess::MutableBorrow,
                                        ..
                                    }
                                )
                            })
                        {
                            return Err(Error::malformed(
                                "a borrowed extraction crosses a nonexclusive reference",
                            ));
                        }
                    }
                }
                // The extracted value transfers its own custody. Its original
                // exclusive loan retains the same storage for handback, so an
                // empty path is a vacancy, not an ended owned home.
                let (source, path) = self.backing(state, *source, path, site)?;
                let root = self.root(source)?.ok_or(Error::unsupported(
                    "a borrowed move requires tracked storage contents",
                ))?;
                if !root.owned && !root.exclusive {
                    return Err(Error::unsupported(
                        "a borrowed move requires its incoming mutable loan",
                    ));
                }
                if root.owned {
                    let terminal_psi::OperationKind::MoveStructuralField {
                        path: carrier_path,
                        field,
                        ..
                    } = &operation.kind
                    else {
                        unreachable!()
                    };
                    if !carrier_path.contains(&StructuralPathSegment::Referent) {
                        return Err(Error::unsupported(
                            "an owned storage window requires an exclusive reference crossing",
                        ));
                    }
                    if !crate::structural_window_selects_child(carrier_path, *field)
                        && !root.whole_restoration
                    {
                        return Err(Error::unsupported(
                            "whole-referent restoration requires unqualified nonlinear owned storage without claims",
                        ));
                    }
                }
                let declared = root.structural_type;
                let mut selected = self.canonical(source, &path)?;
                if let Some(field) = field {
                    selected.push(Segment::Field(*field));
                }
                let selected_type =
                    crate::canonical_structural_path_tip(self.types.iter(), declared, &selected)
                        .ok_or(Error::malformed("a borrowed move has no selected type"))?;
                if operation
                    .result
                    .as_structural()
                    .map(|result| result.structural_type)
                    != Some(selected_type.id)
                {
                    return Err(Error::malformed(
                        "a borrowed move changes its selected type",
                    ));
                }
                if !self.nominal_ancestors_partitioned(state, source, &selected)? {
                    return Err(Error::unsupported(
                        "projected owned transfer crosses an intact nominal wrapper",
                    ));
                }
                self.read(state, source, &selected, site)?;
                // The value leaves, but the authenticated reference still
                // owns access to this allocated home until repair and release.
                self.vacate_contents(state, source, &selected)?;
            }
            Op::ScalarStructuralField {
                source,
                path,
                field,
                ..
            }
            | Op::BooleanStructuralField {
                source,
                path,
                field,
            } => {
                let mut path = path.clone();
                path.push(Segment::Field(*field));
                self.read(state, *source, &path, site)?;
            }
            Op::StructuralByteSequenceFieldRead {
                source,
                path,
                field,
                ..
            }
            | Op::StructuralByteSequenceFieldLength {
                source,
                path,
                field,
            }
            | Op::StructuralByteSequenceFieldByteStore {
                destination: source,
                path,
                field,
                ..
            } => {
                if let Some(root) = self.root(*source)? {
                    let selected = crate::projected_field_declaration(
                        self.types.iter(),
                        root.structural_type,
                        path,
                        *field,
                    )
                    .ok_or(Error::malformed("byte access has no declared field"))?;
                    let mut path = path.clone();
                    path.push(StructuralPathSegment::Field(selected.identity));
                    let (path, _) = self.observed(*source, &path)?;
                    self.read(state, *source, &path, site)?;
                }
            }
            // A store requires a live receiving home, not a value in its hole.
            Op::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => {
                if self.root(*destination)?.is_some() {
                    // A scalar written into an element chosen at run time
                    // needs that element's home live, so the whole array.
                    let (mut path, exact) = self.observed(*destination, path)?;
                    if exact {
                        path.push(Segment::Field(*field));
                    }
                    if state.regions[index(destination.get())?].is_none() {
                        self.read(state, *destination, &path, site)?;
                    } else if !state.regions[index(destination.get())?]
                        .as_ref()
                        .is_some_and(|region| region.available(&path))
                    {
                        return Err(Error::unsupported(
                            "scalar reconstruction of a moved affine carrier",
                        ));
                    }
                }
            }
            Op::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => {
                if self.root(*destination)?.is_some() {
                    let (home, exact) = self.observed(*destination, path)?;
                    // Writing an uninitialized primitive needs no old value.
                    // An indirect destination still needs its descriptor live;
                    // that requirement does not read or establish its backing.
                    if !exact {
                        self.read(state, *destination, &home, site)?;
                    }
                }
            }
            Op::StoreStructuralField { value, .. } => self.argument(state, value, site, false)?,
            Op::StructuralByteViewFieldStore { source, .. } => {
                self.read(state, *source, &[], site)?;
            }
            kind => {
                for (used, _) in kind.storage_uses() {
                    self.storage_use(state, used, site)?;
                }
            }
        }
        match &operation.kind {
            Op::EstablishByteSequenceLiteral { destination, .. } => {
                self.establish(state, *destination, true)?;
            }
            Op::MoveStructuralValue { source } => {
                if self.root(source.place)?.is_some_and(|root| !root.owned) {
                    return Err(Error::unsupported("owned extraction from borrowed storage"));
                }
                self.argument(state, source, site, true)?;
            }
            Op::ReleaseReference { source, path } => {
                if !self.root(*source)?.is_some_and(|root| root.owned) {
                    return Err(Error::unsupported("owned extraction from borrowed storage"));
                }
                let region = self.canonical(*source, path)?;
                self.read(state, *source, &region, site)?;
                if !self.nominal_ancestors_partitioned(state, *source, &region)? {
                    return Err(Error::unsupported(
                        "reference release crosses an intact nominal wrapper",
                    ));
                }
                self.restore_reference_leaves(state, *source, path, site)?;
                self.vacate(state, *source, &region)?;
            }
            kind if kind.constructor_arguments().next().is_some() => {
                // Every constructor transfers its actual owned children. The
                // same roster covers records, cases and fixed/stored arrays;
                // their moved homes must not also receive edge cleanup.
                for argument in kind.constructor_arguments() {
                    self.argument(state, argument, site, true)?;
                }
            }
            Op::StoreStructuralField {
                destination,
                path,
                field,
                ..
            }
            | Op::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                ..
            } => {
                let declared_disposition =
                    if let Op::StoreStructuralField {
                        disposition, value, ..
                    } = &operation.kind
                    {
                        self.argument(state, value, site, true)?;
                        Some(*disposition)
                    } else {
                        // Storing a shared byte view copies its descriptor.
                        None
                    };
                let backings = if self.references.is_some() {
                    self.backings(state, *destination, path, site)?
                } else {
                    WindowBacking::Exact((*destination, path.clone()))
                };
                // Replacing one of several occupied fields leaves the same
                // availability frontier whichever field was chosen. Filling
                // a vacancy requires knowing which field to establish.
                let unique = matches!(backings, WindowBacking::Exact(_));
                let mut required_disposition =
                    (!unique).then_some(StructuralStoreDisposition::ReplaceDiscardable);
                for (destination, path) in backings.targets() {
                    let Some(root) = self.root(*destination)? else {
                        continue;
                    };
                    // The incoming value transfers from its exact source
                    // above. The receiving region may be an indexed element
                    // only when the entire reached array is already intact:
                    // replacing one discardable value then preserves that
                    // availability without identifying or filling a hole.
                    let (mut receiving, exact) = self.observed(*destination, path)?;
                    // A store through a reference lands in that reference's
                    // backing, which is not this root's affine storage
                    // (structural_access.md: "the explicit `Referent` path
                    // segment crosses the borrowed boundary. Owned transfer,
                    // claims, and referent cleanup cannot cross that
                    // segment"). What this root owes is its descriptor, live
                    // at the store. The backing is established for as long
                    // as a reference to it exists (ownership.md: `@write T`
                    // "requires an existing valid `T`, not vacant construction
                    // storage"), so the store displaces a discardable value
                    // and no region of this root changes.
                    if !exact
                        && path
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                    {
                        self.read(state, *destination, &receiving, site)?;
                        required_disposition = Some(StructuralStoreDisposition::ReplaceDiscardable);
                        continue;
                    }
                    // An inline array owns its elements, not the external
                    // storage reached through one of their views/references.
                    if !exact
                        && !crate::projection_stays_in_inline_storage(
                            self.types.iter(),
                            root.structural_type,
                            path,
                        )
                    {
                        return Err(Error::unsupported(
                            "indexed replacement crosses outside its intact array storage",
                        ));
                    }
                    let path = &mut receiving;
                    if exact && let Some(field) = field {
                        path.push(Segment::Field(*field));
                    }
                    // A whole move ends an owned root's home (`vacate`). A
                    // later whole assignment stores into that same home, so
                    // it re-establishes it vacant and fills it, exactly as a
                    // local's `EstablishStructuralLocal` and vacant store do
                    // (structural_access.md: "Root, field, and indexed
                    // destinations do not change construction semantics").
                    if unique
                        && exact
                        && path.is_empty()
                        && root.owned
                        && state.regions[index(destination.get())?].is_none()
                    {
                        self.establish(state, *destination, false)?;
                    }
                    if state.regions[index(destination.get())?].is_none() {
                        self.read(state, *destination, path, site)?;
                    } else if let Some(region) = &mut state.regions[index(destination.get())?] {
                        if region.contains_partition(path) {
                            return Err(Error::unsupported(
                                "store cannot reconstruct a decomposed nominal wrapper",
                            ));
                        }
                        if (!exact || !unique) && !region.available(path) {
                            return Err(Error::unsupported(if unique {
                                "runtime-indexed replacement in an incomplete affine array"
                            } else {
                                "replacement through a choice requires every receiving field occupied"
                            }));
                        }
                        // Incoming frontiers shrink until all predecessor
                        // paths agree. A join may dispose a field on the path
                        // that still holds it, so classify stores only after
                        // that agreement, never from a provisional arrival.
                        if settled {
                            required_disposition = Some(if region.available(path) {
                                StructuralStoreDisposition::ReplaceDiscardable
                            } else if self.vacant_region(*destination, region, path)? {
                                StructuralStoreDisposition::Vacant
                            } else {
                                return Err(Error::unsupported(
                                    "replacement of a partially moved region",
                                ));
                            });
                        }
                        if !region.base && !path.is_empty() {
                            return Err(Error::unsupported(
                                "projected reestablishment of an ended owner",
                            ));
                        }
                        if unique && exact {
                            region.set(path, true);
                        }
                    }
                }
                if settled && let (Some(disposition), Some(required)) =
                    (declared_disposition, required_disposition)
                {
                    if self.verify_stores && disposition != required {
                        return Err(Error::malformed(match required {
                            StructuralStoreDisposition::Vacant => {
                                "an occupied replacement store targets vacated affine contents"
                            }
                            StructuralStoreDisposition::ReplaceDiscardable => {
                                "a vacant store displaces established affine contents"
                            }
                        }));
                    }
                    self.stores.push((operation.id, required));
                }
            }
            kind => {
                for argument in terminal_psi::call_structural_arguments(kind) {
                    self.argument(state, argument, site, true)?;
                }
            }
        }
        if let Some(result) = operation.result.as_structural() {
            self.establish(
                state,
                result.place,
                !matches!(operation.kind, Op::EstablishStructuralLocal {}),
            )?;
        }
        Ok(())
    }

    fn vacant_region(
        &self,
        place: PlaceId,
        region: &Region,
        path: &[Segment],
    ) -> Result<bool, Error> {
        use std::collections::BTreeSet;
        let Some(selected) = region.selected(path) else {
            return Ok(false);
        };
        // A partially rebuilt missing ancestor needs its remaining missing
        // complement modeled explicitly; it cannot be collapsed to a hole.
        if selected.mode == Mode::Components || !selected.base && !selected.children.is_empty() {
            return Ok(false);
        }
        let root = self
            .root(place)?
            .ok_or(Error::malformed("vacancy has no affine owner"))?;
        let selected_type =
            crate::canonical_structural_path_tip(self.types.iter(), root.structural_type, path)
                .ok_or(Error::malformed("vacancy has no typed receiving region"))?
                .id;
        let mut paths = Vec::new();
        selected.vacancies(&mut Vec::new(), &mut paths);
        let missing = paths
            .into_iter()
            .map(|path| {
                crate::canonical_structural_storage_path(self.types.iter(), selected_type, &path)
            })
            .collect::<Option<BTreeSet<_>>>()
            .ok_or(Error::malformed("vacancy has an untyped missing child"))?;
        Ok(crate::structural_region_is_vacant(
            self.types,
            selected_type,
            &missing,
            &[],
            &|_, _| false,
        ))
    }

    fn dispose_components(
        &mut self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        region: &Region,
        path: Vec<StructuralPathSegment>,
        max_residuals: usize,
        actions: &mut Vec<TerminalAffineCleanupAction>,
    ) -> Result<(), Error> {
        if !region.base && region.children.is_empty() {
            return Ok(());
        }
        if region.has_rebuilt_hole() {
            return Err(Error::unsupported(
                "component cleanup of a partially rebuilt home",
            ));
        }
        let declaration = self
            .types
            .get(index(structural_type.get())?)
            .ok_or(Error::malformed(
                "component disposition type is outside the catalog",
            ))?;
        if region.whole {
            if let Some(selection) = &declaration.nominal_cleanup {
                let module = self.module.ok_or(Error::unsupported(
                    "owned cleanup requires its exact module",
                ))?;
                let cleanup = terminal_psi::OwnedCleanupInvocation {
                    subject: StructuralArgument {
                        place,
                        path,
                        access: StructuralAccess::Owned,
                    },
                    structural_type,
                    cleanup_machine: selection.cleanup_machine,
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                };
                crate::nominal_cleanup::owned_target(
                    self.types.iter(),
                    module.machines.iter(),
                    &cleanup,
                )
                .map_err(Error::unsupported)?;
                if actions.len() >= max_residuals {
                    return Err(Error::malformed(
                        "component cleanup exceeds its supplied action bound",
                    ));
                }
                actions.push(TerminalAffineCleanupAction::InvokeOwned(cleanup));
                return Ok(());
            }
            if declaration.no_code_affine_cleanup && declaration.descriptor_cleanup {
                if actions.len() >= max_residuals {
                    return Err(Error::malformed(
                        "component cleanup exceeds its supplied action bound",
                    ));
                }
                actions.push(if path.is_empty() {
                    TerminalAffineCleanupAction::DiscardRoot(place)
                } else {
                    TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place,
                            path,
                            structural_type,
                        },
                    )
                });
                return Ok(());
            }
        }
        if region.mode != Mode::Components
            && (declaration.nominal_cleanup.is_some()
                || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear)
        {
            return Err(Error::unsupported(
                "partial nominal cleanup requires its authorized component partition",
            ));
        }
        let components =
            self.components(place, structural_type, &path)?
                .ok_or(Error::unsupported(
                    "component disposition has no exact structural components",
                ))?;
        // Exactly the declared relevant components, in the same reverse order
        // as the ordinary residual family. The event never invents a new home.
        for (case, field) in components.fields().rev() {
            if field.relevance.is_erased() {
                return Err(Error::unsupported(
                    "erased component disposition is not served",
                ));
            }
            if let Some(child) = field.field_type.as_structural() {
                let mut child_path = path.clone();
                if let Some(case) = case {
                    child_path.push(StructuralPathSegment::Case(case));
                }
                child_path.push(StructuralPathSegment::Field(field.identity));
                let child_region = match case {
                    Some(case) => region.view(&[Segment::Case(case), Segment::Field(field.id)]),
                    None => region.view(&[Segment::Field(field.id)]),
                };
                self.dispose_components(
                    place,
                    child,
                    &child_region,
                    child_path,
                    max_residuals,
                    actions,
                )?;
            } else if field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| carrier.is_borrowed())
            {
                return Err(Error::unsupported(
                    "inline descriptor component disposition requires its exact carrier type",
                ));
            }
        }
        Ok(())
    }

    /// Unread storage still owes its value unless ordinary cleanup is allowed
    /// or every component has moved out of an unclaimed home. Retiring that
    /// empty home grants no disposition of a surviving value or root claim.
    fn needs_explicit_disposition(
        &mut self,
        place: PlaceId,
        region: &Region,
    ) -> Result<bool, Error> {
        let info = self
            .root(place)?
            .cloned()
            .ok_or(Error::malformed("missing affine owner"))?;
        let declaration = &self.types[index(info.structural_type.get())?];
        if (info.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            || (region.whole
                && declaration.nominal_cleanup.is_some()
                && declaration.multiplicity == info.multiplicity))
            && self.can_dispose(info.structural_type)?
        {
            return Ok(false);
        }
        if info.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            && region.whole
            && !region.contains_partition(&[])
        {
            return Ok(true);
        }
        if region.contains_partition(&[]) || region.whole {
            return self.components_need_disposition(
                place,
                info.structural_type,
                region,
                &mut Vec::new(),
            );
        }
        Ok(declaration.owns_linear_claim || !self.vacant_region(place, region, &[])?)
    }

    /// Whether the part of a `ty` value that `region` still holds contains a
    /// linear obligation no declared cleanup ends: an intact linear value or
    /// linear claim. A sum's payloads are searched in every case, which only
    /// matters for a partly moved sum; an answer of `false` leaves the caller
    /// with its refusal as a limit of the check.
    fn holds_live_linear(&self, ty: StructuralTypeId, region: &Region) -> Result<bool, Error> {
        let declaration = &self.types[index(ty.get())?];
        if !region.base && region.children.is_empty() {
            return Ok(false);
        }
        if declaration.nominal_cleanup.is_some() {
            return Ok(false);
        }
        // A claim the value itself owns stays live until the whole value is
        // consumed: moving its fields out does not end it.
        if declaration.owns_linear_claim
            || region.whole
                && declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        {
            return Ok(true);
        }
        for (case, field) in declaration.shape.fields_with_case() {
            if field.relevance.is_erased() {
                continue;
            }
            let Some(child) = field.field_type.as_structural() else {
                continue;
            };
            let held = match case {
                Some(case) => region.view(&[Segment::Case(case.id), Segment::Field(field.id)]),
                None => region.view(&[Segment::Field(field.id)]),
            };
            if self.holds_live_linear(child, &held)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether an arrival holding `held` keeps a linear obligation in a part
    /// the join's `joined` region has vacant.
    fn loses_live_linear(
        &self,
        ty: StructuralTypeId,
        held: &Region,
        joined: &Region,
    ) -> Result<bool, Error> {
        let mut vacant = Vec::new();
        joined.vacancies(&mut Vec::new(), &mut vacant);
        for path in vacant {
            let tip = if path.is_empty() {
                Some(ty)
            } else {
                crate::canonical_structural_path_tip(self.types.iter(), ty, &path)
                    .map(|declaration| declaration.id)
            };
            if let Some(tip) = tip
                && self.holds_live_linear(tip, &held.view(&path))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Ending a wrapper does not end any intact linear component inside it.
    fn components_need_disposition(
        &mut self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        region: &Region,
        path: &mut Vec<StructuralPathSegment>,
    ) -> Result<bool, Error> {
        if !region.base && region.children.is_empty() {
            return Ok(false);
        }
        let types = self.types;
        let declaration = &types[index(structural_type.get())?];
        if !region.contains_partition(&[]) && self.can_dispose(structural_type)? {
            return Ok(false);
        }
        if !region.contains_partition(&[])
            && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        {
            // An intact record still needs its declared disposal or an
            // explicit decomposition. Its field geometry grants neither.
            return Ok(true);
        }
        if region.mode != Mode::Components
            && (declaration.owns_linear_claim
                || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || declaration.nominal_cleanup.is_some())
        {
            return Ok(true);
        }
        let Some(components) = self.components(place, structural_type, path)? else {
            return Ok(true);
        };
        for (case, field) in components.fields() {
            if field.relevance.is_erased() {
                return Err(Error::unsupported(
                    "erased component disposition is not served",
                ));
            }
            if let Some(child) = field.field_type.as_structural() {
                let child_region = match case {
                    Some(case) => region.view(&[Segment::Case(case), Segment::Field(field.id)]),
                    None => region.view(&[Segment::Field(field.id)]),
                };
                if let Some(case) = case {
                    path.push(StructuralPathSegment::Case(case));
                }
                path.push(StructuralPathSegment::Field(field.identity));
                let needed = self.components_need_disposition(place, child, &child_region, path)?;
                path.pop();
                if case.is_some() {
                    path.pop();
                }
                if needed {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Case membership selects the live components; it supplies neither a
    /// disposition for a child nor permission to end an intrinsic root claim.
    fn components(
        &mut self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<Option<Components<'a>>, Error> {
        let types = self.types;
        let declaration = &types[index(structural_type.get())?];
        let (common, cases) = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => {
                return Ok(Some(Components {
                    common: fields,
                    case: None,
                }));
            }
            StructuralTypeShape::Sum { cases } => (&[][..], cases),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
            _ => return Ok(None),
        };
        let root = self
            .root(place)?
            .ok_or(Error::malformed("component cleanup has no root"))?
            .structural_type;
        let subject =
            crate::descriptor_lifetimes::descriptor_location(types, place, root, path, None)
                .ok_or(Error::malformed(
                    "component cleanup has no exact case subject",
                ))?;
        let machine = self
            .body
            .ok_or(Error::unsupported(
                "case component cleanup requires its exact body",
            ))?
            .id;
        let at = self
            .position
            .ok_or(Error::malformed("case component cleanup has no position"))?;
        let selected = self
            .references
            .as_mut()
            .and_then(|references| {
                references.established_case(machine, at, self.successor, &subject)
            })
            .ok_or(Error::unsupported(
                "component disposition needs an established active case",
            ))?;
        let case = terminal_psi::find_structural_case(cases, selected).ok_or(Error::malformed(
            "component disposition selects an undeclared case",
        ))?;
        Ok(Some(Components {
            common,
            case: Some(case),
        }))
    }

    fn dispose(
        &mut self,
        place: PlaceId,
        region: &Region,
        max_residuals: usize,
    ) -> Result<Vec<TerminalAffineCleanupAction>, Error> {
        use std::collections::BTreeSet;
        let info = self
            .root(place)?
            .cloned()
            .ok_or(Error::malformed("missing affine owner"))?;
        let declaration = &self.types[index(info.structural_type.get())?];
        if self.needs_explicit_disposition(place, region)? {
            if self.holds_live_linear(info.structural_type, region)? {
                return Err(Error::Unconsumed {
                    place,
                    at_join: false,
                    site: None,
                });
            }
            return Err(Error::unsupported(
                "normal edge cannot implicitly dispose storage without an authorized disposition",
            ));
        }
        if !region.contains_partition(&[])
            && !region.whole
            && (info.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || !self.can_dispose(info.structural_type)?)
        {
            return Ok(vec![TerminalAffineCleanupAction::RetireHome(place)]);
        }
        let copyable_parameter = matches!(declaration.shape, StructuralTypeShape::TypeParameter)
            && info.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
        if !copyable_parameter
            && (region.contains_partition(&[])
                || declaration.nominal_cleanup.is_some()
                || !declaration.no_code_affine_cleanup
                || !declaration.descriptor_cleanup)
        {
            let mut actions = Vec::new();
            // Components mode can only be reached by the checked consuming
            // operation on this exact root. Its content obligations survive
            // retirement of the storage that held the opened wrapper.
            match &info.decomposition {
                Decomposition::QualifiedRoot if region.mode != Mode::Components => {
                    return Err(Error::unsupported(
                        "component cleanup cannot settle qualifications or independent content custody",
                    ));
                }
                Decomposition::QualifiedFields(fields) => {
                    if !region.moved_whole(&[])
                        && fields.iter().any(|field| !region.moved_whole(field))
                    {
                        return Err(Error::unsupported(
                            "component cleanup cannot settle a surviving qualified field",
                        ));
                    }
                    if region.mode != Mode::Components && self.vacant_region(place, region, &[])? {
                        return Ok(vec![TerminalAffineCleanupAction::RetireHome(place)]);
                    }
                }
                _ => {}
            }
            self.dispose_components(
                place,
                info.structural_type,
                region,
                Vec::new(),
                max_residuals,
                &mut actions,
            )?;
            if !matches!(actions.as_slice(), [TerminalAffineCleanupAction::InvokeOwned(cleanup)] if cleanup.subject.path.is_empty())
            {
                actions.push(TerminalAffineCleanupAction::RetireHome(place));
            }
            return Ok(actions);
        }
        // Retire the unrestricted home after its last use. Each surviving
        // reference copy retains its own link to the checked backing.
        if info.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted {
            return Ok(vec![TerminalAffineCleanupAction::RetireHome(place)]);
        }
        if region.whole {
            return Ok(vec![TerminalAffineCleanupAction::DiscardRoot(place)]);
        }
        let mut paths = Vec::new();
        region.vacancies(&mut Vec::new(), &mut paths);
        if paths.iter().any(Vec::is_empty) {
            return Ok(vec![TerminalAffineCleanupAction::RetireHome(place)]);
        }
        let moved = paths
            .into_iter()
            .map(|path| {
                crate::canonical_structural_storage_path(
                    self.types.iter(),
                    info.structural_type,
                    &path,
                )
            })
            .collect::<Option<BTreeSet<_>>>()
            .ok_or(Error::malformed("moved region has no typed storage path"))?;
        let residuals = crate::partial_affine_residuals(
            self.types,
            info.structural_type,
            &moved,
            max_residuals,
        )
        .ok_or(Error::malformed(
            "affine residual partition exceeds its output or has no typed complement",
        ))?;
        let mut actions = residuals
            .into_iter()
            .map(|(path, structural_type)| {
                TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place,
                        path,
                        structural_type,
                    },
                )
            })
            .collect::<Vec<_>>();
        actions.push(TerminalAffineCleanupAction::RetireHome(place));
        Ok(actions)
    }

    /// Discards the live complement this arrival holds inside the join's
    /// vacancies, keeping the allocated home. The shared typed partition
    /// retains reverse declaration/index order across all receiving regions,
    /// including a region this arrival has already moved out of in part.
    fn equalize(
        &mut self,
        place: PlaceId,
        held: &Region,
        joined: &Region,
        max_residuals: usize,
    ) -> Result<Vec<TerminalAffineCleanupAction>, Error> {
        use std::collections::BTreeSet;
        let info = self
            .root(place)?
            .cloned()
            .ok_or(Error::malformed("missing affine owner"))?;
        // A missing ancestor with individually rebuilt children cannot be
        // flattened to an absent ancestor: that would hide its live children.
        // Receiving stores do not serve that partial-construction state yet.
        if held.has_rebuilt_hole() || joined.has_rebuilt_hole() {
            return Err(Error::unsupported(
                "a partially rebuilt home at a reconvergence",
            ));
        }
        if held == joined {
            return Ok(Vec::new());
        }
        if info.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !self.can_dispose(info.structural_type)?
        {
            if self.loses_live_linear(info.structural_type, held, joined)? {
                return Err(Error::Unconsumed {
                    place,
                    at_join: true,
                    site: None,
                });
            }
            return Err(Error::unsupported(
                "reconvergence cannot implicitly dispose storage without an authorized disposition",
            ));
        }
        let missing = |region: &Region| {
            let mut paths = Vec::new();
            region.vacancies(&mut Vec::new(), &mut paths);
            paths
                .into_iter()
                .map(|path| {
                    crate::canonical_structural_storage_path(
                        self.types.iter(),
                        info.structural_type,
                        &path,
                    )
                })
                .collect::<Option<BTreeSet<_>>>()
                .ok_or(Error::malformed("moved region has no typed storage path"))
        };
        let declaration = &self.types[index(info.structural_type.get())?];
        if held.contains_partition(&[])
            || declaration.nominal_cleanup.is_some()
            || !declaration.no_code_affine_cleanup
            || !declaration.descriptor_cleanup
        {
            if !held.same_partitions(joined) {
                return Err(Error::unsupported(
                    "affine reconvergence changes a nominal component partition",
                ));
            }
            let mut scopes = Vec::new();
            joined.vacancies(&mut Vec::new(), &mut scopes);
            let mut actions = Vec::new();
            for scope in scopes {
                let selected = crate::canonical_structural_path_tip(
                    self.types.iter(),
                    info.structural_type,
                    &scope,
                )
                .ok_or(Error::malformed(
                    "component reconvergence has no selected type",
                ))?;
                let path = crate::canonical_structural_storage_path(
                    self.types.iter(),
                    info.structural_type,
                    &scope,
                )
                .ok_or(Error::malformed(
                    "component reconvergence has no exact storage path",
                ))?;
                self.dispose_components(
                    place,
                    selected.id,
                    &held.view(&scope),
                    path,
                    max_residuals,
                    &mut actions,
                )?;
            }
            return Ok(actions);
        }
        let residuals = crate::partial_affine_residuals_in_regions(
            self.types,
            info.structural_type,
            &missing(held)?,
            &missing(joined)?,
            max_residuals,
        )
        .ok_or(Error::malformed(
            "affine reconvergence partition exceeds its output or has no typed complement",
        ))?;
        Ok(residuals
            .into_iter()
            .map(|(path, structural_type)| {
                TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place,
                        path,
                        structural_type,
                    },
                )
            })
            .collect())
    }

    fn can_dispose(&self, ty: StructuralTypeId) -> Result<bool, Error> {
        self.dispositions
            .get(index(ty.get())?)
            .copied()
            .flatten()
            .ok_or(Error::malformed("disposition type was not prepared"))
    }

    fn root(&self, root: PlaceId) -> Result<Option<&Root>, Error> {
        self.roots
            .get(index(root.get())?)
            .map(Option::as_ref)
            .ok_or(Error::malformed("place is outside the machine"))
    }

    /// A reference carrier stays live while its exact captured lender holds
    /// the window. Resolve at the use, so reassignment cannot retarget a hole.
    fn backings(
        &mut self,
        state: &State,
        root: PlaceId,
        path: &[StructuralPathSegment],
        site: Site,
    ) -> Result<WindowBacking, Error> {
        if !path.contains(&StructuralPathSegment::Referent) {
            return Ok(WindowBacking::Exact((root, path.to_vec())));
        }
        let body = self
            .body
            .ok_or(Error::malformed("a window has no machine"))?;
        let declared = body.structural_place_type(root).ok_or(Error::unsupported(
            "a borrowed window has no typed reference carrier",
        ))?;
        let (carrier, _) = self.observed(root, path)?;
        self.read(state, root, &carrier, site)?;
        let location = crate::descriptor_lifetimes::descriptor_location(
            self.types, root, declared, path, None,
        )
        .ok_or(Error::unsupported(
            "a borrowed window has no exact captured path",
        ))?;
        let machine = body.id;
        let position = self
            .position
            .ok_or(Error::malformed("a window has no use position"))?;
        let references = self.references.as_mut().ok_or(Error::unsupported(
            "a borrowed reference window requires its module's exact backing",
        ))?;
        if let Some(captured) = references.location(machine, position, &location) {
            let declaration =
                body.structural_place_type(captured.root)
                    .ok_or(Error::unsupported(
                        "a borrowed window has no typed lender storage",
                    ))?;
            let path = crate::descriptor_lifetimes::descriptor_path(
                self.types,
                declaration,
                &captured.path,
            )
            .ok_or(Error::unsupported(
                "a borrowed window has no typed lender path",
            ))?;
            return Ok(WindowBacking::Exact((captured.root, path)));
        }
        let unresolved =
            || Error::unsupported("a borrowed window has no exact live backing correspondence");
        let family = references
            .captured_storage_family(machine, position, &location)
            .map_err(|_| unresolved())?;
        family
            .into_iter()
            .map(|origin| {
                let crate::descriptor_results::DescriptorCallOrigin::Argument {
                    source,
                    projection,
                } = origin
                else {
                    return Err(unresolved());
                };
                let path = projection.captured_storage_path().ok_or_else(unresolved)?;
                Ok((source, path))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(WindowBacking::Alternatives)
    }

    /// A move or a receiving store changes one tracked vacancy. A family
    /// can prove that every target was restored, but cannot identify which
    /// target to remove from or add to the affine state.
    fn backing(
        &mut self,
        state: &State,
        root: PlaceId,
        path: &[StructuralPathSegment],
        site: Site,
    ) -> Result<(PlaceId, Vec<StructuralPathSegment>), Error> {
        match self.backings(state, root, path, site)? {
            WindowBacking::Exact(target) => Ok(target),
            WindowBacking::Alternatives(_) => Err(Error::unsupported(
                "a borrowed window has no exact live backing correspondence",
            )),
        }
    }

    fn canonical(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<Segment>, Error> {
        let declaration = self
            .root(root)?
            .ok_or(Error::malformed("untracked affine root"))?;
        crate::canonical_inline_region(self.types.iter(), declaration.structural_type, path).ok_or(
            Error::unsupported("affine projection outside the static structural vocabulary"),
        )
    }

    /// Projected consumption can enter a nominal wrapper only after an
    /// authorized decomposition has ended its disposition. Follow the type
    /// and exact region together so nested wrappers use the same rule.
    fn nominal_ancestors_partitioned(
        &self,
        state: &State,
        place: PlaceId,
        mut path: &[Segment],
    ) -> Result<bool, Error> {
        let root = self
            .root(place)?
            .ok_or(Error::malformed("nominal projection has no affine root"))?;
        let mut declaration = self
            .types
            .get(index(root.structural_type.get())?)
            .filter(|declaration| declaration.id == root.structural_type)
            .ok_or(Error::malformed("nominal projection has no root type"))?;
        let mut region = state
            .regions
            .get(index(place.get())?)
            .ok_or(Error::malformed("nominal projection has no storage row"))?
            .as_ref();
        while !path.is_empty() {
            if declaration.nominal_cleanup.is_some()
                && !region.is_some_and(|region| region.mode == Mode::Components)
            {
                return Ok(false);
            }
            // A case names a payload roster, not a value type of its own.
            // Its field completes one typed step while both region segments
            // retain their exact identity.
            let count = match path {
                [Segment::Case(_), Segment::Field(_), ..] => 2,
                [Segment::Case(_), ..] => {
                    return Err(Error::malformed("nominal projection ends at a case roster"));
                }
                _ => 1,
            };
            let (step, remaining) = path.split_at(count);
            // Callers have already authenticated the complete canonical path.
            // Only strict ancestors need a declaration here; a terminal scalar
            // field need not have a separately materialized structural type.
            if remaining.is_empty() {
                return Ok(true);
            }
            declaration =
                crate::canonical_structural_path_tip(self.types.iter(), declaration.id, step)
                    .ok_or(Error::malformed("nominal projection has an untyped child"))?;
            for segment in step {
                region = region.and_then(|region| {
                    region
                        .children
                        .binary_search_by_key(segment, |(child, _)| *child)
                        .ok()
                        .map(|index| &region.children[index].1)
                });
            }
            path = remaining;
        }
        Ok(true)
    }

    /// The region an observation of `path` under `root` depends on, and
    /// whether that region is exactly the projected place.
    ///
    /// An inline projection whose every step is static has its own region.
    /// A reference or view's backing is separate storage: an observation
    /// depends on the live descriptor, while loan checking judges access to
    /// the backing.
    /// An element chosen at run time has none of its own: which element it
    /// is is not known here, so the observation depends on the whole fixed
    /// array it indexes and is answered from that array's region. This
    /// widening is one-sided: it can refuse an observation
    /// of an element that is in fact available, never admit one of an
    /// element that is not. It grants no move or vacant initialization.
    /// A replacement may rely on this region only while the whole array is
    /// already intact and leaves its availability unchanged. The supplied
    /// value still transfers through `canonical`, which has no region for
    /// moving an element chosen at runtime out of that array.
    fn observed(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(Vec<Segment>, bool), Error> {
        match self.canonical(root, path) {
            Ok(exact) => return Ok((exact, true)),
            Err(Error::Unsupported { .. }) => {}
            Err(malformed) => return Err(malformed),
        }
        let outside =
            Error::unsupported("affine projection outside the static structural vocabulary");
        let declaration = self
            .root(root)?
            .ok_or(Error::malformed("untracked affine root"))?;
        // Reading any borrowed payload needs a live descriptor. Only its
        // carrier has an inline region here; the payload address, access and
        // index bounds are checked at the use. This cannot license a move.
        let descriptor = path.iter().enumerate().find_map(|(position, segment)| {
            if !matches!(
                segment,
                StructuralPathSegment::Referent
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
            ) {
                return None;
            }
            let (prefix, carrier) = crate::canonical_static_projection(
                self.types.iter(),
                declaration.structural_type,
                &path[..position],
            )?;
            carrier.shape.is_borrowed_descriptor().then_some(prefix)
        });
        if let Some(descriptor) = descriptor {
            crate::runtime_read_projection_tip(
                self.types.iter(),
                declaration.structural_type,
                path,
            )
            .ok_or(outside)?;
            return Ok((descriptor, false));
        }
        let array =
            runtime_array_region(self.types, declaration.structural_type, path).ok_or(outside)?;
        Ok((array, false))
    }

    fn read(
        &mut self,
        state: &State,
        place: PlaceId,
        path: &[Segment],
        site: Site,
    ) -> Result<(), Error> {
        if self.root(place)?.is_none() {
            return Ok(());
        }
        if !state
            .regions
            .get(index(place.get())?)
            .and_then(Option::as_ref)
            .is_some_and(|region| region.available(path))
        {
            let last = &mut self.unavailable_roots[index(place.get())?];
            if *last != Some(site) {
                self.unavailable.push(Unavailable {
                    site,
                    place,
                    reason: UnavailableReason::Use,
                });
                *last = Some(site);
            }
        }
        Ok(())
    }

    fn argument(
        &mut self,
        state: &mut State,
        argument: &StructuralArgument,
        site: Site,
        consume: bool,
    ) -> Result<(), Error> {
        let Some(root) = self.root(argument.place)? else {
            return Ok(());
        };
        let owned = root.owned;
        let affine = root.affine;
        let lends_window = root.exclusive;
        let path = if consume && affine && argument.access == StructuralAccess::Owned {
            self.canonical(argument.place, &argument.path)
        } else {
            self.observed(argument.place, &argument.path)
                .map(|(path, _)| path)
        }
        .map_err(|error| error.at(site))?;
        self.read(state, argument.place, &path, site)?;
        if consume && affine && argument.access == StructuralAccess::Owned {
            if state.regions[index(argument.place.get())?]
                .as_ref()
                .is_some_and(|region| region.contains_partition(&path))
            {
                return Err(Error::unsupported(
                    "a decomposed nominal wrapper is unavailable whole",
                ));
            }
            if !self.nominal_ancestors_partitioned(state, argument.place, &path)? {
                return Err(Error::unsupported(
                    "projected owned transfer crosses an intact nominal wrapper",
                ));
            }
            // ownership.md, "Borrowed-storage invariant windows": "An owned
            // move out of a place reached through `@mutable` may temporarily
            // leave that place unestablished ... The move transfers the
            // removed value's exact custody to its destination and records a
            // restoration obligation on the original place and loan." The
            // region below becomes vacant exactly as an owned root's does, so
            // "the absent subtree cannot be read, borrowed as a value, moved
            // again" and its ancestors are unavailable whole; a store into
            // the exact vacant place makes it available again; and `walk`
            // owes the obligation on every edge that leaves the machine. A
            // shared loan lends no such window, and the whole referent is
            // never the machine's to take.
            if !owned && (!lends_window || path.is_empty()) {
                return Err(Error::unsupported("owned extraction from borrowed storage"));
            }
            self.vacate(state, argument.place, &path)?;
        }
        Ok(())
    }

    /// Ordinary owned transfer ends its source home. A storage extraction
    /// through a live reference instead uses `vacate_contents` and keeps it.
    fn vacate(&self, state: &mut State, place: PlaceId, path: &[Segment]) -> Result<(), Error> {
        let root = self
            .root(place)?
            .ok_or(Error::malformed("a moved region has no affine home"))?;
        let region = &mut state.regions[index(place.get())?];
        if root.owned && path.is_empty() {
            *region = None;
            Ok(())
        } else {
            self.vacate_contents(state, place, path)
        }
    }

    fn vacate_contents(
        &self,
        state: &mut State,
        place: PlaceId,
        path: &[Segment],
    ) -> Result<(), Error> {
        let region = state
            .regions
            .get_mut(index(place.get())?)
            .ok_or(Error::malformed("vacated contents are outside the machine"))?;
        if let Some(region) = region {
            if region.contains_partition(path) {
                return Err(Error::unsupported(
                    "a storage extraction cannot vacate an ended nominal wrapper",
                ));
            }
            region.set(path, false);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
