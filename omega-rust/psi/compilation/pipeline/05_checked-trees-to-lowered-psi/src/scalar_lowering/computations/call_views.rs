//! A call that returns a byte carrier is observed only after that call
//! completes. Reserve the result place while the comparison expression is
//! still being built, then emit the call into that same place.
use super::{CheckedTrees, LoweringError};
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedScalarComputationKind, CheckedUnitCallCoordinate, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::StructuralTypeDeclaration;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(crate) expression: ExpressionHandle,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

pub(crate) fn expression(argument: &CheckedUnitStructuralArgumentPlan) -> Option<ExpressionHandle> {
    match argument.source {
        Source::StructuralExpression { expression } => Some(expression),
        _ => None,
    }
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut slots: Vec<Slot> = Vec::new();
    for root in plans.machine_roots(machines) {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let subject = match &plans.nodes.get(handle).kind {
                CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. }
                | CheckedScalarComputationKind::ByteSequencePredicate { subject, .. } => subject,
                _ => continue,
            };
            let Some(expression) = expression(subject) else {
                continue;
            };
            let root = checked
                .facts
                .values
                .structural_values
                .root_for_expression(root.state, root.statement_ordinal, expression)
                .ok_or(LoweringError::Unsupported(
                    "call byte view lost its checked result",
                ))?;
            let identity = validation::stored_structural_type_identity(
                &checked.typed,
                root.type_reference,
                &[],
                &[],
            );
            let declaration = crate::terminal_identities::structural_declaration_by_identity(
                types,
                identities,
                checked,
                identity,
                "call byte view has no structural type namespace",
            )?;
            if let Some(existing) = slots.iter().find(|slot| slot.expression == expression) {
                if existing.structural_type != declaration.id {
                    return super::unsupported("call byte view occurrence has conflicting types");
                }
                continue;
            }
            slots.push(Slot {
                expression,
                place: super::super::place_id(super::super::allocate_dense(next_place)?),
                structural_type: declaration.id,
            });
        }
    }
    Ok(slots)
}

/// The place reserved for this call's byte result, when a comparison observes it.
pub(crate) fn reserved_place(
    checked: &CheckedTrees,
    slots: &[Slot],
    state: symbols::SymbolHandle,
    coordinate: &CheckedUnitCallCoordinate,
) -> Option<PlaceId> {
    if slots.is_empty() {
        return None;
    }
    let flow = &checked.facts.flow;
    let state = flow
        .control
        .states
        .iter()
        .find_map(|(_, state_flow)| (state_flow.state_symbol == state).then_some(state_flow))?;
    let expression = state.calls.handles().find_map(|handle| {
        let call = flow.control.calls.get(handle);
        (call.statement_index == coordinate.statement_index as usize
            && call.call_ordinal == coordinate.call_ordinal as usize)
            .then_some(call.authored_expression)
    })?;
    slots
        .iter()
        .find(|slot| slot.expression == expression)
        .map(|slot| slot.place)
}
