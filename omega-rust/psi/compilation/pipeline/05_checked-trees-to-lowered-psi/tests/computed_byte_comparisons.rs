//! Content observations retain selective descriptor preparation and exact source custody.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const SOURCE: &str = r#"
    boundary trait Output { machine flag(value: bool) reaches Output; }
    data Helper {}
    machine Helper::inspect(bytes: &[u8], other: &[u8], count: u64) reaches Output {
        Output::flag(count <= bytes.len && bytes[0..count] == "look");
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::inspect("look\n", "quit\n", 4);
        Helper::inspect("look", "quit", 4);
        Helper::inspect("quit", "look", 4);
        Helper::inspect("", "look", 4);
        Helper::inspect("", "look", 0);
    }
"#;

fn lower(
    checked: &checked_trees::CheckedTrees,
) -> Result<lowered_psi::LoweredPsi, checked_trees_to_lowered_psi::LoweringError> {
    checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
}

#[test]
fn computed_byte_comparisons_evaluate_only_after_the_bound_guard() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower(&checked).expect("computed view comparison lowers");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("source-free comparison replay and execution");
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    let flags: Vec<_> = result
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("flag effect");
            };
            let [TerminalScalarValue::Boolean(value)] = arguments.as_slice() else {
                panic!("Boolean flag");
            };
            *value
        })
        .collect();
    assert_eq!(flags, [true, true, false, false, false]);
}

#[test]
fn computed_byte_comparisons_reject_a_same_typed_substituted_source() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let mut altered = 0;
    let nodes: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    for handle in nodes {
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle);
        let checked_trees::CheckedScalarComputationKind::ByteSequenceLiteralComparison {
            subject,
            ..
        } = &mut node.kind
        else {
            continue;
        };
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            ..
        } = &mut subject.source
        else {
            continue;
        };
        assert_eq!(
            *root,
            checked_trees::CheckedStorageRoot::Parameter { index: 0 }
        );
        *root = checked_trees::CheckedStorageRoot::Parameter { index: 1 };
        altered += 1;
    }
    assert!(altered > 0, "the computed operand was retained");
    let error = lower(&checked).expect_err("equal carrier shape cannot substitute another operand");
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
        ),
        "source correspondence must reject the substitution before Terminal proof: {error:?}"
    );
}

#[test]
fn computed_byte_comparisons_preserve_whole_view_comparison() {
    let source = SOURCE.replace("count <= bytes.len && bytes[0..count]", "bytes");
    let checked = crate::front_end::checked_program(&source);
    lower(&checked).expect("whole views share the content query without descriptor preparation");
}

#[test]
fn computed_byte_comparisons_retain_unannotated_fixed_array_views() {
    let checked = crate::front_end::checked_program(
        r#"
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Root {}
        machine Root::enter() reaches Output {
            let bytes: [u8; 4] = [108, 111, 111, 107];
            Output::flag(bytes[0..4] == "look");
        }
    "#,
    );
    let lowered =
        lower(&checked).expect("a computed descriptor needs no unrelated view annotation");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("fixed-array view executes through source-free replay");
    assert!(
        matches!(result.effects(), [TerminalEffect::BoundaryCall { arguments, .. }]
        if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    );
}
