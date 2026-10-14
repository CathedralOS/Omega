//! Exact whole-value origins through ordinary copies and unanimous CFG joins.
use super::*;
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{StructuralArgument, Terminator};

fn incoming(
    machine: &TerminalMachine,
    target: BlockId,
) -> Option<Vec<(&[ValueId], &[StructuralArgument])>> {
    let mut rows = Vec::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                target: selected,
                arguments,
                structural_arguments,
                ..
            } if *selected == target => {
                rows.push((arguments.as_slice(), structural_arguments.as_slice()));
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    if edge.target == target {
                        rows.push((
                            edge.arguments.as_slice(),
                            edge.structural_arguments.as_slice(),
                        ));
                    }
                }
            }
            Terminator::StructuralCase { cases, .. }
                if cases.iter().any(|edge| edge.target == target) =>
            {
                return None;
            }
            _ => {}
        }
    }
    (!rows.is_empty()).then_some(rows)
}

pub(super) fn whole_origin(machine: &TerminalMachine, place: PlaceId) -> Option<PlaceId> {
    whole(machine, place, &mut Vec::new())
}
fn whole(machine: &TerminalMachine, place: PlaceId, active: &mut Vec<PlaceId>) -> Option<PlaceId> {
    if active.contains(&place) {
        return None;
    }
    active.push(place);
    let result = (|| {
        if let Some((block, position)) = machine.blocks.iter().find_map(|block| {
            block
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == place)
                .map(|position| (block, position))
        }) {
            let mut origin = None;
            for (_, arguments) in incoming(machine, block.id)? {
                let actual = arguments.get(position)?;
                if !actual.path.is_empty()
                    || quotient_place_type(machine, actual.place)
                        != Some(block.structural_parameters[position].structural_type)
                {
                    return None;
                }
                let candidate = whole(machine, actual.place, active)?;
                if origin.is_some_and(|prior| prior != candidate) {
                    return None;
                }
                origin = Some(candidate);
            }
            return origin;
        }
        if let Some((operation, ty)) = machine.producing_operation(place)
            && let OperationKind::StructuralLeafCopy { source, path } = &operation.kind
            && path.is_empty()
            && quotient_place_type(machine, *source) == Some(ty)
        {
            return whole(machine, *source, active);
        }
        Some(place)
    })();
    active.pop();
    result
}

pub(super) fn scalar_origin(machine: &TerminalMachine, value: ValueId) -> Option<ValueId> {
    scalar(machine, value, &mut Vec::new())
}
fn scalar(machine: &TerminalMachine, value: ValueId, active: &mut Vec<ValueId>) -> Option<ValueId> {
    if active.contains(&value) {
        return None;
    }
    active.push(value);
    let result = (|| {
        if let Some((block, position)) = machine.blocks.iter().find_map(|block| {
            block
                .parameters
                .iter()
                .position(|parameter| parameter.id == value)
                .map(|position| (block, position))
        }) {
            let mut origin = None;
            for (arguments, _) in incoming(machine, block.id)? {
                let candidate = scalar(machine, *arguments.get(position)?, active)?;
                if origin.is_some_and(|prior| prior != candidate) {
                    return None;
                }
                origin = Some(candidate);
            }
            return origin;
        }
        let operation = machine.operations().find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == value)
        })?;
        if let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind {
            if !path.is_empty() {
                return None;
            }
            let source = whole_origin(machine, *source)?;
            let (producer, _) = machine.producing_operation(source)?;
            if let OperationKind::EstablishPrimitiveLocal { value, .. } = producer.kind {
                return scalar(machine, value, active);
            }
            return None;
        }
        Some(value)
    })();
    active.pop();
    result
}
