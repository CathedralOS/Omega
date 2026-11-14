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
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalMachine,
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
    /// Owned copyable storage with trivial cleanup may be discarded or retired.
    /// Borrowed storage grants no such authority.
    pub copyable_owned_roots: IdSet<PlaceId>,
    pub cleanup: Vec<(EdgeId, Vec<TerminalAffineCleanupAction>)>,
    /// Exact receiving regions determine whether a store fills a vacancy or
    /// replaces intact discardable contents. Sorted by operation identity.
    pub stores: Vec<(OperationId, StructuralStoreDisposition)>,
    pub unavailable: Vec<Unavailable>,
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
    parameter: bool,
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

struct Analysis<'a> {
    types: &'a [StructuralTypeDeclaration],
    roots: Vec<Option<Root>>,
    unavailable: Vec<Unavailable>,
    unavailable_roots: Vec<Option<Site>>,
    stores: Vec<(OperationId, StructuralStoreDisposition)>,
    verify_stores: bool,
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
        T::StructuralCase { source, .. } | T::ReturnStructural { source, .. } => vec![*source],
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

fn live_entries(machine: &TerminalMachine) -> Result<Vec<IdSet<PlaceId>>, Error> {
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
            for operation in block.operations.iter().rev() {
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
            }
            for parameter in &block.structural_parameters {
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
    analyze(machine, types, false)
}

/// Reconstructs the canonical schedule without trusting the supplied actions.
/// The producer's actual output bounds residual enumeration before allocation.
/// Exact equality then checks root/path/type identity, maximality and order.
pub fn verify(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
) -> Result<Plan, Error> {
    let plan = analyze(machine, types, true)?;
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
                // Affine cleanup must match the schedule below. Copyable
                // storage may be discarded or retired only by its owner.
                if !plan.affine_roots.contains(action.place())
                    && (!matches!(
                        action,
                        TerminalAffineCleanupAction::RetireHome(_)
                            | TerminalAffineCleanupAction::DiscardRoot(_)
                    ) || !plan.copyable_owned_roots.contains(action.place()))
                {
                    return Err(Error::unsupported(
                        "disposal of storage the affine schedule does not own",
                    ));
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

fn analyze(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    bound_output: bool,
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
    let mut insert_contract =
        |place: PlaceId, structural_type, multiplicity, access| -> Result<(), Error> {
            let slot = contracts
                .get_mut(index(place.get())?)
                .ok_or(Error::malformed(
                    "custody declaration is outside the machine",
                ))?;
            if slot
                .replace((structural_type, multiplicity, access))
                .is_some()
            {
                return Err(Error::malformed("a place has two custody declarations"));
            }
            Ok(())
        };
    for parameter in machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
    {
        insert_contract(
            parameter.place,
            parameter.structural_type,
            parameter.multiplicity,
            parameter.access,
        )?;
    }
    for operation in machine.operations() {
        if let Some(result) = operation.result.as_structural() {
            insert_contract(
                result.place,
                result.structural_type,
                result.multiplicity,
                StructuralAccess::Owned,
            )?;
        }
    }
    if let TerminalMachineResult::Structural(result) = &machine.result {
        insert_contract(
            result.place,
            result.structural_type,
            result.multiplicity,
            StructuralAccess::Owned,
        )?;
    }
    let mut roots = Vec::with_capacity(machine.structural_places.len());
    let mut copyable_owned_roots = IdSet::new();
    for place in &machine.structural_places {
        if index(place.id.get())? != roots.len() {
            return Err(Error::malformed("places are not in dense identity order"));
        }
        let (ty, owned, parameter) = match place.kind {
            StructuralPlaceKind::Parameter { .. } => {
                let (ty, _, access) = contracts[roots.len()]
                    .ok_or(Error::malformed("parameter place has no signature"))?;
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
                if parameter.access != StructuralAccess::Owned {
                    return Err(Error::unsupported(
                        "borrowed structural block parameter storage",
                    ));
                }
                (parameter.structural_type, true, false)
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
        if let Some((declared_type, multiplicity, access)) = contracts[roots.len()] {
            let expected = if declaration.copyable || access != StructuralAccess::Owned {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else if declaration.owns_linear_claim {
                terminal_psi::StructuralMultiplicity::Linear
            } else {
                terminal_psi::StructuralMultiplicity::Affine
            };
            if declared_type != ty || multiplicity != expected {
                return Err(Error::malformed(
                    "place custody disagrees with its exact type declaration",
                ));
            }
        }
        let tracked = !declaration.copyable;
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
            && declaration.copyable
            && declaration.no_code_affine_cleanup
            && declaration.descriptor_cleanup
        {
            copyable_owned_roots.insert(place.id);
        }
        roots.push(tracked.then_some(Root {
            structural_type: ty,
            owned,
            parameter,
        }));
    }
    if roots.iter().all(Option::is_none) {
        return Ok(Plan {
            copyable_owned_roots,
            ..Plan::default()
        });
    }
    let mut affine_roots = IdSet::new();
    for (slot, root) in roots.iter().enumerate() {
        if root.is_some() {
            affine_roots.insert(
                PlaceId::new(slot as u64 + 1).ok_or(Error::malformed("place identity overflow"))?,
            );
        }
    }
    let live = live_entries(machine)?;
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
    let initial = State { regions, active };
    let first = index(machine.entry.get())?;
    if first >= machine.blocks.len() {
        return Err(Error::malformed("entry block is outside the machine"));
    }
    let mut analysis = Analysis {
        types,
        roots,
        unavailable: Vec::new(),
        unavailable_roots: Vec::new(),
        stores: Vec::new(),
        verify_stores: bound_output,
    };
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
                ));
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
    })
}

impl Analysis<'_> {
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
        for operation in &block.operations {
            self.operation(&mut state, operation, entering.is_some())
                .map_err(|error| error.at(Site::Operation(operation.id)))?;
        }
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
            let mut actions = Vec::new();
            let mut retained = Vec::new();
            for place in std::mem::take(&mut outgoing.active).into_iter().rev() {
                let raw = index(place.get())?;
                if let Some(region) = &outgoing.regions[raw] {
                    let read_there = needed.is_some_and(|needed| needed.contains(place));
                    let there = owned_there.map(|there| there.regions[raw].as_ref());
                    if read_there && there.is_none_or(|there| there.is_some()) {
                        if let Some(Some(there)) = there
                            && there != region
                        {
                            let discards = self.equalize(place, region, there, residual_budget)?;
                            residual_budget = residual_budget.checked_sub(discards.len()).ok_or(
                                Error::malformed("affine residual partition exceeds its output"),
                            )?;
                            actions.extend(discards);
                            outgoing.regions[raw] = Some(there.clone());
                        }
                        retained.push(place);
                    } else {
                        let disposal = self.dispose(place, region, residual_budget)?;
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
            let source = machine
                .structural_place_type(argument.place)
                .ok_or(Error::malformed(
                    "structural successor argument has no source type",
                ))?;
            if crate::runtime_structural_path_tip(self.types.iter(), source, &argument.path)
                .is_none_or(|selected| selected.id != parameter.structural_type)
            {
                return Err(Error::malformed(
                    "structural successor argument differs from its parameter type",
                ));
            }
        }
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
            if self.root(parameter.place)?.is_some_and(|root| root.owned)
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
        if let Op::StructuralLeafCopy { source, path } = &operation.kind
            && let Some(root) = self.root(*source)?
            && crate::canonical_static_projection(self.types.iter(), root.structural_type, path)
                .is_some_and(|(_, declaration)| !declaration.copyable)
        {
            return Err(Error::malformed("a leaf copy duplicates an affine region"));
        }
        match &operation.kind {
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
            Op::WriteOnlyPrimitiveStore { .. } => {}
            Op::StoreStructuralField { value, .. } => self.argument(state, value, site, false)?,
            kind => {
                for used in kind.storage_uses() {
                    self.storage_use(state, used, site)?;
                }
            }
        }
        match &operation.kind {
            Op::MoveStructuralValue { source } => self.argument(state, source, site, true)?,
            kind if kind.constructor_arguments().next().is_some() => {
                // Every constructor transfers its actual owned children. The
                // same roster covers records, cases and fixed/stored arrays;
                // their moved homes must not also receive edge cleanup.
                for argument in kind.constructor_arguments() {
                    self.argument(state, argument, site, true)?;
                }
            }
            Op::StoreStructuralField {
                disposition,
                destination,
                path,
                field,
                value,
            } => {
                self.argument(state, value, site, true)?;
                if let Some(root) = self.root(*destination)? {
                    // The incoming value transfers from its exact source
                    // above. The receiving region may be an indexed element
                    // only when the entire reached array is already intact:
                    // replacing one discardable value then preserves that
                    // availability without identifying or filling a hole.
                    let (mut receiving, exact) = self.observed(*destination, path)?;
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
                            if self.verify_stores && *disposition != required {
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

    fn canonical(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<Segment>, Error> {
        let declaration = self
            .root(root)?
            .ok_or(Error::malformed("untracked affine root"))?;
        crate::canonical_static_projection(self.types.iter(), declaration.structural_type, path)
            .map(|(path, _)| path)
            .or_else(|| {
                crate::canonical_primitive_read_path(
                    self.types.iter(),
                    declaration.structural_type,
                    path,
                )
            })
            .or_else(|| {
                // A byte field's descriptor is inline storage of its record,
                // so its region is the carrier's path and the field. The
                // static projection stops there because a view's bytes lie
                // outside the record; the descriptor does not. A payload
                // field's carrier is the sum, and its region lies under the
                // case, as a scalar payload leaf's does.
                let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
                    return None;
                };
                let field = crate::runtime_structural_field(
                    self.types.iter(),
                    declaration.structural_type,
                    path,
                )?;
                field.field_type.byte_sequence_carrier()?;
                let (carrier, case) = match carrier.split_last() {
                    Some((StructuralPathSegment::Case(case), sum)) => (sum, Some(*case)),
                    _ => (carrier, None),
                };
                let (mut canonical, _) = crate::canonical_static_projection(
                    self.types.iter(),
                    declaration.structural_type,
                    carrier,
                )?;
                canonical.extend(case.map(Segment::Case));
                canonical.push(Segment::Field(field.id));
                Some(canonical)
            })
            .ok_or(Error::unsupported(
                "affine projection outside the static structural vocabulary",
            ))
    }

    /// The region an observation of `path` under `root` depends on, and
    /// whether that region is exactly the projected place.
    ///
    /// A projection whose every step is static has exactly its own region.
    /// An element chosen at run time has none of its own: which element it
    /// is is not known here, so the observation depends on the whole fixed
    /// array it indexes and is answered from that array's region. This is
    /// the only widening, and it is one-sided: it can refuse an observation
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
        let Some(indexed) = path
            .iter()
            .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        else {
            return Err(outside);
        };
        let Some((array, indexed_type)) = crate::canonical_static_projection(
            self.types.iter(),
            declaration.structural_type,
            &path[..indexed],
        ) else {
            return Err(outside);
        };
        // An element view's elements lie outside the storage that holds the
        // view, so only an inline array's region covers its elements. The
        // rest of the path must still be a projection the types declare.
        if !matches!(
            indexed_type.shape,
            terminal_psi::StructuralTypeShape::FixedArray { .. }
        ) || !matches!(
            crate::runtime_read_projection_tip(
                self.types.iter(),
                declaration.structural_type,
                path,
            ),
            Some(crate::Position::Declared(_) | crate::Position::ScalarField(_))
        ) {
            return Err(outside);
        }
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
                self.unavailable.push(Unavailable { site, place });
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
        let path = self
            .canonical(argument.place, &argument.path)
            .map_err(|error| error.at(site))?;
        self.read(state, argument.place, &path, site)?;
        if consume && argument.access == StructuralAccess::Owned {
            if !owned {
                return Err(Error::unsupported("owned extraction from borrowed storage"));
            }
            if path.is_empty() {
                // Whole owned transport consumes its source home as well as
                // its value. Only a projected move leaves an allocated owner
                // whose complement needs later disposal and retirement.
                state.regions[index(argument.place.get())?] = None;
            } else if let Some(region) = &mut state.regions[index(argument.place.get())?] {
                region.set(&path, false);
            }
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
            roots: vec![Some(Root {
                structural_type: StructuralTypeId::new(1).unwrap(),
                owned: true,
                parameter: false,
            })],
            unavailable: Vec::new(),
            unavailable_roots: vec![None],
            stores: Vec::new(),
            verify_stores: false,
        }
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
