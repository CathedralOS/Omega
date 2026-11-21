//! Read primitive local homes, their stores and their suspension rows from published Psi alone.
use omega_psi::{
    BlockId, BlockTerminator, CallArgument, CallSite, CompletionContinuation, MachineBody, OperationKind, OperationResult, PlaceId, PlaceOrigin,
    PlaceProjection, ProjectedPlace, ReferenceAccess, SuspensionSite, ValueId, ValueOrigin,
};

/// One operation on a home, in body order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HomeEvent {
    Established(ValueId),
    Borrowed(ReferenceAccess),
    Stored(ValueId),
    Read(ValueId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveHome {
    pub place: PlaceId,
    /// The entry formal whose value the home received, if any.
    pub formal: Option<usize>,
    pub mutable: bool,
    pub events: Vec<HomeEvent>,
}

fn home_of(
    body: &MachineBody,
    place: PlaceId,
) -> Option<ValueId> {
    let PlaceOrigin::Operation(id) = body.places.get(place.index())?.origin else {
        return None;
    };
    match body.operations.get(id.index())?.kind {
        OperationKind::EstablishPrimitiveLocal { value } => Some(value),
        _ => None,
    }
}

fn whole(place: &ProjectedPlace) -> Option<PlaceId> {
    place.path.is_empty().then_some(place.root)
}

/// Every home with the operations that touch it. A whole-primitive store must
/// target a home with write authority and the home's exact type.
pub fn primitive_homes(body: &MachineBody) -> Result<Vec<PrimitiveHome>, &'static str> {
    let mut homes: Vec<PrimitiveHome> = Vec::new();
    for op in &body.operations {
        let (place, event) = match &op.kind {
            OperationKind::EstablishPrimitiveLocal { value } => {
                let OperationResult::Structural(place) = op.result else {
                    return Err("a home is not a place");
                };
                let ty = body.values.get(value.index()).ok_or("missing initializer")?.ty;
                if body.places.get(place.index()).map(|p| (p.ty, p.origin)) != Some((ty, PlaceOrigin::Operation(op.id))) {
                    return Err("home disagrees with its initializer or establishment");
                }
                let formal = match body.values[value.index()].origin {
                    ValueOrigin::Parameter => body
                        .parameters
                        .iter()
                        .position(|p| *p == omega_psi::Parameter::Scalar(*value)),
                    _ => None,
                };
                homes.push(PrimitiveHome {
                    place,
                    formal,
                    mutable: body.mutable_homes.contains(&place),
                    events: Vec::new(),
                });
                (place, HomeEvent::Established(*value))
            }
            OperationKind::BorrowPlace { source, access } => match whole(source).filter(|p| home_of(body, *p).is_some()) {
                Some(place) => (place, HomeEvent::Borrowed(*access)),
                None => continue,
            },
            OperationKind::ProjectedScalarStore { destination, value } if destination.path.is_empty() => {
                let place = destination.root;
                home_of(body, place).ok_or("whole-primitive store outside a primitive home")?;
                if !body.mutable_homes.contains(&place) {
                    return Err("store into a home without write authority");
                }
                if body.values.get(value.index()).map(|v| v.ty) != Some(body.places[place.index()].ty) {
                    return Err("stored value disagrees with its home");
                }
                (place, HomeEvent::Stored(*value))
            }
            OperationKind::PrimitiveScalarRead { source, path } if path.is_empty() && home_of(body, *source).is_some() => {
                let OperationResult::Scalar(read) = op.result else {
                    return Err("a home read is not a scalar");
                };
                (*source, HomeEvent::Read(read))
            }
            _ => continue,
        };
        homes
            .iter_mut()
            .find(|home| home.place == place)
            .ok_or("home used before its establishment")?
            .events
            .push(event);
    }
    Ok(homes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParkedHome {
    pub place: PlaceId,
    pub formal: Option<usize>,
    pub needed_by_callee: bool,
    pub needed_after_completion: bool,
    /// Retained loan descriptors whose borrows lead back to this home.
    pub loans: Vec<PlaceId>,
}

/// The home a loan descriptor's defining borrows lead to.
fn loan_home(
    body: &MachineBody,
    mut descriptor: PlaceId,
) -> Option<PlaceId> {
    loop {
        let PlaceOrigin::Operation(id) = body.places.get(descriptor.index())?.origin else {
            return None;
        };
        let OperationKind::BorrowPlace { source, .. } = &body.operations.get(id.index())?.kind else {
            return None;
        };
        match source.path.as_slice() {
            [] => return home_of(body, source.root).map(|_| source.root),
            [PlaceProjection::Referent] => descriptor = source.root,
            _ => return None,
        }
    }
}

fn uses_home(
    kind: &OperationKind,
    home: PlaceId,
) -> bool {
    match kind {
        OperationKind::PrimitiveScalarRead { source, .. } => *source == home,
        OperationKind::ProjectedScalarStore { destination, .. } | OperationKind::BorrowPlace { source: destination, .. } => destination.root == home,
        OperationKind::Call { arguments, .. } => borrows(arguments, home),
        _ => false,
    }
}

fn borrows(
    arguments: &[CallArgument],
    home: PlaceId,
) -> bool {
    arguments
        .iter()
        .any(|arg| matches!(arg, CallArgument::Borrow { place, .. } if place.root == home))
}

/// Reconstruct which homes the parked call and its continuation need, from the
/// call operands, the operations after completion and the retained loans, then
/// require the site's home rows to say exactly that.
pub fn parked_homes(
    body: &MachineBody,
    site: &SuspensionSite,
) -> Result<Vec<ParkedHome>, &'static str> {
    let (arguments, block, resume) = match site.call {
        CallSite::Cleanup(_) => return Err("expected an operation or terminal call for the parked home"),
        CallSite::Operation(id) => {
            let Some(OperationKind::Call { arguments, .. }) = body.operations.get(id.index()).map(|op| &op.kind) else {
                return Err("site names no call");
            };
            let block = body
                .blocks
                .iter()
                .find(|b| b.operations.contains(&id.index()))
                .ok_or("call outside a block")?;
            (arguments.as_slice(), block, Some(id.index() + 1))
        }
        CallSite::Edge(edge) => {
            let block = body
                .blocks
                .iter()
                .find(|b| matches!(&b.terminator, BlockTerminator::TerminalCall { id, .. } if *id == edge))
                .ok_or("site names no terminal call")?;
            let BlockTerminator::TerminalCall { arguments, .. } = &block.terminator else {
                unreachable!()
            };
            (arguments.as_slice(), block, None)
        }
    };
    match (site.continuation, resume) {
        (CompletionContinuation::Operation(next), Some(after)) if next.index() == after => {}
        (CompletionContinuation::Terminator(id), Some(after)) if id == block.id && after == block.operations.end => {}
        (CompletionContinuation::Return, None) => {}
        _ => return Err("continuation does not follow the call"),
    }
    // Operations and terminators that can run after normal completion.
    let mut later: Vec<&OperationKind> = Vec::new();
    let mut terminators = Vec::new();
    if let Some(after) = resume {
        later.extend(
            body.operations[after..block.operations.end]
                .iter()
                .map(|op| &op.kind),
        );
        let mut pending = vec![block.id];
        let mut seen: Vec<BlockId> = Vec::new();
        while let Some(id) = pending.pop() {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let current = &body.blocks[id.get() - 1];
            if id != block.id {
                later.extend(
                    body.operations[current.operations.clone()]
                        .iter()
                        .map(|op| &op.kind),
                );
            }
            terminators.push(&current.terminator);
            match &current.terminator {
                BlockTerminator::Jump(edge) => pending.push(edge.target),
                BlockTerminator::Conditional { when_true, when_false, .. } => pending.extend([when_true.target, when_false.target]),
                _ => {}
            }
        }
    }
    let mut rows = site.retained_homes.iter().peekable();
    let mut parked = Vec::new();
    for home in primitive_homes(body)? {
        let loans: Vec<_> = site
            .retained_loans
            .iter()
            .filter(|loan| loan_home(body, loan.descriptor) == Some(home.place))
            .collect();
        let by_callee = borrows(arguments, home.place) || loans.iter().any(|loan| loan.needed_by_callee);
        let after = later.iter().any(|kind| uses_home(kind, home.place))
            || terminators
                .iter()
                .any(|t| matches!(t, BlockTerminator::TerminalCall { arguments, .. } if borrows(arguments, home.place)))
            || loans.iter().any(|loan| loan.needed_after_completion);
        if !by_callee && !after {
            continue;
        }
        let row = rows
            .next_if(|row| row.place == home.place)
            .ok_or("a needed home has no row, or the rows are out of order")?;
        if row.ty != body.places[home.place.index()].ty || !row.carry.allows_suspension {
            return Err("home row disagrees with its home type or carry");
        }
        if (row.needed_by_callee, row.needed_after_completion) != (by_callee, after) {
            return Err("home row needs disagree with the call and its continuation");
        }
        if site
            .retained_owners
            .iter()
            .any(|owner| owner.place == home.place)
            || site
                .retained_loans
                .iter()
                .any(|loan| loan.descriptor == home.place)
        {
            return Err("a home is not an owner or a loan");
        }
        parked.push(ParkedHome {
            place: home.place,
            formal: home.formal,
            needed_by_callee: by_callee,
            needed_after_completion: after,
            loans: loans.iter().map(|loan| loan.descriptor).collect(),
        });
    }
    if rows.next().is_some() {
        return Err("a home row names storage that is not a needed home");
    }
    Ok(parked)
}
