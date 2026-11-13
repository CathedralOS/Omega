use semantic_vocabulary::{EdgeId, PlaceId};
use terminal_psi::{
    OperationKind, StructuralArgument, StructuralStorageUse, TerminalAffineCleanupAction,
    Terminator,
};

use super::PlainAffineFailure;

pub(super) fn reads(kind: &OperationKind) -> Vec<PlaceId> {
    kind.storage_uses()
        .into_iter()
        .filter_map(|used| match used {
            StructuralStorageUse::Place(place) => Some(place),
            StructuralStorageUse::Projection(projection)
            | StructuralStorageUse::Element {
                descriptor: projection,
                ..
            } => Some(projection.root),
            StructuralStorageUse::Canonical { root, .. }
            | StructuralStorageUse::ObservedField { root, .. } => Some(root),
            StructuralStorageUse::Type(_) => None,
        })
        .collect()
}

pub(super) fn arguments(kind: &OperationKind) -> Vec<&StructuralArgument> {
    match kind {
        OperationKind::MoveStructuralValue { source } => vec![source],
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_structural())
            .collect(),
        OperationKind::StoreStructuralField { value, .. } => vec![value],
        OperationKind::EstablishStructuralArray { elements } => elements.iter().collect(),
        _ => kind.structural_arguments().iter().collect(),
    }
}

pub(super) fn terminal_arguments(terminator: &Terminator) -> &[StructuralArgument] {
    match terminator {
        Terminator::InvokeNoNormal {
            structural_arguments,
            ..
        }
        | Terminator::ExternalCompletion {
            structural_arguments,
            ..
        }
        | Terminator::Jump {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => &[],
    }
}

pub(super) fn terminal_reads(terminator: &Terminator) -> Vec<PlaceId> {
    let mut reads = terminal_arguments(terminator)
        .iter()
        .map(|argument| argument.place)
        .collect::<Vec<_>>();
    for arguments in terminator.successor_block_arguments() {
        reads.extend(arguments.iter().map(|argument| argument.place));
    }
    match terminator {
        Terminator::ReturnStructural { source, .. } | Terminator::StructuralCase { source, .. } => {
            reads.push(*source)
        }
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            reads.extend(
                when_true
                    .structural_arguments
                    .iter()
                    .chain(&when_false.structural_arguments)
                    .map(|argument| argument.place),
            );
        }
        _ => {}
    }
    reads
}

pub(super) fn cleanup_mut(
    terminator: &mut Terminator,
    mut visit: impl FnMut(
        EdgeId,
        &mut Vec<TerminalAffineCleanupAction>,
    ) -> Result<(), PlainAffineFailure>,
) -> Result<(), PlainAffineFailure> {
    match terminator {
        Terminator::Jump {
            edge,
            cleanup_actions,
            ..
        }
        | Terminator::Return {
            edge,
            cleanup_actions,
            ..
        }
        | Terminator::ReturnUnit {
            edge,
            cleanup_actions,
        }
        | Terminator::ReturnStructural {
            edge,
            cleanup_actions,
            ..
        } => visit(*edge, cleanup_actions),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            visit(when_true.edge, &mut when_true.cleanup_actions)?;
            visit(when_false.edge, &mut when_false.cleanup_actions)
        }
        Terminator::StructuralCase { cases, .. } => {
            for edge in cases {
                visit(edge.edge, &mut edge.cleanup_actions)?;
            }
            Ok(())
        }
        Terminator::InvokeNoNormal { .. }
        | Terminator::ExternalCompletion { .. }
        | Terminator::Crash { .. } => Ok(()),
    }
}
