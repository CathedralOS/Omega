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
    Unsupported(&'static str),
    Malformed(&'static str),
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
            return Err(Error::Malformed("two arrivals track different places"));
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
            return Err(Error::Unsupported(
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
        .ok_or(Error::Malformed("zero or oversized dense identity"))
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
                        .ok_or(Error::Malformed("edge target is outside the machine"))?,
                );
            }
            let valid = |place: PlaceId| -> Result<PlaceId, Error> {
                if index(place.get())? < machine.structural_places.len() {
                    Ok(place)
                } else {
                    Err(Error::Malformed("live place is outside the machine"))
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
            let old = live
                .get_mut(index(block.id.get())?)
                .ok_or(Error::Malformed("block is outside the machine"))?;
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
                    return Err(Error::Malformed("cleanup root is outside the machine"));
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
                    return Err(Error::Unsupported(
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
                return Err(Error::Malformed(
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
            return Err(Error::Malformed("types are not in dense identity order"));
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
                .ok_or(Error::Malformed("nested type is outside the catalog"))?;
            if declaration.copyable && !child.copyable
                || declaration.no_code_affine_cleanup && !child.no_code_affine_cleanup
                || declaration.descriptor_cleanup && !child.descriptor_cleanup
                || !declaration.owns_linear_claim && child.owns_linear_claim
            {
                return Err(Error::Malformed(
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
                .ok_or(Error::Malformed(
                    "custody declaration is outside the machine",
                ))?;
            if slot
                .replace((structural_type, multiplicity, access))
                .is_some()
            {
                return Err(Error::Malformed("a place has two custody declarations"));
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
            return Err(Error::Malformed("places are not in dense identity order"));
        }
        let (ty, owned, parameter) = match place.kind {
            StructuralPlaceKind::Parameter { .. } => {
                let (ty, _, access) = contracts[roots.len()]
                    .ok_or(Error::Malformed("parameter place has no signature"))?;
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
                    return Err(Error::Malformed("result place has no structural signature"));
                };
                (result.structural_type, false, false)
            }
            StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::ProviderAttachment { .. } => {
                return Err(Error::Unsupported("affine block or provider storage"));
            }
        };
        let declaration = types
            .get(index(ty.get())?)
            .filter(|declaration| declaration.id == ty)
            .ok_or(Error::Malformed(
                "structural type is outside its identity table",
            ))?;
        if matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
            && (!declaration.shape.is_shared_readable_byte_view()
                || !declaration.copyable
                || declaration.owns_linear_claim)
        {
            return Err(Error::Malformed(
                "literal backing requires a claim-free shared byte declaration",
            ));
        }
        if matches!(place.kind, StructuralPlaceKind::OperationResult { .. })
            && contracts[roots.len()].is_none()
        {
            return Err(Error::Malformed(
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
                return Err(Error::Malformed(
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
            return Err(Error::Unsupported(
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
                PlaceId::new(slot as u64 + 1).ok_or(Error::Malformed("place identity overflow"))?,
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
            PlaceId::new(slot as u64 + 1).ok_or(Error::Malformed("place identity overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let initial = State { regions, active };
    let first = index(machine.entry.get())?;
    if first >= machine.blocks.len() {
        return Err(Error::Malformed("entry block is outside the machine"));
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
            .ok_or(Error::Malformed("block has no incoming frontier"))?;
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
                return Err(Error::Malformed(
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
            .ok_or(Error::Malformed("block is not at its identity"))?;
        for operation in &block.operations {
            self.operation(&mut state, operation, entering.is_some())?;
        }
        let site = Site::Edge(
            block
                .terminator
                .edges()
                .next()
                .ok_or(Error::Malformed("terminator has no edge"))?,
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
            Terminator::Jump {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    if self.root(argument.place)?.is_some() {
                        return Err(Error::Unsupported("affine block successor arguments"));
                    }
                }
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
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    for argument in &edge.structural_arguments {
                        if self.root(argument.place)?.is_some() {
                            return Err(Error::Unsupported("affine block successor arguments"));
                        }
                    }
                }
            }
            Terminator::Return { .. }
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
        let mut targets = block.terminator.successor_edges();
        targets.sort_by_key(|(edge, _)| *edge);
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
            let target = targets
                .binary_search_by_key(&edge, |(edge, _)| *edge)
                .ok()
                .map(|position| targets[position].1);
            let needed = target
                .map(|target| index(target.get()).map(|index| &live[index]))
                .transpose()?;
            let owned_there = match (entering, target) {
                (Some(entering), Some(target)) => Some(
                    entering
                        .get(index(target.get())?)
                        .and_then(Option::as_ref)
                        .ok_or(Error::Malformed("edge target has no entering frontier"))?,
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
                                Error::Malformed("affine residual partition exceeds its output"),
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
            arrivals.push(Arrival {
                edge,
                target,
                outgoing,
                actions,
            });
        }
        Ok(arrivals)
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
                let path = self.canonical(projection.root, projection.path)?;
                self.read(state, projection.root, &path, site)
            }
            StructuralStorageUse::Canonical { root, path }
            | StructuralStorageUse::ObservedField { root, path } => {
                self.read(state, root, path, site)
            }
            StructuralStorageUse::Type(_) => Ok(()),
            StructuralStorageUse::Element { descriptor, .. } => {
                if self.root(descriptor.root)?.is_some() {
                    return Err(Error::Unsupported("affine element views"));
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
            return Err(Error::Malformed("a leaf copy duplicates an affine region"));
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
                    let mut path = self.canonical(*destination, path)?;
                    path.push(Segment::Field(*field));
                    if state.regions[index(destination.get())?].is_none() {
                        self.read(state, *destination, &path, site)?;
                    } else if !state.regions[index(destination.get())?]
                        .as_ref()
                        .is_some_and(|region| region.available(&path))
                    {
                        return Err(Error::Unsupported(
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
                if self.root(*destination)?.is_some() {
                    let mut path = self.canonical(*destination, path)?;
                    if let Some(field) = field {
                        path.push(Segment::Field(*field));
                    }
                    if state.regions[index(destination.get())?].is_none() {
                        self.read(state, *destination, &path, site)?;
                    } else if let Some(region) = &mut state.regions[index(destination.get())?] {
                        // Incoming frontiers shrink until all predecessor
                        // paths agree. A join may dispose a field on the path
                        // that still holds it, so classify stores only after
                        // that agreement, never from a provisional arrival.
                        if settled {
                            let required = if region.available(&path) {
                                StructuralStoreDisposition::ReplaceDiscardable
                            } else if self.vacant_region(*destination, region, &path)? {
                                StructuralStoreDisposition::Vacant
                            } else {
                                return Err(Error::Unsupported(
                                    "replacement of a partially moved region",
                                ));
                            };
                            if self.verify_stores && *disposition != required {
                                return Err(Error::Malformed(match required {
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
                            return Err(Error::Unsupported(
                                "projected reestablishment of an ended owner",
                            ));
                        }
                        region.set(&path, true);
                    }
                }
            }
            kind => {
                for argument in kind.structural_arguments() {
                    self.argument(state, argument, site, true)?;
                }
            }
        }
        if let Some(result) = operation.result.as_structural()
            && self.root(result.place)?.is_some()
        {
            if self.root(result.place)?.is_some_and(|root| root.owned)
                && state.regions[index(result.place.get())?].is_none()
            {
                state.active.push(result.place);
            }
            state.regions[index(result.place.get())?] = Some(Region::new(!matches!(
                operation.kind,
                Op::EstablishStructuralLocal {}
            )));
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
            .ok_or(Error::Malformed("vacancy has no affine owner"))?;
        let selected_type =
            crate::canonical_structural_path_tip(self.types.iter(), root.structural_type, path)
                .ok_or(Error::Malformed("vacancy has no typed receiving region"))?
                .id;
        let mut paths = Vec::new();
        selected.vacancies(&mut Vec::new(), &mut paths);
        let missing = paths
            .into_iter()
            .map(|path| {
                crate::canonical_structural_storage_path(self.types.iter(), selected_type, &path)
            })
            .collect::<Option<BTreeSet<_>>>()
            .ok_or(Error::Malformed("vacancy has an untyped missing child"))?;
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
            .ok_or(Error::Malformed("missing affine owner"))?;
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
            .ok_or(Error::Malformed("moved region has no typed storage path"))?;
        let residuals = crate::partial_affine_residuals(
            self.types,
            info.structural_type,
            &moved,
            max_residuals,
        )
        .ok_or(Error::Malformed(
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
            .ok_or(Error::Malformed("missing affine owner"))?;
        // A missing ancestor with individually rebuilt children cannot be
        // flattened to an absent ancestor: that would hide its live children.
        // Receiving stores do not serve that partial-construction state yet.
        if held.has_rebuilt_hole() || joined.has_rebuilt_hole() {
            return Err(Error::Unsupported(
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
                .ok_or(Error::Malformed("moved region has no typed storage path"))
        };
        let residuals = crate::partial_affine_residuals_in_regions(
            self.types,
            info.structural_type,
            &missing(held)?,
            &missing(joined)?,
            max_residuals,
        )
        .ok_or(Error::Malformed(
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
            .ok_or(Error::Malformed("place is outside the machine"))
    }

    fn canonical(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<Segment>, Error> {
        let declaration = self
            .root(root)?
            .ok_or(Error::Malformed("untracked affine root"))?;
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
            .ok_or(Error::Unsupported(
                "affine projection outside the static structural vocabulary",
            ))
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
        let path = self.canonical(argument.place, &argument.path)?;
        self.read(state, argument.place, &path, site)?;
        if consume && argument.access == StructuralAccess::Owned {
            if !owned {
                return Err(Error::Unsupported("owned extraction from borrowed storage"));
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
    use semantic_vocabulary::StructuralFieldId;

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
            Err(Error::Unsupported(_))
        ));
    }
}
