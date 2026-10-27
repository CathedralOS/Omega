//! Machine lowering: the selected checked machine to unsealed, target-neutral Psi.
//!
//! [`lower_machine`] selects one checked Terminal machine by
//! [`TerminalMachineSelection`], dispatches it through
//! [`crate::machine_lowering::machine_dispatch`] to the plan family that owns
//! its shape, then closes provider installation entries and sequences the work
//! every selected module still needs: retained custody, float-meaning projections, evidence and proof
//! recursion, checked quotient correspondence bindings, operand proof
//! completion or module validation, and the debug companion.
//! Callback selections join the same program's retained roots. Each body retains its portable
//! checked declaration, so consumers need no source-to-Terminal receipt.
//! State-exit loan closure stays in source checking: it has no executable
//! Terminal operation and needs no serialized lineage certificate. Restored
//! calls below still retain their operation-specific source relationships.
//! Unsupported source constructs fail closed.

mod boundary_applications;
mod build_providers;
mod callback_applications;
pub(crate) mod debug_map;
pub(crate) mod declarations;
pub(crate) mod float_meanings;
pub(crate) mod guarded_exits;
pub(crate) mod machine_dispatch;

use checked_trees::{CheckedTerminalSignatureEligibility, CheckedTrees};
use lowered_psi::LoweredPsi;

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::float_meanings::retain_float_meanings;
use crate::machine_lowering::guarded_exits::reject_unguarded_outcome_guarantees;
use crate::machine_lowering::machine_dispatch::{
    TerminalMachineSelection, lower_selected_machine, select_terminal_machine,
};
use crate::producer_result::{DebugPublication, LoweredSelectedMachine, LoweringCompletion};
use crate::proofs::evidence_lowering::lower_and_install_evidence_artifacts;
use crate::retention::{
    closed_reach_applications, operation_crash_contracts, reborrow_restored_call_use,
    retained_borrow_custody, suspension_call_plan,
};
use crate::unit::attached_unit;

/// Lower the selected checked machine and its required source closure.
/// Preserve source custody, install evidence, validate the completed module,
/// and attach debug companions before returning unsealed Psi.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine: TerminalMachineSelection<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    program_roots: crate::TerminalProgramRoots<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<LoweredPsi, LoweringError> {
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| {
            !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty()
        })
    {
        return unsupported(
            "boundary scalar contract clauses require target closure before Terminal production",
        );
    }
    if checked
        .facts
        .integer_range_obligations
        .iter()
        .any(checked_trees::IntegerRangeObligation::is_pending)
    {
        return unsupported(
            "integer range obligations require target closure before Terminal production",
        );
    }
    // Contract invocation formation was retained with the constant root, even
    // when no call site needed a scalar comparison obligation. Its selected
    // evaluation must finish before source-free publication can erase it.
    let pending_contract_roots =
        checked
            .typed
            .constant_recipes
            .iter()
            .filter_map(|(handle, recipe)| match recipe.position {
                typed_trees::constant_recipes::RetainedConstantPosition::ContractExpression(
                    root,
                ) if checked.typed.closed_constant_recipe_value(handle).is_none() => Some(root),
                _ => None,
            })
            .collect::<Vec<_>>();
    if !pending_contract_roots.is_empty() {
        // Private evaluation projects a demanded declaration roster while
        // preserving backing handles. An unrelated retained recipe is not
        // work owned by that projection; live source clauses remain demands.
        let references = typed_trees::references::collect_program_references(&checked.typed);
        if pending_contract_roots
            .iter()
            .any(|root| references.expressions.contains(root))
        {
            return unsupported(
                "contract integer expressions require target closure before Terminal production",
            );
        }
    }
    // Erased clause roots can have no runtime geometry that would otherwise
    // force closure. Their retained call conditions still need completed
    // evidence; a direct/package producer cannot publish a shared judgment.
    if checked
        .facts
        .constant_recipe_obligations
        .iter()
        .any(checked_trees::ConstantRecipeObligation::is_pending)
    {
        return unsupported(
            "constant recipe obligations require target closure before Terminal production",
        );
    }
    // A shared source judgment may carry target-dependent proof conditions.
    // Only target closure consumes those obligations; direct/package callers
    // cannot publish conditional call evidence as unconditional Terminal code.
    if checked
        .facts
        .constant_predicate_obligations
        .iter()
        .any(checked_trees::ConstantPredicateObligation::is_pending)
    {
        return unsupported(
            "constant proof predicates require target closure before Terminal production",
        );
    }
    // One builder owns the complete closure's identities. Body construction
    // snapshots only a prefix: declaration, evidence and application retention
    // can intern more rows. Seal the complete table after those producers, so
    // every durable index resolves in the module that owns it.
    let mut identities = match publication {
        terminal_codec::TerminalPublication::Portable => {
            terminal_psi::TerminalIdentityTableBuilder::default()
        }
        terminal_codec::TerminalPublication::Invocation => {
            terminal_psi::TerminalIdentityTableBuilder::for_invocation()
        }
    };
    // Nominal callbacks are checked callable roots, even though their foreign
    // invocation is not an ordinary source call edge. Retain them once beside
    // program entries; native ABI adapters select these exact bodies later.
    let mut roots = program_roots.machines.to_vec();
    roots.extend(
        checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .filter(|usage| usage.callback_entry.is_some())
            .map(|usage| usage.selected_machine),
    );
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    let program_roots = crate::TerminalProgramRoots {
        machines: &roots,
        boundaries: program_roots.boundaries,
    };
    let selection = select_terminal_machine(checked, machine)?;
    let (mut lowered, completion) = phase_profile::scope("05.lower_terminal_selection", || {
        lower_terminal_selection(
            &mut identities,
            checked,
            selection,
            exact_toolchain_sources,
            program_roots,
        )
    })
    .map_err(|error| error.with_source_names(checked))?;
    phase_profile::scope("05.boundary_applications", || {
        boundary_applications::retain(
            &mut identities,
            checked,
            exact_toolchain_sources,
            &mut lowered,
        )
    })
    .map_err(LoweringError::Unsupported)?;
    phase_profile::scope("05.callback_applications", || {
        callback_applications::retain(
            &mut identities,
            checked,
            exact_toolchain_sources,
            &mut lowered,
        )
    })
    .map_err(LoweringError::InvalidDeclarationIdentity)?;
    lowered.semantic_module.identities =
        phase_profile::scope("05.identities_build", || identities.build());
    // Declaration and evidence producers intern beyond the body snapshot.
    // Validate against their complete module-owned table before publication.
    if let Err(reason) = phase_profile::scope("05.validate_runtime_evidence_inputs", || {
        terminal_semantics::validate_runtime_evidence_inputs(&lowered.semantic_module)
    }) {
        diagnose_runtime_evidence_case_failure(&lowered.semantic_module, reason);
        return Err(LoweringError::Unsupported(reason));
    }
    // Debug identity binds the whole semantic module, including retained
    // boundary/callback applications and the final interned identity table.
    // Construct it only after those producers finish; stamping the body's
    // earlier table prefix makes even a constant entry's companion stale.
    lowered.debug_map = phase_profile::scope("05.debug_map", || -> Result<_, LoweringError> {
        if selection.signature == CheckedTerminalSignatureEligibility::Eligible
            && completion.debug == DebugPublication::FromCheckedPlan
        {
            let live_states = checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(selection.machine)
                .map(attached_unit::unit_graph_live_states)
                .transpose()?;
            checked
                .facts
                .flow
                .terminal_debug
                .for_machine(selection.machine)
                .map(|plan| build_debug_map(plan, &lowered.semantic_module, live_states.as_deref()))
                .transpose()
        } else {
            Ok(None)
        }
    })?;
    Ok(lowered)
}

/// The exact source declaration designates this callable's cleanup role;
/// merely being attached to the same type or receiving its loan is insufficient.
pub(crate) fn nominal_cleanup_subject(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    if validation::nominal_cleanup_owner(&checked.typed, source).is_none() {
        return Ok(None);
    }
    let rendered = type_ids
        .iter()
        .map(|(identity, id)| {
            (
                checked.type_identity_text(*identity).unwrap_or_default(),
                *id,
            )
        })
        .collect::<Vec<_>>();
    nominal_cleanup_subject_from_types(
        checked,
        source,
        rendered
            .iter()
            .map(|(identity, id)| (identity.as_str(), *id)),
    )
}

fn nominal_cleanup_subject_from_types<'types>(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    mut types: impl Iterator<Item = (&'types str, semantic_vocabulary::StructuralTypeId)>,
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    let Some(owner) = validation::nominal_cleanup_owner(&checked.typed, source) else {
        return Ok(None);
    };
    let machine = checked.machine(source).ok_or(LoweringError::Unsupported(
        "cleanup declaration lost its exact source",
    ))?;
    let data = checked
        .data_definition_by_symbol(owner)
        .ok_or(LoweringError::Unsupported(
            "cleanup declaration lost its exact owner",
        ))?;
    let identity = if machine.attached_data_application.is_valid() {
        checked.normalized_type_identity(machine.attached_data_application)
    } else if let Some(application) = data.generic_instance {
        checked.normalized_type_identity(application)
    } else {
        checked
            .normalized_nominal_type_identity(owner)
            .ok_or(LoweringError::Unsupported(
                "cleanup owner has no normalized nominal identity",
            ))?
    };
    let (_, subject) = types
        .find(|(candidate, _)| {
            *candidate == checked.type_identity_text(identity).unwrap_or_default()
        })
        .ok_or(LoweringError::Unsupported(
            "owner-designated cleanup lost its exact structural declaration",
        ))?;
    Ok(Some(subject))
}

fn retain_nominal_cleanup_roles(
    checked: &CheckedTrees,
    owners: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), LoweringError> {
    for (source, identity) in owners {
        let mut matches = module
            .machines
            .iter_mut()
            .filter(|machine| machine.id == *identity);
        let machine = matches.next().ok_or(LoweringError::Unsupported(
            "cleanup source mapping lost its emitted machine",
        ))?;
        if matches.next().is_some() {
            return unsupported("cleanup source mapping repeats its emitted machine");
        }
        let expected = nominal_cleanup_subject_from_types(
            checked,
            *source,
            module.structural_types.iter().map(|declaration| {
                (
                    module.identities.text(declaration.identity).unwrap_or(""),
                    declaration.id,
                )
            }),
        )?;
        // Nominal ownership survives elision of an unused hook argument; it
        // does not manufacture an implicit receiver or storage declaration.
        machine.attachment = machine.attachment.or(expected);
        machine.nominal_cleanup_subject = expected;
    }
    Ok(())
}

fn lower_terminal_selection(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    program_roots: crate::TerminalProgramRoots<'_>,
) -> Result<(LoweredPsi, LoweringCompletion), LoweringError> {
    let LoweredSelectedMachine {
        terminal: mut lowered,
        completion,
        source_machine_ids,
        source_boundary_ids,
    } = phase_profile::scope("05.01_lower_selected_machine", || {
        lower_selected_machine(
            identities,
            checked,
            selection,
            program_roots,
            exact_toolchain_sources,
        )
    })?;
    if lowered.semantic_module.target_machine_families.len()
        != crate::unit::attached_unit::target_families::runtime_families(checked)?.len()
    {
        return unsupported("entry lowering did not retain the complete target-family catalog");
    }
    let source_machines = source_machine_ids
        .iter()
        .map(|(source, _)| *source)
        .collect::<Vec<_>>();
    phase_profile::scope("05.02_declarations_retain", || {
        declarations::retain(
            identities,
            checked,
            exact_toolchain_sources,
            &source_machine_ids,
            &mut lowered.semantic_module,
        )
    })
    .map_err(LoweringError::InvalidDeclarationIdentity)?;
    phase_profile::scope("05.03_nominal_cleanup_roles", || {
        retain_nominal_cleanup_roles(checked, &source_machine_ids, &mut lowered.semantic_module)
    })?;
    phase_profile::scope("05.04_closed_reach_applications", || {
        closed_reach_applications::retain_closed_reach_applications(
            identities,
            checked,
            &source_machine_ids,
            &lowered.source_call_occurrences,
            &mut lowered.semantic_module,
        )
    })?;
    phase_profile::scope("05.05_reborrow_restored_call_use", || -> Result<_, LoweringError> {
        for (source_machine, terminal_machine) in &source_machine_ids {
            reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
                identities,
                checked,
                *source_machine,
                *terminal_machine,
                &lowered.source_call_occurrences,
                &lowered.semantic_module.machines,
                &mut lowered.semantic_module.reborrow_restored_call_uses,
            )?;
        }
        Ok(())
    })?;
    phase_profile::scope("05.06_placed_and_suspension", || -> Result<_, LoweringError> {
        crate::retention::placed_view_plans::retain(
            identities,
            checked,
            exact_toolchain_sources,
            &mut lowered.semantic_module,
        )?;
        crate::retention::placement_lifecycles::retain(
            identities,
            checked,
            exact_toolchain_sources,
            &mut lowered,
        )?;
        suspension_call_plan::retain_suspension_call_plans(
            identities,
            checked,
            &source_machines,
            &lowered.source_call_occurrences,
            &mut lowered.semantic_module,
        )?;
        retained_borrow_custody::retain_foreign_borrow_custodies(
            identities,
            checked,
            exact_toolchain_sources,
            &mut lowered.semantic_module,
        )?;
        Ok(())
    })?;
    // Selected operator invocations carry their crash contract beside the
    // exact emitted operation; a named invocation's carrier is its own
    // boundary call's declaration, verified through the same projection
    // sources. A crash-qualified use the closure cannot join fails closed
    // here rather than lowering crash-free.
    phase_profile::scope("05.07_operation_crash_contracts", || {
        operation_crash_contracts::retain_operation_crash_contracts(
            identities,
            checked,
            &source_machines,
            &source_machine_ids,
            &mut lowered,
        )
    })?;
    // Guarded guarantees are admitted from the finished closure: whether the
    // entry returns one exact payloadless case is a fact of its lowered body,
    // whichever producer emitted it.
    phase_profile::scope("05.08_conformance_and_dyn", || -> Result<_, LoweringError> {
        reject_unguarded_outcome_guarantees(checked, &source_machine_ids, &lowered.semantic_module)?;
        crate::retention::conformance_applications::lower_closed_conformance_applications(
            identities,
            checked,
            &source_machines,
            &source_boundary_ids,
            &mut lowered.semantic_module,
        )?;
        crate::unit::dynamic_dispatch::lower_dynamic_dispatch_catalog(
            identities,
            checked,
            &source_machines,
            &mut lowered.semantic_module,
        )?;
        retain_float_meanings(identities, checked, &source_machine_ids, &mut lowered)?;
        Ok(())
    })?;
    lowered.build_provider_requirements = phase_profile::scope("05.09_build_providers", || {
        build_providers::retain(
            identities,
            checked,
            exact_toolchain_sources,
            &source_boundary_ids,
            &lowered.source_call_occurrences,
        )
    })?;

    // Copies emitted while composing values can allocate after frame reservation.
    // Retain their ordinary place declarations before publishing the module;
    // quotient adaptations keep their separate exact occurrence licenses.
    phase_profile::scope("05.10_reconcile_and_residences", || -> Result<_, LoweringError> {
        crate::emission::operation_emission::place_declarations::reconcile_leaf_copy_place_declarations(
            &mut lowered.semantic_module,
        );
        crate::emission::descriptor_residences::complete(&mut lowered.semantic_module)?;
        crate::retention::payload_memberships::retain(&mut lowered.semantic_module)?;
        Ok(())
    })?;
    phase_profile::scope("05.11_evidence_artifacts", || {
        lower_and_install_evidence_artifacts(identities, checked, &source_machine_ids, &mut lowered)
    })?;
    Ok((lowered, completion))
}

/// Failure-only imaging of the actual, unpublished module. The normal reader
/// remains the sole admission decision; these queries cannot authorize a call.
/// Keep environment/logging at the producer boundary, outside pure semantics.
fn diagnose_runtime_evidence_case_failure(module: &terminal_psi::TerminalModule, reason: &str) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
        return;
    }
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
        &module.structural_types,
        &graph,
    );
    for invocation in &module.proof_output_calls {
        let Some(runtime) = invocation.runtime_call else {
            continue;
        };
        let Some(caller) = module
            .machines
            .iter()
            .find(|machine| machine.id == invocation.caller)
        else {
            continue;
        };
        let Some(machine) = graph.iter().find(|machine| machine.id == caller.id) else {
            eprintln!(
                "[runtime-evidence-case] caller={:?} consumer={:?} graph=absent reason={reason}",
                caller.id, runtime.site
            );
            continue;
        };
        let feasibility = analysis.flow_feasibility(caller.id);
        for block in &caller.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let Some(parts) = terminal_psi::direct_call_parts(&operation.kind) else {
                    continue;
                };
                for row in parts.selected_evidence.iter().filter(|row| {
                    invocation
                        .evidence_arguments
                        .iter()
                        .any(|argument| argument.source == row.output)
                }) {
                    let terminal_psi::OperationResult::Structural(result) = &operation.result
                    else {
                        continue;
                    };
                    let predicate =
                        terminal_semantics::descriptor_results::DescriptorCasePredicate {
                            subject:
                                terminal_semantics::descriptor_lifetimes::DescriptorLocation::whole(
                                    result.place,
                                ),
                            case: row.guard.result_case,
                        };
                    eprintln!(
                        "[runtime-evidence-case] caller={:?} producer={:?} producer_cut=({:?},{index}) consumer={:?} output={:?} result={:?} type={:?} guard_case={:?} validity_result={:?} reason={reason}",
                        caller.id,
                        operation.id,
                        block.id,
                        runtime.site,
                        row.output,
                        result.place,
                        result.structural_type,
                        row.guard.result_case,
                        row.validity.result
                    );
                    for (position, step) in &machine.flow {
                        let established =
                            analysis.case_is_established(caller.id, *position, &predicate);
                        let membership =
                            analysis.case_feasibility(caller.id, *position, &predicate);
                        eprintln!(
                            "[runtime-evidence-case-step] caller={:?} cut={position:?} flow={:?} original_selected={established} original_membership={membership:?} definitions={:?} derivations={:?} producer={:?} contents_transfer={} unresolved_write={} payload_invalidations={}",
                            caller.id,
                            feasibility.get(position),
                            step.definitions,
                            step.derivations,
                            machine.payload_producers.get(position),
                            machine.contents_transfers.contains_key(position),
                            machine.unresolved_contents_writes.contains(position),
                            machine
                                .payload_invalidations
                                .get(position)
                                .map_or(0, Vec::len)
                        );
                        for edge in &step.successors {
                            eprintln!(
                                "[runtime-evidence-case-edge] caller={:?} cut={position:?} target={:?} parameters={:?} derivations={:?}",
                                caller.id,
                                edge.target,
                                edge.parameters,
                                edge.derivations().collect::<Vec<_>>()
                            );
                        }
                        for (branch, predicates) in &machine.case_successors {
                            for (observed, truth) in predicates
                                .iter()
                                .filter(|(observed, _)| observed.case == row.guard.result_case)
                            {
                                let selected =
                                    analysis.case_is_established(caller.id, *position, observed);
                                eprintln!(
                                    "[runtime-evidence-case-observation] caller={:?} cut={position:?} branch={branch:?} subject={:?} case={:?} truth={truth} selected={selected}",
                                    caller.id, observed.subject, observed.case
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
