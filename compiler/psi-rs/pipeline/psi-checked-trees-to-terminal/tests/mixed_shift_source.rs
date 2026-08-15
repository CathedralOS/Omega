use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue, TerminalStructuralValue,
    interpret_terminal_artifact_with_effect_handler_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}

    machine Root::measure(token: Token, value: u8, signed: i8, enabled: bool) -> bool
    requires value <= 31u8, -32i8 <= signed, signed <= 31i8, 0i8 <= signed
    {
        ((((((value >> 1i8) >> 2u16) << 1i32) << 1u64) < 255u8)
            && (((value >> 1i8) << 4u16) < 255u8))
            && ((((signed >> 1u8) << 3i16) < 127i8)
                && (((((signed >> 7i8) >> 1u16) << 7i32) << 1u64) < 127i8))
            && (((((value >> 7i8) >> 1u16) << 7i32) << 7u64) < 255u8)
            && enabled
    }
"#;

#[test]
fn exact_right_shift_chains_feed_independently_verified_left_shift_prefixes() {
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize mixed shifts");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed shifts");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed shifts");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type mixed shifts");
    let checked = lower_typed_trees(typed).expect("check mixed shifts");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("mixed shifts lower to Terminal Psi");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed-shift entry machine");
    let [token] = entry.structural_parameters.as_slice() else {
        panic!("mixed-shift entry retains its nominal cleanup root")
    };
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let shift_obligations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft { obligation, .. }
            | OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { .. }
            ))
            .count(),
        8,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { .. }
            ))
            .count(),
        8,
    );
    assert_eq!(shift_obligations.len(), 16);
    for (index, obligation) in shift_obligations.iter().enumerate() {
        assert!(!shift_obligations[index + 1..].contains(obligation));
        assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
            evidence.obligation == *obligation
                && matches!(evidence.route, EvidenceRoute::CertificateDerived(_))
        }));
    }

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed shifts verify independently");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("mixed shifts have fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("mixed-shift fuel recomputes");
    drop(verified);

    let semantics = encode_module(&lowered.semantic_module).expect("encode mixed-shift module");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode mixed-shift proof");
    assert_eq!(
        decode_module(&semantics).expect("decode mixed-shift module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof).expect("decode mixed-shift proof"),
        lowered.proof_bundle,
    );

    for obligation in &shift_obligations {
        let mut missing = decode_proof_bundle(&proof).expect("decode mixed-shift proof");
        missing
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            psi_terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode unchanged mixed-shift module"),
                &missing,
                &AdmissionProfile::default(),
            ),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing_obligation))
                if missing_obligation == *obligation
        ));
    }

    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count type");
    let mut changed_count = decode_module(&semantics).expect("decode mixed-shift module");
    let landed_two = changed_count
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(2),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
        })
        .expect("mixed shifts retain their landed u16 count");
    landed_two.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(8),
    };
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_count,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if shift_obligations.contains(&obligation)
    ));

    let scalar_arguments = |enabled| {
        vec![
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Boolean(enabled),
        ]
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    for enabled in [false, true] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &scalar_arguments(enabled),
            &structural_arguments,
            &mut handler,
        )
        .expect("mixed shifts interpret from canonical artifacts");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(enabled)),
        );
        assert!(measured.usage().total_units() <= fixed.ceiling_units());
        assert!(measured.effects().is_empty());
    }
}
