//! A verified parameter-root Jump retains no-code cleanup through installation.

use super::*;

#[test]
fn projected_parameter_cleanup_precedes_a_distinct_native_return() {
    let source = "data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Root {} data Sink {}
        machine Sink::take(value: Token) {}
        machine Root::enter(value: Pair) { Sink::take(value.right); }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let mut terminal = lower_machine(&checked, "Root::enter").unwrap();
    let entry = terminal.semantic_module.entry;
    let successor = semantic_vocabulary::BlockId::new(999).unwrap();
    let returned = semantic_vocabulary::EdgeId::new(999).unwrap();
    let caller = terminal
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap();
    let Terminator::ReturnUnitPartialAffine {
        edge,
        trivial_affine_discards,
        residual_affine_discards,
    } = caller.blocks[0].terminator.clone()
    else {
        panic!("partial parameter return")
    };
    caller.blocks[0].terminator = Terminator::Jump {
        structural_arguments: Vec::new(),
        edge,
        target: successor,
        arguments: Vec::new(),
        trivial_affine_discards,
        residual_affine_discards,
    };
    caller.blocks.push(terminal_psi::Block {
        structural_parameters: Vec::new(),
        id: successor,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: returned,
            trivial_affine_discards: Vec::new(),
        },
    });
    verify_module(
        &terminal.semantic_module,
        &terminal.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    for case in target_cases() {
        let target = lower_to_target_operations(&plan, case.target).unwrap();
        let assigned = assign_registers(&target).unwrap();
        let emitted = emit_machine_code(&assigned).unwrap();
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == entry)
            .unwrap();
        let [continuation] = caller.unit_continuations.as_slice() else {
            panic!("one continuation")
        };
        assert_eq!(continuation.cleanup.psi_edge, edge);
        assert_eq!(continuation.target_block, successor);
        assert_eq!(continuation.cleanup.byte_count, 0);
        let final_cleanup = caller.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(final_cleanup.psi_edge, returned);
        assert_eq!(final_cleanup.code_offset, continuation.cleanup.code_offset);
        assert!(final_cleanup.actions.is_empty());
        let object = build_object_artifact(&emitted).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let decoded =
            decode_installation_record(&encode_installation_record(&installation).unwrap())
                .unwrap();
        validate_installation_record(&decoded, &image).unwrap();
        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            super::affine_call_result_host::execute(
                &image,
                object.entry_function().text_offset,
                true,
            );
        }
    }
}
