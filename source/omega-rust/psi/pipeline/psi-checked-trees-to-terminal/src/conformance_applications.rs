use psi_checked_trees::data::TypeParameterKind;
use psi_checked_trees::{CheckedTrees, ClosedConformanceConstArgument};
use psi_terminal::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, TerminalModule,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

use super::LoweringError;

pub(super) fn lower_closed_conformance_applications(
    checked: &CheckedTrees,
    source_machines: &[psi_symbols::SymbolHandle],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let carries_applications = checked
        .machine_specializations
        .iter()
        .any(|specialization| {
            source_machines.contains(&specialization.instance)
                && !specialization.conformance_applications.is_empty()
        });
    if !carries_applications {
        module.closed_conformance_applications.clear();
        return Ok(());
    }
    if source_machines.len() != module.machines.len() {
        return Err(LoweringError::Unsupported(
            "closed conformance application ownership does not match the terminal machine closure",
        ));
    }

    let owners = source_machines
        .iter()
        .zip(&module.machines)
        .map(|(source, terminal)| (*source, terminal.id))
        .collect::<Vec<_>>();
    let mut applications = Vec::new();
    for specialization in &checked.machine_specializations {
        let Some((_, owner)) = owners
            .iter()
            .find(|(source, _)| *source == specialization.instance)
        else {
            continue;
        };
        for application in &specialization.conformance_applications {
            let selected = checked
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == application.declaration)
                .ok_or(LoweringError::Unsupported(
                    "closed conformance application lost its declaration",
                ))?;
            let selected_rows =
                checked
                    .closed_conformance_rows(selected)
                    .ok_or(LoweringError::Unsupported(
                        "closed conformance application lost its normalized row map",
                    ))?;
            let mut realization_callables = checked
                .facts
                .proof
                .proof_output_calls
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.caller_machine_symbol == specialization.instance)
                        .then_some(invocation.static_requirement_dispatch?)
                })
                .filter(|dispatch| {
                    dispatch.application_report_fingerprint == application.report_fingerprint
                        && dispatch.application_commitment == application.commitment
                })
                .map(|dispatch| {
                    let selected_row = selected_rows
                        .iter()
                        .find(|row| {
                            row.declaring_trait == dispatch.declaring_trait
                                && row.requirement == dispatch.requirement
                                && row.realization_machine == dispatch.realization_machine
                                && row.realization_state == dispatch.realization_state
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance dispatch lost its normalized row",
                        ))?;
                    let machine = owners
                        .iter()
                        .find_map(|(source, terminal)| {
                            (*source == selected_row.realization_machine).then_some(*terminal)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization is absent from the Terminal closure",
                        ))?;
                    let realization_state = checked
                        .typed
                        .machines()
                        .iter()
                        .flat_map(|machine| checked.typed.machine_states(machine))
                        .find(|state| state.symbol == selected_row.realization_state)
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization state is absent",
                        ))?;
                    let requirements = checked
                        .typed
                        .traits()
                        .iter()
                        .find(|definition| definition.symbol == dispatch.declaring_trait)
                        .map(|definition| {
                            checked
                                .typed
                                .trait_machine_signatures(definition)
                                .iter()
                                .filter(|requirement| requirement.symbol == dispatch.requirement)
                                .collect::<Vec<_>>()
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance requirement is absent",
                        ))?;
                    let [requirement] = requirements.as_slice() else {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement is absent or ambiguous",
                        ));
                    };
                    let classify = |return_type: psi_checked_trees::types::TypeReferenceHandle| {
                        if !return_type.is_valid() {
                            return Ok(ClosedConformanceCallableResult::Unit);
                        }
                        match checked.typed.primitive_type_reference(return_type) {
                            Some(super::PrimitiveType::I32) => {
                                Ok(ClosedConformanceCallableResult::I32)
                            }
                            Some(super::PrimitiveType::Bool) => {
                                Ok(ClosedConformanceCallableResult::Bool)
                            }
                            _ => Err(LoweringError::Unsupported(
                                "static conformance callable has an unsupported result class",
                            )),
                        }
                    };
                    let result = classify(realization_state.return_type)?;
                    if classify(requirement.return_type)? != result {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement and realization result classes differ",
                        ));
                    }
                    Ok(ClosedConformanceRealizationCallable {
                        source_callable_identity:
                            super::evidence_lowering::checked_evidence_machine_identity(
                                checked,
                                selected_row.realization_machine,
                            )?,
                        machine,
                        result,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            realization_callables.sort();
            realization_callables.dedup();
            let rows = application
                .rows
                .iter()
                .map(|row| {
                    let selected_row = selected_rows
                        .iter()
                        .find(|candidate| {
                            candidate.declaring_trait == row.declaring_trait
                                && candidate.requirement == row.requirement
                                && candidate.realization_state == row.realization_state
                        })
                        .ok_or(LoweringError::Unsupported(
                            "closed conformance application row no longer matches its declaration",
                        ))?;
                    let selected_by_static_dispatch = checked
                        .facts
                        .proof
                        .proof_output_calls
                        .iter()
                        .filter_map(|(_, invocation)| {
                            (invocation.caller_machine_symbol == specialization.instance)
                                .then_some(invocation.static_requirement_dispatch?)
                        })
                        .any(|dispatch| {
                            dispatch.application_report_fingerprint
                                == application.report_fingerprint
                                && dispatch.application_commitment == application.commitment
                                && dispatch.declaring_trait == selected_row.declaring_trait
                                && dispatch.requirement == selected_row.requirement
                                && dispatch.realization_machine
                                    == selected_row.realization_machine
                                && dispatch.realization_state == selected_row.realization_state
                        });
                    let realization_callable_identity = selected_by_static_dispatch
                        .then(|| {
                            let machine = owners
                                .iter()
                                .find_map(|(source, terminal)| {
                                    (*source == selected_row.realization_machine)
                                        .then_some(*terminal)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance realization is absent from the Terminal closure",
                                ))?;
                            let identity =
                                super::evidence_lowering::checked_evidence_machine_identity(
                                    checked,
                                    selected_row.realization_machine,
                                )?;
                            realization_callables
                                .iter()
                                .any(|callable| {
                                    callable.source_callable_identity == identity
                                        && callable.machine == machine
                                })
                                .then_some(identity)
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance row lost its independent callable registry entry",
                                ))
                        })
                        .transpose()?;
                    Ok(ClosedConformanceRow {
                        declaring_trait_identity: checked
                            .symbols
                            .display_path(selected_row.declaring_trait, "::"),
                        public_requirement_identity:
                            super::evidence_lowering::checked_evidence_requirement_identity(
                                checked,
                                selected_row.declaring_trait,
                                selected_row.requirement,
                            )?,
                        requirement_identity: checked
                            .symbols
                            .display_path(selected_row.requirement, "::"),
                        realization_identity: checked
                            .symbols
                            .display_path(selected_row.realization_state, "::"),
                        realization_callable_identity,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if selected.lifetime_parameters.len() != application.lifetime_arguments.len() {
                return Err(LoweringError::Unsupported(
                    "closed conformance application has an open lifetime telescope",
                ));
            }
            let mut telescope = selected
                .lifetime_parameters
                .iter()
                .zip(&application.lifetime_arguments)
                .map(|(parameter, argument)| ClosedConformanceParameterBinding {
                    parameter: parameter.as_str().to_owned(),
                    kind: ClosedConformanceParameterKind::Lifetime,
                    argument: argument.clone(),
                })
                .collect::<Vec<_>>();
            let mut type_index = 0usize;
            let mut const_index = 0usize;
            let mut machine_index = 0usize;
            for parameter in checked.conformance_type_parameters(selected) {
                let (kind, argument) = match &parameter.kind {
                    TypeParameterKind::Type => {
                        let argument = application.type_arguments.get(type_index).cloned();
                        type_index += 1;
                        (ClosedConformanceParameterKind::Type, argument)
                    }
                    TypeParameterKind::Const { .. } => {
                        let argument = match application.const_arguments.get(const_index) {
                            Some(ClosedConformanceConstArgument::Evaluated { value, .. }) => Some(
                                psi_language_semantics::const_value::CanonicalConstValue::new(
                                    value.type_name.clone(),
                                    value.encoding.clone(),
                                    "",
                                )
                                .atom(),
                            ),
                            Some(ClosedConformanceConstArgument::CallerBinder { .. }) => {
                                return Err(LoweringError::Unsupported(
                                    "closed conformance application reaches Terminal with an unsubstituted const binder",
                                ));
                            }
                            None => None,
                        };
                        const_index += 1;
                        (ClosedConformanceParameterKind::Const, argument)
                    }
                    TypeParameterKind::Machine { .. } => {
                        let argument = application
                            .machine_arguments
                            .get(machine_index)
                            .map(|argument| checked.symbols.display_path(*argument, "::"));
                        machine_index += 1;
                        (ClosedConformanceParameterKind::Machine, argument)
                    }
                    TypeParameterKind::Proposition { .. } => {
                        return Err(LoweringError::Unsupported(
                            "closed conformance application retains a proposition parameter",
                        ));
                    }
                };
                telescope.push(ClosedConformanceParameterBinding {
                    parameter: parameter.name.as_str().to_owned(),
                    kind,
                    argument: argument.ok_or(LoweringError::Unsupported(
                        "closed conformance application has an open static telescope",
                    ))?,
                });
            }
            if type_index != application.type_arguments.len()
                || const_index != application.const_arguments.len()
                || machine_index != application.machine_arguments.len()
            {
                return Err(LoweringError::Unsupported(
                    "closed conformance application telescope has extra arguments",
                ));
            }
            let mut lowered = ClosedConformanceApplication {
                owner: *owner,
                declaration_identity: checked.symbols.display_path(application.declaration, "::"),
                telescope,
                subject_identity: application.subject_identity.clone(),
                trait_identity: checked
                    .symbols
                    .display_path(application.trait_definition, "::"),
                trait_lifetime_arguments: application.trait_lifetime_arguments.clone(),
                trait_arguments: application.trait_arguments.clone(),
                realization_callables,
                rows,
                report_fingerprint: 0,
                commitment: Default::default(),
            };
            lowered.report_fingerprint =
                closed_conformance_application_report_fingerprint(&lowered);
            lowered.commitment = closed_conformance_application_commitment(&lowered);
            applications.push(lowered);
        }
    }
    applications.sort_by(|left, right| {
        (
            left.owner,
            &left.declaration_identity,
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                &right.declaration_identity,
                right.report_fingerprint,
            ))
    });
    module.closed_conformance_applications = applications;
    Ok(())
}
