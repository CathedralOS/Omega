//! Retain the exact source and complete view-type binders of a sealed borrow.
//! Their metadata supplies no view, content validity or backing admission.

use crate::tables::type_constraints;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub fn access_borrow_factory(
    program: &resolved::SymbolResolvedTrees,
    target_symbol: symbols::SymbolHandle,
) -> Result<Option<typed::expression::AccessBorrowOperationRequest>, Diagnostic> {
    if !target_symbol.is_valid() {
        return Ok(None);
    }
    if !program.symbols.symbols().nodes().is_valid(target_symbol) {
        return Err(Diagnostic::error(
            "borrowed View target has no live declaration",
        ));
    }
    let found = if program.symbols.get(target_symbol).kind == symbols::SymbolKind::Machine {
        program.machines.by_symbol(target_symbol)
    } else {
        program.machines.iter().find(|machine| {
            machine.symbol == target_symbol
                || program
                    .machine_state_handles(machine.states)
                    .iter()
                    .any(|state| program.machine_state(*state).symbol == target_symbol)
        })
    };
    let Some(machine) = found else {
        return Ok(None);
    };
    access_borrow_factory_of(program, machine)
}

pub fn access_borrow_factory_of(
    program: &resolved::SymbolResolvedTrees,
    machine: &resolved::machine::Machine,
) -> Result<Option<typed::expression::AccessBorrowOperationRequest>, Diagnostic> {
    use language_core::sealed_intrinsics::{PlacementContent, PlacementOperation, SealedIntrinsic};

    // Record the exact public borrow prototype, not permission to realize it.
    // The private profile-backed primitives share this operation family but
    // carry a profile argument and retain their own admission judgment.
    if !program.symbols.symbols().nodes().is_valid(machine.symbol) {
        return Err(Diagnostic::error(
            "borrowed View prototype has no live declaration",
        ));
    }
    if program.symbols.sealed_intrinsic(machine.symbol)
        != Some(SealedIntrinsic::PlacementLifecycle {
            operation: PlacementOperation::View,
            content: PlacementContent::Resident,
        })
    {
        return Ok(None);
    }
    let malformed =
        |reason: &str| Diagnostic::error(format!("sealed borrowed View prototype: {reason}"));
    if program.machines.unique_by_symbol(machine.symbol) != Some(machine) {
        return Err(malformed("does not match its unique retained declaration"));
    }
    if !machine.is_public {
        return Ok(None);
    }
    if machine.body_is_present
        || machine.spelling.is_some()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.suspends
        || machine.blocks
        || machine.has_structural_type_equations
        || !machine.lifetime_parameters.is_empty()
        || machine.termination_plan
            != (language_semantics::MachineTerminationPlan {
                interface: language_semantics::TerminationInterface::Published(
                    language_semantics::TerminationGuarantee::NoGuarantee,
                ),
                ..Default::default()
            })
        || machine.service_reach_is_installation_bound
        || !program
            .service_reach_rows
            .services(machine.service_reach_row)
            .is_empty()
        || !machine.invokes.is_empty()
        || !machine.satisfies.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.ranking_subjects.is_empty()
        || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
    {
        return Err(malformed("noncanonical declaration or effect clauses"));
    }
    let states = program.machine_state_handles(machine.states);
    let [state] = states else {
        return Err(malformed("must declare exactly one state"));
    };
    let state = program.machine_state(*state);
    let [parameter] = program.state_parameters(state.parameters) else {
        return Err(malformed("must declare exactly one source parameter"));
    };
    let resolved::types::TypeReference::Reference(source) = &parameter.type_reference else {
        return Err(malformed("source parameter must be a reference"));
    };
    let access = source.access;
    if !matches!(
        access,
        language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
    ) || source.lifetime.is_some()
        || parameter.is_const
        || parameter.is_mutable != (access == language_core::ReferenceAccess::Mutable)
        || parameter.relevance.is_erased()
        || !machine.contracts.is_empty()
        || !state.contracts.is_empty()
    {
        return Err(malformed(
            "source reference flags or contract clauses differ",
        ));
    }
    let binders = program.machine_type_parameters(machine);
    if binders
        .iter()
        .any(|binder| !matches!(binder.kind, resolved::data::TypeParameterKind::Type))
    {
        return Err(malformed("all binders must be ordinary type parameters"));
    }
    let (source_parameter, view_parameter, backing) = match binders {
        [source_parameter, view_parameter] => {
            if !matches!(program.child_type_reference(source.referee), resolved::types::TypeReference::Named { symbol, .. } if *symbol == source_parameter.symbol)
            {
                return Err(malformed(
                    "ordinary source must retain its exact type formal",
                ));
            }
            (
                source_parameter.symbol,
                view_parameter.symbol,
                typed::expression::AccessBorrowBacking::Ordinary,
            )
        }
        [policy, source_parameter, view_parameter] => {
            let backing = resident_backing(program, source.referee, policy.symbol, source_parameter.symbol)
                .ok_or_else(|| malformed("resident source must retain exact core Extent, Granted and Resident<P, Source> declarations"))?;
            (source_parameter.symbol, view_parameter.symbol, backing)
        }
        _ => {
            return Err(malformed(
                "ordinary/resident source and full view binders differ",
            ));
        }
    };
    let Some(resolved::types::TypeReference::Reference(result)) = &state.return_type else {
        return Err(malformed("result must be a reference"));
    };
    if result.access != access
        || result.lifetime.is_some()
        || !matches!(program.child_type_reference(result.referee), resolved::types::TypeReference::Named { symbol, .. } if *symbol == view_parameter)
    {
        return Err(malformed(
            "result must retain the exact full view formal and source reference polarity",
        ));
    }
    Ok(Some(typed::expression::AccessBorrowOperationRequest {
        factory: machine.symbol,
        source_parameter,
        view_parameter,
        access,
        backing,
    }))
}

fn resident_backing(
    program: &resolved::SymbolResolvedTrees,
    source: arena::Handle<resolved::types::TypeReference>,
    policy: symbols::SymbolHandle,
    schema: symbols::SymbolHandle,
) -> Option<typed::expression::AccessBorrowBacking> {
    use resolved::types::{TypeConstraint, TypeReference};
    let TypeReference::Constrained(qualified) = program.child_type_reference(source) else {
        return None;
    };
    let TypeReference::Named { symbol: extent, .. } =
        program.child_type_reference(qualified.base_type)
    else {
        return None;
    };
    if program.symbols.core_vocabulary(*extent)
        != Some(language_core::core_vocabulary::CoreVocabulary::Extent)
    {
        return None;
    }
    let domain = |name: &str| {
        let mut definitions = program.domain_definitions.iter().filter(|definition| {
        program
            .symbols
            .core_vocabulary(definition.symbol)
            .is_some_and(|vocabulary| vocabulary.is_extent_domain_named(name))
            && matches!(&definition.target_type, TypeReference::Named { symbol, .. } if symbol == extent)
    });
        let definition = definitions.next()?;
        definitions.next().is_none().then_some(definition)
    };
    let granted = domain("Granted")?;
    let resident = domain("Resident")?;
    let constraints = type_constraints(program).span(qualified.constraints)?;
    if constraints.len() != 2 {
        return None;
    }
    let names = |name: &str, definition: &resolved::domain::DomainDefinition| {
        let declared = definition.name.as_str();
        name.strip_prefix("Extent::").unwrap_or(name)
            == declared.strip_prefix("Extent::").unwrap_or(declared)
    };
    let mut has_granted = false;
    let mut has_resident = false;
    for constraint in constraints {
        match constraint {
            TypeConstraint::Named(name) if names(name.as_str(), granted) && !has_granted => {
                has_granted = true;
            }
            TypeConstraint::Domain(application)
                if names(application.name.as_str(), granted)
                    && application.arguments.is_empty()
                    && !has_granted =>
            {
                has_granted = true;
            }
            TypeConstraint::Domain(application)
                if names(application.name.as_str(), resident) && !has_resident =>
            {
                let [actual_policy, actual_schema] =
                    program.child_type_references(application.arguments)
                else {
                    return None;
                };
                if !matches!(actual_policy, TypeReference::Named { symbol, .. } if *symbol == policy)
                    || !matches!(actual_schema, TypeReference::Named { symbol, .. } if *symbol == schema)
                {
                    return None;
                }
                has_resident = true;
            }
            _ => return None,
        }
    }
    (has_granted && has_resident).then_some(typed::expression::AccessBorrowBacking::Resident {
        extent: *extent,
        granted: granted.symbol,
        resident: resident.symbol,
    })
}
