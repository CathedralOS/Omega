//! Source-produced bounded byte fields presented to an external boundary.
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
mod checked_provider;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralValue,
};
use terminal_psi::OperationKind;

const INPUT_SOURCE: &str = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(out: &mut [u8]) reaches Input; }
        data Record { out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        machine Record::run(&mut self) reaches Input {
            self.out = "old";
            self.other = "QQ";
            Input::read(&mut self.out);
            Input::read(&mut self.out);
            // The boundary loans retired `self.out`'s `Utf8` coverage and a
            // machine's return re-proves its readable `&mut` referents'
            // declared field facts, so the field is re-established before
            // `self` is handed back. The committed boundary bytes are
            // observed mid-execution at the fuel pauses.
            self.out = "fin";
        }
        "#;

fn start(source: &str) -> (terminal_psi::TerminalModule, TerminalExecution) {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .unwrap();
    (module, execution)
}

fn assert_stored_fields(
    module: &terminal_psi::TerminalModule,
    execution: &TerminalExecution,
    expected: &[&[u8]],
) {
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let actual = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } =
                &operation.kind
            {
                Some(
                    execution
                        .structural_byte_sequence_field(73, path, *field)
                        .unwrap(),
                )
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn boundary_byte_buffer_failed_responses_do_not_commit_bytes_or_effects() {
    #[derive(Clone, Copy)]
    enum Failure {
        Host,
        Result,
        Capacity,
    }
    struct Reject(Failure);
    impl TerminalEffectHandler for Reject {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("writeback callback")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            _: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            buffers[0].replace(b"X")?;
            match self.0 {
                Failure::Host => Err(TerminalEffectRejection::new("host rejected")),
                Failure::Result => Ok(TerminalEffectResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Boolean(true),
                )),
                Failure::Capacity => {
                    let error = buffers[0].replace(b"long").unwrap_err();
                    assert_eq!(
                        buffers[0].bytes(),
                        b"X",
                        "oversize must not change staged bytes"
                    );
                    Err(error)
                }
            }
        }
    }
    for failure in [Failure::Host, Failure::Result, Failure::Capacity] {
        let (module, mut execution) = start(INPUT_SOURCE);
        let error = execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                &mut Reject(failure),
            )
            .unwrap_err();
        match failure {
            Failure::Result => assert!(matches!(
                error,
                terminal_interpreter::TerminalInterpretError::VerifiedOperationMalformed
            )),
            _ => assert!(matches!(
                error,
                terminal_interpreter::TerminalInterpretError::EffectRejected { .. }
            )),
        }
        assert!(execution.effects().is_empty());
        assert_stored_fields(&module, &execution, &[b"old", b"QQ", b"old"]);
    }
}

#[test]
fn boundary_byte_buffer_default_handler_and_uninitialized_backing_reject_before_effect() {
    struct Observe(usize);
    impl TerminalEffectHandler for Observe {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            self.0 += 1;
            Ok(())
        }
    }
    for initialized in [true, false] {
        let source = if initialized {
            INPUT_SOURCE.to_owned()
        } else {
            INPUT_SOURCE.replace("self.out = \"old\";", "")
        };
        let (_, mut execution) = start(&source);
        let mut observer = Observe(0);
        let error = execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                &mut observer,
            )
            .unwrap_err();
        if initialized {
            assert!(matches!(
                error,
                terminal_interpreter::TerminalInterpretError::EffectRejected { .. }
            ));
        } else {
            assert!(matches!(
                error,
                terminal_interpreter::TerminalInterpretError::VerifiedOperationMalformed
            ));
        }
        assert_eq!(observer.0, 0);
        assert!(execution.effects().is_empty());
    }
}

#[test]
fn boundary_byte_buffers_keep_field_destinations_separate() {
    let source = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(first: &mut [u8], second: &mut [u8]) reaches Input; }
        data Record { first: [u8; 3] in Utf8; second: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        machine Record::run(&mut self) reaches Input {
            self.first = "old";
            self.second = "two";
            self.other = "QQ";
            Input::read(&mut self.second, &mut self.first);
            // The boundary loan retired both borrowed fields' `Utf8`
            // coverage; the return re-proves the readable `&mut` referent's
            // declared field facts, so both are re-established before `self`
            // is handed back. The committed writebacks are observed
            // mid-execution at the fuel pauses.
            self.first = "one";
            self.second = "two";
        }
    "#;
    struct Input {
        swap: bool,
    }
    impl TerminalEffectHandler for Input {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("writeback callback")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            assert_eq!(buffers.len(), 2);
            assert_eq!(buffers[0].bytes(), b"two");
            assert_eq!(buffers[1].bytes(), b"old");
            let TerminalEffect::BoundaryCall {
                structural_arguments,
                ..
            } = effect
            else {
                panic!("boundary")
            };
            assert_ne!(structural_arguments[0].path, structural_arguments[1].path);
            buffers[0].replace(b"")?;
            buffers[1].replace(&[255])?;
            if self.swap {
                buffers.swap(0, 1);
            }
            Ok(TerminalEffectResult::Unit)
        }
    }
    for swap in [false, true] {
        let (module, mut execution) = start(source);
        let mut input = Input { swap };
        if swap {
            let result = execution.resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                &mut input,
            );
            assert!(matches!(
                result,
                Err(terminal_interpreter::TerminalInterpretError::VerifiedOperationMalformed)
            ));
            assert_stored_fields(
                &module,
                &execution,
                &[b"old", b"two", b"QQ", b"old", b"two"],
            );
            assert!(execution.effects().is_empty());
            continue;
        }
        // Incremental fuel lets each pause observe the staged writeback once
        // it commits to its own field, before the return's domain repair
        // overwrites both borrowed fields.
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == entry.structural_parameters[0].structural_type)
            .unwrap();
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            panic!("caller record")
        };
        let [first, second] = ["first", "second"].map(|identity| {
            fields
                .iter()
                .find(|field| field.identity == identity)
                .unwrap()
                .id
        });
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
        let mut committed = Vec::new();
        let mut complete = false;
        for _ in 0..100 {
            match execution.resume(&mut fuel, &mut input).unwrap() {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                    complete = true;
                    break;
                }
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    committed.push([first, second].map(|field| {
                        execution
                            .structural_byte_sequence_field(73, &[], field)
                            .map(<[u8]>::to_vec)
                    }));
                    fuel.replenish(1).unwrap();
                }
                status => panic!("unexpected {status:?}"),
            }
        }
        assert!(complete);
        assert!(
            committed
                .iter()
                .any(|pair| pair[0].as_deref() == Some(&[255][..])
                    && pair[1].as_deref() == Some(b"".as_slice())),
            "each writeback committed to its own field: {committed:?}"
        );
        assert_stored_fields(
            &module,
            &execution,
            &[b"one", b"two", b"QQ", b"one", b"two"],
        );
        assert_eq!(execution.effects().len(), 1);
    }
}
