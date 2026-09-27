use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use semantic_vocabulary::IntegerValue;
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Main<'s> { console: &'s mut Console; }
    machine Main::main(&mut self)
    reaches Console
    {
        self.console.write_line("Hello, Omega.");
        self.console.exit_process(0);
    }
"#;

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    output
}

fn lower_source(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("provider-backed O0 source should lower")
}

fn lowered() -> lowered_psi::LoweredPsi {
    lower_source(SOURCE)
}

#[test]
fn static_boundary_attachment_composes_without_receiver_custody() {
    for after_handoff in [false, true] {
        let (prefix, suffix) = if after_handoff {
            (
                "transition { _ -> choose(input) } state choose(input: u32) {",
                "}",
            )
        } else {
            ("", "")
        };
        let source = format!(
            "pub data Math {{}} data Root {{}}
             pub boundary requirement Math::choose(value: u32) -> u32;
             machine identity(value: u32) -> u32 {{ value }}
             machine Root::enter(input: u32) {{
                 {prefix}
                 let ordinary: u32 = identity(input);
                 let selected: u32 = Math::choose(ordinary);
                 transition selected == input {{ true -> done() _ -> done() }}
                 {suffix}
                 state done() {{}}
             }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
            .unwrap_or_else(|error| panic!("after_handoff={after_handoff}: {error:?}"));
        let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
            panic!("one exact static boundary requirement");
        };
        assert!(
            boundary.attachment.is_some(),
            "nominal owner remains retained"
        );
        assert!(
            boundary.structural_parameters.is_empty(),
            "no invented self"
        );
        assert!(matches!(
            boundary.result,
            terminal_psi::BoundaryMachineResult::Scalar(_)
        ));
        let calls = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::BoundaryCall {
                    arguments,
                    structural_arguments,
                    ..
                } => Some((arguments, structural_arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(arguments, structural_arguments)] = calls.as_slice() else {
            panic!("one retained boundary call");
        };
        assert_eq!(arguments.len(), 1);
        assert!(structural_arguments.is_empty());
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("static attachment and scalar result independently verify");
        let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
        assert_eq!(
            terminal_codec::decode_module(&bytes).expect("decode"),
            lowered.semantic_module
        );

        let mut invalid = lowered.semantic_module.clone();
        let arguments = invalid
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::BoundaryCall { arguments, .. } => Some(arguments),
                _ => None,
            })
            .expect("boundary scalar arguments");
        arguments.clear();
        assert!(
            terminal_verifier::verify_module(
                &invalid,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "static attachment does not waive exact operand replay"
        );

        let mut missing_owner = checked.clone();
        missing_owner
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines[0]
            .attachment_type_identity = None;
        assert!(
            matches!(
                lower_machine(
                    &missing_owner,
                    TerminalMachineSelection::Name("Root::enter")
                ),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "Unit graph boundary attachment lost its authored owner"
                ))
            ),
            "a static signature still retains its authored nominal attachment"
        );
        let mut substituted_owner = checked.clone();
        let other_identity = substituted_owner
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .find_map(|plan| plan.attachment_type_identity.clone())
            .expect("Root has its own retained attachment");
        assert!(
            substituted_owner
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|shape| shape.identity == other_identity)
        );
        substituted_owner
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines[0]
            .attachment_type_identity = Some(other_identity);
        assert!(
            matches!(
                lower_machine(
                    &substituted_owner,
                    TerminalMachineSelection::Name("Root::enter")
                ),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "Unit graph boundary attachment disagrees with its authored nominal identity"
                ))
            ),
            "another existing structural type cannot replace the boundary owner"
        );
    }
}

#[test]
fn composed_boundary_receiver_keeps_linear_owned_custody() {
    let baseline = crate::front_end::checked_program(
        r#"
            pub data Receipt [linear] { value: u64; }
            boundary machine Receipt::settle(self) ensures true;

            data Root {}
            machine Root::enter(flag: bool, receipt: Receipt) {
                transition flag {
                    true -> yes(receipt)
                    _ -> no(receipt)
                }
                state yes(receipt: Receipt) { receipt.settle(); }
                state no(receipt: Receipt) { receipt.settle(); }
            }
        "#,
    );
    for corruption in ["attachment", "multiplicity", "access", "receiver"] {
        let mut invalid = baseline.clone();
        let boundary = &mut invalid.facts.flow.terminal_unit_effects.boundary_machines[0];
        match corruption {
            "attachment" => boundary.attachment_type_identity = None,
            "multiplicity" => {
                boundary.structural_parameters[0].multiplicity =
                    language_semantics::Multiplicity::Affine
            }
            "access" => {
                boundary.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            _ => boundary.structural_parameters[0].is_self = false,
        }
        let result = lower_machine(&invalid, TerminalMachineSelection::Name("Root::enter"));
        if corruption == "receiver" {
            assert!(
                matches!(
                    result,
                    Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                        "Unit graph boundary receiver disagrees with its authored signature"
                    ))
                ),
                "receiver erasure must fail the independent signature rejoin: {result:?}"
            );
        } else {
            assert!(
                result.is_err(),
                "receiver custody corruption must reject: {corruption}"
            );
        }
    }
}

#[test]
fn source_projection_is_deterministic_and_perturbations_fail_closed() {
    let lowered = lowered();
    let canonical =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode source projection");
    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_WRITE_FIXTURE")
        .or_else(|| std::env::var_os("OMEGA0_WRITE_TERMINAL"))
    {
        std::fs::write(path, hex_bytes(&canonical)).expect("write requested canonical fixture");
    }
    let replay = terminal_codec::encode_module(&lower_source(SOURCE).semantic_module)
        .expect("encode deterministic source projection replay");
    assert_eq!(
        canonical, replay,
        "source projection must be deterministic without a frozen checkpoint"
    );
    assert_eq!(&canonical[..8], b"PSITERM\0");
    assert_eq!(u16::from_le_bytes([canonical[8], canonical[9]]), 107);
    assert_eq!(
        u16::from_le_bytes([canonical[10], canonical[11]]),
        terminal_psi::VocabularyMarker::CURRENT.get()
    );
    let decoded = terminal_codec::decode_module(&canonical).expect("decode O0 fixture");
    terminal_verifier::validate_module(&decoded).expect("validate O0 fixture");
    assert_eq!(
        terminal_codec::encode_module(&decoded).expect("re-encode O0 fixture"),
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
        terminal_codec::encode_module(&literal).expect("encode changed literal"),
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
        terminal_codec::encode_module(&scalar).expect("encode changed scalar"),
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
    // Literal establishments are no longer pinned to the block prefix, so a
    // reordered body is a different but still valid program: it must encode
    // to its own canonical bytes rather than alias the source module's.
    assert_ne!(
        terminal_codec::encode_module(&reordered).expect("encode reordered operations"),
        canonical
    );

    let mut misplaced = decoded.clone();
    misplaced
        .machines
        .iter_mut()
        .find(|machine| machine.id == misplaced.entry)
        .expect("entry machine")
        .structural_places
        .rotate_left(1);
    assert!(terminal_codec::encode_module(&misplaced).is_err());

    let literal_offset = canonical
        .windows(b"Hello, Omega.".len())
        .position(|window| window == b"Hello, Omega.")
        .expect("encoded literal bytes");
    let mut wrong_length = canonical.clone();
    wrong_length[literal_offset - 4] += 1;
    assert!(terminal_codec::decode_module(&wrong_length).is_err());

    for end in 0..canonical.len() {
        assert!(
            terminal_codec::decode_module(&canonical[..end]).is_err(),
            "truncated fixture decoded at byte {end}"
        );
    }

    let mut impossible_count = canonical.clone();
    impossible_count[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(terminal_codec::decode_module(&impossible_count).is_err());

    if let Some(path) = std::env::var_os("OMEGA_TERMINAL_WRITE_VARIANT_FIXTURE")
        .or_else(|| std::env::var_os("OMEGA0_WRITE_VARIANT_TERMINAL"))
    {
        let variant = SOURCE
            .replace("Hello, Omega.", "A\\n")
            .replace("exit_process(0)", "exit_process(2)");
        let bytes = terminal_codec::encode_module(&lower_source(&variant).semantic_module)
            .expect("encode requested shared-codec variant");
        std::fs::write(path, bytes).expect("write requested shared-codec variant");
    }
}
