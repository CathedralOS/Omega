use psi_core::{IntegerValue, StructuralPlaceKind};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{BindingRelevance, OperationKind, StructuralFieldType, StructuralTypeShape};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Main { console: Console; }
    machine Main::main(&mut self)
    reaches Console
    {
        self.console.write_line("Hello, Omega.");
        self.console.exit_process(0);
    }
"#;

fn lower_source(source: &str) -> psi_checked_trees_to_terminal::LoweredTerminalPsi {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    psi_checked_trees_to_terminal::lower_machine(&checked, "Main::main")
        .expect("provider-backed O0 source should lower")
}

fn lowered() -> psi_checked_trees_to_terminal::LoweredTerminalPsi {
    lower_source(SOURCE)
}

fn fixture_bytes() -> Vec<u8> {
    let hex =
        include_str!("../../../../../bootstrap/omega0/gates/fixtures/omega0-terminal-v25.hex");
    let digits = hex
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(digits.len() % 2, 0, "fixture hex must contain whole bytes");
    digits
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => panic!("fixture contains a non-hex digit"),
            };
            digit(pair[0]) << 4 | digit(pair[1])
        })
        .collect()
}

#[test]
fn provider_backed_main_retains_attachment_and_exact_installation_requirements() {
    let lowered = lowered();
    assert!(
        lowered.proof_bundle.evidence.is_empty()
            && lowered.proof_bundle.evidence_producers.is_empty(),
        "proof-free O0 must carry the canonical empty proof bundle"
    );
    let module = &lowered.semantic_module;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let attachment_id = entry
        .attachment
        .expect("Main attachment must not be erased");
    assert!(entry.structural_parameters.is_empty());
    let attachment = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment_id)
        .expect("Main structural declaration");
    let StructuralTypeShape::Record { fields } = &attachment.shape else {
        panic!("Main must remain a record")
    };
    let [console] = fields.as_slice() else {
        panic!("Main must retain exactly the authored console field")
    };
    assert_eq!(console.identity, "console");
    assert_eq!(console.relevance, BindingRelevance::Relevant);
    assert!(matches!(
        &console.field_type,
        StructuralFieldType::Erased { type_identity } if type_identity.contains("Console")
    ));

    let provider_roots = entry
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(provider_roots.len(), 2);
    assert!(
        provider_roots
            .iter()
            .all(|(attachment, field, _)| { *attachment == attachment_id && *field == console.id })
    );
    assert_eq!(
        provider_roots
            .iter()
            .map(|(_, _, boundary)| *boundary)
            .collect::<Vec<_>>(),
        module
            .boundary_machines
            .iter()
            .map(|boundary| boundary.id)
            .collect::<Vec<_>>()
    );

    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("provider-backed attachment specialization verifies");
    let bytes = psi_terminal_codec::encode_module(module).expect("canonical attachment bytes");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode attachment bytes"),
        *module
    );
}

#[test]
fn provider_attachment_tampering_fails_closed() {
    let lowered = lowered();
    let mut missing_requirement = lowered.semantic_module.clone();
    let entry = missing_requirement
        .machines
        .iter_mut()
        .find(|machine| machine.id == missing_requirement.entry)
        .expect("entry machine");
    let root = entry
        .structural_places
        .iter()
        .position(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
        .expect("provider root");
    entry.structural_places.remove(root);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &missing_requirement,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::Module(
            psi_terminal_verifier::ModuleError::InvalidProviderAttachmentSpecialization(_)
        ))
    ));

    let mut missing_attachment = lowered.semantic_module.clone();
    missing_attachment
        .machines
        .iter_mut()
        .find(|machine| machine.id == missing_attachment.entry)
        .expect("entry machine")
        .attachment = None;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &missing_attachment,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::Module(
            psi_terminal_verifier::ModuleError::InvalidProviderAttachmentSpecialization(_)
        ))
    ));
}

#[test]
fn source_projection_is_the_shared_o0_fixture_and_perturbations_fail_closed() {
    let lowered = lowered();
    let canonical = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode source projection");
    let fixture = fixture_bytes();
    assert_eq!(
        canonical, fixture,
        "source projection must own the O0 fixture"
    );
    assert_eq!(&canonical[..8], b"PSITERM\0");
    assert_eq!(u16::from_le_bytes([canonical[8], canonical[9]]), 22);
    assert_eq!(u16::from_le_bytes([canonical[10], canonical[11]]), 25);
    let decoded = psi_terminal_codec::decode_module(&canonical).expect("decode O0 fixture");
    psi_terminal_verifier::validate_module(&decoded).expect("validate O0 fixture");
    assert_eq!(
        psi_terminal_codec::encode_module(&decoded).expect("re-encode O0 fixture"),
        canonical
    );
    assert!(
        decoded
            .machines
            .iter()
            .find(|machine| machine.id == decoded.entry)
            .expect("entry machine")
            .attachment
            .is_some()
    );

    let mut literal = decoded.clone();
    let literal_bytes = literal
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::EstablishByteSequenceLiteral { bytes, .. } => Some(bytes),
            _ => None,
        })
        .expect("byte-sequence literal");
    literal_bytes[0] ^= 1;
    assert_ne!(
        psi_terminal_codec::encode_module(&literal).expect("encode changed literal"),
        canonical
    );

    let mut scalar = decoded.clone();
    let integer = scalar
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::IntegerConstant { value } => Some(value),
            _ => None,
        })
        .expect("exit status literal");
    *integer = IntegerValue::Signed(1);
    assert_ne!(
        psi_terminal_codec::encode_module(&scalar).expect("encode changed scalar"),
        canonical
    );

    let mut reordered = decoded.clone();
    let operations = &mut reordered
        .machines
        .iter_mut()
        .find(|machine| machine.id == reordered.entry)
        .expect("entry machine")
        .blocks[0]
        .operations;
    let operation_ids = operations
        .iter()
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    operations.rotate_left(2);
    for (operation, id) in operations.iter_mut().zip(operation_ids) {
        operation.id = id;
    }
    assert!(psi_terminal_codec::encode_module(&reordered).is_err());

    let literal_offset = canonical
        .windows(b"Hello, Omega.".len())
        .position(|window| window == b"Hello, Omega.")
        .expect("encoded literal bytes");
    let mut wrong_length = canonical.clone();
    wrong_length[literal_offset - 4] += 1;
    assert!(psi_terminal_codec::decode_module(&wrong_length).is_err());

    for end in 0..canonical.len() {
        assert!(
            psi_terminal_codec::decode_module(&canonical[..end]).is_err(),
            "truncated fixture decoded at byte {end}"
        );
    }

    let mut impossible_count = canonical.clone();
    impossible_count[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(psi_terminal_codec::decode_module(&impossible_count).is_err());

    if let Some(path) = std::env::var_os("OMEGA0_WRITE_VARIANT_TERMINAL") {
        let variant = SOURCE
            .replace("Hello, Omega.", "A\\n")
            .replace("exit_process(0)", "exit_process(2)");
        let bytes = psi_terminal_codec::encode_module(&lower_source(&variant).semantic_module)
            .expect("encode requested shared-codec variant");
        std::fs::write(path, bytes).expect("write requested shared-codec variant");
    }
}
