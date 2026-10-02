//! Primitive byte facts follow the exact subslice producer into ordinary calls.

use super::{LoweredPsi, OperationKind, OperationResult, TerminalMachineSelection, lower_machine};
use terminal_psi::{ByteSequencePredicate, TerminalMachine};

const SOURCE: &str = r#"
    domain [u8]::Path(bytes) requires no_nul(bytes);
    boundary trait Output { machine write(value: u64) reaches Output; }
    data Root { end: u64; }
    machine Root::inspect(root: &Root, bytes: &[u8] in Path) reaches Output {
        Output::write(bytes.len);
    }
    machine Root::relay(root: &Root, bytes: &[u8] in Path, other: &[u8]) reaches Output {
        transition root.end < bytes.len {
            true -> Root::inspect(root, bytes[0..root.end])
            false -> empty()
        }
        state empty() {}
    }
"#;

fn lower(source: &str) -> LoweredPsi {
    lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("a primitive qualified subslice remains an ordinary borrowed argument")
}

fn verify(lowered: &LoweredPsi) {
    let encoded = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let decoded = terminal_codec::decode_module(&encoded).unwrap();
    assert_eq!(decoded, lowered.semantic_module);
    terminal_verifier::verify_module(
        &decoded,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-free verification derives the exact subslice's primitive byte facts");
}

fn slice_machine(lowered: &mut LoweredPsi) -> &mut TerminalMachine {
    lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
        })
        .expect("the source retains its runtime subslice")
}

#[test]
fn primitive_qualified_subslice_calls_reconstruct_without_result_annotations() {
    for predicate in ["no_nul", "ascii_only"] {
        let mut lowered = lower(&SOURCE.replace("no_nul", predicate));
        verify(&lowered);
        let machine = slice_machine(&mut lowered);
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }) {
                let result = operation.result.structural().unwrap();
                assert!(result.qualifications.is_empty());
                assert!(result.qualification_establishments.is_empty());
            }
        }
    }
}

#[test]
fn primitive_qualified_subslice_calls_compose_with_empty_chained_slices() {
    let source = SOURCE.replace(
        "Root::inspect(root, bytes[0..root.end])",
        "Root::cut(root, bytes[0..root.end])",
    ) + r#"
            machine Root::cut(root: &Root, bytes: &[u8] in Path) reaches Output {
                Root::inspect(root, bytes[0..0]);
            }
        "#;
    let lowered = lower(&source);
    verify(&lowered);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ByteSequenceSubslice { .. }
            ))
            .count(),
        2,
        "both descriptors retain their independent producer and bounds"
    );
}

#[test]
fn primitive_qualified_subslice_named_descriptors_compose() {
    let source = SOURCE.replace(
        r#"transition root.end < bytes.len {
            true -> Root::inspect(root, bytes[0..root.end])
            false -> empty()
        }
        state empty() {}"#,
        r#"let prefix: &[u8] = bytes[0..0];
        Root::inspect(root, prefix[0..0]);"#,
    );
    let lowered = lower(&source);
    verify(&lowered);
}

#[test]
fn qualified_subslice_call_rejects_a_same_typed_unqualified_source() {
    let mut lowered = lower(SOURCE);
    verify(&lowered);
    let machine = slice_machine(&mut lowered);
    let byte_type = machine
        .structural_parameters
        .iter()
        .find(|parameter| !parameter.qualifications.is_empty())
        .unwrap()
        .structural_type;
    let other = machine
        .structural_parameters
        .iter()
        .find(|parameter| {
            parameter.structural_type == byte_type && parameter.qualifications.is_empty()
        })
        .expect("the unrelated shared byte parameter is unqualified")
        .place;
    let slice = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
        .unwrap()
        .result
        .structural()
        .unwrap()
        .place;
    let argument = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructural {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => structural_arguments
                .iter_mut()
                .find(|argument| argument.place == slice),
            _ => None,
        })
        .expect("ordinary call consumes the established subslice");
    argument.place = other;
    let error = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("equal carrier and access do not supply another source's byte predicate");
    assert!(
        format!("{error:?}").contains("StructuralArgumentMissingQualification"),
        "{error:?}"
    );
}

#[test]
fn qualified_subslice_call_does_not_preserve_nonempty_or_utf8() {
    let base = lower(SOURCE);
    verify(&base);
    for predicate in [
        ByteSequencePredicate::NonEmpty,
        ByteSequencePredicate::ValidUtf8,
    ] {
        let mut changed = base.clone();
        let domain = changed
            .semantic_module
            .structural_domains
            .iter_mut()
            .find(|domain| domain.byte_predicate == Some(ByteSequencePredicate::NoNul))
            .expect("Path publishes its exact primitive predicate");
        domain.byte_predicate = Some(predicate);
        let error = terminal_verifier::verify_module(
            &changed.semantic_module,
            &changed.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect_err("an arbitrary byte cut preserves neither UTF-8 boundaries nor nonemptiness");
        assert!(
            format!("{error:?}").contains("StructuralArgumentMissingQualification"),
            "{predicate:?}: {error:?}"
        );
    }
}

#[test]
fn qualified_subslice_result_cannot_assert_its_own_qualification() {
    let mut lowered = lower(SOURCE);
    verify(&lowered);
    let domain = lowered
        .semantic_module
        .structural_domains
        .iter()
        .find(|domain| domain.byte_predicate == Some(ByteSequencePredicate::NoNul))
        .unwrap()
        .id;
    let operation = slice_machine(&mut lowered)
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
        .unwrap();
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.qualifications.push(domain);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("derived primitive facts never authorize result annotations");
}

#[test]
fn source_subslice_rejects_nonpreserving_domain_membership() {
    for (predicate, slice) in [
        ("non_empty", "bytes[0..0]"),
        ("valid_utf8", "bytes[0..root.end]"),
    ] {
        let source = SOURCE
            .replace("no_nul", predicate)
            .replace("bytes[0..root.end]", slice);
        let diagnostics = crate::front_end::checked_program_result(&source)
            .expect_err("nonpreserving predicates do not qualify an arbitrary subslice");
        assert!(
            format!("{diagnostics:?}").contains("cannot prove requires contract"),
            "{predicate}: {diagnostics:?}"
        );
    }
}
