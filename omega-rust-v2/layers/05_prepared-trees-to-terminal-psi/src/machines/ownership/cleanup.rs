//! Follow the lowered branch edges to derive each normal exit's cleanup from its live places.
use super::super::{branches, expressions::Expressions};
use crate::{
    BlockTerminator, CallArgument, CleanupAction, LoweredData, LoweringError, LoweringTypes, MachineBody, Multiplicity, OperationKind, OperationResult,
    Parameter, PlaceId, PlaceProjection, ProjectedPlace, ReturnEdge, Site, TypeId, TypeShape,
};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
struct LiveRoot {
    place: PlaceId,
    moved: Vec<Vec<PlaceProjection>>,
}

impl LiveRoot {
    /// Sorted paths make every hole below a subtree contiguous.
    fn holes_below(
        &self,
        path: &[PlaceProjection],
    ) -> &[Vec<PlaceProjection>] {
        let first = self.moved.partition_point(|item| item.as_slice() < path);
        let remaining = &self.moved[first..];
        &remaining[..remaining.partition_point(|hole| hole.starts_with(path))]
    }
    /// A case enters a namespace. Only the selected payload contributes
    /// fields to the complement of a partial move.
    fn partial_fields<'a>(
        &self,
        path: &mut Vec<PlaceProjection>,
        schema: &'a LoweredData,
    ) -> Option<&'a [crate::LoweredField]> {
        if schema.cases.is_empty() {
            return Some(&schema.fields);
        }
        let mut enclosed = self.holes_below(path).iter();
        let depth = path.len();
        let selected = enclosed.next()?.get(depth)?;
        let PlaceProjection::Case(case) = selected else {
            return None;
        };
        if enclosed.any(|p| p.get(depth) != Some(selected)) {
            return None;
        }
        let payload = schema.cases.iter().find(|c| &c.declaration == case)?;
        path.push(selected.clone());
        Some(&payload.fields)
    }

    /// Transparent affine records have no extra obligation once every field
    /// has transferred. Preserve any independent nominal or linear claim.
    fn exhausted(
        &self,
        context: &CleanupContext<'_>,
    ) -> bool {
        if self.moved.is_empty() {
            return false;
        }
        let mut pending = vec![(vec![], context.places[self.place.index()].ty)];
        while let Some((mut path, ty)) = pending.pop() {
            if self.moved.binary_search(&path).is_ok() {
                continue;
            }
            let holes = self.holes_below(&path);
            if holes.is_empty() {
                return false;
            }
            // A fixed array settles only when every literal ordinal transferred.
            if let Some(&TypeShape::FixedArray { element, length }) = context.types.get(ty) {
                // Every child needs at least one moved path before the array can be exhausted.
                if u64::try_from(holes.len()).is_ok_and(|paths| paths < length) {
                    return false;
                }
                for ordinal in 0..length {
                    let mut child = path.clone();
                    child.push(PlaceProjection::FixedIndex(ordinal));
                    pending.push((child, element));
                }
                continue;
            }
            let Some(TypeShape::Nominal(id)) = context.types.get(ty) else {
                return false;
            };
            let record = &context.data[id.get() - 1];
            if record.multiplicity != Multiplicity::Affine || record.cleanup_target.is_some() || record.carry.is_some() {
                return false;
            }
            let Some(fields) = self.partial_fields(&mut path, record) else {
                return false;
            };
            for field in fields {
                let mut child = path.clone();
                child.push(PlaceProjection::Field(field.declaration.clone()));
                pending.push((child, field.carrier));
            }
        }
        true
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Frontier(Vec<LiveRoot>);

impl Frontier {
    fn establish(
        &mut self,
        place: PlaceId,
    ) {
        // Reinitializing an earlier local preserves its original cleanup order.
        let slot = self
            .0
            .partition_point(|root| root.place.index() < place.index());
        self.0.insert(slot, LiveRoot { place, moved: vec![] });
    }

    fn take(
        &mut self,
        source: &ProjectedPlace,
        site: &Site,
    ) -> Result<(), LoweringError> {
        let position = self.live(source, site)?;
        let root = &mut self.0[position];
        if super::overlaps_moved_path(&root.moved, &source.path) {
            return Err(LoweringError::new(site, "owned transfer overlaps previously moved storage"));
        }
        if source.path.is_empty() {
            self.0.remove(position);
        } else {
            root.moved.push(source.path.clone());
            root.moved.sort_unstable();
        }
        Ok(())
    }

    /// An authored `move`. A duplicate or prefix-overlapping move is the
    /// lowered-form checker's refusal (terminal-psi/ownership.md#partial-ownership-and-residuals).
    /// The published operations keep both moves in order, so the checker
    /// finds the second one at its own site. Cleanup covers only storage that
    /// no move claimed: the moved paths stay the maximal claimed prefixes.
    /// L5's own cleanup actions and arguments still use the strict `take`.
    fn transfer(
        &mut self,
        source: &ProjectedPlace,
        site: &Site,
    ) -> Result<(), LoweringError> {
        let position = self.live(source, site)?;
        let root = &mut self.0[position];
        if source.path.is_empty() {
            self.0.remove(position);
        } else if !root.moved.iter().any(|hole| source.path.starts_with(hole)) {
            root.moved.retain(|hole| !hole.starts_with(&source.path));
            root.moved.push(source.path.clone());
            root.moved.sort_unstable();
        }
        Ok(())
    }

    fn live(
        &self,
        source: &ProjectedPlace,
        site: &Site,
    ) -> Result<usize, LoweringError> {
        self.0
            .iter()
            .position(|r| r.place == source.root)
            .ok_or_else(|| LoweringError::new(site, "owned transfer has no live source root"))
    }

    fn replace(
        &mut self,
        destination: &ProjectedPlace,
        ty: TypeId,
        context: &CleanupContext<'_>,
    ) -> Result<crate::StructuralStoreDisposition, LoweringError> {
        let Some(root) = self.0.iter_mut().find(|r| r.place == destination.root) else {
            if destination.path.is_empty() {
                self.establish(destination.root);
                return Ok(crate::StructuralStoreDisposition::FillVacancy);
            }
            return Err(LoweringError::new(context.site, "field replacement requires its live enclosing root"));
        };
        if let Some(i) = root.moved.iter().position(|p| p == &destination.path) {
            root.moved.remove(i);
            return Ok(crate::StructuralStoreDisposition::FillVacancy);
        }
        if root.moved.iter().any(|p| destination.path.starts_with(p)) {
            return Err(LoweringError::new(context.site, "field replacement requires its live enclosing storage"));
        }
        let mut actions = Vec::new();
        context.at(root, destination.path.clone(), ty, &mut actions)?;
        root.moved.retain(|p| !p.starts_with(&destination.path));
        Ok(crate::StructuralStoreDisposition::Dispose(actions))
    }

    fn consume_cleanups(
        &mut self,
        actions: &[CleanupAction],
        context: &CleanupContext<'_>,
    ) -> Result<(), LoweringError> {
        let site = context.site;
        for action in actions {
            match action {
                CleanupAction::DiscardRoot(root) => self.take(&ProjectedPlace { root: *root, path: vec![] }, site)?,
                CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => self.take(place, site)?,
            }
        }
        // Field-by-field disposal retires a temporary when its last field dies.
        self.0.retain(|root| {
            let affected = actions
                .iter()
                .any(|a| matches!(a,CleanupAction::DiscardResidual{place,..}|CleanupAction::Invoke{place,..} if place.root==root.place));
            !affected || !root.exhausted(context)
        });
        Ok(())
    }

    /// The frontier holds only owned storage. Transferring an exclusive loan
    /// carrier, such as a `@mut [T]` view arriving at a join, moves no cleanup
    /// custody, exactly as `MoveStructural` of such a place does not.
    fn consume_arguments(
        &mut self,
        arguments: &[CallArgument],
        context: &CleanupContext<'_>,
    ) -> Result<(), LoweringError> {
        for arg in arguments {
            if let CallArgument::MoveWhole(place) = arg
                && owned(context.places[place.index()].ty, context.types, context.data)
            {
                self.take(&ProjectedPlace { root: *place, path: vec![] }, context.site)?;
            }
        }
        Ok(())
    }
}

/// An intact value of this carrier can be displaced without inspecting its
/// contents. Copy permission alone does not retire a stored descriptor or
/// run a nominal cleanup. A whole sum has to permit displacement in any case.
pub(in crate::machines) fn plain_copy_storage(
    ty: TypeId,
    types: &LoweringTypes,
    data: &[LoweredData],
) -> bool {
    enum Visit {
        Enter(TypeId),
        Finish(TypeId),
    }
    use Visit::{Enter, Finish};
    let mut pending = vec![Enter(ty)];
    let mut active = std::collections::HashSet::new();
    let mut complete = std::collections::HashSet::new();
    while let Some(visit) = pending.pop() {
        let ty = match visit {
            Finish(ty) => {
                active.remove(&ty);
                complete.insert(ty);
                continue;
            }
            Enter(ty) if complete.contains(&ty) => continue,
            Enter(ty) => ty,
        };
        if !active.insert(ty) {
            return false;
        }
        pending.push(Finish(ty));
        match types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_)) => {}
            Some(TypeShape::FixedArray { element, .. }) => pending.push(Enter(*element)),
            Some(TypeShape::Nominal(id)) => {
                let Some(schema) = data.get(id.get() - 1) else { return false };
                if schema.multiplicity != Multiplicity::Copy || schema.cleanup_target.is_some() {
                    return false;
                }
                let fields = schema
                    .fields
                    .iter()
                    .chain(schema.cases.iter().flat_map(|case| &case.fields));
                pending.extend(fields.map(|field| Enter(field.carrier)));
            }
            _ => return false,
        }
    }
    true
}

pub(in crate::machines) fn owned(
    mut ty: TypeId,
    types: &LoweringTypes,
    data: &[LoweredData],
) -> bool {
    loop {
        match types.get(ty) {
            Some(TypeShape::FixedArray { element, .. }) => ty = *element,
            Some(TypeShape::Nominal(id)) => return data[id.get() - 1].multiplicity != Multiplicity::Copy,
            _ => return false,
        }
    }
}

pub(in crate::machines) fn dispose_temporary(
    expressions: &Expressions<'_>,
    body: &mut MachineBody,
    flow: &mut branches::BlockFlow,
    place: PlaceId,
    site: Site,
) -> Result<(), LoweringError> {
    let context = CleanupContext {
        places: &body.places,
        types: expressions.types,
        data: expressions.data,
        site: &site,
    };
    let mut actions = Vec::new();
    context.at(&LiveRoot { place, moved: vec![] }, vec![], body.places[place.index()].ty, &mut actions)?;
    let target = flow.reserve();
    let mut edge = branches::successor(body, target, vec![], site);
    edge.cleanup = actions;
    flow.close(body, BlockTerminator::Jump(edge));
    flow.open(body, target, None);
    Ok(())
}

#[path = "parked.rs"]
mod parked;
pub(crate) use parked::OwnedCrossings;

pub(in crate::machines) fn schedule(
    body: &mut MachineBody,
    types: &LoweringTypes,
    data: &[LoweredData],
    callees: &super::super::calls::Callees<'_>,
) -> Result<OwnedCrossings, LoweringError> {
    let mut crossings = OwnedCrossings::default();
    if !body.places.iter().any(|place| owned(place.ty, types, data)) {
        return Ok(crossings);
    }
    let mut initial = Frontier::default();
    for p in &body.parameters {
        if let Parameter::Structural(id) = p
            && owned(body.places[id.index()].ty, types, data)
        {
            initial.establish(*id);
        }
    }
    // Only an actual reconvergence compares frontiers. A straight continuation
    // may still fill the exact vacant fields of its partially transferred root.
    let mut predecessors = vec![0usize; body.blocks.len()];
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::Jump(edge) => predecessors[edge.target.get() - 1] += 1,
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                predecessors[when_true.target.get() - 1] += 1;
                predecessors[when_false.target.get() - 1] += 1;
            }
            _ => {}
        }
    }
    let mut arrivals = vec![None; body.blocks.len()];
    arrivals[0] = Some(initial);
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let mut state = arrivals[index].as_ref().expect("queued predecessor").clone();
        let block = &mut body.blocks[index];
        for &parameter in &block.structural_parameters {
            if owned(body.places[parameter.index()].ty, types, data) {
                state.establish(parameter);
            }
        }
        let mut contents = parked::LocalContents::default();
        for ordinal in block.operations.clone() {
            let site = &body.diagnostics.operations[ordinal];
            let context = CleanupContext {
                places: &body.places,
                types,
                data,
                site,
            };
            contents.observe(&body.operations[ordinal], &body.mutable_homes);
            // Snapshot ownership before this call consumes its arguments.
            if let OperationKind::Call { callee, arguments, .. } = &body.operations[ordinal].kind
                && callees.may_park(*callee)
            {
                let lineage = parked::Lineage {
                    operations: &body.operations[..ordinal],
                    contents: &contents,
                    mutable_homes: &body.mutable_homes,
                };
                let call = crate::CallSite::Operation(body.operations[ordinal].id);
                crossings.record(call, &state, arguments, &context, &lineage)?;
            }
            let op = &mut body.operations[ordinal];
            match &mut op.kind {
                OperationKind::MoveStructural { source } if owned(body.places[source.root.index()].ty, types, data) => state.transfer(source, site)?,
                OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
                    for field in fields {
                        if let crate::ValueOperand::MoveWhole(value) = field.operand {
                            state.take(&ProjectedPlace { root: value, path: vec![] }, site)?;
                        }
                    }
                }
                // Each owned element transfers once beneath its constructor ordinal.
                OperationKind::EstablishStructuralArray { elements } => {
                    for &element in elements.iter() {
                        if owned(body.places[element.index()].ty, types, data) {
                            state.take(&ProjectedPlace { root: element, path: vec![] }, site)?;
                        }
                    }
                }
                OperationKind::DecomposeRecord { source, fields, .. } => {
                    if owned(body.places[source.root.index()].ty, types, data) {
                        state.take(source, site)?;
                    }
                    for component in fields {
                        if let OperationResult::Structural(p) = component.result
                            && owned(body.places[p.index()].ty, types, data)
                        {
                            state.establish(p);
                        }
                    }
                }
                OperationKind::StructuralStore {
                    destination,
                    value,
                    disposition,
                } => {
                    let ty = body.places[value.index()].ty;
                    if owned(ty, types, data) {
                        state.take(&ProjectedPlace { root: *value, path: vec![] }, site)?;
                    }
                    if matches!(disposition, crate::StructuralStoreDisposition::Dispose(_)) {
                        *disposition = state.replace(destination, ty, &context)?;
                    }
                }
                OperationKind::Call { arguments, .. } => state.consume_arguments(arguments, &context)?,
                _ => {}
            }
            if let OperationResult::Structural(place) = op.result
                && owned(body.places[place.index()].ty, types, data)
            {
                state.establish(place);
            }
        }
        let mut arrive = |edge: &crate::SuccessorEdge, mut incoming: Frontier| -> Result<(), LoweringError> {
            let site = &body.diagnostics.edges[edge.id.get() - 1];
            let context = CleanupContext {
                places: &body.places,
                types,
                data,
                site,
            };
            incoming.consume_arguments(&edge.structural_arguments, &context)?;
            incoming.consume_cleanups(&edge.cleanup, &context)?;
            let target = edge.target;
            if predecessors[target.get() - 1] > 1 {
                // Writable homes keep exact vacant paths for field-by-field restoration.
                incoming
                    .0
                    .retain(|root| body.mutable_homes.contains(&root.place) || !root.exhausted(&context));
            }
            // Disagreeing arrivals are the lowered-form checker's refusal
            // (terminal-psi/control_flow.md#graph-and-frontier). Each arriving
            // edge and the transfers before it stay in Psi, so the checker can
            // replay every arrival and compare what survives. Lowering schedules
            // the join's later cleanup from the first arrival it visits
            // (breadth-first from entry, a true edge before its false edge);
            // the checker judges that cleanup only when every arrival agrees.
            let slot = &mut arrivals[target.get() - 1];
            if slot.is_none() {
                *slot = Some(incoming);
                pending.push_back(target.get() - 1);
            }
            Ok(())
        };
        match &mut block.terminator {
            BlockTerminator::Jump(edge) => arrive(edge, state)?,
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                arrive(when_true, state.clone())?;
                arrive(when_false, state)?;
            }
            BlockTerminator::Return { id, result, cleanup } => {
                let site = &body.diagnostics.edges[id.get() - 1];
                if let ReturnEdge::Structural { source, .. } = result
                    && owned(body.places[source.index()].ty, types, data)
                {
                    state.take(&ProjectedPlace { root: *source, path: vec![] }, site)?;
                }
                *cleanup = dispositions(&state, &body.places, types, data, site)?;
            }
            BlockTerminator::TerminalCall {
                id,
                callee,
                arguments,
                cleanup,
                ..
            } => {
                let site = &body.diagnostics.edges[id.get() - 1];
                if callees.may_park(*callee) {
                    let context = CleanupContext { places: &body.places, types, data, site };
                    for arg in arguments.iter() {
                        contents.operand(arg);
                    }
                    let lineage = parked::Lineage {
                        operations: &body.operations[..block.operations.end],
                        contents: &contents,
                        mutable_homes: &body.mutable_homes,
                    };
                    crossings.record(crate::CallSite::Edge(*id), &state, arguments, &context, &lineage)?;
                }
                state.consume_arguments(arguments, &CleanupContext { places: &body.places, types, data, site })?;
                *cleanup = dispositions(&state, &body.places, types, data, site)?;
            }
            // Exit abandons the bound domain: no cleanup, receipt or merged frontier.
            BlockTerminator::ExternalExit(_) => {}
        }
    }
    crossings.sort_by_call();
    Ok(crossings)
}

fn dispositions(
    state: &Frontier,
    places: &[crate::Place],
    types: &LoweringTypes,
    data: &[LoweredData],
    site: &Site,
) -> Result<Vec<CleanupAction>, LoweringError> {
    let mut actions = Vec::new();
    let context = CleanupContext { places, types, data, site };
    for root in state.0.iter().rev() {
        context.at(root, vec![], places[root.place.index()].ty, &mut actions)?;
    }
    Ok(actions)
}

struct CleanupContext<'a> {
    places: &'a [crate::Place],
    types: &'a LoweringTypes,
    data: &'a [LoweredData],
    site: &'a Site,
}

impl CleanupContext<'_> {
    fn at(
        &self,
        root: &LiveRoot,
        start: Vec<PlaceProjection>,
        ty: TypeId,
        actions: &mut Vec<CleanupAction>,
    ) -> Result<(), LoweringError> {
        enum Work {
            Region(Vec<PlaceProjection>,TypeId),
            Array { prefix:Vec<PlaceProjection>, remaining:u64, element:TypeId },
        }
        let Self { types, data, site, .. } = *self;
        let mut pending = vec![Work::Region(start, ty)];
        while let Some(work) = pending.pop() {
            let (mut path,ty) = match work {
                Work::Region(path,ty)=>(path,ty),
                Work::Array{prefix,remaining,element}=>{
                    let Some(ordinal)=remaining.checked_sub(1) else {continue};
                    let mut child=prefix.clone();
                    child.push(PlaceProjection::FixedIndex(ordinal));
                    pending.push(Work::Array{prefix,remaining:ordinal,element});
                    (child,element)
                }
            };
            if root.moved.binary_search(&path).is_ok() {continue;}
            let holes=root.holes_below(&path);
            if let Some(&TypeShape::FixedArray {element,length})=types.get(ty)
                && (!holes.is_empty() || ensure_disposable(element,types,data,site,LinearDebt::Refuse).is_err())
            {
                // Each intact child contributes at least one action. Keep only
                // the descending cursor and the current child's path in scratch.
                let count=usize::try_from(length).map_err(|_|LoweringError::new(site,"fixed-array residual schedule exceeds the host address space"))?;
                actions.try_reserve(count.saturating_sub(holes.len())).map_err(|_|LoweringError::new(site,"insufficient storage for the fixed-array residual schedule"))?;
                pending.push(Work::Array{prefix:path,remaining:length,element});
                continue;
            }
            if !holes.is_empty() {
                let Some(TypeShape::Nominal(id)) = types.get(ty) else {
                    return Err(LoweringError::new(site, "partial transfer needs a nominal field roster"));
                };
                let schema = &data[id.get() - 1];
                // A partially moved value with a selected cleanup still lists
                // its live complement. No action invokes the cleanup on it:
                // L6 compares the moves with that selection and refuses.
                if schema.carry.is_some() || schema.multiplicity == Multiplicity::Linear {
                    return Err(LoweringError::new(
                        site,
                        "partial transfer requires active case, carry or linear disposition evidence",
                    ));
                }
                let fields = root
                    .partial_fields(&mut path, schema)
                    .ok_or_else(|| LoweringError::new(site, "partial payload moves disagree on the selected case"))?;
                for field in fields {
                    let mut child = path.clone();
                    child.push(PlaceProjection::Field(field.declaration.clone()));
                    pending.push(Work::Region(child,field.carrier));
                }
            } else {
                if let Some(TypeShape::Nominal(id)) = types.get(ty)
                    && let Some(target) = data[id.get() - 1].cleanup_target
                {
                    actions.push(CleanupAction::Invoke {
                        place: ProjectedPlace { root: root.place, path },
                        ty,
                        target,
                    });
                    continue;
                }
                if let Err(error) = ensure_disposable(ty, types, data, site, LinearDebt::Refuse) {
                    let Some(TypeShape::Nominal(id)) = types.get(ty) else {
                        return Err(error);
                    };
                    let record = &data[id.get() - 1];
                    if record.multiplicity == Multiplicity::Linear && record.carry.is_none() {
                        // A dying linear value has no authorized disposition here. Publish
                        // the discard the edge performs so the checker can refuse it against
                        // the declared multiplicity, as it judges an affine discard. Contents
                        // that need their own action still stop here: one discard cannot
                        // stand in for a cleanup call or a loan restoration.
                        ensure_disposable(ty, types, data, site, LinearDebt::Publish)?;
                    } else if record.multiplicity == Multiplicity::Linear || record.carry.is_some() || !record.cases.is_empty() {
                        return Err(error);
                    } else {
                        for field in &record.fields {
                            let mut child = path.clone();
                            child.push(PlaceProjection::Field(field.declaration.clone()));
                            pending.push(Work::Region(child,field.carrier));
                        }
                        continue;
                    }
                }
                if path.is_empty() {
                    actions.push(CleanupAction::DiscardRoot(root.place));
                } else {
                    actions.push(CleanupAction::DiscardResidual {
                        place: ProjectedPlace { root: root.place, path },
                        ty,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Whether a discard may name linear storage. `Publish` leaves the verdict on
/// that discard to the checker, which reads the multiplicity from the Psi data.
#[derive(Clone, Copy, PartialEq)]
enum LinearDebt {
    Refuse,
    Publish,
}

fn ensure_disposable(
    ty: TypeId,
    types: &LoweringTypes,
    data: &[LoweredData],
    site: &Site,
    linear: LinearDebt,
) -> Result<(), LoweringError> {
    let mut pending = vec![ty];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match types.get(ty) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. }) => {}
            // A stored reference or view is a loan carrier, not an owner of its
            // referent. Discarding it ends that loan, exactly as overwriting the
            // field would, and never visits the referent. The lowered-form checker
            // judges when the lender regains access from the lowered loan
            // operations; no cleanup action restores it.
            Some(
                TypeShape::ScalarReference { .. }
                | TypeShape::NominalReference { .. }
                | TypeShape::IndexedSharedReference(_)
                | TypeShape::MutableElementView { .. }
                | TypeShape::MutableScalarElementView { .. }
                | TypeShape::SharedElementView { .. }
                | TypeShape::SharedByteView,
            ) => {}
            Some(TypeShape::FixedArray { element, .. }) => pending.push(*element),
            Some(TypeShape::Nominal(id)) => {
                let schema = &data[id.get() - 1];
                let linear_refused = schema.multiplicity == Multiplicity::Linear && linear == LinearDebt::Refuse;
                if linear_refused || schema.carry.is_some() || schema.cleanup_target.is_some() {
                    return Err(LoweringError::new(site, "normal exit needs an authorized disposition for owned storage"));
                }
                pending.extend(
                    schema
                        .fields
                        .iter()
                        .chain(schema.cases.iter().flat_map(|case| &case.fields))
                        .map(|f| f.carrier),
                );
            }
            _ => return Err(LoweringError::new(site, "normal exit needs field loan or resource disposition evidence")),
        }
    }
    Ok(())
}
