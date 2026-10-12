//! Instantiate source-declared requirement applications before checking.
//!
//! Boundary families retain the public declaration's complete finite roster;
//! top-level source requirements carry each demanded tuple to their authored
//! realization through its declared telescope correspondence. Both use the
//! ordinary static application and callable substitution owners. Neither
//! discovers a provider from runtime carrier shapes or chooses a candidate.

use std::sync::Arc;

use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::finite_family::FamilyProbe;
use typed_trees::static_applications::{
    InstantiatedRequirementRealization, StaticApplicationDemand, StaticMachineBinding,
    StaticMachineBindingValue,
};

use super::body_cloning::copy_binding_type_payloads;
use super::demanded_applications;

mod calls;
mod demands;
mod trait_providers;

pub(crate) fn instantiate_boundary_requirement_families(
    program: &mut TypedTrees,
) -> Result<usize, Vec<Diagnostic>> {
    let previous = program.closed_requirement_applications().count();
    let demands = demands::collect(program);
    for demand in demands {
        let requirement = demand.requirement;
        let trait_arguments = program
            .materialize_trait_application_arguments(&demand.environment)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        let has_outer_operands = {
            let scope = demand.environment.scope(program).ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary family lost its complete source application",
                )]
            })?;
            if program.require_receiving_trait_operands(&scope).is_err()
                || scope.portable_application_identity(program).is_none()
            {
                // A local representation key alone cannot close a portable
                // requirement. Leave unresolved source applications to their
                // ordinary obligations, rather than issue an incomplete row.
                continue;
            }
            // This publishes a logical requirement under the retained source
            // telescope, not an executable provider. Regional correspondence
            // is qualified later and travels with the exact candidate.
            !trait_arguments.is_empty() || !scope.declaration.lifetime_parameters.is_empty()
        };
        let (_, signature) = program
            .trait_machine_signature(requirement)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary family lost its source requirement",
                )]
            })?;
        let name = signature.name.clone();
        let parameters = program.state_signature_type_parameters(signature).to_vec();
        let tuples = if parameters.is_empty() {
            if !has_outer_operands {
                continue;
            }
            vec![Vec::new()]
        } else {
            match program.finite_signature_family(signature) {
                FamilyProbe::Finite { tuples, .. } => tuples
                    .into_iter()
                    .map(|tuple| {
                        tuple
                            .identities
                            .iter()
                            .map(|identity| tuple_argument(program, identity))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                FamilyProbe::NotFinite(_) => continue,
            }
        };
        for arguments in tuples {
            let mut bindings = Vec::with_capacity(parameters.len());
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                let actual = program
                    .static_operand_type_reference(argument)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value: StaticMachineBindingValue::Const(actual),
                });
            }
            let application = program.expression_table.retain_static_machine_application(
                requirement,
                name.clone(),
                &arguments,
            );
            let source = program
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = if demand.argument_owner.is_valid() {
                demand.argument_owner
            } else {
                requirement
            };
            source.binding_declaration = requirement;
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            program
                .instantiate_requirement_application(
                    &demand.environment,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
    }
    let retargeted = calls::retarget(program)?;
    Ok(program.closed_requirement_applications().count() - previous + retargeted)
}

fn tuple_argument(
    program: &mut TypedTrees,
    identity: &str,
) -> Result<StaticMachineArgument, Vec<Diagnostic>> {
    let reference = super::dynamic_families::const_identity_type_reference(program, identity)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "boundary family has no represented canonical static operand",
            )]
        })?;
    typed_trees::static_applications::static_argument_from_type_reference(program, reference)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "boundary family lost its retained static operand meaning",
            )]
        })
}

pub(crate) struct RequirementApplicationTemplates {
    authored: TypedTrees,
}

impl RequirementApplicationTemplates {
    pub(crate) fn prepare(program: &TypedTrees) -> Self {
        Self {
            authored: program.clone(),
        }
    }

    fn instantiate_trait_requirements(
        &self,
        program: &mut TypedTrees,
    ) -> Result<usize, Vec<Diagnostic>> {
        trait_providers::instantiate(&self.authored, program)
    }

    pub(crate) fn instantiate(&self, program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
        // Immutable coordinates keep source declarations independent of the
        // ordinary fixed point's changing callers and already produced bodies.
        let requirements = program
            .machine_specializations
            .iter()
            .filter(|receipt| {
                self.authored
                    .machine(receipt.template)
                    .is_some_and(|requirement| {
                        requirement.supply_mode == MachineSupplyMode::TopLevelRequirement
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut produced = self.instantiate_trait_requirements(program)?;
        for receipt in requirements {
            let Some(requirement) = self.authored.machine(receipt.template) else {
                continue;
            };
            for provider in self.authored.machines().iter().filter(|provider| {
                provider.supplies_checked_body() && provider.has_specialization_parameters()
            }) {
                for (ordinal, conformance) in self
                    .authored
                    .machine_trait_conformances(provider)
                    .iter()
                    .enumerate()
                {
                    if conformance.symbol != requirement.symbol
                        || conformance.requirement_symbol != requirement.symbol
                        || conformance.external_binding.is_some()
                        || conformance.via_expression.is_valid()
                    {
                        continue;
                    }
                    let conformance_handle = provider.satisfies.handle_at(ordinal as u32);
                    if program
                        .instantiated_requirement_realizations
                        .iter()
                        .any(|(_, previous)| {
                            previous.conformance == conformance_handle
                                && previous.requirement_instance == receipt.instance
                                && previous.provider_template == provider.symbol
                        })
                    {
                        continue;
                    }
                    let required_parameters = self.authored.machine_type_parameters(requirement);
                    let supplied_parameters = self.authored.machine_type_parameters(provider);
                    if required_parameters.len() != supplied_parameters.len()
                        || requirement.conformance_bounds.len() != provider.conformance_bounds.len()
                        || required_parameters.iter().zip(supplied_parameters).any(
                            |(required, supplied)| {
                                !same_parameter_family(&required.kind, &supplied.kind)
                            },
                        )
                    {
                        return Err(vec![Diagnostic::error(
                            "declared requirement realization does not retain its complete universal telescope correspondence",
                        )]);
                    }
                    let mut correspondence = required_parameters
                        .iter()
                        .zip(supplied_parameters)
                        .map(|(required, supplied)| (required.symbol, supplied.symbol))
                        .collect::<Vec<_>>();
                    for (required, supplied) in requirement
                        .conformance_bounds
                        .iter()
                        .zip(&provider.conformance_bounds)
                    {
                        if let Some((_, mapped)) = correspondence
                            .iter()
                            .find(|(binder, _)| *binder == required.binder)
                        {
                            if *mapped != supplied.binder {
                                return Err(vec![Diagnostic::error(
                                    "declared requirement realization changes an evidence binder's source position",
                                )]);
                            }
                        } else {
                            correspondence.push((required.binder, supplied.binder));
                        }
                    }
                    let mut bindings = program
                        .expression_table
                        .static_machine_application_bindings(receipt.binding_application)
                        .to_vec();
                    if bindings.len() != correspondence.len() {
                        return Err(vec![Diagnostic::error(
                            "declared requirement realization lost a complete demanded substitution",
                        )]);
                    }
                    for binding in &mut bindings {
                        let Some((_, supplied)) = correspondence
                            .iter()
                            .find(|(required, _)| *required == binding.parameter)
                        else {
                            return Err(vec![Diagnostic::error(
                                "declared requirement realization substituted a binder outside its source edge",
                            )]);
                        };
                        binding.parameter = *supplied;
                    }
                    let mut source = self.authored.clone();
                    copy_binding_type_payloads(Some(program), &mut source, &mut bindings, &[]);
                    let original = program
                        .expression_table
                        .static_machine_application(receipt.binding_application);
                    let application = source.expression_table.retain_static_machine_application(
                        provider.symbol,
                        provider.name.clone(),
                        &[],
                    );
                    let row = source
                        .expression_table
                        .static_machine_application_mut(application);
                    row.binding_declaration = provider.symbol;
                    row.lexical_owner = original.lexical_owner;
                    if original.captured_lexical_scope.is_valid() {
                        let scope = program
                            .captured_static_lexical_scope(original.captured_lexical_scope)
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "declared realization lost the demanded lexical capture",
                                )]
                            })?;
                        let captured = super::captured_lexical_scopes::retain(&mut source, scope)
                            .map_err(|diagnostic| vec![diagnostic])?;
                        source
                            .expression_table
                            .static_machine_application_mut(application)
                            .captured_lexical_scope = captured;
                    }
                    source
                        .expression_table
                        .set_static_machine_application_bindings(application, &bindings);
                    if requirement.lifetime_parameters.len() != provider.lifetime_parameters.len() {
                        return Err(vec![Diagnostic::error(
                            "declared requirement realization changes the invocation lifetime telescope",
                        )]);
                    }
                    let mut lifetimes = program
                        .expression_table
                        .static_machine_application_lifetimes(receipt.binding_application)
                        .to_vec();
                    for binding in &mut lifetimes {
                        let Some(ordinal) = requirement
                            .lifetime_parameters
                            .iter()
                            .position(|parameter| parameter == &binding.parameter)
                        else {
                            return Err(vec![Diagnostic::error(
                                "declared requirement realization maps a lifetime outside its source telescope",
                            )]);
                        };
                        binding.parameter = provider.lifetime_parameters[ordinal].clone();
                    }
                    source
                        .expression_table
                        .set_static_machine_application_lifetimes(application, &lifetimes);
                    let demand = StaticApplicationDemand {
                        declaration: provider.symbol,
                        owner: Arc::new(source.into()),
                        application,
                    };
                    demanded_applications::instantiate_one(program, &demand)?;
                    let instance = program.instantiated_static_applications()
                        .find(|(_, produced)| Arc::ptr_eq(&produced.demand.owner, &demand.owner)
                            && produced.demand.application == demand.application)
                        .map(|(_, produced)| produced.instance)
                        .ok_or_else(|| vec![Diagnostic::error(
                            "declared requirement realization lost its produced application coordinate",
                        )])?;
                    let instance_conformance = instance_conformance(program, instance, ordinal)?;
                    program.instantiated_requirement_realizations.append(
                        InstantiatedRequirementRealization {
                            conformance: conformance_handle,
                            instance_conformance,
                            named_conformance: Default::default(),
                            realization_state: Default::default(),
                            requirement_template: requirement.symbol,
                            requirement_instance: receipt.instance,
                            provider_template: provider.symbol,
                            provider_instance: instance,
                        },
                    );
                    produced += 1;
                }
            }
        }
        Ok(produced)
    }
}

fn same_parameter_family(required: &TypeParameterKind, supplied: &TypeParameterKind) -> bool {
    std::mem::discriminant(required) == std::mem::discriminant(supplied)
}

/// The body copier preserves the original edge order while allocating its
/// receiving span. Retain that actual coordinate; no display-name search can
/// recover this correspondence after two equivalent edges have been copied.
fn instance_conformance(
    program: &TypedTrees,
    provider: symbols::SymbolHandle,
    ordinal: usize,
) -> Result<arena::Handle<typed_trees::machine::TraitConformance>, Vec<Diagnostic>> {
    let machine = program.machine(provider).ok_or_else(|| {
        vec![Diagnostic::error(
            "declared provider application lost its produced machine",
        )]
    })?;
    if ordinal >= program.machine_trait_conformances(machine).len() {
        return Err(vec![Diagnostic::error(
            "declared provider application lost its produced conformance edge",
        )]);
    }
    Ok(machine.satisfies.handle_at(ordinal as u32))
}
