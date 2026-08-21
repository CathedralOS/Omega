use super::*;

const AFFINE_CAST_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::measure(
        token: Token,
        unsigned: u16,
        signed: i16,
        pre_zero: u16,
        post_zero: u16,
        enabled: bool
    ) -> bool
    requires unsigned <= 65532u16, unsigned <= 32764u16,
        unsigned <= 124u16, unsigned <= 61u16,
        signed <= 32764i16, -16387i16 <= signed, signed <= 16380i16,
        -67i16 <= signed, signed <= 60i16,
        -35i16 <= signed, signed <= 28i16,
        post_zero <= 65534u16, post_zero <= 254u16
    {
        ((((((unsigned + 3u16) * 2u16) as u8) - 1u8) * 2u8) < 255u8)
            && ((((((signed - -3i16) * 2i16) as i8) + 1i8) * 2i8) < 127i8)
            && ((((pre_zero * 0u16) as u8) + 255u8) <= 255u8)
            && (((((post_zero + 1u16) as u8) * 0u8) + 255u8) <= 255u8)
            && enabled
    }
"#;

#[test]
fn affine_cast_affine_sandwich_retains_every_independent_proof_end_to_end() {
    let tokens = Lexer::new(AFFINE_CAST_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize affine-cast-affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse affine-cast-affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve affine-cast-affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type affine-cast-affine source");
    let checked = lower_typed_trees(typed).expect("check affine-cast-affine source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("affine-cast-affine source lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("affine-cast-affine entry");
    let [token] = entry.structural_parameters.as_slice() else {
        panic!("affine-cast-affine entry retains nominal cleanup")
    };
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let obligations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. }
            | OperationKind::ExactIntegerAdd { obligation, .. }
            | OperationKind::ExactIntegerSubtract { obligation, .. }
            | OperationKind::ExactIntegerMultiply { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(obligations.len(), 17);
    for (index, obligation) in obligations.iter().enumerate() {
        assert!(!obligations[index + 1..].contains(obligation));
        let operation = operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::IntegerExactCast {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerAdd {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerSubtract {
                        obligation: candidate,
                        ..
                    } | OperationKind::ExactIntegerMultiply {
                        obligation: candidate,
                        ..
                    } if candidate == *obligation
                )
            })
            .expect("every sandwich obligation retains its operation");
        assert_eq!(
            TerminalFuelSchedule::CURRENT.operation_units(&operation.kind),
            1
        );
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
    .expect("affine-cast-affine proofs verify independently");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("affine-cast-affine entry has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("affine-cast-affine fuel recomputes");
    drop(verified);

    let semantics = encode_module(&lowered.semantic_module).expect("encode sandwich module");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode sandwich proof");
    assert_eq!(
        decode_module(&semantics).expect("decode sandwich module"),
        lowered.semantic_module,
    );
    assert_eq!(
        decode_proof_bundle(&proof).expect("decode sandwich proof"),
        lowered.proof_bundle,
    );
    for obligation in &obligations {
        let mut missing = decode_proof_bundle(&proof).expect("decode sandwich proof");
        missing
            .evidence
            .retain(|evidence| evidence.obligation != *obligation);
        assert!(matches!(
            psi_terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode unchanged sandwich module"),
                &missing,
                &AdmissionProfile::default(),
            ),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing_obligation))
                if missing_obligation == *obligation
        ));
    }

    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let produced_by_two = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply { right, .. }
                if operations.iter().any(|candidate| {
                    candidate.result.scalar_ref().map(|result| result.id) == Some(right)
                        && matches!(
                            candidate.kind,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(2),
                            }
                        )
                        && candidate.result.scalar_ref().is_some_and(|result| {
                            result.scalar_type == ScalarType::Integer(u16_type)
                        })
                }) =>
            {
                operation.result.scalar_ref().map(|result| result.id)
            }
            _ => None,
        })
        .expect("unsigned source affine chain retains its product");
    let mut stale = decode_module(&semantics).expect("decode sandwich module");
    let cast = stale
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { operand, .. } if operand == produced_by_two
            )
        })
        .expect("sandwich retains the computed exact-cast definition");
    let OperationKind::IntegerExactCast { operand, .. } = &mut cast.kind else {
        unreachable!("selected exact cast")
    };
    *operand = entry.parameters[3].id;
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &stale,
            &decode_proof_bundle(&proof).expect("decode unchanged sandwich proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence { obligation, .. })
            if obligations.contains(&obligation)
    ));

    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let scalar_arguments = |enabled| {
        vec![
            TerminalScalarValue::Integer {
                scalar_type: u16_type,
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 16).expect("i16"),
                value: IntegerValue::Signed(2),
            },
            TerminalScalarValue::Integer {
                scalar_type: u16_type,
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Integer {
                scalar_type: u16_type,
                value: IntegerValue::Unsigned(4),
            },
            TerminalScalarValue::Boolean(enabled),
        ]
    };
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
        .expect("affine-cast-affine artifact interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(enabled)),
        );
        assert!(measured.usage().total_units() <= fixed.ceiling_units());
        assert!(measured.effects().is_empty());
    }
}
