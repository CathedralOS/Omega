//! A fresh guard proves each indexed write without a termination claim.

use super::*;
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralByteArrayValue, TerminalStructuralValue,
};

const FILL: &str = r#"
machine fill(out: &mut [u8], byte: u8) {
    transition { _ -> scan(out, 0, byte) }
    state scan(out: &mut [u8], position: u64, byte: u8) {
        transition position < out.len {
            true -> store(out, position, byte)
            false -> done()
        }
    }
    state store(out: &mut [u8], position: u64, byte: u8) {
        out[position] = byte;
        transition { _ -> scan(out, position + 1, byte) }
    }
    state done() {}
}
"#;

#[test]
fn byte_write_loop_publishes_fresh_guarded_writes() {
    let checked = checked_source(FILL);
    let _artifact = produce_terminal_artifact(&checked, "fill")
        .expect("a fresh guard proves every write and cursor advance");
}

fn entry_argument(artifact: &terminal_codec::CanonicalTerminalArtifact) -> TerminalStructuralValue {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }
}

#[test]
fn byte_write_loop_fills_each_raw_prefix_once_across_fuel_suspension() {
    for initial in [vec![0x11], vec![0x11, 0x80, 0xff]] {
        for byte in [0, 65, 165] {
            let length = initial.len();
            let checked = checked_source(&format!(
                "{FILL}\ndata Record {{ out: [u8; {length}]; other: [u8; {length}]; }}\n\
                 machine Record::run(&mut self) {{ fill(&mut self.out, {byte}); }}"
            ));
            let artifact = produce_terminal_artifact(&checked, "Record::run")
                .expect("a raw array caller reaches the safety-checked fill loop");
            let path = vec![StructuralPathSegment::Field("out".into())];
            let sibling_path = vec![StructuralPathSegment::Field("other".into())];
            let sibling = vec![0x42; length];
            let arrays = [
                TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: path.clone(),
                    bytes: initial.clone(),
                },
                TerminalStructuralByteArrayValue {
                    argument_index: 0,
                    path: sibling_path.clone(),
                    bytes: sibling.clone(),
                },
            ];
            let start = || {
                TerminalExecution::start_artifact_with_structural_arguments_and_byte_arrays(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &proof_admission::AdmissionProfile::default(),
                    &[],
                    &[entry_argument(&artifact)],
                    &arrays,
                )
                .unwrap()
            };
            let mut execution = start();
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            let mut prefixes = vec![initial.clone()];
            let mut completed = false;
            for _ in 0..512 {
                let status = execution.resume(&mut fuel).unwrap();
                let observed = execution.structural_byte_array(73, &path).unwrap().to_vec();
                assert_eq!(observed.len(), length);
                assert_eq!(
                    execution.structural_byte_array(73, &sibling_path),
                    Some(sibling.as_slice())
                );
                assert!(execution.effects().is_empty());
                if prefixes.last() != Some(&observed) {
                    prefixes.push(observed.clone());
                }
                match status {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        let before = fuel.clone();
                        for _ in 0..2 {
                            assert!(matches!(
                                execution.resume(&mut fuel).unwrap(),
                                TerminalExecutionStatus::SponsorExhausted(_)
                            ));
                            assert_eq!(fuel, before, "unfunded resumption charges no work");
                            assert_eq!(
                                execution.structural_byte_array(73, &path),
                                Some(observed.as_slice())
                            );
                            assert_eq!(
                                execution.structural_byte_array(73, &sibling_path),
                                Some(sibling.as_slice())
                            );
                            assert!(execution.effects().is_empty());
                        }
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(result, TerminalExecutionResult::Unit);
                        completed = true;
                        break;
                    }
                    TerminalExecutionStatus::Crashed(crash) => {
                        panic!("unexpected crash: {crash:?}")
                    }
                }
            }
            assert!(
                completed,
                "finite fixture must finish; this is not a published termination certificate"
            );
            let expected = (0..=length)
                .map(|written| {
                    let mut prefix = initial.clone();
                    prefix[..written].fill(byte);
                    prefix
                })
                .collect::<Vec<_>>();
            assert_eq!(
                prefixes, expected,
                "each write advances exactly one prefix byte"
            );
            let mut generous = start();
            let mut generous_fuel = TerminalFuelMeter::with_allowance(10_000);
            assert_eq!(
                generous.resume(&mut generous_fuel).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                generous.structural_byte_array(73, &path),
                execution.structural_byte_array(73, &path)
            );
            assert_eq!(
                generous.structural_byte_array(73, &sibling_path),
                Some(sibling.as_slice())
            );
            assert!(generous.effects().is_empty());
            assert_eq!(
                fuel.usage(),
                generous_fuel.usage(),
                "chunking preserves every semantic site's work"
            );
        }
    }
}

#[test]
fn byte_write_loop_empty_initialized_view_never_writes() {
    // Existing UTF-8 literal initialization supplies a genuinely empty live
    // field view. This is not a zero-length fixed-array declaration.
    let checked = checked_source(&format!(
        r#"
        {FILL}
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        data Record {{ out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }}
        machine Record::run(&mut self) {{
            self.out = "";
            self.other = "QQ";
            fill(&mut self.out, 165);
        }}
    "#
    ));
    let artifact =
        produce_terminal_artifact(&checked, "Record::run").expect("empty initialized view caller");
    let argument = entry_argument(&artifact);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == argument.structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("record caller");
    };
    let [out, other] = ["out", "other"].map(|identity| {
        fields
            .iter()
            .find(|field| field.identity == identity)
            .unwrap()
            .id
    });
    let writes = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::ByteSequenceWrite { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert!(
        !writes.is_empty(),
        "the untaken write must remain in the verified artifact"
    );
    let start = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            std::slice::from_ref(&argument),
        )
        .unwrap()
    };
    let mut execution = start();
    let mut fuel = TerminalFuelMeter::with_allowance(0);
    let mut completed = false;
    for _ in 0..128 {
        match execution.resume(&mut fuel).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let before = fuel.clone();
                let storage = [out, other].map(|field| {
                    execution
                        .structural_byte_sequence_field(73, &[], field)
                        .map(<[u8]>::to_vec)
                });
                assert!(matches!(
                    execution.resume(&mut fuel).unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(fuel, before);
                assert_eq!(
                    [out, other].map(|field| execution
                        .structural_byte_sequence_field(73, &[], field)
                        .map(<[u8]>::to_vec)),
                    storage
                );
                assert!(execution.effects().is_empty());
                fuel.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash: {crash:?}"),
        }
    }
    assert!(completed);
    assert_eq!(
        execution.structural_byte_sequence_field(73, &[], out),
        Some(&[][..])
    );
    assert_eq!(
        execution.structural_byte_sequence_field(73, &[], other),
        Some(b"QQ".as_slice())
    );
    assert!(execution.effects().is_empty());
    assert!(writes.iter().all(|operation| {
        fuel.usage()
            .at(FuelChargeSite::Operation(*operation))
            .is_none()
    }));
    let mut generous = start();
    let mut generous_fuel = TerminalFuelMeter::with_allowance(10_000);
    assert_eq!(
        generous.resume(&mut generous_fuel).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        generous.structural_byte_sequence_field(73, &[], out),
        Some(&[][..])
    );
    assert_eq!(
        generous.structural_byte_sequence_field(73, &[], other),
        Some(b"QQ".as_slice())
    );
    assert!(generous.effects().is_empty());
    assert_eq!(fuel.usage(), generous_fuel.usage());
}
