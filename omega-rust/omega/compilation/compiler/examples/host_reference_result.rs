//! Public SDK completion of captured-input reference results and refusal controls.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationInvocationAccount,
    EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment as Path, IntegerValue,
};
use std::cell::RefCell;
use std::rc::Rc;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryReferenceSelection, TerminalBoundaryStorageBuffer,
    TerminalCallStorageRequirements, TerminalEffect, TerminalEffectHandler,
    TerminalEffectRejection, TerminalEffectResult, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalImportAdapter, TerminalImportSignature, TerminalInstantiation,
    TerminalProgram, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::StructuralAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Good,
    NoOptIn,
    NoCapture,
    Stale,
    WrongType,
    WrongPath,
    Shared,
    Opaque,
    Materialized,
    WrongRegion,
    Quota,
}
#[derive(Default)]
struct Observation {
    calls: usize,
    first: Option<TerminalBoundaryReferenceSelection>,
    blocker: Option<evaluation_accounting::EvaluationStorageLease>,
}
struct Host {
    signature: TerminalImportSignature,
    account: EvaluationInvocationAccount,
    sponsor: BuildEvaluationSponsor,
    mode: Mode,
    value_path: Vec<Path>,
    observed: Rc<RefCell<Observation>>,
}
impl TerminalImportAdapter for Host {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for Host {
    fn accepts_reference_results(&self, _: BoundaryMachineId) -> bool {
        self.mode != Mode::NoOptIn
    }
    fn storage_requirements(
        &self,
        _: BoundaryMachineId,
        _: &[TerminalScalarValue],
    ) -> TerminalCallStorageRequirements<'_> {
        TerminalCallStorageRequirements {
            arguments: if self.mode == Mode::NoCapture {
                &[]
            } else if self.signature.structural_parameters.len() == 2 {
                &[0, 1]
            } else {
                &[0]
            },
            original: false,
        }
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        unreachable!("buffer interface")
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        _: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let mut observed = self.observed.borrow_mut();
        observed.calls += 1;
        let TerminalBoundaryStorageBuffer::Structural(buffer) = &mut buffers[0] else {
            panic!("Cell storage")
        };
        let mut values = buffer.values().to_vec();
        let first_field = self.value_path.clone();
        // Use the admitted nominal field identity and preserve both neighbors.
        let value = values
            .iter_mut()
            .find(|value| value.path == first_field)
            .unwrap();
        let TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { value, .. },
            ..
        } = &mut value.contents
        else {
            panic!("u64")
        };
        *value = IntegerValue::Unsigned(0x2525252525252525);
        buffer.replace(values)?;
        if self.mode == Mode::Materialized {
            let fields = buffer
                .values()
                .iter()
                .map(|field| {
                    let TerminalStorageContents::Scalar { value, .. } = &field.contents else {
                        panic!("scalar Cell fields")
                    };
                    (field.path.clone(), *value)
                })
                .collect();
            return Ok(TerminalEffectResult::Reference(
                terminal_interpreter::TerminalReferenceResult {
                    referent: terminal_interpreter::TerminalReferencePayload::ScalarAggregate(
                        terminal_interpreter::TerminalScalarAggregateValue {
                            structural_type: buffer.structural_type(),
                            result_case: None,
                            fields,
                        },
                    ),
                    references: Vec::new(),
                },
            ));
        }
        let access = if self.mode == Mode::Shared {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::MutableBorrow
        };
        let path = match self.mode {
            Mode::WrongType => first_field,
            Mode::WrongPath => vec![Path::FixedIndex(999)],
            _ => vec![],
        };
        let selected_input = if self.mode == Mode::WrongRegion { 1 } else { 0 };
        let current = buffers[selected_input].select_reference(&path, access)?;
        assert!(
            self.sponsor.live_storage_cells() > 0,
            "captured storage is accounted"
        );
        if self.mode == Mode::Quota {
            let remaining = 1_000_000 - self.sponsor.live_storage_cells();
            observed.blocker = Some(self.account.reserve_storage(remaining, 0).unwrap());
        }
        if self.mode == Mode::Opaque {
            let TerminalEffect::BoundaryCall {
                result: terminal_psi::BoundaryMachineResult::Structural(result),
                ..
            } = effect
            else {
                unreachable!()
            };
            return Ok(TerminalEffectResult::Structural(
                terminal_interpreter::TerminalStructuralValue::minted(
                    999,
                    result.structural_type,
                    vec![],
                ),
            ));
        }
        let selected = if self.mode == Mode::Stale && observed.calls > 1 {
            observed.first.clone().unwrap()
        } else {
            current.clone()
        };
        observed.first.get_or_insert(current);
        Ok(TerminalEffectResult::CapturedReference(selected))
    }
}

fn main() {
    for (project, region_control) in [
        (
            "samples/cli/facts/foreign_reference_result/local/main.omg",
            false,
        ),
        (
            "samples/cli/facts/foreign_reference_result/controls/host_regions/main.omg",
            true,
        ),
    ] {
        let root = fixture_package_inputs::repo_root().join(project);
        let prepared =
            fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
                .expect("ordinary package acceptance");
        let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
        request.package_inputs = prepared.package_inputs;
        let checked =
            orchestration::check_source(&mut compiler::CompilationSession::default(), request)
                .result
                .unwrap_or_else(|error| panic!("{:?}", error.into_diagnostics()));
        let produced = psi_compiler::TerminalProductionRequest::new(
            checked.source_inspection().program(),
            psi_compiler::TerminalMachineSelection::Name("main"),
            checked.sources().exact_toolchain_sources(),
        )
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .unwrap();
        let artifact = produced.artifact().unwrap();
        for decoded in [false, true] {
            let program = if decoded {
                TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
            } else {
                TerminalProgram::borrow(artifact.view())
            };
            let modes: &[Mode] = if region_control {
                &[Mode::Good, Mode::WrongRegion]
            } else {
                &[
                    Mode::Good,
                    Mode::NoOptIn,
                    Mode::NoCapture,
                    Mode::Stale,
                    Mode::WrongType,
                    Mode::WrongPath,
                    Mode::Shared,
                    Mode::Opaque,
                    Mode::Materialized,
                    Mode::Quota,
                ]
            };
            for &mode in modes {
                let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                    fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                    storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                };
                let limits = BuildEvaluationSponsorLimits::new(
                    1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
                    1_000_000,
                )
                .unwrap();
                let sponsor =
                    BuildEvaluationSponsor::new_with_measurement(limits, measurement).unwrap();
                let account = sponsor.begin_invocation(measurement, 100_000).unwrap();
                let observed = Rc::new(RefCell::new(Observation::default()));
                let mut instance = program
                    .instantiate(TerminalInstantiation {
                        account: Some(&account),
                        ..Default::default()
                    })
                    .unwrap();
                let imports = instance.unbound_imports().collect::<Vec<_>>();
                assert_eq!(imports.len(), 1);
                let declaration = artifact
                    .semantic_module()
                    .boundary_machine(imports[0])
                    .unwrap();
                assert_eq!(declaration.result_reference_regions.len(), 1);
                let cell = artifact
                    .semantic_module()
                    .structural_types
                    .iter()
                    .find(|row| row.id == declaration.structural_parameters[0].structural_type)
                    .unwrap();
                let terminal_psi::StructuralTypeShape::Record { fields, .. } = &cell.shape else {
                    panic!("Cell")
                };
                let value_path = vec![Path::Field(
                    fields
                        .iter()
                        .find(|field| {
                            artifact.semantic_module().identities.text(field.identity)
                                == Some("value")
                        })
                        .unwrap()
                        .id,
                )];
                instance
                    .bind_import(
                        imports[0],
                        Host {
                            signature: declaration.into(),
                            account: account.clone(),
                            sponsor: sponsor.clone(),
                            mode,
                            value_path,
                            observed: Rc::clone(&observed),
                        },
                    )
                    .unwrap();
                let result = instance.invoke(
                    &mut terminal_fuel::TerminalFuelMeter::with_invocation(
                        account.clone(),
                        Some(100_000),
                    )
                    .unwrap(),
                );
                if mode == Mode::Good {
                    assert_eq!(
                        result.unwrap(),
                        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                    );
                    assert_eq!(
                        observed.borrow().calls,
                        2,
                        "each authored call executes exactly once"
                    );
                } else {
                    assert!(result.is_err(), "{mode:?} must reject, got {result:?}");
                    assert_eq!(
                        observed.borrow().calls,
                        match mode {
                            Mode::NoOptIn | Mode::NoCapture => 0,
                            Mode::Stale => 2,
                            _ => 1,
                        }
                    );
                    assert!(instance.result().is_none());
                    let error = format!("{:?}", result.unwrap_err());
                    let expected = match mode {
                        Mode::NoOptIn => Some("explicit captured-input interface"),
                        Mode::NoCapture => Some("no admitted borrowed input capture"),
                        Mode::WrongRegion => Some("exceeds its public input regions"),
                        Mode::Quota => Some("LiveStorage"),
                        Mode::Materialized => Some("no invocation-owned public region"),
                        _ => None,
                    };
                    if let Some(expected) = expected {
                        assert!(error.contains(expected), "{mode:?}: {error}");
                    }
                    println!("refusal region={region_control} mode={mode:?}: {error}");
                    observed.borrow_mut().blocker.take();
                    let calls = observed.borrow().calls;
                    assert!(
                        instance
                            .invoke(
                                &mut terminal_fuel::TerminalFuelMeter::with_invocation(
                                    account.clone(),
                                    Some(100_000)
                                )
                                .unwrap()
                            )
                            .is_err(),
                        "completion fault stays latched"
                    );
                    assert_eq!(
                        observed.borrow().calls,
                        calls,
                        "failed completion cannot replay host effects"
                    );
                }
                drop(instance);
                assert_eq!(
                    sponsor.live_storage_cells(),
                    0,
                    "closing releases all interpreter storage"
                );
                assert_eq!(
                    sponsor.live_payload_bytes(),
                    0,
                    "closing releases all interpreter payload"
                );
                println!(
                    "host-reference region={region_control} decoded={decoded} mode={mode:?} calls={} passed",
                    observed.borrow().calls
                );
            }
        }
    }
}
