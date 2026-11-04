//! Authored provider coordinates, before checking or target selection.

mod context;
pub use context::ProviderPlanningContext;

/// Retain every authored target origin in one exact dependency-purpose scope.
/// Target choice remains outside this source query, including for early Build.
pub fn provider_target_declarations(
    typed: &crate::TypedTrees,
    scope: source::DependencyScope,
) -> Vec<language_semantics::provider_declarations::SelectedTargetMachineOrigin> {
    typed
        .machines()
        .iter()
        .filter(|machine| {
            typed
                .symbols
                .symbol_is_in_dependency_scope(machine.symbol, scope)
                && (scope == source::DependencyScope::Product || machine.target.is_none())
        })
        .filter_map(|machine| {
            machine.authored_target.as_ref().map(|target| {
                language_semantics::provider_declarations::SelectedTargetMachineOrigin {
                    machine: machine.symbol,
                    target: target.as_str().to_owned(),
                }
            })
        })
        .collect()
}

/// The declaration owning an authored exact satisfaction. Map membership
/// comes from the retained owned-row relation. A member with missing or
/// ambiguous ownership cannot fall through to an attached data declaration.
/// Existing data-owned declarations remain explicit migration inputs.
pub fn satisfaction_provider_declaration(
    typed: &crate::TypedTrees,
    machine: &crate::machine::Machine,
) -> Option<symbols::SymbolHandle> {
    let owner = typed.symbols.get(machine.symbol).declaration_owner;
    if owner.is_valid() && typed.symbols.get(owner).kind == symbols::SymbolKind::Conformance {
        return typed
            .owned_conformance_realization(machine.symbol)
            .map(|(implementation, _)| implementation.symbol);
    }
    let attached = machine.attached_data.as_ref()?;
    let owner = typed.data_definition_by_symbol(machine.attached_data_symbol)?;
    (owner.name == *attached).then_some(owner.symbol)
}

/// The complete callable selected by one recorded boundary satisfaction.
/// This query follows the retained exact declaration symbol; it never chooses
/// a same-shaped requirement or a namespace with the same display name.
pub fn boundary_satisfaction_identity(
    typed: &crate::TypedTrees,
    machine: &crate::machine::Machine,
    edge: &crate::machine::TraitConformance,
) -> Option<(symbols::SymbolHandle, String)> {
    let declaration = crate::machine::resolve_satisfied_declaration(typed, machine, edge)?;
    let symbol = declaration.symbol();
    if !edge.requirement_symbol.is_valid() || edge.requirement_symbol != symbol {
        return None;
    }
    let identity = match declaration {
        crate::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        } if definition.is_boundary => {
            typed.trait_requirement_callable_identity_text(definition, requirement)
        }
        crate::machine::SatisfiedDeclaration::TopLevelRequirement(requirement)
            if requirement.supply_mode
                == language_semantics::MachineSupplyMode::TopLevelRequirement =>
        {
            typed.machine_callable_identity_text(requirement)?
        }
        crate::machine::SatisfiedDeclaration::Operator(operator) if operator.is_boundary => {
            crate::operator::boundary_operator_requirement_identity(typed, operator)
        }
        _ => return None,
    };
    (!identity.is_empty()).then_some((symbol, identity))
}
