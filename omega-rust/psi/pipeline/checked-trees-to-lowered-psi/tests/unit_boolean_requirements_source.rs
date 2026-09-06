//! Boolean preconditions retain their actual scalar identity across Unit calls.

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn roundtrip(source: &str) -> lowered_psi::LoweredPsi {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("Boolean requirements survive mixed Unit lowering");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent Boolean call requirement verification");
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::main")
        .expect("Boolean requirement source publishes Terminal");
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        module
    );
    lowered
}

const SOURCE: &str = r#"
    data Metrics { current: u64; }
    boundary trait Sink { machine record(flag: bool); }
    data Helper {}
    machine Helper::consume(flag: bool, metrics: Metrics)
    requires flag
    { Sink::record(flag); }
    data Main {}
    machine Main::main(metrics: Metrics, flag: bool)
    requires flag
    { Helper::consume(flag, metrics); }
"#;

#[test]
fn boolean_parameter_requirement_survives_mixed_unit_call() {
    let lowered = roundtrip(SOURCE);
    assert_call_requirement_certificates(&lowered);
}

fn assert_call_requirement_certificates(lowered: &lowered_psi::LoweredPsi) {
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let (operation, callee, requirements) = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                callee,
                requirement_obligations,
                ..
            } => Some((operation.id, *callee, requirement_obligations)),
            _ => None,
        })
        .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == callee)
        .unwrap();
    assert!(
        !callee.contract.requires.is_empty(),
        "authored Boolean requirements are not discarded"
    );
    assert_eq!(requirements.len(), callee.contract.requires.len());
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
    for (position, obligation) in requirements.iter().enumerate() {
        let site = reconstructed
            .iter()
            .find(|site| site.obligation.id == *obligation)
            .unwrap();
        assert_eq!(
            site.owner,
            terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
                machine: root.id,
                operation,
                requirement_position: position as u32,
            }
        );
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .unwrap();
        let proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
        else {
            panic!("Boolean call requirement uses independently checked evidence");
        };
        assert_eq!(certificate.proof.conclusion, site.obligation.proposition);
    }
}

#[test]
fn boolean_requirement_formulas_preserve_scalar_and_structural_subjects() {
    for requirement in [
        "metrics.enabled",
        "!flag",
        "!metrics.enabled",
        "flag && metrics.enabled",
        "flag || metrics.enabled",
        "flag == metrics.enabled",
        "!(flag && metrics.enabled)",
        "!(flag || metrics.enabled)",
        "(flag || metrics.enabled) == !flag",
        "true",
        "!false",
    ] {
        let source = SOURCE
            .replace("current: u64", "enabled: bool")
            .replace("requires flag", &format!("requires {requirement}"));
        let lowered = roundtrip(&source);
        assert_call_requirement_certificates(&lowered);
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let reconstructed =
            terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
        for site in reconstructed.iter().filter(|site| matches!(site.owner,
            terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires { machine, .. } if machine == root.id)) {
            assert!(root.contract.requires.contains(&site.obligation.proposition),
                "forwarded requirement rejoins the caller's exact scalar/place namespace: {requirement}: {:?}", site.obligation.proposition);
        }
    }
}

#[test]
fn boolean_call_requirements_preserve_reordered_and_shared_actuals() {
    for (requirement, shared) in [
        ("flag && other", false),
        ("flag && other", true),
        ("flag || other", false),
        ("flag == other", false),
    ] {
        let source = SOURCE
            .replace(
                "consume(flag: bool, metrics: Metrics)",
                "consume(flag: bool, metrics: Metrics, other: bool)",
            )
            .replace(
                "main(metrics: Metrics, flag: bool)",
                "main(other: bool, metrics: Metrics, flag: bool)",
            )
            .replace("requires flag", &format!("requires {requirement}"))
            .replace(
                "consume(flag, metrics)",
                if shared {
                    "consume(flag, metrics, flag)"
                } else {
                    "consume(flag, metrics, other)"
                },
            );
        let lowered = roundtrip(&source);
        assert_call_requirement_certificates(&lowered);
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let arguments = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit { arguments, .. } => Some(arguments),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            arguments.as_slice(),
            &[
                root.parameters[1].id,
                root.parameters[usize::from(shared)].id
            ]
        );
    }
}

#[test]
fn nested_disjunction_requirements_preserve_reordered_actuals() {
    let source = r#"
        data Metrics { current: u64; }
        boundary trait Sink { machine record(flag: bool); }
        data Helper {}
        machine Helper::consume(right: bool, metrics: Metrics, left: bool, gate: bool)
        requires gate && (left || right)
        { Sink::record(gate); }
        data Main {}
        machine Main::main(left: bool, metrics: Metrics, right: bool, gate: bool)
        requires gate && (left || right)
        { Helper::consume(right, metrics, left, gate); }
    "#;
    assert_call_requirement_certificates(&roundtrip(source));
}

#[test]
fn literal_true_actual_proves_boolean_requirement_without_caller_assumptions() {
    let source = SOURCE.replace(
        "requires flag\n    { Helper::consume(flag, metrics); }",
        "{ Helper::consume(true, metrics); }",
    );
    let lowered = roundtrip(&source);
    assert_call_requirement_certificates(&lowered);
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert!(root.contract.requires.is_empty());
}

#[test]
fn computed_boolean_actual_proves_requirement_from_operation_meaning() {
    computed_boolean_actual("!false", "");
}

fn computed_boolean_actual(actual: &str, caller_requirement: &str) -> lowered_psi::LoweredPsi {
    let source = SOURCE.replace(
        "requires flag\n    { Helper::consume(flag, metrics); }",
        &format!("{caller_requirement}\n    {{ Helper::consume({actual}, metrics); }}"),
    );
    let lowered = roundtrip(&source);
    assert_call_requirement_certificates(&lowered);
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(
        root.contract.requires.is_empty(),
        caller_requirement.is_empty()
    );
    let argument = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit { arguments, .. } => Some(arguments[0]),
            _ => None,
        })
        .unwrap();
    let definition = root.blocks.iter().flat_map(|block| &block.operations).find(|operation|
        matches!(operation.result, terminal_psi::OperationResult::Scalar(value) if value.id == argument)).unwrap();
    assert!(
        matches!(
            definition.kind,
            terminal_psi::OperationKind::BooleanNot { .. }
                | terminal_psi::OperationKind::BooleanEqual { .. }
                | terminal_psi::OperationKind::IntegerEqual { .. }
                | terminal_psi::OperationKind::IntegerLessThan { .. }
                | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
        ),
        "{actual}: call retains the evaluated operation result, not a substituted literal: {:?}",
        definition.kind
    );
    lowered
}

#[test]
fn computed_boolean_equality_actual_retains_its_evaluated_result() {
    for actual in ["true == true", "false == false"] {
        computed_boolean_actual(actual, "");
    }
}

#[test]
fn computed_integer_comparison_actuals_retain_landed_literal_meaning() {
    for primitive in ["u64", "i64"] {
        for (left, operator, right) in [(1, "<", 2), (2, "<=", 2), (2, "==", 2)] {
            computed_boolean_actual(
                &format!("{left}{primitive} {operator} {right}{primitive}"),
                "",
            );
        }
    }
}

#[test]
fn computed_nested_boolean_negations_remain_separate_operations() {
    let lowered = computed_boolean_actual("!!!false", "");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(
        root.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanNot { .. }
            ))
            .count(),
        3
    );
}

#[test]
fn computed_symbolic_negation_uses_the_exact_caller_requirement() {
    computed_boolean_actual("!flag", "requires !flag");
}

#[test]
fn computed_boolean_requirement_rejects_changed_operand_operation_and_missing_evidence() {
    let lowered = computed_boolean_actual("!false", "");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let operation_id = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanNot { .. }
            )
        })
        .unwrap()
        .id;
    let obligation = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                requirement_obligations,
                ..
            } => Some(requirement_obligations[0]),
            _ => None,
        })
        .unwrap();
    for mutation in 0..3 {
        let mut module = lowered.semantic_module.clone();
        let mut proof = lowered.proof_bundle.clone();
        if mutation == 2 {
            proof
                .evidence
                .retain(|evidence| evidence.obligation != obligation);
        } else {
            let operation = module
                .machines
                .iter_mut()
                .find(|machine| machine.id == root.id)
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.id == operation_id)
                .unwrap();
            let terminal_psi::OperationKind::BooleanNot { operand } = operation.kind else {
                unreachable!();
            };
            operation.kind = if mutation == 0 {
                terminal_psi::OperationKind::BooleanNot {
                    operand: root.parameters[0].id,
                }
            } else {
                terminal_psi::OperationKind::BooleanEqual {
                    left: operand,
                    right: root.parameters[0].id,
                }
            };
            terminal_verifier::validate_module(&module)
                .expect("changed Boolean expression is typed but not proven true");
        }
        let error = terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_err();
        if mutation == 2 {
            assert_eq!(
                error,
                terminal_verifier::VerificationError::MissingEvidence(obligation)
            );
        } else {
            assert!(
                matches!(error, terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected, error: proof_admission::EvidenceError::Certificate(_),
            } if rejected == obligation),
                "mutation={mutation}: {error:?}"
            );
        }
    }
}

#[test]
fn structural_boolean_requirement_rejects_same_shaped_unrelated_actual() {
    let source = SOURCE
        .replace("current: u64", "enabled: bool")
        .replace("requires flag", "requires metrics.enabled")
        .replace(
            "consume(flag: bool, metrics: Metrics)",
            "consume(flag: bool, metrics: Metrics, unrelated: Metrics)",
        )
        .replace(
            "main(metrics: Metrics, flag: bool)",
            "main(metrics: Metrics, flag: bool, unrelated: Metrics)",
        )
        .replace(
            "consume(flag, metrics)",
            "consume(flag, metrics, unrelated)",
        );
    let lowered = roundtrip(&source);
    assert_call_requirement_certificates(&lowered);
    let mut changed = lowered.semantic_module.clone();
    let entry = changed.entry;
    let root = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap();
    let (structural_arguments, requirements) = root
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::CallUnit {
                structural_arguments,
                requirement_obligations,
                ..
            } => Some((structural_arguments, requirement_obligations)),
            _ => None,
        })
        .unwrap();
    assert_eq!(structural_arguments.len(), 2);
    let obligation = requirements[0];
    // Both moves remain present exactly once. Only the actual root that must
    // satisfy the callee's field requirement changes.
    structural_arguments.swap(0, 1);
    terminal_verifier::validate_module(&changed)
        .expect("same-shaped moved roots preserve structural custody and typing");
    let error = terminal_verifier::verify_module(
        &changed,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, terminal_verifier::VerificationError::RejectedEvidence {
        obligation: rejected,
        error: proof_admission::EvidenceError::Certificate(proof_admission::ProofError::CertificateConclusionMismatch),
    } if rejected == obligation),
        "unrelated structural actual: {error:?}"
    );
}

#[test]
fn boolean_call_requirements_reject_changed_actuals_and_missing_or_mismatched_certificates() {
    let lowered = roundtrip(&SOURCE.replace(
        "main(metrics: Metrics, flag: bool)",
        "main(metrics: Metrics, flag: bool, other: bool)",
    ));
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let (operation_id, obligation) = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                requirement_obligations,
                ..
            } => Some((operation.id, requirement_obligations[0])),
            _ => None,
        })
        .unwrap();
    for mutation in 0..3 {
        let mut module = lowered.semantic_module.clone();
        let mut proof = lowered.proof_bundle.clone();
        match mutation {
            0 => {
                let operation = module
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == root.id)
                    .unwrap()
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| operation.id == operation_id)
                    .unwrap();
                let terminal_psi::OperationKind::CallUnit { arguments, .. } = &mut operation.kind
                else {
                    unreachable!();
                };
                arguments[0] = root.parameters[1].id;
                terminal_verifier::validate_module(&module)
                    .expect("wrong actual is well-typed but lacks the required Boolean fact");
            }
            1 => proof
                .evidence
                .retain(|evidence| evidence.obligation != obligation),
            2 => {
                let evidence = proof
                    .evidence
                    .iter_mut()
                    .find(|evidence| evidence.obligation == obligation)
                    .unwrap();
                let proof_admission::EvidenceRoute::CertificateDerived(certificate) =
                    &mut evidence.route
                else {
                    unreachable!();
                };
                certificate.proof.conclusion = semantic_vocabulary::Proposition::Truth;
            }
            _ => unreachable!(),
        }
        let error = terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_err();
        if mutation == 1 {
            assert_eq!(
                error,
                terminal_verifier::VerificationError::MissingEvidence(obligation)
            );
        } else if mutation == 0 {
            assert!(
                matches!(error, terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected, error: proof_admission::EvidenceError::Certificate(proof_admission::ProofError::CertificateConclusionMismatch),
            } if rejected == obligation),
                "mutation={mutation}: {error:?}"
            );
        } else {
            assert!(
                matches!(error, terminal_verifier::VerificationError::RejectedEvidence {
                    obligation: rejected,
                    error: proof_admission::EvidenceError::Certificate(
                        proof_admission::ProofError::AssumptionConclusionMismatch(0)
                    ),
                } if rejected == obligation),
                "a forged assumption conclusion is rejected before accepting the root: {error:?}"
            );
        }
    }
}
