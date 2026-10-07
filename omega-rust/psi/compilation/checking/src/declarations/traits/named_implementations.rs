//! Named implementations (`PointShape satisfies Shape<Point> { ... }`) own one
//! closed map from every inherited requirement to exactly one realization row.
//! Each row's realization is checked against its requirement with the same
//! signature matching as machine-level `satisfies` clauses, under the
//! implementation's explicit trait arguments. No row is discovered from
//! machines attached to a data type: data does not satisfy traits.
//!
//! Check-then-synthesize (chapter 13): a written member is CHECKED; trait
//! machines with bodies are synthesized as member rows before resolution, and
//! a closed `Equatable<T>` implementation without an `equals` member receives
//! a synthesized row that typed lowering expands structurally (see the
//! lowering crate's `equatable` module). Every row, authored or synthesized, is
//! validated here against its requirement.

use super::trait_definition_by_symbol;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::trait_definition::ConformanceRowSource;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn validate_conformances(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, conformance) in program.conformances().iter().enumerate() {
        let implementation_name = conformance
            .alias
            .as_ref()
            .map_or("<unnamed implementation>", |alias| alias.as_str());
        let trait_name = conformance.trait_name.as_str();
        let conformance_name = conformance.alias.as_ref().map(|name| name.as_str());

        if program.conformances()[..index].iter().any(|previous| {
            !program
                .symbols
                .source_scopes_separate(previous.symbol, conformance.symbol)
                && previous.alias == conformance.alias
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "named implementation `{implementation_name}` is declared more than once"
            )));
        }

        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.trait_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{implementation_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            )));
            continue;
        };

        let expected_lifetimes = trait_definition.lifetime_parameters.len();
        if conformance.trait_lifetime_arguments.len() != expected_lifetimes {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{implementation_name} satisfies {trait_name}` expects {expected_lifetimes} target-trait lifetime argument(s), got {}",
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
                "conformance `{implementation_name} satisfies {trait_name}` retains a target-trait lifetime outside its conformance telescope"
            )));
            continue;
        }

        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected = program.trait_type_parameters(trait_definition).len();
        if arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{implementation_name} satisfies {trait_name}` expects {expected} generic argument(s), got {}",
                arguments.len()
            )));
            continue;
        }

        super::conformance::validate_trait_application_obligations(
            program,
            trait_definition,
            arguments,
            &format!("conformance `{implementation_name} satisfies {trait_name}`"),
            diagnostics,
        );

        validate_closed_rows(
            program,
            service_reaches,
            implementation_name,
            trait_definition,
            arguments,
            &conformance.rows,
            conformance_name,
            diagnostics,
        );
    }
}

fn validate_closed_rows(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    implementation_name: &str,
    root_trait: &TraitDefinition,
    root_arguments: &[TypeReferenceHandle],
    rows: &[typed_trees::trait_definition::ConformanceRow],
    conformance_name: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for row in rows {
        let Some(declaring_trait) = trait_definition_by_symbol(program, row.declaring_trait) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` retains an unresolved declaring trait for row `{}`",
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
                "closed conformance `{implementation_name} satisfies {}` retains an unresolved requirement `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        if row.source == ConformanceRowSource::TraitDefault && !requirement.is_default {
            diagnostics.push(Diagnostic::error(format!(
                    "closed conformance `{implementation_name} satisfies {}` selects a default for bodyless requirement `{}::{}`",
                    root_trait.name, row.declaring_trait_name, row.requirement_name
                )));
        }
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization machine",
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
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization state",
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
                "closed conformance `{implementation_name} satisfies {}` cannot instantiate inherited row `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        super::conformance::validate_machine_state_satisfies_trait_signature_with_arguments(
            program,
            service_reaches,
            machine,
            state,
            declaring_trait,
            requirement,
            None,
            &arguments,
            diagnostics,
        );
        crate::proof_contracts::contract_entailment::check_law_conformance(
            program,
            machine,
            conformance_name,
            declaring_trait,
            requirement,
            &arguments,
            diagnostics,
        );
        // Authored rows choose their machine; a default or synthesized row is
        // the requirement's own declaration instantiated for this map.
        if matches!(
            row.source,
            ConformanceRowSource::Inline | ConformanceRowSource::Reference
        ) {
            crate::proof_contracts::contract_entailment::check_contract_refinement(
                program,
                machine,
                declaring_trait,
                requirement,
                diagnostics,
            );
        }
    }
}

pub(crate) fn arguments_for_declaring_trait(
    program: &TypedTrees,
    current_trait: &TraitDefinition,
    current_arguments: &[TypeReferenceHandle],
    target_trait: symbols::SymbolHandle,
    visited: &mut Vec<symbols::SymbolHandle>,
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
