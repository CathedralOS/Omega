//! Common budgets and baseline selected-artifact fixtures.

use crate::tests::*;

pub(crate) fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(128, 128, 128, 128, 16).unwrap()
}

pub(crate) fn selected_lowering_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 64).unwrap()
}

pub(crate) fn canonical_artifact(
    semantic: &[u8],
    proof: &[u8],
) -> psi_terminal_codec::CanonicalTerminalArtifact {
    let module = psi_terminal_codec::decode_module(semantic).unwrap();
    let proof = psi_terminal_codec::decode_proof_bundle(proof).unwrap();
    psi_terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, None).unwrap()
}

/// Build certificates for exact integer operations in a fixture.
///
/// Satisfiable operations use the same independently checked producer as Psi
/// lowering. Deliberately overflowing or underflowing negative fixtures retain
/// a certificate-shaped rejection case so the artifact verifier, rather than
/// fixture construction, remains the boundary under test.
pub(crate) fn operation_proof_bundle(module: &TerminalModule) -> ProofBundle {
    let validated = psi_terminal_verifier::validate_module(module).unwrap();
    let reconstructed = reconstruct_operation_obligations(module).unwrap();
    let mut evidence = reconstructed
        .into_iter()
        .map(|question| {
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == question.owner.machine())
                .expect("reconstructed operation owner belongs to the fixture module");
            assert!(
                question.canonical_certificate,
                "fixture operation must expose a canonical certificate goal"
            );
            let context = validated.value_context(machine).unwrap();
            let machine_parameter_values = machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect();
            let proof = psi_checked_trees_to_terminal::produce_checked_canonical_integer_proof(
                &context,
                &question.obligation.proposition,
                &machine.contract.requires,
                &question.semantic_axioms,
                &machine_parameter_values,
            )
            .unwrap_or_else(|| ProofNode {
                conclusion: question.obligation.proposition.clone(),
                rule: ProofRule::Assumption { index: 0 },
            });
            ObligationEvidence {
                obligation: question.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(question.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|evidence| evidence.obligation);
    ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence,
    }
}

pub(crate) fn artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(2_001).unwrap();
    let entry = BlockId::new(2_002).unwrap();
    let exit = BlockId::new(2_003).unwrap();
    let left = ValueId::new(2_004).unwrap();
    let right = ValueId::new(2_005).unwrap();
    let computed = ValueId::new(2_006).unwrap();
    let forwarded = ValueId::new(2_007).unwrap();
    let also_forwarded = ValueId::new(2_016).unwrap();
    let result = ValueId::new(2_008).unwrap();
    let obligation = ObligationId::new(2_009).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: OperationId::new(2_010).unwrap(),
                            result: OperationResult::Scalar(declaration(left)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            id: OperationId::new(2_011).unwrap(),
                            result: OperationResult::Scalar(declaration(right)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(8),
                            },
                        },
                        Operation {
                            id: OperationId::new(2_012).unwrap(),
                            result: OperationResult::Scalar(declaration(computed)),
                            kind: OperationKind::ExactIntegerAdd {
                                left,
                                right,
                                obligation,
                            },
                        },
                    ],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(2_013).unwrap(),
                        target: exit,
                        arguments: vec![computed, computed],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: exit,
                    parameters: vec![declaration(forwarded), declaration(also_forwarded)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(2_014).unwrap(),
                        value: forwarded,
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(2_015).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = operation_proof_bundle(&module);
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn conditional_immediate_artifact() -> (Vec<u8>, Vec<u8>) {
    conditional_immediate_artifact_with_type(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

pub(crate) fn conditional_u64_integer_equal_parameters_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = conditional_u64_integer_equal_parameters_machine(19_000, [7, 9]);
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn conditional_u64_integer_less_than_parameters_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = conditional_u64_integer_less_than_parameters_machine(19_200, [7, 9]);
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn conditional_u64_integer_less_than_parameters_machine(
    base: u64,
    literals: [u128; 2],
) -> TerminalMachine {
    let mut machine = conditional_u64_integer_equal_parameters_machine(base, literals);
    let OperationKind::IntegerEqual { left, right } = machine.blocks[0].operations[0].kind else {
        unreachable!("shared comparison fixture must begin with integer equality")
    };
    machine.blocks[0].operations[0].kind = OperationKind::IntegerLessThan { left, right };
    machine
}

pub(crate) fn conditional_u64_integer_equal_parameters_machine(
    base: u64,
    literals: [u128; 2],
) -> TerminalMachine {
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let when_true = BlockId::new(base + 3).unwrap();
    let when_false = BlockId::new(base + 4).unwrap();
    let left = ValueId::new(base + 5).unwrap();
    let right = ValueId::new(base + 6).unwrap();
    let condition = ValueId::new(base + 7).unwrap();
    let true_value = ValueId::new(base + 8).unwrap();
    let false_value = ValueId::new(base + 9).unwrap();
    let result = ValueId::new(base + 10).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    TerminalMachine {
        id: machine,
        attachment: None,
        parameters: vec![
            declaration(left, scalar_type),
            declaration(right, scalar_type),
        ],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(base + 11).unwrap(),
                    result: OperationResult::Scalar(declaration(condition, ScalarType::Boolean)),
                    kind: OperationKind::IntegerEqual { left, right },
                }],
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(base + 14).unwrap(),
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(base + 15).unwrap(),
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: when_true,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(base + 12).unwrap(),
                    result: OperationResult::Scalar(declaration(true_value, scalar_type)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(literals[0]),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(base + 16).unwrap(),
                    value: true_value,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: when_false,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(base + 13).unwrap(),
                    result: OperationResult::Scalar(declaration(false_value, scalar_type)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(literals[1]),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(base + 17).unwrap(),
                    value: false_value,
                    cleanup_actions: Vec::new(),
                },
            },
        ],
        contract: MachineContract {
            id: ContractId::new(base + 18).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

pub(crate) fn conditional_immediate_machine(
    base: u64,
    integer_type: IntegerType,
    literals: [u128; 2],
) -> TerminalMachine {
    let machine = MachineId::new(base + 1).unwrap();
    let entry = BlockId::new(base + 2).unwrap();
    let when_true = BlockId::new(base + 3).unwrap();
    let when_false = BlockId::new(base + 4).unwrap();
    let condition = ValueId::new(base + 5).unwrap();
    let true_value = ValueId::new(base + 6).unwrap();
    let false_value = ValueId::new(base + 7).unwrap();
    let result = ValueId::new(base + 8).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    TerminalMachine {
        id: machine,
        attachment: None,
        parameters: vec![declaration(condition, ScalarType::Boolean)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![
            Block {
                id: entry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(base + 11).unwrap(),
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(base + 12).unwrap(),
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            Block {
                id: when_true,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(base + 9).unwrap(),
                    result: OperationResult::Scalar(declaration(true_value, scalar_type)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(literals[0]),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(base + 13).unwrap(),
                    value: true_value,
                    cleanup_actions: Vec::new(),
                },
            },
            Block {
                id: when_false,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(base + 10).unwrap(),
                    result: OperationResult::Scalar(declaration(false_value, scalar_type)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(literals[1]),
                    },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(base + 14).unwrap(),
                    value: false_value,
                    cleanup_actions: Vec::new(),
                },
            },
        ],
        contract: MachineContract {
            id: ContractId::new(base + 15).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

pub(crate) fn conditional_immediate_module(
    entry: MachineId,
    machines: Vec<TerminalMachine>,
) -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines,
    }
}

pub(crate) fn conditional_immediate_artifact_with_type(
    integer_type: IntegerType,
) -> (Vec<u8>, Vec<u8>) {
    let machine = conditional_immediate_machine(3_000, integer_type, [7, 9]);
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn disconnected_conditional_artifact() -> (Vec<u8>, Vec<u8>) {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let entry = conditional_immediate_machine(16_000, integer_type, [7, 9]);
    let detached = conditional_immediate_machine(17_000, integer_type, [11, 13]);
    let module = conditional_immediate_module(entry.id, vec![entry, detached]);
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(crate) fn staged_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_immediate_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
