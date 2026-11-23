//! Follow the lowered branch edges to derive each normal exit's cleanup from its live places.
use super::super::{branches, expressions::OperationOutput};
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
            if record.multiplicity != Multiplicity::Affine || record.cleanup_target.is_some() {
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

enum Replacement {
    Direct(crate::StructuralStoreDisposition),
    Dispatch {
        plan: sum_cleanup::Plan,
        destination_intact: bool,
    },
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
    ) -> Result<Replacement, LoweringError> {
        let Some(root) = self.0.iter_mut().find(|r| r.place == destination.root) else {
            if destination.path.is_empty() {
                self.establish(destination.root);
                return Ok(Replacement::Direct(crate::StructuralStoreDisposition::FillVacancy));
            }
            return Err(LoweringError::new(context.site, "field replacement requires its live enclosing root"));
        };
        if let Some(i) = root.moved.iter().position(|p| p == &destination.path) {
            root.moved.remove(i);
            return Ok(Replacement::Direct(crate::StructuralStoreDisposition::FillVacancy));
        }
        if root.moved.iter().any(|p| destination.path.starts_with(p)) {
            return Err(LoweringError::new(context.site, "field replacement requires its live enclosing storage"));
        }
        let mut plan = sum_cleanup::Plan::default();
        context.at(root, destination.path.clone(), ty, &mut plan)?;
        let disposition = match plan.into_flat_or_dispatch() {
            Ok(actions) => Replacement::Direct(crate::StructuralStoreDisposition::Dispose(actions)),
            Err(plan) => Replacement::Dispatch {
                plan,
                destination_intact: root.holes_below(&destination.path).is_empty(),
            },
        };
        root.moved.retain(|p| !p.starts_with(&destination.path));
        Ok(disposition)
    }

    fn consume_cleanups(
        &mut self,
        actions: &[CleanupAction],
        context: &CleanupContext<'_>,
    ) -> Result<(), LoweringError> {
        let site = context.site;
        for action in actions {
            match action {
                CleanupAction::RetireHome(place) => {
                    let home = self.live(&ProjectedPlace { root: *place, path: vec![] }, site)?;
                    self.0.remove(home);
                }
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
    /// carrier, such as a `@mutable [T]` view arriving at a join, moves no cleanup
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
            Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                return data[id.get() - 1].multiplicity != Multiplicity::Copy || types.cleanup_target(ty, data).is_some();
            }
            _ => return false,
        }
    }
}

pub(in crate::machines) fn dispose_temporary(
    body: &mut MachineBody,
    flow: &mut branches::BlockFlow,
    place: PlaceId,
    site: Site,
) {
    let target = flow.reserve();
    let mut edge = branches::successor(body, target, vec![], site);
    // Ownership scheduling expands this temporary's disposition before publication.
    edge.cleanup.push(CleanupAction::DiscardRoot(place));
    flow.close(body, BlockTerminator::Jump(edge));
    flow.open(body, target, None);
}

#[path = "sum_cleanup.rs"]
mod sum_cleanup;

#[path = "parked.rs"]
mod parked;
pub(crate) use parked::OwnedCrossings;
pub(in crate::machines) use parked::ReturnBackings;

pub(in crate::machines) fn schedule(
    body: &mut MachineBody,
    types: &LoweringTypes,
    data: &[LoweredData],
    callees: &super::super::calls::Callees<'_>,
    completion_result: OperationOutput,
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
    let scoped_sum_homes: std::collections::HashSet<_> = body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::Jump(edge) => edge.cleanup.iter().find_map(|action| match action {
            CleanupAction::RetireHome(place) => Some(place.index()),
            _ => None,
        }),
        _ => None,
    }).collect();
    let mut arrivals = vec![None; body.blocks.len()];
    arrivals[0] = Some(initial);
    let mut pending = VecDeque::from([0]);
    let mut exit_plans = Vec::new();
    let mut temporary_plans = Vec::new();
    let mut store_plans = Vec::new();
    while let Some(index) = pending.pop_front() {
        let mut state = arrivals[index].as_ref().expect("queued predecessor").clone();
        let block = &mut body.blocks[index];
        for &parameter in &block.structural_parameters {
            if owned(body.places[parameter.index()].ty, types, data) {
                state.establish(parameter);
            }
        }
        for ordinal in block.operations.clone() {
            let site = &body.diagnostics.operations[ordinal];
            let context = CleanupContext {
                places: &body.places,
                types,
                data,
                site,
            };
            // Snapshot ownership before this call consumes its arguments.
            if let OperationKind::Call { callee, arguments, .. } = &body.operations[ordinal].kind
                && callees.may_park(*callee)
            {
                let call = crate::CallSite::Operation(body.operations[ordinal].id);
                crossings.record(call, &state, arguments, &context)?;
            }
            let op = &mut body.operations[ordinal];
            match &mut op.kind {
                OperationKind::MoveStructural { source } if owned(body.places[source.root.index()].ty, types, data) => state.transfer(source, site)?,
                OperationKind::EstablishRecord { fields } | OperationKind::EstablishCase { fields, .. } => {
                    for field in fields {
                        if let crate::ValueOperand::MoveWhole(value) = field.operand
                            && owned(body.places[value.index()].ty, types, data)
                        {
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
                        *disposition = match state.replace(destination, ty, &context)? {
                            Replacement::Direct(disposition) => disposition,
                            Replacement::Dispatch { plan, destination_intact } => {
                                store_plans.push((index, ordinal, plan, destination_intact));
                                if destination_intact {
                                    crate::StructuralStoreDisposition::FillVacancy
                                } else {
                                    // The continuing graph disposes all surviving regions
                                    // before this store. The enclosing home stays reserved.
                                    crate::StructuralStoreDisposition::Dispose(vec![])
                                }
                            }
                        };
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
        let mut arrive = |edge: &mut crate::SuccessorEdge, mut incoming: Frontier| -> Result<(), LoweringError> {
            let site = &body.diagnostics.edges[edge.id.get() - 1];
            let context = CleanupContext {
                places: &body.places,
                types,
                data,
                site,
            };
            incoming.consume_arguments(&edge.structural_arguments, &context)?;
            let owned_case_subject = edge.cleanup.iter().find_map(|action| match action {
                CleanupAction::RetireHome(place) => Some(*place),
                _ => None,
            });
            if !edge.cleanup.is_empty() {
                // Before scheduling, edge cleanup marks local scope exits.
                // Arguments transfer first. A captured root that is still live
                // then disposes, and an exhausted sum home may retire.
                let mut plan = sum_cleanup::Plan::default();
                edge.cleanup.retain(|action| match action {
                    CleanupAction::DiscardRoot(place) => incoming.0.iter().any(|root| root.place == *place),
                    CleanupAction::RetireHome(place) => !body.mutable_homes.contains(place)
                        && incoming.0.iter().any(|root| root.place == *place && root.exhausted(&context)),
                    _ => unreachable!("only unscheduled root markers precede ownership scheduling"),
                });
                for action in &edge.cleanup {
                    match action {
                        CleanupAction::DiscardRoot(place) => {
                            let root = &incoming.0[incoming.live(&ProjectedPlace { root: *place, path: vec![] }, site)?];
                            plan.extend(context.scoped_root(root)?);
                        }
                        CleanupAction::RetireHome(place) => plan.push(CleanupAction::RetireHome(*place)),
                        _ => unreachable!("only unscheduled root markers precede ownership scheduling"),
                    }
                }
                match plan.into_flat_or_dispatch() {
                    Ok(actions) => edge.cleanup = actions,
                    Err(plan) => temporary_plans.push((index, plan)),
                }
            }
            incoming.consume_cleanups(&edge.cleanup, &context)?;
            let target = edge.target;
            if predecessors[target.get() - 1] > 1 {
                // Writable homes keep exact vacant paths for field-by-field restoration.
                incoming
                    .0
                    .retain(|root| body.mutable_homes.contains(&root.place)
                        || scoped_sum_homes.contains(&root.place.index())
                        || !root.exhausted(&context));
            }
            // Disagreeing arrivals are the lowered-form checker's refusal
            // (terminal-psi/control_flow.md#graph-and-frontier). Each arriving
            // edge and the transfers before it stay in Psi, so the checker can
            // replay every arrival and compare what survives. Lowering schedules
            // the join's later cleanup from the first arrival it visits
            // (breadth-first from entry, a true edge before its false edge);
            // the checker judges that cleanup only when every arrival agrees.
            let slot = &mut arrivals[target.get() - 1];
            if let Some(subject) = owned_case_subject
                && let Some(first) = slot.as_ref()
                && first.0.iter().find(|root| root.place == subject) != incoming.0.iter().find(|root| root.place == subject)
            {
                return Err(LoweringError::new(site, "case payload join has different live owned subject regions on its arrivals"));
            }
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
                let context = CleanupContext { places: &body.places, types, data, site };
                match context.frontier(&state)?.into_flat_or_dispatch() {
                    Ok(actions) => *cleanup = actions,
                    Err(plan) => exit_plans.push((index, plan)),
                }
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
                    crossings.record(crate::CallSite::Edge(*id), &state, arguments, &context)?;
                }
                state.consume_arguments(arguments, &CleanupContext { places: &body.places, types, data, site })?;
                let context = CleanupContext { places: &body.places, types, data, site };
                match context.frontier(&state)?.into_flat_or_dispatch() {
                    Ok(actions) => *cleanup = actions,
                    Err(plan) => exit_plans.push((index, plan)),
                }
            }
            // Exit abandons the bound domain: no cleanup, receipt or merged frontier.
            BlockTerminator::ExternalExit(_) => {}
        }
    }
    for (block, plan) in exit_plans {
        sum_cleanup::emit_exit(body, block, plan, types, data, completion_result)?;
    }
    for (block, plan) in temporary_plans {
        sum_cleanup::emit_temporary(body, block, plan, types)?;
    }
    // Split a block's stores from last to first so each earlier split still
    // finds its operation in the original block, before the later suffix.
    for (block, operation, plan, destination_intact) in store_plans.into_iter().rev() {
        sum_cleanup::emit_store(body, block, operation, plan, types, destination_intact)?;
    }
    crossings.sort_by_call();
    let context = CleanupContext {
        places: &body.places,
        types,
        data,
        site: &body.diagnostics.body,
    };
    crossings.trace(body, &context)?;
    Ok(crossings)
}

struct CleanupContext<'a> {
    places: &'a [crate::Place],
    types: &'a LoweringTypes,
    data: &'a [LoweredData],
    site: &'a Site,
}

impl CleanupContext<'_> {
    fn frontier(&self, state: &Frontier) -> Result<sum_cleanup::Plan, LoweringError> {
        let mut plan = sum_cleanup::Plan::default();
        for root in state.0.iter().rev() {
            plan.extend(self.root(root)?);
        }
        Ok(plan)
    }

    fn root(&self, root: &LiveRoot) -> Result<sum_cleanup::Plan, LoweringError> {
        let mut plan = sum_cleanup::Plan::default();
        self.at(root, vec![], self.places[root.place.index()].ty, &mut plan)?;
        if plan.dispatches() {
            plan.retire(root.place);
        }
        Ok(plan)
    }

    fn scoped_root(&self, root: &LiveRoot) -> Result<sum_cleanup::Plan, LoweringError> {
        let plan = self.root(root)?;
        match plan.into_flat_or_dispatch() {
            Ok(mut actions) => {
                let consumes_home = actions.iter().any(|action| match action {
                    CleanupAction::DiscardRoot(place) => *place == root.place,
                    CleanupAction::Invoke { place, .. } => place.root == root.place && place.path.is_empty(),
                    _ => false,
                });
                if !consumes_home {
                    actions.push(CleanupAction::RetireHome(root.place));
                }
                Ok(sum_cleanup::Plan::from_actions(actions))
            }
            Err(plan) => Ok(plan),
        }
    }

    fn at(
        &self,
        root: &LiveRoot,
        start: Vec<PlaceProjection>,
        ty: TypeId,
        plan: &mut sum_cleanup::Plan,
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
                && length != 0
                && (!holes.is_empty() || ensure_disposable(element,types,data,site,LinearDebt::Refuse).is_err())
            {
                // Account for each intact child before enumerating its index.
                // Dynamic case plans reserve phase slots instead of action rows.
                let intact = length.saturating_sub(holes.len() as u64);
                let dispatch = sum_cleanup::contains_dispatch(element, self);
                // Every intact element needs its own case-selection block. The
                // portable block count is u32, independent of available host RAM.
                if dispatch && intact > u64::from(u32::MAX) {
                    return Err(LoweringError::new(site, "fixed-array cleanup exceeds the Psi block count"));
                }
                let count = usize::try_from(intact).map_err(|_| {
                    LoweringError::new(site, "fixed-array residual schedule exceeds the host address space")
                })?;
                plan.reserve_array(count, dispatch).map_err(|_| {
                    LoweringError::new(site, "insufficient storage for the fixed-array residual schedule")
                })?;
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
                if schema.multiplicity == Multiplicity::Linear {
                    return Err(LoweringError::new(
                        site,
                        "partial transfer of a linear value requires disposition evidence",
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
                if let Some(target) = types.cleanup_target(ty, data)
                {
                    plan.push(CleanupAction::Invoke {
                        place: ProjectedPlace { root: root.place, path },
                        ty,
                        target,
                    });
                    continue;
                }
                if let Err(error) = ensure_disposable(ty, types, data, site, LinearDebt::Refuse) {
                    let Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) = types.get(ty) else {
                        return Err(error);
                    };
                    let record = &data[id.get() - 1];
                    if record.multiplicity == Multiplicity::Linear {
                        // A dying linear value has no authorized disposition here. Publish
                        // the discard the edge performs so the checker can refuse it against
                        // the declared multiplicity, as it judges an affine discard. Contents
                        // that need their own action still stop here: one discard cannot
                        // stand in for a cleanup call or a loan restoration.
                        ensure_disposable(ty, types, data, site, LinearDebt::Publish)?;
                    } else if !record.cases.is_empty() {
                        if !sum_cleanup::contains_cleanup_call(record, self) {
                            return Err(error);
                        }
                        let mut alternatives = Vec::new();
                        for case in &record.cases {
                            let mut payload = sum_cleanup::Plan::default();
                            for field in case.fields.iter().rev() {
                                if plain_copy_storage(field.carrier, types, data)
                                    || matches!(types.get(field.carrier), Some(TypeShape::QualifiedScalar { .. }))
                                {
                                    continue;
                                }
                                let mut child = path.clone();
                                child.push(PlaceProjection::Case(case.declaration.clone()));
                                child.push(PlaceProjection::Field(field.declaration.clone()));
                                self.at(root, child, field.carrier, &mut payload)?;
                            }
                            alternatives.push((case.declaration.clone(), payload));
                        }
                        plan.cases(ProjectedPlace { root: root.place, path }, ty, alternatives);
                        continue;
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
                    plan.push(CleanupAction::DiscardRoot(root.place));
                } else {
                    plan.push(CleanupAction::DiscardResidual {
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

/// Refuse a package whose by-value sequence would hold a linear element or one
/// with a selected cleanup, anywhere inside the element. The sequence form
/// admits no linear claim and no executable cleanup
/// (`wiki/spec/terminal-psi/structural_access.md#complete-structural-array-construction`):
/// its copies share one backing, and dropping a copy disposes no element.
/// A callable stored in an element keeps its contract in the callable table.
/// Call this after every nominal cleanup is selected.
pub(crate) fn admit_sequence_elements(
    types: &LoweringTypes,
    data: &[LoweredData],
) -> Result<(), LoweringError> {
    let rows = (0..).map_while(|index| TypeId::from_index(index).and_then(|id| types.get(id)));
    for shape in rows {
        let TypeShape::ElementSequence { element } = shape else {
            continue;
        };
        // Each pending type keeps the outermost record or sum it sits in: the
        // refusal names that declaration, the element as its author wrote it.
        let mut pending: Vec<(TypeId, Option<&LoweredData>)> = vec![(*element, None)];
        let mut visited = std::collections::BTreeSet::new();
        while let Some((ty, outermost)) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            match types.get(ty) {
                Some(TypeShape::FixedArray { element, .. }) => pending.push((*element, outermost)),
                Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                    let schema = &data[id.get() - 1];
                    let outermost = outermost.unwrap_or(schema);
                    if schema.multiplicity == Multiplicity::Linear || types.cleanup_target(ty, data).is_some() {
                        return Err(LoweringError::new(
                            &outermost.site,
                            "an immutable sequence cannot hold an element that is linear or needs cleanup",
                        ));
                    }
                    pending.extend(
                        schema
                            .fields
                            .iter()
                            .chain(schema.cases.iter().flat_map(|case| &case.fields))
                            .map(|field| (field.carrier, Some(outermost))),
                    );
                }
                // A scalar owns nothing. A reference or view owns no referent,
                // and a nested sequence is judged at its own row.
                _ => {}
            }
        }
    }
    Ok(())
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
                | TypeShape::SharedQualifiedScalarReference { .. }
                | TypeShape::NominalReference { .. }
                | TypeShape::IndexedSharedReference(_)
                | TypeShape::MutableElementView { .. }
                | TypeShape::SharedElementView { .. }
                | TypeShape::SharedByteView,
            ) => {}
            // A by-value sequence is a descriptor too. Dropping it disposes no
            // element: they stay in the backing its constructor established,
            // which holds nothing linear and nothing with a cleanup
            // (`admit_sequence_elements`).
            Some(TypeShape::ElementSequence { .. }) => {}
            Some(TypeShape::FixedArray { element, length }) if *length != 0 => pending.push(*element),
            Some(TypeShape::FixedArray { .. }) => {}
            Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                let schema = &data[id.get() - 1];
                let linear_refused = schema.multiplicity == Multiplicity::Linear && linear == LinearDebt::Refuse;
                if linear_refused || types.cleanup_target(ty, data).is_some() {
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
