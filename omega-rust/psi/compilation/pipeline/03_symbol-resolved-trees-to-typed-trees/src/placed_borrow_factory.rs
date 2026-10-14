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
    let access = match machine.name.as_str().rsplit("::").next() {
        Some("view") => language_core::ReferenceAccess::Shared,
        Some("view_mut") => language_core::ReferenceAccess::Mutable,
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
    if source.access != access
        || source.lifetime.is_some()
        || !matches!(program.child_type_reference(source.referee), resolved::types::TypeReference::Named { symbol, .. } if *symbol == schema.symbol)
    {
        return Err(malformed(
            "source reference must name exactly the schema formal with declared polarity",
        ));
    }
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
    }))
}
