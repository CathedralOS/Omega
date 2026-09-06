use abstract_operations_to_target_operations::lower_to_target_operations;
use checked_trees_to_lowered_psi::lower_machine;
use image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, validate_installation_record,
};
use machine_emission::emit_machine_code;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ProfileDecisionId;
use target::NativeTarget;
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;

#[test]
fn projected_subtree_copy_rejects_coordinated_function_and_custody_byte_mutations() {
    let source = "
        data Token { value: u64; }
        data Helper {}
        machine Helper::take(token: Token) {}
        machine Helper::take_row(values: [Token; 3]) {}
        data Root {}
        machine Root::enter(values: [[Token; 3]; 2]) {
            Helper::take_row(values[1]);
            Helper::take(values[0][1]);
        }
    ";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "Root::enter").unwrap();
    let plan = lower_artifact_sections(
        &encode_module(&terminal.semantic_module).unwrap(),
        &encode_proof_bundle(&terminal.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&lowered).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let object = build_object_artifact(&machine).unwrap_or_else(|error| {
            panic!("valid projected subtree copy on {target:?}: {error:?}")
        });
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();

        let caller = machine
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let copy = &caller.internal_unit_calls[0].arguments[0];
        assert_eq!(copy.shape.byte_size, 24);
        assert_eq!(copy.source_byte_offset, 24);
        assert!(!copy.bytes.is_empty());
        for position in [0, copy.bytes.len() / 2, copy.bytes.len() - 1] {
            let mut forged = machine.clone();
            let caller = forged
                .functions
                .iter_mut()
                .find(|function| function.machine == plan.entry)
                .unwrap();
            let argument = &mut caller.internal_unit_calls[0].arguments[0];
            argument.bytes[position] ^= 1;
            caller.bytes[argument.code_offset + position] ^= 1;
            assert!(
                build_object_artifact(&forged).is_err(),
                "matching forged function and custody bytes must fail for {target:?}, byte {position}"
            );
        }

        let encoded = encode_installation_record(&installation).unwrap();
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
        let offsets = encoded
            .windows(copy.bytes.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == copy.bytes).then_some(offset))
            .collect::<Vec<_>>();
        assert!(
            !offsets.is_empty(),
            "installation retains the subtree copy bytes"
        );
        for offset in offsets {
            let mut forged = encoded.clone();
            forged[offset] ^= 1;
            if let Ok(record) = decode_installation_record(&forged) {
                assert!(validate_installation_record(&record, &image).is_err());
            }
        }
    }
}
