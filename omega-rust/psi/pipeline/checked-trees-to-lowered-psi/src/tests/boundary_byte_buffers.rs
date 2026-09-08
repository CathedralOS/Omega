//! Source-produced bounded byte fields presented to an external boundary.

use super::*;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralValue,
};

const INPUT_SOURCE: &str = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(out: &mut [u8]) reaches Input; }
        data Record { out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        machine Record::run(&mut self) reaches Input {
            self.out = "old";
            self.other = "QQ";
            Input::read(&mut self.out);
            Input::read(&mut self.out);
        }
        "#;

fn start(source: &str) -> (terminal_psi::TerminalModule, TerminalExecution) {
    let checked = checked_source(source);
    let artifact = produce_terminal_artifact(&checked, "Record::run").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let execution = TerminalExecution::start_artifact_with_structural_arguments(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 73,
            structural_type: entry.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
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
fn boundary_byte_buffer_preserves_initialized_field() {
    let (module, mut execution) = start(INPUT_SOURCE);
    struct Input(usize);
    impl TerminalEffectHandler for Input {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("mutable buffers require the writeback handler")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let [buffer] = buffers else {
                panic!("one mutable argument")
            };
            assert_eq!(buffer.argument_index(), 0);
            assert_eq!(buffer.capacity(), 3);
            assert_eq!(
                buffer.bytes(),
                if self.0 == 0 {
                    b"old".as_slice()
                } else {
                    &[0, 128, 255]
                }
            );
            let TerminalEffect::BoundaryCall {
                structural_arguments,
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("boundary")
            };
            assert_eq!(structural_arguments[0].opaque_identity, 73);
            assert!(!structural_arguments[0].path.is_empty());
            assert_eq!(byte_sequence_arguments[0].as_deref(), Some(buffer.bytes()));
            buffer.replace(if self.0 == 0 { &[0, 128, 255] } else { b"X" })?;
            self.0 += 1;
            Ok(TerminalEffectResult::Unit)
        }
    }
    let mut input = Input(0);
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut complete = false;
    for _ in 0..100 {
        match execution
            .resume_with_effect_handler(&mut fuel, &mut input)
            .unwrap()
        {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
            status => panic!("unexpected {status:?}"),
        }
    }
    assert!(complete);
    assert_eq!(input.0, 2);
    assert_eq!(execution.effects().len(), 2);
    assert_stored_fields(&module, &execution, &[b"X", b"QQ"]);
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
            .resume_with_effect_handler(
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
        assert_stored_fields(&module, &execution, &[b"old", b"QQ"]);
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
            .resume_with_effect_handler(
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
        let result = execution.resume_with_effect_handler(
            &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
            &mut Input { swap },
        );
        if swap {
            assert!(matches!(
                result,
                Err(terminal_interpreter::TerminalInterpretError::VerifiedOperationMalformed)
            ));
            assert_stored_fields(&module, &execution, &[b"old", b"two", b"QQ"]);
            assert!(execution.effects().is_empty());
        } else {
            assert_eq!(
                result.unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_stored_fields(&module, &execution, &[&[255], b"", b"QQ"]);
            assert_eq!(execution.effects().len(), 1);
        }
    }
}

#[test]
fn boundary_byte_buffer_terminal_nested_path_retains_original_owner() {
    // Canonical Terminal execution coverage, not a claim that the source
    // producer admits nested mutable boundary syntax. Wrap the produced record
    // and rederive operation evidence for the new exact store paths.
    let checked = checked_source(INPUT_SOURCE);
    let mut lowered = lower_machine(&checked, "Record::run").unwrap();
    let module = &mut lowered.semantic_module;
    let record = module.machines[0].structural_parameters[0].structural_type;
    let next_type = module
        .structural_types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap()
        + 1;
    let array = structural_type_id(next_type);
    let owner = structural_type_id(next_type + 1);
    module.structural_types.extend([
        terminal_psi::StructuralTypeDeclaration {
            id: array,
            identity: "RecordArray".into(),
            shape: terminal_psi::StructuralTypeShape::FixedArray {
                element: record,
                length: 2,
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: owner,
            identity: "Owner".into(),
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "records".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(array),
                }],
            },
        },
    ]);
    let prefix = vec![
        terminal_psi::StructuralPathSegment::Field("records".into()),
        terminal_psi::StructuralPathSegment::FixedIndex(1),
    ];
    let machine = &mut module.machines[0];
    machine.attachment = Some(owner);
    machine.structural_parameters[0].structural_type = owner;
    for operation in &mut machine.blocks[0].operations {
        match &mut operation.kind {
            OperationKind::StructuralByteSequenceFieldStore { path, .. } => {
                path.splice(0..0, prefix.clone());
            }
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    argument.path.splice(0..0, prefix.clone());
                }
            }
            _ => {}
        }
    }
    lowered.proof_bundle.evidence.clear();
    crate::operation_emission::finalize_operation_proofs(&mut lowered).unwrap();
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 73,
            structural_type: owner,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .unwrap();
    struct Input;
    impl TerminalEffectHandler for Input {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("writeback callback")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                structural_arguments,
                ..
            } = effect
            else {
                panic!("boundary")
            };
            assert_eq!(structural_arguments[0].opaque_identity, 73);
            assert_eq!(structural_arguments[0].path.len(), 3);
            assert_eq!(
                structural_arguments[0].path[1],
                terminal_psi::StructuralPathSegment::FixedIndex(1)
            );
            buffers[0].replace(b"X")?;
            Ok(TerminalEffectResult::Unit)
        }
    }
    assert_eq!(
        execution
            .resume_with_effect_handler(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                &mut Input
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_stored_fields(&lowered.semantic_module, &execution, &[b"X", b"QQ"]);
    let mut other_element = prefix;
    other_element[1] = terminal_psi::StructuralPathSegment::FixedIndex(0);
    for operation in &lowered.semantic_module.machines[0].blocks[0].operations {
        if let OperationKind::StructuralByteSequenceFieldStore { field, .. } = operation.kind {
            assert_eq!(
                execution.structural_byte_sequence_field(73, &other_element, field),
                None
            );
        }
    }
}
