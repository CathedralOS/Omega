//! A core borrow uses its actual closed result and the source's live storage.
//! The test supplies no policy, resident, layout or host-provider authority.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};
use typed_trees::{TypedTrees, expression::ExpressionNode};

const SOURCE: &str = r#"
use omega::language::core::Access;
machine relay<'owner>(source: &'owner mut i64) -> &'owner mut i64 crashes none {
    Access::view_mut<i64, i64>(source)
}
machine other(source: &mut u64) -> &mut u64 crashes none {
    Access::view_mut<u64, u64>(source)
}
machine run() -> i64 crashes none {
    let mut backing: i64 = 17;
    let child: &mut i64 = relay(&mut backing);
    child = 41;
    let observed: i64 = child;
    observed
}
"#;

fn typed(source: &str) -> TypedTrees {
    let library =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library");
    super::schema_customer_source::actual_library_source(
        &library,
        source,
        semantic_vocabulary::PackageKeyIdentity::from_digest([0xA1; 32])
            .expect("retained source test package"),
    )
    .expect("actual core and allocator sources match the compiler catalog")
}

fn run(source: &str) -> Result<BuildTimeValue, String> {
    let source = typed(source);
    let requester = source.realized_machine_named("run").unwrap().symbol;
    let prepared = PreparedBuildMachineProgram::prepare(&source, None)
        .map_err(|errors| diagnostics::join_messages(&errors))?;
    let mut accounting = SourceCompletionAccounting::default();
    let entry = prepared.entry(requester, requester, &[])?;
    describe_reference_flow(&entry);
    let result = entry.invoke_recorded(
        InvocationTarget {
            // The existing pure source-control profile is explicit. This
            // supplies neither native layout nor host access to the borrow.
            name: Some("linux_x86_64"),
            layout_target: None,
            carrier_layout: None,
            address_model: None,
        },
        &mut accounting,
    )?;
    accounting.validate_charges()?;
    Ok(result.value().clone())
}

fn describe_reference_flow(entry: &crate::PreparedBuildMachineEntry) {
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorLocation, descriptor_location_lifetime,
    };
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorStoredOrigin, terminal_reference_graph_from,
    };

    let accepted = entry.accepted();
    let module = accepted.module();
    let Some(root) = accepted.machine_of(entry.symbol()) else {
        eprintln!("Reference-flow observation has no actual source machine");
        return;
    };
    let graph = match terminal_reference_graph_from(module, [root]) {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("Reference-flow graph is unavailable: {error:?}");
            return;
        }
    };
    let mut analysis = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graph);
    eprintln!(
        "Actual reference flow for {}: source machine {root:?}",
        entry.source().symbols.name(entry.symbol()),
    );
    for graph_machine in &graph {
        let body = module.machine(graph_machine.id).unwrap();
        eprintln!(
            "Machine {:?}: formals {:?}; declared result {:?}",
            body.id, body.structural_parameters, body.result,
        );
        if let Some(tables) = accepted.closure().elaboration.tables.machines.get(body.id) {
            eprintln!("  Exact places {:?}", tables.places);
            eprintln!("  Obligations {:?}", tables.obligations);
            eprintln!("  Writes {:?}", tables.writes);
        }
        match analysis.result_slots(body.id) {
            Some(slots) => {
                for slot in slots {
                    for origin in slot.origins {
                        let description = match origin {
                            DescriptorStoredOrigin::Parameter {
                                position,
                                projection,
                            } => {
                                format!("parameter {position}: {projection:?}")
                            }
                            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                format!("incoming slot {position}: {path:?}")
                            }
                            DescriptorStoredOrigin::IncomingContents {
                                position,
                                steps,
                                projection,
                            } => {
                                format!("incoming contents {position}: {steps:?}; {projection:?}")
                            }
                            DescriptorStoredOrigin::Invocation {
                                carrier_type,
                                projection,
                            } => {
                                format!("invocation {carrier_type:?}: {projection:?}")
                            }
                            DescriptorStoredOrigin::AccessFormation {
                                machine,
                                operation,
                                place,
                                carrier,
                                projection,
                            } => {
                                format!(
                                    "access formation {machine:?}/{operation:?}/{place:?}/{carrier:?}: {projection:?}"
                                )
                            }
                            DescriptorStoredOrigin::Empty => "empty".into(),
                            DescriptorStoredOrigin::Literal { machine, place } => {
                                format!("literal {machine:?}/{place:?}")
                            }
                            DescriptorStoredOrigin::Provider(_) => "provider".into(),
                        };
                        eprintln!("  Result slot {:?}: {description}", slot.path);
                    }
                }
            }
            None => eprintln!("  Result slots unavailable"),
        }
        let flow = analysis.flow(body.id);
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                let at = (block.id, u32::try_from(ordinal).unwrap());
                eprintln!(
                    "  {at:?}: {:?}; result {:?}",
                    operation.kind, operation.result
                );
                if let Some(step) = flow.as_ref().and_then(|flow| flow.get(&at)) {
                    eprintln!(
                        "    inputs {:?}; definitions {:?}; derivations {:?}; authorization {:?}",
                        step.inputs, step.definitions, step.derivations, step.authority_derivations,
                    );
                }
                if matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishReference { .. }
                ) && let Some(result) = operation.result.as_structural()
                    && let Some(flow) = &flow
                {
                    let lifetime = descriptor_location_lifetime(
                        flow,
                        at,
                        &DescriptorLocation::whole(result.place),
                    );
                    eprintln!(
                        "    Formed loan live {:?}; authorized {:?}",
                        lifetime.live, lifetime.authorized
                    );
                }
            }
            eprintln!("  Block {:?} exits with {:?}", block.id, block.terminator);
        }
    }
}

#[test]
fn a_closed_core_borrow_returns_the_original_storage_and_excludes_its_parent() {
    assert_eq!(
        run(SOURCE).expect("actual core reference and helper return"),
        BuildTimeValue::Int(41)
    );
    let conflicting = SOURCE.replace("    child = 41;", "    backing = 7;\n    child = 41;");
    assert_ne!(conflicting, SOURCE);
    let refusal = run(&conflicting).expect_err("the returned reference keeps its original loan");
    assert!(
        refusal.contains("storage is changed while a conflicting reference remains live"),
        "{refusal}"
    );
}

#[test]
fn a_closed_core_borrow_cannot_invent_a_different_view_interpretation() {
    let source = SOURCE
        .replace("-> &'owner mut i64", "-> &'owner mut u64")
        .replace("Access::view_mut<i64, i64>", "Access::view_mut<i64, u64>")
        .replace("machine run() -> i64", "machine run() -> u64")
        .replace("let child: &mut i64", "let child: &mut u64")
        .replace("let observed: i64", "let observed: u64");
    let refusal = run(&source).expect_err("different content needs its actual admission");
    assert!(
        refusal.contains(
            "a different qualified view needs evaluated policy, layout and backing correspondence"
        ),
        "{refusal}"
    );
}

#[test]
fn a_prepared_bodyless_contract_cannot_follow_changed_application_bindings() {
    let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(SOURCE),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("same ordinary preparation pass prepares the real bodyless declarations");
    let relay = source.realized_machine_named("relay").unwrap().symbol;
    let other = source.realized_machine_named("other").unwrap().symbol;
    let call = |owner| {
        let describe = |symbol| {
            if source.symbols.symbols().nodes().is_valid(symbol) {
                format!(
                    "{} at {:?}",
                    source.symbols.name(symbol),
                    source.symbols.symbol_provenance_source_span(symbol)
                )
            } else {
                format!("invalid {symbol:?}")
            }
        };
        let mut calls =
            source
                .expression_table
                .iter_expressions()
                .filter_map(|(expression, node)| {
                    let ExpressionNode::Call(call) = node else {
                        return None;
                    };
                    (source.access_borrow_factory(call.target_symbol).is_some()
                        && typed_trees::references::StaticArgumentCall::Expression(
                            expression, call,
                        )
                        .lexical_owner(&source)
                            == owner)
                        .then_some((call.authored_machine_application, call.target_symbol))
                });
        let result = calls.next().unwrap_or_else(|| {
            let observed = source
                .expression_table
                .iter_expressions()
                .filter_map(|(expression, node)| {
                    let ExpressionNode::Call(call) = node else {
                        return None;
                    };
                    let lexical = typed_trees::references::StaticArgumentCall::Expression(
                        expression, call,
                    )
                    .lexical_owner(&source);
                    let factory = source.access_borrow_factory(call.target_symbol);
                    let containing = source.machine_holding_state(lexical);
                    (lexical == owner
                        || containing.is_some_and(|machine| machine.symbol == owner)
                        || factory.is_some())
                    .then(|| {
                        format!(
                            "call {} from {}; canonical factory: {}; retained application: {:?}",
                            describe(call.target_symbol),
                            describe(lexical),
                            factory.is_some(),
                            call.authored_machine_application,
                        )
                    })
                })
                .collect::<Vec<_>>();
            let factories = source
                .access_borrow_factories
                .iter()
                .map(|factory| describe(factory.factory))
                .collect::<Vec<_>>();
            panic!(
                "actual authored core borrow in {} was not found; observed: {observed:?}; retained factories: {factories:?}",
                describe(owner),
            );
        });
        assert!(calls.next().is_none());
        result
    };
    let (application, target) = call(relay);
    let (other_application, other_target) = call(other);
    let factory = source.access_borrow_factory(target).unwrap();
    let (entry_owner, _) = source.unique_machine_by_entry_state_symbol(target).unwrap();
    assert_eq!(entry_owner.symbol, factory.factory);
    let relay_entry = source
        .machine_by_symbol(relay)
        .map(|machine| source.machine_states(machine)[0].symbol)
        .unwrap();
    assert!(source.access_borrow_factory(relay_entry).is_none());
    let signature = source
        .bodyless_callable_signature(application, target)
        .unwrap()
        .unwrap();
    let other_signature = source
        .bodyless_callable_signature(other_application, other_target)
        .unwrap()
        .unwrap();
    assert_ne!(signature.return_type, other_signature.return_type);
    let bindings = source
        .expression_table
        .static_machine_application_bindings(application)
        .to_vec();
    let other_bindings = source
        .expression_table
        .static_machine_application_bindings(other_application);
    assert_eq!(bindings.len(), 2);
    assert_eq!(other_bindings.len(), 2);
    let mut changed = bindings;
    changed[1].value = other_bindings[1].value.clone();
    // Query the genuine frozen result above, then reopen only this corrupted
    // input copy. The prepared header must not authorize its changed binding.
    let mut source = source.into_inner();
    source
        .expression_table
        .set_static_machine_application_bindings(application, &changed);
    assert_eq!(
        source
            .bodyless_callable_signature(application, target)
            .unwrap_err(),
        "bodyless call source or bindings changed after contract preparation"
    );
}
