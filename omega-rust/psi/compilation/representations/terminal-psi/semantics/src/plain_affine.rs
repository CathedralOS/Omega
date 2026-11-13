//! Ownership of plain affine storage, independently of its scalar contents.
//!
//! A value exists even when it has no scalar leaf (an empty case, for example).
//! Moves therefore change an exact region's availability, while scalar reaching
//! definitions remain a separate analysis. The producer uses the resulting
//! edge schedule; a checker reconstructs it from the same unsealed operations.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, EdgeId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralTypeDeclaration,
    TerminalAffineCleanupAction, TerminalMachine,
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
    pub cleanup: Vec<(EdgeId, Vec<TerminalAffineCleanupAction>)>,
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
}

#[derive(Debug, Clone)]
struct Root {
    structural_type: StructuralTypeId,
    owned: bool,
    parameter: bool,
}

type State = Vec<Option<Region>>;

struct Analysis<'a> {
    types: &'a [StructuralTypeDeclaration],
    roots: Vec<Option<Root>>,
    unavailable: Vec<Unavailable>,
    unavailable_roots: Vec<Option<Site>>,
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

fn live_entries(machine: &TerminalMachine) -> Result<Vec<Vec<bool>>, Error> {
    let mut live = vec![vec![false; machine.structural_places.len()]; machine.blocks.len()];
    loop {
        let mut changed = false;
        for block in machine.blocks.iter().rev() {
            let mut before = vec![false; machine.structural_places.len()];
            for target in block.terminator.successor_targets() {
                let entering = live
                    .get(index(target.get())?)
                    .ok_or(Error::Malformed("edge target is outside the machine"))?;
                for (needed, successor) in before.iter_mut().zip(entering) {
                    *needed |= successor;
                }
            }
            for used in block.terminator.storage_uses() {
                *before
                    .get_mut(index(used.root.get())?)
                    .ok_or(Error::Malformed("terminator place is outside the machine"))? = true;
            }
            for operation in block.operations.iter().rev() {
                if let Some(result) = operation.result.as_structural() {
                    *before
                        .get_mut(index(result.place.get())?)
                        .ok_or(Error::Malformed("result place is outside the machine"))? = false;
                }
                for used in operation
                    .kind
                    .storage_uses()
                    .into_iter()
                    .filter_map(storage_root)
                {
                    *before
                        .get_mut(index(used.get())?)
                        .ok_or(Error::Malformed("operand place is outside the machine"))? = true;
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

/// Plans only no-code affine storage. Nominal hooks, conserved claims and
/// borrowed extraction require their own contracts, rather than an empty plan.
pub fn plan(machine: &TerminalMachine, types: &[StructuralTypeDeclaration]) -> Result<Plan, Error> {
    use terminal_psi::{StructuralPlaceKind, TerminalMachineResult, Terminator};
    // A generic requirement is a check-form signature, not an executable
    // activation. Its caller still transfers each argument normally.
    if machine.blocks.is_empty() {
        return Ok(Plan::default());
    }
    let mut roots = Vec::with_capacity(machine.structural_places.len());
    for place in &machine.structural_places {
        if index(place.id.get())? != roots.len() {
            return Err(Error::Malformed("places are not in dense identity order"));
        }
        let (ty, owned, parameter) = match place.kind {
            StructuralPlaceKind::Parameter { position } => {
                let parameter = machine
                    .structural_parameters
                    .get(position as usize)
                    .filter(|parameter| parameter.place == place.id)
                    .ok_or(Error::Malformed("parameter place has no signature"))?;
                (
                    parameter.structural_type,
                    parameter.access == StructuralAccess::Owned,
                    true,
                )
            }
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            }
            | StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } => (structural_type, true, false),
            StructuralPlaceKind::Result => {
                let TerminalMachineResult::Structural(result) = &machine.result else {
                    return Err(Error::Malformed("result place has no structural signature"));
                };
                (result.structural_type, false, false)
            }
            StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::ByteSequenceLiteral { .. }
            | StructuralPlaceKind::ProviderAttachment { .. } => {
                return Err(Error::Unsupported("affine block, byte or provider storage"));
            }
        };
        let declaration = types
            .get(index(ty.get())?)
            .filter(|declaration| declaration.id == ty)
            .ok_or(Error::Malformed(
                "structural type is outside its identity table",
            ))?;
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
        roots.push(tracked.then_some(Root {
            structural_type: ty,
            owned,
            parameter,
        }));
    }
    if roots.iter().all(Option::is_none) {
        return Ok(Plan::default());
    }
    let live = live_entries(machine)?;
    let initial: State = roots
        .iter()
        .map(|root| {
            root.as_ref()
                .filter(|root| root.parameter)
                .map(|_| Region::new(true))
        })
        .collect();
    let mut entering: Vec<Option<State>> = vec![None; machine.blocks.len()];
    let first = index(machine.entry.get())?;
    *entering
        .get_mut(first)
        .ok_or(Error::Malformed("entry block is outside the machine"))? = Some(initial);
    let mut pending = vec![machine.entry];
    let mut analyzed = vec![false; machine.blocks.len()];
    let unavailable_roots = vec![None; roots.len()];
    let mut analysis = Analysis {
        types,
        roots,
        unavailable: Vec::new(),
        unavailable_roots,
    };
    let mut cleanup = Vec::new();
    while let Some(block_id) = pending.pop() {
        let slot = index(block_id.get())?;
        if std::mem::replace(&mut analyzed[slot], true) {
            continue;
        }
        let block = machine
            .blocks
            .get(slot)
            .filter(|block| block.id == block_id)
            .ok_or(Error::Malformed("block is not at its identity"))?;
        let mut state = entering[slot]
            .clone()
            .ok_or(Error::Malformed("block has no incoming frontier"))?;
        for operation in &block.operations {
            analysis.operation(&mut state, operation)?;
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
                analysis.argument(
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
                analysis.read(&state, *source, &[], site)?;
            }
            Terminator::Jump {
                structural_arguments,
                ..
            }
            | Terminator::InvokeNoNormal {
                structural_arguments,
                ..
            }
            | Terminator::ExternalCompletion {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    analysis.argument(&mut state, argument, site, false)?;
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    for argument in &edge.structural_arguments {
                        analysis.argument(&mut state, argument, Site::Edge(edge.edge), false)?;
                    }
                }
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => {}
        }
        let no_normal = matches!(
            block.terminator,
            Terminator::Crash { .. }
                | Terminator::InvokeNoNormal { .. }
                | Terminator::ExternalCompletion { .. }
        );
        if no_normal {
            continue;
        }
        if let Terminator::ReturnStructural { source, .. } = &block.terminator {
            if analysis.root(*source)?.is_some() {
                state[index(source.get())?] = None;
            }
        }
        let targets = block.terminator.successor_edges();
        let mut target_by_edge = vec![
            None;
            block
                .terminator
                .edges()
                .map(|edge| index(edge.get()).map(|index| index + 1))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .unwrap_or(0)
        ];
        for (edge, target) in targets {
            target_by_edge[index(edge.get())?] = Some(target);
        }
        for (edge, existing) in block.terminator.cleanup_rosters() {
            let mut outgoing = state.clone();
            for action in existing {
                analysis.cleanup(&mut outgoing, action)?;
            }
            let target = target_by_edge[index(edge.get())?];
            let needed = target
                .map(|target| index(target.get()).map(|index| &live[index]))
                .transpose()?;
            let mut actions = Vec::new();
            for raw in (0..outgoing.len()).rev() {
                let Some(info) = &analysis.roots[raw] else {
                    continue;
                };
                if !info.owned || needed.is_some_and(|needed| needed[raw]) {
                    continue;
                }
                if let Some(region) = &outgoing[raw] {
                    let place = PlaceId::new(raw as u64 + 1)
                        .ok_or(Error::Malformed("place identity overflow"))?;
                    actions.extend(analysis.dispose(place, region)?);
                    outgoing[raw] = None;
                }
            }
            if !actions.is_empty() {
                cleanup.push((edge, actions));
            }
            if let Some(target) = target {
                let destination = index(target.get())?;
                match &entering[destination] {
                    Some(earlier) if earlier != &outgoing => {
                        return Err(Error::Unsupported(
                            "different affine frontiers at a reconvergence",
                        ));
                    }
                    Some(_) => {}
                    None => {
                        entering[destination] = Some(outgoing);
                        pending.push(target);
                    }
                }
            }
        }
    }
    cleanup.sort_by_key(|(edge, _)| *edge);
    Ok(Plan {
        cleanup,
        unavailable: analysis.unavailable,
    })
}

impl Analysis<'_> {
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
    ) -> Result<(), Error> {
        use terminal_psi::{OperationKind as Op, RecordFieldValue};
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
                    if state[index(destination.get())?].is_none() {
                        self.read(state, *destination, &path, site)?;
                    } else if !state[index(destination.get())?]
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
            Op::EstablishRecord { fields } | Op::EstablishStructuralCase { fields, .. } => {
                for field in fields {
                    if let RecordFieldValue::Structural(argument) = &field.value {
                        self.argument(state, argument, site, true)?;
                    }
                }
            }
            Op::StoreStructuralField {
                destination,
                path,
                field,
                value,
                ..
            } => {
                self.argument(state, value, site, true)?;
                if self.root(*destination)?.is_some() {
                    let mut path = self.canonical(*destination, path)?;
                    if let Some(field) = field {
                        path.push(Segment::Field(*field));
                    }
                    if state[index(destination.get())?].is_none() {
                        self.read(state, *destination, &path, site)?;
                    } else if let Some(region) = &mut state[index(destination.get())?] {
                        if !region.base {
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
            state[index(result.place.get())?] = Some(Region::new(true));
        }
        Ok(())
    }

    fn cleanup(
        &self,
        state: &mut State,
        action: &TerminalAffineCleanupAction,
    ) -> Result<(), Error> {
        let place = action.place();
        let Some(info) = self.root(place)? else {
            return Ok(());
        };
        if !info.owned {
            return Err(Error::Malformed("cleanup disposes a borrowed parameter"));
        }
        let slot = &mut state[index(place.get())?];
        // Invalid source may transfer the same home twice. Retain its earlier
        // unavailable-use verdict instead of hiding it behind a producer error;
        // a repeated cleanup without such a source error remains malformed.
        let Some(region) = slot.as_mut() else {
            return if self.unavailable_roots[index(place.get())?].is_some() {
                Ok(())
            } else {
                Err(Error::Malformed("cleanup repeats an ended home"))
            };
        };
        match action {
            TerminalAffineCleanupAction::DiscardRoot(_) => {
                if !region.whole {
                    return Err(Error::Malformed(
                        "whole discard follows a partial or complete move",
                    ));
                }
                *slot = None;
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                let path = self.canonical(place, &discard.path)?;
                if !region.available(&path) {
                    return Err(Error::Malformed("residual discard overlaps a moved region"));
                }
                region.set(&path, false);
            }
            TerminalAffineCleanupAction::RetireHome(_) => {
                if region.whole {
                    return Err(Error::Malformed("home retirement omits its affine value"));
                }
                if self.dispose(place, region)?.len() != 1 {
                    return Err(Error::Malformed(
                        "home retirement omits a residual affine region",
                    ));
                }
                *slot = None;
            }
            TerminalAffineCleanupAction::InvokeNominal(_) => {
                return Err(Error::Unsupported("nominal affine cleanup"));
            }
        }
        Ok(())
    }

    fn dispose(
        &self,
        place: PlaceId,
        region: &Region,
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
        let residuals = crate::owned_structure::partial_affine_residuals(
            self.types,
            info.structural_type,
            &moved,
            usize::MAX,
        )
        .ok_or(Error::Unsupported("affine residual partition"))?;
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
            if let Some(region) = &mut state[index(argument.place.get())?] {
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
}
