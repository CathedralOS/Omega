//! Keep known storage behind an exposed address available to the ordinary
//! loan and return checks. A numeric address from the caller has no such place.
use semantic_vocabulary::ValueId;
use terminal_psi::{OperationKind, StructuralArgument, TerminalMachine};

pub struct NativeAddressOrigin<'a> {
    /// Storage whose observed address contributed to this scalar, including at joins.
    pub sources: Vec<&'a StructuralArgument>,
    /// Every route supplies the start of one and the same place.
    pub exact: bool,
}

pub fn address_origin(machine: &TerminalMachine, value: ValueId) -> NativeAddressOrigin<'_> {
    let mut work = vec![value];
    let mut visited = std::collections::HashSet::new();
    let mut sources = Vec::new();
    let mut exact = true;
    while let Some(value) = work.pop() {
        if !visited.insert(value) {
            continue;
        }
        if let Some(operation) = machine.operations().find(|operation| {
            operation.result.as_scalar().is_some_and(|result| result.id == value)
        }) {
            match &operation.kind {
                OperationKind::StorageAddress { source } => {
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
                }
                OperationKind::ScalarRepresentationView { operand, .. }
                | OperationKind::IntegerWiden { operand }
                | OperationKind::IntegerExactCast { operand, .. } => work.push(*operand),
                OperationKind::ExactIntegerAdd { left, right, .. }
                | OperationKind::WrappingIntegerAdd { left, right }
                | OperationKind::SaturatingIntegerAdd { left, right }
                | OperationKind::ExactIntegerSubtract { left, right, .. }
                | OperationKind::WrappingIntegerSubtract { left, right }
                | OperationKind::SaturatingIntegerSubtract { left, right } => {
                    exact = false;
                    work.extend([*left, *right]);
                }
                _ => exact = false,
            }
        } else if let Some((block, position)) = machine.blocks.iter().find_map(|block| {
            block.parameters.iter().position(|parameter| parameter.id == value)
                .map(|position| (block.id, position))
        }) {
            for predecessor in &machine.blocks {
                for arguments in predecessor.terminator.scalar_arguments_to(block) {
                    if let Some(actual) = arguments.get(position) {
                        work.push(*actual);
                    } else {
                        exact = false;
                    }
                }
            }
        } else {
            exact = false;
        }
    }
    NativeAddressOrigin { exact: exact && sources.len() == 1, sources }
}
