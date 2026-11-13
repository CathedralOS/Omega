// Provider declaration and realization-row matching. Named implementations own
// a selection rather than a hidden receiver value; independent exact
// satisfactions retain the member's own trait application instead of replacing
// that edge with the map's primary telescope.

use crate::ProviderPlanningContext;
use crate::provider_planning::entry_state;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};
use symbols::SymbolHandle;
use typed_trees::trait_definition::Conformance;

pub(crate) use build_time_evaluation::provider_declarations::{
    eligible, named_requirement_lifetime_arguments,
};

pub(crate) fn named_provider<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
) -> Option<&'a Conformance> {
    let typed = context.typed;
    let mut matching = typed.conformances().iter().filter(|definition| {
        (plan.application_identity().is_some() || eligible(definition))
            && context.contains(definition.symbol)
            && typed.symbols.display_path(definition.symbol, "::") == plan.provider_type
            && typed.symbols.symbol_package_identity(definition.symbol)
                == plan.provider_type_package_identity
    });
    let result = matching.next()?;
    matching.next().is_none().then_some(result)
}

pub enum ProviderTraitApplication<'a> {
    Named {
        declaration: &'a Conformance,
        lifetime_arguments: &'a [u32],
    },
    Machine(&'a typed_trees::machine::TraitConformance),
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
    machine: &typed_trees::machine::Machine,
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
        if rows
            .iter()
            .filter(|entry| {
                entry.declaring_trait == declaring_trait && entry.requirement == requirement
            })
            .count()
            == 1
        {
            let lifetime_arguments =
                named_requirement_lifetime_arguments(typed, implementation, declaring_trait)?;
            return Some(ProviderTraitApplication::Named {
                declaration: implementation,
                lifetime_arguments,
            });
        }
    }
    let mut matching = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement: signature,
            }) = typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)
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

pub fn provider_trait_requirement_matches(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &typed_trees::machine::Machine,
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

pub(crate) fn named_provider_row<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Option<&'a typed_trees::trait_definition::ConformanceRow> {
    let typed = context.typed;
    let definition = named_provider(context, plan)?;
    let mut matching = definition.rows.iter().filter(|entry| {
        let own_identity = typed
            .trait_definition_by_symbol(entry.declaring_trait)
            .and_then(|owner| {
                typed
                    .trait_machine_signature_by_symbol(owner, entry.requirement)
                    .map(|requirement| {
                        typed.trait_requirement_callable_identity_text(owner, requirement)
                    })
            });
        if own_identity.as_deref() == Some(plan.identities.text(row.requirement_identity)) {
            return true;
        }
        let Some(member) = typed.machine(entry.realization_machine) else {
            return false;
        };
        if typed_trees::provider_declarations::satisfaction_provider_declaration(typed, member)
            != Some(definition.symbol)
        {
            return false;
        }
        typed
            .machine_trait_conformances(member)
            .iter()
            .filter_map(|edge| {
                typed_trees::provider_declarations::boundary_satisfaction_identity(
                    typed, member, edge,
                )
            })
            .filter(|(requirement, identity)| {
                context.contains(*requirement)
                    && typed.symbols.symbol_package_identity(*requirement)
                        == plan.schema.declaration.trait_package_identity
                    && identity == plan.identities.text(row.requirement_identity)
            })
            .count()
            == 1
    });
    let entry = matching.next()?;
    (matching.next().is_none() && context.contains(entry.realization_machine)).then_some(entry)
}

pub fn provider_row_matches(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &typed_trees::machine::Machine,
) -> bool {
    let typed = context.typed;
    if !context.contains(machine.symbol) {
        return false;
    }
    if let Some(implementation) = named_provider(context, plan) {
        let Some(entry) = named_provider_row(context, plan, row) else {
            return false;
        };
        let Some(realization) = typed.machine(entry.realization_machine) else {
            return false;
        };
        let realization_states = typed.machine_states(realization);
        if !realization_states
            .first()
            .is_some_and(|state| state.symbol == entry.realization_state)
        {
            return false;
        }
        if plan.application_identity().is_none()
            && crate::target_machines::realized_row_machine(
                context,
                entry.realization_machine,
                Some(&plan.target),
            )
            .is_some_and(|realized| realized.symbol == machine.symbol)
        {
            return true;
        }
        let Some(entry_state) = entry_state(typed, machine) else {
            return false;
        };
        if !typed.instantiated_machine_state_matches(
            machine.symbol,
            entry.realization_machine,
            entry.realization_state,
            entry_state.symbol,
        ) {
            return false;
        }
        match plan.application_identity() {
            Some(expected) => {
                typed
                    .implementation_capture(
                        machine.symbol,
                        entry.realization_machine,
                        implementation.symbol,
                    )
                    .and_then(|capture| {
                        typed.portable_conformance_application_identity(
                            &capture.conformance_application,
                            capture.supplying_application,
                        )
                    })
                    .and_then(|identity| typed.type_identity_text(identity))
                    .as_deref()
                    == Some(expected)
            }
            None => typed
                .implementation_capture(
                    machine.symbol,
                    entry.realization_machine,
                    implementation.symbol,
                )
                .is_none(),
        }
    } else {
        machine.attached_data_symbol.is_valid()
            && context.contains(machine.attached_data_symbol)
            && typed.attached_data_path(machine).as_deref() == Some(plan.provider_type.as_str())
            && typed
                .symbols
                .symbol_package_identity(machine.attached_data_symbol)
                == plan.provider_type_package_identity
    }
}
