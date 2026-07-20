//! Standalone conformance items (frozen decision 8): `Point satisfies
//! Equatable;` claims a whole trait for a data type. The claim is checked
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
use crate::symbols::TopLevelSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::trait_definition::TraitDefinition;

pub(crate) fn validate_data_conformances(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.data_conformances() {
        let type_name = conformance.type_name.as_str();
        let trait_name = conformance.trait_name.as_str();

        let data_exists = program
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == type_name);
        if !data_exists {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{type_name} satisfies {trait_name}` names unknown data `{type_name}`"
            )));
            continue;
        }

        let Some(trait_definition) = symbols.trait_definition(trait_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{type_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            )));
            continue;
        };

        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected = program.trait_type_parameters(trait_definition).len();
        if arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{type_name} satisfies {trait_name}` expects {expected} generic argument(s), got {}",
                arguments.len()
            )));
            continue;
        }

        validate_data_satisfies_trait(
            program,
            type_name,
            trait_definition,
            arguments,
            diagnostics,
            &mut Vec::new(),
        );
    }
}

fn validate_data_satisfies_trait(
    program: &TypedTrees,
    type_name: &str,
    trait_definition: &TraitDefinition,
    explicit_type_arguments: &[omega_typed_trees::types::TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
    visited_traits: &mut Vec<omega_core::symbols::SymbolHandle>,
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
            attached_state_named(program, type_name, requirement.name.as_str())
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
            type_name,
            required_trait,
            &required_arguments,
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
    type_name: &str,
    state_name: &str,
) -> Option<(&'program Machine, &'program State)> {
    program
        .machines()
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == type_name)
        })
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == state_name)
                .map(|state| (machine, state))
        })
}
