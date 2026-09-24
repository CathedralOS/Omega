//! Authored floating entry ranges keep their exact IEEE endpoints through
//! source lowering: the exclusive maximum stays authored, the retained rows
//! reach the published catalog, calls deliver only provable members, and an
//! independently replayed artifact rejects an endpoint delivery.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, ScalarType};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_psi::OperationKind;

fn lower(source: &str, entry: &str) -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:?}"))
}

#[test]
fn exclusive_range_rows_reach_the_terminal_catalog_and_execute() {
    let source = r#"
        machine wide(value: f64[0.0..=1.5]) -> f64 { value }
        machine accept(value: f64[0.0..1.5]) -> f64 { wide(value) }
        machine value() -> f64 { accept(1.0) }
    "#;
    let lowered = lower(source, "value");
    let ranges = &lowered
        .semantic_module
        .scalar_qualifications
        .float_entry_ranges;
    assert_eq!(ranges.len(), 2, "{source}");
    // Canonical (machine, parameter) order; each row names the ranged entry
    // parameter of a different emitted machine.
    assert!(ranges[0].machine < ranges[1].machine);
    let exclusive = ranges
        .iter()
        .find(|range| !range.maximum_inclusive)
        .expect("accept retains its exclusive endpoint");
    let inclusive = ranges
        .iter()
        .find(|range| range.maximum_inclusive)
        .expect("wide retains its inclusive endpoint");
    for range in ranges {
        assert_eq!(range.minimum, IeeeFloatValue::Binary64(0.0f64.to_bits()));
        assert_eq!(range.maximum, IeeeFloatValue::Binary64(1.5f64.to_bits()));
        assert_eq!(range.format(), IeeeFloatFormat::Binary64);
        let owner = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == range.machine)
            .expect("range owner");
        let parameter = owner
            .parameters
            .iter()
            .find(|parameter| parameter.id == range.parameter)
            .expect("ranged parameter");
        assert_eq!(
            parameter.scalar_type,
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
        );
        // The requires tail publishes no proposition for the range.
        assert!(owner.contract.requires.is_empty());
    }
    // The exclusive row rides on accept, whose body calls wide — the
    // inclusive row's machine — so strictness travels with the caller.
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == exclusive.machine)
        .expect("accept machine");
    assert!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                &operation.kind,
                OperationKind::Call { callee, .. } if *callee == inclusive.machine
            )),
        "accept must call the machine retaining the inclusive row: {source}"
    );
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let decoded = decode_module(&module_bytes).expect("canonical module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            1.0f64.to_bits()
        ))),
        "{source}",
    );
}

#[test]
fn inclusive_endpoint_admits_the_endpoint_itself() {
    let source = r#"
        machine accept(value: f64[0.0..=1.5]) -> f64 { value }
        machine value() -> f64 { accept(1.5) }
    "#;
    let lowered = lower(source, "value");
    let [range] = lowered
        .semantic_module
        .scalar_qualifications
        .float_entry_ranges
        .as_slice()
    else {
        panic!("one retained range: {source}")
    };
    assert!(range.maximum_inclusive);
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            1.5f64.to_bits()
        ))),
        "{source}",
    );
}

#[test]
fn f32_range_rows_retain_binary32_endpoints() {
    let source = r#"
        machine accept(value: f32[0.5..2.25]) -> f32 { value }
        machine value() -> f32 { accept(1.0) }
    "#;
    let lowered = lower(source, "value");
    let [range] = lowered
        .semantic_module
        .scalar_qualifications
        .float_entry_ranges
        .as_slice()
    else {
        panic!("one retained range: {source}")
    };
    assert_eq!(range.minimum, IeeeFloatValue::Binary32(0.5f32.to_bits()));
    assert_eq!(range.maximum, IeeeFloatValue::Binary32(2.25f32.to_bits()));
    assert!(!range.maximum_inclusive);
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(
            1.0f32.to_bits()
        ))),
        "{source}",
    );
}

#[test]
fn exclusive_endpoint_delivery_is_rejected() {
    for endpoint in ["1.5", "2.0"] {
        let source = format!(
            r#"
            machine accept(value: f64[0.0..1.5]) -> f64 {{ value }}
            machine value() -> f64 {{ accept({endpoint}) }}
            "#,
        );
        assert!(
            matches!(
                checked_trees_to_lowered_psi::lower_machine(
                    &crate::front_end::checked_program(&source),
                    TerminalMachineSelection::Name("value")
                ),
                Err(
                    checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                        terminal_verifier::ModuleError::ScalarFloatRangeDelivery { .. }
                    )
                )
            ),
            "{endpoint} is outside f64[0.0..1.5): {source}"
        );
    }
}

#[test]
fn unproven_deliveries_into_a_range_are_rejected() {
    // An unranged caller parameter proves nothing: only constants inside
    // the window or caller parameters subsumed by it may deliver.
    let source = r#"
        machine accept(value: f64[0.0..1.5]) -> f64 { value }
        machine pass(value: f64) -> f64 { accept(value) }
        machine value() -> f64 { pass(1.0) }
    "#;
    let outcome = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("value"),
    );
    assert!(
        matches!(
            outcome,
            Err(
                checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                    terminal_verifier::ModuleError::ScalarFloatRangeDelivery { .. }
                )
            )
        ),
        "unproven delivery must fail closed: {source}\noutcome: {}",
        match &outcome {
            Ok(_) => "lowered cleanly".to_string(),
            Err(error) => format!("{error:?}"),
        }
    );
}

#[test]
fn shared_catalog_range_rows_survive_attached_assembly() {
    // A scalar machine carrying ordinary Unit operations routes through the
    // shared attached-Unit assembler; the ranged helper's retained row must
    // merge into the root catalog with its exact exclusive endpoint, and the
    // published artifact must decode and verify standalone.
    let source = r#"
        machine touch(destination: &mut f64) { destination = 2.0; }
        machine accept(value: f64[0.0..1.5]) -> f64 { value }
        machine caller(seed: f64) -> f64 {
            let mut scratch: f64 = seed;
            touch(&mut scratch);
            accept(1.0)
        }
        machine value() -> f64 { caller(0.5) }
    "#;
    let lowered = lower(source, "value");
    let ranges = &lowered
        .semantic_module
        .scalar_qualifications
        .float_entry_ranges;
    let [range] = ranges.as_slice() else {
        panic!("accept's row must reach the shared catalog: {source}")
    };
    assert_eq!(range.minimum, IeeeFloatValue::Binary64(0.0f64.to_bits()));
    assert_eq!(range.maximum, IeeeFloatValue::Binary64(1.5f64.to_bits()));
    assert!(!range.maximum_inclusive);
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let decoded = decode_module(&module_bytes).expect("canonical module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            1.0f64.to_bits()
        ))),
        "{source}",
    );
}

#[test]
fn shared_catalog_unproven_delivery_is_rejected() {
    // `scratch` is rewritten by an ordinary Unit call: its read carries no
    // retained range, so the delivery into accept's exclusive window fails
    // closed inside the shared assembler's merged catalog.
    let source = r#"
        machine touch(destination: &mut f64) { destination = 2.0; }
        machine accept(value: f64[0.0..1.5]) -> f64 { value }
        machine caller(seed: f64) -> f64 {
            let mut scratch: f64 = seed;
            touch(&mut scratch);
            accept(scratch)
        }
        machine value() -> f64 { caller(0.5) }
    "#;
    let outcome = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("value"),
    );
    assert!(
        matches!(
            outcome,
            Err(
                checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                    terminal_verifier::ModuleError::ScalarFloatRangeDelivery { .. }
                )
            )
        ),
        "unproven delivery must fail closed: {source}\noutcome: {}",
        match &outcome {
            Ok(_) => "lowered cleanly".to_string(),
            Err(error) => format!("{error:?}"),
        }
    );
}

#[test]
fn narrower_ranged_parameter_delivers_into_a_wider_range() {
    // accept's caller parameter retains [0.0, 1.5) which is subsumed by
    // wide's inclusive [0.0, 1.5]: the forwarding call discharges delivery
    // without a constant.
    let source = r#"
        machine wide(value: f64[0.0..=1.5]) -> f64 { value }
        machine accept(value: f64[0.0..1.5]) -> f64 { wide(value) }
        machine value() -> f64 { accept(1.0) }
    "#;
    let lowered = lower(source, "value");
    let module_bytes = encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    assert_eq!(
        interpret_terminal_artifact(
            &module_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap_or_else(|error| panic!("{source}: {error:?}")),
        TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            1.0f64.to_bits()
        ))),
        "{source}",
    );
}

#[test]
fn wider_ranged_parameter_cannot_deliver_into_a_narrower_range() {
    // pass admits up to and including 1.5, but accept's exclusive endpoint
    // cannot prove that delivery.
    let source = r#"
        machine accept(value: f64[0.0..1.5]) -> f64 { value }
        machine pass(value: f64[0.0..=1.5]) -> f64 { accept(value) }
        machine value() -> f64 { pass(1.0) }
    "#;
    assert!(
        matches!(
            checked_trees_to_lowered_psi::lower_machine(
                &crate::front_end::checked_program(source),
                TerminalMachineSelection::Name("value")
            ),
            Err(
                checked_trees_to_lowered_psi::LoweringError::InvalidTerminalModule(
                    terminal_verifier::ModuleError::ScalarFloatRangeDelivery { .. }
                )
            )
        ),
        "{source}"
    );
}
