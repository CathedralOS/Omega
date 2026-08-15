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

    machine Root::measure(
        token: Token,
        value: u8,
        signed: i8,
        wide: u16,
        signed_wide: i16,
        post_signed: i16,
        post_unsigned: u16,
        affine_unsigned: u8,
        affine_signed: i8,
        zero_root: u8,
        shift_affine_unsigned: u8,
        shift_affine_signed: i8,
        shift_zero_root: u8,
        sandwich_unsigned: u16,
        sandwich_signed: i16,
        sandwich_right_only: u16,
        affine_cast_shift_unsigned: u16,
        affine_cast_shift_signed: i16,
        affine_cast_shift_zero: u16,
        shift_cast_affine_unsigned: u16,
        shift_cast_affine_signed: i16,
        shift_cast_affine_zero: u16,
        enabled: bool
    ) -> bool
    requires value <= 127u8, value <= 63u8, value <= 31u8,
        -32i8 <= signed, signed <= 31i8, 0i8 <= signed,
        wide <= 32767u16, wide <= 16383u16, wide <= 63u16,
        -16384i16 <= signed_wide, signed_wide <= 16383i16,
        0i16 <= signed_wide, signed_wide <= 127i16,
        0i16 <= post_signed, post_signed <= 255i16,
        post_signed <= 127i16, post_signed <= 63i16,
        post_unsigned <= 127u16, post_unsigned <= 63u16,
        affine_unsigned <= 252u8, affine_unsigned <= 124u8,
        affine_unsigned <= 60u8,
        affine_signed <= 124i8, -67i8 <= affine_signed,
        affine_signed <= 60i8, -35i8 <= affine_signed, affine_signed <= 28i8,
        zero_root <= 0u8,
        shift_affine_unsigned <= 127u8, shift_affine_unsigned <= 63u8,
        -64i8 <= shift_affine_signed, shift_affine_signed <= 63i8,
        -32i8 <= shift_affine_signed, shift_affine_signed <= 31i8,
        shift_zero_root <= 127u8,
        sandwich_unsigned <= 32767u16, sandwich_unsigned <= 127u16,
        sandwich_unsigned <= 63u16,
        -16384i16 <= sandwich_signed, sandwich_signed <= 16383i16,
        0i16 <= sandwich_signed, sandwich_signed <= 127i16,
        sandwich_signed <= 63i16,
        sandwich_right_only <= 32767u16, sandwich_right_only <= 127u16,
        affine_cast_shift_unsigned <= 65534u16,
        affine_cast_shift_unsigned <= 32766u16,
        affine_cast_shift_unsigned <= 126u16,
        affine_cast_shift_unsigned <= 62u16,
        affine_cast_shift_signed <= 32764i16,
        -16387i16 <= affine_cast_shift_signed,
        affine_cast_shift_signed <= 16380i16,
        -3i16 <= affine_cast_shift_signed,
        affine_cast_shift_signed <= 124i16,
        affine_cast_shift_signed <= 60i16,
        affine_cast_shift_zero <= 0u16,
        shift_cast_affine_unsigned <= 32767u16,
        shift_cast_affine_unsigned <= 127u16,
        shift_cast_affine_unsigned <= 63u16,
        -16384i16 <= shift_cast_affine_signed,
        shift_cast_affine_signed <= 16383i16,
        0i16 <= shift_cast_affine_signed,
        shift_cast_affine_signed <= 127i16,
        shift_cast_affine_signed <= 63i16,
        shift_cast_affine_zero <= 32767u16,
        shift_cast_affine_zero <= 127u16
    {
        ((((((value >> 1i8) >> 2u16) << 1i32) << 1u64) < 255u8)
            && (((value >> 1i8) << 4u16) < 255u8))
            && ((((signed >> 1u8) << 3i16) < 127i8)
                && (((((signed >> 7i8) >> 1u16) << 7i32) << 1u64) < 127i8))
            && (((((value >> 7i8) >> 1u16) << 7i32) << 7u64) < 255u8)
            && (((value << 1i8) >> 2u16) < 255u8)
            && (((((value << 1i8) >> 2u16) << 3i32) >> 1u64) < 255u8)
            && (((((wide << 1i8) >> 2u16) << 3i32) as u8) < 255u8)
            && ((((signed_wide >> 1u8) << 2i16) as u8) < 255u8)
            && ((((((post_signed as u8) << 1i8) >> 2u16) << 3i32) < 255u8))
            && (((((post_unsigned as i8) << 1u8) >> 2i16) < 127i8))
            && ((((((affine_unsigned + 3u8) * 2u8) >> 1i8) << 2u16) < 255u8))
            && ((((((affine_signed - -3i8) * 2i8) >> 1u16) << 2i32) < 127i8))
            && ((((((zero_root + 255u8) * 0u8) << 1u8) >> 1i16) < 255u8))
            && ((((((shift_affine_unsigned >> 1i8) << 2u16) + 3u8) * 2u8) < 255u8))
            && ((((((shift_affine_signed >> 1u8) << 2i16) - -3i8) * 2i8) < 127i8))
            && (((((shift_zero_root << 1u8) * 0u8) + 255u8) <= 255u8))
            && (((((sandwich_unsigned >> 1i8) << 2u16) as u8) >> 1i32) << 2u64) < 255u8
            && (((((sandwich_signed >> 1u8) << 2i16) as u8) >> 1u32) << 2i64) < 255u8
            && (((sandwich_right_only << 1u8) as u8) >> 1i16) < 255u8
            && ((((((affine_cast_shift_unsigned + 1u16) * 2u16) as u8) >> 1i8) << 2u32) < 255u8)
            && ((((((affine_cast_shift_signed - -3i16) * 2i16) as u8) >> 1u16) << 2i32) < 255u8)
            && (((((affine_cast_shift_zero + 65535u16) * 0u16) as u8) << 2u8) < 255u8)
            && ((((((shift_cast_affine_unsigned >> 1i8) << 2u16) as u8) + 3u8) * 2u8) < 255u8)
            && ((((((shift_cast_affine_signed >> 1u8) << 2i16) as u8) + 3u8) * 2u8) < 255u8)
            && (((((shift_cast_affine_zero << 1u8) as u8) * 0u8) + 255u8) <= 255u8)
            && enabled
    }
"#;

#[test]
fn arbitrary_exact_mixed_shift_chains_retain_independent_prefix_proofs() {
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
    let value_parameter = entry.parameters[0].id;
    let wide_parameter = entry.parameters[2].id;
    let signed_wide_parameter = entry.parameters[3].id;
    let post_signed_parameter = entry.parameters[4].id;
    let affine_unsigned_parameter = entry.parameters[6].id;
    let shift_affine_unsigned_parameter = entry.parameters[9].id;
    let sandwich_unsigned_parameter = entry.parameters[12].id;
    let affine_cast_shift_unsigned_parameter = entry.parameters[15].id;
    let shift_cast_affine_unsigned_parameter = entry.parameters[18].id;
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
    let proof_obligations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. }
            | OperationKind::ExactIntegerAdd { obligation, .. }
            | OperationKind::ExactIntegerSubtract { obligation, .. }
            | OperationKind::ExactIntegerMultiply { obligation, .. }
            | OperationKind::ExactIntegerShiftLeft { obligation, .. }
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
        29,
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { .. }
            ))
            .count(),
        34,
    );
    assert_eq!(shift_obligations.len(), 63);
    assert_eq!(proof_obligations.len(), 100);
    for (index, obligation) in proof_obligations.iter().enumerate() {
        assert!(!proof_obligations[index + 1..].contains(obligation));
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

    for obligation in &proof_obligations {
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
            if proof_obligations.contains(&obligation)
    ));

    let mut stale_definition = decode_module(&semantics).expect("decode mixed-shift module");
    let four = stale_definition
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(4),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
        })
        .and_then(|operation| operation.result.scalar().map(|result| result.id))
        .expect("mixed shifts retain their landed 4u16 count");
    let redirected = stale_definition
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { count, .. } if count == four
            )
        })
        .expect("mixed shifts retain the 4u16 exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected.kind else {
        unreachable!("selected exact-left definition")
    };
    *value = value_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale_definition,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count type");
    let mut stale_cast_chain = decode_module(&semantics).expect("decode mixed-shift module");
    let landed_threes = stale_cast_chain
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            (matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(3),
                }
            ) && operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(i32_type)))
            .then(|| operation.result.scalar().map(|result| result.id))
            .flatten()
        })
        .collect::<Vec<_>>();
    let redirected_cast_chain = stale_cast_chain
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Integer(u16_type))
                && matches!(
                    operation.kind,
                    OperationKind::ExactIntegerShiftLeft { count, .. }
                        if landed_threes.contains(&count)
                )
        })
        .expect("mixed-shift cast retains its outer 3i32 exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected_cast_chain.kind else {
        unreachable!("selected exact-left definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale_cast_chain,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_post_cast = decode_module(&semantics).expect("decode mixed-shift module");
    let post_cast = redirected_post_cast
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { operand, .. }
                    if operand == post_signed_parameter
            )
        })
        .expect("post-cast mixed chain retains its direct cast definition");
    let OperationKind::IntegerExactCast { operand, .. } = &mut post_cast.kind else {
        unreachable!("selected exact-cast definition")
    };
    *operand = signed_wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_post_cast,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine = decode_module(&semantics).expect("decode mixed-shift module");
    let affine_multiply = redirected_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerMultiply { .. })
                && operation.result.scalar_ref().is_some_and(|result| {
                    result.scalar_type
                        == ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
                        )
                })
        })
        .expect("arithmetic-to-shift chain retains its affine definition");
    let OperationKind::ExactIntegerMultiply { left, .. } = &mut affine_multiply.kind else {
        unreachable!("selected exact-multiply definition")
    };
    *left = affine_unsigned_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_shift_affine = decode_module(&semantics).expect("decode mixed-shift module");
    let shift_results = redirected_shift_affine
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. })
                .then(|| operation.result.scalar().map(|result| result.id))
                .flatten()
        })
        .collect::<Vec<_>>();
    let shift_feeding_arithmetic = redirected_shift_affine
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { left, .. }
                if shift_results.contains(&left)
                    && operation.result.scalar_ref().is_some_and(|result| {
                        result.scalar_type
                            == ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type"),
                            )
                    }) =>
            {
                Some(left)
            }
            _ => None,
        })
        .expect("shift-to-arithmetic chain retains its shift definition");
    let redirected_shift = redirected_shift_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == shift_feeding_arithmetic)
        })
        .expect("shift-to-arithmetic chain retains its exact-left result");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut redirected_shift.kind else {
        unreachable!("selected exact-left definition")
    };
    assert_ne!(*value, shift_affine_unsigned_parameter);
    *value = value_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_shift_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_sandwich = decode_module(&semantics).expect("decode mixed-shift module");
    let sandwich_source_right = redirected_sandwich
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight { value, .. }
                if value == sandwich_unsigned_parameter =>
            {
                operation.result.scalar().map(|result| result.id)
            }
            _ => None,
        })
        .expect("sandwich retains its source exact-right definition");
    let sandwich_source_left = redirected_sandwich
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftLeft { value, .. }
                    if value == sandwich_source_right
            )
        })
        .expect("sandwich retains its source exact-left definition");
    let OperationKind::ExactIntegerShiftLeft { value, .. } = &mut sandwich_source_left.kind else {
        unreachable!("selected sandwich exact-left definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_sandwich,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_affine_cast_shift =
        decode_module(&semantics).expect("decode mixed-shift module");
    let affine_source_add = redirected_affine_cast_shift
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerAdd { left, .. }
                    if left == affine_cast_shift_unsigned_parameter
            )
        })
        .expect("affine-to-shift sandwich retains its source exact-add definition");
    let OperationKind::ExactIntegerAdd { left, .. } = &mut affine_source_add.kind else {
        unreachable!("selected affine source exact-add definition")
    };
    *left = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_affine_cast_shift,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
    ));

    let mut redirected_shift_cast_affine =
        decode_module(&semantics).expect("decode mixed-shift module");
    let shift_source_right = redirected_shift_cast_affine
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { value, .. }
                    if value == shift_cast_affine_unsigned_parameter
            )
        })
        .expect("shift-to-affine sandwich retains its source exact-right definition");
    let OperationKind::ExactIntegerShiftRight { value, .. } = &mut shift_source_right.kind else {
        unreachable!("selected shift source exact-right definition")
    };
    *value = wide_parameter;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &redirected_shift_cast_affine,
            &decode_proof_bundle(&proof).expect("decode unchanged mixed-shift proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if proof_obligations.contains(&obligation)
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
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 8).expect("i8 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(0),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16 value"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 value"),
                value: IntegerValue::Unsigned(4),
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
