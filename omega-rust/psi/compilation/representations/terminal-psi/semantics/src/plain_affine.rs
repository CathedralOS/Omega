//! Ownership of plain affine storage, independently of its scalar contents.
//!
//! A value exists even when it has no scalar leaf (an empty case, for example).
//! Moves therefore change an exact region's availability, while scalar reaching
//! definitions remain a separate analysis. The producer uses the resulting
//! edge schedule; a checker reconstructs it from the same unsealed operations.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, EdgeId, IdSet, OperationId, PlaceId,
    StructuralTypeId,
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
    /// The site returns custody of a borrowed region while an owned move
    /// still leaves part of it vacant. This includes descriptor release and
    /// cleanup as well as normal handback of incoming storage. Every other
    /// row is a use of storage unavailable at its observation.
    pub unrestored: bool,
}

#[derive(Debug)]
pub enum Error {
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
            Self::Unsupported { site, .. } | Self::Malformed { site, .. } => *site,
        }
    }
}

#[derive(Debug, Default)]
pub struct Plan {
    pub affine_roots: IdSet<PlaceId>,
    /// Owned copyable storage with trivial cleanup may retire its binding.
    /// It owns no affine contents to discard.
    pub copyable_owned_roots: IdSet<PlaceId>,
    pub cleanup: Vec<(EdgeId, Vec<TerminalAffineCleanupAction>)>,
    /// Exact receiving regions determine whether a store fills a vacancy or
    /// replaces intact discardable contents. Sorted by operation identity.
    pub stores: Vec<(OperationId, StructuralStoreDisposition)>,
    pub unavailable: Vec<Unavailable>,
    pub reference_ends: Vec<ReferenceEnd>,
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct Region {
    base: bool,
    whole: bool,
    unavailable_children: usize,
    children: Vec<(Segment, Region)>,
}

impl Region {
    fn new(available: bool) -> Self {
        Self {
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
        child.set(tail, available);
        let now_unavailable = !child.whole;
        self.unavailable_children =
            self.unavailable_children + usize::from(now_unavailable) - usize::from(was_unavailable);
        if child.base == self.base && child.children.is_empty() {
            self.unavailable_children -= usize::from(!child.whole);
            self.children.remove(position);
        }
        self.whole = self.base && self.unavailable_children == 0;
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
    parameter: bool,
    /// A parameter lent with `&mut`: the only storage the machine does not
    /// own that an owned value may be moved out of, leaving a window the
    /// machine must close before it returns (`Analysis::argument`).
    exclusive: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct State {
    regions: Vec<Option<Region>>,
    // Establishment order of the currently allocated owned homes. Normal-edge
    // disposal walks this frontier, not every root ever produced by the body.
    active: Vec<PlaceId>,
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
                (Some(mine), Some(theirs)) => Ok(Some(mine.intersection(theirs))),
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
                "affine homes established in different orders at a reconvergence",
            ));
        }
        Ok(State { regions, active })
    }
}

/// One normal edge out of a walked block.
struct Arrival {
    edge: EdgeId,
    /// `None` for an edge that leaves the machine.
    target: Option<semantic_vocabulary::BlockId>,
    outgoing: State,
    actions: Vec<TerminalAffineCleanupAction>,
}

struct Residence {
    path: Vec<Segment>,
    live: std::collections::BTreeSet<crate::descriptor_lifetimes::DescriptorPosition>,
}

struct Analysis<'a> {
    types: &'a [StructuralTypeDeclaration],
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
    reference_candidates: Option<&'a crate::reference_ends::Candidates>,
    reference_ends: Vec<ReferenceEnd>,
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
    std::collections::BTreeMap<crate::descriptor_lifetimes::DescriptorPosition, IdSet<PlaceId>>,
    Error,
> {
    use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPathSegment};
    use crate::descriptor_results::{DescriptorSlotKind, FlowFeasibility};
    let mut uses = std::collections::BTreeMap::new();
    for (at, step) in &graph.flow {
        let mut inputs = step.inputs.clone();
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
            let ty = *graph.place_types.get(&input.root).ok_or(Error::malformed(
                "reference observation lost its source type",
            ))?;
            let slots = DescriptorSlotKind::References
                .owned_slots(types, input.root, ty)
                .ok_or(Error::unsupported(
                    "reference observation has no finite owned slots",
                ))?;
            'slots: for mut slot in slots {
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
                        },
                        case: *case,
                    };
                    if references.case_feasibility(graph.id, *at, &predicate)
                        == FlowFeasibility::ProvedImpossible
                    {
                        continue 'slots;
                    }
                }
                slot.path.push(DescriptorPathSegment::Referent);
                let source =
                    references
                        .location(graph.id, *at, &slot)
                        .ok_or(Error::unsupported(
                            "reference observation has no exact live backing",
                        ))?;
                if !graph.place_types.contains_key(&source.root) {
                    return Err(Error::malformed("reference backing is outside its machine"));
                }
                uses.entry(*at)
                    .or_insert_with(IdSet::new)
                    .insert(source.root);
            }
        }
    }
    Ok(uses)
}

fn live_entries(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    reference_candidates: Option<&crate::reference_ends::Candidates>,
    reference_backing: &std::collections::BTreeMap<
        crate::descriptor_lifetimes::DescriptorPosition,
        IdSet<PlaceId>,
    >,
) -> Result<Vec<IdSet<PlaceId>>, Error> {
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
            let valid = |place: PlaceId| -> Result<PlaceId, Error> {
                if index(place.get())? < machine.structural_places.len() {
                    Ok(place)
                } else {
                    Err(Error::malformed("live place is outside the machine"))
                }
            };
            for root in terminator_roots(&block.terminator) {
                before.insert(valid(root)?);
            }
            let end = u32::try_from(block.operations.len())
                .map_err(|_| Error::malformed("reference observation position overflows"))?;
            if let Some(backing) = reference_backing.get(&(block.id, end)) {
                before.union_with(backing);
            }
            for (position, operation) in block.operations.iter().enumerate().rev() {
                if let Some(result) = operation.result.as_structural() {
                    before.remove(valid(result.place)?);
                }
                for used in operation
                    .kind
                    .storage_uses()
                    .into_iter()
                    .filter_map(storage_root)
                {
                    before.insert(valid(used)?);
                }
                // A planned release is a real future use of its owned
                // descriptor home. Keep that home until the planner can emit
                // it; otherwise edge cleanup could close a parent while its
                // child still runs. The final verifier reads emitted uses.
                let position = u32::try_from(position)
                    .map_err(|_| Error::malformed("reference end position overflows"))?;
                if let Some(backing) = reference_backing.get(&(block.id, position)) {
                    before.union_with(backing);
                }
                for candidate in reference_candidates
                    .and_then(|candidates| candidates.get(&(block.id, position)))
                    .into_iter()
                    .flatten()
                {
                    before.insert(valid(candidate.argument.place)?);
                }
            }
            for parameter in &block.structural_parameters {
                if parameter.access != StructuralAccess::Owned
                    && before.contains(parameter.place)
                    && !crate::descriptor_results::is_descriptor(
                        types.iter(),
                        parameter.structural_type,
                    )
                {
                    // The new name still borrows the captured home. Keep its
                    // original owners until the last use of that name.
                    let captures = crate::storage_names(machine, block.id, parameter.place);
                    for root in captures.captured_roots() {
                        before.insert(valid(root)?);
                    }
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

/// Plans only no-code affine storage, including each receiving region's store
/// disposition. The producer installs these dispositions before verification.
/// Nominal hooks, conserved claims and borrowed extraction require their own
/// contracts, rather than an empty plan.
pub fn plan(machine: &TerminalMachine, types: &[StructuralTypeDeclaration]) -> Result<Plan, Error> {
    analyze(machine, types, false, None, None)
}

/// Resolve destructive reference paths with the same descriptor graph as
/// storage and loan checking. The detached value never owns its lender's home.
pub fn plan_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<Plan, Error> {
    let machine = module_machine(module, identity)?;
    analyze(machine, &module.structural_types, false, Some(module), None)
}

/// Resolve dead descriptor candidates against the same exact ownership frontier
/// that supplies residual cleanup. Moved or already disposed leaves are not
/// released again, and borrowed descriptor storage never grants disposition.
pub fn plan_reference_ends(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
    candidates: &crate::reference_ends::Candidates,
) -> Result<Plan, Error> {
    let machine = module_machine(module, identity)?;
    analyze(
        machine,
        &module.structural_types,
        false,
        Some(module),
        Some(candidates),
    )
}

/// Reconstructs the canonical schedule without trusting the supplied actions.
/// The producer's actual output bounds residual enumeration before allocation.
/// Exact equality then checks root/path/type identity, maximality and order.
pub fn verify(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
) -> Result<Plan, Error> {
    verify_schedule(machine, analyze(machine, types, true, None, None)?)
}

/// The receiving checker reconstructs both the exact backing and its schedule.
pub fn verify_in(
    module: &terminal_psi::TerminalModule,
    identity: semantic_vocabulary::MachineId,
) -> Result<Plan, Error> {
    let machine = module_machine(module, identity)?;
    verify_schedule(
        machine,
        analyze(machine, &module.structural_types, true, Some(module), None)?,
    )
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
                .filter(|action| plan.affine_roots.contains(action.place()));
            let expected = plan
                .cleanup
                .binary_search_by_key(&edge, |(edge, _)| *edge)
                .ok()
                .map(|position| plan.cleanup[position].1.as_slice())
                .unwrap_or(&[]);
            if !actual.eq(expected.iter()) {
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
    if declaration.copyable || access != StructuralAccess::Owned {
        StructuralMultiplicity::Unrestricted
    } else if declaration.owns_linear_claim {
        StructuralMultiplicity::Linear
    } else {
        StructuralMultiplicity::Affine
    }
}

/// Each edge into a block parameter that carries a fresh permission on
/// (`carried`) supplies a whole root of that affine custody, so the
/// parameter's loan lifecycle is the one its argument owned.
fn carried_permissions(
    machine: &TerminalMachine,
    contracts: &[Option<(
        semantic_vocabulary::StructuralTypeId,
        terminal_psi::StructuralMultiplicity,
        StructuralAccess,
        Option<terminal_psi::StructuralMultiplicity>,
    )>],
    carried: &[PlaceId],
) -> Result<(), Error> {
    if carried.is_empty() {
        return Ok(());
    }
    for block in &machine.blocks {
        let edges = block.terminator.successor_edges();
        let arguments = block.terminator.successor_block_arguments();
        for ((_, target), arguments) in edges.into_iter().zip(arguments) {
            let Some(target) = machine.blocks.iter().find(|block| block.id == target) else {
                return Err(Error::malformed(
                    "a structural successor names an absent block",
                ));
            };
            for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
                if !carried.contains(&parameter.place) {
                    continue;
                }
                let affine = argument.path.is_empty()
                    && argument.access == StructuralAccess::Owned
                    && contracts
                        .get(index(argument.place.get())?)
                        .copied()
                        .flatten()
                        .is_some_and(|(structural_type, multiplicity, _, _)| {
                            structural_type == parameter.structural_type
                                && multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        });
                if !affine {
                    return Err(Error::malformed(
                        "an affine permission parameter receives other than its carrier",
                    ));
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

fn analyze(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    bound_output: bool,
    module: Option<&terminal_psi::TerminalModule>,
    reference_candidates: Option<&crate::reference_ends::Candidates>,
) -> Result<Plan, Error> {
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::TerminalMachineResult;
    // A generic requirement is a check-form signature, not an executable
    // activation. Its caller still transfers each argument normally.
    if machine.blocks.is_empty() {
        return Ok(Plan::default());
    }
    // A parent's permission cannot erase the custody of its represented
    // children. Check direct dependencies once; no recursive rescanning is
    // needed because every declaration in the dense catalog is visited.
    for (slot, declaration) in types.iter().enumerate() {
        if index(declaration.id.get())? != slot {
            return Err(Error::malformed("types are not in dense identity order"));
        }
        let mut children = declaration
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
            })
            .collect::<Vec<_>>();
        if let terminal_psi::StructuralTypeShape::FixedArray { element, .. } = declaration.shape {
            children.push(element);
        }
        for child in children {
            let child = types
                .get(index(child.get())?)
                .ok_or(Error::malformed("nested type is outside the catalog"))?;
            if declaration.copyable && !child.copyable
                || declaration.no_code_affine_cleanup && !child.no_code_affine_cleanup
                || declaration.descriptor_cleanup && !child.descriptor_cleanup
                || !declaration.owns_linear_claim && child.owns_linear_claim
            {
                return Err(Error::malformed(
                    "parent type permissions erase a nested ownership obligation",
                ));
            }
        }
    }
    let mut contracts = vec![None; machine.structural_places.len()];
    // Read static whole-origin restrictions once with the custody declarations.
    // Actual loan authority and the current value remain separate judgments.
    let mut whole_restoration = vec![true; machine.structural_places.len()];
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
    // a carrier whole (`carried_permissions`), so the marking is never an
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
        let required = (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            && parameter.access == StructuralAccess::Owned
            && machine
                .block_structural_parameters()
                .any(|block| block.place == parameter.place)
            && types
                .get(index(parameter.structural_type.get())?)
                .is_some_and(|declaration| {
                    declaration.copyable
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::Reference {
                                access: StructuralAccess::SharedBorrow,
                                ..
                            }
                        )
                }))
        .then_some(terminal_psi::StructuralMultiplicity::Affine);
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
                                && (!declaration.copyable || access == StructuralAccess::SharedBorrow)))
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
    carried_permissions(machine, &contracts, &carried)?;
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
                || !declaration.copyable
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
        let tracked = !declaration.copyable
            || terminal_psi::structural_type_contains_reference_by(
                |id| terminal_psi::find_structural_type(types, id),
                ty,
            )
            || required_result.is_some_and(|multiplicity| {
                multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            });

        if tracked
            && (declaration.owns_linear_claim
                || !declaration.no_code_affine_cleanup
                || !declaration.descriptor_cleanup)
        {
            return Err(Error::unsupported(
                "affine storage with nontrivial disposition",
            ));
        }
        if owned
            && !tracked
            && declaration.copyable
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
                parameter,
                exclusive,
            }),
        );
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
    if roots.iter().all(Option::is_none) && !has_borrowed_windows {
        return Ok(Plan {
            copyable_owned_roots,
            ..Plan::default()
        });
    }
    let mut affine_roots = IdSet::new();
    for (slot, root) in roots.iter().enumerate() {
        if root.as_ref().is_some_and(|root| root.affine) {
            affine_roots.insert(
                PlaceId::new(slot as u64 + 1).ok_or(Error::malformed("place identity overflow"))?,
            );
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
                .is_some_and(|root| root.parameter && root.owned && root.affine)
        })
        .map(|(slot, _)| {
            PlaceId::new(slot as u64 + 1).ok_or(Error::malformed("place identity overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let initial = State { regions, active };
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
    let reference_graph = if needs_backing {
        let module = module.ok_or(Error::unsupported(
            "reference storage requires its module's exact backing",
        ))?;
        crate::descriptor_results::terminal_reference_graph_from(module, [machine.id])
            .map_err(|_| Error::unsupported("reference storage has no complete closure"))?
    } else {
        Vec::new()
    };
    let mut references = needs_backing.then(|| {
        crate::descriptor_results::DescriptorAnalysis::with_reference_slots(types, &reference_graph)
    });
    let backing = if let Some(references) = &mut references {
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
        types,
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
        reference_candidates,
        reference_ends: Vec::new(),
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
    for block in &machine.blocks {
        let Some(state) = entering[index(block.id.get())?].clone() else {
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
        }
    }
    cleanup.sort_by_key(|(edge, _)| *edge);
    analysis.stores.sort_by_key(|(operation, _)| *operation);
    Ok(Plan {
        affine_roots,
        copyable_owned_roots,
        cleanup,
        stores: analysis.stores,
        unavailable: analysis.unavailable,
        reference_ends: analysis.reference_ends,
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
        if crate::runtime_structural_path_tip(types.iter(), source, &argument.path)
            .is_none_or(|selected| selected.id != parameter.structural_type)
        {
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

impl Analysis<'_> {
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
            .ok_or(Error::unsupported(
                "captured storage has no complete residence flow",
            ))?;
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
        let (root, captured) = self.backing(state, source, &referent, site)?;
        if self.root(root)?.is_none() {
            return Ok(());
        }
        let (captured, inline) = self.observed(root, &captured)?;
        // An incoming descriptor can name storage outside its carrier. The
        // typed observation still checks that carrier, but only an exact
        // inline region can hold a vacancy in this frontier: extraction uses
        // `canonical`, which refuses indirect and runtime-selected regions.
        // Their loan obligations remain with reference checking.
        if !inline {
            return Ok(());
        }
        if !state.regions[index(root.get())?]
            .as_ref()
            .is_some_and(|region| region.available(&captured))
        {
            self.unavailable.push(Unavailable {
                site,
                place: root,
                unrestored: true,
            });
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
        let selected = crate::runtime_structural_path_tip(self.types.iter(), root_type, path)
            .ok_or(Error::unsupported(
                "reference closure has no selected carrier type",
            ))?;
        let leaves = crate::reference_leaves(self.types.iter(), selected.id, usize::MAX).ok_or(
            Error::unsupported("reference closure has no finite descriptor roster"),
        )?;
        for leaf in leaves {
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
    /// the frontier it carries and the disposals it performs. A root the
    /// successor does not read again is disposed of on the edge. With
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
            if operation.suspension_crossing.is_some()
                && (self.residences.iter().enumerate().any(|(raw, captures)| {
                    captures.iter().any(|capture| {
                        self.position.is_some_and(|at| capture.live.contains(&at))
                            && state.regions[raw]
                                .as_ref()
                                .is_none_or(|region| !region.available(&capture.path))
                    })
                }) || self.roots.iter().enumerate().any(|(raw, root)| {
                    root.as_ref()
                        .is_some_and(|root| root.parameter && !root.owned && root.affine)
                        && state.regions[raw]
                            .as_ref()
                            .is_some_and(|region| !region.available(&[]))
                }))
            {
                return Err(Error::unsupported(
                    "a possibly suspending call while an owned value is out of borrowed storage",
                )
                .at(site));
            }
            self.operation(&mut state, operation, entering.is_some())
                .map_err(|error| error.at(site))?;
        }
        self.position = Some((
            block.id,
            u32::try_from(block.operations.len())
                .map_err(|_| Error::malformed("terminal position overflows"))?,
        ));
        let site = Site::Edge(
            block
                .terminator
                .edges()
                .next()
                .ok_or(Error::malformed("terminator has no edge"))?,
        );
        match &block.terminator {
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
                        matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
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
            let successor = targets
                .binary_search_by_key(&edge, |(edge, _, _)| *edge)
                .ok()
                .map(|position| targets[position]);
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
                            unrestored: true,
                        });
                    }
                }
            }
            // A borrowed header names captured storage; it does not own a
            // value that needs cleanup. End a dead name before the next header
            // is bound, leaving its backing and all loan checks untouched.
            // A surviving use still needs a name established on every arrival.
            for place in &self.borrowed_headers {
                let raw = index(place.get())?;
                let forgotten = !needed.is_some_and(|needed| needed.contains(*place))
                    || owned_there.is_some_and(|there| there.regions[raw].is_none());
                if forgotten && let Some(region) = &outgoing.regions[raw] {
                    if !region.available(&[]) {
                        return Err(Error::unsupported(
                            "borrowed block storage still owes restoration at its last use",
                        )
                        .at(Site::Edge(edge)));
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
            let mut actions = Vec::new();
            let mut retained = Vec::new();
            for place in std::mem::take(&mut outgoing.active).into_iter().rev() {
                let raw = index(place.get())?;
                if let Some(region) = &outgoing.regions[raw] {
                    let pinned = target.is_some_and(|target| {
                        self.residences.get(raw).is_some_and(|regions| {
                            regions
                                .iter()
                                .any(|region| region.live.contains(&(target, 0)))
                        })
                    });
                    let read_there = pinned || needed.is_some_and(|needed| needed.contains(place));
                    let there = owned_there.map(|there| there.regions[raw].as_ref());
                    if read_there && there.is_none_or(|there| there.is_some()) {
                        if let Some(Some(there)) = there
                            && there != region
                        {
                            let discards = self.equalize(place, region, there, residual_budget)?;
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
                        let disposal = self.dispose(place, region, residual_budget)?;
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
                                matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
                            })
                            .count();
                        actions.extend(disposal);
                        outgoing.regions[raw] = None;
                    }
                }
            }
            retained.reverse();
            outgoing.active = retained;
            // A copyable home is not an affine owner, so the loop above never
            // disposes of it. When one arrival released the descriptor it
            // holds and this arrival did not, the successor enters without
            // it: the planner retires it on this edge. A checked plan must
            // supply that retirement itself (below).
            if let (false, Some(there)) = (bound_output, owned_there) {
                for (raw, root) in self.roots.iter().enumerate() {
                    let retire = root.as_ref().is_some_and(|root| {
                        root.owned
                            && !root.affine
                            && root.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted
                    }) && outgoing.regions[raw].is_some()
                        && there.regions[raw].is_none();
                    if retire {
                        let place = PlaceId::new(raw as u64 + 1)
                            .ok_or(Error::malformed("place identity overflow"))?;
                        actions.push(TerminalAffineCleanupAction::RetireHome(place));
                        outgoing.regions[raw] = None;
                    }
                }
            }
            // Explicit retirement still ends a copyable home's availability;
            // merely copying its value above does not. These supplied actions
            // confer no affine scheduling authority.
            if bound_output {
                for action in supplied {
                    if self.root(action.place())?.is_some_and(|root| !root.affine)
                        && matches!(action, TerminalAffineCleanupAction::RetireHome(_))
                    {
                        outgoing.regions[index(action.place().get())?] = None;
                    }
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
        if root.owned && root.affine && region.is_none() {
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
            StructuralStorageUse::Projection(projection) => {
                if self.root(projection.root)?.is_none() {
                    return Ok(());
                }
                let (path, _) = self.observed(projection.root, projection.path)?;
                self.read(state, projection.root, &path, site)
            }
            StructuralStorageUse::Canonical { root, path }
            | StructuralStorageUse::ObservedField { root, path } => {
                self.read(state, root, path, site)
            }
            StructuralStorageUse::Type(_) => Ok(()),
            StructuralStorageUse::Element { descriptor, .. } => {
                if self.root(descriptor.root)?.is_some() {
                    return Err(Error::unsupported("affine element views"));
                }
                Ok(())
            }
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
        for argument in terminal_psi::call_structural_arguments(&operation.kind) {
            self.restore_reference_leaves(state, argument.place, &argument.path, site)?;
        }
        if let Op::StructuralLeafCopy { source, path } = &operation.kind
            && let Some(root) = self.root(*source)?
            && crate::canonical_static_projection(self.types.iter(), root.structural_type, path)
                .is_some_and(|(_, declaration)| !declaration.copyable)
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
                if !root.owned && (!root.parameter || !root.exclusive) {
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
                for used in kind.storage_uses() {
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
                let (backing, projection) = if self.references.is_some() {
                    self.backing(state, *destination, path, site)?
                } else {
                    (*destination, path.clone())
                };
                let destination = &backing;
                let path = &projection;
                if let Some(root) = self.root(*destination)? {
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
                    // as a reference to it exists (ownership.md: `&write T`
                    // "requires an existing valid `T`, not vacant construction
                    // storage"), so the store displaces a discardable value
                    // and no region of this root changes.
                    if !exact
                        && path
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                    {
                        self.read(state, *destination, &receiving, site)?;
                        if settled && let Some(disposition) = declared_disposition {
                            let required = StructuralStoreDisposition::ReplaceDiscardable;
                            if self.verify_stores && disposition != required {
                                return Err(Error::malformed(
                                    "a vacant store targets established storage behind a reference",
                                ));
                            }
                            self.stores.push((operation.id, required));
                        }
                        // A field store produces no structural result.
                        return Ok(());
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
                    if state.regions[index(destination.get())?].is_none() {
                        self.read(state, *destination, path, site)?;
                    } else if let Some(region) = &mut state.regions[index(destination.get())?] {
                        if !exact && !region.available(path) {
                            return Err(Error::unsupported(
                                "runtime-indexed replacement in an incomplete affine array",
                            ));
                        }
                        // Incoming frontiers shrink until all predecessor
                        // paths agree. A join may dispose a field on the path
                        // that still holds it, so classify stores only after
                        // that agreement, never from a provisional arrival.
                        if settled {
                            let required = if region.available(path) {
                                StructuralStoreDisposition::ReplaceDiscardable
                            } else if self.vacant_region(*destination, region, path)? {
                                StructuralStoreDisposition::Vacant
                            } else {
                                return Err(Error::unsupported(
                                    "replacement of a partially moved region",
                                ));
                            };
                            if let Some(disposition) = declared_disposition {
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
                        if !region.base && !path.is_empty() {
                            return Err(Error::unsupported(
                                "projected reestablishment of an ended owner",
                            ));
                        }
                        if exact {
                            region.set(path, true);
                        }
                    }
                }
            }
            kind => {
                for argument in kind.structural_arguments() {
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
        if !selected.base && !selected.children.is_empty() {
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

    fn dispose(
        &self,
        place: PlaceId,
        region: &Region,
        max_residuals: usize,
    ) -> Result<Vec<TerminalAffineCleanupAction>, Error> {
        use std::collections::BTreeSet;
        let info = self
            .root(place)?
            .ok_or(Error::malformed("missing affine owner"))?;
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
        &self,
        place: PlaceId,
        held: &Region,
        joined: &Region,
        max_residuals: usize,
    ) -> Result<Vec<TerminalAffineCleanupAction>, Error> {
        use std::collections::BTreeSet;
        let info = self
            .root(place)?
            .ok_or(Error::malformed("missing affine owner"))?;
        // A missing ancestor with individually rebuilt children cannot be
        // flattened to an absent ancestor: that would hide its live children.
        // Receiving stores do not serve that partial-construction state yet.
        if held.has_rebuilt_hole() || joined.has_rebuilt_hole() {
            return Err(Error::unsupported(
                "a partially rebuilt home at a reconvergence",
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

    fn root(&self, root: PlaceId) -> Result<Option<&Root>, Error> {
        self.roots
            .get(index(root.get())?)
            .map(Option::as_ref)
            .ok_or(Error::malformed("place is outside the machine"))
    }

    /// A reference carrier stays live while its exact captured lender holds
    /// the window. Resolve at the use, so reassignment cannot retarget a hole.
    fn backing(
        &mut self,
        state: &State,
        root: PlaceId,
        path: &[StructuralPathSegment],
        site: Site,
    ) -> Result<(PlaceId, Vec<StructuralPathSegment>), Error> {
        if !path.contains(&StructuralPathSegment::Referent) {
            return Ok((root, path.to_vec()));
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
        let captured = self
            .references
            .as_mut()
            .ok_or(Error::unsupported(
                "a borrowed reference window requires its module's exact backing",
            ))?
            .location(machine, position, &location)
            .ok_or(Error::unsupported(
                "a borrowed window has no exact live backing correspondence",
            ))?;
        let declaration = body
            .structural_place_type(captured.root)
            .ok_or(Error::unsupported(
                "a borrowed window has no typed lender storage",
            ))?;
        let path =
            crate::descriptor_lifetimes::descriptor_path(self.types, declaration, &captured.path)
                .ok_or(Error::unsupported(
                "a borrowed window has no typed lender path",
            ))?;
        Ok((captured.root, path))
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

    /// The region an observation of `path` under `root` depends on, and
    /// whether that region is exactly the projected place.
    ///
    /// An inline projection whose every step is static has its own region.
    /// A reference's backing is separate storage: an observation depends on
    /// the live descriptor, while loan checking judges access to the backing.
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
        if let Some(referent) = path
            .iter()
            .position(|segment| matches!(segment, StructuralPathSegment::Referent))
            && let Some((descriptor, _)) = crate::canonical_static_projection(
                self.types.iter(),
                declaration.structural_type,
                &path[..referent],
            )
        {
            if !matches!(
                crate::runtime_read_projection_tip(
                    self.types.iter(),
                    declaration.structural_type,
                    path,
                ),
                Some(crate::Position::Declared(_) | crate::Position::ScalarField(_))
            ) {
                return Err(outside);
            }
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
                    unrestored: false,
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
            // ownership.md, "Borrowed-storage invariant windows": "An owned
            // move out of a place reached through `&mut` may temporarily
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
            region.set(path, false);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{StructuralFieldId, StructuralPlaceKind};

    fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }

    fn transferring(
        sources: &[StructuralTypeId],
        destinations: &[StructuralTypeId],
        arguments: Vec<StructuralArgument>,
    ) -> TerminalMachine {
        use terminal_psi::{
            Block, BoundaryParameterKind, MachineContract, StructuralParameterDeclaration,
            StructuralPlaceDeclaration, Terminator,
        };
        let parameter = |place, position, structural_type| StructuralParameterDeclaration {
            place,
            position,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        };
        let mut contract = MachineContract::empty(id(1));
        contract.structural_parameters = sources
            .iter()
            .enumerate()
            .map(|(position, ty)| parameter(id(position as u64 + 1), position as u32, *ty))
            .collect();
        contract.parameter_order = vec![BoundaryParameterKind::Structural; sources.len()];
        let entry = Block::plain(
            id(1),
            Vec::new(),
            Terminator::Jump {
                edge: id(1),
                target: id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: arguments,
                cleanup_actions: Vec::new(),
            },
        );
        let mut target = Block::plain(
            id(2),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: id(2),
                cleanup_actions: Vec::new(),
            },
        );
        target.structural_parameters = destinations
            .iter()
            .enumerate()
            .map(|(position, ty)| {
                parameter(
                    id((sources.len() + position) as u64 + 1),
                    position as u32,
                    *ty,
                )
            })
            .collect();
        let structural_places = contract
            .structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
            })
            .chain(target.structural_parameters.iter().map(|parameter| {
                StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: id(2),
                        position: parameter.position,
                    },
                }
            }))
            .collect();
        TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: id(1),
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places,
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id(1),
            blocks: vec![entry, target],
            contract,
        }
    }

    fn field_argument(field: u32) -> StructuralArgument {
        StructuralArgument {
            place: id(1),
            path: vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(field),
            )],
            access: StructuralAccess::Owned,
        }
    }

    #[test]
    fn successor_values_are_captured_before_source_cleanup_and_destination_binding() {
        use terminal_psi::Terminator;
        let types = reconvergence_types();
        let mut machine = transferring(
            &[id(2)],
            &[id(3), id(3)],
            vec![field_argument(3), field_argument(4)],
        );
        let planned = plan(&machine, &types).unwrap();
        assert!(planned.unavailable.is_empty());
        assert_eq!(
            planned.cleanup,
            vec![
                (id(1), vec![TerminalAffineCleanupAction::RetireHome(id(1))]),
                (
                    id(2),
                    vec![
                        TerminalAffineCleanupAction::DiscardRoot(id(3)),
                        TerminalAffineCleanupAction::DiscardRoot(id(2))
                    ]
                ),
            ]
        );
        let Terminator::Jump {
            cleanup_actions, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        *cleanup_actions = planned.cleanup[0].1.clone();
        let Terminator::ReturnUnit {
            cleanup_actions, ..
        } = &mut machine.blocks[1].terminator
        else {
            unreachable!()
        };
        *cleanup_actions = planned.cleanup[1].1.clone();
        verify(&machine, &types).unwrap();
        let Terminator::Jump {
            cleanup_actions, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanup_actions.clear();
        assert!(
            verify(&machine, &types).is_err(),
            "the source's empty home still needs its own retirement"
        );

        let duplicate = transferring(
            &[id(2)],
            &[id(3), id(3)],
            vec![field_argument(3), field_argument(3)],
        );
        assert!(
            !plan(&duplicate, &types).unwrap().unavailable.is_empty(),
            "one projected owner cannot supply both parameters"
        );
        let short = transferring(&[id(2)], &[id(3), id(3)], vec![field_argument(3)]);
        assert!(plan(&short, &types).is_err());
        let mut borrowed = field_argument(3);
        borrowed.access = StructuralAccess::SharedBorrow;
        assert!(plan(&transferring(&[id(2)], &[id(3)], vec![borrowed]), &types).is_err());
        let mut borrowed_source = transferring(&[id(2)], &[id(3)], vec![field_argument(3)]);
        borrowed_source.contract.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        borrowed_source.contract.structural_parameters[0].multiplicity =
            terminal_psi::StructuralMultiplicity::Unrestricted;
        assert!(matches!(
            plan(&borrowed_source, &types),
            Err(Error::Unsupported {
                detail: "owned extraction from borrowed storage",
                ..
            })
        ));
        let mut foreign = types.to_vec();
        let mut same_layout = foreign[2].clone();
        same_layout.id = id(4);
        same_layout.identity = terminal_psi::TerminalIdentityIndex(4);
        foreign.push(same_layout);
        assert!(
            plan(
                &transferring(&[id(2)], &[id(4)], vec![field_argument(3)]),
                &foreign
            )
            .is_err(),
            "matching bytes do not establish nominal type identity"
        );
    }

    fn borrowed_backedge(access: StructuralAccess) -> TerminalMachine {
        let mut machine = transferring(
            &[id(3)],
            &[id(3)],
            vec![StructuralArgument::whole(id(1), access)],
        );
        for parameter in machine
            .contract
            .structural_parameters
            .iter_mut()
            .chain(&mut machine.blocks[1].structural_parameters)
        {
            parameter.access = access;
            parameter.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
        }
        machine.blocks[1].terminator = terminal_psi::Terminator::Jump {
            edge: id(2),
            target: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument::whole(id(2), access)],
            cleanup_actions: Vec::new(),
        };
        machine
    }

    #[test]
    fn a_borrowed_successor_keeps_its_owned_backing_until_the_last_use() {
        use terminal_psi::{Operation, OperationKind, OperationResult, StructuralMultiplicity};
        let types = reconvergence_types();
        let mut machine = transferring(
            &[id(3)],
            &[id(3)],
            vec![StructuralArgument::whole(
                id(1),
                StructuralAccess::SharedBorrow,
            )],
        );
        let target = &mut machine.blocks[1];
        target.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        target.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        target.operations.push(Operation::unsuspended(
            id(1),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument::whole(
                    id(2),
                    StructuralAccess::SharedBorrow,
                )],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
        let checked = plan(&machine, &types).unwrap();
        assert!(checked.unavailable.is_empty());
        assert!(
            !checked.cleanup.iter().any(|(edge, actions)| *edge == id(1)
                && actions.iter().any(|action| action.place() == id(1))),
            "the source home must survive capture and successor use"
        );
        assert!(
            checked.cleanup.iter().any(|(edge, actions)| *edge == id(2)
                && actions.iter().any(|action| action.place() == id(1))),
            "the original owner is disposed after its last borrowed use"
        );
        let terminal_psi::Terminator::Jump {
            cleanup_actions, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(id(1)));
        assert!(
            verify(&machine, &types).is_err(),
            "cleanup cannot destroy a still-borrowed source home"
        );
    }

    #[test]
    fn a_dead_borrowed_header_can_rejoin_a_path_that_never_entered_it() {
        use terminal_psi::{
            Block, BoundaryParameterKind, Operation, OperationKind, OperationResult, SuccessorEdge,
            Terminator, ValueDeclaration,
        };
        let types = reconvergence_types();
        let mut machine = borrowed_backedge(StructuralAccess::SharedBorrow);
        machine.contract.parameters.push(ValueDeclaration {
            id: id(1),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        });
        machine
            .contract
            .parameter_order
            .insert(0, BoundaryParameterKind::Scalar);
        let edge = |raw, target, structural_arguments| SuccessorEdge {
            edge: id(raw),
            target: id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            cleanup_actions: Vec::new(),
        };
        machine.blocks[0].terminator = Terminator::Conditional {
            condition: id(1),
            when_true: edge(
                1,
                2,
                vec![StructuralArgument::whole(
                    id(1),
                    StructuralAccess::SharedBorrow,
                )],
            ),
            when_false: edge(2, 3, Vec::new()),
        };
        let use_header = Operation::unsuspended(
            id(1),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument::whole(
                    id(2),
                    StructuralAccess::SharedBorrow,
                )],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        machine.blocks[1].operations.push(use_header.clone());
        machine.blocks[1].terminator = Terminator::Jump {
            edge: id(3),
            target: id(3),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        machine.blocks.push(Block::plain(
            id(3),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: id(4),
                cleanup_actions: Vec::new(),
            },
        ));
        let planned = plan(&machine, &types)
            .expect("the dead borrowed name is not an ownership obligation at the join");
        assert!(planned.unavailable.is_empty());
        assert!(
            planned.cleanup.is_empty(),
            "retiring a borrowed name must not discard its backing"
        );
        let mut use_header = use_header;
        use_header.id = id(2);
        machine.blocks[2].operations.push(use_header);
        assert!(
            !plan(&machine, &types)
                .expect("the joined frontier remains available for checking")
                .unavailable
                .is_empty(),
            "a path that never received the header cannot use that borrow after the join"
        );
    }

    #[test]
    fn borrowed_block_storage_survives_a_backedge_without_becoming_owned() {
        let types = reconvergence_types();
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut machine = borrowed_backedge(access);
            let checked = verify(&machine, &types).unwrap();
            assert!(checked.unavailable.is_empty());
            assert!(
                checked.cleanup.is_empty(),
                "borrowed storage is never an owned cleanup obligation"
            );
            let terminal_psi::Terminator::Jump {
                cleanup_actions, ..
            } = &mut machine.blocks[1].terminator
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::RetireHome(id(2)));
            assert!(
                verify(&machine, &types).is_err(),
                "a backedge cannot dispose its borrowed referent"
            );
        }
    }

    #[test]
    fn borrowed_successors_cannot_strengthen_actual_access_even_for_copyable_storage() {
        for copyable in [false, true] {
            let mut types = reconvergence_types();
            // Use one empty record so copy permission has no nested premise.
            for declaration in &mut types {
                declaration.copyable = copyable;
            }
            let original = borrowed_backedge(StructuralAccess::SharedBorrow);
            assert!(verify(&original, &types).unwrap().unavailable.is_empty());
            for forged in [StructuralAccess::MutableBorrow, StructuralAccess::Owned] {
                let mut machine = original.clone();
                let terminal_psi::Terminator::Jump {
                    structural_arguments,
                    ..
                } = &mut machine.blocks[1].terminator
                else {
                    unreachable!()
                };
                structural_arguments[0].access = forged;
                assert!(
                    matches!(verify(&machine, &types), Err(Error::Malformed { .. })),
                    "the destination header cannot grant {forged:?} to a shared source"
                );
            }
        }
    }

    #[test]
    fn a_successor_keeps_the_transferred_field_out_of_its_sources_disposal() {
        let types = reconvergence_types();
        let planned = plan(
            &transferring(&[id(2)], &[id(3)], vec![field_argument(3)]),
            &types,
        )
        .unwrap();
        assert!(planned.unavailable.is_empty());
        assert_eq!(
            planned.cleanup,
            vec![
                (
                    id(1),
                    vec![
                        TerminalAffineCleanupAction::DiscardResidual(
                            terminal_psi::StructuralAffineDiscard {
                                place: id(1),
                                path: field_argument(4).path,
                                structural_type: id(3),
                            }
                        ),
                        TerminalAffineCleanupAction::RetireHome(id(1))
                    ]
                ),
                (id(2), vec![TerminalAffineCleanupAction::DiscardRoot(id(2))]),
            ]
        );
    }

    #[test]
    fn a_backedge_captures_both_swapped_owners_before_rebinding_their_names() {
        use terminal_psi::{
            Block, BoundaryParameterKind, SuccessorEdge, Terminator, ValueDeclaration,
        };
        let types = reconvergence_types();
        let mut machine = transferring(
            &[id(3), id(3)],
            &[id(3), id(3)],
            vec![
                StructuralArgument::whole(id(1), StructuralAccess::Owned),
                StructuralArgument::whole(id(2), StructuralAccess::Owned),
            ],
        );
        machine.contract.parameters.push(ValueDeclaration {
            id: id(1),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        });
        machine
            .contract
            .parameter_order
            .insert(0, BoundaryParameterKind::Scalar);
        let edge = |raw, target, structural_arguments| SuccessorEdge {
            edge: id(raw),
            target: id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            cleanup_actions: Vec::new(),
        };
        machine.blocks[1].terminator = Terminator::Conditional {
            condition: id(1),
            when_true: edge(
                2,
                2,
                vec![
                    StructuralArgument::whole(id(4), StructuralAccess::Owned),
                    StructuralArgument::whole(id(3), StructuralAccess::Owned),
                ],
            ),
            when_false: edge(3, 3, Vec::new()),
        };
        machine.blocks.push(Block::plain(
            id(3),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: id(4),
                cleanup_actions: Vec::new(),
            },
        ));
        let planned = plan(&machine, &types).unwrap();
        assert!(planned.unavailable.is_empty());
        assert_eq!(
            planned.cleanup,
            vec![(
                id(3),
                vec![
                    TerminalAffineCleanupAction::DiscardRoot(id(4)),
                    TerminalAffineCleanupAction::DiscardRoot(id(3))
                ]
            )]
        );
        let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
            unreachable!()
        };
        when_true.structural_arguments[1] = when_true.structural_arguments[0].clone();
        assert!(!plan(&machine, &types).unwrap().unavailable.is_empty());
    }

    fn field(raw: u64) -> Segment {
        Segment::Field(StructuralFieldId::new(raw).unwrap())
    }

    #[test]
    fn availability_is_owned_by_the_region_not_its_scalar_leaves() {
        let mut value = Region::new(true);
        assert!(value.available(&[]));
        value.set(&[], false);
        assert!(!value.available(&[]), "a moved empty value is still moved");
    }

    #[test]
    fn partial_moves_preserve_siblings_and_exact_replacement_restores_the_parent() {
        let mut value = Region::new(true);
        value.set(&[field(1), field(2)], false);
        assert!(!value.available(&[]));
        assert!(!value.available(&[field(1)]));
        assert!(value.available(&[field(1), field(3)]));
        assert!(value.available(&[field(4)]));
        let mut vacancies = Vec::new();
        value.vacancies(&mut Vec::new(), &mut vacancies);
        assert_eq!(vacancies, [vec![field(1), field(2)]]);
        value.set(&[field(1), field(2)], true);
        assert_eq!(value, Region::new(true));
    }

    #[test]
    fn a_join_owns_only_what_every_arrival_still_owns() {
        let mut moved_a = Region::new(true);
        moved_a.set(&[field(1)], false);
        let mut moved_b = Region::new(true);
        moved_b.set(&[field(2)], false);
        let joined = moved_a.intersection(&moved_b);
        assert!(!joined.available(&[field(1)]));
        assert!(!joined.available(&[field(2)]));
        assert!(joined.available(&[field(3)]));
        assert_eq!(joined, moved_b.intersection(&moved_a));
        assert_eq!(Region::new(true).intersection(&moved_a), moved_a);
        let mut vacancies = Vec::new();
        joined.vacancies(&mut Vec::new(), &mut vacancies);
        assert_eq!(vacancies, [vec![field(1)], vec![field(2)]]);
    }

    fn reconvergence_types() -> [StructuralTypeDeclaration; 3] {
        use terminal_psi::{
            BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
            StructuralRecordStorage, StructuralTypeShape, TerminalIdentityIndex,
        };

        let member = |raw, child| StructuralFieldDeclaration {
            id: StructuralFieldId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(StructuralTypeId::new(child).unwrap()),
            storage: None,
            inline_carrier: None,
        };
        let record = |raw, fields| StructuralTypeDeclaration {
            id: StructuralTypeId::new(raw).unwrap(),
            identity: TerminalIdentityIndex(raw as u32),
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            quotient_carrier: None,
            scalar_restriction: None,
            requires: Vec::new(),
            shape: StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            },
        };
        [
            record(1, vec![member(1, 2), member(2, 2)]),
            record(2, vec![member(3, 3), member(4, 3)]),
            record(3, Vec::new()),
        ]
    }

    fn owned_analysis(types: &[StructuralTypeDeclaration]) -> Analysis<'_> {
        Analysis {
            types,
            borrowed_headers: Vec::new(),
            roots: vec![Some(Root {
                structural_type: StructuralTypeId::new(1).unwrap(),
                owned: true,
                affine: true,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                whole_restoration: true,
                parameter: false,
                exclusive: false,
            })],
            unavailable: Vec::new(),
            unavailable_roots: vec![None],
            stores: Vec::new(),
            verify_stores: false,
            references: None,
            body: None,
            residences: Vec::new(),
            position: None,
            reference_candidates: None,
            reference_ends: Vec::new(),
        }
    }

    #[test]
    fn an_invalid_last_use_path_keeps_its_scheduled_operation() {
        use terminal_psi::{Operation, OperationKind, OperationResult};

        let types = reconvergence_types();
        let place = id(1);
        let operation = id(1);
        let mut machine = transferring(&[id(1)], &[], Vec::new());
        let block = machine.entry;
        machine.blocks[0].operations.push(Operation::unsuspended(
            operation,
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: place,
                path: Vec::new(),
            },
        ));
        let argument = StructuralArgument {
            place,
            path: vec![StructuralPathSegment::FixedIndex(7)],
            access: StructuralAccess::Owned,
        };
        let candidates = crate::reference_ends::Candidates::from([(
            (block, 0),
            vec![crate::reference_ends::Candidate {
                argument,
                unproved_case: None,
            }],
        )]);
        let mut analysis = owned_analysis(&types);
        analysis.reference_candidates = Some(&candidates);
        let state = State {
            regions: vec![Some(Region::new(true))],
            active: vec![place],
        };
        let result = analysis.walk(&machine, block, state, &[], false, None);
        assert!(matches!(result, Err(Error::Unsupported {
            detail: "affine projection outside the static structural vocabulary",
            site: Some(Site::Operation(at)),
        }) if at == operation));
    }

    #[test]
    fn a_reference_release_consumes_only_the_owned_descriptor() {
        use terminal_psi::{
            Operation, OperationKind, OperationResult, StructuralTypeShape,
            TerminalIdentityIndex as Identity,
        };
        let mut types = reconvergence_types();
        types[1].shape = StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(3).unwrap(),
            access: StructuralAccess::MutableBorrow,
        };
        let place = PlaceId::new(1).unwrap();
        let mut analysis = owned_analysis(&types);
        let mut state = State {
            regions: vec![Some(Region::new(true))],
            active: vec![place],
        };
        let release = Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: place,
                path: vec![StructuralPathSegment::Field(Identity(1))],
            },
        );
        analysis.operation(&mut state, &release, true).unwrap();
        let region = state.regions[0].as_ref().unwrap();
        assert!(!region.available(&[field(1)]));
        assert!(region.available(&[field(2)]));
        assert_eq!(
            analysis.dispose(place, region, 1).unwrap(),
            [
                TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place,
                        path: vec![StructuralPathSegment::Field(Identity(2))],
                        structural_type: StructuralTypeId::new(2).unwrap(),
                    }
                ),
                TerminalAffineCleanupAction::RetireHome(place),
            ]
        );
        let mut repeated = release.clone();
        repeated.id = OperationId::new(2).unwrap();
        analysis.operation(&mut state, &repeated, true).unwrap();
        assert!(
            matches!(analysis.unavailable.as_slice(), [Unavailable {site: Site::Operation(id), place: found, ..}]
            if *id == repeated.id && *found == place)
        );
        let mut borrowed = owned_analysis(&types);
        borrowed.roots[0].as_mut().unwrap().owned = false;
        let mut borrowed_state = State {
            regions: vec![Some(Region::new(true))],
            active: Vec::new(),
        };
        let before = borrowed_state.clone();
        assert!(matches!(
            borrowed.operation(&mut borrowed_state, &release, true),
            Err(Error::Unsupported {
                detail: "owned extraction from borrowed storage",
                ..
            })
        ));
        assert_eq!(borrowed_state, before);
    }

    #[test]
    fn a_reference_observation_needs_its_descriptor_without_owning_its_backing() {
        use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex as Identity};

        let mut types = reconvergence_types();
        types[0].shape = StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(2).unwrap(),
            access: StructuralAccess::MutableBorrow,
        };
        let mut analysis = owned_analysis(&types);
        let place = PlaceId::new(1).unwrap();
        let site = Site::Operation(OperationId::new(1).unwrap());
        let mut state = State {
            regions: vec![Some(Region::new(true))],
            active: vec![place],
        };
        let mut argument = StructuralArgument {
            place,
            path: vec![
                StructuralPathSegment::Referent,
                StructuralPathSegment::Field(Identity(3)),
            ],
            access: StructuralAccess::SharedBorrow,
        };
        assert_eq!(
            analysis.observed(place, &argument.path).unwrap(),
            (vec![], false)
        );
        let original = state.clone();
        analysis
            .argument(&mut state, &argument, site, true)
            .unwrap();
        assert_eq!(state, original, "a borrow cannot move referenced storage");
        assert!(analysis.unavailable.is_empty());

        argument.access = StructuralAccess::Owned;
        assert!(matches!(
            analysis.argument(&mut state, &argument, site, true),
            Err(Error::Unsupported { .. })
        ));
        assert_eq!(state, original);
        argument.path[1] = StructuralPathSegment::Field(Identity(999));
        assert!(analysis.observed(place, &argument.path).is_err());

        argument.path[1] = StructuralPathSegment::Field(Identity(3));
        argument.access = StructuralAccess::SharedBorrow;
        state.regions[0] = None;
        analysis
            .argument(&mut state, &argument, site, true)
            .unwrap();
        assert!(
            matches!(analysis.unavailable.as_slice(), [Unavailable { site: found, place: found_place, .. }]
            if *found == site && *found_place == place)
        );
        assert!(
            state.regions[0].is_none(),
            "observation cannot revive a moved descriptor"
        );
    }

    #[test]
    fn a_partly_moved_arrival_discards_only_the_nested_remainder_at_a_join() {
        use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};
        // One arrival transferred selected.first; the other transferred
        // selected whole. Equalizing their retained owner must discard only
        // selected.second, leaving the sibling subtree and moved value alone.
        let types = reconvergence_types();
        let analysis = owned_analysis(&types);
        let mut partial = Region::new(true);
        partial.set(&[field(1), field(3)], false);
        let mut whole = Region::new(true);
        whole.set(&[field(1)], false);
        let place = PlaceId::new(1).unwrap();
        let actions = analysis
            .equalize(place, &partial, &whole, 1)
            .expect("the partial arrival has one exact no-code residual");
        assert_eq!(
            actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                terminal_psi::StructuralAffineDiscard {
                    place,
                    path: vec![
                        StructuralPathSegment::Field(TerminalIdentityIndex(1)),
                        StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                    ],
                    structural_type: StructuralTypeId::new(3).unwrap(),
                },
            )]
        );
        assert!(analysis.equalize(place, &partial, &whole, 0).is_err());
        assert!(
            analysis
                .equalize(place, &whole, &whole, 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reconvergence_scopes_keep_reverse_declaration_order_across_regions() {
        use terminal_psi::{
            StructuralPathSegment as Path, StructuralTypeShape, TerminalIdentityIndex as Identity,
        };
        let mut types = reconvergence_types();
        let StructuralTypeShape::Record { fields, .. } = &mut types[0].shape else {
            unreachable!()
        };
        let mut third = fields[1].clone();
        third.id = StructuralFieldId::new(5).unwrap();
        fields[0].identity = Identity(9);
        fields[1].identity = Identity(2);
        third.identity = Identity(5);
        fields.push(third);
        let moved = [vec![Path::Field(Identity(9)), Path::Field(Identity(3))]]
            .into_iter()
            .collect();
        let scopes = [9, 2, 5]
            .map(|identity| vec![Path::Field(Identity(identity))])
            .into_iter()
            .collect();
        let root = StructuralTypeId::new(1).unwrap();
        let residuals =
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &scopes, 3).unwrap();
        assert_eq!(
            residuals
                .iter()
                .map(|(path, ty)| (path.clone(), ty.get()))
                .collect::<Vec<_>>(),
            [
                (vec![Path::Field(Identity(5))], 2),
                (vec![Path::Field(Identity(2))], 2),
                (vec![Path::Field(Identity(9)), Path::Field(Identity(4))], 3),
            ]
        );
        assert!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &scopes, 2).is_none()
        );
        let sibling = [vec![Path::Field(Identity(2))]].into_iter().collect();
        assert_eq!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &sibling, 1).unwrap(),
            [(
                vec![Path::Field(Identity(2))],
                StructuralTypeId::new(2).unwrap()
            ),]
        );
    }

    #[test]
    fn a_scoped_array_partition_does_not_enumerate_untouched_elements() {
        use terminal_psi::{
            ArrayExtent, StructuralPathSegment as Path, StructuralTypeShape,
            TerminalIdentityIndex as Identity,
        };
        let mut types = reconvergence_types();
        types[0].shape = StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(2).unwrap(),
            length: ArrayExtent::Literal(u64::MAX),
        };
        let root = StructuralTypeId::new(1).unwrap();
        let moved = [vec![Path::FixedIndex(1), Path::Field(Identity(3))]]
            .into_iter()
            .collect();
        let selected = [vec![Path::FixedIndex(1)]].into_iter().collect();
        assert_eq!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &selected, 1).unwrap(),
            [(
                vec![Path::FixedIndex(1), Path::Field(Identity(4))],
                StructuralTypeId::new(3).unwrap()
            ),]
        );
        let whole = [Vec::new()].into_iter().collect();
        assert!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &whole, 1).is_none()
        );
    }

    #[test]
    fn rebuilt_children_cannot_be_erased_as_an_absent_ancestor() {
        let types = reconvergence_types();
        let analysis = owned_analysis(&types);
        let mut partial = Region::new(true);
        partial.set(&[field(1)], false);
        partial.set(&[field(1), field(3)], true);
        assert!(matches!(
            analysis.equalize(PlaceId::new(1).unwrap(), &partial, &Region::new(false), 2),
            Err(Error::Unsupported { .. })
        ));
    }

    #[test]
    fn a_common_field_scope_does_not_require_or_invent_payload_membership() {
        use semantic_vocabulary::StructuralCaseId;
        use std::collections::BTreeSet;
        use terminal_psi::{
            StructuralCaseDeclaration, StructuralPathSegment as Path, StructuralTypeShape,
            TerminalIdentityIndex as Identity,
        };
        let mut types = reconvergence_types();
        let fields = types[0].shape.direct_fields().unwrap().to_vec();
        let mut second_payload = fields[1].clone();
        second_payload.id = StructuralFieldId::new(5).unwrap();
        second_payload.identity = Identity(5);
        types[0].shape = StructuralTypeShape::Mixed {
            fields: vec![fields[0].clone()],
            cases: vec![
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).unwrap(),
                    identity: Identity(10),
                    fields: vec![fields[1].clone()],
                },
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).unwrap(),
                    identity: Identity(11),
                    fields: vec![second_payload],
                },
            ],
        };
        let root = StructuralTypeId::new(1).unwrap();
        let common = [vec![Path::Field(Identity(1))]].into_iter().collect();
        let missing = BTreeSet::new();
        assert_eq!(
            crate::partial_affine_residuals_in_regions(&types, root, &missing, &common, 1).unwrap(),
            [(
                vec![Path::Field(Identity(1))],
                StructuralTypeId::new(2).unwrap()
            ),]
        );
        let payload = [vec![
            Path::Case(StructuralCaseId::new(1).unwrap()),
            Path::Field(Identity(2)),
        ]]
        .into_iter()
        .collect();
        assert!(
            crate::partial_affine_residuals_in_regions(&types, root, &missing, &payload, 1)
                .is_none(),
            "a receiving scope does not establish its payload case"
        );
        let moved = [vec![
            Path::Case(StructuralCaseId::new(1).unwrap()),
            Path::Field(Identity(2)),
            Path::Field(Identity(3)),
        ]]
        .into_iter()
        .collect();
        assert_eq!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &payload, 1).unwrap(),
            [(
                vec![
                    Path::Case(StructuralCaseId::new(1).unwrap()),
                    Path::Field(Identity(2)),
                    Path::Field(Identity(4))
                ],
                StructuralTypeId::new(3).unwrap()
            ),]
        );
    }

    #[test]
    fn scoped_cleanup_does_not_inspect_an_unrequested_erased_field() {
        use terminal_psi::{
            BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
            StructuralPathSegment as Path, StructuralTypeShape, TerminalIdentityIndex as Identity,
        };
        let mut types = reconvergence_types();
        let StructuralTypeShape::Record { fields, .. } = &mut types[0].shape else {
            unreachable!()
        };
        fields.push(StructuralFieldDeclaration {
            id: StructuralFieldId::new(5).unwrap(),
            identity: Identity(50),
            relevance: BindingRelevance::Erased,
            field_type: StructuralFieldType::Erased {
                type_identity: Identity(60),
            },
            storage: None,
            inline_carrier: None,
        });
        let root = StructuralTypeId::new(1).unwrap();
        let moved = [vec![Path::Field(Identity(1)), Path::Field(Identity(3))]]
            .into_iter()
            .collect();
        let selected = [vec![Path::Field(Identity(1))]].into_iter().collect();
        assert_eq!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &selected, 1)
                .expect("an unrelated erased field is outside the cleanup scope"),
            [(
                vec![Path::Field(Identity(1)), Path::Field(Identity(4))],
                StructuralTypeId::new(3).unwrap()
            ),]
        );
        let erased = [vec![Path::Field(Identity(50))]].into_iter().collect();
        assert!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &erased, 1).is_none(),
            "scoping cannot create runtime custody for an erased field"
        );
        let whole = [Vec::new()].into_iter().collect();
        assert!(
            crate::partial_affine_residuals_in_regions(&types, root, &moved, &whole, 3).is_none()
        );
        assert!(
            crate::partial_affine_residuals(&types, root, &moved, 3).is_none(),
            "the existing whole-partition refusal remains unchanged"
        );
    }
}
