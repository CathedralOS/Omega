//! Named bodyless conformance items (`PointEquatable: Point satisfies
//! Equatable;`) claim a whole trait for a data type. The claim is checked
//! against the type's attached machines (`machine Point::equals(...)`), whether
//! authored or synthesized, using the same signature matching as machine-level
//! `satisfies` clauses.
//!
//! Check-then-synthesize (chapter 13): a written member is CHECKED; the core
//! `Equatable::equals` member and trait machines with bodies are synthesized as
//! ordinary attached machines before resolution. `equals` then expands to
//! structural equality during resolved->typed lowering, which also enforces
//! the structural prerequisites at the conformance item (see the lowering
//! crate's `equatable` module). Written members remain authoritative; a missing
//! bodyless requirement stays an error naming the machine the type must write.

use super::shared::trait_definition_by_symbol;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::trait_definition::{ConformanceImplementation, ConformanceRowSource};
use psi_typed_trees::types::TypeReferenceHandle;

pub(crate) fn validate_conformances(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for (index, conformance) in program.conformances().iter().enumerate() {
        let (subject_name, carrier_name) = match &conformance.subject {
            psi_typed_trees::trait_definition::ConformanceSubject::Carrier(type_name) => {
                (type_name.as_str(), Some(type_name.as_str()))
            }
            psi_typed_trees::trait_definition::ConformanceSubject::Subjectless => (
                conformance
                    .alias
                    .as_ref()
                    .map_or("<subjectless>", |alias| alias.as_str()),
                None,
            ),
        };
        let trait_name = conformance.trait_name.as_str();
        let conformance_name = conformance.alias.as_ref().map(|name| name.as_str());

        for previous in &program.conformances()[..index] {
            if program
                .symbols
                .source_scopes_separate(previous.symbol, conformance.symbol)
            {
                continue;
            }
            match (&previous.subject, &conformance.subject) {
                (
                    psi_typed_trees::trait_definition::ConformanceSubject::Carrier(previous_type),
                    psi_typed_trees::trait_definition::ConformanceSubject::Carrier(type_name),
                ) if previous_type == type_name => match (&previous.alias, &conformance.alias) {
                    (Some(previous), Some(alias)) if previous == alias => {
                        diagnostics.push(Diagnostic::error(format!(
                            "data `{subject_name}` declares conformance name `{alias}` more than once"
                        )));
                    }
                    (None, None) if previous.trait_name == conformance.trait_name => {
                        diagnostics.push(Diagnostic::error(format!(
                            "data `{subject_name}` declares unnamed conformance to `{trait_name}` more than once"
                        )));
                    }
                    _ => {}
                },
                (
                    psi_typed_trees::trait_definition::ConformanceSubject::Subjectless,
                    psi_typed_trees::trait_definition::ConformanceSubject::Subjectless,
                ) if previous.alias == conformance.alias => {
                    diagnostics.push(Diagnostic::error(format!(
                        "subjectless conformance name `{subject_name}` is declared more than once"
                    )));
                }
                _ => {}
            }
        }

        if let Some(type_name) = carrier_name {
            let data_exists = program
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == conformance.carrier_symbol);
            let name_owned_type_parameter = program
                .conformance_type_parameters(conformance)
                .iter()
                .any(|parameter| {
                    parameter.symbol == conformance.carrier_symbol
                        && matches!(
                            parameter.kind,
                            psi_typed_trees::data::TypeParameterKind::Type
                        )
                });
            if !data_exists && !name_owned_type_parameter {
                diagnostics.push(Diagnostic::error(format!(
                    "conformance `{type_name} satisfies {trait_name}` names unknown data `{type_name}`"
                )));
                continue;
            }
        }

        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.trait_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{subject_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            )));
            continue;
        };

        let expected_lifetimes = trait_definition.lifetime_parameters.len();
        if conformance.trait_lifetime_arguments.len() != expected_lifetimes {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{subject_name} satisfies {trait_name}` expects {expected_lifetimes} target-trait lifetime argument(s), got {}",
                conformance.trait_lifetime_arguments.len()
            )));
            continue;
        }
        if conformance.trait_lifetime_arguments.iter().any(|ordinal| {
            usize::try_from(*ordinal).map_or(true, |ordinal| {
                ordinal >= conformance.lifetime_parameters.len()
            })
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{subject_name} satisfies {trait_name}` retains a target-trait lifetime outside its conformance telescope"
            )));
            continue;
        }

        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected = program.trait_type_parameters(trait_definition).len();
        if arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{subject_name} satisfies {trait_name}` expects {expected} generic argument(s), got {}",
                arguments.len()
            )));
            continue;
        }

        super::conformance::validate_trait_application_obligations(
            program,
            trait_definition,
            arguments,
            &[],
            &format!("conformance `{subject_name} satisfies {trait_name}`"),
            diagnostics,
        );

        match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => {
                if let Some(type_name) = carrier_name {
                    validate_data_satisfies_trait(
                        program,
                        conformance.carrier_symbol,
                        type_name,
                        trait_definition,
                        arguments,
                        conformance_name,
                        diagnostics,
                        &mut Vec::new(),
                    );
                } else {
                    diagnostics.push(Diagnostic::error(
                        "a subjectless conformance must own a closed member map",
                    ));
                }
            }
            ConformanceImplementation::Closed { rows } => validate_closed_rows(
                program,
                subject_name,
                trait_definition,
                arguments,
                rows,
                conformance_name,
                diagnostics,
            ),
        }
    }
}

fn validate_closed_rows(
    program: &TypedTrees,
    type_name: &str,
    root_trait: &TraitDefinition,
    root_arguments: &[TypeReferenceHandle],
    rows: &[psi_typed_trees::trait_definition::ConformanceRow],
    conformance_name: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for row in rows {
        let Some(declaring_trait) = trait_definition_by_symbol(program, row.declaring_trait) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{type_name} satisfies {}` retains an unresolved declaring trait for row `{}`",
                root_trait.name, row.requirement_name
            )));
            continue;
        };
        let Some(requirement) = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .find(|requirement| requirement.symbol == row.requirement)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{type_name} satisfies {}` retains an unresolved requirement `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        if row.source == ConformanceRowSource::TraitDefault {
            if !requirement.is_default {
                diagnostics.push(Diagnostic::error(format!(
                    "closed conformance `{type_name} satisfies {}` selects a default for bodyless requirement `{}::{}`",
                    root_trait.name, row.declaring_trait_name, row.requirement_name
                )));
            }
        }
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{type_name} satisfies {}` row `{}::{}` retains no exact realization machine",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == row.realization_state)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{type_name} satisfies {}` row `{}::{}` retains no exact realization state",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        let Some(arguments) = arguments_for_declaring_trait(
            program,
            root_trait,
            root_arguments,
            row.declaring_trait,
            &mut Vec::new(),
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{type_name} satisfies {}` cannot instantiate inherited row `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        super::conformance::validate_machine_state_satisfies_trait_signature_with_arguments(
            program,
            machine,
            state,
            declaring_trait,
            requirement,
            &arguments,
            diagnostics,
        );
        crate::contract_entailment::check_law_conformance(
            program,
            machine,
            conformance_name,
            declaring_trait,
            requirement,
            &arguments,
            diagnostics,
        );
    }
}

pub(crate) fn arguments_for_declaring_trait(
    program: &TypedTrees,
    current_trait: &TraitDefinition,
    current_arguments: &[TypeReferenceHandle],
    target_trait: psi_symbols::SymbolHandle,
    visited: &mut Vec<psi_symbols::SymbolHandle>,
) -> Option<Vec<TypeReferenceHandle>> {
    if current_trait.symbol == target_trait {
        return Some(current_arguments.to_vec());
    }
    if visited.contains(&current_trait.symbol) {
        return None;
    }
    visited.push(current_trait.symbol);
    for parent in program.trait_requirements(current_trait) {
        let Some(parent_trait) = trait_definition_by_symbol(program, parent.symbol) else {
            continue;
        };
        let parent_arguments = super::conformance::compose_forwarded_trait_arguments(
            program,
            current_trait,
            current_arguments,
            program
                .type_reference_table
                .type_reference_handles(parent.arguments),
        );
        if let Some(arguments) = arguments_for_declaring_trait(
            program,
            parent_trait,
            &parent_arguments,
            target_trait,
            visited,
        ) {
            return Some(arguments);
        }
    }
    None
}

fn validate_data_satisfies_trait(
    program: &TypedTrees,
    data_symbol: psi_symbols::SymbolHandle,
    type_name: &str,
    trait_definition: &TraitDefinition,
    explicit_type_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    conformance_name: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
    visited_traits: &mut Vec<psi_symbols::SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }
    visited_traits.push(trait_definition.symbol);

    for requirement in program.trait_machine_signatures(trait_definition) {
        let Some((machine, state)) =
            attached_state_named(program, data_symbol, type_name, requirement.name.as_str())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "data `{type_name}` does not satisfy trait `{}`: no written or default machine `{type_name}::{}`",
                trait_definition.name, requirement.name
            )));
            continue;
        };

        super::conformance::validate_machine_state_satisfies_trait_signature_with_arguments(
            program,
            machine,
            state,
            trait_definition,
            requirement,
            explicit_type_arguments,
            diagnostics,
        );
        crate::contract_entailment::check_law_conformance(
            program,
            machine,
            conformance_name,
            trait_definition,
            requirement,
            explicit_type_arguments,
            diagnostics,
        );
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };
        let required_arguments = super::conformance::compose_forwarded_trait_arguments(
            program,
            trait_definition,
            explicit_type_arguments,
            program
                .type_reference_table
                .type_reference_handles(requirement.arguments),
        );
        validate_data_satisfies_trait(
            program,
            data_symbol,
            type_name,
            required_trait,
            &required_arguments,
            conformance_name,
            diagnostics,
            visited_traits,
        );
    }

    visited_traits.pop();
}

/// Find a state named `state_name` on any machine attached to `type_name`
/// (`machine Point::equals(...)` declares attached data `Point` with an entry
/// state `equals`).
fn attached_state_named<'program>(
    program: &'program TypedTrees,
    data_symbol: psi_symbols::SymbolHandle,
    type_name: &str,
    state_name: &str,
) -> Option<(&'program Machine, &'program State)> {
    program
        .machines()
        .iter()
        .filter(|machine| {
            machine.attached_data_symbol == data_symbol
                || (!program.symbols.has_source_metadata()
                    && machine
                        .attached_data
                        .as_ref()
                        .is_some_and(|attached| attached.as_str() == type_name))
        })
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == state_name)
                .map(|state| (machine, state))
        })
}
