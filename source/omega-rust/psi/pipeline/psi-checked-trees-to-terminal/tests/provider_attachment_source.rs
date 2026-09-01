use psi_core::{
    BoundaryMachineId, IntegerValue, OperationId, PlaceId, StructuralPlaceKind, ValueId,
};
use psi_proof_admission::AdmissionProfile;
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

const SCALAR_RESULT_SOURCE: &str = r#"
    boundary trait Console {
        machine read_code() -> i32
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Main { console: Console; }
    machine Main::main(&mut self)
    reaches Console
    {
        let result: i32 = self.console.read_code();
        self.console.exit_process(result);
    }
"#;

fn straight_line_console_source(write_literals: &[String], exit_status: i32) -> String {
    let writes = write_literals
        .iter()
        .map(|literal| format!("        self.console.write_line(\"{literal}\");\n"))
        .collect::<String>();
    format!(
        r#"
    boundary trait Console {{
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }}

    data Main {{ console: Console; }}
    machine Main::main(&mut self)
    reaches Console
    {{
{writes}        self.console.exit_process({exit_status});
    }}
"#
    )
}

fn numbered_literals(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("line-{index:02}")).collect()
}

fn numbered_stdout(count: usize) -> Vec<u8> {
    (0..count)
        .flat_map(|index| format!("line-{index:02}\n").into_bytes())
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    output
}

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
fn provider_attached_scalar_result_forwards_to_later_call() {
    let lowered = lower_source(SCALAR_RESULT_SOURCE);
    let module = &lowered.semantic_module;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let provider_boundaries = entry
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment { boundary, .. } => Some(boundary),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(provider_boundaries.len(), 2);
    assert!(
        module
            .boundary_machines
            .iter()
            .all(|boundary| provider_boundaries.contains(&boundary.id))
    );

    let boundary_calls = entry.blocks[0]
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
        .collect::<Vec<_>>();
    let [producer, consumer] = boundary_calls.as_slice() else {
        panic!("entry should retain one scalar producer and one Unit consumer")
    };
    let psi_terminal::OperationResult::Scalar(result) = producer.result else {
        panic!("provider-attached boundary call should publish its scalar result")
    };
    let OperationKind::BoundaryCall { arguments, .. } = &consumer.kind else {
        unreachable!()
    };
    assert_eq!(arguments, &[result.id]);

    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("provider-attached scalar result flow verifies");
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
fn source_projection_is_deterministic_and_perturbations_fail_closed() {
    let lowered = lowered();
    let canonical = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode source projection");
    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_WRITE_FIXTURE")
        .or_else(|| std::env::var_os("OMEGA0_WRITE_TERMINAL"))
    {
        std::fs::write(path, hex_bytes(&canonical)).expect("write requested canonical fixture");
    }
    let replay = psi_terminal_codec::encode_module(&lower_source(SOURCE).semantic_module)
        .expect("encode deterministic source projection replay");
    assert_eq!(
        canonical, replay,
        "source projection must be deterministic without a frozen checkpoint"
    );
    assert_eq!(&canonical[..8], b"PSITERM\0");
    assert_eq!(u16::from_le_bytes([canonical[8], canonical[9]]), 54);
    assert_eq!(
        u16::from_le_bytes([canonical[10], canonical[11]]),
        psi_terminal::VocabularyMarker::CURRENT.get()
    );
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

    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_WRITE_VARIANT_FIXTURE")
        .or_else(|| std::env::var_os("OMEGA0_WRITE_VARIANT_TERMINAL"))
    {
        let variant = SOURCE
            .replace("Hello, Omega.", "A\\n")
            .replace("exit_process(0)", "exit_process(2)");
        let bytes = psi_terminal_codec::encode_module(&lower_source(&variant).semantic_module)
            .expect("encode requested shared-codec variant");
        std::fs::write(path, bytes).expect("write requested shared-codec variant");
    }
}

#[test]
fn straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes() {
    let mut exports = Vec::new();
    for count in [0, 1, 2, 16] {
        let source = straight_line_console_source(&numbered_literals(count), count as i32);
        let lowered = lower_source(&source);
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("encode representative straight-line console module");
        let decoded = psi_terminal_codec::decode_module(&bytes)
            .expect("decode representative straight-line console module");
        psi_terminal_verifier::verify_module(
            &decoded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("representative straight-line console module verifies");

        let entry = decoded
            .machines
            .iter()
            .find(|machine| machine.id == decoded.entry)
            .expect("entry machine");
        let operations = &entry.blocks[0].operations;
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::EstablishByteSequenceLiteral { .. }
                ))
                .count(),
            count
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .count(),
            count + 1
        );

        let exit_boundary = decoded
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains("Console::exit_process"))
            .expect("exit boundary")
            .id;
        assert_eq!(exit_boundary, BoundaryMachineId::new(1).unwrap());
        let write_boundary = decoded
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains("Console::write_line"))
            .map(|boundary| boundary.id);
        assert_eq!(write_boundary.is_some(), count > 0);

        let provider_roots = entry
            .structural_places
            .iter()
            .filter(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            .count();
        assert_eq!(provider_roots, if count == 0 { 1 } else { 2 });
        assert_eq!(operations.len(), count * 2 + 2);
        if count > 0 {
            let write_boundary = write_boundary.expect("write boundary for nonempty case");
            assert_eq!(write_boundary, BoundaryMachineId::new(2).unwrap());
            for index in 0..count {
                let literal_place = PlaceId::new(4 + index as u64).unwrap();
                assert!(matches!(
                    &operations[index].kind,
                    OperationKind::EstablishByteSequenceLiteral { destination, bytes }
                        if *destination == literal_place
                            && bytes == format!("line-{index:02}").as_bytes()
                ));
                assert_eq!(
                    operations[index].id,
                    OperationId::new(1 + index as u64).unwrap()
                );
                assert!(matches!(
                    &operations[count + index].kind,
                    OperationKind::BoundaryCall {
                        boundary,
                        arguments,
                        structural_arguments,
                        ..
                    } if *boundary == write_boundary
                        && arguments.is_empty()
                        && structural_arguments.len() == 1
                        && structural_arguments[0].place == literal_place
                        && structural_arguments[0].path.is_empty()
                ));
            }
        }
        assert!(matches!(
            &operations[count * 2].kind,
            OperationKind::IntegerConstant { value: IntegerValue::Signed(value) }
                if *value == count as i128
        ));
        assert!(matches!(
            &operations[count * 2].result,
            psi_terminal::OperationResult::Scalar(value) if value.id == ValueId::new(1).unwrap()
        ));
        assert!(matches!(
            &operations[count * 2 + 1].kind,
            OperationKind::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                ..
            } if *boundary == exit_boundary
                && arguments.as_slice() == [ValueId::new(1).unwrap()]
                && structural_arguments.is_empty()
        ));

        exports.push((
            format!("writes-{count}"),
            count,
            count as i32,
            numbered_stdout(count),
            bytes,
            true,
        ));
    }

    if let Some(directory) = std::env::var_os("OMEGA1_WRITE_TERMINAL_REFERENCES") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create O1 terminal reference directory");
        let oversized_literals = vec!["x".repeat(600), "y".repeat(600)];
        for (name, literals, exit_status) in [
            ("reject-writes-17", numbered_literals(17), 17),
            ("reject-bytes-1200", oversized_literals, 23),
        ] {
            let source = straight_line_console_source(&literals, exit_status);
            let lowered = lower_source(&source);
            let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
                .expect("encode out-of-profile canonical terminal reference");
            psi_terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .expect("out-of-profile terminal reference remains canonical product Psi");
            let mut stdout = Vec::new();
            for literal in &literals {
                stdout.extend_from_slice(literal.as_bytes());
                stdout.push(b'\n');
            }
            exports.push((
                name.to_owned(),
                literals.len(),
                exit_status,
                stdout,
                bytes,
                false,
            ));
        }

        let mut manifest = String::from("case\to1_admitted\twrites\texit\tstdout_hex\tterminal\n");
        for (name, count, exit_status, stdout, bytes, admitted) in exports {
            let file = format!("{name}.terminal");
            std::fs::write(directory.join(&file), bytes)
                .expect("write requested O1 terminal reference");
            manifest.push_str(&format!(
                "{name}\t{}\t{count}\t{exit_status}\t{}\t{file}\n",
                if admitted { 1 } else { 0 },
                hex_bytes(&stdout)
            ));
        }
        std::fs::write(directory.join("manifest.tsv"), manifest)
            .expect("write requested O1 terminal reference manifest");
    }
}
