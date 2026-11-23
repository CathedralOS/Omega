//! Follow one concrete execution of a two-store indexed restoration window.
//! Tokens identify custody, even when two cells have identical bytes.
use omega_psi::{
    ArithmeticPolicy, BlockTerminator, CleanupAction, IntegerPrimitive, LoweredMachine, LoweredPackage, MachineId, OperationKind, OperationResult, Parameter,
    PlaceProjection, ProjectedPlace, ReturnEdge, StructuralStoreDisposition,
};

#[derive(Debug, PartialEq)]
pub struct WindowTrace {
    pub cells: [u8; 2],
    /// Completed stores, consumed token and selected cleanup target.
    pub disposal: (usize, u8, Option<MachineId>),
    pub writes: Vec<usize>,
    pub operations: usize,
    pub jumps: usize,
    pub exit: omega_psi::BlockId,
}

pub fn run(
    package: &LoweredPackage,
    machine: &LoweredMachine,
    selectors: &[usize],
) -> Option<WindowTrace> {
    super::view_element_bounds(package, machine).ok()?;
    let body = &machine.body;
    let [
        Parameter::Structural(view),
        Parameter::Structural(spare),
        formals @ ..,
    ] = &body.parameters[..]
    else {
        return None;
    };
    let mut cells = [Some(10), Some(20)];
    let mut owned = vec![None; body.places.len()];
    owned[spare.index()] = Some(30);
    let mut values = vec![None; body.values.len()];
    for (formal, &value) in formals.iter().zip(selectors) {
        let Parameter::Scalar(id) = formal else { return None };
        values[id.index()] = Some(value);
    }
    let mut writes = vec![];
    let mut disposal = None;
    let mut operations = 0;
    let mut jumps = 0;
    let mut block = body.blocks.iter().find(|b| b.id.get() == 1)?;
    let mut visited = vec![false; body.blocks.len()];
    loop {
        if std::mem::replace(visited.get_mut(block.id.get() - 1)?, true) {
            return None;
        }
        for op in body.operations.get(block.operations.clone())? {
            let element = |place: &ProjectedPlace| {
                let [
                    PlaceProjection::Referent,
                    PlaceProjection::ViewElement { index, length },
                ] = &place.path[..]
                else {
                    return None;
                };
                let position = values[index.selector.index()]?;
                (place.root == *view && position < values[length.index()]?).then_some(position)
            };
            operations += 1;
            match (&op.kind, op.result) {
                (OperationKind::ElementViewLength { source }, OperationResult::Scalar(v)) if source == view => values[v.index()] = Some(cells.len()),
                (OperationKind::BorrowedMove { source }, OperationResult::Structural(out)) => {
                    owned[out.index()] = Some(cells.get_mut(element(source)?)?.take()?)
                }
                (OperationKind::MoveStructural { source }, OperationResult::Structural(out)) if source.path.is_empty() => {
                    owned[out.index()] = Some(owned[source.root.index()].take()?);
                }
                (OperationKind::Integer { operation, operands }, OperationResult::Scalar(out))
                    if operation.primitive == IntegerPrimitive::Equal && operation.policy == ArithmeticPolicy::Exact && operation.obligations.is_empty() =>
                {
                    let &[a, b] = operands.as_slice() else { return None };
                    values[out.index()] = Some(usize::from(values[a.index()]? == values[b.index()]?));
                }
                (
                    OperationKind::StructuralStore {
                        destination,
                        value,
                        disposition: StructuralStoreDisposition::FillVacancy,
                    },
                    OperationResult::Unit,
                ) => {
                    let index = element(destination)?;
                    if cells
                        .get_mut(index)?
                        .replace(owned[value.index()].take()?)
                        .is_some()
                    {
                        return None;
                    }
                    writes.push(index);
                }
                _ => return None,
            }
        }
        let edge = match &block.terminator {
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => match values[condition.index()]? {
                0 => when_false,
                1 => when_true,
                _ => return None,
            },
            BlockTerminator::Jump(edge) => edge,
            BlockTerminator::Return {
                result: ReturnEdge::Unit,
                cleanup,
                ..
            } if cleanup.is_empty() && owned.iter().all(Option::is_none) => {
                return Some(WindowTrace {
                    cells: [cells[0]?, cells[1]?],
                    disposal: disposal?,
                    writes,
                    operations,
                    jumps,
                    exit: block.id,
                });
            }
            _ => return None,
        };
        for action in &edge.cleanup {
            let (place, target) = match action {
                CleanupAction::Invoke { place, target, ty } if place.path.is_empty() && body.places[place.root.index()].ty == *ty => {
                    (place.root, Some(*target))
                }
                CleanupAction::DiscardRoot(place) => (*place, None),
                _ => return None,
            };
            if disposal
                .replace((writes.len(), owned[place.index()].take()?, target))
                .is_some()
            {
                return None;
            }
        }
        jumps += 1;
        block = body.blocks.iter().find(|b| b.id == edge.target)?;
    }
}
