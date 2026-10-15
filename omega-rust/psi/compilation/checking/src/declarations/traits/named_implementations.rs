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
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::trait_definition::{AppliedTraitScope, ConformanceRowSource};

pub(crate) fn validate_conformances(
    program: &dyn crate::CheckingInput,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    symbols: &crate::declarations::symbols::TopLevelSymbols<'_>,
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

        let applied_lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| conformance.lifetime_parameters[*ordinal as usize].clone())
            .collect::<Vec<_>>();
        super::conformance::validate_trait_application_obligations(
            program,
            trait_definition,
            arguments,
            &applied_lifetimes,
            &format!("conformance `{implementation_name} satisfies {trait_name}`"),
            symbols,
            crate::value_custody::type_references::TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: conformance.symbol,
                type_parameters: program
                    .data_type_parameters
                    .span_or_empty(conformance.type_parameters),
                lifetime_parameters: &conformance.lifetime_parameters,
            },
            diagnostics,
        );

        validate_closed_rows(
            program,
            service_reaches,
            implementation_name,
            trait_definition,
            &conformance.rows,
            conformance_name,
            conformance,
            diagnostics,
        );
    }
}

fn validate_closed_rows(
    program: &dyn crate::CheckingInput,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    implementation_name: &str,
    root_trait: &TraitDefinition,
    rows: &[typed_trees::trait_definition::ConformanceRow],
    conformance_name: Option<&str>,
    conformance: &typed_trees::trait_definition::Conformance,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(root_application) =
        AppliedTraitScope::from_conformance(program.typed_program(), conformance)
    else {
        return;
    };
    for row in rows {
        let Some(declaring_trait) = trait_definition_by_symbol(program, row.declaring_trait) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` retains an unresolved declaring trait for row `{}`",
                root_trait.name, row.requirement_name
            )));
            continue;
        };
        let Some(requirement) =
            program.trait_machine_signature_by_symbol(declaring_trait, row.requirement)
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
        let Some(machine) = program.machine(row.realization_machine) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization machine",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        let Some(state) = program.state_in_machine(machine, row.realization_state) else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` row `{}::{}` retains no exact realization state",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        let Some(application) =
            root_application.declaring_scope(program.typed_program(), row.declaring_trait)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "closed conformance `{implementation_name} satisfies {}` cannot instantiate inherited row `{}::{}`",
                root_trait.name, row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        super::conformance::validate_applied_trait_signature(
            program,
            service_reaches,
            machine,
            state,
            &application,
            requirement,
            diagnostics,
        );
        crate::proof_contracts::contract_entailment::check_law_conformance(
            program,
            machine,
            conformance_name,
            &application,
            requirement,
            diagnostics,
            &mut Vec::new(),
        );
    }
}
