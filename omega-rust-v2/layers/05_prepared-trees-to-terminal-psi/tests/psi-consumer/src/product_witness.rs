//! Product predicates shared by a positive L5 test and its in-suite corruption control.
//! Each witness returns its first disagreement instead of panicking, so the control can
//! show that the same predicate that accepts the lowered product rejects a corrupted copy.
use omega_psi::{
    BlockTerminator, CallArgument, CleanupAction, MachineBody, OperationKind, Parameter, PlaceId, PlaceProjection, ProjectedPlace, ReturnEdge,
    StructuralStoreDisposition,
};

pub type Witness = Result<(), String>;

fn structural_formal(
    body: &MachineBody,
    index: usize,
) -> Result<PlaceId, String> {
    match body.parameters.get(index) {
        Some(Parameter::Structural(place)) => Ok(*place),
        other => Err(format!("formal {index} is not a structural input: {other:?}")),
    }
}

fn names_field(
    place: &ProjectedPlace,
    root: PlaceId,
    field: &str,
) -> bool {
    place.root == root && matches!(place.path.as_slice(), [PlaceProjection::Field(f)] if f.path == field)
}

/// The entry branch hands each successor the stated formal and that successor returns it unchanged.
pub fn branch_returns_formals(
    body: &MachineBody,
    when_true: usize,
    when_false: usize,
) -> Witness {
    let Some(BlockTerminator::Conditional {
        when_true: true_edge,
        when_false: false_edge,
        ..
    }) = body.blocks.first().map(|b| &b.terminator)
    else {
        return Err("entry block does not branch".into());
    };
    for (arm, edge, formal) in [
        ("true", true_edge, when_true),
        ("false", false_edge, when_false),
    ] {
        let input = structural_formal(body, formal)?;
        let block = body
            .blocks
            .get(edge.target.get() - 1)
            .ok_or_else(|| format!("{arm} edge has no destination"))?;
        match &block.terminator {
            BlockTerminator::Return {
                result: ReturnEdge::Structural { source, .. },
                ..
            } if *source == input => {}
            other => return Err(format!("{arm} successor returns {other:?}, expected formal {formal}")),
        }
    }
    Ok(())
}

/// Each operand reads the listed formal at its authored position: a scalar formal
/// by value and a structural formal as a whole copy. Operand types are not consulted,
/// so swapping two equal-typed formals is a disagreement.
pub fn call_reads_formals(
    body: &MachineBody,
    arguments: &[CallArgument],
    formals: &[usize],
) -> Witness {
    let expected = formals
        .iter()
        .map(|&index| match body.parameters.get(index) {
            Some(Parameter::Scalar(value)) => Ok(CallArgument::Scalar(*value)),
            Some(Parameter::Structural(place)) => Ok(CallArgument::CopyWhole(*place)),
            None => Err(format!("missing formal {index}")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if arguments == expected.as_slice() {
        Ok(())
    } else {
        Err(format!("call operands {arguments:?}, expected formals {formals:?} as {expected:?}"))
    }
}

/// The first structural store refills a vacancy rather than replacing a live value.
pub fn vacancy_refill(body: &MachineBody) -> Result<(&ProjectedPlace, PlaceId), String> {
    let (destination, value, disposition) = body
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::StructuralStore {
                destination,
                value,
                disposition,
            } => Some((destination, *value, disposition)),
            _ => None,
        })
        .ok_or("no structural store")?;
    match disposition {
        StructuralStoreDisposition::FillVacancy => Ok((destination, value)),
        other => Err(format!("store into {destination:?} is {other:?}, expected a vacancy refill")),
    }
}

/// Restoration: the refill writes exactly the place the first exclusive move vacated.
pub fn restores_moved_place(body: &MachineBody) -> Witness {
    let moved = body
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::BorrowedMove { source } => Some(source),
            _ => None,
        })
        .ok_or("no exclusive move")?;
    let (destination, _) = vacancy_refill(body)?;
    if destination == moved {
        Ok(())
    } else {
        Err(format!("refill writes {destination:?}, but the move vacated {moved:?}"))
    }
}

/// A single-block body moves `field` of its owned formal into a call and its return edge
/// disposes only the residual `sibling`, never the transferred field or the whole root.
pub fn transfer_leaves_sibling(
    body: &MachineBody,
    field: &str,
    sibling: &str,
) -> Witness {
    let [transfer, call] = body.operations.as_slice() else {
        return Err(format!("expected a transfer and a call, found {} operations", body.operations.len()));
    };
    let [Parameter::Structural(root)] = body.parameters[..] else {
        return Err("expected one owned formal".into());
    };
    match &transfer.kind {
        OperationKind::MoveStructural { source } if names_field(source, root, field) => {}
        other => return Err(format!("transfer {other:?} does not move {field}")),
    }
    match &call.kind {
        OperationKind::Call { arguments, .. } if matches!(arguments.as_slice(), [CallArgument::MoveWhole(place)] if transfer.result == omega_psi::OperationResult::Structural(*place)) =>
            {}
        other => return Err(format!("call {other:?} does not receive the transferred field")),
    }
    match body.blocks.first().map(|b| &b.terminator) {
        Some(BlockTerminator::Return { cleanup, .. }) if matches!(cleanup.as_slice(), [CleanupAction::DiscardResidual { place, .. }] if names_field(place, root, sibling)) => {
            Ok(())
        }
        other => Err(format!("return cleanup {other:?}, expected only the residual {sibling}")),
    }
}

/// Whether any action disposes storage beneath `root`.
fn disposes_beneath(
    actions: &[CleanupAction],
    root: PlaceId,
) -> bool {
    actions.iter().any(|action| match action {
        CleanupAction::DiscardRoot(place) => *place == root,
        CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => place.root == root,
    })
}

/// The checker compares what each arrival at a join leaves of an owned formal.
/// Read that comparison input from Psi alone: the entry branch reaches one block per
/// arm, the arm moves exactly the named field of the first formal, its jump hands that
/// claim to the shared join, and neither the branch edge nor the jump disposes any
/// of the formal's remaining storage. Each arrival's moved field therefore fixes what
/// survives it.
pub fn join_arrivals_move_fields(
    body: &MachineBody,
    when_true: &str,
    when_false: &str,
) -> Witness {
    let root = structural_formal(body, 0)?;
    let Some(BlockTerminator::Conditional {
        when_true: true_edge,
        when_false: false_edge,
        ..
    }) = body.blocks.first().map(|b| &b.terminator)
    else {
        return Err("entry block does not branch".into());
    };
    let mut join = None;
    for (arm, branch, field) in [
        ("true", true_edge, when_true),
        ("false", false_edge, when_false),
    ] {
        if disposes_beneath(&branch.cleanup, root) {
            return Err(format!("{arm} branch edge disposes the formal: {:?}", branch.cleanup));
        }
        let block = body
            .blocks
            .get(branch.target.get() - 1)
            .ok_or_else(|| format!("{arm} edge has no destination"))?;
        let operations = body
            .operations
            .get(block.operations.clone())
            .ok_or_else(|| format!("{arm} arm names missing operations"))?;
        let moves: Vec<_> = operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::MoveStructural { source } if source.root == root => Some((source, op.result)),
                _ => None,
            })
            .collect();
        let [(source, claim)] = moves[..] else {
            return Err(format!("{arm} arm moves {moves:?} from the formal, expected one transfer"));
        };
        if !names_field(source, root, field) {
            return Err(format!("{arm} arm moves {source:?}, expected {field}"));
        }
        let BlockTerminator::Jump(arrival) = &block.terminator else {
            return Err(format!("{arm} arm ends with {:?}, expected a jump to the join", block.terminator));
        };
        if !matches!(arrival.structural_arguments.as_slice(), [CallArgument::MoveWhole(place)] if claim == omega_psi::OperationResult::Structural(*place)) {
            return Err(format!("{arm} arrival hands over {:?}, not its moved field", arrival.structural_arguments));
        }
        if disposes_beneath(&arrival.cleanup, root) {
            return Err(format!("{arm} arrival disposes the formal: {:?}", arrival.cleanup));
        }
        match join {
            None => join = Some(arrival.target),
            Some(target) if target == arrival.target => {}
            Some(target) => return Err(format!("arms reach {target:?} and {:?}, not one join", arrival.target)),
        }
    }
    Ok(())
}

/// The scalar entry value of formal `index`.
fn scalar_formal(
    body: &MachineBody,
    index: usize,
) -> Result<omega_psi::ValueId, String> {
    match body.parameters.get(index) {
        Some(Parameter::Scalar(value)) => Ok(*value),
        other => Err(format!("formal {index} is not a scalar input: {other:?}")),
    }
}

/// The evaluated selector of one runtime element directly below `view`'s referent.
fn view_selector(
    place: &ProjectedPlace,
    view: PlaceId,
) -> Option<omega_psi::ValueId> {
    match place.path.as_slice() {
        [PlaceProjection::Referent, PlaceProjection::ViewElement { index, .. }] if place.root == view => Some(index.selector),
        _ => None,
    }
}

/// The published entry clauses state `left operator right` over these formals, in either order.
/// Only an authored clause can be this evidence; a store or move never states a relation.
fn states_relation(
    machine: &omega_psi::LoweredMachine,
    left: usize,
    right: usize,
    operator: omega_psi::PredicateOperator,
) -> Witness {
    use omega_psi::PredicateStep::{Apply, Formal};
    let stated = machine.contract.requires.iter().any(|clause| {
        clause.predicate == [Formal(left), Formal(right), Apply(operator)] || clause.predicate == [Formal(right), Formal(left), Apply(operator)]
    });
    if stated {
        Ok(())
    } else {
        Err(format!("no entry clause states formal {left} {operator:?} formal {right}: {:?}", machine.contract.requires))
    }
}

/// Each element extraction and each element store of `view`, in operation order, as
/// (selector, result place) and (selector, stored value, disposition).
#[allow(clippy::type_complexity)]
fn element_window(
    body: &MachineBody,
    view: PlaceId,
) -> Result<(Vec<(omega_psi::ValueId, PlaceId)>, Vec<(omega_psi::ValueId, PlaceId, &StructuralStoreDisposition)>), String> {
    let (mut moves, mut stores) = (Vec::new(), Vec::new());
    for op in &body.operations {
        match (&op.kind, op.result) {
            (OperationKind::BorrowedMove { source }, omega_psi::OperationResult::Structural(place)) => {
                moves.push((view_selector(source, view).ok_or_else(|| format!("extraction {source:?} is not one element of the view"))?, place));
            }
            (
                OperationKind::StructuralStore {
                    destination,
                    value,
                    disposition,
                },
                _,
            ) => stores.push((
                view_selector(destination, view).ok_or_else(|| format!("store {destination:?} is not one element of the view"))?,
                *value,
                disposition,
            )),
            _ => {}
        }
    }
    Ok((moves, stores))
}

/// Whether some cleanup edge of the body discards exactly this owned place.
fn discards(
    body: &MachineBody,
    place: PlaceId,
) -> bool {
    body.blocks.iter().any(|block| {
        let cleanup = match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => cleanup.as_slice(),
            BlockTerminator::Jump(edge) => edge.cleanup.as_slice(),
            _ => &[],
        };
        cleanup.contains(&CleanupAction::DiscardRoot(place))
    })
}

/// Under an authored `vacated == stored` entry clause, the program's one extraction vacates
/// the element selected by formal `vacated` and its one store fills the element selected by
/// formal `stored`, without extracting or discarding that element. The two selectors stay
/// distinct values, so only the published clause can justify the repair.
pub fn premised_vacancy_fill(
    machine: &omega_psi::LoweredMachine,
    view: usize,
    vacated: usize,
    stored: usize,
) -> Witness {
    let body = &machine.body;
    states_relation(machine, vacated, stored, omega_psi::PredicateOperator::Equal)?;
    let (root, taken, written) = (structural_formal(body, view)?, scalar_formal(body, vacated)?, scalar_formal(body, stored)?);
    let (moves, stores) = element_window(body, root)?;
    let [(selector, removed)] = moves[..] else {
        return Err(format!("expected one extraction, found {moves:?}"));
    };
    if selector != taken {
        return Err(format!("the extraction selects {selector:?}, expected formal {vacated}"));
    }
    let [(selector, _, disposition)] = stores[..] else {
        return Err(format!("expected one store, found {stores:?}"));
    };
    if selector != written || *disposition != StructuralStoreDisposition::FillVacancy {
        return Err(format!("the store selects {selector:?} as {disposition:?}, expected a vacancy fill through formal {stored}"));
    }
    if discards(body, removed) {
        return Err("the extracted element is discarded instead of restored".into());
    }
    Ok(())
}

/// Under an authored `vacated != stored` entry clause, the store through formal `stored`
/// first extracts that occupied element and discards it on a cleanup edge, then fills it;
/// a later store fills the element vacated through formal `vacated`.
pub fn premised_sibling_displacement(
    machine: &omega_psi::LoweredMachine,
    view: usize,
    vacated: usize,
    stored: usize,
) -> Witness {
    let body = &machine.body;
    states_relation(machine, vacated, stored, omega_psi::PredicateOperator::NotEqual)?;
    let (root, taken, written) = (structural_formal(body, view)?, scalar_formal(body, vacated)?, scalar_formal(body, stored)?);
    let (moves, stores) = element_window(body, root)?;
    let [(first, _), (second, displaced)] = moves[..] else {
        return Err(format!("expected the vacancy and one displacement, found {moves:?}"));
    };
    if (first, second) != (taken, written) {
        return Err(format!("extractions select {first:?} then {second:?}, expected formals {vacated} then {stored}"));
    }
    if !discards(body, displaced) {
        return Err("the displaced occupied element is never discarded".into());
    }
    let selectors: Vec<_> = stores.iter().map(|(selector, _, disposition)| (*selector, (*disposition).clone())).collect();
    let fill = StructuralStoreDisposition::FillVacancy;
    if selectors != [(written, fill.clone()), (taken, fill)] {
        return Err(format!("stores are {selectors:?}, expected fills through formal {stored} then formal {vacated}"));
    }
    Ok(())
}

/// A runtime call after a Boolean branch invokes the descriptor that arrived at the join.
/// The entry block branches on scalar formal `condition`. Each edge reaches an arm whose one
/// operation refines that edge's stated formal into the call's contract and whose jump hands
/// the result to the shared join. The join's only scalar parameter is the descriptor the
/// runtime call invokes, and its arguments read `arguments`' scalar formals in order.
/// Descriptor types are not compared, so swapping which formal each edge reaches is a
/// disagreement even though every value keeps its type.
pub fn runtime_call_after_branch(
    body: &MachineBody,
    condition: usize,
    when_true: usize,
    when_false: usize,
    arguments: &[usize],
) -> Witness {
    use omega_psi::{CallTarget, OperationResult};
    let calls: Vec<_> = body
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, op)| match &op.kind {
            OperationKind::Call { callee, arguments, .. } => Some((index, callee, arguments)),
            _ => None,
        })
        .collect();
    let [(call, CallTarget::Runtime { descriptor, contract }, actuals)] = calls.as_slice() else {
        return Err(format!("expected one runtime call, found {calls:?}"));
    };
    let expected = arguments
        .iter()
        .map(|&formal| scalar_formal(body, formal).map(CallArgument::Scalar))
        .collect::<Result<Vec<_>, _>>()?;
    if **actuals != expected {
        return Err(format!("runtime call arguments {actuals:?}, expected formals {arguments:?}"));
    }
    let Some(BlockTerminator::Conditional {
        condition: tested,
        when_true: true_edge,
        when_false: false_edge,
    }) = body.blocks.first().map(|b| &b.terminator)
    else {
        return Err("entry block does not branch".into());
    };
    if *tested != scalar_formal(body, condition)? {
        return Err(format!("entry branch tests {tested:?}, expected formal {condition}"));
    }
    let mut join = None;
    for (arm, edge, formal) in [
        ("true", true_edge, when_true),
        ("false", false_edge, when_false),
    ] {
        let selected = scalar_formal(body, formal)?;
        let block = body
            .blocks
            .get(edge.target.get() - 1)
            .ok_or_else(|| format!("{arm} edge has no destination"))?;
        let refined = match body.operations.get(block.operations.clone()) {
            Some(
                [omega_psi::PsiOperation {
                    kind: OperationKind::RefineCallableDescriptor { source, contract: into },
                    result: OperationResult::Scalar(refined),
                    ..
                }],
            ) if *source == selected && into == contract => *refined,
            other => return Err(format!("{arm} arm runs {other:?}, expected one refinement of formal {formal} into {contract:?}")),
        };
        let BlockTerminator::Jump(arrival) = &block.terminator else {
            return Err(format!("{arm} arm ends with {:?}, expected a jump to the join", block.terminator));
        };
        if arrival.arguments != [refined] {
            return Err(format!("{arm} arm passes {:?} to the join, expected its refinement {refined:?}", arrival.arguments));
        }
        if *join.get_or_insert(arrival.target) != arrival.target {
            return Err("the arms reach different joins".into());
        }
    }
    let join = &body.blocks[join.expect("two arms").get() - 1];
    if join.scalar_parameters != [*descriptor] || !join.operations.contains(call) {
        return Err(format!(
            "the call in operation {call} invokes {descriptor:?}, expected the parameter {:?} of the join running {:?}",
            join.scalar_parameters, join.operations
        ));
    }
    Ok(())
}

/// Each rank claim of `machine`, re-derived from the transfer it names. The claimed
/// block must hold that exact call into the claimed member, and substituting the
/// member's entry rank with *that* call's actuals must give the claimed successor.
/// Several back edges can enter one member with different actuals, so a claim moved
/// to a sibling edge fails here even though every edge is still claimed once.
/// Every in-component call or terminal transfer of the body must be claimed exactly once.
pub fn cycle_claims(
    package: &omega_psi::LoweredPackage,
    machine: omega_psi::MachineId,
) -> Witness {
    use omega_psi::{CallSite, CallTarget};
    let owner = package.machines.iter().find(|m| m.id == machine).ok_or("no such machine")?;
    let ranking = owner.contract.ranking.as_ref().ok_or("the machine publishes no ranking witness")?;
    let mut internal = Vec::new();
    for block in &owner.body.blocks {
        for op in &owner.body.operations[block.operations.clone()] {
            if let OperationKind::Call { callee: CallTarget::Machine(target), .. } = &op.kind
                && ranking.component.contains(target)
            {
                internal.push((block.id, CallSite::Operation(op.id), *target));
            }
        }
        if let BlockTerminator::TerminalCall { id, callee: CallTarget::Machine(target), .. } = &block.terminator
            && ranking.component.contains(target)
        {
            internal.push((block.id, CallSite::Edge(*id), *target));
        }
    }
    let claimed: Vec<_> = ranking.edges.iter().map(|edge| (edge.block, edge.call, edge.target)).collect();
    if claimed != internal {
        return Err(format!("claims name {claimed:?}, but the body's in-component transfers are {internal:?}"));
    }
    for edge in &ranking.edges {
        let derived = crate::successor_from_call(package, machine, edge)?;
        if edge.successor.as_ref() != Some(&derived) {
            return Err(format!(
                "the claim on {:?} in block {:?} states successor {:?}, but that transfer's own actuals give {derived:?}",
                edge.call, edge.block, edge.successor
            ));
        }
    }
    Ok(())
}

/// Where the cleanup of `root` runs in a body whose only owned local is established
/// before its dispatch: each normal completion of the activation (a `Return` or a
/// `TerminalCall`, including a transfer back into the cycle) invokes `target` on the
/// whole root exactly once. A guard or join edge between blocks disposes nothing,
/// and an external exit abandons the root without cleanup. Returns the completion
/// edges that dispose it, in block order.
pub fn completion_cleanup(
    body: &MachineBody,
    root: PlaceId,
    target: omega_psi::MachineId,
) -> Result<Vec<omega_psi::EdgeId>, String> {
    let whole = |actions: &[CleanupAction]| {
        matches!(actions, [CleanupAction::Invoke { place, target: t, .. }] if place.root == root && place.path.is_empty() && *t == target)
    };
    let mut disposing = Vec::new();
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::Return { id, cleanup, .. } | BlockTerminator::TerminalCall { id, cleanup, .. } => {
                if !whole(cleanup) {
                    return Err(format!("completion {id:?} in block {:?} runs {cleanup:?}, not one whole-root cleanup", block.id));
                }
                disposing.push(*id);
            }
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                for edge in [when_true, when_false] {
                    if !edge.cleanup.is_empty() {
                        return Err(format!("guard edge {:?} disposes {:?} before its arm completes", edge.id, edge.cleanup));
                    }
                }
            }
            BlockTerminator::Jump(edge) => {
                if !edge.cleanup.is_empty() {
                    return Err(format!("jump {:?} disposes {:?} before its arm completes", edge.id, edge.cleanup));
                }
            }
            BlockTerminator::ExternalExit(_) => {}
        }
    }
    Ok(disposing)
}
