//! Reserve fresh storage for whole unrestricted call operands. Reservation is
//! not a read: the ordered operand stream establishes each snapshot where its
//! authored argument is evaluated, before later operands can change the source.
//! Source custody still independently validates the exact owned argument.

use super::{CheckedTrees, LoweringError, unsupported};
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
            ref callee,
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
                if argument.access == CheckedStructuralAccess::Owned
                    && !matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }))
        }) {
            continue;
        }
        let Some((target_machine, target_state)) = callee.static_target() else {
            return unsupported("dynamic scalar call awaits its emission route");
        };
        let parameters =
            super::call_signature::structural_parameters(checked, target_machine, target_state)?;
        if parameters.len() != arguments.len() {
            return unsupported("computed copy disagrees with its selected callee signature");
        }
        for (argument_ordinal, (argument, parameter)) in
            arguments.iter().zip(parameters).enumerate()
        {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            if argument.access != CheckedStructuralAccess::Owned
                || matches!(
                    argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
                )
            {
                continue;
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
