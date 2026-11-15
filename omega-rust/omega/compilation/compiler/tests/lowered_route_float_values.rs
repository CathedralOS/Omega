//! Float values use ordinary scalar transport and canonical operator calls.
//! Receiving authenticates each numeric declaration and its complete IEEE
//! contract, before target planning gives it an executable realization.

fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_owned()
}

fn receiving_error(module: terminal_psi::TerminalModule) -> String {
    let produced = terminal_codec::ClosedTerminalModule::new(module).unwrap();
    match terminal_codec::ClosedTerminalModule::from_semantic_bytes(produced.semantic_bytes()) {
        Ok(_) => panic!("receiving accepted a changed canonical declaration"),
        Err(error) => format!("{error:?}"),
    }
}

#[test]
fn canonical_float_calls_keep_their_full_contract_and_reject_format_conversion() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository().join("tests/fixtures/run/float/canonical_comparisons/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            ).result.unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let publication = accepted.closure().publish().unwrap();
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(publication.artifact.semantic_bytes()).unwrap();
            let module = publication.artifact.semantic_module();
            let numeric = module.boundary_machines.iter().filter(|boundary|
                boundary.declaration.as_ref().is_some_and(|declaration|
                    declaration.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive)
            ).collect::<Vec<_>>();
            assert_eq!(numeric.len(), 12, "all six comparisons of each permanent format");
            for boundary in numeric {
                assert_eq!(boundary.scalar_ensures.len(), 2, "both directions of IEEE result equivalence");
                terminal_semantics::numeric_primitives::validate_boundary(module, boundary).unwrap();
                let mut wrong_entry = module.clone();
                let changed = wrong_entry.boundary_machines.iter_mut().find(|row| row.id == boundary.id).unwrap();
                changed.declaration.as_mut().unwrap().source.state = changed.owner.clone();
                assert_eq!(terminal_semantics::numeric_primitives::validate_boundary(&wrong_entry,
                    wrong_entry.boundary_machines.iter().find(|row| row.id == boundary.id).unwrap()).unwrap_err(),
                    "numeric primitive differs from its exact Core declaration and full contract: source entry state");
                let error = receiving_error(wrong_entry);
                assert!(error.contains("MalformedStructuralFoundation"), "{error}");
                for omitted in 0..2 {
                    let mut changed = module.clone();
                    changed.boundary_machines.iter_mut().find(|row| row.id == boundary.id)
                        .unwrap().scalar_ensures.remove(omitted);
                    let error = receiving_error(changed);
                    assert!(error.contains("MalformedStructuralFoundation"), "{error}");
                }
            }
            let schemas = provider_planning::compiler_primitive_schemas(module).unwrap();
            let leaves = provider_planning::direct_compiler_primitive_leaves(module, &schemas, "macos_arm64").unwrap();
            assert_eq!(leaves.len(), 12);
            for leaf in leaves {
                assert_eq!(leaf.target_realization, Some(target::CanonicalFloatRealization::CatalogIntrinsic {
                    isa: target::FloatIsaAdmission::Baseline,
                    controls: target::FloatControlObligation::CanonicalControls,
                }));
                leaf.validate_bound_calling_custody(module).unwrap();
                let mut missing = leaf.clone();
                missing.target_realization = None;
                assert!(missing.validate_bound_calling_custody(module).unwrap_err().contains("target realization"));
                let mut forged = leaf.clone();
                std::sync::Arc::make_mut(&mut forged.satisfied_schema.declaration).methods[0].declared_type_signature = [0; 32];
                std::sync::Arc::make_mut(&mut forged.plan.schema.declaration).methods[0].declared_type_signature = [0; 32];
                forged.validate_calling_custody().unwrap();
                assert!(forged.validate_bound_calling_custody(module).unwrap_err().contains("canonical declared schema"));
            }
            assert!(module.operations().all(|operation|
                !matches!(operation.kind, terminal_psi::OperationKind::IeeeFloatCompare { .. })),
                "canonical operators use the ordinary boundary call path");
            let invalid = repository().join("tests/fixtures/fail/float/canonical_comparisons/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            ).result.expect_err("an explicitly landed f64 cannot enter an f32 local").into_diagnostics();
            assert!(format!("{diagnostics:?}").contains("no implicit scalar conversion"), "{diagnostics:?}");
        }).unwrap().join().unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
