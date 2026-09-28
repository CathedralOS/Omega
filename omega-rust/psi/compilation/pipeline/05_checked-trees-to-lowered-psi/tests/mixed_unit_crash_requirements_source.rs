//! Mixed Unit crash arithmetic uses exact runtime requirement evidence.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const SOURCE: &str = r#"
    data Metrics { current: u64; }
    boundary trait Sink { machine record(divisor: u64, limit: u64); }
    data Helper {}
    machine Helper::consume(divisor: u64, metrics: Metrics, limit: u64)
    reaches Sink requires 1u64 <= divisor
    crashes Abort metrics.current / divisor <= limit
    { Sink::record(divisor, limit); }
    data Main {}
    machine Main::main(metrics: Metrics, limit: u64, divisor: u64)
    requires 1u64 <= divisor
    crashes Abort metrics.current / divisor <= limit
    { Helper::consume(divisor, metrics, limit); }
"#;

fn roundtrip(checked: &checked_trees::CheckedTrees) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("mixed crash arithmetic retains its scalar runtime requirements");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification of mixed runtime requirements");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed runtime requirements publish")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        module
    );
    lowered
}

#[test]
fn computed_scalar_divisor_carries_its_requirement_into_the_mixed_unit_call() {
    for computation in [
        "divisor + 0u64",
        "divisor - 0u64",
        "divisor * 1u64",
        "divisor + 1u64",
    ] {
        let source = SOURCE.replace(
            "crashes Abort metrics.current / divisor <= limit\n    { Helper::consume(divisor, metrics, limit); }",
            &format!("crashes Abort\n    {{ Helper::consume({computation}, metrics, limit); }}"),
        );
        let source = if computation == "divisor + 1u64" {
            source.replace(
                "requires 1u64 <= divisor\n    crashes Abort\n",
                "requires 1u64 <= divisor, divisor <= 100u64\n    crashes Abort\n",
            )
        } else {
            source
        };
        let lowered = roundtrip(&crate::front_end::checked_program(&source));
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let (arithmetic, arithmetic_obligation) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                terminal_psi::OperationKind::ExactIntegerAdd { obligation, .. }
                | terminal_psi::OperationKind::ExactIntegerSubtract { obligation, .. }
                | terminal_psi::OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    Some((operation, obligation))
                }
                _ => None,
            })
            .expect("authored operand remains an evaluated Exact operation");
        let terminal_psi::OperationResult::Scalar(result) = arithmetic.result else {
            panic!("arithmetic publishes the argument value");
        };
        let (arguments, requirements) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit {
                    arguments,
                    requirement_obligations,
                    ..
                } => Some((arguments, requirement_obligations)),
                _ => None,
            })
            .unwrap();
        assert_eq!(arguments[0], result.id, "{computation}");
        assert_eq!(requirements.len(), 1);
        assert_ne!(requirements[0], arithmetic_obligation);
        for obligation in [arithmetic_obligation, requirements[0]] {
            assert!(
                lowered
                    .proof_bundle
                    .evidence
                    .iter()
                    .any(|evidence| evidence.obligation == obligation
                        && matches!(
                            evidence.route,
                            proof_admission::EvidenceRoute::CertificateDerived(_)
                        ))
            );
            let mut missing = lowered.proof_bundle.clone();
            missing
                .evidence
                .retain(|evidence| evidence.obligation != obligation);
            assert!(
                matches!(terminal_verifier::verify_module(&lowered.semantic_module, &missing, &proof_admission::AdmissionProfile::default()),
                Err(terminal_verifier::VerificationError::MissingEvidence(rejected)) if rejected == obligation),
                "{computation}: {obligation:?}"
            );
        }
        let mut missing_definition = lowered.semantic_module.clone();
        let changed_root = missing_definition
            .machines
            .iter_mut()
            .find(|machine| machine.id == root.id)
            .unwrap();
        for block in &mut changed_root.blocks {
            block
                .operations
                .retain(|operation| operation.id != arithmetic.id);
        }
        assert!(
            terminal_verifier::validate_module(&missing_definition).is_err(),
            "undefined computed argument: {computation}"
        );
    }
}

#[test]
fn scalar_call_requirements_reject_weaker_bounds_and_disjoint_subjects() {
    for (callee_requirement, caller_requirement, disjoint_subject) in [
        ("2u64 <= limit", "1u64 <= limit", false),
        ("1u64 <= limit", "2u64 <= spare", true),
    ] {
        let source = SOURCE.replacen(
            "requires 1u64 <= divisor",
            &format!("requires 1u64 <= divisor, {callee_requirement}"),
            1,
        );
        let source = source.replace(
            "requires 1u64 <= divisor\n    crashes Abort metrics.current / divisor <= limit\n    { Helper",
            &format!("requires 1u64 <= divisor, {caller_requirement}\n    crashes Abort metrics.current / divisor <= limit\n    {{ Helper"),
        );
        let source = if disjoint_subject {
            source.replace(
                "Main::main(metrics: Metrics, limit: u64, divisor: u64)",
                "Main::main(metrics: Metrics, limit: u64, divisor: u64, spare: u64)",
            )
        } else {
            source
        };
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("caller bounds must imply the requirement on the same scalar subject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("requires")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn structural_divisor_keeps_whole_root_requirements_and_rejects_partial_cleanup() {
    let source = r#"
        data Metrics { divisor: u64; }
        data Envelope { metrics: Metrics; spare: Metrics; }
        data Helper {}
        boundary trait Sink { machine record(numerator: u64, limit: u64); }
        machine Helper::consume(numerator: u64, metrics: Metrics, limit: u64)
        reaches Sink requires 1u64 <= metrics.divisor
        crashes Abort numerator / metrics.divisor <= limit
        { Sink::record(numerator, limit); }
        data Main {}
        machine Main::main(limit: u64, envelope: Envelope, numerator: u64)
        requires 1u64 <= envelope.metrics.divisor
        crashes Abort numerator / envelope.metrics.divisor <= limit
        { Helper::consume(numerator, envelope.metrics, limit); }
    "#;
    for projected in [false, true] {
        let source = if projected {
            source.to_owned()
        } else {
            source
                .replace("envelope: Envelope", "metrics: Metrics")
                .replace("envelope.metrics", "metrics")
        };
        let checked = crate::front_end::checked_program(&source);
        if projected {
            // Projected owned moves need the separate partial-affine cleanup
            // consumer, whose current source shape excludes mixed scalar
            // inputs and an effectful callee. Do not erase the live spare.
            assert!(matches!(
                checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("Main::main")),
                Err(checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
                    machine, reason, ..
                }) if machine == "Main::main"
                    && reason == "attached Unit closure is missing a checked transitive machine plan"
            ));
        } else {
            roundtrip(&checked);
        }
    }
}

const SHIFT_SOURCE: &str = r#"
    data Bits { value: u8; }
    data Helper {}
    boundary trait Sink { machine record(count: i16); }
    machine Helper::consume(count: i16, bits: Bits, limit: u8)
    reaches Sink requires 0i16 <= count, count < 8i16, bits.value <= 1u8
    crashes Abort bits.value << count == limit
    { Sink::record(count); }
    data Main {}
    machine Main::main(bits: Bits, limit: u8, count: i16)
    requires bits.value <= 1u8, count < 8i16, 0i16 <= count
    crashes Abort bits.value << count == limit
    { Helper::consume(count, bits, limit); }
"#;

#[test]
fn exact_shift_and_signed_division_retain_mixed_totality_requirements() {
    roundtrip(&crate::front_end::checked_program(SHIFT_SOURCE));
    roundtrip(&crate::front_end::checked_program(
        &SOURCE.replace("u64", "i64"),
    ));
}

#[test]
fn mixed_arithmetic_rejects_missing_nonzero_signed_overflow_and_count_bounds() {
    let sources = [
        (
            SOURCE.replace("requires 1u64 <= divisor", ""),
            "divisor must be proven nonzero",
        ),
        (
            SOURCE
                .replace("u64", "i64")
                .replace("1i64 <= divisor", "divisor <= -1i64"),
            "MIN / -1",
        ),
        (
            SHIFT_SOURCE.replace("count < 8i16", "count < 9i16"),
            "shift count",
        ),
        (
            SHIFT_SOURCE.replace("0i16 <= count", "-1i16 <= count"),
            "shift count",
        ),
    ];
    for (source, expected) in sources {
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("mixed specification arithmetic still needs complete totality facts");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{source}: {diagnostics:?}"
        );
    }
}
