//! Project checked task obligations once, before provider selection or target layout.
//! This operation is also the checking-only facade: it emits the same portable
//! bundle without demanding executable bodies or inventing a module MachineId.
mod carry_crossings;
mod runtime_requirements;
mod specialization_commitments;
mod stack_graphs;
mod start_selections;
mod target_layouts;

use crate::unit::attached_unit::{bodies::UnitPlans, catalog::lower_unit_structural_type_roots};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use terminal_psi::*;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Project the task obligations independently of native realization.
pub fn lower_task_activation_demands(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<TerminalTaskActivationDemands, Vec<Diagnostic>> {
    let typed = &checked.typed;
    let selections = start_selections::task_start_selections(typed)?;
    if selections.is_empty() {
        return Ok(TerminalTaskActivationDemands::default());
    }
    let mut sources = Vec::new();
    let mut roots = Vec::new();
    for selection in selections {
        let (machine, entry) = start_selections::exact_task_activation_target(
            typed,
            selection.target_machine,
            selection.target_entry,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "TaskRuntime start specialization has an invalid retained target coordinate: {}",
                error.message()
            ))]
        })?;
        if !typed.machine_type_parameters(machine).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` is still generic after specialization",
                machine.name
            ))]);
        }
        let graph =
            stack_graphs::task_call_graph(typed, &checked.facts, identities, machine, entry)?;
        for reference in typed
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.type_reference)
            .chain(std::iter::once(entry.return_type))
            .chain(
                graph
                    .frames
                    .iter()
                    .flat_map(|frame| &frame.states)
                    .flat_map(|state| {
                        state
                            .resident
                            .iter()
                            .chain(state.crossing_arguments.iter().flatten())
                    })
                    .copied(),
            )
        {
            if scalar_or_unit(checked, reference)?.is_none() {
                roots.push(validation::stored_structural_type_identity(
                    typed,
                    reference,
                    &[],
                    &[],
                ));
            }
        }
        sources.push((selection, machine, entry, graph));
    }
    roots.sort();
    roots.dedup();
    let plans = UnitPlans::published(&checked.facts.flow);
    let (structural_types, type_ids, structural_referents) = lower_unit_structural_type_roots(
        identities,
        checked,
        &crate::unit::attached_unit::scalar_applications::SourceScalarApplications::EMPTY,
        plans,
        &roots,
    )
    .map_err(lowering_diagnostics)?;
    let target_layout_applications =
        target_layouts::project(checked, plans, &structural_types, &type_ids)?;
    let mut demands = Vec::new();
    for (selection, machine, entry, graph) in sources {
        let (owner, _, authored) =
            runtime_requirements::exact_task_runtime_requirement(typed, &selection)?;
        let requirement = typed
            .trait_machine_signature_by_symbol(owner, selection.requirement)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task requirement lost its closed signature",
                )]
            })?;
        let contract = specialization_commitments::exact_task_machine_contract(
            &checked.facts,
            machine.symbol,
            machine.name.as_str(),
        )?;
        let specialization_commitment = specialization_commitments::task_specialization_commitment(
            typed,
            &checked.facts,
            &selection,
            machine,
            entry,
            contract,
            &typed.callable_identity_text(authored),
        )?;
        let mut crossings = graph
            .crossings
            .iter()
            .map(|crossing| carry_crossings::canonical_suspension_crossing(typed, crossing))
            .collect::<Result<Vec<_>, _>>()?;
        crossings.sort_by_key(|crossing| crossing.identity);
        crossings.dedup();
        let mut frames = Vec::new();
        for frame in graph.frames {
            let mut states = Vec::new();
            for state in frame.states {
                let resident = state
                    .resident
                    .into_iter()
                    .map(|reference| resident_value(checked, identities, &type_ids, reference))
                    .collect::<Result<Vec<_>, _>>()?;
                let crossing_arguments = state
                    .crossing_arguments
                    .into_iter()
                    .map(|arguments| {
                        arguments
                            .into_iter()
                            .map(|reference| {
                                resident_value(checked, identities, &type_ids, reference)
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                states.push(TerminalTaskFrameState {
                    state: state.state,
                    resident,
                    crossing_arguments,
                });
            }
            frames.push(TerminalTaskFrame {
                identity: frame.identity,
                machine_callable_identity: frame.machine_callable_identity,
                entry: frame.entry,
                states,
                calls: frame.calls,
                unresolved_calls: frame.unresolved_calls,
            });
        }
        frames.sort_by_key(|frame| frame.identity);
        let signature_arguments = typed
            .state_parameters(entry)
            .iter()
            .map(|parameter| {
                let value =
                    resident_value(checked, identities, &type_ids, parameter.type_reference)?;
                Ok(TerminalTaskSignatureValue {
                    value_type: value.value_type,
                    normalized_identity: value.normalized_identity,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let outcome = resident_value(checked, identities, &type_ids, entry.return_type)?;
        demands.push(TerminalTaskActivationDemand {
            operation: selection.operation,
            report_fingerprint: selection.report_fingerprint,
            requirement_callable_identity: identities
                .intern(&typed.trait_requirement_callable_identity_text(owner, requirement)),
            requirement_owner_path: identities.intern(&typed.trait_declaration_path(owner)),
            requirement_owner_package: typed.symbols.symbol_package_identity(owner.symbol),
            requirement_in_scope: typed
                .symbols
                .symbol_provenance_source_span(owner.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product),
            authored_requirement_identity: identities
                .intern(&typed.callable_identity_text(authored)),
            machine_callable_identity: identities.intern(
                &typed
                    .machine_callable_identity_text(machine)
                    .ok_or_else(|| {
                        vec![Diagnostic::error("task target lacks its callable identity")]
                    })?,
            ),
            entry: identities.intern(entry.name.as_str()),
            entry_report_fingerprint: entry_report_fingerprint(typed, machine, entry),
            contract_report_fingerprint: contract.report_fingerprint,
            specialization_commitment,
            specialization_report_fingerprint: selection.report_fingerprint,
            may_suspend: runtime_requirements::exact_task_machine_suspension(
                &checked.facts,
                machine.symbol,
                machine.name.as_str(),
            )?
            .checked_may_suspend,
            may_block: runtime_requirements::exact_task_machine_blocking(
                &checked.facts,
                machine.symbol,
                machine.name.as_str(),
            )?
            .checked_may_block,
            cancellation_required: true,
            signature_arguments,
            signature_outcome: TerminalTaskSignatureValue {
                value_type: outcome.value_type,
                normalized_identity: outcome.normalized_identity,
                is_mutable: false,
            },
            root_frame: graph.root,
            frames,
            crossings,
            carry_obligations: carry_crossings::carry_obligations(graph.carry),
        });
    }
    demands.sort_by_key(|demand| {
        (
            demand.report_fingerprint,
            demand.machine_callable_identity,
            demand.entry,
        )
    });
    Ok(TerminalTaskActivationDemands {
        demands,
        structural_types,
        structural_referents,
        target_layout_applications,
    })
}

fn scalar_or_unit(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<Option<TerminalTaskValueType>, Vec<Diagnostic>> {
    if matches!(
        checked.typed.type_reference_table.type_reference(reference),
        TypeReferenceNode::Unit
    ) {
        return Ok(Some(TerminalTaskValueType::Unit));
    }
    checked
        .typed
        .type_reference_table
        .primitive_type(reference)
        .map(|primitive| {
            crate::emission::scalar_types::terminal_scalar_type(primitive)
                .map(TerminalTaskValueType::Scalar)
                .map_err(lowering_diagnostics)
        })
        .transpose()
}

fn resident_value(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    types: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    reference: TypeReferenceHandle,
) -> Result<TerminalTaskResidentValue, Vec<Diagnostic>> {
    let value_type = match scalar_or_unit(checked, reference)? {
        Some(value_type) => value_type,
        None => TerminalTaskValueType::Structural(
            crate::terminal_identities::lookup_type_id(
                types,
                &validation::stored_structural_type_identity(&checked.typed, reference, &[], &[]),
            )
            .map_err(lowering_diagnostics)?,
        ),
    };
    Ok(TerminalTaskResidentValue {
        value_type,
        normalized_identity: identities
            .intern(&checked.typed.normalized_type_identity_text(reference)),
    })
}

fn lowering_diagnostics(error: crate::LoweringError) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "task activation type projection failed: {error:?}"
    ))]
}

fn entry_report_fingerprint(
    typed: &checked_trees::typed_trees::TypedTrees,
    machine: &checked_trees::machine::Machine,
    entry: &checked_trees::state::State,
) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.byte(0x45);
    hash.string_terminated(machine.name.as_str());
    hash.string_terminated(entry.name.as_str());
    for parameter in typed.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_mutable));
        hash.string_terminated(&typed.normalized_type_identity_text(parameter.type_reference));
    }
    hash.byte(0xfe);
    hash.string_terminated(&typed.normalized_type_identity_text(entry.return_type));
    hash.finish_or(1)
}
