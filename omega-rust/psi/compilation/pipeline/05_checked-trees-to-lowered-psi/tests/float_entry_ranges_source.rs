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

#[test]
fn finite_value_contracts_transport_entry_result_and_local() {
    for carrier in ["f32", "f64"] {
        for initializer in ["retain(0.5)", "retain(retain(0.5))"] {
            let source = format!(
                r#"
            machine retain(value: {carrier} in Finite) -> {carrier} in Finite {{ value }}
            machine value() -> {carrier} {{
                let result: {carrier} in Finite = {initializer};
                transition {{ _ -> (result) }}
            }}
        "#
            );
            let lowered = lower(&source, "value");
            let module = &lowered.semantic_module;
            assert!(
                module
                    .machines
                    .iter()
                    .any(|machine| !machine.contract.requires.is_empty())
            );
            assert!(
                module
                    .machines
                    .iter()
                    .any(|machine| !machine.contract.ensures.is_empty())
            );
            assert!(
                !module.scalar_block_invariants.is_empty(),
                "local establishment has an arrival proof"
            );
            let module_bytes = encode_module(module).unwrap();
            let proof_bytes = encode_proof_section(module, &lowered.proof_bundle).unwrap();
            assert_eq!(decode_module(&module_bytes).unwrap(), *module);
            let expected = if carrier == "f32" {
                IeeeFloatValue::Binary32(0.5f32.to_bits())
            } else {
                IeeeFloatValue::Binary64(0.5f64.to_bits())
            };
            assert_eq!(
                interpret_terminal_artifact(
                    &module_bytes,
                    &proof_bytes,
                    &proof_admission::AdmissionProfile::default(),
                    &[]
                )
                .unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::IeeeFloat(expected))
            );
        }
    }
}

#[test]
fn finite_value_contracts_reject_nonfinite_call_delivery_after_publication() {
    let source = r#"
        machine retain(value: f32 in Finite) -> f32 in Finite { value }
        machine value() -> f32 { retain(0.5) }
    "#;
    let lowered = lower(source, "value");
    for bits in [
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        f32::NAN.to_bits(),
    ] {
        let mut module = lowered.semantic_module.clone();
        let constant = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::IeeeFloatConstant { value }
                    if *value == IeeeFloatValue::Binary32(0.5f32.to_bits()) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("call actual constant");
        *constant = IeeeFloatValue::Binary32(bits);
        let module_bytes = encode_module(&module).unwrap();
        let proof_bytes = encode_proof_section(&module, &lowered.proof_bundle).unwrap();
        assert!(
            interpret_terminal_artifact(
                &module_bytes,
                &proof_bytes,
                &proof_admission::AdmissionProfile::default(),
                &[]
            )
            .is_err(),
            "nonfinite delivery must not borrow the original evidence"
        );
    }
}

#[test]
fn finite_value_contracts_reject_nonfinite_local_after_publication() {
    let lowered = lower(
        "machine value() -> f32 { let result: f32 in Finite = 0.5; transition { _ -> (result) } }",
        "value",
    );
    assert!(!lowered.semantic_module.scalar_block_invariants.is_empty());
    for bits in [
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        f32::NAN.to_bits(),
    ] {
        let mut module = lowered.semantic_module.clone();
        let constant = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::IeeeFloatConstant { value } => Some(value),
                _ => None,
            })
            .expect("local initializer");
        *constant = IeeeFloatValue::Binary32(bits);
        let module_bytes = encode_module(&module).unwrap();
        let proof_bytes = encode_proof_section(&module, &lowered.proof_bundle).unwrap();
        assert!(
            interpret_terminal_artifact(
                &module_bytes,
                &proof_bytes,
                &proof_admission::AdmissionProfile::default(),
                &[]
            )
            .is_err()
        );
    }
}

#[test]
fn finite_value_contracts_reject_nonfinite_return_after_publication() {
    let lowered = lower("machine value() -> f32 in Finite { 0.5 }", "value");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .any(|machine| !machine.contract.ensures.is_empty())
    );
    for bits in [
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        f32::NAN.to_bits(),
    ] {
        let mut module = lowered.semantic_module.clone();
        let constant = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::IeeeFloatConstant { value } => Some(value),
                _ => None,
            })
            .expect("returned constant");
        *constant = IeeeFloatValue::Binary32(bits);
        let module_bytes = encode_module(&module).unwrap();
        let proof_bytes = encode_proof_section(&module, &lowered.proof_bundle).unwrap();
        assert!(
            interpret_terminal_artifact(
                &module_bytes,
                &proof_bytes,
                &proof_admission::AdmissionProfile::default(),
                &[]
            )
            .is_err()
        );
    }
}

#[test]
fn finite_value_contracts_preserve_predicate_free_result_qualifications() {
    let lowered = lower(
        r#"
        domain i64::Km;
        machine retain(value: i64 in Km) -> i64 in Km { value }
        machine value() -> i64 { retain(5 as i64 in Km) as i64 }
    "#,
        "value",
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("semantic result tags retain their catalog owner");
}
