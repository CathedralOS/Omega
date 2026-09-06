//! Mixed Unit crash arithmetic uses exact runtime requirement evidence.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

const SOURCE: &str = r#"
    data Metrics { current: u64; }
    boundary trait Sink { machine record(divisor: u64, limit: u64); }
    data Helper {}
    machine Helper::consume(divisor: u64, metrics: Metrics, limit: u64)
    requires 1u64 <= divisor
    crashes Abort metrics.current / divisor <= limit
    { Sink::record(divisor, limit); }
    data Main {}
    machine Main::main(metrics: Metrics, limit: u64, divisor: u64)
    requires 1u64 <= divisor
    crashes Abort metrics.current / divisor <= limit
    { Helper::consume(divisor, metrics, limit); }
"#;

fn roundtrip(checked: &checked_trees::CheckedTrees) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("mixed crash arithmetic retains its scalar runtime requirements");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification of mixed runtime requirements");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    let artifact = terminal_production::produce_terminal_artifact(checked, "Main::main")
        .expect("mixed runtime requirements publish");
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        module
    );
    lowered
}

#[test]
fn scalar_divisor_requirement_survives_reordered_mixed_unit_call() {
    roundtrip(&checked(SOURCE));
    let free = SOURCE
        .replace("Helper::consume", "consume")
        .replace("{ Sink::record(divisor, limit); }", "{}");
    roundtrip(&checked(&free));
}

#[test]
fn computed_scalar_divisor_carries_its_requirement_into_the_mixed_unit_call() {
    for computation in [
        "divisor + 0u64",
        "divisor - 0u64",
        "divisor * 1u64",
        "divisor + 1u64",
    ] {
        let source = SOURCE.replace(
            "crashes Abort metrics.current / divisor <= limit\n    { Helper::consume(divisor, metrics, limit); }",
            &format!("crashes Abort\n    {{ Helper::consume({computation}, metrics, limit); }}"),
        );
        let source = if computation == "divisor + 1u64" {
            source.replace(
                "requires 1u64 <= divisor\n    crashes Abort\n",
                "requires 1u64 <= divisor, divisor <= 100u64\n    crashes Abort\n",
            )
        } else {
            source
        };
        let lowered = roundtrip(&checked(&source));
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let (arithmetic, arithmetic_obligation) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                terminal_psi::OperationKind::ExactIntegerAdd { obligation, .. }
                | terminal_psi::OperationKind::ExactIntegerSubtract { obligation, .. }
                | terminal_psi::OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    Some((operation, obligation))
                }
                _ => None,
            })
            .expect("authored operand remains an evaluated Exact operation");
        let terminal_psi::OperationResult::Scalar(result) = arithmetic.result else {
            panic!("arithmetic publishes the argument value");
        };
        let (arguments, requirements) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit {
                    arguments,
                    requirement_obligations,
                    ..
                } => Some((arguments, requirement_obligations)),
                _ => None,
            })
            .unwrap();
        assert_eq!(arguments[0], result.id, "{computation}");
        assert_eq!(requirements.len(), 1);
        assert_ne!(requirements[0], arithmetic_obligation);
        for obligation in [arithmetic_obligation, requirements[0]] {
            assert!(
                lowered
                    .proof_bundle
                    .evidence
                    .iter()
                    .any(|evidence| evidence.obligation == obligation
                        && matches!(
                            evidence.route,
                            proof_admission::EvidenceRoute::CertificateDerived(_)
                        ))
            );
            let mut missing = lowered.proof_bundle.clone();
            missing
                .evidence
                .retain(|evidence| evidence.obligation != obligation);
            assert!(
                matches!(terminal_verifier::verify_module(&lowered.semantic_module, &missing, &proof_admission::AdmissionProfile::default()),
                Err(terminal_verifier::VerificationError::MissingEvidence(rejected)) if rejected == obligation),
                "{computation}: {obligation:?}"
            );
        }
        let mut missing_definition = lowered.semantic_module.clone();
        let changed_root = missing_definition
            .machines
            .iter_mut()
            .find(|machine| machine.id == root.id)
            .unwrap();
        for block in &mut changed_root.blocks {
            block
                .operations
                .retain(|operation| operation.id != arithmetic.id);
        }
        assert!(
            terminal_verifier::validate_module(&missing_definition).is_err(),
            "undefined computed argument: {computation}"
        );
    }
}

#[test]
fn computed_argument_keeps_entry_requirements_without_a_crash_ceiling() {
    let source = SOURCE
        .replace("crashes Abort metrics.current / divisor <= limit", "")
        .replace(
            "Helper::consume(divisor, metrics, limit)",
            "Helper::consume(divisor + 0u64, metrics, limit)",
        );
    let lowered = roundtrip(&checked(&source));
    assert!(lowered.semantic_module.machines.iter().all(|machine| {
        machine.contract.crash_routes.is_empty() && machine.contract.requires.len() == 1
    }));
}

#[test]
fn reflexive_call_requirement_cannot_prove_unbounded_argument_addition_safe() {
    for primitive in ["u64", "i64"] {
        let source = format!(
            r#"
            data Metrics {{ current: {primitive}; }}
            boundary trait Sink {{ machine record(value: {primitive}); }}
            data Helper {{}}
            machine Helper::consume(value: {primitive}, metrics: Metrics)
            requires value == value
            {{ Sink::record(value); }}
            data Main {{}}
            machine Main::main(metrics: Metrics, input: {primitive})
            {{ Helper::consume(input + 1{primitive}, metrics); }}
        "#
        );
        let checked = checked(&source);
        let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
            .expect_err("a reflexive call requirement cannot establish Exact addition safety");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_)
            ),
            "{primitive}: {error:?}"
        );
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "Main::main").is_err(),
            "unsafe computed arithmetic must not publish: {primitive}"
        );
    }
}

#[test]
fn literal_first_scalar_equality_requirement_keeps_canonical_value_identity() {
    let source = SOURCE.replace(
        "requires 1u64 <= divisor",
        "requires 1u64 <= divisor, 1u64 == divisor",
    );
    roundtrip(&checked(&source));
}

#[test]
fn reversed_scalar_actuals_prove_the_symmetric_callee_requirement() {
    let source = SOURCE.replace(
        "requires 1u64 <= divisor",
        "requires 1u64 <= divisor, divisor == limit",
    );
    let lowered = roundtrip(&checked(&source));
    let obligation = call_requirement(&lowered, |requirement| {
        matches!(requirement, semantic_vocabulary::Proposition::Equal(_, _))
    });
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .unwrap();
    let proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("symmetry uses checked certificate evidence");
    };
    let proof_admission::ProofRule::EqualitySymmetry { equality } = &certificate.proof.rule else {
        panic!("reversed actuals require the existing equality symmetry rule");
    };
    let proof_admission::ProofRule::Assumption { index } = equality.rule else {
        panic!("symmetry child retains the caller equality premise");
    };
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(equality.conclusion, root.contract.requires[index]);
    let semantic_vocabulary::Proposition::Equal(left, right) = &equality.conclusion else {
        panic!("equality child");
    };
    assert_eq!(
        certificate.proof.conclusion,
        semantic_vocabulary::Proposition::Equal(right.clone(), left.clone())
    );
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
    assert_eq!(
        reconstructed
            .iter()
            .find(|site| site.obligation.id == obligation)
            .unwrap()
            .obligation
            .proposition,
        certificate.proof.conclusion
    );
    let wrong_index = root
        .contract
        .requires
        .iter()
        .position(|requirement| requirement != &equality.conclusion)
        .unwrap();
    for mutation in 0..3 {
        let mut changed = lowered.proof_bundle.clone();
        let proof_admission::EvidenceRoute::CertificateDerived(certificate) = &mut changed
            .evidence
            .iter_mut()
            .find(|evidence| evidence.obligation == obligation)
            .unwrap()
            .route
        else {
            unreachable!();
        };
        let proof_admission::ProofRule::EqualitySymmetry { equality } = &mut certificate.proof.rule
        else {
            unreachable!();
        };
        let expected = match mutation {
            0 => {
                equality.rule = proof_admission::ProofRule::Assumption { index: usize::MAX };
                proof_admission::ProofError::UnknownAssumption(usize::MAX)
            }
            1 => {
                equality.rule = proof_admission::ProofRule::Assumption { index: wrong_index };
                proof_admission::ProofError::AssumptionConclusionMismatch(wrong_index)
            }
            2 => {
                certificate.proof = (**equality).clone();
                proof_admission::ProofError::CertificateConclusionMismatch
            }
            _ => unreachable!(),
        };
        assert!(
            matches!(terminal_verifier::verify_module(&lowered.semantic_module, &changed, &proof_admission::AdmissionProfile::default()),
            Err(terminal_verifier::VerificationError::RejectedEvidence {
                obligation: rejected, error: proof_admission::EvidenceError::Certificate(error)
            }) if rejected == obligation && error == expected),
            "symmetry evidence mutation={mutation}"
        );
    }
}

fn call_requirement(
    lowered: &lowered_psi::LoweredPsi,
    select: impl Fn(&semantic_vocabulary::Proposition) -> bool,
) -> semantic_vocabulary::ObligationId {
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let (target, obligations) = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                callee,
                requirement_obligations,
                ..
            } => Some((*callee, requirement_obligations)),
            _ => None,
        })
        .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == target)
        .unwrap();
    let position = callee.contract.requires.iter().position(select).unwrap();
    obligations[position]
}

#[test]
fn stronger_caller_scalar_bound_proves_the_weaker_callee_requirement() {
    let source = SOURCE.replace("requires 1u64 <= divisor", "requires 1u64 <= divisor, 1u64 <= limit")
        .replace("requires 1u64 <= divisor, 1u64 <= limit\n    crashes Abort metrics.current / divisor <= limit\n    { Helper",
            "requires 1u64 <= divisor, 2u64 <= limit\n    crashes Abort metrics.current / divisor <= limit\n    { Helper");
    let lowered = roundtrip(&checked(&source));
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let target = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            terminal_psi::OperationKind::CallUnit { callee, .. } => Some(callee),
            _ => None,
        })
        .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == target)
        .unwrap();
    let limit = semantic_vocabulary::ScalarTerm::value(
        callee.parameters[1].id,
        callee.parameters[1].scalar_type,
    );
    let obligation = call_requirement(&lowered, |requirement| {
        matches!(requirement,
        semantic_vocabulary::Proposition::LessOrEqual(_, subject) if subject == &limit)
    });
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .unwrap();
    let proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("stronger caller bound needs checked proof");
    };
    let proof_admission::ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = &certificate.proof.rule
    else {
        panic!("existing integer-order transitivity combines 1<=2 with 2<=limit");
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        proof_admission::ProofRule::Primitive(
            proof_admission::PrimitiveJudgment::ClosedIntegerRelation
        )
    ));
    let proof_admission::ProofRule::Assumption { index } = middle_less_or_equal_right.rule else {
        panic!("stronger bound child names caller premise");
    };
    assert_eq!(
        middle_less_or_equal_right.conclusion,
        root.contract.requires[index]
    );
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
    assert_eq!(
        reconstructed
            .iter()
            .find(|site| site.obligation.id == obligation)
            .unwrap()
            .obligation
            .proposition,
        certificate.proof.conclusion
    );
}

#[test]
fn scalar_call_requirements_reject_weaker_bounds_and_disjoint_subjects() {
    for (callee_requirement, caller_requirement, disjoint_subject) in [
        ("2u64 <= limit", "1u64 <= limit", false),
        ("1u64 <= limit", "2u64 <= spare", true),
    ] {
        let source = SOURCE.replacen(
            "requires 1u64 <= divisor",
            &format!("requires 1u64 <= divisor, {callee_requirement}"),
            1,
        );
        let source = source.replace(
            "requires 1u64 <= divisor\n    crashes Abort metrics.current / divisor <= limit\n    { Helper",
            &format!("requires 1u64 <= divisor, {caller_requirement}\n    crashes Abort metrics.current / divisor <= limit\n    {{ Helper"),
        );
        let source = if disjoint_subject {
            source.replace(
                "Main::main(metrics: Metrics, limit: u64, divisor: u64)",
                "Main::main(metrics: Metrics, limit: u64, divisor: u64, spare: u64)",
            )
        } else {
            source
        };
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect_err("caller bounds must imply the requirement on the same scalar subject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("requires")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn mixed_call_requirements_keep_callee_order_and_equal_actual_slots() {
    for equal_actuals in [false, true] {
        let source = SOURCE.replace(
            "requires 1u64 <= divisor\n    crashes Abort metrics.current / divisor <= limit\n    { Sink",
            "requires 1u64 <= divisor, 1u64 <= limit\n    crashes Abort metrics.current / divisor <= limit\n    { Sink",
        ).replace(
            "requires 1u64 <= divisor\n    crashes Abort metrics.current / divisor <= limit\n    { Helper",
            "requires 1u64 <= limit, 1u64 <= divisor\n    crashes Abort metrics.current / divisor <= limit\n    { Helper",
        );
        let source = if equal_actuals {
            source.replace("requires 1u64 <= limit, 1u64 <= divisor", "requires 1u64 <= divisor")
                .replace("crashes Abort metrics.current / divisor <= limit\n    { Helper::consume(divisor, metrics, limit); }",
                    "crashes Abort metrics.current / divisor <= divisor\n    { Helper::consume(divisor, metrics, divisor); }")
        } else {
            source
        };
        let lowered = roundtrip(&checked(&source));
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let (target, arguments, obligations) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit {
                    callee,
                    arguments,
                    requirement_obligations,
                    ..
                } => Some((*callee, arguments, requirement_obligations)),
                _ => None,
            })
            .unwrap();
        let callee = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == target)
            .unwrap();
        assert_eq!(callee.contract.requires.len(), 2);
        for (requirement, parameter) in callee.contract.requires.iter().zip(&callee.parameters) {
            let semantic_vocabulary::Proposition::LessOrEqual(_, subject) = requirement else {
                panic!("ordered scalar lower bound");
            };
            assert_eq!(
                subject,
                &semantic_vocabulary::ScalarTerm::value(parameter.id, parameter.scalar_type)
            );
        }
        assert_eq!(
            obligations.len(),
            2,
            "distinct authored precondition slots are not deduplicated"
        );
        assert_ne!(obligations[0], obligations[1]);
        assert_eq!(
            arguments[0], root.parameters[1].id,
            "divisor is the second caller scalar"
        );
        assert_eq!(arguments[1], root.parameters[usize::from(equal_actuals)].id);
        assert_eq!(
            root.contract.requires.len(),
            if equal_actuals { 1 } else { 2 }
        );
    }
}

#[test]
fn structural_divisor_keeps_whole_root_requirements_and_rejects_partial_cleanup() {
    let source = r#"
        data Metrics { divisor: u64; }
        data Envelope { metrics: Metrics; spare: Metrics; }
        data Helper {}
        boundary trait Sink { machine record(numerator: u64, limit: u64); }
        machine Helper::consume(numerator: u64, metrics: Metrics, limit: u64)
        requires 1u64 <= metrics.divisor
        crashes Abort numerator / metrics.divisor <= limit
        { Sink::record(numerator, limit); }
        data Main {}
        machine Main::main(limit: u64, envelope: Envelope, numerator: u64)
        requires 1u64 <= envelope.metrics.divisor
        crashes Abort numerator / envelope.metrics.divisor <= limit
        { Helper::consume(numerator, envelope.metrics, limit); }
    "#;
    for projected in [false, true] {
        let source = if projected {
            source.to_owned()
        } else {
            source
                .replace("envelope: Envelope", "metrics: Metrics")
                .replace("envelope.metrics", "metrics")
        };
        let checked = checked(&source);
        if projected {
            // Projected owned moves need the separate partial-affine cleanup
            // consumer, whose current source shape excludes mixed scalar
            // inputs and an effectful callee. Do not erase the live spare.
            assert!(matches!(
                checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main"),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "attached Unit closure is missing a checked transitive machine plan"
                ))
            ));
        } else {
            roundtrip(&checked);
        }
    }
}

#[test]
fn retained_call_certificate_cannot_prove_a_changed_scalar_requirement() {
    let source = SOURCE.replace(
        "requires 1u64 <= divisor",
        "requires 1u64 <= divisor, 1u64 <= limit",
    );
    let lowered = roundtrip(&checked(&source));
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let (target, obligations) = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                callee,
                requirement_obligations,
                ..
            } => Some((*callee, requirement_obligations)),
            _ => None,
        })
        .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == target)
        .unwrap();
    let limit = semantic_vocabulary::ScalarTerm::value(
        callee.parameters[1].id,
        callee.parameters[1].scalar_type,
    );
    let slot = callee
        .contract
        .requires
        .iter()
        .position(|requirement| {
            matches!(requirement,
        semantic_vocabulary::Proposition::LessOrEqual(_, subject) if subject == &limit)
        })
        .unwrap();
    let obligation = obligations[slot];
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .unwrap();
    assert!(matches!(
        evidence.route,
        proof_admission::EvidenceRoute::CertificateDerived(_)
    ));
    let mut changed = lowered.semantic_module.clone();
    let callee = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == target)
        .unwrap();
    let semantic_vocabulary::Proposition::LessOrEqual(bound, _) =
        &mut callee.contract.requires[slot]
    else {
        panic!("callee scalar lower bound");
    };
    *bound = semantic_vocabulary::ScalarTerm::integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
        semantic_vocabulary::IntegerValue::Unsigned(2),
    )
    .unwrap();
    terminal_verifier::validate_module(&changed).expect(
        "divisor safety is unchanged; stronger independent limit bound remains well formed",
    );
    assert!(
        matches!(terminal_verifier::verify_module(&changed, &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default()),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            error: proof_admission::EvidenceError::Certificate(proof_admission::ProofError::CertificateConclusionMismatch),
        }) if rejected == obligation),
        "the old call certificate must not prove the stronger reconstructed precondition"
    );
}

const SHIFT_SOURCE: &str = r#"
    data Bits { value: u8; }
    data Helper {}
    boundary trait Sink { machine record(count: i16); }
    machine Helper::consume(count: i16, bits: Bits, limit: u8)
    requires 0i16 <= count, count < 8i16, bits.value <= 1u8
    crashes Abort bits.value << count == limit
    { Sink::record(count); }
    data Main {}
    machine Main::main(bits: Bits, limit: u8, count: i16)
    requires bits.value <= 1u8, count < 8i16, 0i16 <= count
    crashes Abort bits.value << count == limit
    { Helper::consume(count, bits, limit); }
"#;

#[test]
fn exact_shift_and_signed_division_retain_mixed_totality_requirements() {
    roundtrip(&checked(SHIFT_SOURCE));
    roundtrip(&checked(&SOURCE.replace("u64", "i64")));
}

#[test]
fn mixed_arithmetic_rejects_missing_nonzero_signed_overflow_and_count_bounds() {
    let sources = [
        (
            SOURCE.replace("requires 1u64 <= divisor", ""),
            "divisor must be proven nonzero",
        ),
        (
            SOURCE
                .replace("u64", "i64")
                .replace("1i64 <= divisor", "divisor <= -1i64"),
            "MIN / -1",
        ),
        (
            SHIFT_SOURCE.replace("count < 8i16", "count < 9i16"),
            "shift count",
        ),
        (
            SHIFT_SOURCE.replace("0i16 <= count", "-1i16 <= count"),
            "shift count",
        ),
    ];
    for (source, expected) in sources {
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect_err("mixed specification arithmetic still needs complete totality facts");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn verifier_rejects_redirected_scalar_arguments_and_requirement_evidence() {
    let lowered = roundtrip(&checked(SOURCE));
    let root_index = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let root = &lowered.semantic_module.machines[root_index];
    let (block, operation, target) = root
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.operations
                .iter()
                .enumerate()
                .find_map(|(operation, item)| match item.kind {
                    terminal_psi::OperationKind::CallUnit { callee, .. } => {
                        Some((block, operation, callee))
                    }
                    _ => None,
                })
        })
        .unwrap();
    let operation_id = root.blocks[block].operations[operation].id;
    for mutation in 0..3 {
        let mut changed = lowered.semantic_module.clone();
        match mutation {
            0 => {
                let terminal_psi::OperationKind::CallUnit { arguments, .. } =
                    &mut changed.machines[root_index].blocks[block].operations[operation].kind
                else {
                    unreachable!();
                };
                arguments.swap(0, 1);
            }
            1 => changed.machines[root_index].contract.requires.clear(),
            2 => {
                let semantic_vocabulary::Proposition::LessOrEqual(_, right) =
                    &mut changed.machines[root_index].contract.requires[0]
                else {
                    panic!("exact scalar nonzero bound");
                };
                *right = semantic_vocabulary::ScalarTerm::value(
                    root.parameters[0].id,
                    root.parameters[0].scalar_type,
                );
            }
            _ => unreachable!(),
        }
        let expected = if mutation == 0 {
            terminal_verifier::ModuleError::CallCrashContinuationsMismatch {
                operation: operation_id,
                callee: target,
            }
        } else {
            terminal_verifier::ModuleError::UnsafeStructuralCrashExactDivisor {
                machine: root.id,
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            }
        };
        assert_eq!(
            terminal_verifier::validate_module(&changed).unwrap_err(),
            expected,
            "mutation={mutation}"
        );
    }
}
