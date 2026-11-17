use std::sync::Arc;

#[path = "../../../tests/support/frontend.rs"]
mod frontend_fixture;

fn main() {
    let driver_started = std::time::Instant::now();
    let mut arguments = std::env::args().skip(1);
    let count: usize = arguments.next().expect("helper count").parse().unwrap();
    let text = std::fs::read_to_string(arguments.next().expect("frozen source file")).unwrap();
    let fields_mode = match arguments.next().as_deref() {
        None | Some("window") => false,
        Some("fields") => true,
        other => panic!("unknown diagnostic mode {other:?}"),
    };
    assert!(arguments.next().is_none());
    assert!(
        !fields_mode || MEASURE_COMPLETIONS,
        "field counters need the tracking allocator"
    );
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("completion/main.omg".into(), text.clone())
        .source_id;
    let sources = Arc::new(sources);
    let tokens = frontend_fixture::Lexer::new(&text).tokenize().unwrap();
    let syntax = frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let mut frontier =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_numeric_probe(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(Arc::clone(&sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
        .unwrap();
    let mut typing =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
            frontier.shared_trees(),
        )
        .unwrap();
    frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let (activated, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
    frontier = activated;
    typing = typing.activate_body_completion(activation).unwrap();
    let selected: Vec<_> = frontier
        .pending_machines()
        .filter(|symbol| {
            frontier
                .trees()
                .symbols
                .name(*symbol)
                .starts_with("helper_")
        })
        .collect();
    assert_eq!(selected.len(), count);
    let setup_ns = driver_started.elapsed().as_nanos();
    let (
        mut resolution_ns,
        mut completion_ns,
        mut basic_oracle_ns,
        mut recorded_oracle_ns,
        mut pointer_oracle_ns,
    ) = (0u128, 0u128, 0u128, 0u128, 0u128);
    for (number, selected) in selected.into_iter().enumerate() {
        let resolution_started = std::time::Instant::now();
        let _context = selected_body_allocation_trace::context("nonempty-selected");
        frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let predecessor = typing.clone();
        let prior_count = predecessor.typed().deferred_refusals.len();
        assert_eq!(prior_count, 2 * number);
        let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
        let (source, successor) = completed
            .rebase_authored_selections_for_typed_completion(
                typing.typed().authored_declaration_selections(),
            )
            .unwrap();
        resolution_ns += resolution_started.elapsed().as_nanos();
        if fields_mode {
            profile_actual_owners(predecessor.typed());
        }
        let completion_started = std::time::Instant::now();
        let complete = || {
            let mut next = typing.complete_bodies(source).unwrap();
            build_time_evaluation::PreCheckEvaluation::defer_declared_requests(next.typed_mut())
                .unwrap();
            next
        };
        let (next, counts) = if MEASURE_COMPLETIONS && !fields_mode {
            selected_body_allocation_trace::measure(complete)
        } else {
            (
                complete(),
                selected_body_allocation_trace::Counts::default(),
            )
        };
        typing = next;
        completion_ns += completion_started.elapsed().as_nanos();
        selected_body_allocation_trace::log(format_args!(
            "NONEMPTY_WINDOW ordinal={} allocations={} requested_bytes={} live_delta={} peak_delta={} ns={}",
            number,
            counts.allocations,
            counts.requested_bytes,
            counts.live_delta,
            counts.peak_delta,
            counts.nanoseconds
        ));
        let basic_oracle_started = std::time::Instant::now();
        frontier = successor;
        assert_eq!(predecessor.typed().deferred_refusals.len(), prior_count);
        assert_eq!(typing.typed().deferred_refusals.len(), prior_count + 2);
        assert_eq!(typing.resolved(), frontier.trees());
        assert_eq!(
            typing
                .typed()
                .deferred_refusals_of(selected)
                .map(|row| row.request)
                .collect::<Vec<_>>(),
            [
                typed_trees::deferred_refusals::DeferredRequest::TypeExpressionEvaluation,
                typed_trees::deferred_refusals::DeferredRequest::ArrayExtentEvaluation
            ]
        );
        basic_oracle_ns += basic_oracle_started.elapsed().as_nanos();
        let recorded_oracle_started = std::time::Instant::now();
        build_time_evaluation::PreCheckEvaluation::require_recorded_requests(typing.typed())
            .unwrap();
        recorded_oracle_ns += recorded_oracle_started.elapsed().as_nanos();
        let pointer_oracle_started = std::time::Instant::now();
        let shared_rows = predecessor
            .typed()
            .deferred_refusals
            .iter()
            .filter(|old| {
                let retained = typing
                    .typed()
                    .deferred_refusals_of(old.declaration)
                    .find(|new| new.request == old.request)
                    .unwrap();
                assert_eq!(*old, retained);
                std::ptr::eq(*old, retained)
            })
            .count();
        pointer_oracle_ns += pointer_oracle_started.elapsed().as_nanos();
        if fields_mode {
            profile_refusal_operations(predecessor.typed(), typing.typed(), selected);
        }
        selected_body_allocation_trace::log(format_args!(
            "NONEMPTY_BATCH ordinal={} completed=1 retained_refusals={} resulting_refusals={} shared_retained_rows={}",
            number,
            prior_count,
            prior_count + 2,
            shared_rows
        ));
    }
    if let Some(path) = std::env::var_os("OMEGA_NONEMPTY_PHASES") {
        let driver_ns = driver_started.elapsed().as_nanos();
        std::fs::write(path, format!(r#"{{"setup_ns":{setup_ns},"resolution_ns":{resolution_ns},"completion_ns":{completion_ns},"basic_oracle_ns":{basic_oracle_ns},"recorded_oracle_ns":{recorded_oracle_ns},"pointer_oracle_ns":{pointer_oracle_ns},"driver_ns":{driver_ns}}}"#)).unwrap();
    }
    println!(
        "completed_nonempty_batches={} retained_refusals={} pending_length_body={}",
        count,
        typing.typed().deferred_refusals.len(),
        frontier.pending_machines().count()
    );
    assert_eq!(frontier.pending_machines().count(), 1);
}

fn profile_owner<T: Clone>(name: &'static str, rows: Option<usize>, owner: &T) {
    let (copy, counts) = selected_body_allocation_trace::measure(|| owner.clone());
    drop(copy);
    selected_body_allocation_trace::log(format_args!(
        "OWNER_CLONE name={} rows={} allocations={} requested_bytes={} live_delta={} peak_delta={}",
        name,
        rows.map_or_else(|| "unknown".to_owned(), |n| n.to_string()),
        counts.allocations,
        counts.requested_bytes,
        counts.live_delta,
        counts.peak_delta
    ));
}

fn profile_actual_owners(program: &typed_trees::TypedTrees) {
    profile_owner(
        "excluded_authored_selection_occurrences",
        None,
        &program.excluded_authored_selection_occurrences,
    );
    profile_owner("roots", None, &program.roots);
    profile_owner(
        "service_reaches",
        Some(program.service_reaches.definitions().len()),
        &program.service_reaches,
    );
    profile_owner(
        "service_reach_rows",
        Some(program.service_reach_rows.len()),
        &program.service_reach_rows,
    );
    profile_owner(
        "authored_service_reach_rows",
        Some(program.authored_service_reach_rows.len()),
        &program.authored_service_reach_rows,
    );
    profile_owner(
        "semantic_domains",
        Some(program.semantic_domains.len()),
        &program.semantic_domains,
    );
    profile_owner(
        "applied_domain_applications",
        Some(program.applied_domain_applications.iter().len()),
        &program.applied_domain_applications,
    );
    profile_owner(
        "applied_data_cleanup_selections",
        Some(program.applied_data_cleanup_selections.len()),
        &program.applied_data_cleanup_selections,
    );
    profile_owner(
        "external_bindings",
        Some(program.external_bindings.identities().len()),
        &program.external_bindings,
    );
    profile_owner(
        "plan_laid_layouts",
        Some(program.plan_laid_layouts.len()),
        &program.plan_laid_layouts,
    );
    profile_owner(
        "plan_laid_target_layouts",
        Some(program.plan_laid_target_layouts.len()),
        &program.plan_laid_target_layouts,
    );
    profile_owner(
        "plan_laid_schema_fields",
        Some(program.plan_laid_schema_fields.len()),
        &program.plan_laid_schema_fields,
    );
    profile_owner(
        "plan_laid_retired_field_identities",
        Some(program.plan_laid_retired_field_identities.len()),
        &program.plan_laid_retired_field_identities,
    );
    profile_owner(
        "access_view_plans",
        Some(program.access_view_plans.len()),
        &program.access_view_plans,
    );
    profile_owner(
        "access_lifecycle_plans",
        Some(program.access_lifecycle_plans.len()),
        &program.access_lifecycle_plans,
    );
    profile_owner(
        "access_policy_invocations",
        Some(program.access_policy_invocations.len()),
        &program.access_policy_invocations,
    );
    profile_owner(
        "access_borrow_factories",
        Some(program.access_borrow_factories.len()),
        &program.access_borrow_factories,
    );
    profile_owner(
        "wire_placements",
        Some(program.wire_placements.len()),
        &program.wire_placements,
    );
    profile_owner(
        "wire_encode_obligations",
        Some(program.wire_encode_obligations.len()),
        &program.wire_encode_obligations,
    );
    profile_owner(
        "wire_schema_plans",
        Some(program.wire_schema_plans.len()),
        &program.wire_schema_plans,
    );
    profile_owner(
        "machine_specializations",
        None,
        &program.machine_specializations,
    );
    profile_owner(
        "captured_machine_applications",
        Some(program.captured_machine_applications.len()),
        &program.captured_machine_applications,
    );
    profile_owner(
        "captured_machine_states",
        Some(program.captured_machine_states.len()),
        &program.captured_machine_states,
    );
    profile_owner(
        "trait_implementation_applications",
        Some(program.trait_implementation_applications.len()),
        &program.trait_implementation_applications,
    );
    profile_owner(
        "data_implementation_applications",
        Some(program.data_implementation_applications.len()),
        &program.data_implementation_applications,
    );
    profile_owner(
        "target_machine_families",
        Some(program.target_machine_families.len()),
        &program.target_machine_families,
    );
    profile_owner(
        "deferred_refusals",
        Some(program.deferred_refusals.len()),
        &program.deferred_refusals,
    );
    profile_owner(
        "evidence_forwardings",
        Some(program.evidence_forwardings.len()),
        &program.evidence_forwardings,
    );
    profile_owner(
        "proof_output_calls",
        Some(program.proof_output_calls.len()),
        &program.proof_output_calls,
    );
    profile_owner("declaration_indexes", None, &program.declaration_indexes);
    profile_owner(
        "ranking_expression_custody",
        Some(program.ranking_expression_custody.len()),
        &program.ranking_expression_custody,
    );
    profile_owner(
        "pending_const_range_endpoints",
        None,
        &program.pending_const_range_endpoints,
    );
    profile_owner(
        "constant_recipes",
        Some(program.constant_recipes.len()),
        &program.constant_recipes,
    );
    profile_owner(
        "constant_expression_contexts",
        Some(program.constant_expression_contexts.len()),
        &program.constant_expression_contexts,
    );
    profile_owner(
        "constant_recipe_closure",
        None,
        &program.constant_recipe_closure,
    );
    profile_owner("callable_identities", None, &program.callable_identities);
    profile_owner("type_identities", None, &program.type_identities);
    profile_owner("table-total", None, &program.tables);
    profile_owner(
        "tables.runtime_callable_contracts",
        Some(program.tables.runtime_callable_contracts.len()),
        &program.tables.runtime_callable_contracts,
    );
    profile_owner(
        "tables.runtime_callable_lowering",
        None,
        &program.tables.runtime_callable_lowering,
    );
    profile_owner(
        "tables.designated_static_type_arguments",
        Some(program.tables.designated_static_type_arguments.len()),
        &program.tables.designated_static_type_arguments,
    );
    profile_owner(
        "tables.retained_product_application_handles",
        Some(program.tables.retained_product_application_handles.len()),
        &program.tables.retained_product_application_handles,
    );
    profile_owner(
        "tables.product_input_selections",
        Some(program.tables.product_input_selections.len()),
        &program.tables.product_input_selections,
    );
    profile_owner(
        "tables.product_input_argument_steps",
        Some(program.tables.product_input_argument_steps.len()),
        &program.tables.product_input_argument_steps,
    );
    profile_owner(
        "tables.product_operand_applications",
        Some(program.tables.product_operand_applications.len()),
        &program.tables.product_operand_applications,
    );
    profile_owner(
        "tables.const_declarations",
        Some(program.tables.const_declarations.len()),
        &program.tables.const_declarations,
    );
    profile_owner(
        "tables.data_definitions",
        None,
        &program.tables.data_definitions,
    );
    profile_owner(
        "tables.type_aliases",
        Some(program.tables.type_aliases.len()),
        &program.tables.type_aliases,
    );
    profile_owner(
        "tables.data_application_definitions",
        Some(program.tables.data_application_definitions.len()),
        &program.tables.data_application_definitions,
    );
    profile_owner(
        "tables.data_type_parameters",
        None,
        &program.tables.data_type_parameters,
    );
    profile_owner(
        "tables.declared_type_equations",
        Some(program.tables.declared_type_equations.len()),
        &program.tables.declared_type_equations,
    );
    profile_owner(
        "tables.static_type_contexts",
        None,
        &program.tables.static_type_contexts,
    );
    profile_owner(
        "tables.data_members",
        Some(program.tables.data_members.len()),
        &program.tables.data_members,
    );
    profile_owner(
        "tables.data_payload_fields",
        Some(program.tables.data_payload_fields.len()),
        &program.tables.data_payload_fields,
    );
    profile_owner(
        "tables.domain_definitions",
        None,
        &program.tables.domain_definitions,
    );
    profile_owner(
        "tables.proof_facts",
        Some(program.tables.proof_facts.len()),
        &program.tables.proof_facts,
    );
    profile_owner(
        "tables.propositions",
        Some(program.tables.propositions.len()),
        &program.tables.propositions,
    );
    profile_owner(
        "tables.proposition_binders",
        Some(program.tables.proposition_binders.len()),
        &program.tables.proposition_binders,
    );
    profile_owner(
        "tables.mathematical_definitions",
        Some(program.tables.mathematical_definitions.len()),
        &program.tables.mathematical_definitions,
    );
    profile_owner(
        "tables.mathematical_parameters",
        Some(program.tables.mathematical_parameters.len()),
        &program.tables.mathematical_parameters,
    );
    profile_owner(
        "tables.mathematical_types",
        Some(program.tables.mathematical_types.len()),
        &program.tables.mathematical_types,
    );
    profile_owner(
        "tables.domain_path_members",
        Some(program.tables.domain_path_members.len()),
        &program.tables.domain_path_members,
    );
    profile_owner(
        "tables.operator_path_members",
        Some(program.tables.operator_path_members.len()),
        &program.tables.operator_path_members,
    );
    profile_owner("tables.machines", None, &program.tables.machines);
    profile_owner(
        "tables.measures",
        Some(program.tables.measures.len()),
        &program.tables.measures,
    );
    profile_owner(
        "tables.measure_path_members",
        Some(program.tables.measure_path_members.len()),
        &program.tables.measure_path_members,
    );
    profile_owner("tables.operators", None, &program.tables.operators);
    profile_owner(
        "tables.machine_owned_data",
        Some(program.tables.machine_owned_data.len()),
        &program.tables.machine_owned_data,
    );
    profile_owner(
        "tables.machine_trait_conformances",
        Some(program.tables.machine_trait_conformances.len()),
        &program.tables.machine_trait_conformances,
    );
    profile_owner(
        "tables.machine_states",
        Some(program.tables.machine_states.len()),
        &program.tables.machine_states,
    );
    profile_owner(
        "tables.state_parameters",
        Some(program.tables.state_parameters.len()),
        &program.tables.state_parameters,
    );
    profile_owner(
        "tables.traits",
        Some(program.tables.traits.len()),
        &program.tables.traits,
    );
    profile_owner(
        "tables.conformances",
        Some(program.tables.conformances.len()),
        &program.tables.conformances,
    );
    profile_owner(
        "tables.trait_requirements",
        Some(program.tables.trait_requirements.len()),
        &program.tables.trait_requirements,
    );
    profile_owner(
        "tables.trait_machine_signatures",
        Some(program.tables.trait_machine_signatures.len()),
        &program.tables.trait_machine_signatures,
    );
    profile_owner(
        "tables.signature_invokes",
        Some(program.tables.signature_invokes.len()),
        &program.tables.signature_invokes,
    );
    profile_owner(
        "tables.signature_contracts",
        Some(program.tables.signature_contracts.len()),
        &program.tables.signature_contracts,
    );
    profile_owner(
        "tables.expression_table",
        None,
        &program.tables.expression_table,
    );
    profile_owner(
        "tables.statement_table",
        None,
        &program.tables.statement_table,
    );
    profile_owner(
        "tables.type_reference_table",
        None,
        &program.tables.type_reference_table,
    );
    profile_owner(
        "tables.wire_schemas",
        Some(program.tables.wire_schemas.len()),
        &program.tables.wire_schemas,
    );
    profile_owner(
        "tables.wire_members",
        Some(program.tables.wire_members.len()),
        &program.tables.wire_members,
    );
}

fn profile_refusal_operations(
    predecessor: &typed_trees::TypedTrees,
    successor: &typed_trees::TypedTrees,
    selected: symbols::SymbolHandle,
) {
    // Clone setup and the genuine new row values are outside the mutation
    // window. The window calls the production recorder on the real checkpoint.
    let mut append_probe = predecessor.clone();
    let introduced: Vec<_> = successor.deferred_refusals_of(selected).cloned().collect();
    let (_, append) = selected_body_allocation_trace::measure(|| {
        for row in introduced {
            append_probe.record_deferred_refusal_with_detail(
                row.declaration,
                row.request,
                row.source_span,
                row.detail,
            );
        }
    });
    assert_eq!(append_probe.deferred_refusals, successor.deferred_refusals);
    selected_body_allocation_trace::log(format_args!(
        "REFUSAL_APPEND retained_rows={} allocations={} requested_bytes={} live_delta={} peak_delta={} ns={}",
        predecessor.deferred_refusals.len(),
        append.allocations,
        append.requested_bytes,
        append.live_delta,
        append.peak_delta,
        append.nanoseconds
    ));
    drop(append_probe);

    let mut lookup_probe = predecessor.clone();
    let retained: Vec<_> = predecessor.deferred_refusals.iter().cloned().collect();
    const REPEATS: usize = 64;
    let (_, read) = selected_body_allocation_trace::measure(|| {
        for _ in 0..REPEATS {
            for row in &retained {
                std::hint::black_box(&mut lookup_probe).record_deferred_refusal(
                    row.declaration,
                    row.request,
                    row.source_span,
                );
            }
        }
    });
    assert_eq!(
        lookup_probe.deferred_refusals,
        predecessor.deferred_refusals
    );
    selected_body_allocation_trace::log(format_args!(
        "REFUSAL_EXISTING calls={} allocations={} requested_bytes={} ns={}",
        retained.len() * REPEATS,
        read.allocations,
        read.requested_bytes,
        read.nanoseconds
    ));
}
