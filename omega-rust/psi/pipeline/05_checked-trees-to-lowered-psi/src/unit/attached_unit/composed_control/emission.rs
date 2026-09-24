//! Operation emission for one composed-graph state or call leaf.
//!
//! `emit_call_operations` walks a state's checked operations in order and
//! emits every one through `attached_unit::operation_frame::OperationFrame`,
//! the per-operation emitter the ordinary machine uses. This file owns only
//! what is composed-specific: the frame over the state's environment (its
//! registry-backed result namespace, the machine-wide claim table and the
//! shared temporary roster) and the state's own operand schedule, which
//! evaluates each call's scalar operands and literals just before the call
//! (`literal_arguments`).
use super::super::super::{
    BlockId, ClaimId, LoweredSourceCallOccurrence, PermissionClaimIdentity,
    StructuralParameterDeclaration,
};
use super::super::bodies::UnitPlans;
use super::super::operation_frame::{
    CallInputs, Callees, CallerCustody, ClaimBindings, OperationFrame, PrivatePlaces,
    StructuralResults, StructuralTypeRoster,
};
use super::super::{
    Block, CheckedUnitEffectOperationPlan, Terminator, ValueDeclaration, allocate_dense, edge_id,
    unsupported,
};
use super::{CheckedTrees, LoweringError, catalogs, literal_arguments};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations;
use std::borrow::Cow;

pub(crate) fn emit_call_leaf(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    scalar_parameters: &[ValueDeclaration],
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Vec<Block>, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    if !state.erased_scalar_parameters.is_empty() {
        return unsupported(
            "composed call leaves with erased formals require erased edge operands",
        );
    }
    // A retained `requires` row belongs on a state-graph header invariant;
    // call leaves have no header lane to publish it through.
    if state.requires.iter().any(Option::is_some) {
        return unsupported("composed call leaf cannot publish its retained requires");
    }
    let mut operations = OperationBuffer::new(*next_operation - 1);
    let mut evaluation = super::super::argument_evaluation::Evaluation {
        structural_value_owners: Vec::new(),
        selection_cleanups: Vec::new(),
        structural_locals: Vec::new(),
        view_locals: Vec::new(),
        element_views: std::collections::BTreeMap::new(),
        local_cases: Vec::new(),
        record_fields: crate::scalar_graph::scalar_computations::fields::prepare(
            checked,
            machine,
            &catalogs.structural_types,
        )?,
        arrays: crate::scalar_graph::scalar_computations::arrays::prepare(
            checked,
            machine,
            &catalogs.structural_types,
            &mut catalogs.next_place,
        )?,
        cases: crate::scalar_graph::scalar_computations::cases::prepare(
            checked,
            machine,
            &catalogs.structural_types,
            &mut catalogs.next_place,
        )?,
        primitive_storage: Vec::new(),
        scalar_bindings: None,
        structural_fields: Vec::new(),
        structural_cases: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: erased_parameters.to_vec(),
        erased_proof_formals: state.erased_proof_parameters.clone(),
        entry: block,
        current: block,
        parameters: scalar_parameters.to_vec(),
        block_structural_parameters: Vec::new(),
        operation_start: 0,
        blocks: Vec::new(),
    };
    let mut values = scalar_parameters.to_vec();
    let result_start = catalogs.result_places.len();
    emit_call_operations(
        checked,
        machine,
        state,
        &state.operations,
        catalogs,
        parameters,
        claim_bindings,
        &mut evaluation,
        &mut values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        &mut operations,
    )?;
    *next_operation = operations.next_identity;
    let discards = evaluation.selection_return_discards(
        catalogs.result_places[result_start..]
            .iter()
            .rev()
            .map(|declaration| {
                let discard = operations
                    .structural_values
                    .iter()
                    .find(|(_, result)| result.place == declaration.id)
                    .and_then(|(ordinal, _)| {
                        state
                            .operations
                            .iter()
                            .find_map(|operation| match operation {
                                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                    result,
                                    discard_result_on_return,
                                    ..
                                } if result.binding_ordinal == *ordinal => {
                                    Some(*discard_result_on_return)
                                }
                                _ => None,
                            })
                    })
                    .unwrap_or(true);
                (declaration.id, discard)
            })
            .collect(),
    )?;
    evaluation.remap_transported_call_operands(&mut operations);
    evaluation.blocks.push(Block {
        structural_parameters: evaluation.block_structural_parameters,
        id: evaluation.current,
        parameters: evaluation.parameters,
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: erased_proof_formal_declarations(&state.erased_proof_parameters),
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::ReturnUnit {
            edge: edge_id(allocate_dense(next_edge)?),
            trivial_affine_discards: discards,
        },
    });
    Ok((evaluation.blocks, operations.source_calls))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_call_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    planned_operations: &[CheckedUnitEffectOperationPlan],
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut super::super::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    // The emitting machine's erased roster, for `ErasedParameter` actuals
    // forwarded through proof-only call operands.
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    // Borrowed-storage windows open and close within one straight-line
    // operation sequence. Every exit of the sequence's block — the state's
    // terminator, its successor edges and any return — follows the last
    // operation, so requiring the ledger closed here closes it on every exit.
    // No state-graph join can therefore receive an open window, and the
    // blocks argument evaluation splits off in between rejoin with the one
    // frontier this sequence carries.
    let mut windows = crate::emission::borrowed_window::BorrowedWindowLedger::default();
    let plans = UnitPlans::published(&checked.facts.flow.terminal_unit_effects);
    // A returned final expression evaluates in the `Return` role.
    let scalar_result = match &state.terminator {
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
            completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
        } => Some(binding),
        _ => None,
    };
    for operation in planned_operations {
        // A call's operands complete just before it, on this state's own
        // schedule; every other operation reads its operands itself.
        let call_operands = if OperationFrame::lowers(operation) {
            None
        } else {
            Some(literal_arguments::evaluate(
                checked,
                machine,
                state.state,
                operation,
                catalogs,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?)
        };
        let mut calls = catalogs.scalar_calls.emission_context();
        let frame = OperationFrame {
            checked,
            machine,
            state: state.state,
            scalar_result,
            scalar_parameter_count: state.scalar_parameters.len(),
            source_value_count: values.len(),
            parameters,
            structural_types: match &mut catalogs.structural_types {
                Cow::Owned(types) => StructuralTypeRoster::Owned(types),
                Cow::Borrowed(types) => StructuralTypeRoster::Published(types),
            },
            type_ids: &catalogs.type_ids,
            service_ids: &catalogs.service_ids,
            primitive_locals: &[],
            results: StructuralResults::StateGraph {
                state,
                places: &mut catalogs.result_places,
            },
            // A composed body keeps literals, construction and join places
            // in its one private temporary roster.
            private_places: PrivatePlaces::Shared(&mut catalogs.temporary_places),
            windows: &mut windows,
            evaluation,
            values,
            next_place: &mut catalogs.next_place,
            next_value,
            next_block,
            next_edge,
            calls: &mut calls,
            operations,
            callees: Callees {
                plans,
                signatures: &catalogs.signatures,
                boundaries: &catalogs.boundary_parameters,
                domain_ids: &catalogs.domain_ids,
                closure: catalogs.closure,
                prepared_scalar_machines: catalogs.prepared_scalar_machines,
            },
            caller: CallerCustody {
                erased_scalar_parameters: erased_parameters,
                erased_proof_parameters: &state.erased_proof_parameters,
                entry_claims: &state.entry_claims,
                claims: ClaimBindings::Fixed(claim_bindings),
                local_places: &[],
            },
        };
        match &call_operands {
            None => frame.emit(operation)?,
            Some((arguments, byte_places)) => {
                if frame
                    .emit_call(
                        operation,
                        CallInputs {
                            evaluated_scalar_arguments: arguments.as_deref(),
                            byte_places,
                            staged: false,
                        },
                    )?
                    .is_some()
                {
                    return unsupported("composed call staged a private scalar result");
                }
            }
        }
        catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
    }
    windows.require_closed()
}
