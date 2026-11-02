//! Publish requirement and occurrence classification while Psi owns source.
//! This inventory describes no selected provider and requires no implementation.

use checked_trees::{CheckedProviderRequirement, CheckedProviderRequirementKind, CheckedTrees};
use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement;

pub(crate) fn retain_provider_requirements(checked: &mut CheckedTrees) {
    let typed = &checked.typed;
    let in_product = |symbol| {
        typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| typed.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
    };
    let mut requirements = arena::Arena::new();
    for operator in typed.operators() {
        requirements.append(CheckedProviderRequirement {
            symbol: operator.symbol,
            kind: CheckedProviderRequirementKind::Operator {
                is_product_boundary: operator.is_boundary && in_product(operator.symbol),
            },
            schema_identity: typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            ),
            package_identity: typed.symbols.symbol_package_identity(operator.symbol),
        });
    }
    for machine in typed.machines() {
        let Some(requirement) = IntrinsicRequirement::from_requirement(typed, machine) else {
            continue;
        };
        // The source schema is projectable only with a published termination
        // interface and an owned callable path. Keep this declaration fact here;
        // no Omega ServiceSchema or provider policy enters Psi.
        let path = typed.symbols.display_path(machine.symbol, "::");
        if !machine.symbol.is_valid()
            || !in_product(machine.symbol)
            || machine.termination_plan.interface.published().is_none()
            || !path
                .rsplit_once("::")
                .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty())
        {
            continue;
        }
        requirements.append(CheckedProviderRequirement {
            symbol: machine.symbol,
            kind: CheckedProviderRequirementKind::DirectCall {
                diagnostic_name: requirement.display(),
            },
            schema_identity: if machine.spelling.is_some() {
                requirement.requirement_identity
            } else {
                path
            },
            package_identity: requirement.package_identity,
        });
    }
    checked.facts.operators.provider_requirements = requirements;
}
