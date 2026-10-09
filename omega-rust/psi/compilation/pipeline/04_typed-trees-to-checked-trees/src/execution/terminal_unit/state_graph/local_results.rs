//! Selected successors partition existing local obligations, not value storage.

use super::super::{CheckedUnitStructuralResultBindingPlan, SymbolHandle};
use super::{CheckedStructuralControlSuccessorPlan, Multiplicity};

/// A state-exit receipt permits disposal only on successors that do not move
/// this local. Keep the operation's global discard flag false: scalar edge
/// operands still observe its original home before selected-edge cleanup.
pub(super) fn permits_disposal(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    successors: &[&CheckedStructuralControlSuccessorPlan],
    disposable_locals: &[SymbolHandle],
) -> bool {
    validate(program, state, result, successors, disposable_locals).is_some()
}

fn validate(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    successors: &[&CheckedStructuralControlSuccessorPlan],
    disposable_locals: &[SymbolHandle],
) -> Option<()> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let local = statements
        .get(result.statement_index as usize)?
        .as_local_data()?;
    // Disposal rejoins the same stored carrier identity that establishment
    // and shape collection publish. Policy and predicate qualifications keep
    // their own checked obligations; they cannot change the storage identity
    // used to select a local's disposition. The shared owner also preserves
    // bounded byte owners, borrowed views, and stored reference descriptors.
    let expected_identity =
        validation::stored_structural_type_identity(program, local.type_reference, &[], &[]);
    // A reference-loaned result publishes the storage it lends rather than a
    // stored descriptor. This is a genuine carrier distinction; use the same
    // shell and referent identity owners as its establishment producer.
    let referent_identity = validation::reference_shell(program, local.type_reference)
        .map(|shell| validation::structural_type_identity(program, shell.referee, &[], &[]));
    // An affine reference/view binding is accounted for by its loan release,
    // not by owned-local disposal. Counting both would invent a second owner.
    // Unrestricted aliases still need the ordinary dominance permit below.
    if referent_identity.is_some() && result.multiplicity != Multiplicity::Unrestricted {
        return None;
    }
    let source_multiplicity = program.type_multiplicity(local.type_reference);
    let carrier_multiplicity =
        validation::reference_result_custody::binding_multiplicity(program, local.type_reference);
    if !local.symbol.is_valid()
        || program
            .state_local_declaration(state, local.symbol)
            .is_none()
        || program
            .state_parameter_by_symbol(state, local.symbol)
            .is_some()
        || !(result.type_identity == expected_identity
            || referent_identity.as_ref() == Some(&result.type_identity))
        || result.multiplicity != carrier_multiplicity
    {
        return None;
    }
    // A normal return has no successor transfer, whatever its result kind. It
    // still needs the same exact lexical disposal receipt as a
    // non-transferring selected edge.
    let mut has_disposal_edge = successors.is_empty();
    for successor in successors {
        if result.statement_index >= successor.statement_ordinal {
            return None;
        }
        // An edge whose operand call takes the local whole moves it there
        // just as a direct transfer does; only a non-moving edge disposes.
        match successor.result_moves(result) {
            0 => has_disposal_edge = true,
            1 => {}
            _ => return None,
        }
    }
    if !has_disposal_edge {
        return None;
    }
    // Source-unrestricted values need dominance, not an owned-disposal
    // receipt. Their materialized reference carriers can still be affine:
    // lowering ends each retained loan when this binding is discarded.
    if source_multiplicity == Multiplicity::Unrestricted {
        return Some(());
    }
    // Permission eligibility has one producer owner. Here it is rejoined to
    // this operation's result identity and selected successor partition. The
    // normal-edge planner subsequently selects no-code or executable disposal;
    // nominal hooks must not be mistaken for missing ownership receipts here.
    // The result already identifies its established structural storage. A
    // whole-type content gate would reject a valid residual disposition after
    // contained claims leave; selected normal-edge actions own that geometry.
    (result.multiplicity == Multiplicity::Affine && disposable_locals.contains(&local.symbol))
        .then_some(())
}
