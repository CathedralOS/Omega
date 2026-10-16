//! Close checked boundary suppliers from complete public invocation tuples.
//!
//! Source satisfaction edges, not Build choices, own candidate membership. Every
//! candidate receives the same explicit local telescope as the public contract;
//! ordinary specialization retains its operands and immutable checking judges
//! conformance. No private body supplies missing arguments or public premises.

use super::{Diagnostic, TypedTrees, candidate, explicit_arguments, saved_calls};
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(crate) fn generate_requirement_provider_specializations(
    program: &mut TypedTrees,
) -> Result<usize, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    for (index, machine) in program.machines().iter().enumerate() {
        if !machine.supplies_checked_body() || !machine.has_specialization_parameters() {
            continue;
        }
        for conformance in program.machine_trait_conformances(machine) {
            if conformance.external_binding.is_some() || conformance.via_expression.is_valid() {
                continue;
            }
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)
            else {
                continue;
            };
            if !definition.is_boundary {
                continue;
            }
            for application in program.closed_requirement_applications() {
                if application.template == requirement.symbol
                    && typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                        program,
                        machine,
                        conformance,
                    )
                    .is_some_and(|scope| {
                        program.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                    })
                {
                    demands.push((index, application.application));
                }
            }
        }
    }
    let mut generated = 0;
    for (index, application) in demands {
        let mut provider = candidate::from_machine(program, index);
        let machine = &program.machines()[index];
        let Some(state) = program.machine_states(machine).first().cloned() else {
            continue;
        };
        let arguments = program
            .expression_table
            .static_machine_application_arguments(application)
            .to_vec();
        explicit_arguments::bind(program, &mut provider, &state, state.symbol, &arguments)?;
        provider.binding_application = application;
        if provider.conflicted
            || provider.type_bindings.iter().any(Option::is_none)
            || provider.const_bindings.iter().any(Option::is_none)
            || provider.machine_bindings.iter().any(Option::is_none)
            || provider.evidence_bindings.iter().any(Option::is_none)
        {
            continue;
        }
        if saved_calls::candidate_instance(program, program, &provider).is_some() {
            continue;
        }
        let identity = normalized_machine_identity(program, &program.machines()[index])
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary provider has no normalized declaration identity",
                )]
            })?;
        let commitment = accepted_template_commitment(program, index);
        let ordinal = program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == provider.template.template_symbol)
            .count();
        super::body_cloning::clone_specialized_machine(
            None, program, &provider, ordinal, identity, commitment,
        )?;
        generated += 1;
    }
    Ok(generated)
}
