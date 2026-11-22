use super::{CallArgument, CleanupContext, Frontier, LiveRoot, LoweringError, PlaceProjection, TypeId, TypeShape};
use crate::{
    BlockTerminator, CallSite, CleanupAction, EdgeId, EmbeddedLoanOrigin, EmbeddedLoanSource, MachineBody, OperationKind, OperationResult, PlaceId,
    PlaceOrigin, PsiOperation, ReferenceAccess, SuccessorEdge, ValueOperand, WritableLoanSite,
};

const UNTRACED_DESCRIPTOR: &str = "parking this owner requires the backing of its embedded descriptors";

/// Ownership reaches a call in forward execution order; liveness visits it
/// backward. Keep only nonempty snapshots, keyed by the published call.
#[derive(Default)]
pub(crate) struct OwnedCrossings {
    sites: Vec<(crate::CallSite, Vec<crate::RetainedOwner>)>,
    /// Descriptor classification by type, shared by every crossing of the body.
    embeds: Embeds,
}

impl OwnedCrossings {
    pub(super) fn sort_by_call(&mut self) {
        self.sites.sort_unstable_by_key(|(call, _)| Self::key(*call));
    }

    /// A total order over every call coordinate; only calls and terminal
    /// edges park, so cleanup sites never match a recorded crossing.
    fn key(call: crate::CallSite) -> (u8, usize, usize) {
        match call {
            crate::CallSite::Operation(id) => (0, id.index(), 0),
            crate::CallSite::Edge(id) => (1, id.get() - 1, 0),
            crate::CallSite::Cleanup(crate::CleanupSite {
                list: crate::CleanupList::Edge(id),
                action,
            }) => (2, id.get() - 1, action),
            crate::CallSite::Cleanup(crate::CleanupSite {
                list: crate::CleanupList::Store(id),
                action,
            }) => (3, id.index(), action),
        }
    }

    pub(in crate::machines) fn take(
        &mut self,
        call: crate::CallSite,
    ) -> Vec<crate::RetainedOwner> {
        match self
            .sites
            .binary_search_by_key(&Self::key(call), |(site, _)| Self::key(*site))
        {
            Ok(index) => std::mem::take(&mut self.sites[index].1),
            Err(_) => Vec::new(),
        }
    }

    /// Records the owners live at one parked call. Their embedded descriptors
    /// get their origins from `trace` once the body's whole graph is known.
    pub(super) fn record(
        &mut self,
        call: crate::CallSite,
        state: &Frontier,
        arguments: &[CallArgument],
        context: &CleanupContext<'_>,
    ) -> Result<(), LoweringError> {
        let mut owners = Vec::new();
        for root in &state.0 {
            if root.exhausted(context) {
                continue;
            }
            let ty = context.places[root.place.index()].ty;
            let embedded_loans = embedded_loans(root, ty, context, &mut self.embeds)?;
            owners.push(crate::RetainedOwner {
                place: root.place,
                ty,
                moved: root.moved.clone(),
                custody: crate::OwnedCustody::Caller,
                embedded_loans,
            });
        }
        // A direct borrow stays on the call operand. Only a whole-value
        // transfer changes custody of the backing owner.
        for (slot, arg) in arguments.iter().enumerate() {
            let (place, transfer) = match arg {
                CallArgument::MoveWhole(place) => (*place, true),
                CallArgument::CopyWhole(place) => (*place, false),
                CallArgument::Borrow { .. } | CallArgument::Scalar(_) => continue,
            };
            if let Ok(index) = owners.binary_search_by_key(&place.index(), |owner| owner.place.index()) {
                if !transfer {
                    return Err(LoweringError::new(context.site, "parked call cannot copy an owned argument"));
                }
                owners[index].custody = crate::OwnedCustody::CalleeArgument(slot);
            }
        }
        if !owners.is_empty() {
            self.sites.push((call, owners));
        }
        Ok(())
    }

    /// Names every origin of each recorded descriptor from the finished body,
    /// the same blocks, edges and operations the checker reads. A descriptor
    /// whose origins lowering cannot all name refuses its crossing.
    pub(super) fn trace(
        &mut self,
        body: &MachineBody,
        context: &CleanupContext<'_>,
    ) -> Result<(), LoweringError> {
        let sites = &mut self.sites;
        if sites
            .iter()
            .all(|(_, owners)| owners.iter().all(|owner| owner.embedded_loans.is_empty()))
        {
            return Ok(());
        }
        let mut arrivals = Arrivals::new(body, context);
        for (call, owners) in sites.iter_mut() {
            let site = match *call {
                CallSite::Operation(id) => &body.diagnostics.operations[id.index()],
                CallSite::Edge(id) => &body.diagnostics.edges[id.get() - 1],
                CallSite::Cleanup(_) => context.site,
            };
            let refuse = || LoweringError::new(site, UNTRACED_DESCRIPTOR);
            let crossing = arrivals.crossing(*call).ok_or_else(refuse)?;
            for owner in owners.iter_mut() {
                let transferred_to = match owner.custody {
                    crate::OwnedCustody::CalleeArgument(slot) => Some(slot),
                    crate::OwnedCustody::Caller => None,
                };
                arrivals.trace_loans(&crossing, owner.place, &mut owner.embedded_loans, transferred_to).ok_or_else(refuse)?;
            }
        }
        Ok(())
    }
}

/// An owner row describes custody of its storage. Each live descriptor in that
/// storage also needs its backing, traced to the incoming formal it came from,
/// and a transfer out of the owner is a recorded hole. Paths into sum payloads
/// are conditional on the case at each enclosing discriminator. Nonempty
/// fixed arrays holding descriptors and callable descriptors keep the
/// crossing unsupported.
fn embedded_loans(
    root: &LiveRoot,
    ty: TypeId,
    context: &CleanupContext<'_>,
    embeds: &mut Embeds,
) -> Result<Vec<crate::EmbeddedLoan>, LoweringError> {
    let refuse = || LoweringError::new(context.site, UNTRACED_DESCRIPTOR);
    let mut loans = Vec::new();
    let mut pending = vec![(Vec::new(), ty)];
    while let Some((path, ty)) = pending.pop() {
        if root.moved.binary_search(&path).is_ok() || !embeds.contains(ty, context).ok_or_else(refuse)? {
            continue;
        }
        let access = match context.types.get(ty) {
            Some(TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. }) => *access,
            Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. } | TypeShape::SharedQualifiedScalarReference { .. } | TypeShape::IndexedSharedReference(_)) => ReferenceAccess::Shared,
            Some(TypeShape::MutableElementView { element: crate::ViewElement::Nominal(_) }) => ReferenceAccess::Mutable,
            Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                let schema = &context.data[id.get() - 1];
                // A case projection guards every descriptor in that payload.
                // Reverse pushes keep the rows in declaration pre-order.
                for case in schema.cases.iter().rev() {
                    for field in case.fields.iter().rev() {
                        let mut child = path.clone();
                        child.push(PlaceProjection::Case(case.declaration.clone()));
                        child.push(PlaceProjection::Field(field.declaration.clone()));
                        pending.push((child, field.carrier));
                    }
                }
                for field in schema.fields.iter().rev() {
                    let mut child = path.clone();
                    child.push(PlaceProjection::Field(field.declaration.clone()));
                    pending.push((child, field.carrier));
                }
                continue;
            }
            // One row per literal ordinal could be enormous. A callable
            // descriptor needs contract evidence rather than a loan origin.
            _ => return Err(refuse()),
        };
        loans.push(crate::EmbeddedLoan {
            path,
            ty,
            access,
            origins: Vec::new(),
        });
    }
    Ok(loans)
}

/// A position in the body: before operation ordinal `end` of block `block`.
/// The end of a block's range is after its last operation, where its
/// terminator evaluates its edge arguments.
#[derive(Clone, Copy)]
struct Point {
    block: usize,
    end: usize,
}

/// The parked call, its position and its own operands.
struct Crossing<'a> {
    call: CallSite,
    at: Point,
    arguments: &'a [CallArgument],
}

/// A join the trace enters: a writable home's contents at the start of
/// `block`, or a block parameter. `ty` is the descriptor type the origin must
/// have, which a shared reborrow changes to its parent's.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Visit {
    Home {
        home: PlaceId,
        path: Vec<PlaceProjection>,
        ty: TypeId,
        block: usize,
    },
    Parameter {
        place: PlaceId,
        path: Vec<PlaceProjection>,
        ty: TypeId,
        block: usize,
    },
}

impl Visit {
    fn block(&self) -> usize {
        match self {
            Self::Home { block, .. } | Self::Parameter { block, .. } => *block,
        }
    }
}

/// Every origin a trace found. `None` in its place means lowering cannot name them all.
type Found = Option<Vec<EmbeddedLoanOrigin>>;

/// What one operation does to a home's descriptor at a path.
enum Effect {
    None,
    Unknown,
    /// The newest store at a prefix of the path: its value, and the rest of the path inside it.
    Store(PlaceId, Vec<PlaceProjection>),
}

/// Backward trace of descriptor origins over one finished body.
struct Arrivals<'a> {
    body: &'a MachineBody,
    context: &'a CleanupContext<'a>,
    /// The block of each operation ordinal.
    block_of: Vec<usize>,
    /// `(edge, source block)` for each successor edge, grouped by target block
    /// through `starts` and in edge order within each group.
    incoming: Vec<(EdgeId, usize)>,
    starts: Vec<usize>,
    stack: Vec<Visit>,
    /// Joins already traced, with their origins.
    memo: Vec<(Visit, Vec<EmbeddedLoanOrigin>)>,
}

impl<'a> Arrivals<'a> {
    fn new(
        body: &'a MachineBody,
        context: &'a CleanupContext<'a>,
    ) -> Self {
        let mut block_of = vec![usize::MAX; body.operations.len()];
        let mut incoming = Vec::new();
        for (index, block) in body.blocks.iter().enumerate() {
            for ordinal in block.operations.clone() {
                block_of[ordinal] = index;
            }
            for edge in successors(&block.terminator) {
                incoming.push((edge.target.get() - 1, edge.id, index));
            }
        }
        incoming.sort_unstable_by_key(|&(target, edge, _)| (target, edge.get()));
        let mut starts = vec![0; body.blocks.len() + 1];
        for &(target, _, _) in &incoming {
            starts[target + 1] += 1;
        }
        for index in 0..body.blocks.len() {
            starts[index + 1] += starts[index];
        }
        let incoming = incoming
            .into_iter()
            .map(|(_, edge, source)| (edge, source))
            .collect();
        Self {
            body,
            context,
            block_of,
            incoming,
            starts,
            stack: Vec::new(),
            memo: Vec::new(),
        }
    }

    fn crossing(
        &self,
        call: CallSite,
    ) -> Option<Crossing<'a>> {
        match call {
            CallSite::Operation(id) => {
                let OperationKind::Call { arguments, .. } = &self.body.operations.get(id.index())?.kind else {
                    return None;
                };
                Some(Crossing {
                    call,
                    at: self.point(id.index())?,
                    arguments,
                })
            }
            CallSite::Edge(id) => self
                .body
                .blocks
                .iter()
                .enumerate()
                .find_map(|(index, block)| match &block.terminator {
                    BlockTerminator::TerminalCall { id: edge, arguments, .. } if *edge == id => Some(Crossing {
                        call,
                        at: Point {
                            block: index,
                            end: block.operations.end,
                        },
                        arguments,
                    }),
                    _ => None,
                }),
            CallSite::Cleanup(_) => None,
        }
    }

    fn point(
        &self,
        ordinal: usize,
    ) -> Option<Point> {
        let block = *self.block_of.get(ordinal)?;
        (block != usize::MAX).then_some(Point { block, end: ordinal })
    }

    fn end_of(
        &self,
        block: usize,
    ) -> Point {
        Point {
            block,
            end: self.body.blocks[block].operations.end,
        }
    }

    /// A constructed alternative has no descriptor at the path of another
    /// case. Omit that row when every arrival establishes a different case.
    fn trace_loans(
        &mut self,
        crossing: &Crossing<'_>,
        place: PlaceId,
        loans: &mut Vec<crate::EmbeddedLoan>,
        transferred_to: Option<usize>,
    ) -> Option<()> {
        for loan in loans.iter_mut() {
            loan.origins = self.origins(crossing, place, &loan.path, loan.ty, transferred_to)?;
        }
        loans.retain(|loan| !loan.origins.is_empty());
        Some(())
    }

    /// Every origin of the descriptor at `path` inside the owner `root` at the crossing.
    fn origins(
        &mut self,
        crossing: &Crossing<'_>,
        root: PlaceId,
        path: &[PlaceProjection],
        ty: TypeId,
        transferred_to: Option<usize>,
    ) -> Option<Vec<EmbeddedLoanOrigin>> {
        // The parked callee may write through its own writable operand while
        // it is parked; that loan is the nearest possible origin.
        let mut found = Vec::new();
        lent(crossing.call, crossing.arguments, root, path, transferred_to, &mut found)?;
        if !found.is_empty() && !self.body.mutable_homes.contains(&root) {
            return None;
        }
        found.extend(self.value(root, path.to_vec(), ty, crossing.at)?);
        Some(found)
    }

    /// The origins of the descriptor at `path` inside `place`, read at `at`
    /// when `place` is a writable home.
    fn value(
        &mut self,
        place: PlaceId,
        mut path: Vec<PlaceProjection>,
        ty: TypeId,
        at: Point,
    ) -> Found {
        if self.body.mutable_homes.contains(&place) {
            return self.home(place, &path, ty, at);
        }
        match self.body.places.get(place.index())?.origin {
            PlaceOrigin::Parameter => {
                if field_type(self.body.places[place.index()].ty, &path, self.context)? != ty {
                    return None;
                }
                Some(vec![EmbeddedLoanOrigin {
                    arrival: Vec::new(),
                    source: EmbeddedLoanSource::Formal { formal: place, path },
                    reborrows: Vec::new(),
                }])
            }
            PlaceOrigin::Operation(id) => self.operation(id.index(), place, path, ty),
            PlaceOrigin::Component { operation, position } => {
                let OperationKind::DecomposeRecord { source, fields, .. } = &self.body.operations.get(operation.index())?.kind else {
                    return None;
                };
                let component = fields.get(position)?;
                if component.result != OperationResult::Structural(place) {
                    return None;
                }
                path.insert(0, PlaceProjection::Field(component.field.clone()));
                let root = rebase(source, &mut path)?;
                let at = self.point(operation.index())?;
                self.value(root, path, ty, at)
            }
            PlaceOrigin::BlockParameter(block) => self.join(Visit::Parameter {
                place,
                path,
                ty,
                block: block.get() - 1,
            }),
            PlaceOrigin::Result => None,
        }
    }

    /// Follows the operation that established `place`. A record field
    /// initializer, a field-only transfer or copy each name exactly one
    /// source. A shared reborrow continues from its parent descriptor and
    /// records itself. A completed aggregate call supplies a coordinate for
    /// the checker to resolve using the selected return. Borrows of local
    /// storage and other operations have no traced origin here.
    fn operation(
        &mut self,
        ordinal: usize,
        place: PlaceId,
        mut path: Vec<PlaceProjection>,
        ty: TypeId,
    ) -> Found {
        let body = self.body;
        let op = body.operations.get(ordinal)?;
        if op.result != OperationResult::Structural(place) {
            return None;
        }
        let at = self.point(ordinal)?;
        match &op.kind {
            OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
                if let OperationKind::EstablishCase { case, .. } = &op.kind {
                    let Some(PlaceProjection::Case(guard)) = path.first() else { return None };
                    if guard != case {
                        return Some(Vec::new());
                    }
                    path.remove(0);
                }
                let Some(PlaceProjection::Field(name)) = path.first() else {
                    return None;
                };
                let field = fields.iter().find(|field| &field.field == name)?;
                let source = match field.operand {
                    ValueOperand::CopyWhole(source) => source,
                    // A descriptor itself is copied; an aggregate holding it may move.
                    ValueOperand::MoveWhole(source) if path.len() > 1 => source,
                    _ => return None,
                };
                path.remove(0);
                self.value(source, path, ty, at)
            }
            OperationKind::MoveStructural { source } | OperationKind::CopyStructural { source } => {
                let root = rebase(source, &mut path)?;
                self.value(root, path, ty, at)
            }
            OperationKind::Call { callee: crate::CallTarget::Machine(_), .. }
                if !path.is_empty() && field_type(body.places[place.index()].ty, &path, self.context)? == ty =>
            {
                Some(vec![EmbeddedLoanOrigin {
                    arrival: Vec::new(),
                    source: EmbeddedLoanSource::CallResult { operation: op.id, result: place, path },
                    reborrows: Vec::new(),
                }])
            }
            OperationKind::BorrowPlace {
                source,
                access: ReferenceAccess::Shared,
            } if path.is_empty() && body.places[place.index()].ty == ty => {
                let referent = source
                    .path
                    .iter()
                    .position(|step| *step == PlaceProjection::Referent)?;
                let parent = &source.path[..referent];
                if source.path[referent + 1..].contains(&PlaceProjection::Referent) {
                    return None;
                }
                let expected = field_type(body.places.get(source.root.index())?.ty, parent, self.context)?;
                if !matches!(
                    self.context.types.get(expected),
                    Some(
                        TypeShape::ScalarReference {
                            access: ReferenceAccess::Shared,
                            ..
                        } | TypeShape::NominalReference {
                            access: ReferenceAccess::Shared,
                            ..
                        }
                    )
                ) {
                    return None;
                }
                let mut origins = self.value(source.root, parent.to_vec(), expected, at)?;
                for origin in &mut origins {
                    origin.reborrows.push(op.id);
                }
                Some(origins)
            }
            _ => None,
        }
    }

    /// The descriptor at `path` of a writable home just before `at`: each
    /// writable loan formed at a prefix of the path, newest first, then the
    /// newest store at a prefix of the path or the home's establishment,
    /// searched backward through each predecessor.
    fn home(
        &mut self,
        home: PlaceId,
        path: &[PlaceProjection],
        ty: TypeId,
        at: Point,
    ) -> Found {
        let mut found = Vec::new();
        let block = &self.body.blocks[at.block];
        let earlier = 'walk: {
            for ordinal in (block.operations.start..at.end).rev() {
                let op = &self.body.operations[ordinal];
                if op.result == OperationResult::Structural(home) {
                    break 'walk self.operation(ordinal, home, path.to_vec(), ty)?;
                }
                match effect(op, home, path, &mut found) {
                    Effect::None => {}
                    Effect::Unknown => return None,
                    Effect::Store(value, rest) => {
                        break 'walk self.value(value, rest, ty, Point { block: at.block, end: ordinal })?;
                    }
                }
            }
            self.join(Visit::Home {
                home,
                path: path.to_vec(),
                ty,
                block: at.block,
            })?
        };
        found.extend(earlier);
        Some(found)
    }

    /// Enters `visit`'s block through each predecessor edge. Predecessors that
    /// agree pass their origins through; otherwise each edge prefixes its own.
    /// Lowering emits no cycle inside a body. A walk that comes back to a join
    /// it is still tracing would need unboundedly many arrivals, so it refuses.
    fn join(
        &mut self,
        visit: Visit,
    ) -> Found {
        if let Some((_, known)) = self.memo.iter().find(|(seen, _)| *seen == visit) {
            return Some(known.clone());
        }
        if self.stack.contains(&visit) {
            return None;
        }
        let block = visit.block();
        // A home is established inside the body, never before its entry.
        if block == 0 && matches!(visit, Visit::Home { .. }) {
            return None;
        }
        self.stack.push(visit.clone());
        let mut found = Vec::with_capacity(self.starts[block + 1] - self.starts[block]);
        for index in self.starts[block]..self.starts[block + 1] {
            let (edge, source) = self.incoming[index];
            let Some(arrived) = self.arrive(&visit, edge, source) else {
                self.stack.pop();
                return None;
            };
            found.push((edge, arrived));
        }
        self.stack.pop();
        let ((_, first), rest) = found.split_first()?;
        let origins = if rest.iter().all(|(_, arrived)| arrived == first) {
            first.clone()
        } else {
            let mut origins = Vec::new();
            for (edge, arrived) in found {
                for mut origin in arrived {
                    origin.arrival.insert(0, edge);
                    origins.push(origin);
                }
            }
            origins
        };
        self.memo.push((visit, origins.clone()));
        Some(origins)
    }

    fn arrive(
        &mut self,
        visit: &Visit,
        edge: EdgeId,
        source: usize,
    ) -> Found {
        let body = self.body;
        let successor = successors(&body.blocks[source].terminator).find(|successor| successor.id == edge)?;
        let end = self.end_of(source);
        match visit {
            Visit::Home { home, path, ty, .. } => {
                // The edge itself may move, lend or clean up the home before the join.
                if successor
                    .structural_arguments
                    .iter()
                    .any(|arg| touches(arg, *home, path))
                    || successor.cleanup.iter().any(|action| cleans(action, *home))
                {
                    return None;
                }
                self.home(*home, path, *ty, end)
            }
            Visit::Parameter { place, path, ty, block } => {
                let position = body.blocks[*block]
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter == place)?;
                let argument = match successor.structural_arguments.get(position)? {
                    ValueOperand::CopyWhole(argument) => *argument,
                    ValueOperand::MoveWhole(argument) if !path.is_empty() => *argument,
                    _ => return None,
                };
                self.value(argument, path.clone(), *ty, end)
            }
        }
    }
}

fn successors(terminator: &BlockTerminator) -> impl Iterator<Item = &SuccessorEdge> {
    let (first, second) = match terminator {
        BlockTerminator::Jump(edge) => (Some(edge), None),
        BlockTerminator::Conditional { when_true, when_false, .. } => (Some(when_true), Some(when_false)),
        _ => (None, None),
    };
    first.into_iter().chain(second)
}

fn overlaps(
    a: &[PlaceProjection],
    b: &[PlaceProjection],
) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// Whether an operand moves the home or lends a part of it overlapping `path` with write access.
fn touches(
    operand: &ValueOperand,
    home: PlaceId,
    path: &[PlaceProjection],
) -> bool {
    match operand {
        ValueOperand::MoveWhole(place) => *place == home,
        ValueOperand::Borrow { place, access } => *access != ReferenceAccess::Shared && place.root == home && overlaps(&place.path, path),
        ValueOperand::CopyWhole(_) | ValueOperand::Scalar(_) => false,
    }
}

fn cleans(
    action: &CleanupAction,
    home: PlaceId,
) -> bool {
    match action {
        CleanupAction::DiscardRoot(place) | CleanupAction::RetireHome(place) => *place == home,
        CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => place.root == home,
    }
}

/// A writable loan whose captured place is the descriptor or one of its
/// enclosing fields: an origin naming where the loan is formed and the path
/// from its captured place to the descriptor.
fn written(
    loan: WritableLoanSite,
    captured: &[PlaceProjection],
    path: &[PlaceProjection],
) -> EmbeddedLoanOrigin {
    EmbeddedLoanOrigin {
        arrival: Vec::new(),
        source: EmbeddedLoanSource::Written {
            loan,
            path: path[captured.len()..].to_vec(),
        },
        reborrows: Vec::new(),
    }
}

/// Adds the writable loans that `arguments` of `call` form at a prefix of
/// `path` in `home`, in argument order. A move of the home, or a writable loan
/// below the descriptor, is not followed.
fn lent(
    call: CallSite,
    arguments: &[CallArgument],
    home: PlaceId,
    path: &[PlaceProjection],
    transferred_to: Option<usize>,
    found: &mut Vec<EmbeddedLoanOrigin>,
) -> Option<()> {
    for (argument, operand) in arguments.iter().enumerate() {
        // This call transfers the retained owner intact to this callee slot.
        // Its contents are the contents traced immediately before the call.
        if transferred_to == Some(argument) && matches!(operand, ValueOperand::MoveWhole(place) if *place == home) {
            continue;
        }
        if let ValueOperand::Borrow { place, access } = operand
            && *access != ReferenceAccess::Shared
            && place.root == home
            && path.starts_with(&place.path)
        {
            found.push(written(WritableLoanSite::Call { call, argument }, &place.path, path));
        } else if touches(operand, home, path) {
            return None;
        }
    }
    Some(())
}

/// A store through a reference held in the home, and a store below the
/// descriptor, are not followed, and neither are a move out of an
/// overlapping part or a consuming decomposition. A writable loan at a prefix
/// of the path, a named borrow or a call operand, adds its origin to `found`
/// and the walk continues: the loan's holders may write nothing. Lowering
/// never forms a writable loan in a constructor field or on an edge, so the
/// trace does not follow one there.
fn effect(
    op: &PsiOperation,
    home: PlaceId,
    path: &[PlaceProjection],
    found: &mut Vec<EmbeddedLoanOrigin>,
) -> Effect {
    let unknown = |hit: bool| if hit { Effect::Unknown } else { Effect::None };
    match &op.kind {
        OperationKind::StructuralStore { destination, value, .. } if destination.root == home => {
            if destination.path.contains(&PlaceProjection::Referent) {
                Effect::Unknown
            } else if path.starts_with(&destination.path) {
                Effect::Store(*value, path[destination.path.len()..].to_vec())
            } else {
                unknown(destination.path.starts_with(path))
            }
        }
        OperationKind::BorrowPlace { source, access } if *access != ReferenceAccess::Shared && source.root == home => {
            if path.starts_with(&source.path) {
                found.push(written(WritableLoanSite::Borrow(op.id), &source.path, path));
                Effect::None
            } else {
                unknown(source.path.starts_with(path))
            }
        }
        OperationKind::MoveStructural { source } | OperationKind::DecomposeRecord { source, .. } => {
            unknown(source.root == home && overlaps(&source.path, path))
        }
        OperationKind::Call { arguments, .. } => match lent(CallSite::Operation(op.id), arguments, home, path, None, found) {
            Some(()) => Effect::None,
            None => Effect::Unknown,
        },
        OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
            unknown(fields.iter().any(|field| touches(&field.operand, home, path)))
        }
        _ => Effect::None,
    }
}

/// Prefixes a source path inside a record or active case and returns its root.
fn rebase(
    source: &crate::ProjectedPlace,
    path: &mut Vec<PlaceProjection>,
) -> Option<PlaceId> {
    if !source
        .path
        .iter()
        .all(|step| matches!(step, PlaceProjection::Case(_) | PlaceProjection::Field(_)))
    {
        return None;
    }
    path.splice(0..0, source.path.iter().cloned());
    Some(source.root)
}

/// Resolve each field under the declaration or selected case that owns it.
fn field_type(
    mut ty: TypeId,
    path: &[PlaceProjection],
    context: &CleanupContext<'_>,
) -> Option<TypeId> {
    let mut steps = path.iter();
    while let Some(step) = steps.next() {
        let (TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) = context.types.get(ty)? else {
            return None;
        };
        let schema = context.data.get(id.get() - 1)?;
        let (fields, field) = match step {
            PlaceProjection::Case(selected) => {
                let case = schema.cases.iter().find(|case| &case.declaration == selected)?;
                (&case.fields, steps.next()?)
            }
            _ => (&schema.fields, step),
        };
        let PlaceProjection::Field(name) = field else { return None };
        ty = fields.iter().find(|field| &field.declaration == name)?.carrier;
    }
    Some(ty)
}

/// Classification of one type: not yet visited, on the explicit stack, or settled.
#[derive(Clone, Copy, Default)]
enum Embedding {
    #[default]
    Unknown,
    Visiting,
    Known(bool),
}

/// Whether a carrier stores a descriptor by value, classified once per type
/// with an explicit stack and indexed by the dense type id. Meeting a type
/// still being classified exposes a value cycle, which is refused.
#[derive(Default)]
struct Embeds(Vec<Embedding>);

impl Embeds {
    /// An id outside the type table has no classification and is refused.
    fn index(
        ty: TypeId,
        context: &CleanupContext<'_>,
    ) -> Option<usize> {
        context.types.get(ty)?;
        usize::try_from(ty.get() - 1).ok()
    }

    fn state(
        &self,
        index: usize,
    ) -> Embedding {
        self.0.get(index).copied().unwrap_or_default()
    }

    fn set(
        &mut self,
        index: usize,
        state: Embedding,
    ) {
        if index >= self.0.len() {
            self.0.resize(index + 1, Embedding::Unknown);
        }
        self.0[index] = state;
    }

    fn contains(
        &mut self,
        ty: TypeId,
        context: &CleanupContext<'_>,
    ) -> Option<bool> {
        let mut pending = vec![(ty, false)];
        let mut children = Vec::new();
        while let Some((ty, finished)) = pending.pop() {
            let index = Self::index(ty, context)?;
            children.clear();
            if !finished {
                if !matches!(self.state(index), Embedding::Unknown) {
                    continue;
                }
                Self::children(ty, context, &mut children);
                self.set(index, Embedding::Visiting);
                pending.push((ty, true));
                pending.extend(children.iter().map(|&child| (child, false)));
                continue;
            }
            let mut found = Self::children(ty, context, &mut children);
            for &child in &children {
                let Embedding::Known(child) = self.state(Self::index(child, context)?) else {
                    return None;
                };
                found |= child;
            }
            self.set(index, Embedding::Known(found));
        }
        match self.state(Self::index(ty, context)?) {
            Embedding::Known(found) => Some(found),
            Embedding::Unknown | Embedding::Visiting => None,
        }
    }

    /// Collects the carriers stored by value and reports whether the type is
    /// itself a descriptor. An unknown shape counts as one so that it is refused.
    fn children(
        ty: TypeId,
        context: &CleanupContext<'_>,
        children: &mut Vec<TypeId>,
    ) -> bool {
        match context.types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }) => false,
            Some(TypeShape::FixedArray { element, length }) => {
                if *length != 0 {
                    children.push(*element);
                }
                false
            }
            Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                let schema = &context.data[id.get() - 1];
                children.extend(
                    schema
                        .fields
                        .iter()
                        .chain(schema.cases.iter().flat_map(|case| &case.fields))
                        .map(|field| field.carrier),
                );
                false
            }
            _ => true,
        }
    }
}

impl OwnedCrossings {
    /// A copyable home has no custody claim, but its descriptors still need
    /// the same reaching stores and backing as an owned record's descriptors.
    pub(in crate::machines) fn trace_home_loans(
        &mut self,
        body: &MachineBody,
        sites: &mut [crate::SuspensionSite],
        types: &crate::LoweringTypes,
        data: &[crate::LoweredData],
    ) -> Result<(), LoweringError> {
        let context = CleanupContext { places: &body.places, types, data, site: &body.diagnostics.body };
        let mut arrivals = None;
        for site in sites {
            let source = match site.call {
                CallSite::Operation(id) => &body.diagnostics.operations[id.index()],
                CallSite::Edge(id) => &body.diagnostics.edges[id.get() - 1],
                CallSite::Cleanup(_) => context.site,
            };
            let at_call = CleanupContext { site: source, ..context };
            for home in &mut site.retained_homes {
                let root = LiveRoot { place: home.place, moved: Vec::new() };
                let mut loans = embedded_loans(&root, home.ty, &at_call, &mut self.embeds)?;
                if !loans.is_empty() {
                    let tracer = arrivals.get_or_insert_with(|| Arrivals::new(body, &context));
                    let refuse = || LoweringError::new(source, UNTRACED_DESCRIPTOR);
                    let crossing = tracer.crossing(site.call).ok_or_else(refuse)?;
                    tracer.trace_loans(&crossing, home.place, &mut loans, None).ok_or_else(refuse)?;
                }
                home.embedded_loans = loans;
            }
        }
        Ok(())
    }
}
