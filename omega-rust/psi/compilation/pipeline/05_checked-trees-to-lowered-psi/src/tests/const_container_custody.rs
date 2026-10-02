//! Const-specialized record calls retain their witness and receiver custody.

use crate::TerminalMachineSelection;

#[test]
fn interval_domain_casts_preserve_one_result_and_original_input_prefix() {
    for (parameters, operand) in [
        ("", "1 as u64 in AtMost<256>"),
        ("preserved: u8", "1 as u64 in AtMost<256>"),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "domain<const Bound: u64> u64::AtMost<Bound> requires self <= Bound;
             machine convert({parameters}) -> u64 in AtMost<256> {{ {operand} }}",
        ));
        let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("convert"))
            .expect("the cast transfers one evaluated result and preserves the original inputs");
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
        .expect("serialized conversion replays its original input prefix and sole result");
    }
}

#[test]
fn indexed_domain_capacity_customer_survives_serialized_terminal_replay() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/generics/omitted_data_binder_range_equation/main.omg"
    ));
    // This unit helper admits closed generic data, but does not run the full
    // compiler's legacy range-equation normalization. Select the unchanged
    // runtime customer and its domain; the corpus compiles the entire fixture.
    let domain_start = source.find("domain<const Bound: u64>").unwrap();
    let domain_end = source[domain_start..].find("\ndata Bytes").unwrap() + domain_start;
    let customer_start = source
        .find("// The specification's domain-index equation")
        .unwrap();
    let source = format!(
        "pub boundary trait Console {{ machine exit_process(code: i32) reaches Console; }}\n{}\n{}",
        &source[domain_start..domain_end],
        &source[customer_start..],
    );
    let checked = super::checked_generic_source_with_core_service(&source);
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("inferred capacity retains nested fixed storage and domain mint");
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
    .expect("serialized customer and evidence verify without typed source");
    let index = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::PrimitiveScalarRead { path, .. } => {
                path.iter_mut().find_map(|segment| match segment {
                    terminal_psi::StructuralPathSegment::FixedIndex(index) => Some(index),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("nested backing read retains its exact physical index");
    assert_eq!(*index, 255);
    *index = 256;
    assert!(
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default()
        )
        .is_err(),
        "inferred capacity rejects an out-of-bounds serialized backing read"
    );
}

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
