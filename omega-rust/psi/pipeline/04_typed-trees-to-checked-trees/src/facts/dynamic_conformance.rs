//! Dynamic conformance facts and normalized dynamic row identities.

use typed_trees::TypedTrees;

pub(crate) fn build_dynamic_conformance_facts(
    program: &TypedTrees,
) -> Result<checked_trees::DynamicConformanceFacts, Vec<diagnostics::Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    let validated_selections = validation::collect_dynamic_conformance_selections(program)?;
    let validated_storages =
        validation::collect_dynamic_descriptor_storages(program, &validated_selections);
    for selection in validated_selections {
        let selected = selected_data_conformance(program, &selection);
        let mut rows = Vec::new();
        for row in selected
            .and_then(|conformance| program.closed_conformance_rows(conformance))
            .unwrap_or_default()
        {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "dynamic conformance row `{}::{}` reached checked lowering without an exact checked realization",
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
                    typed_trees::trait_definition::ConformanceRowSource::Inline => {
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
        selections.push(checked_trees::DynamicConformanceSelectionFact {
            occurrence: selection.occurrence,
            binding: selection.binding,
            binding_name: selection.binding_name.clone(),
            machine: selection.machine,
            state: selection.state,
            statement_index: selection.statement_index,
            source_symbol: selection.source_symbol,
            source_name: selection.source_name.clone(),
            source_path: selection.source_path.clone(),
            source_data: selection.source_data,
            target_trait: selection.target_trait,
            conformance: selection.conformance,
            rows,
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let binding_facts = checked_trees::DynamicConformanceFacts {
        selections: selections.clone(),
        storages: Vec::new(),
    }
    .binding_facts();
    let mut storages = Vec::with_capacity(validated_storages.len());
    for storage in validated_storages {
        let Some(selection) = binding_facts.selections.iter().find(|candidate| {
            candidate.machine == storage.selection.machine
                && candidate.state == storage.selection.state
                && candidate.statement_index == storage.selection.statement_index
                && candidate.binding == storage.selection.binding
                && candidate.target_trait == storage.selection.target_trait
                && candidate.conformance == storage.selection.conformance
        }) else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "dynamic descriptor storage lost its exact checked conformance selection",
            ));
            continue;
        };
        storages.push(checked_trees::DynamicDescriptorStorageFact {
            occurrence: storage.occurrence,
            machine: storage.machine,
            state: storage.state,
            statement_index: storage.statement_index,
            destination_binding: storage.destination_binding,
            destination_name: storage.destination_name,
            destination_field: storage.destination_field,
            destination_path: storage.destination_path,
            source_binding: storage.source_binding,
            source_name: storage.source_name,
            source_path: storage.source_path,
            selection: selection.clone(),
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(checked_trees::DynamicConformanceFacts {
        selections,
        storages,
    })
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
        .normalized_machine_overload_identity(realization_machine)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "dynamic conformance row realization has no normalized callable identity",
            )
        })?
        .identity();
    let requirement_identity = program
        .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
        .identity();
    Ok((requirement_identity, realization_identity))
}

fn selected_data_conformance<'program>(
    program: &'program TypedTrees,
    selection: &validation::DynamicConformanceSelection,
) -> Option<&'program typed_trees::trait_definition::Conformance> {
    if let Some(symbol) = selection.conformance {
        return program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == symbol);
    }
    let source_name = program.symbols.name(selection.source_data);
    let trait_name = program.symbols.name(selection.target_trait);
    let mut matches = program.conformances().iter().filter(|conformance| {
        conformance
            .carrier_name()
            .is_some_and(|carrier| carrier.as_str() == source_name)
            && conformance.trait_name.as_str() == trait_name
            && conformance.alias.is_none()
    });
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected)
}
