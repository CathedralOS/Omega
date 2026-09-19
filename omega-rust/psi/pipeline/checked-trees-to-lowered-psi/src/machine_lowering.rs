//! Machine lowering: the selected checked machine to unsealed, target-neutral Psi.
//!
//! [`lower_machine`] and [`lower_machine_by_symbol`] select one checked Terminal
//! machine, dispatch it through [`crate::machine_lowering::machine_dispatch`] to the plan family
//! that owns its shape, then sequence the work every selected module still
//! needs: retained custody, float-meaning projections, evidence and proof
//! recursion, operand proof completion or module validation, and the debug
//! companion. [`lower_bounded_callback_identity_machine`] is the deliberately
//! narrower callback-body entrance. Unsupported source constructs fail closed.

pub(crate) mod debug_map;
pub(crate) mod machine_dispatch;

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedScalarStateTerminator, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitEffectOperationPlan,
};
use lowered_psi::{CallbackTerminalLoweringReceipt, LoweredCallbackPsi, LoweredPsi};

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::machine_dispatch::{
    lower_selected_machine, select_terminal_machine, select_terminal_machine_by_symbol,
};
use crate::producer_result::{
    ConformancePublication, DebugPublication, LoweredSelectedMachine, OperandProofCompletion,
};
use crate::proofs::evidence_lowering::lower_and_install_evidence_artifacts;
use crate::proofs::float_meaning_projection::{
    lower_float_meaning_equality, lower_float_meaning_projection,
    resolve_direct_float_source_binding,
};
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::proofs::proof_recursion::lower_and_install_proof_recursion;
use crate::retention::conformance_applications::lower_closed_conformance_applications;
use crate::retention::placed_view_inputs::retain_selected_placed_view_inputs;
use crate::retention::{
    closed_reach_applications, conformance_applications, operation_crash_contracts,
    reborrow_restored_call_use, reborrow_root_handoff, retained_borrow_custody,
    suspension_call_plan,
};
use crate::scalar_graph::scalar_graph_lowering::lower_selected_scalar_graph_machine;
use crate::unit::attached_unit;

/// Lower a selected checked machine and its required source closure.
/// Preserve source custody, install evidence, validate the completed module,
/// and attach debug companions before returning unsealed Psi.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredPsi, LoweringError> {
    let selection = select_terminal_machine(checked, machine_name)?;
    lower_terminal_selection(checked, selection)
}

/// Lower the machine whose exact checked symbol was selected upstream.
///
/// Build product operands resolve their implementation lexically and retain
/// the exact machine symbol, so production must rejoin that symbol rather than
/// a qualified name another package could also declare. This entry point
/// shares every post-selection obligation with [`lower_machine`]; only the
/// lookup key differs.
pub fn lower_machine_by_symbol(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<LoweredPsi, LoweringError> {
    let selection = select_terminal_machine_by_symbol(checked, machine)?;
    lower_terminal_selection(checked, selection)
}

/// The checked `let`/`boundary let` mathematical declarations carry no
/// Terminal Psi evidence encoding yet, so no production route may emit a
/// module that omits them: every public lowering entrance refuses the roster
/// loudly until PROOF-CONTRACT-MIGRATION consumes it downstream.
fn reject_mathematical_declarations(checked: &CheckedTrees) -> Result<(), LoweringError> {
    if checked.facts.proof.mathematical_declarations.is_empty() {
        return Ok(());
    }
    unsupported(
        "mathematical `let`/`boundary let` declarations check onto checked trees, but \
         no Terminal evidence encoding carries them yet (PROOF-CONTRACT-MIGRATION)",
    )
}

/// Source custody may select different claim lineages at different normal
/// exits. Terminal's current structural return transfers carry one lineage,
/// not that checked choice. Refuse only demanded owners: an unused source
/// allocator must not prevent publishing an unrelated machine.
fn reject_conditional_claim_joins(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    if checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .any(|(_, event)| {
            source_machines.contains(&event.machine_symbol)
                && matches!(
                    event.provenance,
                    language_semantics::PermissionProvenance::Joined { .. }
                )
        })
        || checked
            .facts
            .flow
            .ownership
            .claim_join_receipts
            .iter()
            .any(|(_, receipt)| source_machines.contains(&receipt.machine_symbol))
    {
        return unsupported(
            "conditional result custody requires Terminal exit-alternative correspondence",
        );
    }
    Ok(())
}

fn lower_terminal_selection(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
) -> Result<LoweredPsi, LoweringError> {
    reject_mathematical_declarations(checked)?;
    reject_conditional_claim_joins(checked, &[selection.machine])?;
    operation_crash_contracts::reject_unjoinable_named_sites(checked, selection.machine)?;
    attached_unit::validate_direct_unit_parameter_custody(checked)?;
    let exact_guarded_payloadless = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(selection.machine)
        .is_some();
    let guarded_payloadless_callee = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selection.machine)
        .map(|plan| plan.target_machine);
    if !exact_guarded_payloadless
        && checked
            .facts
            .proof
            .outcome_specific_guarantees
            .iter()
            .any(|(_, guarantee)| guarantee.machine_symbol == selection.machine)
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    let LoweredSelectedMachine {
        terminal: mut lowered,
        completion,
        source_machines,
        source_mapping,
    } = lower_selected_machine(checked, selection)?;
    reject_conditional_claim_joins(checked, &source_machines)?;
    let has_exact_source_owners = source_mapping.exact_owners().is_some();
    // Reborrow custody belongs to every included source body, not only the
    // requested entry. Use the lowering route's exact source mapping; ordinal
    // correspondence is not evidence of a callee's identity.
    let entry_source = [(selection.machine, lowered.semantic_module.entry)];
    let handoff_sources = source_mapping.exact_owners().unwrap_or(&entry_source);
    for source in &source_machines {
        if checked
            .facts
            .borrow
            .reborrow_loan_resources
            .iter()
            .any(|(_, resource)| resource.machine_symbol == *source)
            && handoff_sources
                .iter()
                .filter(|(owner, _)| owner == source)
                .count()
                != 1
        {
            return unsupported("reborrow call closure has no exact source owner mapping");
        }
    }
    for (source, terminal) in handoff_sources {
        if lowered
            .semantic_module
            .machines
            .iter()
            .filter(|machine| machine.id == *terminal)
            .count()
            != 1
        {
            return unsupported("reborrow source owner has no unique Terminal machine");
        }
        reborrow_root_handoff::retain_selected_reborrow_root_handoffs(
            checked,
            *source,
            *terminal,
            &mut lowered.semantic_module.reborrow_root_handoffs,
        )?;
    }
    let direct_float_source_machines =
        source_mapping.projection_sources(&lowered, selection.machine, &source_machines)?;
    // Specialization custody applies to ordinary calls as well as calls that
    // produce proof evidence. Reuse exact source owners and call occurrences;
    // display names and matching callback signatures cannot select a body.
    // Replay once per selected batch before source companions are discarded.
    let specialization_instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            source_machines.contains(&specialization.instance)
                || checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.instance
                        && checked.machine_states(machine).iter().any(|state| {
                            lowered.source_call_occurrences.iter().any(|call| {
                                call.source_state == state.symbol
                                    || call.source_target == state.symbol
                            })
                        })
                })
        })
        .map(|specialization| specialization.instance)
        .collect::<Vec<_>>();
    validation::validate_checked_machine_specialization_commitments(
        checked,
        &specialization_instances,
    )
    .map_err(LoweringError::Unsupported)?;
    closed_reach_applications::retain_closed_reach_applications(
        checked,
        &direct_float_source_machines,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retain_selected_placed_view_inputs(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &mut lowered.semantic_module,
    )?;
    reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &lowered.source_call_occurrences,
        &lowered.semantic_module.machines,
        &mut lowered.semantic_module.reborrow_restored_call_uses,
    )?;
    suspension_call_plan::retain_suspension_call_plans(
        checked,
        &source_machines,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retained_borrow_custody::retain_foreign_borrow_custodies(
        checked,
        &mut lowered.semantic_module,
    )?;
    // Selected operator invocations carry their crash contract beside the
    // exact emitted operation; a crash-qualified use the closure cannot join
    // to one operation fails closed here rather than lowering crash-free.
    operation_crash_contracts::retain_operation_crash_contracts(
        checked,
        &source_machines,
        &mut lowered,
    )?;
    if checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .any(|(_, guarantee)| {
            source_machines.contains(&guarantee.machine_symbol)
                && !((exact_guarded_payloadless
                    && source_machines.as_slice() == [selection.machine]
                    && guarantee.machine_symbol == selection.machine)
                    || guarded_payloadless_callee == Some(guarantee.machine_symbol))
        })
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    let dynamic_root_application_count = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            !has_exact_source_owners || application.owner == lowered.semantic_module.entry
        })
        .count();
    match completion.conformances {
        ConformancePublication::ExactRoot if dynamic_root_application_count != 1 => {
            return unsupported(
                "direct dynamic dispatch must publish exactly one closed conformance application",
            );
        }
        ConformancePublication::BoundedRoot
            if !(1..=2).contains(&dynamic_root_application_count) =>
        {
            return unsupported(
                "rebound dynamic dispatch must publish one or two closed conformance applications",
            );
        }
        ConformancePublication::BoundedModule
            if !(1..=2).contains(
                &lowered
                    .semantic_module
                    .closed_conformance_applications
                    .len(),
            ) =>
        {
            return unsupported(
                "joined dynamic dispatch must publish one or two closed conformance applications",
            );
        }
        ConformancePublication::Reconstruct => {
            lower_closed_conformance_applications(
                checked,
                &source_machines,
                &mut lowered.semantic_module,
            )?;
        }
        _ => {}
    }
    if has_exact_source_owners
        && matches!(
            completion.conformances,
            ConformancePublication::ExactRoot | ConformancePublication::BoundedRoot
        )
    {
        conformance_applications::append_closed_conformance_applications_excluding(
            checked,
            &direct_float_source_machines,
            selection.machine,
            &mut lowered.semantic_module,
        )?;
    }
    lowered.semantic_module.float_meaning_projections = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .cloned()
        .map(|projection| {
            let direct_source = resolve_direct_float_source_binding(
                checked,
                &direct_float_source_machines,
                &lowered.semantic_module.machines,
                &lowered.semantic_module.structural_types,
                &lowered.source_call_occurrences,
                &lowered.selected_ieee_float_fma_occurrences,
                projection.clone(),
            )?;
            lower_float_meaning_projection(projection, direct_source)
                .map_err(LoweringError::InvalidFloatMeaningProjection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Resolved direct sources never emit their checked transitional fallback,
    // so surviving fallback identities renumber densely in emission order.
    let mut transitional_sources = Vec::<u32>::new();
    for projection in &mut lowered.semantic_module.float_meaning_projections {
        if let terminal_psi::FloatMeaningSource::TransitionalInput(input) = &mut projection.source {
            let next = match transitional_sources.iter().position(|id| *id == input.id.0) {
                Some(index) => index,
                None => {
                    transitional_sources.push(input.id.0);
                    transitional_sources.len() - 1
                }
            };
            input.id = terminal_psi::FloatProjectionInputId(u32::try_from(next).map_err(|_| {
                LoweringError::Unsupported(
                    "float-meaning transitional sources exceed their dense identity space",
                )
            })?);
        }
    }
    lowered.semantic_module.float_meaning_equalities = checked
        .facts
        .proof
        .float_meaning_equalities
        .iter()
        .copied()
        .map(lower_float_meaning_equality)
        .collect();
    lower_and_install_evidence_artifacts(checked, selection.machine, &mut lowered)?;
    lower_and_install_proof_recursion(
        checked,
        &source_machines,
        &mut lowered.semantic_module,
        &mut lowered.proof_bundle,
    )?;
    // Unit closures can be provisional inputs to cleanup/borrow assembly.
    // Discharge operand obligations only after the selected module is complete.
    if completion.operands == OperandProofCompletion::Finalize {
        finalize_operation_proofs(&mut lowered)?;
    } else if lowered
        .semantic_module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    } else {
        terminal_verifier::validate_module(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    }
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && completion.debug == DebugPublication::FromCheckedPlan
    {
        checked
            .facts
            .flow
            .terminal_debug
            .for_machine(selection.machine)
            .map(|plan| build_debug_map(plan, &lowered.semantic_module))
            .transpose()?
    } else {
        None
    };
    Ok(lowered)
}

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
    reject_mathematical_declarations(checked)?;
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
