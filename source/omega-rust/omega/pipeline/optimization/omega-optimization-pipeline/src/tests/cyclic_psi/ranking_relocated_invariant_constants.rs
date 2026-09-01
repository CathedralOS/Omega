//! Optimizer module role: test leaf. Location-independent countdown ranking replay.

use super::*;

use omega_optimization_unit::{
    EffectLink, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use omega_optimization_validation::OptimizerUnsignedCountdownRankingCertificate;

#[derive(Clone, Copy)]
enum Relocation {
    Zero,
    One,
    Both,
}

struct RelocatedCountdown {
    input: VerifiedPsiOptimizationInput,
    unit: PsiOptimizationUnit,
    certificate: OptimizerUnsignedCountdownRankingCertificate,
    preheader: psi_core::BlockId,
}

#[test]
fn zero_one_and_pair_relocation_reconstruct_the_original_ranking_before_freeze() {
    for relocation in [Relocation::Zero, Relocation::One, Relocation::Both] {
        let moved = relocated_countdown(relocation);
        assert!(matches!(
            validate_transformed_psi_optimization_unit(&moved.input, &moved.unit),
            Err(OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine,
                block,
            }) if machine == moved.certificate.component.machine
                && moved.certificate.component.internal_edges.iter().any(|edge| {
                    edge.source == block || edge.target == block
                })
        ));
    }
}

#[test]
fn relocated_constants_reject_noncanonical_suffixes_and_foreign_blocks() {
    let mut reversed = relocated_countdown(Relocation::Both);
    let preheader = block_mut(&mut reversed.unit, reversed.preheader);
    let jump = preheader.nodes.len() - 1;
    preheader.nodes.swap(jump - 2, jump - 1);
    refresh_coordinates_and_effects(&mut reversed.unit);
    assert_ranking_mismatch(&reversed);

    let mut gapped = relocated_countdown(Relocation::Zero);
    let comparison = find_operation_mut(
        &mut gapped.unit,
        gapped.certificate.guard.comparison_operation,
    )
    .clone();
    let preheader = block_mut(&mut gapped.unit, gapped.preheader);
    let jump = preheader.nodes.len() - 1;
    preheader.nodes.insert(jump, comparison);
    refresh_coordinates_and_effects(&mut gapped.unit);
    assert_ranking_mismatch(&gapped);

    let mut foreign = relocated_countdown(Relocation::Zero);
    let zero = take_operation(&mut foreign.unit, foreign.certificate.guard.zero_operation);
    let foreign_block = foreign.unit.functions[0]
        .blocks
        .iter()
        .map(|block| block.id)
        .find(|block| {
            *block != foreign.preheader
                && foreign
                    .certificate
                    .component
                    .internal_edges
                    .iter()
                    .all(|edge| edge.source != *block && edge.target != *block)
        })
        .expect("countdown exit block is outside the component and preheader");
    let destination = block_mut(&mut foreign.unit, foreign_block);
    let terminator = destination.nodes.len() - 1;
    destination.nodes.insert(terminator, zero);
    refresh_coordinates_and_effects(&mut foreign.unit);
    assert_ranking_mismatch(&foreign);
}

#[test]
fn relocated_constant_shape_corruption_fails_ranking_before_freeze() {
    let mut moved = relocated_countdown(Relocation::Zero);
    let zero = find_operation_mut(&mut moved.unit, moved.certificate.guard.zero_operation);
    let AbstractOperation::IntegerConstant { value, .. } = &mut zero.operation else {
        panic!("guard zero remains an integer constant")
    };
    *value = psi_core::IntegerValue::Unsigned(1);
    moved.unit.identity = recompute_psi_optimization_unit_identity(&moved.unit);
    assert_ranking_mismatch(&moved);
}

#[test]
fn acyclic_replay_remains_empty() {
    let verified = super::countdown_invariant_constants::acyclic_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified acyclic session");
    assert!(session.cycle_components().components().is_empty());
    assert!(session.ranking_certificates().certificates().is_empty());
}

#[test]
fn fuel_mutation_still_reaches_the_unchanged_frozen_block_fence() {
    let (_, verified) = countdown_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified countdown");
    let certificate = session.ranking_certificates().certificates()[0].clone();
    let (input, mut unit) = session.into_parts();
    find_operation_mut(&mut unit, certificate.guard.zero_operation).fuel[0].units += 1;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert_eq!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: certificate.component.machine,
                block: certificate.header,
            }
        )
    );
}

fn relocated_countdown(relocation: Relocation) -> RelocatedCountdown {
    let (_, verified) = countdown_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified countdown");
    let certificate = session.ranking_certificates().certificates()[0].clone();
    let component = &session.cycle_components().components()[0];
    let [entry] = component.entries.as_slice() else {
        panic!("countdown has one preheader")
    };
    let preheader = entry.source;
    let (input, mut unit) = session.into_parts();
    let mut moved = Vec::new();
    if matches!(relocation, Relocation::Zero | Relocation::Both) {
        moved.push(take_operation(&mut unit, certificate.guard.zero_operation));
    }
    if matches!(relocation, Relocation::One | Relocation::Both) {
        moved.push(take_operation(&mut unit, certificate.descent.one_operation));
    }
    let destination = block_mut(&mut unit, preheader);
    let jump = destination.nodes.len() - 1;
    for (offset, node) in moved.into_iter().enumerate() {
        destination.nodes.insert(jump + offset, node);
    }
    refresh_coordinates_and_effects(&mut unit);
    RelocatedCountdown {
        input,
        unit,
        certificate,
        preheader,
    }
}

fn take_operation(
    unit: &mut PsiOptimizationUnit,
    operation: psi_core::OperationId,
) -> omega_optimization_unit::OptimizationNode {
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if let Some(index) = block.nodes.iter().position(|node| {
                node.provenance.first() == Some(&PsiProvenance::Operation(operation))
            }) {
                return block.nodes.remove(index);
            }
        }
    }
    panic!("operation {operation:?} exists")
}

fn find_operation_mut(
    unit: &mut PsiOptimizationUnit,
    operation: psi_core::OperationId,
) -> &mut omega_optimization_unit::OptimizationNode {
    unit.functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| node.provenance.first() == Some(&PsiProvenance::Operation(operation)))
        .expect("operation exists")
}

fn block_mut(
    unit: &mut PsiOptimizationUnit,
    block: psi_core::BlockId,
) -> &mut omega_optimization_unit::OptimizationBlock {
    unit.functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == block)
        .expect("block exists")
}

fn refresh_coordinates_and_effects(unit: &mut PsiOptimizationUnit) {
    for function in &mut unit.functions {
        let mut effect = 0u64;
        for block in &mut function.blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("test fixture fits u32");
                for definition in &mut node.definitions {
                    definition.site = ValueDefinitionSite::Node {
                        block: block.id,
                        node: node_index,
                    };
                }
                for value_use in &mut node.uses {
                    value_use.block = block.id;
                    value_use.node = node_index;
                }
                node.effect = EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

fn assert_ranking_mismatch(moved: &RelocatedCountdown) {
    assert_eq!(
        validate_transformed_psi_optimization_unit(&moved.input, &moved.unit),
        Err(
            OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch {
                machine: moved.certificate.component.machine,
            }
        )
    );
}
