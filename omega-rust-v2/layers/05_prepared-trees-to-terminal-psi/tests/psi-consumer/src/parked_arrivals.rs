//! Replay every origin a parked descriptor can hold, reading published Psi alone.
//!
//! `h.r = match c {true -> second, false -> first,}` stores a block parameter
//! whose two arriving edges copy different formals. When a later call parks
//! with `h` live, the descriptor in `h.r` holds `second`'s loan on one arrival
//! and `first`'s on the other (`wiki/spec/language/lifetimes.md#carried-loan-transport`:
//! an unresolved selection retains all possible sources). This reader walks
//! back from the parked call through the home's stores, the blocks' incoming
//! edges and those edges' arguments, without the producer's rows, and then
//! compares what it found with each published embedded loan. It follows only
//! what it can read exactly and refuses the rest, including any cycle.
//!
//! A writable loan of the home at the descriptor or an enclosing field, a
//! named `BorrowPlace` or a call's borrow operand, is a possible writer: the
//! reader lists it and keeps walking, because its holders may write nothing.
//! It does not ask whether the loan ended or what the callee does; the
//! checker judges both. The newest store into the home ends the walk.
use omega_psi::{
    BlockTerminator, CallSite, EdgeId, EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, MachineBody, OperationId, OperationKind, OperationResult,
    Parameter, PlaceId, PlaceOrigin, PlaceProjection, ReferenceAccess, SuccessorEdge, ValueOperand, WritableLoanSite,
};

use super::machine;

/// What a replayed origin is: the incoming formal at `position` and the path
/// inside it, or a writable loan and the path below its captured place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrivalSource {
    Formal { position: usize, path: Vec<PlaceProjection> },
    Written { loan: WritableLoanSite, path: Vec<PlaceProjection> },
}

/// One replayed origin: the join edges taken, nearest the parked call first,
/// its source and its shared reborrows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arrival {
    pub edges: Vec<EdgeId>,
    pub source: ArrivalSource,
    pub reborrows: Vec<OperationId>,
}

/// One embedded descriptor of a retained owner at a parked call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParkedDescriptor {
    pub call: CallSite,
    pub owner: PlaceId,
    pub path: Vec<PlaceProjection>,
    pub arrivals: Vec<Arrival>,
}

/// For every parked call of `caller`, each retained owner's embedded
/// descriptors with the origins replayed from the body, after checking that
/// the published rows name exactly those origins.
pub fn parked_descriptor_arrivals(
    package: &LoweredPackage,
    caller: &str,
) -> Result<Vec<ParkedDescriptor>, &'static str> {
    let body = &machine(package, caller).ok_or("missing caller")?.body;
    let mut rows = Vec::new();
    for site in &body.suspension_sites {
        let (block, end, arguments) = position(body, site.call)?;
        for owner in &site.retained_owners {
            for loan in &owner.embedded_loans {
                // The parked callee may write through its own operand while parked.
                let mut found = writers(site.call, arguments, owner.place, &loan.path)?;
                if !found.is_empty() && !body.mutable_homes.contains(&owner.place) {
                    return Err("the parked call lends an owner that is not a writable home");
                }
                let mut replay = Replay { body, walking: Vec::new() };
                found.extend(replay.read(owner.place, loan.path.clone(), Some((block, end)))?);
                compare(body, &loan.origins, &found)?;
                rows.push(ParkedDescriptor {
                    call: site.call,
                    owner: owner.place,
                    path: loan.path.clone(),
                    arrivals: found,
                });
            }
        }
    }
    Ok(rows)
}

/// The block holding the parked call, the ordinal it occupies (or the
/// block's end for a terminal call) and the call's operands.
fn position(
    body: &MachineBody,
    call: CallSite,
) -> Result<(usize, usize, &[ValueOperand]), &'static str> {
    for (index, block) in body.blocks.iter().enumerate() {
        match (call, &block.terminator) {
            (CallSite::Edge(id), BlockTerminator::TerminalCall { id: edge, arguments, .. }) if *edge == id => {
                return Ok((index, block.operations.end, arguments));
            }
            (CallSite::Operation(id), _) if block.operations.contains(&id.index()) => {
                let OperationKind::Call { arguments, .. } = &body.operations[id.index()].kind else {
                    return Err("the parked operation is not a call");
                };
                return Ok((index, id.index(), arguments));
            }
            _ => {}
        }
    }
    Err("the parked call is not in any block")
}

fn lends(
    operand: &ValueOperand,
    root: PlaceId,
    path: &[PlaceProjection],
) -> bool {
    matches!(operand, ValueOperand::Borrow { place, access } if *access != ReferenceAccess::Shared
        && place.root == root && (place.path.starts_with(path) || path.starts_with(&place.path)))
}

fn written(
    loan: WritableLoanSite,
    below: &[PlaceProjection],
) -> Arrival {
    Arrival {
        edges: Vec::new(),
        source: ArrivalSource::Written { loan, path: below.to_vec() },
        reborrows: Vec::new(),
    }
}

/// The writable loans that a call's operands form of `root` at the
/// descriptor or an enclosing field, in argument order. A move of `root` or a
/// writable loan below the descriptor is refused.
fn writers(
    call: CallSite,
    arguments: &[ValueOperand],
    root: PlaceId,
    path: &[PlaceProjection],
) -> Result<Vec<Arrival>, &'static str> {
    let mut found = Vec::new();
    for (argument, operand) in arguments.iter().enumerate() {
        match operand {
            ValueOperand::MoveWhole(place) if *place == root => return Err("the descriptor was moved out and not restored"),
            ValueOperand::Borrow { place, access } if *access != ReferenceAccess::Shared && place.root == root && path.starts_with(&place.path) => {
                found.push(written(WritableLoanSite::Call { call, argument }, &path[place.path.len()..]));
            }
            _ if lends(operand, root, path) => return Err("this reader does not follow a writable loan below the descriptor"),
            _ => {}
        }
    }
    Ok(found)
}

fn edges(terminator: &BlockTerminator) -> Vec<&SuccessorEdge> {
    match terminator {
        BlockTerminator::Jump(edge) => vec![edge],
        BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true, when_false],
        _ => Vec::new(),
    }
}

struct Replay<'a> {
    body: &'a MachineBody,
    /// Block starts entered and not yet left; meeting one again is a cycle.
    walking: Vec<(usize, PlaceId)>,
}

impl Replay<'_> {
    /// The origins of the descriptor at `path` inside `place`. A writable
    /// home is read at `at`, the block and ordinal of the read.
    fn read(
        &mut self,
        place: PlaceId,
        mut path: Vec<PlaceProjection>,
        at: Option<(usize, usize)>,
    ) -> Result<Vec<Arrival>, &'static str> {
        let body = self.body;
        if body.mutable_homes.contains(&place) {
            let (block, end) = at.ok_or("this reader needs the position of a read from a home")?;
            return self.home(place, path, block, end);
        }
        match body.places[place.index()].origin {
            PlaceOrigin::Parameter => {
                let formal = body
                    .parameters
                    .iter()
                    .position(|p| *p == Parameter::Structural(place))
                    .ok_or("a parameter place is not a formal")?;
                Ok(vec![Arrival {
                    edges: Vec::new(),
                    source: ArrivalSource::Formal { position: formal, path },
                    reborrows: Vec::new(),
                }])
            }
            PlaceOrigin::Operation(id) => {
                let op = &body.operations[id.index()];
                let here = Some(self.block_of(id.index())?);
                match &op.kind {
                    OperationKind::EstablishRecord { fields } => {
                        let Some(PlaceProjection::Field(name)) = path.first() else {
                            return Err("a record's descriptor sits below one of its fields");
                        };
                        let field = fields
                            .iter()
                            .find(|f| &f.field == name)
                            .ok_or("missing field initializer")?;
                        let source = match field.operand {
                            ValueOperand::CopyWhole(source) => source,
                            ValueOperand::MoveWhole(source) if path.len() > 1 => source,
                            _ => return Err("this reader follows copied descriptors and moved aggregates only"),
                        };
                        path.remove(0);
                        self.read(source, path, here)
                    }
                    OperationKind::MoveStructural { source } | OperationKind::CopyStructural { source } => {
                        if !source
                            .path
                            .iter()
                            .all(|step| matches!(step, PlaceProjection::Field(_)))
                        {
                            return Err("this reader follows field paths only");
                        }
                        let mut full = source.path.clone();
                        full.extend(path);
                        self.read(source.root, full, here)
                    }
                    OperationKind::BorrowPlace {
                        source,
                        access: ReferenceAccess::Shared,
                    } if path.is_empty() => {
                        let [parent @ .., PlaceProjection::Referent] = source.path.as_slice() else {
                            return Err("this reader follows a reborrow of one stored reference");
                        };
                        let mut found = self.read(source.root, parent.to_vec(), here)?;
                        for arrival in &mut found {
                            arrival.reborrows.push(op.id);
                        }
                        Ok(found)
                    }
                    _ => Err("this reader cannot follow the operation that formed the descriptor"),
                }
            }
            PlaceOrigin::BlockParameter(block) => {
                let target = block.get() - 1;
                let slot = body.blocks[target]
                    .structural_parameters
                    .iter()
                    .position(|p| *p == place)
                    .ok_or("a block parameter is not in its block")?;
                self.enter(target, place, |this, edge, source| {
                    let argument = match edge.structural_arguments.get(slot) {
                        Some(ValueOperand::CopyWhole(argument)) => *argument,
                        Some(ValueOperand::MoveWhole(argument)) if !path.is_empty() => *argument,
                        _ => return Err("an edge does not copy the joined descriptor"),
                    };
                    this.read(argument, path.clone(), Some((source, body.blocks[source].operations.end)))
                })
            }
            _ => Err("this reader cannot follow the descriptor's place"),
        }
    }

    fn block_of(
        &self,
        ordinal: usize,
    ) -> Result<(usize, usize), &'static str> {
        let block = self
            .body
            .blocks
            .iter()
            .position(|block| block.operations.contains(&ordinal))
            .ok_or("an operation is in no block")?;
        Ok((block, ordinal))
    }

    /// Read the home's descriptor backward from `end` in `block`.
    fn home(
        &mut self,
        home: PlaceId,
        path: Vec<PlaceProjection>,
        block: usize,
        end: usize,
    ) -> Result<Vec<Arrival>, &'static str> {
        let body = self.body;
        let mut found = Vec::new();
        for ordinal in (body.blocks[block].operations.start..end).rev() {
            let op = &body.operations[ordinal];
            if op.result == OperationResult::Structural(home) {
                let OperationKind::EstablishRecord { fields } = &op.kind else {
                    return Err("this reader follows a home established by a record");
                };
                let Some(PlaceProjection::Field(name)) = path.first() else {
                    return Err("a home's descriptor sits below one of its fields");
                };
                let field = fields
                    .iter()
                    .find(|f| &f.field == name)
                    .ok_or("missing field initializer")?;
                let source = match field.operand {
                    ValueOperand::CopyWhole(source) => source,
                    ValueOperand::MoveWhole(source) if path.len() > 1 => source,
                    _ => return Err("this reader follows copied descriptors and moved aggregates only"),
                };
                found.extend(self.read(source, path[1..].to_vec(), Some((block, ordinal)))?);
                return Ok(found);
            }
            match &op.kind {
                OperationKind::StructuralStore { destination, value, .. } if destination.root == home => {
                    if path.starts_with(&destination.path) && !destination.path.contains(&PlaceProjection::Referent) {
                        found.extend(self.read(*value, path[destination.path.len()..].to_vec(), Some((block, ordinal)))?);
                        return Ok(found);
                    }
                    if destination.path.starts_with(&path) || destination.path.contains(&PlaceProjection::Referent) {
                        return Err("a store changes the descriptor without replacing it");
                    }
                }
                OperationKind::BorrowPlace { source, access } if *access != ReferenceAccess::Shared && source.root == home => {
                    if path.starts_with(&source.path) {
                        found.push(written(WritableLoanSite::Borrow(op.id), &path[source.path.len()..]));
                    } else if source.path.starts_with(&path) {
                        return Err("this reader does not follow a writable loan below the descriptor");
                    }
                }
                OperationKind::Call { arguments, .. } => {
                    found.extend(writers(CallSite::Operation(op.id), arguments, home, &path)?);
                }
                OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. }
                    if fields.iter().any(|field| lends(&field.operand, home, &path)) =>
                {
                    return Err("a constructed value retains a writable loan of the home");
                }
                OperationKind::MoveStructural { source } | OperationKind::DecomposeRecord { source, .. }
                    if source.root == home && (source.path.starts_with(&path) || path.starts_with(&source.path)) =>
                {
                    return Err("the descriptor was moved out and not restored");
                }
                _ => {}
            }
        }
        found.extend(self.enter(block, home, |this, edge, source| {
            if edge
                .structural_arguments
                .iter()
                .any(|arg| matches!(arg, ValueOperand::MoveWhole(p) if *p == home) || lends(arg, home, &path))
            {
                return Err("an edge moves or lends the home");
            }
            this.home(home, path.clone(), source, body.blocks[source].operations.end)
        })?);
        Ok(found)
    }

    /// Enter `block` from every predecessor. Agreeing predecessors pass their
    /// arrivals through; otherwise each edge prefixes its own.
    fn enter(
        &mut self,
        block: usize,
        subject: PlaceId,
        mut from: impl FnMut(&mut Self, &SuccessorEdge, usize) -> Result<Vec<Arrival>, &'static str>,
    ) -> Result<Vec<Arrival>, &'static str> {
        if self.walking.contains(&(block, subject)) {
            return Err("this reader refuses a cycle");
        }
        let body = self.body;
        let mut incoming: Vec<(&SuccessorEdge, usize)> = body
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(source, b)| edges(&b.terminator).into_iter().map(move |edge| (edge, source)))
            .filter(|(edge, _)| edge.target.get() - 1 == block)
            .collect();
        if incoming.is_empty() {
            return Err("the subject is not established on every arrival");
        }
        incoming.sort_by_key(|(edge, _)| edge.id.get());
        self.walking.push((block, subject));
        let mut each = Vec::new();
        for (edge, source) in incoming {
            let found = from(self, edge, source);
            match found {
                Ok(found) => each.push((edge.id, found)),
                Err(error) => {
                    self.walking.pop();
                    return Err(error);
                }
            }
        }
        self.walking.pop();
        if each.iter().all(|(_, found)| *found == each[0].1) {
            return Ok(each.swap_remove(0).1);
        }
        Ok(each
            .into_iter()
            .flat_map(|(edge, found)| {
                found.into_iter().map(move |mut arrival| {
                    arrival.edges.insert(0, edge);
                    arrival
                })
            })
            .collect())
    }
}

/// Consecutive origins that share an arrival.
fn groups<T>(
    items: &[T],
    edges: impl Fn(&T) -> &[EdgeId],
) -> Vec<(&[EdgeId], &[T])> {
    let mut groups: Vec<(&[EdgeId], &[T])> = Vec::new();
    let mut start = 0;
    for index in 1..=items.len() {
        if index == items.len() || edges(&items[index]) != edges(&items[start]) {
            groups.push((edges(&items[start]), &items[start..index]));
            start = index;
        }
    }
    groups
}

/// Compare one published embedded loan with the replayed arrivals: the same
/// arrivals in edge order, and within each the same writers in walk order
/// before the same formal origin.
fn compare(
    body: &MachineBody,
    published: &[EmbeddedLoanOrigin],
    replayed: &[Arrival],
) -> Result<(), &'static str> {
    let position = |formal: PlaceId| {
        body.parameters
            .iter()
            .position(|p| *p == Parameter::Structural(formal))
    };
    let same = |origin: &EmbeddedLoanOrigin, arrival: &Arrival| {
        origin.reborrows == arrival.reborrows
            && match (&origin.source, &arrival.source) {
                (EmbeddedLoanSource::Formal { formal, path }, ArrivalSource::Formal { position: at, path: inner }) => {
                    position(*formal) == Some(*at) && path == inner
                }
                (EmbeddedLoanSource::Written { loan, path }, ArrivalSource::Written { loan: replayed, path: inner }) => loan == replayed && path == inner,
                _ => false,
            }
    };
    let is_writer = |origin: &EmbeddedLoanOrigin| matches!(origin.source, EmbeddedLoanSource::Written { .. });
    let replayed_groups = groups(replayed, |arrival| &arrival.edges);
    for (edges, _) in &replayed_groups {
        if !published.iter().any(|origin| origin.arrival == *edges) {
            return Err("a possible arrival is missing");
        }
    }
    let published_groups = groups(published, |origin| &origin.arrival);
    for (edges, origins) in &published_groups {
        let (_, found) = replayed_groups
            .iter()
            .find(|(replayed, _)| replayed == edges)
            .ok_or("a published arrival takes edges that no arrival takes")?;
        let (wrote, formal): (Vec<_>, Vec<_>) = origins.iter().partition(|origin| is_writer(origin));
        let (replayed_wrote, replayed_formal): (Vec<_>, Vec<_>) = found
            .iter()
            .partition(|arrival| matches!(arrival.source, ArrivalSource::Written { .. }));
        if wrote.len() < replayed_wrote.len() {
            return Err("a possible writer is missing");
        }
        if wrote.len() > replayed_wrote.len() {
            return Err("a published writer forms no writable loan of this storage");
        }
        if !wrote
            .iter()
            .all(|origin| replayed_wrote.iter().any(|arrival| same(origin, arrival)))
        {
            return Err("a writer names a different loan");
        }
        if !wrote
            .iter()
            .zip(&replayed_wrote)
            .all(|(origin, arrival)| same(origin, arrival))
            || origins
                .iter()
                .take(wrote.len())
                .any(|origin| !is_writer(origin))
        {
            return Err("an arrival lists its writers out of walk order");
        }
        if formal.len() != replayed_formal.len()
            || !formal
                .iter()
                .zip(&replayed_formal)
                .all(|(origin, arrival)| same(origin, arrival))
        {
            if formal.iter().any(|origin| {
                replayed
                    .iter()
                    .any(|other| other.edges != origin.arrival && same(origin, other))
            }) {
                return Err("an origin is associated with another predecessor");
            }
            return Err("an arrival names a different origin");
        }
    }
    if published.len() != replayed.len() {
        return Err("an arrival is repeated");
    }
    let order = |edges: &[EdgeId]| edges.iter().map(|edge| edge.get()).collect::<Vec<_>>();
    if published_groups
        .windows(2)
        .any(|pair| order(pair[0].0) >= order(pair[1].0))
    {
        return Err("arrivals are out of edge order");
    }
    Ok(())
}
