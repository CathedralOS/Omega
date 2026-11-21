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
