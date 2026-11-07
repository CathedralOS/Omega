//! A returned loan retains its actual entry backing until the instance closes.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use psi_work::WorkMeter;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerValue};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstantiation, TerminalProgram, TerminalReturnedLoanError, TerminalScalarValue,
    TerminalStorageContents, TerminalStructuralContents, TerminalStructuralInputs,
    TerminalStructuralValue,
};

fn produce(project: &str, entries: &[&str]) -> Vec<psi_compiler::ProducedTerminalProgram> {
    let root = fixture_package_inputs::repo_root().join(project);
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
        .expect("ordinary fixture acceptance");
    let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked =
        orchestration::check_source(&mut compiler::CompilationSession::default(), request)
            .result
            .unwrap_or_else(|error| panic!("checked borrow fact: {:?}", error.into_diagnostics()));
    entries
        .iter()
        .map(|entry| {
            psi_compiler::TerminalProductionRequest::new(
                checked.source_inspection().program(),
                psi_compiler::TerminalMachineSelection::Name(entry),
                checked.sources().exact_toolchain_sources(),
            )
            .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
            .expect("source-produced borrowed result")
        })
        .collect()
}

fn cell_contents(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    argument_index: u32,
    structural_type: semantic_vocabulary::StructuralTypeId,
    contents: [(&str, u128); 3],
) -> TerminalStructuralContents {
    let module = artifact.semantic_module();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("the fixture lends Cell records")
    };
    let mut values = contents
        .into_iter()
        .map(|(name, value)| {
            let field = fields
                .iter()
                .find(|field| module.identities.text(field.identity) == Some(name))
                .unwrap();
            let scalar_type = field.field_type.scalar_type().unwrap();
            let semantic_vocabulary::ScalarType::Integer(integer) = scalar_type else {
                panic!("the fixture's Cell fields are integers")
            };
            TerminalBoundaryStorageValue {
                path: vec![Path::Field(field.id)],
                contents: TerminalStorageContents::Scalar {
                    value: TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(value),
                    },
                    stored_scalar: scalar_type,
                },
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.path.cmp(&right.path));
    TerminalStructuralContents {
        argument_index,
        values,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultShape {
    Original,
    Mutated,
    Optional,
    WriteOnly,
}

fn main() {
    let scenarios: &[(&str, &[(&str, ResultShape)])] = &[
        (
            "tests/omega/pass/facts/build_activation_borrow/field/main.omg",
            &[("selected", ResultShape::Original)],
        ),
        (
            "samples/cli/facts/build_activation_borrow/controls/entry_return/main.omg",
            &[
                ("host_selected", ResultShape::Mutated),
                ("host_optional", ResultShape::Optional),
                ("host_write_only", ResultShape::WriteOnly),
            ],
        ),
    ];
    for (project, entries) in scenarios {
        let names = entries.iter().map(|(name, _)| *name).collect::<Vec<_>>();
        for ((name, shape), produced) in entries.iter().zip(produce(project, &names)) {
            let artifact = produced
                .artifact()
                .expect("portable returned-loan artifact");
            for decoded in [false, true] {
                let program = if decoded {
                    TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
                } else {
                    TerminalProgram::borrow(artifact.view())
                };
                let entry = program.entry().unwrap();
                let result_type = entry.result.structural().unwrap().structural_type;
                let path = loan_path(artifact, result_type, *shape);
                let arguments = entry
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| {
                        TerminalStructuralValue::minted(
                            41 + u64::try_from(ordinal).unwrap(),
                            parameter.structural_type,
                            Vec::new(),
                        )
                    })
                    .collect::<Vec<_>>();
                let contents = [
                    cell_contents(
                        artifact,
                        0,
                        arguments[0].structural_type,
                        [("before", 11), ("value", 5), ("after", 17)],
                    ),
                    cell_contents(
                        artifact,
                        1,
                        arguments[1].structural_type,
                        [("before", 23), ("value", 9), ("after", 29)],
                    ),
                ];
                for first in [true, false] {
                    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                        work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                    };
                    let limits = BuildEvaluationSponsorLimits::new(
                        1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
                        1_000_000, 1_000_000,
                    )
                    .unwrap();
                    let sponsor =
                        BuildEvaluationSponsor::new_with_measurement(limits, measurement).unwrap();
                    let account = sponsor.begin_invocation(measurement, 100_000).unwrap();
                    if *shape == ResultShape::Mutated && !decoded && first {
                        let mut refused = program
                            .instantiate(TerminalInstantiation {
                                scalar_arguments: &[TerminalScalarValue::Boolean(first)],
                                structural_inputs: TerminalStructuralInputs {
                                    arguments: &arguments,
                                    contents: &contents,
                                    ..Default::default()
                                },
                                account: Some(&account),
                                ..Default::default()
                            })
                            .unwrap();
                        refused.retain_returned_loans().unwrap();
                        let remaining =
                            limits.maximum_live_storage_cells() - sponsor.live_storage_cells();
                        let blocker = account.reserve_storage(remaining - 2, 0).unwrap();
                        let reserved = sponsor.live_storage_cells();
                        let error = refused.invoke(&mut WorkMeter::with_invocation(account.clone(), Some(0)).unwrap()).expect_err("two cells cannot hold one return slot and both possible input backings");
                        assert!(
                            matches!(error, terminal_interpreter::TerminalInstanceError::Execution(terminal_interpreter::TerminalInterpretError::ExecutionSite {cause, ..}) if matches!(*cause, terminal_interpreter::TerminalInterpretError::Storage(evaluation_accounting::EvaluationAccountingError::BudgetExhausted(evaluation_accounting::EvaluationResource::LiveStorage))))
                        );
                        assert_eq!(
                            sponsor.live_storage_cells(),
                            reserved,
                            "failed capture must reserve nothing"
                        );
                        assert_eq!(
                            account.snapshot().unwrap().record().work_units,
                            0,
                            "capture refusal precedes guest work"
                        );
                        drop(blocker);
                        assert!(
                            refused
                                .invoke(
                                    &mut WorkMeter::with_invocation(account.clone(), Some(100_000))
                                        .unwrap()
                                )
                                .is_err(),
                            "a host fault is latched, not a replayable suspension"
                        );
                        drop(refused);
                        assert_eq!(sponsor.live_storage_cells(), 0);
                        println!(
                            "underfunded entry capture refused atomically before guest work; fault remains latched"
                        );
                    }
                    let mut instance = program
                        .instantiate(TerminalInstantiation {
                            scalar_arguments: &[TerminalScalarValue::Boolean(first)],
                            structural_inputs: TerminalStructuralInputs {
                                arguments: &arguments,
                                contents: &contents,
                                ..Default::default()
                            },
                            account: Some(&account),
                            ..Default::default()
                        })
                        .unwrap();
                    assert!(matches!(
                        instance.returned_loan(&path),
                        Err(TerminalReturnedLoanError::AdmissionRequired)
                    ));
                    instance.retain_returned_loans().unwrap();
                    assert!(matches!(
                        instance.retain_returned_loans(),
                        Err(TerminalReturnedLoanError::AdmissionAlreadyProvided)
                    ));
                    if *shape == ResultShape::Original && !decoded && first {
                        let unprovided_account =
                            sponsor.begin_invocation(measurement, 100_000).unwrap();
                        let mut unprovided = program
                            .instantiate(TerminalInstantiation {
                                scalar_arguments: &[TerminalScalarValue::Boolean(first)],
                                structural_inputs: TerminalStructuralInputs {
                                    arguments: &arguments,
                                    contents: &contents,
                                    ..Default::default()
                                },
                                account: Some(&unprovided_account),
                                ..Default::default()
                            })
                            .unwrap();
                        let error = unprovided
                            .invoke(
                                &mut WorkMeter::with_invocation(
                                    unprovided_account.clone(),
                                    Some(100_000),
                                )
                                .unwrap(),
                            )
                            .expect_err("the host must explicitly accept returned loans");
                        assert!(
                            matches!(error, terminal_interpreter::TerminalInstanceError::Execution(terminal_interpreter::TerminalInterpretError::ExecutionSite {cause, ..}) if matches!(*cause, terminal_interpreter::TerminalInterpretError::UnsupportedSemanticVariant("host reference results require an explicit loan carrier interface")))
                        );
                        assert_eq!(
                            unprovided_account.snapshot().unwrap().record().work_units,
                            0
                        );
                        drop(unprovided);
                        println!(
                            "unprovided entry-result loan admission refused before guest work"
                        );
                    }
                    let before_capture = sponsor.live_storage_cells();
                    let pause = instance
                        .invoke(&mut WorkMeter::with_invocation(account.clone(), Some(0)).unwrap())
                        .unwrap();
                    assert!(matches!(
                        pause,
                        TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    let after_capture = sponsor.live_storage_cells();
                    assert!(after_capture > before_capture);
                    let repeated = instance
                        .invoke(&mut WorkMeter::with_invocation(account.clone(), Some(0)).unwrap())
                        .unwrap();
                    assert!(matches!(
                        repeated,
                        TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    assert_eq!(
                        sponsor.live_storage_cells(),
                        after_capture,
                        "a pause must not capture or charge the same return sources twice"
                    );
                    assert!(matches!(
                        instance.retain_returned_loans(),
                        Err(TerminalReturnedLoanError::AdmissionAfterStart)
                    ));
                    assert!(matches!(
                        instance.returned_loan(&path),
                        Err(TerminalReturnedLoanError::NormalReturnRequired)
                    ));
                    let outcome = instance
                        .invoke(
                            &mut WorkMeter::with_invocation(account.clone(), Some(100_000))
                                .unwrap(),
                        )
                        .unwrap();
                    assert!(
                        matches!(
                            &outcome,
                            TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
                                _
                            ))
                        ) || (*shape == ResultShape::Optional
                            && !first
                            && matches!(
                                &outcome,
                                TerminalExecutionStatus::Complete(
                                    TerminalExecutionResult::ScalarCase(_)
                                )
                            ))
                    );

                    if *shape == ResultShape::Optional && !first {
                        assert!(matches!(
                            instance.returned_loan(&path),
                            Err(TerminalReturnedLoanError::UnknownPath)
                        ));
                    } else {
                        let loan = instance.returned_loan(&path).unwrap();
                        if *shape == ResultShape::WriteOnly {
                            assert_eq!(
                                loan.access(),
                                terminal_psi::StructuralAccess::WriteOnlyBorrow
                            );
                            assert!(matches!(
                                loan.read_scalar(&[Path::Field(field_id(
                                    artifact,
                                    loan.structural_type(),
                                    "value"
                                ))]),
                                Err(TerminalReturnedLoanError::UnreadableAccess)
                            ));
                        } else {
                            assert_eq!(
                                loan.access(),
                                terminal_psi::StructuralAccess::MutableBorrow
                            );
                            let expected = if *shape == ResultShape::Original {
                                if first { 5 } else { 9 }
                            } else {
                                41
                            };
                            let selected_left = first || *shape == ResultShape::Optional;
                            for (field_name, expected_value) in if selected_left {
                                [("before", 11), ("value", expected), ("after", 17)]
                            } else {
                                [("before", 23), ("value", expected), ("after", 29)]
                            } {
                                let field = field_id(artifact, loan.structural_type(), field_name);
                                assert!(
                                    matches!(loan.read_scalar(&[Path::Field(field)]).unwrap(),TerminalScalarValue::Integer {value:IntegerValue::Unsigned(observed),..} if observed==expected_value)
                                );
                            }
                            assert!(matches!(
                                loan.read_scalar(&[Path::FixedIndex(99)]),
                                Err(TerminalReturnedLoanError::UnknownPath)
                            ));
                        }
                    }
                    // The host observes the same caller input that the guest
                    // changed, rather than a saved copy of its initial fields.
                    if *shape == ResultShape::Mutated || (*shape == ResultShape::Optional && first)
                    {
                        let selected = if first { 0 } else { 1 };
                        let field =
                            field_id(artifact, arguments[selected].structural_type, "value");
                        assert!(matches!(
                            instance
                                .observe_borrowed_entry(
                                    u32::try_from(selected).unwrap(),
                                    &[Path::Field(field)]
                                )
                                .unwrap(),
                            terminal_interpreter::TerminalStorageObservationValue::Storage(
                                TerminalStorageContents::Scalar {
                                    value: TerminalScalarValue::Integer {
                                        value: IntegerValue::Unsigned(41),
                                        ..
                                    },
                                    ..
                                }
                            )
                        ));
                    }
                    instance.close().unwrap();
                    assert_eq!(
                        sponsor.live_storage_cells(),
                        0,
                        "closing retires all instance-owned return-source metadata and backing"
                    );
                    assert!(matches!(
                        instance.returned_loan(&path),
                        Err(TerminalReturnedLoanError::Closed)
                    ));
                    println!(
                        "entry={name} decoded={decoded} first={first}: live backing, access and quota retention agree"
                    );
                }
            }
        }
    }
}

fn loan_path(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    result_type: semantic_vocabulary::StructuralTypeId,
    shape: ResultShape,
) -> Vec<Path> {
    if shape != ResultShape::Optional {
        return vec![Path::Field(field_id(artifact, result_type, "cell"))];
    }
    let module = artifact.semantic_module();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        panic!("case carrier")
    };
    let case = cases
        .iter()
        .find(|case| module.identities.text(case.identity) == Some("Present"))
        .unwrap();
    let field = &case.fields[0];
    let terminal_psi::StructuralFieldType::Structural(carrier) = field.field_type else {
        panic!("inline carried loan")
    };
    vec![
        Path::Case(case.id),
        Path::Field(field.id),
        Path::Field(field_id(artifact, carrier, "cell")),
    ]
}
fn field_id(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    structural_type: semantic_vocabulary::StructuralTypeId,
    name: &str,
) -> semantic_vocabulary::StructuralFieldId {
    let module = artifact.semantic_module();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .expect("declared carrier");
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("record carrier")
    };
    fields
        .iter()
        .find(|field| module.identities.text(field.identity) == Some(name))
        .expect("declared field")
        .id
}
