//! All-path operation-point custody, including length-changing backedges.

use super::*;

fn operation_position(machine: &TerminalMachine, identity: OperationId) -> Option<(usize, usize)> {
    machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, declaration)| {
            declaration
                .operations
                .iter()
                .position(|operation| operation.id == identity)
                .map(|position| (block, position))
        })
}

/// Every finite execution reaching `use_site` must have passed `producer`
/// without an invalidating operation. Visiting a graph point twice adds no
/// additional paths; mutations on cyclic paths are still examined in full.
fn unchanged_since(
    machine: &TerminalMachine,
    producer: OperationId,
    use_site: OperationId,
    invalidates: impl Fn(&terminal_psi::Operation) -> bool,
) -> bool {
    let Some(producer_position) = operation_position(machine, producer) else {
        return false;
    };
    let Some(use_position) = operation_position(machine, use_site) else {
        return false;
    };
    let outgoing = crate::control_graph::successors(machine);
    let mut pending = vec![use_position];
    let mut visited = BTreeSet::new();
    let mut reached_producer = false;
    while let Some((block_index, position)) = pending.pop() {
        if !visited.insert((block_index, position)) {
            continue;
        }
        let block = &machine.blocks[block_index];
        if position != 0 {
            let previous = (block_index, position - 1);
            if previous == producer_position {
                reached_producer = true;
            } else {
                if invalidates(&block.operations[position - 1]) {
                    return false;
                }
                pending.push(previous);
            }
        } else {
            if block.id == machine.entry {
                return false;
            }
            let mut has_predecessor = false;
            for (predecessor_index, predecessor) in machine.blocks.iter().enumerate() {
                if outgoing[&predecessor.id]
                    .iter()
                    .any(|(_, target)| *target == block.id)
                {
                    has_predecessor = true;
                    pending.push((predecessor_index, predecessor.operations.len()));
                }
            }
            if !has_predecessor {
                return false;
            }
        }
    }
    reached_producer
}

fn overlaps(left: &[StructuralPathSegment], right: &[StructuralPathSegment]) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn changes_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    operation: &terminal_psi::Operation,
) -> bool {
    match &operation.kind {
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path: written,
            field,
            ..
        } => {
            *destination == root
                && field_path(module, machine, *destination, written, *field)
                    .is_none_or(|written| overlaps(path, &written))
        }
        OperationKind::WriteOnlyPrimitiveStore { destination, .. } => *destination == root,
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.iter().any(|argument| {
            argument.place == root
                && overlaps(path, &argument.path)
                && matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                        | StructuralAccess::Owned
                )
        }),
        // Dynamic dispatch retains its receiver in a separate catalog. Until
        // that exact footprint is resolved here it cannot preserve this fact.
        OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::StoreDynamicDescriptor { .. } => true,
        _ => false,
    }
}

pub(super) fn exact_length_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> bool {
    let OperationKind::StructuralByteSequenceFieldByteStore {
        destination,
        path,
        field,
        length,
        ..
    } = &operation.kind
    else {
        return false;
    };
    let Some(exact_path) = field_path(module, machine, *destination, path, *field) else {
        return false;
    };
    let Some(producer) = machine.blocks.iter().flat_map(|block| &block.operations).find(|candidate| {
        candidate.result.scalar().is_some_and(|result| result.id == *length)
            && matches!(&candidate.kind, OperationKind::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field }
                if source == destination && measured_path == path && measured_field == field)
    }) else { return false; };
    unchanged_since(machine, producer.id, operation.id, |candidate| {
        changes_length(module, machine, *destination, &exact_path, candidate)
    })
}

pub(crate) fn replacement_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<Option<Proposition>, ModuleError> {
    let OperationKind::StructuralByteSequenceFieldLength {
        source: root,
        path,
        field,
    } = &operation.kind
    else {
        return Ok(None);
    };
    let Some(exact_path) = field_path(module, machine, *root, path, *field) else {
        return Ok(None);
    };
    let result = operation.result.expect_scalar().id;
    for replacement in machine.blocks.iter().flat_map(|block| &block.operations) {
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path: written_path,
            field: written_field,
            source,
            length,
            ..
        } = &replacement.kind
        else {
            continue;
        };
        if destination != root
            || written_path != path
            || written_field != field
            || !unchanged_since(machine, replacement.id, operation.id, |candidate| {
                changes_length(module, machine, *root, &exact_path, candidate)
            })
        {
            continue;
        }
        let scalar_type = byte_count_type();
        let value = ScalarTerm::value(result, scalar_type);
        // Literal identity never changes on reentry. Derive the field count
        // directly, without importing first-iteration scalar assumptions.
        if let Some(literal) = machine.blocks.iter().flat_map(|block| &block.operations).find(|candidate| matches!(&candidate.kind,
            OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == source)) {
            let observation = terminal_semantics::StructuralEffectObservation::ByteSequenceLengthRead { source: *source, result };
            return terminal_semantics::literal_length_equation(&observation, literal)
                .map_err(ModuleError::OperationSemanticSchema);
        }
        // A saved source length is a valid snapshot only while that SSA slot
        // has not been rebound along any path since the replacement.
        if unchanged_since(machine, replacement.id, operation.id, |candidate| {
            candidate
                .result
                .scalar()
                .is_some_and(|result| result.id == *length)
        }) {
            return Ok(Some(Proposition::Equal(
                value,
                ScalarTerm::value(*length, scalar_type),
            )));
        }
    }
    Ok(None)
}

/// Whole mutable-view writes retain their exact observation across every path.
/// Calls that may change the original referent invalidate an earlier extent.
pub(crate) fn view_length_is_current(
    module: &TerminalModule,
    machine: &TerminalMachine,
    producer: OperationId,
    operation: OperationId,
    destination: PlaceId,
) -> bool {
    unchanged_since(machine, producer, operation, |candidate| {
        changes_length(module, machine, destination, &[], candidate)
    })
}
