//! Exact provider declarations and their independently owned realization rows.
//! A named implementation owns a selection, never a hidden receiver value.

use crate::ProviderPlanningContext;
use checked_trees::typed_trees::trait_definition::Conformance;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};
use symbols::SymbolHandle;

mod receiving_application;
pub use receiving_application::{ProviderReceiverApplication, provider_receiver_application};

pub(crate) use build_time_evaluation::provider_declarations::{
    eligible, named_requirement_lifetime_arguments,
};

pub(crate) fn named_provider<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
) -> Option<&'a Conformance> {
    let typed = context.typed;
    let mut matching = typed.conformances().iter().filter(|definition| {
        eligible(definition)
            && context.contains(definition.symbol)
            && typed.symbols.display_path(definition.symbol, "::") == plan.provider_type
            && typed.symbols.symbol_package_identity(definition.symbol)
                == plan.provider_type_package_identity
    });
    let result = matching.next()?;
    matching.next().is_none().then_some(result)
}

/// Borrow the declaration that owns one exact provider requirement application.
/// This view retains the original lifetime substitution without inventing a
/// machine edge for a closed implementation map.
pub enum ProviderTraitApplication<'a> {
    Named {
        declaration: &'a Conformance,
        lifetime_arguments: &'a [u32],
    },
    Machine(&'a checked_trees::typed_trees::machine::TraitConformance),
}

impl ProviderTraitApplication<'_> {
    pub fn lifetime_arguments(&self) -> &[u32] {
        match self {
            Self::Named {
                lifetime_arguments, ..
            } => lifetime_arguments,
            Self::Machine(application) => &application.trait_lifetime_arguments,
        }
    }
}

pub fn provider_trait_application<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &checked_trees::typed_trees::machine::Machine,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
) -> Option<ProviderTraitApplication<'a>> {
    let typed = context.typed;
    if !context.contains(machine.symbol)
        || !context.contains(declaring_trait)
        || !context.contains(requirement)
    {
        return None;
    }
    if let Some(implementation) = named_provider(context, plan) {
        if !provider_row_matches(context, plan, row, machine) {
            return None;
        }
        let rows = &implementation.rows;
        let lifetime_arguments =
            named_requirement_lifetime_arguments(typed, implementation, declaring_trait)?;
        return (rows
            .iter()
            .filter(|entry| {
                entry.declaring_trait == declaring_trait && entry.requirement == requirement
            })
            .count()
            == 1)
            .then_some(ProviderTraitApplication::Named {
                declaration: implementation,
                lifetime_arguments,
            });
    }
    let mut matching = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            let Some(checked_trees::typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement: signature,
            }) = checked_trees::typed_trees::machine::resolve_satisfied_declaration(
                typed,
                machine,
                conformance,
            )
            else {
                return false;
            };
            definition.symbol == declaring_trait
                && signature.symbol == requirement
                && typed.trait_requirement_callable_identity_text(definition, signature)
                    == plan.identities.text(row.requirement_identity)
        });
    let application = matching.next()?;
    matching
        .next()
        .is_none()
        .then_some(ProviderTraitApplication::Machine(application))
}

/// Authenticate a checked adapter's association at its declaration owner.
pub fn provider_trait_requirement_matches(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &checked_trees::typed_trees::machine::Machine,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    provider_row_matches(context, plan, row, machine)
        && provider_trait_application(context, plan, row, machine, declaring_trait, requirement)
            .is_some_and(|application| match application {
                ProviderTraitApplication::Named { .. } => true,
                ProviderTraitApplication::Machine(edge) => edge.external_binding.is_none(),
            })
}

/// The one row of a named provider's map that a plan row installs, joined by
/// the requirement's exact callable identity. `None` when the plan is not a
/// named provider's, or when zero or several rows (or an unretained
/// realization) answer the identity.
pub(crate) fn named_provider_row<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Option<&'a checked_trees::typed_trees::trait_definition::ConformanceRow> {
    let typed = context.typed;
    let definition = named_provider(context, plan)?;
    let mut matching = definition.rows.iter().filter(|entry| {
        let Some(owner) = typed.trait_definition_by_symbol(entry.declaring_trait) else {
            return false;
        };
        let Some(requirement) = typed.trait_machine_signature_by_symbol(owner, entry.requirement)
        else {
            return false;
        };
        typed.trait_requirement_callable_identity_text(owner, requirement)
            == plan.identities.text(row.requirement_identity)
    });
    let entry = matching.next()?;
    (matching.next().is_none() && context.contains(entry.realization_machine)).then_some(entry)
}

/// Rejoin a provider row to its declaration, including an exact specialization
/// of the map's realization. Physical machine ownership is not map ownership.
pub fn provider_row_matches(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    let typed = context.typed;
    if !context.contains(machine.symbol) {
        return false;
    }
    if named_provider(context, plan).is_some() {
        let Some(entry) = named_provider_row(context, plan, row) else {
            return false;
        };
        let Ok(realization) =
            crate::target_machines::named_row_realization(context, entry, Some(&plan.target))
        else {
            return false;
        };
        let realization_states = typed.machine_states(realization);
        if realization.symbol == machine.symbol {
            return true;
        }
        // The specialization producer clones states in declaration order;
        // its receipt authenticates that correspondence. Rejoin the map's
        // exact entry before accepting the corresponding instance entry.
        if typed.machine_states(machine).len() != realization_states.len() {
            return false;
        }
        typed
            .machine_specializations
            .iter()
            .filter(|specialization| {
                specialization.template == realization.symbol
                    && specialization.instance == machine.symbol
            })
            .count()
            == 1
    } else {
        let direct_attachment = machine.attached_data_symbol.is_valid()
            && context.contains(machine.attached_data_symbol)
            && typed.attached_data_path(machine).as_deref() == Some(plan.provider_type.as_str())
            && typed
                .symbols
                .symbol_package_identity(machine.attached_data_symbol)
                == plan.provider_type_package_identity;
        let trait_member = typed
            .machine_trait_conformances(machine)
            .iter()
            .any(|edge| {
                typed
                    .trait_machine_signature(edge.requirement_symbol)
                    .is_some_and(|(declaration, requirement)| {
                        typed.trait_requirement_callable_identity_text(declaration, requirement)
                            == plan.identities.text(row.requirement_identity)
                    })
            });
        if !trait_member {
            // Operator and top-level requirement rows keep their own genuine
            // application owners; they have no whole boundary receiver roster.
            return direct_attachment;
        }
        let original = direct_attachment
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .enumerate()
                .any(|(ordinal, _)| {
                    typed
                        .machine_conformance_sources(
                            machine,
                            machine.satisfies.handle_at(ordinal as u32),
                        )
                        .into_iter()
                        .any(|source| {
                            source.family.symbol == machine.symbol
                                && source.provider == machine.attached_data_symbol
                                && source_edge_matches_row(typed, plan, row, &source)
                                && crate::service_schema::source_receiver_matches_schema(
                                    typed,
                                    source.scope,
                                    &plan.schema,
                                )
                        })
                });
        if original {
            return true;
        }
        // A checked adapter is the original universal callable. A changed
        // attached provider is supplied only by its issued specialization edge,
        // with complete receiver identity under that original occurrence.
        typed
            .machines()
            .iter()
            .filter(|instance| {
                instance.symbol != machine.symbol
                    && typed
                        .authored_conformance_machine(instance)
                        .is_ok_and(|family| family.symbol == machine.symbol)
            })
            .filter(|instance| {
                context.contains(instance.symbol)
                    && instance.attached_data_symbol.is_valid()
                    && context.contains(instance.attached_data_symbol)
                    && typed.attached_data_path(instance).as_deref()
                        == Some(plan.provider_type.as_str())
                    && typed
                        .symbols
                        .symbol_package_identity(instance.attached_data_symbol)
                        == plan.provider_type_package_identity
            })
            .any(|instance| {
                typed
                    .machine_trait_conformances(instance)
                    .iter()
                    .enumerate()
                    .any(|(ordinal, _)| {
                        typed
                            .machine_conformance_sources(
                                instance,
                                instance.satisfies.handle_at(ordinal as u32),
                            )
                            .into_iter()
                            .any(|source| {
                                source.family.symbol == machine.symbol
                                    && source.provider == instance.attached_data_symbol
                                    && source_edge_matches_row(typed, plan, row, &source)
                                    && crate::service_schema::source_receiver_matches_schema(
                                        typed,
                                        source.scope,
                                        &plan.schema,
                                    )
                            })
                    })
            })
    }
}

fn source_edge_matches_row(
    typed: &checked_trees::typed_trees::TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    source: &checked_trees::typed_trees::typed_trees::declarations::machine_conformances::MachineConformanceSource<'_>,
) -> bool {
    source.edge.requirement_symbol.is_valid()
        && typed
            .trait_machine_signature(source.edge.requirement_symbol)
            .is_some_and(|(declaration, requirement)| {
                typed.trait_requirement_callable_identity_text(declaration, requirement)
                    == plan.identities.text(row.requirement_identity)
            })
}
