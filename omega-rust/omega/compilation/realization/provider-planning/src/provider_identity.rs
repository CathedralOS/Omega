//! Exact provider declarations and their independently owned realization rows.
//! A named implementation owns a selection, never a hidden receiver value.

use crate::ProviderPlanningContext;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    trait_definition::{Conformance, ConformanceImplementation, ConformanceSubject},
};

pub(crate) fn is_named_provider(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
    typed
        .conformances()
        .iter()
        .any(|definition| definition.symbol == symbol && eligible(definition))
}

pub(crate) fn eligible(definition: &Conformance) -> bool {
    matches!(definition.subject, ConformanceSubject::Subjectless)
        && matches!(
            definition.implementation,
            ConformanceImplementation::Closed { .. }
        )
        && definition.type_parameters.is_empty()
        && definition.lifetime_parameters.is_empty()
}

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

/// Authenticate the selected trait association at its declaration owner.
/// A closed map owns its rows; a nominal provider owns authored machine edges.
pub fn provider_trait_requirement_matches(
    context: ProviderPlanningContext<'_>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &typed_trees::machine::Machine,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    let typed = context.typed;
    if !context.contains(declaring_trait)
        || !context.contains(requirement)
        || !provider_row_matches(context, plan, row, machine)
    {
        return false;
    }
    if let Some(implementation) = named_provider(context, plan) {
        let ConformanceImplementation::Closed { rows } = &implementation.implementation else {
            return false;
        };
        return rows
            .iter()
            .filter(|entry| {
                entry.declaring_trait == declaring_trait && entry.requirement == requirement
            })
            .count()
            == 1;
    }
    typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            if conformance.external_binding.is_some() {
                return false;
            }
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
                    == row.requirement_identity
        })
        .count()
        == 1
}

/// Rejoin a provider row to its declaration, including an exact specialization
/// of the map's realization. Physical machine ownership is not map ownership.
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
    if let Some(definition) = named_provider(context, plan) {
        let ConformanceImplementation::Closed { rows } = &definition.implementation else {
            return false;
        };
        let mut matching = rows.iter().filter(|entry| {
            let Some(owner) = typed
                .traits()
                .iter()
                .find(|owner| owner.symbol == entry.declaring_trait)
            else {
                return false;
            };
            let Some(requirement) = typed
                .trait_machine_signatures(owner)
                .iter()
                .find(|requirement| requirement.symbol == entry.requirement)
            else {
                return false;
            };
            typed.trait_requirement_callable_identity_text(owner, requirement)
                == row.requirement_identity
        });
        let Some(entry) = matching.next() else {
            return false;
        };
        if matching.next().is_some() || !context.contains(entry.realization_machine) {
            return false;
        }
        let Some(realization) = typed
            .machines()
            .iter()
            .find(|realization| realization.symbol == entry.realization_machine)
        else {
            return false;
        };
        let realization_states = typed.machine_states(realization);
        if !realization_states
            .first()
            .is_some_and(|state| state.symbol == entry.realization_state)
        {
            return false;
        }
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
                specialization.template == entry.realization_machine
                    && specialization.instance == machine.symbol
            })
            .count()
            == 1
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
