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

/// Retain only a declaring-trait lifetime application the map actually owns.
/// Parent lifetime composition is not retained on closed rows. An empty parent
/// telescope needs no substitution; a nonempty one needs that missing evidence.
pub(crate) fn named_requirement_lifetime_arguments<'a>(
    typed: &'a TypedTrees,
    implementation: &'a Conformance,
    declaring_trait: SymbolHandle,
) -> Option<&'a [u32]> {
    let root = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == implementation.trait_symbol)?;
    let arguments = implementation.trait_lifetime_arguments.as_slice();
    if arguments.len() != root.lifetime_parameters.len()
        || arguments.iter().any(|ordinal| {
            usize::try_from(*ordinal).map_or(true, |ordinal| {
                ordinal >= implementation.lifetime_parameters.len()
            })
        })
    {
        return None;
    }
    if root.symbol == declaring_trait {
        return Some(arguments);
    }
    let declaring = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == declaring_trait)?;
    declaring.lifetime_parameters.is_empty().then_some(&[])
}

/// Borrow the declaration that owns one exact provider requirement application.
/// This view retains the original lifetime substitution without inventing a
/// machine edge for a closed implementation map.
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
        let ConformanceImplementation::Closed { rows } = &implementation.implementation else {
            return None;
        };
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

/// The one row of a named provider's map that a plan row installs, joined by
/// the requirement's exact callable identity. `None` when the plan is not a
/// named provider's, or when zero or several rows (or an unretained
/// realization) answer the identity.
pub(crate) fn named_provider_row<'a>(
    context: ProviderPlanningContext<'a>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Option<&'a typed_trees::trait_definition::ConformanceRow> {
    let typed = context.typed;
    let definition = named_provider(context, plan)?;
    let ConformanceImplementation::Closed { rows } = &definition.implementation else {
        return None;
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
    let entry = matching.next()?;
    (matching.next().is_none() && context.contains(entry.realization_machine)).then_some(entry)
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
    if named_provider(context, plan).is_some() {
        let Some(entry) = named_provider_row(context, plan, row) else {
            return false;
        };
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
