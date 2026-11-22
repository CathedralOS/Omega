//! Plan ordered disposition, branching only where a live sum's tag selects it.
use super::*;
use crate::machines::body::{MachineBodyValues, next_id};
use crate::machines::expressions::{EmitOperation, OperationOutput};
use crate::{BlockId, BranchBlock, CaseSelection, DeclarationId, Place, PlaceOrigin, ReferenceAccess, SuccessorEdge, ValueOrigin};

#[derive(Default)]
pub(super) struct Plan(Vec<Phase>);

enum Phase {
    Actions(Vec<CleanupAction>),
    Cases {
        source: ProjectedPlace,
        ty: TypeId,
        alternatives: Vec<(DeclarationId, Plan)>,
    },
}

impl Plan {
    pub(super) fn from_actions(actions: Vec<CleanupAction>) -> Self {
        Self(vec![Phase::Actions(actions)])
    }

    fn actions(&mut self) -> &mut Vec<CleanupAction> {
        if !matches!(self.0.last(), Some(Phase::Actions(_))) {
            self.0.push(Phase::Actions(Vec::new()));
        }
        let Some(Phase::Actions(actions)) = self.0.last_mut() else { unreachable!() };
        actions
    }

    pub(super) fn push(
        &mut self,
        action: CleanupAction,
    ) {
        self.actions().push(action);
    }

    pub(super) fn reserve_array(
        &mut self,
        count: usize,
        dispatch: bool,
    ) -> Result<(), std::collections::TryReserveError> {
        if dispatch {
            self.0.try_reserve(count)
        } else {
            self.actions().try_reserve(count)
        }
    }

    pub(super) fn extend(
        &mut self,
        other: Self,
    ) {
        for phase in other.0 {
            match phase {
                Phase::Actions(actions) => self.actions().extend(actions),
                dispatch => self.0.push(dispatch),
            }
        }
    }

    pub(super) fn cases(
        &mut self,
        source: ProjectedPlace,
        ty: TypeId,
        alternatives: Vec<(DeclarationId, Plan)>,
    ) {
        self.0.push(Phase::Cases { source, ty, alternatives });
    }

    pub(super) fn retire(
        &mut self,
        root: PlaceId,
    ) {
        // Settle the whole selected payload before its alternatives rejoin.
        if let [Phase::Cases { source, alternatives, .. }] = self.0.as_mut_slice()
            && source.root == root
            && source.path.is_empty()
        {
            for (_, alternative) in alternatives {
                alternative.push(CleanupAction::RetireHome(root));
            }
        } else {
            self.push(CleanupAction::RetireHome(root));
        }
    }

    pub(super) fn dispatches(&self) -> bool {
        self.0.iter().any(|phase| matches!(phase, Phase::Cases { .. }))
    }

    fn relocate(
        &mut self,
        from: &ProjectedPlace,
        to: PlaceId,
    ) {
        let relocate_place = |place: &mut ProjectedPlace| {
            debug_assert_eq!(place.root, from.root);
            debug_assert!(place.path.starts_with(&from.path));
            place.root = to;
            place.path.drain(..from.path.len());
        };
        for phase in &mut self.0 {
            match phase {
                Phase::Actions(actions) => {
                    for action in actions {
                        match action {
                            CleanupAction::DiscardRoot(place) | CleanupAction::RetireHome(place) => {
                                debug_assert_eq!(*place, from.root);
                                *place = to;
                            }
                            CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => relocate_place(place),
                        }
                    }
                }
                Phase::Cases { source, alternatives, .. } => {
                    relocate_place(source);
                    for (_, alternative) in alternatives {
                        alternative.relocate(from, to);
                    }
                }
            }
        }
    }

    pub(super) fn into_flat_or_dispatch(mut self) -> Result<Vec<CleanupAction>, Self> {
        if self.dispatches() {
            return Err(self);
        }
        let actions = match self.0.pop() {
            Some(Phase::Actions(actions)) => actions,
            None => Vec::new(),
            _ => unreachable!(),
        };
        debug_assert!(self.0.is_empty());
        Ok(actions)
    }
}

/// A child that selects a cleanup machine requires an executable disposition
/// for the case that actually contains it.
pub(super) fn contains_cleanup_call(
    schema: &LoweredData,
    context: &CleanupContext<'_>,
) -> bool {
    let mut pending: Vec<_> = schema
        .cases
        .iter()
        .flat_map(|case| &case.fields)
        .map(|field| field.carrier)
        .collect();
    let mut visited = std::collections::HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        if context.types.cleanup_target(ty, context.data).is_some() {
            return true;
        }
        match context.types.get(ty) {
            Some(TypeShape::FixedArray { element, length }) if *length != 0 => pending.push(*element),
            Some(TypeShape::Nominal(owner) | TypeShape::QualifiedNominal { data: owner, .. }) => {
                let data = &context.data[owner.get() - 1];
                pending.extend(
                    data.fields
                        .iter()
                        .chain(data.cases.iter().flat_map(|case| &case.fields))
                        .map(|field| field.carrier),
                );
            }
            _ => {}
        }
    }
    false
}

/// Transparent array elements can each contribute a case phase. Check
/// the lower bound on their required storage before expanding the array.
pub(super) fn contains_dispatch(
    ty: TypeId,
    context: &CleanupContext<'_>,
) -> bool {
    let mut pending = vec![ty];
    let mut visited = std::collections::HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) || context.types.cleanup_target(ty, context.data).is_some() {
            continue;
        }
        match context.types.get(ty) {
            Some(TypeShape::FixedArray { element, length }) if *length != 0 => pending.push(*element),
            Some(TypeShape::Nominal(id) | TypeShape::QualifiedNominal { data: id, .. }) => {
                let schema = &context.data[id.get() - 1];
                if !schema.cases.is_empty() && contains_cleanup_call(schema, context) {
                    return true;
                }
                pending.extend(schema.fields.iter().map(|field| field.carrier));
            }
            _ => {}
        }
    }
    false
}

fn append_block(
    body: &mut MachineBody,
    start: usize,
    terminator: BlockTerminator,
) -> BlockId {
    let id = BlockId(next_id(body.blocks.len()));
    body.blocks.push(BranchBlock {
        id,
        scalar_parameters: vec![],
        structural_parameters: vec![],
        scalar_domain_invariants: vec![],
        operations: start..body.operations.len(),
        terminator,
    });
    id
}

struct Emitter<'a> {
    body: &'a mut MachineBody,
    site: &'a Site,
    bool_ty: TypeId,
}

impl Emitter<'_> {
    fn edge(
        &mut self,
        target: BlockId,
    ) -> SuccessorEdge {
        branches::successor(self.body, target, vec![], self.site.clone())
    }

    /// Emit each case's prefix on its selected edge, and share the continuation
    /// across the alternatives after all selected payload actions finish.
    fn selected_edge(
        &mut self,
        target: BlockId,
        source: &ProjectedPlace,
        case: DeclarationId,
        plan: Plan,
    ) -> SuccessorEdge {
        let (first_actions, rest) = {
            let mut phases = plan.0.into_iter();
            match phases.next() {
                Some(Phase::Actions(actions)) => (actions, Plan(phases.collect())),
                Some(first) => (Vec::new(), Plan(std::iter::once(first).chain(phases).collect())),
                None => (Vec::new(), Plan::default()),
            }
        };
        let start = self.prepend(rest, target);
        let mut edge = self.edge(start);
        edge.selected_case = Some(CaseSelection { source: source.clone(), case });
        edge.cleanup = first_actions;
        edge
    }

    fn prepend(
        &mut self,
        plan: Plan,
        mut target: BlockId,
    ) -> BlockId {
        for phase in plan.0.into_iter().rev() {
            match phase {
                Phase::Actions(cleanup) if cleanup.is_empty() => {}
                Phase::Actions(cleanup) => {
                    let mut edge = self.edge(target);
                    edge.cleanup = cleanup;
                    target = append_block(self.body, self.body.operations.len(), BlockTerminator::Jump(edge));
                }
                Phase::Cases { source, ty, alternatives } => {
                    target = self.case_dispatch(source, ty, alternatives, target);
                }
            }
        }
        target
    }

    fn case_dispatch(
        &mut self,
        mut source: ProjectedPlace,
        ty: TypeId,
        mut alternatives: Vec<(DeclarationId, Plan)>,
        target: BlockId,
    ) -> BlockId {
        // On every alternative the enclosing home now has the same missing
        // region. A temporary's selected payload and home die before the join.
        let displacement = if source.path.is_empty() {
            None
        } else {
            let start = self.body.operations.len();
            let OperationResult::Structural(old) = self.body.push_operation(
                OperationOutput::Structural(ty),
                OperationKind::MoveStructural { source: source.clone() },
                self.site.clone(),
            ) else {
                unreachable!("moving a sum produces structural storage")
            };
            for (_, plan) in &mut alternatives {
                plan.relocate(&source, old);
                plan.push(CleanupAction::RetireHome(old));
            }
            source = ProjectedPlace { root: old, path: vec![] };
            let edge = self.edge(target);
            Some(append_block(self.body, start, BlockTerminator::Jump(edge)))
        };
        let (last, plan) = alternatives.pop().expect("a sum has a case");
        let mut next = self.selected_edge(target, &source, last, plan);
        let mut tests = alternatives.into_iter().rev().peekable();
        let first = loop {
            let Some((case, plan)) = tests.next() else {
                break append_block(self.body, self.body.operations.len(), BlockTerminator::Jump(next));
            };
            let when_true = self.selected_edge(target, &source, case.clone(), plan);
            let start = self.body.operations.len();
            let OperationResult::Scalar(condition) = self.body.push_operation(
                OperationOutput::Scalar(self.bool_ty),
                OperationKind::StructuralCaseMembership { source: source.clone(), case },
                self.site.clone(),
            ) else {
                unreachable!("case membership is Boolean")
            };
            let test = append_block(
                self.body,
                start,
                BlockTerminator::Conditional {
                    condition,
                    when_true,
                    when_false: next,
                },
            );
            if tests.peek().is_none() {
                break test;
            }
            next = self.edge(test);
        };
        if let Some(displacement) = displacement {
            let BlockTerminator::Jump(edge) = &mut self.body.blocks[displacement.get() - 1].terminator else {
                unreachable!()
            };
            edge.target = first;
            displacement
        } else {
            first
        }
    }
}

fn bool_type(
    types: &LoweringTypes,
    site: &Site,
) -> Result<TypeId, LoweringError> {
    types
        .id_for(&omega_typed::ValueType::Bool)
        .map_err(|error| LoweringError::new(site, error.to_string()))
}

pub(super) fn emit_temporary(
    body: &mut MachineBody,
    original: usize,
    plan: Plan,
    types: &LoweringTypes,
) -> Result<(), LoweringError> {
    let BlockTerminator::Jump(edge) = &body.blocks[original].terminator else {
        unreachable!("a temporary disposes on a normal continuation")
    };
    let site = body.diagnostics.edges[edge.id.get() - 1].clone();
    let (target, bool_ty) = (edge.target, bool_type(types, &site)?);
    let entry = Emitter { body, site: &site, bool_ty }.prepend(plan, target);
    let BlockTerminator::Jump(edge) = &mut body.blocks[original].terminator else {
        unreachable!()
    };
    edge.target = entry;
    edge.cleanup.clear();
    Ok(())
}

pub(super) fn emit_store(
    body: &mut MachineBody,
    original: usize,
    operation: usize,
    mut plan: Plan,
    types: &LoweringTypes,
    destination_intact: bool,
) -> Result<(), LoweringError> {
    let OperationKind::StructuralStore { destination, value, .. } = &body.operations[operation].kind else {
        unreachable!("the store installs the replacement after disposal")
    };
    let destination = destination.clone();
    let ty = body.places[value.index()].ty;
    let site = body.diagnostics.operations[operation].clone();
    let bool_ty = bool_type(types, &site)?;

    // Keep the original store operation identity. Its RHS and destination
    // selector operations already dominate the new displacement and its tests.
    let tail = BlockId(next_id(body.blocks.len()));
    let enter = branches::successor(body, tail, vec![], site.clone());
    let block = &mut body.blocks[original];
    let terminator = std::mem::replace(&mut block.terminator, BlockTerminator::Jump(enter));
    let end = std::mem::replace(&mut block.operations.end, operation);
    body.blocks.push(BranchBlock {
        id: tail,
        scalar_parameters: vec![],
        structural_parameters: vec![],
        scalar_domain_invariants: vec![],
        operations: operation..end,
        terminator,
    });

    let (displacement, cleanup_entry) = if destination_intact {
        let start = body.operations.len();
        let OperationResult::Structural(old) = body.push_operation(
            OperationOutput::Structural(ty),
            OperationKind::MoveStructural { source: destination.clone() },
            site.clone(),
        ) else {
            unreachable!("displacing a value produces a structural place")
        };
        plan.relocate(&destination, old);
        plan.retire(old);

        // Finish the displacement block before appending membership operations.
        let placeholder = branches::successor(body, tail, vec![], site.clone());
        let displacement = append_block(body, start, BlockTerminator::Jump(placeholder));
        let cleanup = Emitter { body, site: &site, bool_ty }.prepend(plan, tail);
        (Some(displacement), cleanup)
    } else {
        // The partial destination itself cannot move. Its live regions
        // dispose in place without retiring the mutable home.
        (None, Emitter { body, site: &site, bool_ty }.prepend(plan, tail))
    };
    if let Some(displacement) = displacement {
        let BlockTerminator::Jump(edge) = &mut body.blocks[displacement.get() - 1].terminator else {
            unreachable!()
        };
        edge.target = cleanup_entry;
    }
    let BlockTerminator::Jump(edge) = &mut body.blocks[original].terminator else {
        unreachable!()
    };
    edge.target = displacement.unwrap_or(cleanup_entry);
    Ok(())
}

pub(super) fn emit_exit(
    body: &mut MachineBody,
    original: usize,
    plan: Plan,
    types: &LoweringTypes,
    data: &[LoweredData],
    completion_result: OperationOutput,
) -> Result<(), LoweringError> {
    let (site_id, mut result, terminal) = match body.blocks[original].terminator {
        BlockTerminator::Return { id, result, .. } => (id, result, false),
        BlockTerminator::TerminalCall { id, .. } => (id, ReturnEdge::Unit, true),
        _ => unreachable!("a planned normal exit"),
    };
    let site = body.diagnostics.edges[site_id.get() - 1].clone();
    let bool_ty = bool_type(types, &site)?;
    let return_id = if terminal {
        let id = crate::EdgeId(next_id(body.diagnostics.edges.len()));
        body.diagnostics.edges.push(site.clone());
        id
    } else {
        site_id
    };
    let finish = append_block(
        body,
        body.operations.len(),
        BlockTerminator::Return {
            id: return_id,
            result,
            cleanup: vec![],
        },
    );
    let mut emitter = Emitter { body, site: &site, bool_ty };
    let first = emitter.prepend(plan, finish);
    if terminal {
        result = match completion_result {
            OperationOutput::Unit => ReturnEdge::Unit,
            OperationOutput::Scalar(ty) => {
                let value = emitter.body.push_value(ty, ValueOrigin::BlockParameter(first));
                emitter.body.blocks[first.get() - 1]
                    .scalar_parameters
                    .push(value);
                ReturnEdge::Scalar(value)
            }
            OperationOutput::Structural(ty) => {
                let result_home = emitter
                    .body
                    .places
                    .iter()
                    .position(|place| place.origin == PlaceOrigin::Result)
                    .map(|index| PlaceId(next_id(index)))
                    .unwrap_or_else(|| push_place(emitter.body, ty, PlaceOrigin::Result));
                let source = capture_place(emitter.body, first, ty);
                ReturnEdge::Structural { source, result: result_home }
            }
        };
        let BlockTerminator::TerminalCall { completion, .. } = &mut emitter.body.blocks[original].terminator else {
            unreachable!()
        };
        *completion = Some(first);
    } else {
        let mut enter = emitter.edge(first);
        // A scalar already names its evaluated SSA value. Capture a structural
        // result before any cleanup, preserving the existing transfer/copy rule.
        if let ReturnEdge::Structural { source, result: output } = result {
            let ty = emitter.body.places[source.index()].ty;
            let exclusive = matches!(
                types.get(ty),
                Some(
                    TypeShape::MutableElementView { .. }
                        | TypeShape::ScalarReference {
                            access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                            ..
                        }
                        | TypeShape::NominalReference {
                            access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
                            ..
                        }
                )
            );
            enter
                .structural_arguments
                .push(if exclusive || owned(ty, types, data) {
                    CallArgument::MoveWhole(source)
                } else {
                    CallArgument::CopyWhole(source)
                });
            result = ReturnEdge::Structural {
                source: capture_place(emitter.body, first, ty),
                result: output,
            };
        }
        emitter.body.blocks[original].terminator = BlockTerminator::Jump(enter);
    }
    let BlockTerminator::Return { result: returned, .. } = &mut emitter.body.blocks[finish.get() - 1].terminator else {
        unreachable!()
    };
    *returned = result;
    Ok(())
}

fn push_place(
    body: &mut MachineBody,
    ty: TypeId,
    origin: PlaceOrigin,
) -> PlaceId {
    let place = PlaceId(next_id(body.places.len()));
    body.places.push(Place { ty, origin });
    place
}

fn capture_place(
    body: &mut MachineBody,
    block: BlockId,
    ty: TypeId,
) -> PlaceId {
    let place = push_place(body, ty, PlaceOrigin::BlockParameter(block));
    body.blocks[block.get() - 1].structural_parameters.push(place);
    place
}
