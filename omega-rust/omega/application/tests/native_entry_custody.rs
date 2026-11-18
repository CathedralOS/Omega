//! Exercise the published entry through the same dependencies as the production CLI.

#[test]
fn linux_entry_calling_plan_uses_psi_issued_custody_after_source_release() {
    use std::sync::Arc;

    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(3)
                .expect("application crate lives under omega-rust/omega");
            let main =
                repository.join("tests/fixtures/run/calls/scalar_contract_route_exit/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked.terminal_publication();
            let source = publication
                .require_published(publication.optimizations())
                .unwrap();
            let selections = source.source_selections().unwrap();
            let released = Arc::downgrade(selections);
            let module = source.artifact().semantic_module();
            let contract = module.program_entry_contract("linux_x86_64").unwrap();
            let schema = contract.schema_declaration;
            let other = module
                .machine(contract.entry)
                .unwrap()
                .declaration
                .as_ref()
                .unwrap()
                .semantic_declaration;
            assert_ne!(schema, other);
            let name = module.identities.text(schema).unwrap();
            let expected = selections
                .declaration_custody(
                    source.artifact().view().into(),
                    name,
                    semantic_vocabulary::source_selections::SelectionPurpose::Product,
                )
                .unwrap();

            // The bridge must retain the metadata Psi issued. Rebuilding its own
            // table while selections are still available would mask a reach-back.
            let bridge = products::PublishedProgram::from_source(&source);
            assert!(Arc::ptr_eq(
                source.entry_metadata(),
                bridge.entry_metadata()
            ));
            drop(source);
            let published = checked
                .with_published_program(Arc::new(bridge))
                .unwrap()
                .into_published()
                .unwrap();
            assert!(released.upgrade().is_none());

            let program = published.program();
            let actual = program.entry_schema_custody(schema).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                program.entry_schema_custody(other).unwrap_err(),
                "selected declaration has no authenticated source custody"
            );
            let settle = |requester| {
                let selection = products::PublishedNativeEntry::for_program(
                    program.artifact(),
                    published.execution(),
                )
                .unwrap();
                assert_eq!(selection.schema_declaration(), schema);
                let binding = selection
                    .binding_role()
                    .and_then(|role| published.package_inputs().accepted_semantic_binding(role));
                selection
                    .settle_calling_plans(
                        requester,
                        binding,
                        published.package_inputs().declaration_permissions(),
                    )
                    .unwrap()
            };
            let entry = settle(actual);
            assert!(entry.calling_plans().is_some());
            assert_eq!(entry, settle(expected));
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
