//! Close checked boundary suppliers from complete public invocation tuples.
//!
//! Source satisfaction edges, not Build choices, own candidate membership. Every
//! candidate receives the same explicit local telescope as the public contract;
//! ordinary specialization retains its operands and immutable checking judges
//! conformance. No private body supplies missing arguments or public premises.

use super::{Diagnostic, TypedTrees, candidate, failed, saved_calls};
use crate::tables::expressions_of;
use typed_trees::trait_definition::AppliedTraitScope;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

pub(crate) fn generate_requirement_provider_specializations(
    program: &mut TypedTrees,
) -> Result<usize, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    for (index, machine) in program.machines().iter().enumerate() {
        if !machine.supplies_checked_body() || !machine.has_specialization_parameters() {
            continue;
        }
        // Both source forms declare candidate membership. Named map rows do
        // not also carry a machine satisfaction edge; requiring one silently
        // omitted their local generic instances. Gather their exact declaring
        // scopes before using the shared tuple specialization path.
        let mut requirements = Vec::new();
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
            if let Some(scope) =
                AppliedTraitScope::from_machine_conformance(program, machine, conformance)
            {
                requirements.push((requirement.symbol, scope));
            }
        }
        for conformance in program.conformances() {
            if !conformance.type_parameters.is_empty() {
                continue;
            }
            let Some(scope) = AppliedTraitScope::from_conformance(program, conformance) else {
                continue;
            };
            for row in &conformance.rows {
                if row.realization_machine != machine.symbol {
                    continue;
                }
                let Some(declaring) = scope.declaring_scope(program, row.declaring_trait) else {
                    continue;
                };
                if declaring.declaration.is_boundary {
                    requirements.push((row.requirement, declaring));
                }
            }
        }
        for (requirement, scope) in requirements {
            for application in program.closed_requirement_applications() {
                if application.template == requirement
                    && program
                        .requirement_application_matches_scope(application.signature.symbol, &scope)
                {
                    demands.push((index, application.signature.symbol));
                }
            }
        }
    }
    let mut generated = 0;
    for (index, requirement) in demands {
        let application = program
            .closed_requirement_application(requirement)
            .ok_or_else(|| failed("provider projection lost its exact requirement instance"))?;
        let mut provider = candidate::from_machine(program, index);
        let bindings =
            projected_provider_bindings(program, &program.machines()[index], &application)?;
        if !super::applications::bind_typed_tuple(program, &mut provider, &bindings) {
            return Err(failed(
                "provider projection does not match its complete declaration telescope",
            ));
        }
        provider.binding_application = application.application;
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
            .ok_or_else(|| failed("boundary provider has no normalized declaration identity"))?;
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

/// Public invocation coordinates and private provider binders have independent
/// namespaces. Refinement relates their full telescopes; preparation transports
/// that same ordinal/kind map without parsing a representative caller operand.
fn projected_provider_bindings(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    application: &typed_trees::typed_trees::ClosedRequirementApplication<'_>,
) -> Result<Vec<typed_trees::static_applications::StaticMachineBinding>, Vec<Diagnostic>> {
    use typed_trees::data::TypeParameterKind;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    use typed_trees::typed_trees::RequirementLocalArgument;
    let (_, declared) = program
        .trait_machine_signature(application.template)
        .ok_or_else(|| failed("provider projection lost its public declaration"))?;
    let public = program.state_signature_type_parameters(declared);
    let private = program.requirement_callable_parameters(
        application.signature,
        &program.machine_callable_parameters(machine),
    );
    let retained =
        expressions_of(program).static_machine_application_bindings(application.application);
    if public.len() != private.len()
        || public.len() != application.local_arguments.len()
        || retained.len() != public.len()
    {
        return Err(failed(
            "provider projection has an incomplete public or private invocation telescope",
        ));
    }
    public
        .iter()
        .zip(private)
        .zip(application.local_arguments)
        .map(|((public, private), argument)| {
            if std::mem::discriminant(&public.kind) != std::mem::discriminant(&private.kind) {
                return Err(failed(
                    "provider projection changed an invocation binder kind",
                ));
            }
            let mut source = retained
                .iter()
                .filter(|binding| binding.parameter == public.symbol);
            let binding = source
                .next()
                .filter(|_| source.next().is_none())
                .ok_or_else(|| {
                    failed("provider projection has a missing or duplicate source binding")
                })?;
            let value = match (&private.kind, argument, &binding.value) {
                (
                    TypeParameterKind::Value { .. },
                    RequirementLocalArgument::RuntimeValue { carrier },
                    StaticMachineBindingValue::RuntimeValue { .. },
                ) => StaticMachineBindingValue::RuntimeValue {
                    type_reference: *carrier,
                    subject: Default::default(),
                    expression: Default::default(),
                },
                (
                    TypeParameterKind::Type,
                    RequirementLocalArgument::Static(actual),
                    StaticMachineBindingValue::Type(_),
                ) => StaticMachineBindingValue::Type(*actual),
                (
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                    RequirementLocalArgument::Static(actual),
                    StaticMachineBindingValue::Const(_),
                ) => StaticMachineBindingValue::Const(*actual),
                (
                    TypeParameterKind::Machine { .. },
                    RequirementLocalArgument::Static(_),
                    StaticMachineBindingValue::Machine(actual),
                ) => StaticMachineBindingValue::Machine(actual.clone()),
                (
                    TypeParameterKind::Implementation,
                    RequirementLocalArgument::Static(_),
                    StaticMachineBindingValue::Conformance(actual),
                ) => StaticMachineBindingValue::Conformance(actual.clone()),
                _ => {
                    return Err(failed(
                        "provider projection changed its retained operand role",
                    ));
                }
            };
            Ok(StaticMachineBinding {
                parameter: private.symbol,
                value,
            })
        })
        .collect()
}
