//! Source-produced bounded byte fields presented to an external boundary.
use super::{checked_source, lower_machine};
use terminal_interpreter::TerminalStructuralInputs;
mod checked_provider;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
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
    let checked = checked_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Record::run")
        .produce_artifact()
        .unwrap();
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
fn boundary_byte_buffer_preserves_initialized_field() {
    assert_sequential_input(INPUT_SOURCE, &[b"fin", b"QQ", b"fin"]);
}

#[test]
fn boundary_crash_retains_pre_call_buffers_and_skips_later_effects() {
    let source = INPUT_SOURCE.replace("reaches Input", "reaches Input crashes Abort");
    let (module, mut execution) = start(&source);
    struct CrashingInput(usize);
    impl TerminalEffectHandler for CrashingInput {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("mutable input requires the staged buffer handler")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            _: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            self.0 += 1;
            let [buffer] = buffers else {
                panic!("one input buffer")
            };
            assert_eq!(buffer.bytes(), b"old");
            buffer.replace(b"new")?;
            Ok(TerminalEffectResult::Crash(terminal_psi::CrashCause::Abort))
        }
    }
    let mut handler = CrashingInput(0);
    let mut fuel = terminal_fuel::TerminalFuelMeter::unbounded();
    let status = execution.resume(&mut fuel, &mut handler).unwrap();
    assert!(matches!(&status, TerminalExecutionStatus::Crashed(crash)
        if crash.cause == terminal_psi::CrashCause::Abort));
    assert_stored_fields(&module, &execution, &[b"old", b"QQ", b"old"]);
    assert_eq!(handler.0, 1);
    assert_eq!(execution.effects().len(), 1);
    let units = fuel.usage().total_units();
    assert_eq!(execution.resume(&mut fuel, &mut handler).unwrap(), status);
    assert_eq!(fuel.usage().total_units(), units);
    assert_eq!(handler.0, 1);
}

#[test]
fn boundary_byte_buffer_nested_source_preserves_initialized_field() {
    let source = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(out: &mut [u8]) reaches Input; }
        data Cell { out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        data Record { cells: [Cell; 2]; }
        machine Record::run(&mut self) reaches Input {
            self.cells[1].out = "old";
            self.cells[1].other = "QQ";
            Input::read(&mut self.cells[1].out);
            Input::read(&mut self.cells[1].out);
            // The boundary loans retired every `self.cells` element's `Utf8`
            // coverage; the return re-proves the readable `&mut` referent's
            // declared field facts, so all four are re-established before
            // `self` is handed back.
            self.cells[1].out = "fin";
            self.cells[1].other = "QQ";
            self.cells[0].out = "aa";
            self.cells[0].other = "bb";
        }
    "#;
    assert_sequential_input(source, &[b"fin", b"QQ", b"fin", b"QQ", b"aa", b"bb"]);
    assert_sequential_input(
        &source
            .replace("machine read(out:", "machine read(marker: i32, out:")
            .replace("Input::read(&mut", "Input::read(7, &mut"),
        &[b"fin", b"QQ", b"fin", b"QQ", b"aa", b"bb"],
    );
    assert_sequential_input(
        &source
            .replace("[Cell; 2]", "Cell")
            .replace("cells[1]", "cells")
            .replace("cells[0]", "cells"),
        &[b"aa", b"bb", b"aa", b"bb", b"aa", b"bb"],
    );
    for change_store in [false, true] {
        let mut checked = checked_source(source);
        let operation = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .find(|operation| {
                if change_store {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                    )
                } else {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    )
                }
            })
            .unwrap();
        let path = match operation {
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                &mut store.carrier_path
            }
            CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } => &mut structural_arguments[0].path,
            _ => unreachable!(),
        };
        let array_segment = path
            .iter_mut()
            .find(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))
            .unwrap();
        *array_segment = CheckedUnitStructuralPathSegment::FixedIndex(0);
        assert!(
            lower_machine(&checked, "Record::run").is_err(),
            "a different valid array index does not match the source"
        );
    }
}

fn assert_sequential_input(source: &str, expected_stored: &[&[u8]]) {
    let (module, mut execution) = start(source);
    // The first byte-sequence field store is `out`'s initialization; its
    // (path, field) pair is where every `read` writeback commits. The final
    // `self.out = "fin"` re-establishment overwrites it before return, so the
    // committed boundary bytes are collected at the fuel pauses instead.
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let (out_path, out_field) = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldStore { path, field, .. } => {
                Some((path.clone(), *field))
            }
            _ => None,
        })
        .expect("the borrowed field's authored store");
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
    let mut committed = Vec::new();
    for _ in 0..100 {
        match execution.resume(&mut fuel, &mut input).unwrap() {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => {
                committed.push(
                    execution
                        .structural_byte_sequence_field(73, &out_path, out_field)
                        .map(<[u8]>::to_vec),
                );
                fuel.replenish(1).unwrap();
            }
            status => panic!("unexpected {status:?}"),
        }
    }
    assert!(complete);
    assert_eq!(input.0, 2);
    assert_eq!(execution.effects().len(), 2);
    // Both writebacks committed to the borrowed field before the return's
    // domain repair overwrote them.
    assert!(
        committed
            .iter()
            .any(|bytes| bytes.as_deref() == Some(&[0, 128, 255][..])),
        "the first writeback committed to the field: {committed:?}"
    );
    assert!(
        committed
            .iter()
            .any(|bytes| bytes.as_deref() == Some(b"X".as_slice())),
        "the second writeback committed to the field: {committed:?}"
    );
    assert_stored_fields(&module, &execution, expected_stored);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        if let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } = &operation.kind
        {
            let mut sibling = path.clone();
            if let Some(array_segment) = sibling.iter_mut().find(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            }) {
                // The boundary writebacks committed only to the borrowed
                // element; its sibling element keeps its own literals.
                let terminal_psi::StructuralPathSegment::FixedIndex(index) = *array_segment else {
                    unreachable!()
                };
                *array_segment = terminal_psi::StructuralPathSegment::FixedIndex(1 - index);
                let sibling_bytes = execution.structural_byte_sequence_field(73, &sibling, *field);
                assert_ne!(sibling_bytes, Some(&[0, 128, 255][..]));
                assert_ne!(sibling_bytes, Some(b"X".as_slice()));
            }
        }
    }
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
