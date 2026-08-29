use std::{collections::BTreeMap, sync::Arc};

use omega_abstract_operations::AbstractOperation;
use omega_optimization_core::{
    Optimization, OptimizationCandidateIdentity, OptimizationCandidateVerdict,
    OptimizationFactReference, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget,
};
use omega_optimization_policy::{
    BaselineDecisionOutcome, ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionLog,
    ExternalDecisionPoint, ExternalDecisionSchemaError, ValidatedCandidateSummary,
    external_psi_decision_schema_v1_identity, psi_target_neutral_decision_target_v1_identity,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewritePatch, PsiTransformationLedger};
use omega_optimization_validation::OptimizationUnitValidationError;
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

use super::*;
use crate::{
    AnalysisManager, AnalysisProduct, ExactIntegerAddConstantsRule, OrderedRuleRegistry,
    PsiOptimizationRule, RuleAnalysisView, RuleProposalError, built_in_psi_registries,
    built_in_psi_registry,
    rules::tests::{
        SelfDividePolicy, SelfRemainderPolicy, boolean_unit, compatible_policy_local_cse_unit,
        compatible_policy_phi_translated_gvn_unit, constant_conditional_same_target_unit,
        dead_exact_add_unit, dead_wrapping_add_unit, dependent_exact_chain_unit,
        diamond_dominator_gvn_unit, dominator_gvn_unit, exact_add_unit, linear_empty_block_unit,
        live_divide_by_one_unit, live_exact_multiply_by_zero_unit, live_exact_self_subtract_unit,
        live_exact_zero_value_shift_unit, live_remainder_by_one_unit, live_self_divide_unit,
        live_self_remainder_unit, live_signed_remainder_by_negative_one_unit, local_cse_unit,
        non_adjacent_merge_unit, phi_translated_gvn_unit, proof_certified_dominator_gvn_unit,
        proof_certified_local_cse_unit, proof_certified_phi_translated_gvn_unit,
        propagated_block_parameter_unit, randomized_built_in_registries,
        redundant_block_parameter_unit, wrapping_add_unit,
    },
};

#[derive(Debug)]
struct NonProfitableExactRule;

impl PsiOptimizationRule for NonProfitableExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        ExactIntegerAddConstantsRule
                .propose(unit, analyses)?
                .into_iter()
                .map(|candidate| {
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        0,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
    }
}

#[derive(Debug)]
struct DuplicateExactRule;

impl PsiOptimizationRule for DuplicateExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        let mut candidates = ExactIntegerAddConstantsRule.propose(unit, analyses)?;
        candidates.push(candidates[0].clone());
        Ok(candidates)
    }
}

#[derive(Debug)]
struct InvalidEvaluationExactRule;

impl PsiOptimizationRule for InvalidEvaluationExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        ExactIntegerAddConstantsRule
                .propose(unit, analyses)?
                .into_iter()
                .map(|candidate| {
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(mut patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    patch.constant = psi_core::IntegerValue::Unsigned(0);
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        candidate.predicted_cost_delta(),
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
    }
}

fn verified_empty_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{BlockId, ContractId, EdgeId, MachineId};
    use psi_terminal::{
        Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker,
    };

    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(401).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(401).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(402).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(402).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(403).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(404).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof =
        psi_terminal_codec::encode_proof_bundle(&psi_terminal_verifier::ProofBundle::default())
            .unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_add_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(8))
}

fn verified_exact_add_zero_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(0))
}

fn verified_compatible_policy_cse_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(451).unwrap();
    let block = BlockId::new(452).unwrap();
    let left = ValueId::new(453).unwrap();
    let right = ValueId::new(454).unwrap();
    let leader = ValueId::new(455).unwrap();
    let redundant = ValueId::new(456).unwrap();
    let result = ValueId::new(462).unwrap();
    let obligation = ObligationId::new(457).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(463).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(464).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(8),
                        },
                    },
                    Operation {
                        id: OperationId::new(458).unwrap(),
                        result: OperationResult::Scalar(declaration(leader)),
                        kind: OperationKind::WrappingIntegerAdd { left, right },
                    },
                    Operation {
                        id: OperationId::new(459).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant)),
                        kind: OperationKind::ExactIntegerAdd {
                            left: right,
                            right: left,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(460).unwrap(),
                    value: redundant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(461).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_compatible_policy_phi_gvn_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(501).unwrap();
    let join = BlockId::new(502).unwrap();
    let left_block = BlockId::new(503).unwrap();
    let entry = BlockId::new(504).unwrap();
    let right_block = BlockId::new(505).unwrap();
    let condition = ValueId::new(506).unwrap();
    let left_a = ValueId::new(507).unwrap();
    let left_b = ValueId::new(508).unwrap();
    let right_a = ValueId::new(509).unwrap();
    let right_b = ValueId::new(510).unwrap();
    let join_a = ValueId::new(511).unwrap();
    let join_b = ValueId::new(512).unwrap();
    let left_leader = ValueId::new(513).unwrap();
    let right_leader = ValueId::new(514).unwrap();
    let redundant = ValueId::new(515).unwrap();
    let result = ValueId::new(516).unwrap();
    let obligation = ObligationId::new(517).unwrap();
    let zero = ValueId::new(527).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![
                declaration(condition, ScalarType::Boolean),
                declaration(left_a, scalar_type),
                declaration(left_b, scalar_type),
                declaration(right_a, scalar_type),
                declaration(right_b, scalar_type),
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
                    id: join,
                    parameters: vec![
                        declaration(join_a, scalar_type),
                        declaration(join_b, scalar_type),
                    ],
                    operations: vec![Operation {
                        id: OperationId::new(518).unwrap(),
                        result: OperationResult::Scalar(declaration(redundant, scalar_type)),
                        kind: OperationKind::ExactIntegerShiftLeft {
                            value: join_a,
                            count: join_b,
                            obligation,
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(519).unwrap(),
                        value: redundant,
                    },
                },
                Block {
                    id: left_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(520).unwrap(),
                        result: OperationResult::Scalar(declaration(left_leader, scalar_type)),
                        kind: OperationKind::WrappingIntegerShiftLeft {
                            value: left_a,
                            count: zero,
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(521).unwrap(),
                        target: join,
                        arguments: vec![left_a, zero],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(528).unwrap(),
                        result: OperationResult::Scalar(declaration(zero, scalar_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(0),
                        },
                    }],
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(522).unwrap(),
                            target: left_block,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(523).unwrap(),
                            target: right_block,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: right_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(524).unwrap(),
                        result: OperationResult::Scalar(declaration(right_leader, scalar_type)),
                        kind: OperationKind::WrappingIntegerShiftLeft {
                            value: right_a,
                            count: zero,
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(525).unwrap(),
                        target: join,
                        arguments: vec![right_a, zero],
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(526).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_add_unit_with_right(
    right_constant: psi_core::IntegerValue,
) -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(411).unwrap();
    let block = BlockId::new(412).unwrap();
    let left = ValueId::new(413).unwrap();
    let right = ValueId::new(414).unwrap();
    let computed = ValueId::new(415).unwrap();
    let result = ValueId::new(422).unwrap();
    let obligation = ObligationId::new(419).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(416).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(417).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: right_constant,
                        },
                    },
                    Operation {
                        id: OperationId::new(418).unwrap(),
                        result: OperationResult::Scalar(declaration(computed)),
                        kind: OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(420).unwrap(),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(421).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_self_division_or_remainder_unit(divide: bool) -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
        MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
    };
    use psi_proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(431).unwrap();
    let block = BlockId::new(432).unwrap();
    let operand = ValueId::new(433).unwrap();
    let remainder = ValueId::new(434).unwrap();
    let result = ValueId::new(435).unwrap();
    let obligation = ObligationId::new(436).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let goal = Proposition::LessOrEqual(one, ScalarTerm::value(operand, scalar_type));
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(operand)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(437).unwrap(),
                    result: OperationResult::Scalar(declaration(remainder)),
                    kind: if divide {
                        OperationKind::ExactIntegerDivide {
                            left: operand,
                            right: operand,
                            obligation,
                        }
                    } else {
                        OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: operand,
                            obligation,
                        }
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(438).unwrap(),
                    value: remainder,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(439).unwrap(),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(440).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_remainder_by_one_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(451).unwrap();
    let block = BlockId::new(452).unwrap();
    let operand = ValueId::new(453).unwrap();
    let one = ValueId::new(454).unwrap();
    let remainder = ValueId::new(455).unwrap();
    let result = ValueId::new(456).unwrap();
    let obligation = ObligationId::new(457).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(operand)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(458).unwrap(),
                        result: OperationResult::Scalar(declaration(one)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    },
                    Operation {
                        id: OperationId::new(459).unwrap(),
                        result: OperationResult::Scalar(declaration(remainder)),
                        kind: OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: one,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(460).unwrap(),
                    value: remainder,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(461).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_signed_remainder_by_negative_one_unit() -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(471).unwrap();
    let block = BlockId::new(472).unwrap();
    let operand = ValueId::new(473).unwrap();
    let negative_one = ValueId::new(474).unwrap();
    let remainder = ValueId::new(475).unwrap();
    let result = ValueId::new(476).unwrap();
    let obligation = ObligationId::new(477).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(478).unwrap(),
                        result: OperationResult::Scalar(declaration(operand)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(479).unwrap(),
                        result: OperationResult::Scalar(declaration(negative_one)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-1),
                        },
                    },
                    Operation {
                        id: OperationId::new(480).unwrap(),
                        result: OperationResult::Scalar(declaration(remainder)),
                        kind: OperationKind::ExactIntegerRemainder {
                            left: operand,
                            right: negative_one,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(481).unwrap(),
                    value: remainder,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(482).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}

fn verified_exact_self_remainder_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(false)
}

fn verified_exact_self_divide_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_self_division_or_remainder_unit(true)
}

fn budget(iterations: u64) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, iterations).unwrap()
}

fn external_log_with(
    context: ExternalDecisionContext,
    points: impl IntoIterator<Item = ExternalDecisionPoint>,
) -> ExternalDecisionLog {
    ExternalDecisionLog::new(context, points).unwrap()
}

fn run_test_pipeline(
    mut unit: PsiOptimizationUnit,
    registries: &[OrderedRuleRegistry],
) -> (
    PsiOptimizationUnit,
    Vec<OptimizationPassManifestRecord>,
    PsiTransformationLedger,
) {
    let input = unit.identity;
    let psi = unit.psi;
    let fuel_schedule = unit.fuel_schedule;
    let mut manifests = Vec::with_capacity(registries.len());
    let mut records = Vec::new();
    for registry in registries {
        let (output, _, _, _, manifest, ledger) = run_unit(unit, registry, budget(8)).unwrap();
        manifests.push(manifest.expect("a selected pass emits a manifest row"));
        records.extend_from_slice(ledger.records());
        unit = output;
    }
    let ledger =
        PsiTransformationLedger::new(psi, fuel_schedule, input, unit.identity, records).unwrap();
    (unit, manifests, ledger)
}

mod execution;
mod external_decisions;
