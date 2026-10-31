//! Dynamic conformance facts and normalized dynamic row identities.

pub(crate) fn build_dynamic_conformance_facts(
    program: &dyn ::validation::CheckingInput,
) -> Result<checked_trees::DynamicConformanceFacts, Vec<diagnostics::Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for selection in validation::collect_dynamic_implementation_selections(program)? {
        let Some(implementation) = program.conformance_by_symbol(selection.implementation) else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "dynamic selection names no exact implementation",
            ));
            continue;
        };
        let mut rows = Vec::new();
        for row in &implementation.rows {
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
            let source = match row.source {
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
            };
            // A finite-family requirement's row fans out to one checked row
            // per declared roster tuple: each tuple's realization is the
            // specialization `generate_dynamic_family_specializations`
            // materialized for it, joined by the same canonical const
            // identities the roster declares. A missing tuple row is a
            // program error -- a selected conformance must cover the roster.
            match family_roster(program, row) {
                FamilyRows::Exact => rows.push(checked_trees::DynamicConformanceRowFact {
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    requirement_identity,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                    realization_identity,
                    family_tuple: Box::default(),
                    source,
                }),
                FamilyRows::Finite(tuples) => {
                    for tuple in &tuples {
                        let Some(instance) =
                            family_specialization_instance(program, row.realization_machine, tuple)
                        else {
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                                "dynamic implementation row `{}::{}` has no checked specialization for declared family tuple {}",
                                row.declaring_trait_name,
                                row.requirement_name,
                                tuple.display.join(", ")
                            )));
                            continue;
                        };
                        let Some(instance_state) =
                            family_realization_state(program, instance, row.realization_state)
                        else {
                            diagnostics.push(diagnostics::Diagnostic::error(format!(
                                "dynamic family specialization `{}` has no realization state for tuple {}",
                                program.symbols.name(instance),
                                tuple.display.join(", ")
                            )));
                            continue;
                        };
                        let Some(instance_machine) = program.machine_by_symbol(instance) else {
                            continue;
                        };
                        rows.push(checked_trees::DynamicConformanceRowFact {
                            declaring_trait: row.declaring_trait,
                            requirement: row.requirement,
                            requirement_identity,
                            realization_machine: instance,
                            realization_state: instance_state,
                            realization_identity: program
                                .machine_callable_identity(instance_machine),
                            family_tuple: tuple.identities.clone(),
                            source,
                        });
                    }
                }
            }
        }
        selections.push(checked_trees::DynamicSelectionFact {
            occurrence: selection.occurrence,
            machine: selection.machine,
            state: selection.state,
            implementation: selection.implementation,
            target_trait: selection.target_trait,
            interface_identity: program
                .normalized_type_identity(selection.selector_type)
                .to_owned(),
            rows,
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(checked_trees::DynamicConformanceFacts { selections })
}

/// The roster one conformance row must cover: exact rows produce their
/// template realization; finite rows produce one specialization per tuple.
enum FamilyRows {
    Exact,
    Finite(Vec<typed_trees::finite_family::FamilyTuple>),
}

/// The declared roster of a conformance row's requirement, when the applied
/// requirement signature enumerates a finite family. Reads the prepared
/// applied signature (the same view the specialization demand collector used)
/// so an inherited family's carriers match the checked roster.
fn family_roster(
    program: &dyn ::validation::CheckingInput,
    row: &typed_trees::trait_definition::ConformanceRow,
) -> FamilyRows {
    let Some(definition) = program.trait_definition_by_symbol(row.declaring_trait) else {
        return FamilyRows::Exact;
    };
    let Some(signature) = program.trait_machine_signature_by_symbol(definition, row.requirement)
    else {
        return FamilyRows::Exact;
    };
    match program.finite_signature_family(signature) {
        typed_trees::finite_family::FamilyProbe::Finite { tuples, .. } => {
            FamilyRows::Finite(tuples)
        }
        typed_trees::finite_family::FamilyProbe::NotFinite(_) => FamilyRows::Exact,
    }
}

/// The specialization instance a family tuple commits: the checked-body
/// specialization of `template` whose retained const identities equal the
/// tuple exactly, with no non-value argument coordinates.
fn family_specialization_instance(
    program: &dyn ::validation::CheckingInput,
    template: symbols::SymbolHandle,
    tuple: &typed_trees::finite_family::FamilyTuple,
) -> Option<symbols::SymbolHandle> {
    program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.template == template
                && specialization.const_argument_identities.as_slice() == tuple.identities.as_ref()
                && specialization.is_const_only()
        })
        .map(|specialization| specialization.instance)
}

/// The specialization's realization state for the template row's entry:
/// specialization cloning preserves state names, so the instance state with
/// the template realization's name is the same body position.
fn family_realization_state(
    program: &dyn ::validation::CheckingInput,
    instance: symbols::SymbolHandle,
    template_state: symbols::SymbolHandle,
) -> Option<symbols::SymbolHandle> {
    let machine = program.machine_by_symbol(instance)?;
    let name = program.symbols.name(template_state);
    program
        .machine_states(machine)
        .iter()
        .find(|state| program.symbols.name(state.symbol) == name)
        .or_else(|| program.machine_states(machine).first())
        .map(|state| state.symbol)
}

pub(crate) fn normalized_dynamic_row_identities(
    program: &dyn ::validation::CheckingInput,
    row: &typed_trees::trait_definition::ConformanceRow,
) -> Result<
    (
        typed_trees::type_identity::CallableIdentityId,
        typed_trees::type_identity::CallableIdentityId,
    ),
    diagnostics::Diagnostic,
> {
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
    let realization_identity = program.machine_callable_identity(realization_machine);
    let requirement_identity =
        program.trait_requirement_callable_identity(declaring_trait, requirement);
    Ok((requirement_identity, realization_identity))
}
