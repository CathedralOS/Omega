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

fn refuse_undeclared_conversion(module: &terminal_psi::TerminalModule) {
    use language_semantics::numeric_intrinsics::{NumericIntrinsic, NumericType};
    use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};
    use terminal_psi::NominalDeclarationIdentity;
    let original = NumericIntrinsic::Conversion {
        source: NumericType::Integer(FixedInteger::signed(32)),
        target: NumericType::F64,
        domain: ArithmeticDomain::Exact,
    };
    let invented = NumericIntrinsic::Conversion {
        source: NumericType::Integer(FixedInteger::unsigned(13)),
        target: NumericType::F64,
        domain: ArithmeticDomain::Exact,
    };
    // A valid rootless capsule isolates declaration receiving from unrelated
    // callers and their proof scopes. Corruption must reach the catalog guard.
    let mut declaration = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.numeric_intrinsic == Some(original))
        .unwrap()
        .clone();
    declaration.id = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    let module = terminal_psi::TerminalModule {
        identities: module.identities.clone(),
        boundary_machines: vec![declaration],
        ..terminal_psi::TerminalModule::declarations()
    };
    let baseline = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(baseline.semantic_bytes()).unwrap();
    let mut changed = module.clone();
    let boundary = changed
        .boundary_machines
        .iter_mut()
        .find(|boundary| boundary.numeric_intrinsic == Some(original))
        .unwrap();
    let mut identities = module.identities.clone().into_builder();
    // Forge the whole declaration, not only its meaning tag: family custody
    // alone used to describe a nonexistent Core member under the real source.
    let mut rekey = |identity| {
        identities.intern(
            &module
                .identities
                .text(identity)
                .unwrap()
                .replace("from_i32", "from_u13")
                .replace("i32", "u13"),
        )
    };
    boundary.owner = NominalDeclarationIdentity::new(rekey(boundary.owner.identity()));
    boundary.identity = rekey(boundary.identity);
    let source = &mut boundary.declaration.as_mut().unwrap().source;
    source.machine = NominalDeclarationIdentity::new(rekey(source.machine.identity()));
    source.state = NominalDeclarationIdentity::new(rekey(source.state.identity()));
    source.semantic_declaration = rekey(source.semantic_declaration);
    source.callable = rekey(source.callable);
    for parameter in &mut source.parameter_types {
        *parameter = rekey(*parameter);
    }
    boundary.numeric_intrinsic = Some(invented);
    boundary.scalar_parameters = vec![
        NumericType::Integer(FixedInteger::unsigned(13))
            .scalar_type()
            .unwrap(),
    ];
    boundary.scalar_ensures = terminal_semantics::numeric_primitives::ensures(invented).unwrap();
    let boundary_id = boundary.id;
    changed.identities = identities.build();
    let message = "numeric primitive has no exact admitted catalog declaration";
    assert_eq!(
        terminal_semantics::numeric_primitives::validate_boundary(
            &changed,
            changed.boundary_machine(boundary_id).unwrap()
        ),
        Err(message)
    );
    let error = receiving_error(changed);
    assert!(error.contains(message), "{error}");
}

#[test]
fn canonical_float_conversions_retain_integer_inputs_and_refuse_changed_carriers() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let main = repository().join("tests/fixtures/run/float/canonical_conversions/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("windows_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap();
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = publication.artifact.semantic_module();
            let mut conversions = 0;
            for boundary in &module.boundary_machines {
                let Some(language_semantics::numeric_intrinsics::NumericIntrinsic::Conversion {
                    source,
                    ..
                }) = boundary.numeric_intrinsic
                else {
                    continue;
                };
                conversions += 1;
                terminal_semantics::numeric_primitives::validate_boundary(module, boundary)
                    .unwrap();
                let [semantic_vocabulary::Proposition::FloatMeaningEqual(clause)] =
                    boundary.scalar_ensures.as_slice()
                else {
                    panic!("conversion keeps its complete result equation");
                };
                if let language_semantics::numeric_intrinsics::NumericType::Integer(_) = source {
                    assert!(
                        clause
                            .sources()
                            .any(|term| term.scalar_type() == source.scalar_type().unwrap())
                    );
                    assert_eq!(clause.projection_sources().count(), 1);
                }
                let mut changed = module.clone();
                changed
                    .boundary_machines
                    .iter_mut()
                    .find(|row| row.id == boundary.id)
                    .unwrap()
                    .scalar_ensures
                    .clear();
                assert!(receiving_error(changed).contains("MalformedStructuralFoundation"));
            }
            assert_eq!(conversions, 6);
            refuse_undeclared_conversion(module);
            let schemas = provider_planning::compiler_primitive_schemas(module).unwrap();
            let leaves = provider_planning::direct_compiler_primitive_leaves(
                module,
                &schemas,
                "windows_x86_64",
                module.boundary_machines.iter().map(|boundary| boundary.id),
            )
            .unwrap();
            for leaf in leaves {
                leaf.validate_bound_calling_custody(module).unwrap();
            }
            let invalid =
                repository().join("tests/fixtures/fail/float/canonical_conversions/main.omg");
            let diagnostic = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("windows_x86_64")),
            )
            .result
            .expect_err("an i64 does not match the selected i32 conversion")
            .into_diagnostics();
            let [diagnostic] = diagnostic.as_slice() else {
                panic!("one carrier mismatch: {diagnostic:?}");
            };
            assert!(diagnostic.message.starts_with("this value has scalar carrier `i64` but the parameter it is passed for requires `i32`: there is no implicit scalar conversion at a receiving type"), "{diagnostic:?}");
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
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
            let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
            let imports = program.imports().map(|declaration| declaration.id).collect::<Vec<_>>();
            assert_eq!(imports.len(), 12, "numeric declarations need admitted scalar adapters");
            assert!(imports.iter().all(|slot| !program.executes_boundary_internally(*slot)));
            let request = terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None, x86_flags: None, memory_ordering: None,
                entry: module.entry, installation: None, host_imports: &[],
                target_machines: None, target_layouts: None,
            };
            let empty = program.admit_runtime_supplies(request).unwrap();
            assert!(program.defer_component(empty, imports[0]).is_err(), "a component cannot replace a canonical numeric declaration");
            let installed = program.admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                host_imports: &imports, ..request
            }).unwrap();
            let requirements = program.execution_requirements(Some(&installed)).unwrap();
            assert_eq!(requirements.imports.len(), 12, "the dependency walk must retain numeric imports");
            let duplicates = [imports[0], imports[0]];
            assert!(program.admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                host_imports: &duplicates, ..request
            }).is_err(), "a canonical adapter is installed only once");
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
            let leaves = provider_planning::direct_compiler_primitive_leaves(module, &schemas, "macos_arm64", module.boundary_machines.iter().map(|boundary| boundary.id)).unwrap();
            assert_eq!(leaves.len(), 12);
            assert!(provider_planning::direct_compiler_primitive_leaves(module, &schemas, "uefi_x86_64", []).unwrap().is_empty(), "declarations alone do not request execution");
            let first = module.boundary_machines[0].id;
            assert!(provider_planning::direct_compiler_primitive_leaves(module, &schemas, "macos_arm64", [first, first]).is_err(), "a repeated request is not a second realization");
            assert!(provider_planning::direct_compiler_primitive_leaves(module, &schemas, "macos_arm64", [semantic_vocabulary::BoundaryMachineId::new(u64::MAX).unwrap()]).is_err(), "a request cannot name another module's declaration");
            let unsupported = provider_planning::direct_compiler_primitive_leaves(module, &schemas, "uefi_x86_64", module.boundary_machines.iter().map(|boundary| boundary.id)).unwrap_err();
            assert!(format!("{unsupported:?}").contains("the target declares no floating-point realization table"), "{unsupported:?}");
            let selection = effects::SelectedProviderPlanFacts::default()
                .with_direct_boundary_plans(leaves.iter().map(|leaf| leaf.plan.clone()).collect()).unwrap();
            let executions = selection.plans().iter().map(|plan| {
                let leaf = leaves.iter().find(|leaf| leaf.plan == *plan).unwrap();
                let backend_plan::DirectBoundaryExecution::Intrinsic(execution) = leaf.execution else { panic!("numeric leaf needs an intrinsic") };
                vec![execution]
            }).collect::<Vec<_>>();
            let demands = module.boundary_machines.iter().map(|boundary|
                module.identities.text(boundary.identity).unwrap().to_owned()).collect();
            let settle = |direct: &[backend_plan::DirectBoundaryLeaf]| {
                terminal_psi_to_abstract_operations::provider_admission::derive_selected_intrinsic_settlement_proposals(
                    &selection, selection.plans().iter().zip(&executions).map(|(plan, row_executions)|
                        provider_selection::CompilerIntrinsicPlanProvenance { plan, semantic_schema: &plan.schema, row_executions }),
                    &demands, module, direct)
            };
            let proposals = settle(&leaves).unwrap();
            assert_eq!(proposals.len(), 12);
            assert!(proposals.iter().all(|proposal| matches!(proposal.settlement(),
                terminal_psi_to_abstract_operations::provider_admission::TerminalCompilerBuiltinSettlement::Execution { .. })));
            assert!(settle(&[]).is_err(), "a numeric meaning alone supplies no execution custody");
            let mut missing_target = leaves.clone();
            missing_target[0].target_realization = None;
            assert!(format!("{:?}", settle(&missing_target).unwrap_err()).contains("target realization"));
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

#[test]
fn canonical_float_arithmetic_keeps_meaning_contracts_and_actual_source_classes() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            use language_semantics::numeric_intrinsics::{
                NumericIntrinsic, PrimitiveFloatBinaryOperation as Operation,
            };
            use semantic_vocabulary::{
                FloatMeaningClauseOperand, FloatMeaningClauseValue, ScalarTerm, ScalarType,
            };
            let main = repository().join("tests/fixtures/run/float/canonical_arithmetic/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap();
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = publication.artifact.semantic_module();
            let arithmetic = module
                .boundary_machines
                .iter()
                .filter(|boundary| {
                    matches!(
                        boundary.numeric_intrinsic,
                        Some(NumericIntrinsic::FloatBinary {
                            operation: Operation::Add
                                | Operation::Subtract
                                | Operation::Multiply
                                | Operation::Divide,
                            ..
                        })
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(arithmetic.len(), 8);
            for boundary in &arithmetic {
                terminal_semantics::numeric_primitives::validate_boundary(module, boundary)
                    .unwrap();
                assert_eq!(boundary.scalar_ensures.len(), 1);
                let semantic_vocabulary::Proposition::FloatMeaningEqual(clause) =
                    &boundary.scalar_ensures[0]
                else {
                    panic!("arithmetic needs meaning equality, not IEEE equality or Truth")
                };
                clause.validate().unwrap();
                let terminal_psi::BoundaryMachineResult::Scalar(result) = boundary.result else {
                    panic!("scalar arithmetic result")
                };
                assert!(matches!(result.scalar_type, ScalarType::IeeeFloat(_)));
            }
            let subtract = arithmetic
                .iter()
                .copied()
                .find(|boundary| {
                    matches!(
                        boundary.numeric_intrinsic,
                        Some(NumericIntrinsic::FloatBinary {
                            operation: Operation::Subtract,
                            ..
                        })
                    )
                })
                .unwrap();
            let mut reversed = module.clone();
            let changed = reversed
                .boundary_machines
                .iter_mut()
                .find(|row| row.id == subtract.id)
                .unwrap();
            let semantic_vocabulary::Proposition::FloatMeaningEqual(clause) =
                &mut changed.scalar_ensures[0]
            else {
                unreachable!()
            };
            let application = clause
                .values
                .iter_mut()
                .find_map(|row| match row {
                    FloatMeaningClauseValue::Application { operands, .. } => Some(operands),
                    _ => None,
                })
                .unwrap();
            assert!(matches!(
                application[0],
                FloatMeaningClauseOperand::Format(_)
            ));
            application.swap(1, 2);
            clause.validate().unwrap();
            let error = receiving_error(reversed);
            assert!(error.contains("full canonical result contract"), "{error}");

            // Corrupt a real catalog signature commitment in encoded bytes,
            // so the receiving codec, rather than the producer, refuses it.
            let semantic_vocabulary::Proposition::FloatMeaningEqual(clause) =
                &subtract.scalar_ensures[0]
            else {
                unreachable!()
            };
            let commitment = clause
                .values
                .iter()
                .find_map(|row| match row {
                    FloatMeaningClauseValue::Application { contract, .. } => {
                        Some(contract.commitment)
                    }
                    _ => None,
                })
                .unwrap();
            let mut bytes = publication.artifact.semantic_bytes().to_vec();
            let position = bytes
                .windows(commitment.len())
                .position(|window| window == commitment)
                .unwrap();
            bytes[position] ^= 1;
            assert!(terminal_codec::ClosedTerminalModule::from_semantic_bytes(&bytes).is_err());

            let (machine, operation, result) = module
                .machines
                .iter()
                .find_map(|machine| {
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .find_map(|operation| {
                            let result = operation.result.scalar_ref()?;
                            (matches!(result.scalar_type, ScalarType::IeeeFloat(_))
                                && operation.declared_invocation().is_some())
                            .then_some((machine, operation, result))
                        })
                })
                .unwrap();
            let sources =
                terminal_semantics::float_meaning_sources::Sources::for_machine(machine).unwrap();
            let term = ScalarTerm::value(result.id, result.scalar_type);
            let actual = sources.source(&term).unwrap();
            let terminal_psi::FloatMeaningSource::DirectCallResult(actual) = actual else {
                panic!("boundary results are call results")
            };
            assert_eq!(actual.producer, operation.id);
            let mut wrong_owner = actual;
            wrong_owner.owner = semantic_vocabulary::MachineId::new(u64::MAX).unwrap();
            assert!(
                sources
                    .validate_source(
                        &term,
                        &terminal_psi::FloatMeaningSource::DirectCallResult(wrong_owner)
                    )
                    .is_err()
            );
            let wrong_class = terminal_psi::FloatMeaningSource::DirectOperationResult(
                terminal_psi::DirectOperationFloatResult {
                    owner: actual.owner,
                    producer: actual.producer,
                    result: actual.result,
                    format: actual.format,
                },
            );
            assert!(sources.validate_source(&term, &wrong_class).is_err());

            let invalid =
                repository().join("tests/fixtures/fail/float/canonical_arithmetic/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            )
            .result
            .expect_err("f64 cannot silently enter f32 arithmetic")
            .into_diagnostics();
            let message = format!("{diagnostics:?}");
            assert!(
                message.contains("float arithmetic operand types differ: f32 and f64"),
                "{message}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

#[test]
fn inline_float_comparison_keeps_the_literal_format_in_the_saved_operator() {
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType};

    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let fixture =
                repository().join("tests/fixtures/run/float/inline_literal_comparison/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&fixture, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let publication = accepted.closure().publish().unwrap();
            let reopened = terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = reopened.module();
            let [operator] = module.boundary_machines.as_slice() else {
                panic!("the inline comparison selects exactly one numeric declaration");
            };
            assert_eq!(
                operator.scalar_parameters,
                [ScalarType::IeeeFloat(IeeeFloatFormat::Binary64); 2]
            );
            assert!(
                module
                    .identities
                    .text(operator.identity)
                    .unwrap()
                    .contains("Float::equal")
            );
            terminal_semantics::numeric_primitives::validate_boundary(module, operator).unwrap();

            let invalid =
                repository().join("tests/fixtures/fail/float/inline_literal_comparison/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            )
            .result
            .expect_err("f64 and the explicitly suffixed f32 cannot select one equality")
            .into_diagnostics();
            let diagnostic = format!("{diagnostics:?}");
            assert!(diagnostic.contains("typing operand types"), "{diagnostic}");
            assert!(diagnostic.contains("__transition_subject"), "{diagnostic}");
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

#[test]
fn float_zero_locals_keep_positive_bits_and_require_the_default_domain() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            use semantic_vocabulary::{IeeeFloatValue, ScalarType};
            let source =
                repository().join("tests/fixtures/run/float/zero_constructed_locals/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&source, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap();
            let reopened = terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = reopened.module();
            let mut initializers = Vec::new();
            for machine in &module.machines {
                for operation in machine.operations() {
                    let terminal_psi::OperationKind::EstablishPrimitiveLocal { value, .. } =
                        operation.kind
                    else {
                        continue;
                    };
                    let producer = machine
                        .operations()
                        .find(|producer| {
                            producer
                                .result
                                .scalar_ref()
                                .is_some_and(|row| row.id == value)
                        })
                        .unwrap();
                    let terminal_psi::OperationKind::IeeeFloatConstant { value } = producer.kind
                    else {
                        continue;
                    };
                    let home = operation.result.as_structural().unwrap().structural_type;
                    let declaration = module
                        .structural_types
                        .iter()
                        .find(|row| row.id == home)
                        .unwrap();
                    assert_eq!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                            value.format()
                        ))
                    );
                    initializers.push(value);
                }
            }
            initializers.sort();
            assert_eq!(
                initializers,
                [IeeeFloatValue::Binary32(0), IeeeFloatValue::Binary64(0)]
            );

            let invalid =
                repository().join("tests/fixtures/fail/float/zero_constructed_locals/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            )
            .result
            .expect_err("zero storage cannot establish the default domain that excludes zero")
            .into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("cannot prove requires clause")
                        && diagnostic.message.contains("data type")
                        && diagnostic.message.contains("probe")
                }),
                "{diagnostics:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| !diagnostic.message.contains("not supported yet")),
                "{diagnostics:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

#[test]
fn exact_float_widening_retains_its_unary_contract_and_rejects_a_qualified_operand() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            use language_semantics::numeric_intrinsics::{NumericIntrinsic, NumericType};
            use semantic_vocabulary::{
                FloatMeaningClauseValue, IeeeFloatFormat, ScalarType, ValueId,
            };
            let main = repository().join("tests/fixtures/run/float/exact_widening/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap();
            terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = publication.artifact.semantic_module();
            let meaning = NumericIntrinsic::Conversion {
                source: NumericType::F32,
                target: NumericType::F64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            };
            let boundary = module
                .boundary_machines
                .iter()
                .find(|boundary| boundary.numeric_intrinsic == Some(meaning))
                .unwrap();
            assert_eq!(
                boundary.scalar_parameters,
                [ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)]
            );
            assert_eq!(
                boundary.result.scalar(),
                Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
            );
            let [semantic_vocabulary::Proposition::FloatMeaningEqual(clause)] =
                boundary.scalar_ensures.as_slice()
            else {
                panic!("conversion must publish its complete meaning equation")
            };
            let sources = clause.sources().cloned().collect::<Vec<_>>();
            assert!(sources.contains(&semantic_vocabulary::ScalarTerm::value(
                ValueId::new(1).unwrap(),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)
            )));
            assert!(sources.contains(&semantic_vocabulary::ScalarTerm::value(
                ValueId::new(2).unwrap(),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
            )));
            let applications = clause
                .values
                .iter()
                .filter_map(|row| match row {
                    FloatMeaningClauseValue::Application {
                        contract,
                        format,
                        operands,
                    } => Some((contract, format, operands)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(applications.len(), 1);
            assert_eq!(*applications[0].1, IeeeFloatFormat::Binary64);
            assert_eq!(
                applications[0].2.len(),
                2,
                "destination format and one source meaning"
            );
            let schemas = provider_planning::compiler_primitive_schemas(module).unwrap();
            let leaves = provider_planning::direct_compiler_primitive_leaves(
                module,
                &schemas,
                "macos_arm64",
                module.boundary_machines.iter().map(|boundary| boundary.id),
            )
            .unwrap();
            let leaf = leaves
                .iter()
                .find(|leaf| {
                    leaf.callable_identity == module.identities.text(boundary.identity).unwrap()
                })
                .unwrap();
            leaf.validate_bound_calling_custody(module).unwrap();
            assert_eq!(
                leaf.satisfied_schema.declaration.methods[0].requirement_owner,
                "F64"
            );
            assert_eq!(
                leaf.satisfied_schema.declaration.methods[0].name,
                "from_f32"
            );
            // Named conversions now belong to real modules and boundary
            // machines; their runtime operation and complete equation stay the same.
            assert_eq!(
                boundary.kind,
                semantic_vocabulary::BoundaryRequirementKind::Machine
            );
            for altered in 0..2 {
                let mut changed = module.clone();
                let row = changed
                    .boundary_machines
                    .iter_mut()
                    .find(|row| row.id == boundary.id)
                    .unwrap();
                match altered {
                    0 => {
                        row.scalar_parameters[0] = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
                    }
                    1 => row.scalar_ensures.clear(),
                    _ => unreachable!(),
                }
                assert!(receiving_error(changed).contains("MalformedStructuralFoundation"));
            }
            // Corrupt the published bytes so receiving, rather than the
            // producer's equation validation, must reject the catalog change.
            let commitment = applications[0].0.commitment;
            let mut bytes = publication.artifact.semantic_bytes().to_vec();
            let position = bytes
                .windows(commitment.len())
                .position(|window| window == commitment)
                .unwrap();
            bytes[position] ^= 1;
            assert!(terminal_codec::ClosedTerminalModule::from_semantic_bytes(&bytes).is_err());
            let invalid = repository().join("tests/fixtures/fail/float/exact_widening/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            )
            .result
            .expect_err("carrier cast must remove the source domain")
            .into_diagnostics();
            let diagnostics = format!("{diagnostics:?}");
            assert!(
                diagnostics.contains("operand of a carrier cast carries no domain"),
                "{diagnostics}"
            );
            assert!(!diagnostics.contains("not supported yet"), "{diagnostics}");
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

#[test]
fn exact_float_narrowing_uses_a_prior_guarantee_and_refuses_precision_loss() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            use semantic_vocabulary::{
                FloatMeaningClauseValue, IeeeFloatFormat, Proposition, ScalarType,
            };
            let main = repository().join("tests/fixtures/run/float/exact_narrowing/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let publication = checked
                .terminal_publication()
                .accepted()
                .unwrap()
                .closure()
                .publish()
                .unwrap();
            let reopened = terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                publication.artifact.semantic_bytes(),
            )
            .unwrap();
            let module = reopened.module();
            let goals = module
                .scalar_block_invariants
                .iter()
                .filter_map(|invariant| {
                    let Proposition::FloatMeaningEqual(clause) = &invariant.predicate else {
                        return None;
                    };
                    Some((invariant, clause))
                })
                .collect::<Vec<_>>();
            assert_eq!(goals.len(), 1, "one exactness obligation before narrowing");
            let (invariant, clause) = goals[0];
            assert!(!invariant.arrivals.is_empty());
            let sources = clause.sources().collect::<Vec<_>>();
            assert_eq!(
                sources.len(),
                1,
                "the goal names only the existing wide operand"
            );
            assert_eq!(
                sources[0].scalar_type(),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
            );
            let formats = clause
                .values
                .iter()
                .filter_map(|row| match row {
                    FloatMeaningClauseValue::Application { format, .. } => Some(*format),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                formats,
                [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64]
            );
            for boundary in &module.boundary_machines {
                if boundary.numeric_intrinsic.is_some() {
                    terminal_semantics::numeric_primitives::validate_boundary(module, boundary)
                        .unwrap();
                }
            }
            let invalid = repository().join("tests/fixtures/fail/float/exact_narrowing/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&invalid, Some("macos_arm64")),
            )
            .result
            .expect_err("range alone cannot justify loss of precision")
            .into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove the required condition of this cast")),
                "{diagnostics:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| !diagnostic.message.contains("not supported yet")),
                "{diagnostics:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
