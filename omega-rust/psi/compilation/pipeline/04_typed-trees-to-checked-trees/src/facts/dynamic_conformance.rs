//! Dynamic conformance facts and normalized dynamic row identities.

use typed_trees::TypedTrees;

pub(crate) fn build_dynamic_conformance_facts(
    program: &TypedTrees,
) -> Result<checked_trees::DynamicConformanceFacts, Vec<diagnostics::Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for selection in validation::collect_dynamic_implementation_selections(program)? {
        let Some(implementation) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == selection.implementation)
        else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "dynamic selection names no exact implementation",
            ));
            continue;
        };
        let mut rows = Vec::new();
        for row in program
            .closed_conformance_rows(implementation)
            .unwrap_or_default()
        {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "dynamic implementation row `{}::{}` reached checked lowering without an exact checked realization",
                    row.declaring_trait_name, row.requirement_name
                )));
                continue;
            }
            let (requirement_identity, realization_identity) =
                match normalized_dynamic_row_identities(program, row) {
                    Ok(identities) => identities,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
            rows.push(checked_trees::DynamicConformanceRowFact {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                requirement_identity,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                realization_identity,
                source: match row.source {
                    // A synthesized core row is an ordinary checked member
                    // body of its own block, exactly as an inline row is.
                    typed_trees::trait_definition::ConformanceRowSource::Inline
                    | typed_trees::trait_definition::ConformanceRowSource::Synthesized => {
                        checked_trees::DynamicConformanceRowSource::Inline
                    }
                    typed_trees::trait_definition::ConformanceRowSource::Reference => {
                        checked_trees::DynamicConformanceRowSource::Reference
                    }
                    typed_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                        checked_trees::DynamicConformanceRowSource::TraitDefault
                    }
                },
            });
        }
        selections.push(checked_trees::DynamicSelectionFact {
            occurrence: selection.occurrence,
            machine: selection.machine,
            state: selection.state,
            implementation: selection.implementation,
            target_trait: selection.target_trait,
            interface_identity: program
                .normalized_type_identity(selection.selector_type)
                .as_str()
                .to_owned(),
            rows,
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(checked_trees::DynamicConformanceFacts { selections })
}

pub(crate) fn normalized_dynamic_row_identities(
    program: &TypedTrees,
    row: &typed_trees::trait_definition::ConformanceRow,
) -> Result<(String, String), diagnostics::Diagnostic> {
    let mut declaring_traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == row.declaring_trait);
    let declaring_trait = declaring_traits.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact declaring trait for normalized identity",
        )
    })?;
    if declaring_traits.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous declaring trait for normalized identity",
        ));
    }

    let mut requirements = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .filter(|requirement| requirement.symbol == row.requirement);
    let requirement = requirements.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact requirement for normalized identity",
        )
    })?;
    if requirements.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous requirement for normalized identity",
        ));
    }

    let mut realization_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == row.realization_machine);
    let realization_machine = realization_machines.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact realization machine for normalized identity",
        )
    })?;
    if realization_machines.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous realization machine for normalized identity",
        ));
    }
    let realization_identity = program
        .machine_callable_identity_text(realization_machine)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "dynamic conformance row realization has no normalized callable identity",
            )
        })?;
    let requirement_identity =
        program.trait_requirement_callable_identity_text(declaring_trait, requirement);
    Ok((requirement_identity, realization_identity))
}
