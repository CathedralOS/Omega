use omega_abstract_operations::AbstractOperation;
use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::ProfileDecisionId;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::TerminalAffineCleanupAction;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Empty {}
    data Token { value: u64; }
    data Root {}

    machine Root::cleanup(input_first: Token, input_second: Token) {
        let local_first: Empty = Empty {};
        let local_second: Empty = Empty {};
    }
"#;

#[test]
fn unit_affine_local_cleanup_survives_all_native_artifacts() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::cleanup").expect("terminal Psi");
    let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified Omega entry");
    assert!(matches!(
        abstract_plan.functions[0].operations.as_slice(),
        [
            AbstractOperation::EstablishTrivialAffineLocal { .. },
            AbstractOperation::EstablishTrivialAffineLocal { .. },
            AbstractOperation::ReturnUnit { cleanup_actions, .. }
        ] if cleanup_actions.len() == 4
    ));

    for (target, subsystem, expected_subsystem) in [
        (NativeTarget::linux_x64(), 3, None),
        (NativeTarget::windows_x64(), 3, Some(3)),
        (NativeTarget::uefi_x64(), 10, Some(10)),
        (NativeTarget::linux_arm64(), 3, None),
        (NativeTarget::macos_arm64(), 3, None),
    ] {
        let target_plan = lower_to_target_operations(&abstract_plan, target).expect("target");
        let assigned = assign_registers(&target_plan).expect("assignment");
        let machine = emit_machine_code(&assigned).expect("machine");
        let cleanup = machine.functions[0]
            .unit_affine_cleanup
            .as_ref()
            .expect("machine cleanup custody");
        assert_eq!(cleanup.locals.len(), 2);
        assert_eq!(cleanup.actions.len(), 4);
        assert_eq!(
            cleanup.actions,
            [
                TerminalAffineCleanupAction::DiscardRoot(cleanup.locals[1].1.id),
                TerminalAffineCleanupAction::DiscardRoot(cleanup.locals[0].1.id),
                TerminalAffineCleanupAction::DiscardRoot(
                    machine.functions[0].unit_parameters[1].place,
                ),
                TerminalAffineCleanupAction::DiscardRoot(
                    machine.functions[0].unit_parameters[0].place,
                ),
            ]
        );
        assert_eq!(
            machine.functions[0]
                .semantic_code_attribution
                .iter()
                .filter(|row| row.byte_count == 0)
                .count(),
            2
        );

        let mut reordered = machine.clone();
        reordered.functions[0]
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .swap(0, 1);
        assert!(build_object_artifact(&reordered).is_err());

        let mut missing = machine.clone();
        missing.functions[0].unit_affine_cleanup = None;
        assert!(build_object_artifact(&missing).is_err());

        let mut duplicate_operation = machine.clone();
        let first_operation = duplicate_operation.functions[0]
            .unit_affine_cleanup
            .as_ref()
            .unwrap()
            .locals[0]
            .0;
        duplicate_operation.functions[0]
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .locals[1]
            .0 = first_operation;
        assert!(build_object_artifact(&duplicate_operation).is_err());

        let mut forged_signature = machine.clone();
        forged_signature.functions[0].unit_parameter_homes[0].multiplicity =
            psi_terminal::StructuralMultiplicity::Unrestricted;
        assert!(build_object_artifact(&forged_signature).is_err());

        let object = build_object_artifact(&machine).expect("object");
        let image = emit_executable_image(&object, subsystem).expect("image");
        assert_eq!(image.subsystem(), expected_subsystem);
        let installation = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
            .expect("installation");
        validate_installation_record(&installation, &image).expect("image binding");
        let bytes = encode_installation_record(&installation).expect("install encoding");
        assert_eq!(decode_installation_record(&bytes), Ok(installation));
    }
}
