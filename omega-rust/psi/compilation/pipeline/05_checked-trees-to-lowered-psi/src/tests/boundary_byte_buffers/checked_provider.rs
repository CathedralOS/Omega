//! Authored checked providers preserve borrowed destination custody across calls.
use super::{
    INPUT_SOURCE, OperationKind, TerminalBoundaryByteBuffer, TerminalEffect, TerminalEffectHandler,
    TerminalEffectRejection, TerminalEffectResult, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralValue, assert_stored_fields,
};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    ProviderInstallationSelection, admit_provider_installation_from_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn checked_provider_byte_buffers_forward_original_array_path_across_fuel_suspension() {
    let source = INPUT_SOURCE
        .replace("data Record {", "data Cell {")
        .replace(
            "machine Record::run",
            "data Record { cells: [Cell; 2]; } machine Record::run",
        )
        .replace("self.out", "self.cells[1].out")
        .replace("self.other", "self.cells[1].other")
        // The boundary loans retired every `self.cells` element's `Utf8`
        // coverage, so all four leaf fields are re-established before the
        // `&mut self` return re-proves them.
        .replace(
            "self.cells[1].out = \"fin\";",
            "self.cells[1].out = \"fin\";\n            self.cells[1].other = \"QQ\";\n            self.cells[0].out = \"aa\";\n            self.cells[0].other = \"bb\";",
        );
    assert_forwarded_input(
        &source,
        false,
        &[b"fin", b"QQ", b"fin", b"QQ", b"aa", b"bb"],
    );
}

#[test]
fn checked_provider_byte_buffers_forward_through_ordinary_helper() {
    assert_forwarded_input(INPUT_SOURCE, true, &[b"fin", b"QQ", b"fin"]);
}

fn assert_forwarded_input(source: &str, ordinary_helper: bool, expected_stored: &[&[u8]]) {
    let mut source = source.replace(
        "boundary trait Input { machine read(out: &mut [u8]) reaches Input; }",
        "boundary trait Input { machine read(out: &mut [u8]) reaches Input invokes Input; machine refill(out: &mut [u8]) reaches Input; }",
    ).replace(
        "machine Record::run",
        r#"
        data Provider {}
        machine Provider::read(out: &mut [u8]) satisfies Input::read reaches Input {
            Input::refill(&mut out);
        }
        machine Record::run"#,
    );
    if ordinary_helper {
        source = source.replace("Input::refill(&mut out);", "forward(&mut out);");
        source = source.replace(
            "satisfies Input::read reaches Input {",
            "satisfies Input::read {",
        );
        source.push_str(
            r#"
            machine forward(out: &mut [u8]) reaches Input {
                Input::refill(&mut out);
            }
        "#,
        );
    }
    let checked = crate::front_end::checked_program(&source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("authored forwarding provider produces verified Terminal")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one authored provider candidate")
    };
    let profile = proof_admission::AdmissionProfile::default();
    let installation = admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("exact source-produced provider installation");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let expected_path = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => Some(structural_arguments[0].path.clone()),
            _ => None,
        })
        .expect("authored outer destination path");
    struct Input {
        calls: usize,
        outer: semantic_vocabulary::BoundaryMachineId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    }
    impl TerminalEffectHandler for Input {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            panic!("forwarded buffers require writeback callback")
        }
        fn handle_effect_with_byte_buffers(
            &mut self,
            effect: &TerminalEffect,
            buffers: &mut [TerminalBoundaryByteBuffer],
        ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                boundary,
                structural_arguments,
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("inner boundary effect")
            };
            assert_ne!(
                *boundary, self.outer,
                "installed outer call must execute its body"
            );
            let [buffer] = buffers else {
                panic!("one forwarded byte destination")
            };
            assert_eq!(buffer.capacity(), 3);
            assert_eq!(buffer.argument_index(), 0);
            assert_eq!(
                buffer.bytes(),
                if self.calls == 0 {
                    b"old".as_slice()
                } else {
                    &[0, 128, 255]
                }
            );
            assert_eq!(structural_arguments[0].opaque_identity, 73);
            assert_eq!(structural_arguments[0].path, self.path);
            assert_eq!(byte_sequence_arguments[0].as_deref(), Some(buffer.bytes()));
            buffer.replace(if self.calls == 0 {
                &[0, 128, 255]
            } else {
                b"X"
            })?;
            self.calls += 1;
            Ok(TerminalEffectResult::Unit)
        }
    }
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
    let mut usages = Vec::new();
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_installed_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&argument),
                ..Default::default()
            },
            &installation,
        )
        .unwrap();
        let mut input = Input {
            calls: 0,
            outer: candidate.boundary,
            path: expected_path.clone(),
        };
        let mut fuel =
            terminal_fuel::TerminalFuelMeter::with_allowance(if incremental { 0 } else { 100 });
        let mut complete = false;
        let mut committed = Vec::new();
        for _ in 0..100 {
            match execution
                .resume(&mut fuel, &mut input)
                .expect("checked provider forwards mutable byte destination")
            {
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
        assert_eq!(input.calls, 2);
        assert_eq!(execution.effects().len(), 2);
        if incremental {
            // The forwarded writebacks committed to the borrowed field before
            // the return's domain repair overwrote them.
            assert!(
                committed
                    .iter()
                    .any(|bytes| bytes.as_deref() == Some(&[0, 128, 255][..])),
                "the first forwarded writeback committed: {committed:?}"
            );
            assert!(
                committed
                    .iter()
                    .any(|bytes| bytes.as_deref() == Some(b"X".as_slice())),
                "the second forwarded writeback committed: {committed:?}"
            );
        }
        assert_stored_fields(&module, &execution, expected_stored);
        for operation in entry.blocks.iter().flat_map(|block| &block.operations) {
            if let OperationKind::StructuralByteSequenceFieldStore { path, field, .. } =
                &operation.kind
            {
                let mut sibling = path.clone();
                if let Some(segment) = sibling.iter_mut().find(|segment| {
                    matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
                }) {
                    // The forwarded writebacks committed only to the borrowed
                    // element; its sibling element keeps its own literals.
                    let terminal_psi::StructuralPathSegment::FixedIndex(index) = *segment else {
                        unreachable!()
                    };
                    *segment = terminal_psi::StructuralPathSegment::FixedIndex(1 - index);
                    let bytes = execution.structural_byte_sequence_field(73, &sibling, *field);
                    assert_ne!(
                        bytes,
                        Some(&[0, 128, 255][..]),
                        "forwarding cannot write a different array element"
                    );
                    assert_ne!(
                        bytes,
                        Some(b"X".as_slice()),
                        "forwarding cannot write a different array element"
                    );
                }
            }
        }
        usages.push(fuel.into_usage());
    }
    assert_eq!(
        usages[0], usages[1],
        "suspension cannot repeat charged work"
    );
}
