//! Both source supplier declarations use one closed application producer.
//!
//! A machine edge retains its original and copied conformance handles. A
//! named map retains its declaration and exact realization entry instead:
//! it does not manufacture a machine satisfaction edge. The same ordinary
//! application inference and specialization close both source tuples before
//! checking. These associations declare candidates, never elect suppliers.

use arena::Handle;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::{Machine, TraitConformance};
use typed_trees::static_applications::{
    InstantiatedRequirementRealization, InstantiatedRequirementSignature, StaticApplicationDemand,
};

#[derive(Clone, Copy)]
enum ProviderSource {
    Machine {
        conformance: Handle<TraitConformance>,
        ordinal: usize,
    },
    Named {
        conformance: Handle<typed_trees::trait_definition::Conformance>,
        state: SymbolHandle,
    },
}

pub(super) fn instantiate(
    authored: &TypedTrees,
    program: &mut TypedTrees,
) -> Result<usize, Vec<Diagnostic>> {
    super::super::requirement_signatures::complete_interface_headers(program)?;
    let requirements = program
        .instantiated_requirement_signatures
        .iter()
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut produced = 0;
    for requirement in requirements {
        for provider in authored.machines().iter().filter(|provider| {
            provider.supplies_checked_body()
                    // A produced body has consumed its telescope. Its exact
                    // receipt remains an application, never another source
                    // declaration of the universal requirement family.
                    && authored.machine_specialization(provider.symbol).is_none()
        }) {
            for (ordinal, edge) in authored
                .machine_trait_conformances(provider)
                .iter()
                .enumerate()
            {
                if edge.requirement_symbol != requirement.requirement_template
                    || edge.external_binding.is_some()
                    || edge.via_expression.is_valid()
                {
                    continue;
                }
                produced += usize::from(instantiate_provider(
                    authored,
                    program,
                    provider,
                    edge,
                    &requirement,
                    ProviderSource::Machine {
                        conformance: provider.satisfies.handle_at(ordinal as u32),
                        ordinal,
                    },
                )?);
            }
        }
        if !authored
            .trait_definition_by_symbol(requirement.declaring_trait)
            .is_some_and(|definition| definition.is_boundary)
        {
            continue;
        }
        for (handle, declaration) in authored.conformances.iter() {
            let mut rows = declaration.rows.iter().filter(|row| {
                row.declaring_trait == requirement.declaring_trait
                    && row.requirement == requirement.requirement_template
            });
            let Some(row) = rows.next() else {
                continue;
            };
            if rows.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "named provider repeats an original requirement row",
                )]);
            }
            let provider = authored.machine(row.realization_machine).ok_or_else(|| {
                vec![Diagnostic::error(
                    "named provider lost its original realization machine",
                )]
            })?;
            if !authored
                .machine_states(provider)
                .first()
                .is_some_and(|entry| entry.symbol == row.realization_state)
            {
                return Err(vec![Diagnostic::error(
                    "named provider lost its original realization entry",
                )]);
            }
            // A portable head supplies the source family contract, not a
            // private body. The ordinary application producer closes its full
            // retained roster; checking then checks every member independently.
            // Opaque boundaries have neither source obligation and stay out.
            if !provider.supplies_checked_body() && !provider.target_family_interface {
                continue;
            }
            // This scratch view projects the map's actual source tuple into
            // the existing inference owner; the durable source remains the
            // named declaration and row, never a synthesized satisfies edge.
            let edge = TraitConformance {
                symbol: declaration.trait_symbol,
                name: declaration.trait_name.clone(),
                trait_lifetime_arguments: declaration.trait_lifetime_arguments.clone(),
                arguments: declaration.arguments,
                requirement: Some(row.requirement_name.clone()),
                requirement_symbol: row.requirement,
                ..Default::default()
            };
            produced += usize::from(instantiate_provider(
                authored,
                program,
                provider,
                &edge,
                &requirement,
                ProviderSource::Named {
                    conformance: handle,
                    state: row.realization_state,
                },
            )?);
        }
    }
    Ok(produced)
}

fn instantiate_provider(
    authored: &TypedTrees,
    program: &mut TypedTrees,
    provider: &Machine,
    edge: &TraitConformance,
    requirement: &InstantiatedRequirementSignature,
    source: ProviderSource,
) -> Result<bool, Vec<Diagnostic>> {
    let requirement_instance = program
        .trait_machine_signatures
        .get(requirement.signature)
        .symbol;
    let (conformance, named_conformance, realization_state) = match source {
        ProviderSource::Machine { conformance, .. } => {
            (conformance, Default::default(), Default::default())
        }
        ProviderSource::Named { conformance, state } => (Default::default(), conformance, state),
    };
    if program
        .instantiated_requirement_realizations
        .iter()
        .any(|(_, previous)| {
            previous.conformance == conformance
                && previous.named_conformance == named_conformance
                && previous.realization_state == realization_state
                && previous.requirement_instance == requirement_instance
                && previous.provider_template == provider.symbol
        })
    {
        return Ok(false);
    }
    let required_bindings = program
        .expression_table
        .static_machine_application_bindings(requirement.declaring_binding_application);
    let mut payload = authored.clone();
    let (required_bindings, application) =
        super::super::demanded_applications::copy_application_payload(
            &mut payload,
            program,
            requirement.requirement_template,
            requirement.declaring_binding_application,
            required_bindings,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    let Some(selected) = super::super::trait_provider_applications::provider_application(
        &mut payload,
        provider,
        edge,
        named_conformance,
        requirement.declaring_trait,
        requirement.requirement_template,
        application,
        &required_bindings,
    )
    .map_err(|diagnostic| vec![diagnostic])?
    else {
        return Ok(false);
    };
    let original = payload
        .expression_table
        .static_machine_application(application)
        .clone();
    let application = payload.expression_table.retain_static_machine_application(
        provider.symbol,
        provider.name.clone(),
        &[],
    );
    let tuple = payload
        .expression_table
        .static_machine_application_mut(application);
    tuple.binding_declaration = provider.symbol;
    tuple.lexical_owner = original.lexical_owner;
    tuple.captured_lexical_scope = original.captured_lexical_scope;
    payload
        .expression_table
        .set_static_machine_application_bindings(application, &selected.bindings);
    payload
        .expression_table
        .set_static_machine_application_lifetimes(application, &selected.lifetimes);
    let demand = StaticApplicationDemand {
        declaration: provider.symbol,
        owner: Arc::new(payload.into()),
        application,
    };
    super::super::demanded_applications::instantiate_one(program, &demand)?;
    let provider_instance = program
        .instantiated_static_applications()
        .find(|(_, produced)| {
            Arc::ptr_eq(&produced.demand.owner, &demand.owner)
                && produced.demand.application == demand.application
        })
        .map(|(_, produced)| produced.instance)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "declared trait provider lost its exact produced application",
            )]
        })?;
    let instance_conformance = match source {
        ProviderSource::Machine { ordinal, .. } => {
            super::instance_conformance(program, provider_instance, ordinal)?
        }
        ProviderSource::Named { .. } => Default::default(),
    };
    program
        .instantiated_requirement_realizations
        .append(InstantiatedRequirementRealization {
            conformance,
            instance_conformance,
            named_conformance,
            realization_state,
            requirement_template: requirement.requirement_template,
            requirement_instance,
            provider_template: provider.symbol,
            provider_instance,
        });
    Ok(true)
}
