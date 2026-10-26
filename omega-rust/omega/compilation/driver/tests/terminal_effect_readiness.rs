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
    for machine in ["main", "storage_entry", "synchronous_entry"] {
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
                vec![1, if machine == "storage_entry" { 7 } else { 42 }, 42]
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
