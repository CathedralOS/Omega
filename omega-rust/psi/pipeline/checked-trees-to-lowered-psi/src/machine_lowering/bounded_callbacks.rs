//! Bounded callback-body lowering for isolated entrance publication.
//!
//! One exact callback-body cohort lowers without treating a nominally
//! attached static machine as an attached Unit program root: either one
//! checked `u64 -> u64` scalar graph with an identity return, or one checked
//! `u64 -> Unit` complete-only Unit body, each over a single selected entry
//! with no local operations. The callback placement separately owns its
//! satisfaction and ABI evidence; this producer owns only executable body
//! semantics and the checked-to-Terminal coordinate join. Admission rejects
//! any checked shape outside the cohort; contract, proof, and operand
//! obligations reuse the ordinary production work.

use checked_trees::types::PrimitiveType;
use checked_trees::{CheckedScalarStateTerminator, CheckedTrees, CheckedUnitEffectOperationPlan};
use lowered_psi::{CallbackTerminalLoweringReceipt, LoweredCallbackPsi, LoweredPsi};

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::reject_conditional_claim_joins;
use crate::proofs::mathematical_declarations::admit_mathematical_declarations;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::retention::{closed_reach_applications, operation_crash_contracts};
use crate::scalar_graph::scalar_graph_lowering::lower_selected_scalar_graph_machine;
use crate::unit::attached_unit;

/// Lower one exact callback-body cohort without treating a nominally
/// attached static machine as an attached Unit program root.
///
/// This entry point is deliberately narrower than general machine lowering:
/// either one checked `u64 -> u64` scalar graph with an identity return, or
/// one checked `u64 -> Unit` complete-only Unit body, each over a single
/// selected entry with no local operations. The callback placement separately
/// owns its satisfaction and ABI evidence; this producer owns only executable
/// body semantics and the checked-to-Terminal coordinate join.
pub fn lower_bounded_callback_identity_machine(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    source_entry: symbols::SymbolHandle,
) -> Result<LoweredCallbackPsi, LoweringError> {
    admit_mathematical_declarations(checked)?;
    reject_conditional_claim_joins(checked, &[source_machine])?;
    let matching_selection_count = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == source_machine)
        .count();
    if matching_selection_count != 1 {
        return unsupported(
            "bounded callback body must name one exact checked Terminal machine selection",
        );
    }
    let mut lowered = if let Some(graph) = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(source_machine)
    {
        let [state] = graph.states.as_slice() else {
            return unsupported("bounded callback body must contain exactly one checked state");
        };
        if state.state != source_entry
            || state.parameter_types.as_slice() != [PrimitiveType::U64]
            || state.result_type != PrimitiveType::U64
            || !state.bindings.is_empty()
            || !matches!(
                state.terminator,
                CheckedScalarStateTerminator::Return {
                    statement_ordinal: 0
                }
            )
        {
            return unsupported(
                "bounded callback body must be the exact one-state u64 identity-return cohort",
            );
        }
        lower_selected_scalar_graph_machine(checked, source_machine, graph)?
    } else {
        lower_bounded_callback_unit_body(checked, source_machine, source_entry)?
    };
    // Isolated callback production is another public lowering entrance, not
    // permission to omit the selected generic body's application custody.
    let specialization_instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == source_machine)
        .map(|specialization| specialization.instance)
        .collect::<Vec<_>>();
    validation::validate_checked_machine_specialization_commitments(
        checked,
        &specialization_instances,
    )
    .map_err(LoweringError::Unsupported)?;
    // Callback isolation changes the publication root, not the selected body's
    // contract custody. Reuse the ordinary projection with this exact owner;
    // unused selections need not invent executable callback bodies.
    closed_reach_applications::retain_closed_reach_applications(
        checked,
        &[(source_machine, lowered.semantic_module.entry)],
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    operation_crash_contracts::retain_operation_crash_contracts(
        checked,
        &[source_machine],
        &mut lowered,
    )?;
    terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        return unsupported("bounded callback lowering did not produce one Terminal machine");
    };
    Ok(LoweredCallbackPsi {
        receipt: CallbackTerminalLoweringReceipt {
            source_machine,
            source_entry,
            terminal_machine: machine.id,
            terminal_entry: machine.entry,
        },
        terminal: lowered,
    })
}

/// Lower the void callback cohort: the exact checked Unit plan for one
/// attached body whose only operation completes without producing a value.
///
/// Void bodies carry no scalar graph; their checked custody lives in the
/// ordinary Unit-effect plan instead. Reusing `lower_unit_effect_closure`
/// keeps the same contract, proof, and operand obligations as publication —
/// this entrance only narrows which checked shape may produce a thunk body.
/// The returned module must still close over exactly the selected machine.
fn lower_bounded_callback_unit_body(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    source_entry: symbols::SymbolHandle,
) -> Result<LoweredPsi, LoweringError> {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "bounded callback body has neither a checked scalar graph nor a checked unit plan",
        ))?;
    let is_exact_unit_leaf = plan.state == source_entry
        && plan.scalar_result.is_none()
        && plan.scalar_control.is_none()
        && plan.structural_result.is_none()
        && plan.structural_parameters.is_empty()
        && matches!(
            plan.scalar_parameters.as_slice(),
            [parameter]
                if parameter.source_position == 0
                    && parameter.primitive_type == PrimitiveType::U64
        )
        && plan.provider_attachment_requirements.is_empty()
        && plan.trivial_affine_locals.is_empty()
        && plan.entry_claims.is_empty()
        && plan.body_qualifications.is_empty()
        && matches!(
            plan.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::Complete {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            }] if trivial_affine_local_discard_ordinals.is_empty()
                && trivial_affine_discards.is_empty()
        );
    if !is_exact_unit_leaf {
        return unsupported(
            "bounded callback unit body must be the exact one-state u64 complete-only cohort",
        );
    }
    let lowered = attached_unit::lower_unit_effect_closure(checked, source_machine)?;
    let [(owner, _)] = lowered.source_machine_ids.as_slice() else {
        return unsupported(
            "bounded callback unit body must close over exactly its selected machine",
        );
    };
    if *owner != source_machine {
        return unsupported("bounded callback unit body lost its exact source owner");
    }
    let mut terminal = lowered.terminal;
    finalize_operation_proofs(&mut terminal)?;
    Ok(terminal)
}
