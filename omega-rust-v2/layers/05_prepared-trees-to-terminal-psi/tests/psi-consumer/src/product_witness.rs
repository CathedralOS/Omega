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
    for (arm, edge, formal) in [("true", true_edge, when_true), ("false", false_edge, when_false)] {
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
        OperationKind::Call { arguments, .. }
            if matches!(arguments.as_slice(), [CallArgument::MoveWhole(place)] if transfer.result == omega_psi::OperationResult::Structural(*place)) => {}
        other => return Err(format!("call {other:?} does not receive the transferred field")),
    }
    match body.blocks.first().map(|b| &b.terminator) {
        Some(BlockTerminator::Return { cleanup, .. })
            if matches!(cleanup.as_slice(), [CleanupAction::DiscardResidual { place, .. }] if names_field(place, root, sibling)) =>
        {
            Ok(())
        }
        other => Err(format!("return cleanup {other:?}, expected only the residual {sibling}")),
    }
}
