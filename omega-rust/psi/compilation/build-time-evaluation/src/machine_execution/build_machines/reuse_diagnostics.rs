//! Temporary, opt-in explanation of exact checked-program inequality.
//! The equality owner decides first; these comparisons cannot admit reuse.

macro_rules! unequal_fields {
    ($owner:literal, $left:expr, $right:expr; $($field:ident),* $(,)?) => {
        $(if ($left).$field != ($right).$field {
            eprintln!(concat!("Build reuse: unequal ", $owner, ".", stringify!($field)));
        })*
    };
}

pub(super) fn checked(left: &checked_trees::CheckedTrees, right: &checked_trees::CheckedTrees) {
    eprintln!(
        "Build reuse: checked content differs: typed={} facts={}",
        left.typed == right.typed,
        left.facts == right.facts
    );
    if left.typed != right.typed {
        typed("checked typed content", &left.typed, &right.typed);
    }
    unequal_fields!("facts", left.facts, right.facts;
    declaration_descriptions,
    proof_applications,
    deferred_layout_applications,
    quotient_formations,
    quotient_operations,
    specialization_identities,
    accepted_template_identities,
    finalized_selections,
    finalized_selection_occurrences,
    resolved_types,
    executable_bindings,
    stored_integer_writes,
    proven_scalar_indexes,
    scalar_index_coordinates,
    scalar_selector_coordinates,
    proven_byte_writes,
    proven_assignment_indexes,
    decided_call_requires,
    operational,
    semantic,
    borrow,
    reference_custody,
    state_locals,
    provider_closed_requirements,
    provider_instance_publication,
    content_classes,
    contextual_cleanup,
    proof,
    values,
    domains,
    dynamic_conformances,
    nominal_machine_uses,
    requirement_call_specializations,
    operators,
    capabilities,
    flow,
    index_compatibility,
    mutation,
    service_reaches,
    synchronous_invocations,
    suspensions,
    blocking,
    termination,
    qualifications,
    contract_plans,
    carry,
    fact_call_projections,
    access_view_inputs,
    constant_recipe_obligations,
    constant_predicate_obligations,
    integer_range_obligations
        );
}

pub(super) fn typed(label: &str, left: &typed_trees::TypedTrees, right: &typed_trees::TypedTrees) {
    eprintln!("Build reuse: compare {label}");
    unequal_fields!("typed", left, right;
    identity,
    product_declaration_frontier,
    excluded_authored_selection_occurrences,
    roots,
    tables,
    symbols,
    service_reaches,
    service_reach_rows,
    authored_service_reach_rows,
    semantic_domains,
    applied_domain_applications,
    applied_data_cleanup_selections,
    external_bindings,
    plan_laid_layouts,
    plan_laid_target_layouts,
    plan_laid_schema_fields,
    plan_laid_retired_field_identities,
    access_view_plans,
    access_lifecycle_plans,
    access_policy_invocations,
    access_borrow_factories,
    wire_placements,
    wire_encode_obligations,
    wire_schema_plans,
    machine_specializations,
    captured_machine_applications,
    captured_machine_states,
    trait_implementation_applications,
    data_implementation_applications,
    dynamic_slots,
    target_machine_families,
    boundary_completion_declarations,
    evidence_forwardings,
    proof_output_calls,
    declaration_indexes,
    ranking_expression_custody,
    pending_const_range_endpoints,
    constant_recipes,
    constant_expression_contexts,
    constant_recipe_closure,
    callable_identities,
    callable_identity_memo,
    type_identities
        );
    if left.roots != right.roots {
        unequal_fields!("roots", left.roots, right.roots;
        retained_product_applications,
        const_declarations,
        data_definitions,
        type_aliases,
        domain_definitions,
        machines,
        measures,
        operators,
        machine_token_bindings,
        propositions,
        mathematical_definitions,
        traits,
        conformances,
        wire_schemas
                );
    }
    if left.tables != right.tables {
        unequal_fields!("tables", left.tables, right.tables;
        designated_static_type_arguments,
        retained_product_application_handles,
        product_input_selections,
        product_input_argument_steps,
        product_operand_applications,
        const_declarations,
        data_definitions,
        type_aliases,
        data_application_definitions,
        data_type_parameters,
        declared_type_equations,
        static_type_contexts,
        data_members,
        data_payload_fields,
        domain_definitions,
        proof_facts,
        propositions,
        proposition_binders,
        mathematical_definitions,
        mathematical_parameters,
        mathematical_types,
        domain_path_members,
        operator_path_members,
        machines,
        measures,
        measure_path_members,
        operators,
        machine_owned_data,
        machine_trait_conformances,
        machine_states,
        state_parameters,
        traits,
        conformances,
        trait_requirements,
        trait_machine_signatures,
        signature_invokes,
        signature_contracts,
        expression_table,
        statement_table,
        type_reference_table,
        wire_schemas,
        wire_members
                );
    }
    if left.symbols != right.symbols {
        let left_sources = left.symbols.source_map();
        let right_sources = right.symbols.source_map();
        eprintln!(
            "Build reuse: symbol counts {} / {}; source maps equal={}",
            left.symbols.symbols().len(),
            right.symbols.symbols().len(),
            left_sources == right_sources
        );
        if let (Some(left), Some(right)) = (left_sources, right_sources) {
            eprintln!(
                "Build reuse: source counts {} / {}; selected {} / {}",
                left.files().count(),
                right.files().count(),
                left.selected_files().count(),
                right.selected_files().count()
            );
            for (left, right) in left
                .files()
                .zip(right.files())
                .filter(|(left, right)| left != right)
                .take(8)
            {
                eprintln!(
                    "Build reuse: source {:?}/{:?}: path={} root={} package={} scope={} origin={} stratum={} bytes={}",
                    left.source_id,
                    right.source_id,
                    left.path == right.path,
                    left.package_root == right.package_root,
                    left.package_identity == right.package_identity,
                    left.dependency_scope == right.dependency_scope,
                    left.origin == right.origin,
                    left.resolution_stratum == right.resolution_stratum,
                    left.source == right.source
                );
            }
        }
    }
}
