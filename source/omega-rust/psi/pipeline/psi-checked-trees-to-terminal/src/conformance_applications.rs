use psi_checked_trees::CheckedTrees;
use psi_checked_trees::data::TypeParameterKind;
use psi_terminal::{
    ClosedConformanceApplication, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow, TerminalModule,
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
                        let argument = application.const_arguments.get(const_index).cloned();
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
                trait_arguments: application.trait_arguments.clone(),
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
