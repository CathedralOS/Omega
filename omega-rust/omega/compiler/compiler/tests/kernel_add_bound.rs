//! The existing runtime-complement addition customer retains a kernel-derived
//! bounds, with fixed arithmetic assumptions visible in their exact closures.

use std::path::Path;

use compiler::{CheckedCompileRequest, compile_to_checked};
use proof_admission::{
    Budget, Term, certificate_assumption_closure,
    verify_bounded_certificate_with_machine_parameters, verify_mathematical_certificate,
};
use semantic_vocabulary::{Proposition, ScalarTerm};
use terminal_psi::{EvidenceRoute, ProofNode, ProofRule};

fn correlated_add(proof: &ProofNode, lower: bool) -> Option<&ProofNode> {
    match &proof.rule {
        ProofRule::IntegerAffineBound { witness, .. }
            if matches!(witness.target, ScalarTerm::ExactIntegerAdd { .. })
                && match &proof.conclusion {
                    Proposition::IntegerMathLessOrEqual(left, right) => matches!(
                        if lower { right } else { left },
                        semantic_vocabulary::IntegerMathTerm::Add(_, _)
                    ),
                    _ => false,
                } =>
        {
            Some(proof)
        }
        ProofRule::ConjunctionIntroduction(children) => children
            .iter()
            .find_map(|child| correlated_add(child, lower)),
        ProofRule::IntegerOrderSubstitution {
            relation, equality, ..
        } => correlated_add(relation, lower).or_else(|| correlated_add(equality, lower)),
        ProofRule::ValueEqualityTransport { premise, .. }
        | ProofRule::PredicateDenotation { premise }
        | ProofRule::IntegerOrderWeakening { relation: premise }
        | ProofRule::ConjunctionElimination {
            conjunction: premise,
            ..
        } => correlated_add(premise, lower),
        _ => None,
    }
}

#[test]
fn source_correlated_add_bound_has_checked_kernel_evidence() {
    check_source_correlated_add_bound("terminal_exact_add_u64_runtime_bound", false);
}

#[test]
fn source_correlated_add_lower_bound_has_checked_kernel_evidence() {
    check_source_correlated_add_bound("terminal_exact_add_signed_nonpositive_bound", true);
}

fn check_source_correlated_add_bound(machine_name: &str, lower: bool) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega/pass/terminal_psi/integer_control_contract/main.omg");
    let checked = compile_to_checked(CheckedCompileRequest::new(&source, Some("linux_x86_64")))
        .expect("existing exact arithmetic source customer checks");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, machine_name)
        .produce_artifact()
        .expect("existing customer produces Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let validated = terminal_verifier::validate_module(&module).unwrap();
    let questions =
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap();
    let mut witnessed = 0;
    for machine in &module.machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let terminal_psi::OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } = operation.kind
            else {
                continue;
            };
            let site = questions
                .obligations()
                .iter()
                .find(|site| site.obligation.id == obligation)
                .unwrap();
            let evidence = bundle
                .evidence
                .iter()
                .find(|evidence| evidence.obligation == obligation)
                .unwrap();
            let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
                panic!("exact add requires retained certificate")
            };
            let target = correlated_add(&certificate.proof, lower).unwrap_or_else(|| {
                panic!("expected correlated add witness: {:?}", certificate.proof)
            });
            witnessed += 1;
            let ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } = &target.rule
            else {
                unreachable!()
            };
            let ScalarTerm::ExactIntegerAdd {
                left: witness_left,
                right: witness_right,
                ..
            } = &witness.target
            else {
                unreachable!()
            };
            assert!(matches!(witness_left.as_ref(), ScalarTerm::Value { id, .. } if *id == left));
            assert!(matches!(witness_right.as_ref(), ScalarTerm::Value { id, .. } if *id == right));
            assert_eq!(
                root_bound.conclusion,
                if lower {
                    Proposition::LessOrEqual(witness.root.clone(), witness_left.as_ref().clone())
                } else {
                    Proposition::LessOrEqual(witness_left.as_ref().clone(), witness.root.clone())
                }
            );
            let [definition_index] = witness.definition_axioms.as_slice() else {
                panic!("source complement retains one exact subtraction definition")
            };
            let Proposition::Equal(
                root,
                ScalarTerm::ExactIntegerSubtract {
                    left: endpoint,
                    right: decrement,
                    ..
                },
            ) = &site.semantic_axioms[*definition_index]
            else {
                panic!("source subtraction definition must remain explicit")
            };
            assert_eq!(root, &witness.root);
            assert_eq!(decrement, witness_right);
            let [literal_index] = witness.literal_axioms.as_slice() else {
                panic!("source complement retains one endpoint landing coordinate")
            };
            let endpoint_value = if let Some(literal_index) = literal_index {
                let Proposition::Equal(landed, ScalarTerm::Integer { value, .. }) =
                    &site.semantic_axioms[*literal_index]
                else {
                    panic!("runtime endpoint must retain its literal equality")
                };
                assert_eq!(landed, endpoint.as_ref());
                *value
            } else {
                assert!(
                    lower,
                    "the original upper customer retains its runtime landing"
                );
                endpoint.integer_value().expect("closed endpoint").1
            };
            assert_eq!(
                endpoint_value,
                if lower {
                    semantic_vocabulary::IntegerValue::Signed(i32::MIN as i128)
                } else {
                    semantic_vocabulary::IntegerValue::Unsigned(u64::MAX as u128)
                }
            );
            let context = validated.value_context(machine).unwrap();
            let parameters = machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect();
            // Check the complete operation against independently reconstructed
            // requirements and semantics before examining its target bound.
            verify_bounded_certificate_with_machine_parameters(
                &context,
                &site.obligation.proposition,
                &site.requirements,
                &site.semantic_axioms,
                &parameters,
                &certificate.proof,
                &mut Budget::default(),
            )
            .unwrap();
            let mut kernel_budget = Budget::default();
            let available_steps = kernel_budget.remaining();
            let denoted = verify_bounded_certificate_with_machine_parameters(
                &context,
                &target.conclusion,
                &site.requirements,
                &site.semantic_axioms,
                &parameters,
                target,
                &mut kernel_budget,
            )
            .unwrap();
            for declaration in &denoted.certificate.signature {
                if declaration.body.is_some() {
                    continue;
                }
                let mut conclusion = declaration.ty;
                while let Term::Pi { codomain, .. } = denoted.arena.get(conclusion) {
                    conclusion = codomain;
                }
                assert!(
                    !denoted
                        .arena
                        .structurally_equal(conclusion, denoted.certificate.expected),
                    "correlated addition target must not be an instance assumption"
                );
            }
            let closure = certificate_assumption_closure(&denoted.arena, &denoted.certificate);
            // The exact closure retains the input vocabulary and fixed
            // monotonicity, cancellation and equality transport laws. Numeral
            // prefixes have checked bodies: MAX uses positions 3..=66, while
            // the signed customer's zero, MIN prefixes and negation are
            // interleaved with the independently reconstructed context.
            // This is the target bound's closure, not a claim that unrelated
            // rules in the complete operation certificate are axiom-free.
            if lower {
                assert_eq!(denoted.certificate.signature.len(), 64);
                assert_eq!(closure, (0..16).chain([17, 49]).chain(51..64).collect());
            } else {
                assert_eq!(denoted.certificate.signature.len(), 95);
                assert_eq!(closure, (0..3).chain(67..95).collect());
            }
            let definitions = denoted
                .certificate
                .signature
                .iter()
                .enumerate()
                .filter_map(|(position, declaration)| {
                    declaration.body.is_some().then_some(position)
                })
                .collect::<Vec<_>>();
            if lower {
                assert_eq!(
                    definitions,
                    [16].into_iter()
                        .chain(18..49)
                        .chain([50])
                        .collect::<Vec<_>>()
                );
            } else {
                assert_eq!(definitions, (3..67).collect::<Vec<_>>());
            }
            eprintln!(
                "source complement definition={definition_index}, literal={literal_index:?}; guard={:?}",
                root_bound.rule
            );
            eprintln!(
                "correlated add closure={closure:?}; signature={}; receipt={:?}",
                denoted.certificate.signature.len(),
                denoted.receipt()
            );
            let bytes = terminal_codec::encode_mathematical_certificate(
                &denoted.arena,
                &denoted.certificate,
            )
            .unwrap();
            eprintln!(
                "target normalization/conversion steps consumed={}, remaining={}; mathematical wire bytes={}",
                available_steps - kernel_budget.remaining(),
                kernel_budget.remaining(),
                bytes.len(),
            );
            let mut decoded = terminal_codec::decode_mathematical_certificate(&bytes).unwrap();
            verify_mathematical_certificate(
                &mut decoded.arena,
                &decoded.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            assert_eq!(
                certificate_assumption_closure(&decoded.arena, &decoded.certificate),
                closure
            );
            assert_eq!(
                terminal_codec::encode_mathematical_certificate(
                    &decoded.arena,
                    &decoded.certificate
                )
                .unwrap(),
                bytes
            );
            decoded.certificate.term = decoded.arena.insert(Term::TwoZero);
            assert!(
                verify_mathematical_certificate(
                    &mut decoded.arena,
                    &decoded.certificate,
                    &mut Budget::default()
                )
                .is_err()
            );
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &context,
                    &target.conclusion,
                    &[],
                    &[],
                    &parameters,
                    target,
                    &mut Budget::default(),
                )
                .is_err()
            );
            let mut changed_operand = target.clone();
            let ProofRule::IntegerAffineBound { witness, .. } = &mut changed_operand.rule else {
                unreachable!()
            };
            let ScalarTerm::ExactIntegerAdd { left, right, .. } = &mut witness.target else {
                unreachable!()
            };
            *right = left.clone();
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &context,
                    &changed_operand.conclusion,
                    &site.requirements,
                    &site.semantic_axioms,
                    &parameters,
                    &changed_operand,
                    &mut Budget::default(),
                )
                .is_err(),
                "the guard for the original right operand cannot justify a substituted addend"
            );
            let mut changed = target.clone();
            let ProofRule::IntegerAffineBound { root_bound, .. } = &mut changed.rule else {
                unreachable!()
            };
            let Proposition::LessOrEqual(left, right) = &mut root_bound.conclusion else {
                unreachable!()
            };
            std::mem::swap(left, right);
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &context,
                    &changed.conclusion,
                    &site.requirements,
                    &site.semantic_axioms,
                    &parameters,
                    &changed,
                    &mut Budget::default(),
                )
                .is_err(),
                "reversing the source guard cannot justify the bound"
            );
        }
    }
    assert_eq!(
        witnessed, 1,
        "the unchanged source must retain its addition operation"
    );
}
