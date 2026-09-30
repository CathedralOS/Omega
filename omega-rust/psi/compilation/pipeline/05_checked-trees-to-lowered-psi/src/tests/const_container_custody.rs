//! Const-specialized record calls retain their witness and receiver custody.

use crate::TerminalMachineSelection;

#[test]
fn const_container_calls_preserve_witness_and_receiver_liveness() {
    // Only package loading/provider selection is replaced: the stores, witness
    // locals, state transfers and two-call getter sum are the authored customer.
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/generics/runtime_const_container_methods_exit/main.omg"
    ))
    .replace("use omega_language_std::console;", "")
    .replace("use omega::language::core::binding;", "");
    let source = format!(
        "pub boundary trait Console {{ machine exit_process(code: i32) reaches Console; }}\n{source}"
    );
    let checked = super::checked_generic_source_with_core_service(&source);
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the normal entrypoint must retain and finalize the same closure");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("encode module");
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode evidence");
    let mut module = terminal_codec::decode_module(&semantic).expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(&evidence).expect("reload evidence");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-free module and evidence independently verify");

    // Recreate the bug on an emitted private call block: the old state's
    // receiver is not usable after its edge transferred exclusive custody.
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let mut changed = false;
    for block in &mut entry.blocks {
        let Some(parameter) = block
            .structural_parameters
            .iter()
            .find(|parameter| parameter.access == terminal_psi::StructuralAccess::MutableBorrow)
        else {
            continue;
        };
        for operation in &mut block.operations {
            if let terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } = &mut operation.kind
            {
                if structural_arguments.len() != 1 {
                    continue;
                }
                if let Some(argument) = structural_arguments
                    .iter_mut()
                    .find(|argument| argument.place == parameter.place)
                {
                    argument.place = entry.structural_parameters[0].place;
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            break;
        }
    }
    assert!(
        changed,
        "the getter must consume a forwarded private receiver"
    );
    assert!(
        terminal_verifier::validate_module(&module).is_err(),
        "stale receiver names cannot bypass custody"
    );
}
