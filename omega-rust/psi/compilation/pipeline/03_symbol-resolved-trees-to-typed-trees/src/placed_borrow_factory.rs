//! Select a sealed borrowed View prototype. This request carries declaration
//! identity only; checking must still establish an exact compatible source loan.

use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub fn placed_borrow_factory(
    program: &resolved::SymbolResolvedTrees,
    target_symbol: symbols::SymbolHandle,
) -> Result<Option<typed::expression::PlacedBorrowOperationRequest>, Diagnostic> {
    let Some(machine) = program.machines.iter().find(|machine| {
        machine.symbol == target_symbol
            || program
                .machine_state_handles(machine.states)
                .iter()
                .any(|state| program.machine_state(*state).symbol == target_symbol)
    }) else {
        return Ok(None);
    };
    let (access, resident) = match machine.name.as_str().rsplit("::").next() {
        Some("view") => (language_core::ReferenceAccess::Shared, false),
        Some("view_mut") => (language_core::ReferenceAccess::Mutable, false),
        Some("view_resident_mut") => (language_core::ReferenceAccess::Mutable, true),
        _ => return Ok(None),
    };
    let sealed = |symbol| {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path.strip_prefix(&source.package_root).ok()
                        == Some(std::path::Path::new("layout.omg"))
            })
    };
    if !sealed(machine.symbol)
        || !sealed(machine.attached_data_symbol)
        || program.symbols.name(machine.attached_data_symbol) != "Placed"
    {
        return Ok(None);
    }
    let malformed =
        |reason: &str| Diagnostic::error(format!("sealed borrowed View prototype: {reason}"));
    let [policy, schema] = program.machine_type_parameters(machine) else {
        return Err(malformed("must declare exactly two type parameters"));
    };
    if !machine.is_public
        || !matches!(policy.kind, resolved::data::TypeParameterKind::Type)
        || !matches!(schema.kind, resolved::data::TypeParameterKind::Type)
        || !machine.lifetime_parameters.is_empty()
        || machine.body_is_present
        || machine.spelling.is_some()
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.suspends
        || machine.blocks
        || machine.has_structural_type_equations
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
        return Err(malformed(
            "noncanonical type binders or declaration/effect clauses",
        ));
    }
    let states = program.machine_state_handles(machine.states);
    let [state] = states else {
        return Err(malformed("must declare exactly one state"));
    };
    let state = program.machine_state(*state);
    let [parameter] = program.state_parameters(state.parameters) else {
        return Err(malformed("must declare exactly one source parameter"));
    };
    if parameter.is_const
        || parameter.is_mutable != (access == language_core::ReferenceAccess::Mutable)
        || parameter.relevance.is_erased()
        || !machine.contracts.is_empty()
        || !state.contracts.is_empty()
    {
        return Err(malformed(
            "source parameter flags or contract clauses differ",
        ));
    }

    let resolved::types::TypeReference::Reference(source) = &parameter.type_reference else {
        return Err(malformed("source parameter must be a reference"));
    };
    if source.access != access || source.lifetime.is_some() {
        return Err(malformed("source reference polarity or lifetime differs"));
    }
    let backing = if resident {
        resident_backing(program, source.referee, policy.symbol, schema.symbol)
            .ok_or_else(|| malformed("resident source must retain exact core Extent, Granted and Resident<P, T> declarations"))?
    } else {
        if !matches!(program.child_type_reference(source.referee), resolved::types::TypeReference::Named { symbol, .. } if *symbol == schema.symbol)
        {
            return Err(malformed(
                "source reference must name exactly the schema formal",
            ));
        }
        typed::expression::PlacedBorrowBacking::Ordinary
    };
    let Some(resolved::types::TypeReference::Reference(result)) = &state.return_type else {
        return Err(malformed("result must be a reference"));
    };
    if result.access != access || result.lifetime.is_some() {
        return Err(malformed("result reference polarity or lifetime differs"));
    }
    let resolved::types::TypeReference::Generic(view) =
        program.child_type_reference(result.referee)
    else {
        return Err(malformed(
            "result referent must retain the formal Placed application",
        ));
    };
    let [result_policy, result_schema] = program.child_type_references(view.arguments) else {
        return Err(malformed(
            "result Placed application must have two arguments",
        ));
    };
    if view.base_symbol != machine.attached_data_symbol
        || !view.lifetime_arguments.is_empty()
        || !matches!(result_policy, resolved::types::TypeReference::Named { symbol, .. } if *symbol == policy.symbol)
        || !matches!(result_schema, resolved::types::TypeReference::Named { symbol, .. } if *symbol == schema.symbol)
    {
        return Err(malformed(
            "result does not preserve exact policy/schema formal identities",
        ));
    }
    Ok(Some(typed::expression::PlacedBorrowOperationRequest {
        factory: machine.symbol,
        access,
        backing,
    }))
}

/// Resolve the sealed prototype's declaration coordinates once. These names
/// belong to the toolchain interface being admitted, not to later callers.
/// Call checking uses only the retained symbols and closed type arguments.
fn resident_backing(
    program: &resolved::SymbolResolvedTrees,
    source: arena::Handle<resolved::types::TypeReference>,
    policy: symbols::SymbolHandle,
    schema: symbols::SymbolHandle,
) -> Option<typed::expression::PlacedBorrowBacking> {
    use resolved::types::{TypeConstraint, TypeReference};
    let sealed_extent_declaration = |symbol| {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path.strip_prefix(&source.package_root).ok()
                        == Some(std::path::Path::new("extent.omg"))
            })
    };
    let TypeReference::Constrained(qualified) = program.child_type_reference(source) else {
        return None;
    };
    let TypeReference::Named { symbol: extent, .. } =
        program.child_type_reference(qualified.base_type)
    else {
        return None;
    };
    if !sealed_extent_declaration(*extent) || program.symbols.name(*extent) != "Extent" {
        return None;
    }
    let domain = |name: &str| {
        let mut definitions = program.domain_definitions.iter().filter(|definition| {
            sealed_extent_declaration(definition.symbol)
                && definition.name.as_str().rsplit("::").next() == Some(name)
                && matches!(&definition.target_type, TypeReference::Named { symbol, .. } if symbol == extent)
        });
        let definition = definitions.next()?;
        definitions.next().is_none().then_some(definition)
    };
    let granted = domain("Granted")?;
    let resident = domain("Resident")?;
    let constraints = program
        .tables
        .types
        .constraints
        .span(qualified.constraints)?;
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
    (has_granted && has_resident).then_some(typed::expression::PlacedBorrowBacking::Resident {
        extent: *extent,
        granted: granted.symbol,
        resident: resident.symbol,
    })
}
