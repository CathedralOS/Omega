//! Real checked suspension sites park before host invocation, not by replaying
//! source or pretending scheduling-fuel exhaustion is provider readiness.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};

struct ControlledImport {
    signature: TerminalImportSignature,
    park: bool,
    ready: Arc<AtomicBool>,
    observations: Arc<Mutex<Vec<TerminalScalarValue>>>,
}

struct HostComputation(ControlledImport);

impl TerminalImportAdapter for HostComputation {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.0.implemented_signature()
    }
}

impl TerminalEffectHandler for HostComputation {
    fn poll_host_computation(
        &mut self,
        _: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(self.0.ready.load(Ordering::Relaxed))
    }

    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.0.handle_effect(effect)
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.0.handle_effect_result(effect)
    }
}

#[test]
fn synchronous_host_computation_retains_the_invocation_without_guest_suspension() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/concurrency/host_boundary_readiness_exit/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
        .expect("sealed fixture inputs");
    let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .expect("checked ordinary boundary call");
    let produced = psi_compiler::TerminalProductionRequest::new(
        &checked.execution().program,
        psi_compiler::TerminalMachineSelection::Name("synchronous_entry"),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
    .expect("Terminal production");
    let artifact = produced.artifact().expect("portable product");
    for loaded in [false, true] {
        let program = if loaded {
            TerminalProgram::load_envelope(&artifact.to_bytes()).expect("external admission")
        } else {
            TerminalProgram::borrow(artifact.view())
        };
        let requirements = program.execution_requirements(None).unwrap();
        let mut host_imports = requirements.imports;
        host_imports.extend(requirements.missing_providers);
        let supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                host_imports: &host_imports,
                installation: None,
                entry: None,
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .unwrap();
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                installation: Some(&supplies),
                ..Default::default()
            })
            .unwrap();
        let ready = Arc::new(AtomicBool::new(false));
        let observations = Arc::new(Mutex::new(Vec::new()));
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            let signature = TerminalImportSignature::from(instance.import(boundary).unwrap());
            instance
                .bind_import(
                    boundary,
                    HostComputation(ControlledImport {
                        signature,
                        park: false,
                        ready: Arc::clone(&ready),
                        observations: Arc::clone(&observations),
                    }),
                )
                .unwrap();
        }
        let mut meter = TerminalFuelMeter::unbounded();
        let pending = instance
            .invoke(&mut meter)
            .expect("executor pause, not a guest suspension");
        assert!(matches!(
            pending,
            TerminalExecutionStatus::HostComputationPending(_)
        ));
        let charged = meter.usage().total_units();
        for _ in 0..3 {
            assert_eq!(instance.invoke(&mut meter).unwrap(), pending);
            assert_eq!(meter.usage().total_units(), charged);
            assert!(observations.lock().unwrap().is_empty());
            assert!(instance.effects().is_empty());
            assert!(instance.result().is_none());
        }
        assert_eq!(
            instance.close(),
            Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
        );
        ready.store(true, Ordering::Relaxed);
        let completed = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
        assert_eq!(instance.invoke(&mut meter).unwrap(), completed);
        assert_eq!(instance.effects().len(), 1);
        assert!(matches!(
            observations.lock().unwrap().as_slice(),
            [TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(9),
                ..
            }]
        ));
        assert_eq!(instance.invoke(&mut meter).unwrap(), completed);
        assert_eq!(
            observations.lock().unwrap().len(),
            1,
            "completion never replays the host call"
        );
        instance.close().unwrap();
    }
}

#[test]
fn readiness_installs_exact_component_without_replaying_the_receiver() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/concurrency/host_boundary_readiness_exit/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
        .expect("sealed fixture inputs");
    let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .expect("checked component readiness fixture");
    for entry in ["component_entry", "component_premature_entry"] {
        let produced = psi_compiler::TerminalProductionRequest::new(
            &checked.execution().program,
            psi_compiler::TerminalMachineSelection::Name(entry),
            checked.sources().exact_toolchain_sources(),
        )
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("Terminal production");
        let artifact = produced.artifact().expect("portable artifact");
        let module = artifact.view().module();
        let candidates = module
            .provider_candidates
            .iter()
            .filter(|candidate| {
                module
                    .identities
                    .text(candidate.requirement_identity)
                    .is_some_and(|identity| identity.contains("Readiness::wait"))
            })
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 1, "exact authored Immediate backing");
        let candidate = candidates[0];
        let boundary = candidate.boundary;
        let selection = terminal_installation::ProviderInstallationSelection {
            boundary,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .unwrap()
                .into(),
            provider_declaration: module
                .identities
                .text(candidate.provider_declaration.identity())
                .unwrap()
                .into(),
            candidate: candidate.candidate,
        };
        let produce_invocation = || {
            let mut request = psi_compiler::TerminalProductionRequest::new(
                &checked.execution().program,
                psi_compiler::TerminalMachineSelection::Name(entry),
                checked.sources().exact_toolchain_sources(),
            );
            request.publication = terminal_codec::TerminalPublication::Invocation;
            request
                .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
                .unwrap()
        };
        let invocation = produce_invocation();
        let other_invocation = produce_invocation();
        assert!(invocation.artifact().is_err(), "receiver stays private");
        assert_eq!(
            invocation.view().identity(),
            other_invocation.view().identity(),
            "identical semantic content"
        );
        assert_ne!(
            invocation.view().custody_identity(),
            other_invocation.view().custody_identity(),
            "distinct live code owners"
        );
        for (receiver, program, loaded) in [
            (
                artifact.view().into(),
                TerminalProgram::borrow(artifact.view()),
                false,
            ),
            (
                artifact.view().into(),
                TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap(),
                true,
            ),
            (
                invocation.view(),
                TerminalProgram::borrow(invocation.view()),
                true,
            ),
        ] {
            let admitted = terminal_installation::admit_component_provider(
                receiver,
                boundary,
                artifact.view(),
                &selection,
            )
            .expect("exact checked component contract and receiver custody");
            let requirements = program.execution_requirements(None).unwrap();
            let host_imports = requirements
                .imports
                .iter()
                .chain(&requirements.missing_providers)
                .copied()
                .filter(|slot| *slot != boundary)
                .collect::<Vec<_>>();
            let supplies = program
                .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                    host_imports: &host_imports,
                    installation: None,
                    entry: None,
                    x86_flags: None,
                    memory_ordering: None,
                    target_machines: None,
                    target_layouts: None,
                })
                .unwrap();
            let supplies = program.defer_component(supplies, boundary).unwrap();
            if receiver.custody_identity() == invocation.view().custody_identity() {
                let other = TerminalProgram::borrow(other_invocation.view());
                assert!(
                    other
                        .instantiate(terminal_interpreter::TerminalInstantiation {
                            installation: Some(&supplies),
                            ..Default::default()
                        })
                        .is_err(),
                    "another private owner cannot reuse the execution plan"
                );
                let other_supplies = other
                    .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                        host_imports: &host_imports,
                        installation: None,
                        entry: None,
                        x86_flags: None,
                        memory_ordering: None,
                        target_machines: None,
                        target_layouts: None,
                    })
                    .unwrap();
                assert!(
                    other_supplies.with_component_provider(&admitted).is_err(),
                    "semantic equality does not transfer the admitted component selection"
                );
            }
            assert!(
                program.defer_component(supplies.clone(), boundary).is_err(),
                "reservation cannot replace a supply"
            );
            let mut instance = program
                .instantiate(terminal_interpreter::TerminalInstantiation {
                    installation: Some(&supplies),
                    ..Default::default()
                })
                .unwrap();
            let ready = Arc::new(AtomicBool::new(false));
            let computed = Arc::new(AtomicBool::new(false));
            let observations = Arc::new(Mutex::new(Vec::new()));
            assert!(
                matches!(
                    instance.invoke(&mut TerminalFuelMeter::unbounded()),
                    Err(terminal_interpreter::TerminalInstanceError::UnboundImports { .. })
                ),
                "ordinary host imports still bind before any effect"
            );
            for slot in instance.unbound_imports().collect::<Vec<_>>() {
                let signature = TerminalImportSignature::from(instance.import(slot).unwrap());
                let park = matches!(
                    signature.result,
                    terminal_psi::BoundaryMachineResult::Scalar(_)
                );
                let adapter = ControlledImport {
                    signature,
                    park,
                    ready: ready.clone(),
                    observations: observations.clone(),
                };
                let result = if park {
                    instance.bind_import(slot, adapter)
                } else {
                    instance.bind_import(
                        slot,
                        HostComputation(ControlledImport {
                            ready: Arc::clone(&computed),
                            ..adapter
                        }),
                    )
                };
                if slot == boundary {
                    assert_eq!(
                        result,
                        Err(
                            terminal_interpreter::TerminalBindingRejection::ProviderRealized {
                                boundary
                            }
                        )
                    );
                } else {
                    result.unwrap();
                }
            }
            let make_binding = || {
                let provider = if loaded {
                    TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
                } else {
                    TerminalProgram::borrow(artifact.view())
                };
                let supplies = provider
                    .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                        host_imports: &[],
                        installation: Some(admitted.selection()),
                        entry: Some(terminal_psi::TargetMachineCallable::Machine(
                            admitted.candidate(),
                        )),
                        x86_flags: None,
                        memory_ordering: None,
                        target_machines: None,
                        target_layouts: None,
                    })
                    .unwrap();
                terminal_interpreter::TerminalComponentBinding::new(
                    admitted.clone(),
                    provider,
                    supplies,
                )
                .unwrap()
            };
            let mut meter = TerminalFuelMeter::with_allowance(0);
            assert!(matches!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert!(
                instance.bind_component(make_binding()).is_err(),
                "fuel exhaustion is not a checked readiness crossing"
            );
            meter.replenish(100_000).unwrap();
            if entry == "component_premature_entry" {
                let refusal = instance.invoke(&mut meter).unwrap_err();
                assert!(
                    refusal
                        .to_string()
                        .contains("before its admitted installation"),
                    "{refusal}"
                );
                assert!(observations.lock().unwrap().is_empty());
                assert_eq!(instance.invoke(&mut meter), Err(refusal));
                assert!(
                    instance.bind_component(make_binding()).is_err(),
                    "failed invocation cannot be repaired by rebinding"
                );
                continue;
            }
            let computation = instance.invoke(&mut meter).unwrap();
            assert!(matches!(
                computation,
                TerminalExecutionStatus::HostComputationPending(_)
            ));
            assert!(
                instance.bind_component(make_binding()).is_err(),
                "a synchronous host computation grants no provider installation authority"
            );
            assert!(observations.lock().unwrap().is_empty());
            computed.store(true, Ordering::Relaxed);
            let paused = instance.invoke(&mut meter).unwrap();
            assert!(
                matches!(paused, TerminalExecutionStatus::AwaitingEffect(_)),
                "{paused:?}"
            );
            let charged = meter.usage().total_units();
            assert_eq!(instance.invoke(&mut meter).unwrap(), paused);
            assert_eq!(meter.usage().total_units(), charged);
            assert_eq!(observations.lock().unwrap().len(), 1);
            instance
                .bind_component(make_binding())
                .expect("install in the same paused receiver");
            assert!(
                instance.bind_component(make_binding()).is_err(),
                "no rebinding"
            );
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                paused,
                "installation is not readiness completion"
            );
            ready.store(true, Ordering::Relaxed);
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let observed = observations.lock().unwrap().clone();
            let values = observed
                .iter()
                .map(|value| match value {
                    TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    } => *value,
                    other => panic!("unexpected observation: {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                values,
                vec![1, 0, 42],
                "component result comes from real guest code, no prefix replay"
            );
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(*observations.lock().unwrap(), observed);
            instance.close().unwrap();
        }
    }
}

impl TerminalImportAdapter for ControlledImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for ControlledImport {
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        bytes: &mut [terminal_interpreter::TerminalBoundaryByteBuffer],
        storage: &mut [terminal_interpreter::TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        assert!(bytes.is_empty());
        if storage.is_empty() {
            return self.handle_effect_result(effect);
        }
        let [terminal_interpreter::TerminalBoundaryStorageBuffer::Scalar(buffer)] = storage else {
            return Err(TerminalEffectRejection::new(
                "expected the original scalar loan",
            ));
        };
        let original = buffer.value();
        self.observations.lock().unwrap().push(original);
        let TerminalScalarValue::Integer { scalar_type, .. } = original else {
            return Err(TerminalEffectRejection::new("expected an integer loan"));
        };
        buffer.replace(TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Signed(42),
        })?;
        Ok(TerminalEffectResult::Unit)
    }

    fn poll_effect_readiness(
        &mut self,
        _: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(!self.park || self.ready.load(Ordering::Relaxed))
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "use the exact result protocol",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            return Err(TerminalEffectRejection::new("expected a boundary call"));
        };
        let [value] = arguments.as_slice() else {
            return Err(TerminalEffectRejection::new("expected one scalar argument"));
        };
        self.observations.lock().unwrap().push(value.clone());
        Ok(match self.signature.result {
            terminal_psi::BoundaryMachineResult::Scalar(_) => {
                TerminalEffectResult::Scalar(value.clone())
            }
            terminal_psi::BoundaryMachineResult::Unit => TerminalEffectResult::Unit,
            _ => return Err(TerminalEffectRejection::new("unexpected result contract")),
        })
    }
}

#[test]
fn readiness_preserves_nested_continuation_and_rejects_unchecked_suspension() {
    let root = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/concurrency/host_boundary_readiness_exit/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
        .expect("sealed fixture inputs");
    let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .expect("checked suspension fixture");
    for machine in [
        "main",
        "storage_entry",
        "incoming_storage_entry",
        "synchronous_entry",
    ] {
        let produced = psi_compiler::TerminalProductionRequest::new(
            &checked.execution().program,
            psi_compiler::TerminalMachineSelection::Name(machine),
            checked.sources().exact_toolchain_sources(),
        )
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .expect("Terminal production");
        let artifact = produced.artifact().expect("portable product");
        for loaded in [false, true] {
            let program = if loaded {
                TerminalProgram::load_envelope(&artifact.to_bytes()).expect("external admission")
            } else {
                TerminalProgram::borrow(artifact.view())
            };
            let requirements = program.execution_requirements(None).unwrap();
            let mut host_imports = requirements.imports;
            host_imports.extend(requirements.missing_providers);
            let supplies = program
                .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                    host_imports: &host_imports,
                    installation: None,
                    entry: None,
                    x86_flags: None,
                    memory_ordering: None,
                    target_machines: None,
                    target_layouts: None,
                })
                .unwrap();
            let mut instance = program
                .instantiate(terminal_interpreter::TerminalInstantiation {
                    installation: Some(&supplies),
                    ..Default::default()
                })
                .unwrap();
            let ready = Arc::new(AtomicBool::new(false));
            let observations = Arc::new(Mutex::new(Vec::new()));
            for boundary in instance.unbound_imports().collect::<Vec<_>>() {
                let signature = TerminalImportSignature::from(instance.import(boundary).unwrap());
                let park = machine == "synchronous_entry"
                    || !signature.structural_parameters.is_empty()
                    || matches!(
                        signature.result,
                        terminal_psi::BoundaryMachineResult::Scalar(_)
                    );
                instance
                    .bind_import(
                        boundary,
                        ControlledImport {
                            signature,
                            park,
                            ready: ready.clone(),
                            observations: observations.clone(),
                        },
                    )
                    .unwrap();
            }
            let mut meter = TerminalFuelMeter::unbounded();
            if machine == "synchronous_entry" {
                let refusal = instance.invoke(&mut meter).unwrap_err();
                assert!(
                    refusal.to_string().contains("checked suspension crossing"),
                    "{refusal}"
                );
                let charged = meter.usage().total_units();
                ready.store(true, Ordering::Relaxed);
                assert_eq!(instance.invoke(&mut meter), Err(refusal));
                assert_eq!(meter.usage().total_units(), charged);
                assert!(observations.lock().unwrap().is_empty());
                continue;
            }
            let first = instance.invoke(&mut meter).unwrap();
            assert!(
                matches!(first, TerminalExecutionStatus::AwaitingEffect(_)),
                "{first:?}"
            );
            assert_eq!(
                observations.lock().unwrap().len(),
                1,
                "only the preceding effect ran"
            );
            let charged = meter.usage().total_units();
            for _ in 0..3 {
                assert_eq!(instance.invoke(&mut meter).unwrap(), first);
                assert_eq!(
                    meter.usage().total_units(),
                    charged,
                    "waiting is not repeated execution"
                );
                assert_eq!(observations.lock().unwrap().len(), 1);
                assert!(instance.result().is_none());
            }
            assert_eq!(
                instance.close(),
                Err(terminal_interpreter::TerminalInstanceCloseError::Busy)
            );
            ready.store(true, Ordering::Relaxed);
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let observed = observations.lock().unwrap().clone();
            let values = observed
                .iter()
                .map(|value| match value {
                    TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    } => *value,
                    other => panic!("unexpected observed value: {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                values,
                vec![
                    1,
                    if matches!(machine, "storage_entry" | "incoming_storage_entry") {
                        7
                    } else {
                        42
                    },
                    42
                ]
            );
            assert_eq!(instance.effects().len(), 3);
            assert_eq!(
                instance.invoke(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                *observations.lock().unwrap(),
                observed,
                "normal return cannot replay effects"
            );
            instance.close().unwrap();
        }
    }
}
