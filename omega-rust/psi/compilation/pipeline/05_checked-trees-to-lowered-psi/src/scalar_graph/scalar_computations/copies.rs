//! Reserve fresh storage for whole unrestricted call operands. Reservation is
//! not a read: the ordered operand stream establishes each snapshot where its
//! authored argument is evaluated, before later operands can change the source.
//! Source custody still independently validates the exact owned argument.

use super::{CheckedTrees, LoweringError, unsupported};
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedStructuralAccess,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::StructuralTypeDeclaration;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(crate) source_call: arena::Handle<checked_trees::FlowCallFact>,
    pub(crate) argument_ordinal: usize,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let CheckedScalarComputationKind::Call {
            source_call,
            target_machine,
            target_state,
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        let arguments = plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed copy has stale structural operands",
            ))?;
        if !arguments.iter().any(|argument| {
            matches!(argument, CheckedScalarComputationStructuralArgument::Place(argument)
                if argument.access == CheckedStructuralAccess::Owned)
        }) {
            continue;
        }
        let callee = CheckedScalarCallee::find_for_unit_call(checked, target_machine)?;
        if callee.entry_state()? != target_state
            || callee.structural_parameters().len() != arguments.len()
        {
            return unsupported("computed copy disagrees with its selected callee signature");
        }
        for (argument_ordinal, (argument, parameter)) in arguments
            .iter()
            .zip(callee.structural_parameters())
            .enumerate()
        {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            if argument.access != CheckedStructuralAccess::Owned {
                continue;
            }
            if parameter.access != CheckedStructuralAccess::Owned
                || argument.type_identity != parameter.type_identity
                || !argument.path.is_empty()
            {
                return unsupported("computed copy changed its whole owned formal");
            }
            if parameter.multiplicity != Multiplicity::Unrestricted {
                continue;
            }
            let declaration = types
                .iter()
                .find(|declaration| declaration.identity == argument.type_identity)
                .ok_or(LoweringError::Unsupported(
                    "computed copy has no structural type namespace",
                ))?;
            if let Some(existing) = slots.iter().find(|slot| {
                slot.source_call == source_call && slot.argument_ordinal == argument_ordinal
            }) {
                if existing.structural_type != declaration.id {
                    return unsupported("computed copy occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                source_call,
                argument_ordinal,
                place: super::super::place_id(super::super::allocate_dense(next_place)?),
                structural_type: declaration.id,
            });
        }
    }
    Ok(slots)
}

/// A copy operation declares the fresh value it actually produced. Reservations
/// alone publish no storage; another emitter may already own this same exact
/// result declaration, so callers merge by producer and type, not just place ID.
pub(crate) fn declarations(
    operations: &[terminal_psi::Operation],
) -> impl Iterator<Item = terminal_psi::StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(declaration)
}

fn declaration(
    operation: &terminal_psi::Operation,
) -> Option<terminal_psi::StructuralPlaceDeclaration> {
    if !matches!(
        operation.kind,
        terminal_psi::OperationKind::StructuralLeafCopy { .. }
    ) {
        return None;
    }
    let terminal_psi::OperationResult::Structural(result) = &operation.result else {
        return None;
    };
    Some(terminal_psi::StructuralPlaceDeclaration {
        id: result.place,
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation.id,
            structural_type: result.structural_type,
        },
    })
}

/// Unit emitters keep a vector namespace; preserve any independently published
/// declaration only when it names this same copy result and producer.
pub(crate) fn retain_declarations<'a>(
    operations: impl Iterator<Item = &'a terminal_psi::Operation>,
    declarations: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
) -> Result<(), LoweringError> {
    for declaration in operations.filter_map(declaration) {
        if let Some(existing) = declarations
            .iter()
            .find(|existing| existing.id == declaration.id)
        {
            if *existing != declaration {
                return unsupported("computed copy conflicts with an existing place declaration");
            }
        } else {
            declarations.push(declaration);
        }
    }
    Ok(())
}
