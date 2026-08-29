//! Canonical candidate and rewrite-plan identity encoding.

use super::model::*;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_candidate(
    input: OptimizationUnitIdentity,
    contract: OptimizationRuleContract,
    decision_point: &PsiRewriteDecisionPoint,
    affected_blocks: &[BlockId],
    substitutions: &[ScalarSubstitution],
    provenance: &[ProvenanceRewrite],
    witness: &PsiRewriteWitness,
    predicted_cost_delta: i64,
    patch: &PsiRewritePatch,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    // The version identifies this extensible tagged schema, not the current
    // largest tag. Rehashing established candidates when a new tag is added
    // would also change deterministic cost-tie decisions for unrelated rules.
    bytes.extend_from_slice(b"omega.psi-rewrite-candidate.v24\0");
    bytes.extend_from_slice(&input.bytes());
    bytes.extend_from_slice(&contract.encode());
    match decision_point {
        PsiRewriteDecisionPoint::Node(location) => {
            bytes.push(1);
            encode_location(&mut bytes, *location);
        }
        PsiRewriteDecisionPoint::MachineSet(machines) => {
            bytes.push(2);
            encode_len(&mut bytes, machines.len());
            for machine in machines {
                bytes.extend_from_slice(&machine.get().to_le_bytes());
            }
        }
    }
    encode_len(&mut bytes, affected_blocks.len());
    for block in affected_blocks {
        bytes.extend_from_slice(&block.get().to_le_bytes());
    }
    encode_len(&mut bytes, substitutions.len());
    for substitution in substitutions {
        bytes.extend_from_slice(&substitution.from.get().to_le_bytes());
        bytes.extend_from_slice(&substitution.to.get().to_le_bytes());
        encode_scalar_type(&mut bytes, substitution.scalar_type);
    }
    encode_len(&mut bytes, provenance.len());
    for row in provenance {
        encode_realization_site(&mut bytes, row.input);
        match row.disposition {
            ProvenanceDisposition::RealizedAt(site) => {
                bytes.push(row.disposition.canonical_tag());
                encode_realization_site(&mut bytes, site);
            }
            ProvenanceDisposition::ProvenUnreachableAt(site) => {
                bytes.push(row.disposition.canonical_tag());
                encode_realization_site(&mut bytes, site);
            }
        }
        encode_len(&mut bytes, row.sources.len());
        for source in &row.sources {
            match source {
                PsiProvenance::Operation(operation) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&operation.get().to_le_bytes());
                }
                PsiProvenance::Edge(edge) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&edge.get().to_le_bytes());
                }
            }
        }
        encode_len(&mut bytes, row.fuel.len());
        for settlement in &row.fuel {
            match settlement.site {
                PsiProvenance::Operation(operation) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&operation.get().to_le_bytes());
                }
                PsiProvenance::Edge(edge) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&edge.get().to_le_bytes());
                }
            }
            bytes.extend_from_slice(&settlement.units.to_le_bytes());
        }
    }
    match witness {
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary { operand_fact }) => {
            bytes.extend_from_slice(&[1, 1]);
            bytes.extend_from_slice(&operand_fact.bytes());
        }
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Binary {
            left_fact,
            right_fact,
        }) => {
            bytes.extend_from_slice(&[1, 2]);
            bytes.extend_from_slice(&left_fact.bytes());
            bytes.extend_from_slice(&right_fact.bytes());
        }
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::ProofCertifiedUnary {
            operand_fact,
            obligation_fact,
        }) => {
            bytes.extend_from_slice(&[1, 3]);
            bytes.extend_from_slice(&operand_fact.bytes());
            bytes.extend_from_slice(&obligation_fact.bytes());
        }
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::ProofCertifiedBinary {
            left_fact,
            right_fact,
            obligation_fact,
        }) => {
            bytes.extend_from_slice(&[1, 4]);
            bytes.extend_from_slice(&left_fact.bytes());
            bytes.extend_from_slice(&right_fact.bytes());
            bytes.extend_from_slice(&obligation_fact.bytes());
        }
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::RangeAgainstConstant {
            range_fact,
            constant_fact,
        }) => {
            bytes.extend_from_slice(&[1, 5]);
            bytes.extend_from_slice(&range_fact.bytes());
            bytes.extend_from_slice(&constant_fact.bytes());
        }
        PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::RangeAgainstRange {
            left_range_fact,
            right_range_fact,
        }) => {
            bytes.extend_from_slice(&[1, 6]);
            bytes.extend_from_slice(&left_range_fact.bytes());
            bytes.extend_from_slice(&right_range_fact.bytes());
        }
        PsiRewriteWitness::RedundantBlockParameter(witness) => {
            bytes.push(2);
            encode_len(&mut bytes, witness.incoming.len());
            for incoming in &witness.incoming {
                bytes.extend_from_slice(&incoming.source.get().to_le_bytes());
                bytes.extend_from_slice(&incoming.edge.get().to_le_bytes());
                bytes.extend_from_slice(&incoming.argument.get().to_le_bytes());
            }
        }
        PsiRewriteWitness::StructuralIdentity => bytes.push(3),
        PsiRewriteWitness::AcceptedObligation(identity) => {
            bytes.push(4);
            bytes.extend_from_slice(&identity.bytes());
        }
        PsiRewriteWitness::ProofCertifiedScalarIdentity {
            constant_fact,
            obligation_fact,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&constant_fact.bytes());
            bytes.extend_from_slice(&obligation_fact.bytes());
        }
        PsiRewriteWitness::OwnershipFrontiers(witness) => {
            bytes.push(6);
            encode_len(&mut bytes, witness.rows.len());
            for row in &witness.rows {
                encode_ownership_frontier_site(&mut bytes, row.site);
                bytes.extend_from_slice(&row.fact.bytes());
            }
        }
        PsiRewriteWitness::TotalScalarIdentity { constant_fact } => {
            bytes.push(7);
            bytes.extend_from_slice(&constant_fact.bytes());
        }
    }
    bytes.extend_from_slice(&predicted_cost_delta.to_le_bytes());
    match patch {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
            bytes.push(1);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            encode_integer_type(&mut bytes, patch.scalar_type);
            encode_integer_value(&mut bytes, patch.constant);
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
            bytes.push(2);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            bytes.push(u8::from(patch.constant));
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
            bytes.push(3);
            bytes.extend_from_slice(&patch.machine.get().to_le_bytes());
            bytes.extend_from_slice(&patch.block.get().to_le_bytes());
            bytes.extend_from_slice(&patch.position.to_le_bytes());
            bytes.extend_from_slice(&patch.parameter.get().to_le_bytes());
            bytes.extend_from_slice(&patch.replacement.get().to_le_bytes());
            encode_scalar_type(&mut bytes, patch.scalar_type);
        }
        PsiRewritePatch::FoldConstantConditional(patch) => {
            bytes.push(4);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.condition.get().to_le_bytes());
            bytes.push(u8::from(patch.constant));
            bytes.extend_from_slice(&patch.selected_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.rejected_edge.get().to_le_bytes());
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
            bytes.push(5);
            encode_location(&mut bytes, patch.predecessor);
            bytes.extend_from_slice(&patch.incoming_edge.get().to_le_bytes());
            encode_location(&mut bytes, patch.empty);
            bytes.extend_from_slice(&patch.outgoing_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.target.get().to_le_bytes());
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
            bytes.push(6);
            encode_location(&mut bytes, patch.empty);
            bytes.extend_from_slice(&patch.outgoing_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.target.get().to_le_bytes());
        }
        PsiRewritePatch::MergeAdjacentBlock(patch) => {
            bytes.push(7);
            encode_location(&mut bytes, patch.predecessor);
            bytes.extend_from_slice(&patch.incoming_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.target.get().to_le_bytes());
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
            bytes.push(8);
            encode_len(&mut bytes, patch.machines.len());
            for custody in &patch.machines {
                bytes.extend_from_slice(&custody.machine.get().to_le_bytes());
                bytes.extend_from_slice(&custody.source_ordinal.to_le_bytes());
            }
        }
        PsiRewritePatch::FuseSharedTerminalJump(patch) => {
            bytes.push(9);
            encode_location(&mut bytes, patch.predecessor);
            bytes.extend_from_slice(&patch.incoming_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.target.get().to_le_bytes());
        }
        PsiRewritePatch::RemoveDeadScalarNode(patch) => {
            bytes.push(10);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            encode_scalar_type(&mut bytes, patch.scalar_type);
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
            bytes.push(11);
            encode_location(&mut bytes, patch.leader);
            encode_location(&mut bytes, patch.redundant);
            bytes.extend_from_slice(&patch.leader_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.redundant_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.leader_result.get().to_le_bytes());
            bytes.extend_from_slice(&patch.redundant_result.get().to_le_bytes());
            encode_scalar_type(&mut bytes, patch.scalar_type);
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
            bytes.push(12);
            encode_location(&mut bytes, patch.leader);
            encode_location(&mut bytes, patch.redundant);
            bytes.extend_from_slice(&patch.leader_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.redundant_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.leader_result.get().to_le_bytes());
            bytes.extend_from_slice(&patch.redundant_result.get().to_le_bytes());
            encode_scalar_type(&mut bytes, patch.scalar_type);
        }
        PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
            bytes.push(13);
            encode_location(&mut bytes, patch.predecessor);
            bytes.extend_from_slice(&patch.incoming_edge.get().to_le_bytes());
            bytes.extend_from_slice(&patch.target.get().to_le_bytes());
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
            bytes.push(14);
            encode_location(&mut bytes, patch.redundant);
            bytes.extend_from_slice(&patch.redundant_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.redundant_result.get().to_le_bytes());
            encode_scalar_type(&mut bytes, patch.scalar_type);
            bytes.extend_from_slice(&patch.parameter_position.to_le_bytes());
            encode_len(&mut bytes, patch.incoming.len());
            for incoming in &patch.incoming {
                bytes.extend_from_slice(&incoming.source.get().to_le_bytes());
                bytes.extend_from_slice(&incoming.edge.get().to_le_bytes());
                encode_location(&mut bytes, incoming.leader);
                bytes.extend_from_slice(&incoming.leader_operation.get().to_le_bytes());
                bytes.extend_from_slice(&incoming.leader_result.get().to_le_bytes());
            }
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
            bytes.push(15);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            bytes.extend_from_slice(&patch.replacement.get().to_le_bytes());
            encode_integer_type(&mut bytes, patch.scalar_type);
            bytes.push(match patch.identity {
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft => 1,
                ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight => 2,
                ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight => 3,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft => 4,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight => 5,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount => 6,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount => 7,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight => 8,
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight => 9,
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight => 10,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft => 11,
                ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight => 12,
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft => 13,
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft => 14,
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft => 15,
                ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft => 16,
                ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft => 17,
                ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft => 18,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue => 19,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue => 20,
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue => 21,
            });
        }
        PsiRewritePatch::EliminateTotalScalarIdentity(patch) => {
            bytes.push(16);
            encode_location(&mut bytes, patch.location);
            bytes.extend_from_slice(&patch.source_operation.get().to_le_bytes());
            bytes.extend_from_slice(&patch.result.get().to_le_bytes());
            bytes.extend_from_slice(&patch.replacement.get().to_le_bytes());
            encode_integer_type(&mut bytes, patch.scalar_type);
            bytes.push(match patch.identity {
                TotalScalarIdentityKind::WrappingIntegerAddZeroLeft => 1,
                TotalScalarIdentityKind::WrappingIntegerAddZeroRight => 2,
                TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight => 3,
                TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft => 4,
                TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => 5,
                TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount => 6,
                TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => 7,
                TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft => 8,
                TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => 9,
                TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft => 10,
                TotalScalarIdentityKind::SaturatingIntegerAddZeroRight => 11,
                TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight => 12,
                TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft => 13,
                TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => 14,
                TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft => 15,
                TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => 16,
                TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft => 17,
                TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight => 18,
                TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft => 19,
                TotalScalarIdentityKind::IntegerBitwiseOrZeroRight => 20,
                TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft => 21,
                TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => 22,
            });
        }
    }
    bytes
}

pub(super) fn encode_location(bytes: &mut Vec<u8>, location: NodeLocation) {
    bytes.extend_from_slice(&location.machine.get().to_le_bytes());
    bytes.extend_from_slice(&location.block.get().to_le_bytes());
    bytes.extend_from_slice(&location.node.to_le_bytes());
}

pub(super) fn encode_ownership_frontier_site(bytes: &mut Vec<u8>, site: OwnershipFrontierSite) {
    let (tag, identity) = match site {
        OwnershipFrontierSite::BlockEntry(id) => (1, id.get()),
        OwnershipFrontierSite::OperationEntry(id) => (2, id.get()),
        OwnershipFrontierSite::OperationExit(id) => (3, id.get()),
        OwnershipFrontierSite::EdgeEntry(id) => (4, id.get()),
        OwnershipFrontierSite::EdgeExit(id) => (5, id.get()),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&identity.to_le_bytes());
}

pub(super) fn encode_realization_site(bytes: &mut Vec<u8>, site: PsiRealizationSite) {
    match site {
        PsiRealizationSite::Node(location) => {
            bytes.push(1);
            encode_location(bytes, location);
        }
        PsiRealizationSite::Edge { machine, edge } => {
            bytes.push(2);
            bytes.extend_from_slice(&machine.get().to_le_bytes());
            bytes.extend_from_slice(&edge.get().to_le_bytes());
        }
    }
}

pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(3);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical candidate list length fits u64")
            .to_le_bytes(),
    );
}

pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(1),
        ScalarType::Integer(integer) => {
            bytes.push(2);
            encode_integer_type(bytes, integer);
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut Vec<u8>, integer: IntegerType) {
    bytes.push(match integer.carrier() {
        IntegerCarrier::Fixed => 1,
        IntegerCarrier::Address => 2,
    });
    bytes.push(match integer.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    bytes.extend_from_slice(&integer.bits().to_le_bytes());
}

pub(super) fn encode_integer_value(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
