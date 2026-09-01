//! Optimizer module role: test leaf. Location-independent countdown ranking replay.

use super::*;

use omega_optimization_unit::{
    EffectLink, FuelSettlement, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use omega_optimization_validation::{
    OptimizerUnsignedCountdownRankingCertificate, validate_transformed_psi_cycle_components,
};
use omega_psi_optimizer::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
};

#[derive(Clone, Copy)]
pub(super) enum Relocation {
    Zero,
    One,
    Both,
}

pub(super) struct RelocatedCountdown {
    pub(super) input: VerifiedPsiOptimizationInput,
    pub(super) unit: PsiOptimizationUnit,
    pub(super) certificate: OptimizerUnsignedCountdownRankingCertificate,
    pub(super) preheader: psi_core::BlockId,
}

#[test]
fn zero_one_and_pair_relocation_preserve_authenticated_ranking_and_freeze() {
    for relocation in [Relocation::Zero, Relocation::One, Relocation::Both] {
        let moved = relocated_countdown(relocation);
        let validated = validate_transformed_psi_cycle_components(&moved.input, &moved.unit)
            .expect("exact relocation preserves ranked-cycle analysis custody");
        assert_eq!(
            validated.ranking_certificates().certificates(),
            [moved.certificate]
        );
    }
}

#[test]
fn relocated_revisions_reconstruct_counted_invariant_and_placement_custody() {
    for relocation in [Relocation::Zero, Relocation::One, Relocation::Both] {
        let RelocatedCountdown {
            input,
            unit,
            certificate,
            preheader,
        } = relocated_countdown(relocation);
        let revision = unit.identity;
        let session = VerifiedPsiOptimizationSession::from_transformed(input, unit)
            .expect("exact relocation rebinds transformed analysis custody");
        assert_eq!(session.unit().identity, revision);
        assert_eq!(
            session.ranking_certificates().certificates(),
            [certificate.clone()]
        );

        let counted = session
            .counted_loop_analysis()
            .expect("counted-loop custody reconstructs after relocation");
        assert_eq!(counted.snapshot().revision, revision);
        let invariants = session
            .countdown_invariant_constant_analysis()
            .expect("invariant custody reconstructs after relocation");
        session
            .validate_countdown_invariant_constant_analysis(invariants.snapshot())
            .expect("relocated invariant snapshot independently replays");
        let [constants] = invariants.loops() else {
            panic!("one relocated countdown invariant row")
        };
        let [zero, one] = constants.constants.as_slice() else {
            panic!("relocated countdown retains exact zero and one")
        };
        assert_eq!(
            zero.location.block,
            if matches!(relocation, Relocation::Zero | Relocation::Both) {
                preheader
            } else {
                certificate.header
            }
        );
        assert_eq!(
            one.location.block,
            if matches!(relocation, Relocation::One | Relocation::Both) {
                preheader
            } else {
                certificate.descent.backedge.source
            }
        );

        let placements = session
            .countdown_invariant_constant_placement_analysis()
            .expect("placement custody reconstructs after relocation");
        session
            .validate_countdown_invariant_constant_placement_analysis(placements.snapshot())
            .expect("relocated placement snapshot independently replays");
        let [placed] = placements.loops() else {
            panic!("one relocated countdown placement row")
        };
        let jump = block(session.unit(), preheader).nodes.len() - 1;
        for (placement, constant) in placed.placements.iter().zip(&constants.constants) {
            assert_eq!(&placement.constant, constant);
            assert_eq!(placement.destination.before.block, preheader);
            assert_eq!(
                usize::try_from(placement.destination.before.node).ok(),
                Some(jump)
            );
            assert!(certificate.component.internal_edges.iter().any(|edge| {
                edge.source == placement.consumer.location.block
                    || edge.target == placement.consumer.location.block
            }));
        }
    }
}

#[test]
fn relocated_snapshot_corruption_fails_independent_replay() {
    let RelocatedCountdown { input, unit, .. } = relocated_countdown(Relocation::Both);
    let session = VerifiedPsiOptimizationSession::from_transformed(input, unit)
        .expect("exact pair relocation rebinds analysis custody");

    let mut invariants = session
        .countdown_invariant_constant_analysis()
        .expect("relocated invariants")
        .snapshot()
        .clone();
    invariants.loops[0].constants[0].location.node += 1;
    assert_eq!(
        session.validate_countdown_invariant_constant_analysis(&invariants),
        Err(CountdownInvariantConstantAnalysisError::SnapshotMismatch)
    );

    let mut placements = session
        .countdown_invariant_constant_placement_analysis()
        .expect("relocated placements")
        .snapshot()
        .clone();
    placements.loops[0].placements[1].destination.before.node -= 1;
    assert_eq!(
        session.validate_countdown_invariant_constant_placement_analysis(&placements),
        Err(CountdownInvariantConstantPlacementAnalysisError::SnapshotMismatch)
    );
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
fn relocated_constant_provenance_and_fuel_must_match_the_frozen_source_node() {
    let mut fuel = relocated_countdown(Relocation::Zero);
    find_operation_mut(&mut fuel.unit, fuel.certificate.guard.zero_operation).fuel[0].units += 1;
    fuel.unit.identity = recompute_psi_optimization_unit_identity(&fuel.unit);
    assert_frozen_at_original_role(&fuel, fuel.certificate.header);

    let mut provenance = relocated_countdown(Relocation::One);
    let inherited = PsiProvenance::Operation(provenance.certificate.descent.subtract_operation);
    let one = find_operation_mut(
        &mut provenance.unit,
        provenance.certificate.descent.one_operation,
    );
    one.provenance.push(inherited);
    one.fuel.push(FuelSettlement {
        site: inherited,
        units: 1,
    });
    provenance.unit.identity = recompute_psi_optimization_unit_identity(&provenance.unit);
    assert_frozen_at_original_role(&provenance, provenance.certificate.descent.backedge.source);
}

#[test]
fn relocation_does_not_thaw_any_other_ranked_node() {
    let mut moved = relocated_countdown(Relocation::Zero);
    find_operation_mut(
        &mut moved.unit,
        moved.certificate.guard.comparison_operation,
    )
    .fuel[0]
        .units += 1;
    moved.unit.identity = recompute_psi_optimization_unit_identity(&moved.unit);
    assert_frozen_at_original_role(&moved, moved.certificate.header);
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

pub(super) fn relocated_countdown(relocation: Relocation) -> RelocatedCountdown {
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

fn block(
    unit: &PsiOptimizationUnit,
    block: psi_core::BlockId,
) -> &omega_optimization_unit::OptimizationBlock {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
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
        let operation_order = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .enumerate()
            .filter_map(|(position, node)| match node.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        function.facts.sort_by_key(|fact| {
            let support = match fact {
                omega_optimization_unit::OptimizationFact::OperationObligationReference {
                    support,
                    ..
                }
                | omega_optimization_unit::OptimizationFact::BooleanConstant { support, .. }
                | omega_optimization_unit::OptimizationFact::IntegerConstant { support, .. } => {
                    support
                }
            };
            operation_order.get(support).copied()
        });
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

fn assert_frozen_at_original_role(moved: &RelocatedCountdown, block: psi_core::BlockId) {
    assert_eq!(
        validate_transformed_psi_optimization_unit(&moved.input, &moved.unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: moved.certificate.component.machine,
                block,
            }
        )
    );
}
