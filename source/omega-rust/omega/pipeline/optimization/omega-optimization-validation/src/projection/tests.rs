use super::custody::validate_source_custody;
use super::*;

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationValidatorIdentity,
};
use omega_optimization_unit::{
    FuelSettlement, NodeLocation, ProvenanceRewrite, PsiTransformationRecord,
    reconstruct_psi_optimization_unit_seed,
};
use psi_core::{BlockId, EdgeId, MachineId};
use psi_terminal::{SemanticFingerprint, VocabularyMarker};

fn receipt() -> ValidatedOptimizedAbstractPlanProjection {
    ValidatedOptimizedAbstractPlanProjection {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(2).unwrap(),
        initial_unit: OptimizationUnitIdentity::from_canonical_bytes(b"initial"),
        final_unit: OptimizationUnitIdentity::from_canonical_bytes(b"final"),
        selections: OptimizationSelectionIdentity::from_bytes([3; 32]),
        psi_selections: OptimizationSelectionIdentity::from_bytes([4; 32]),
        ledger: TransformationLedgerIdentity::from_canonical_bytes(b"ledger"),
        bundle: omega_optimization_core::OptimizationIdentityBundleIdentity::from_canonical_bytes(
            b"bundle",
        ),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator"),
    }
}

fn custody_unit() -> PsiOptimizationUnit {
    let machine = MachineId::new(41).unwrap();
    let block = BlockId::new(42).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(44).unwrap(),
                    cleanup_actions: Vec::new(),
                }],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

fn custody_record(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    disposition: ProvenanceDisposition,
    source: PsiProvenance,
) -> omega_optimization_unit::PsiTransformationRecord {
    PsiTransformationRecord {
        rule: OptimizationRuleIdentity::from_canonical_bytes(b"custody-rule"),
        candidate: OptimizationCandidateIdentity::from_canonical_bytes(&output.bytes()),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(b"custody-validator"),
        input,
        output,
        pruned_machines: Vec::new(),
        provenance: vec![ProvenanceRewrite {
            input: disposition.site(),
            disposition,
            sources: vec![source],
            fuel: vec![FuelSettlement {
                site: source,
                units: 1,
            }],
        }],
    }
}

#[test]
fn projection_identity_binds_every_validated_custody_field() {
    let baseline = receipt();
    let changed = [
        ValidatedOptimizedAbstractPlanProjection {
            psi: TerminalPsiIdentity {
                program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
                ..baseline.psi
            },
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            fuel_schedule: FuelScheduleIdentity::new(9).unwrap(),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            initial_unit: OptimizationUnitIdentity::from_canonical_bytes(b"initial-drift"),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            final_unit: OptimizationUnitIdentity::from_canonical_bytes(b"final-drift"),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            selections: OptimizationSelectionIdentity::from_bytes([9; 32]),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            psi_selections: OptimizationSelectionIdentity::from_bytes([9; 32]),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            ledger: TransformationLedgerIdentity::from_canonical_bytes(b"ledger-drift"),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            bundle:
                omega_optimization_core::OptimizationIdentityBundleIdentity::from_canonical_bytes(
                    b"bundle-drift",
                ),
            ..baseline
        },
        ValidatedOptimizedAbstractPlanProjection {
            validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator-drift"),
            ..baseline
        },
    ];

    assert_eq!(baseline.identity(), receipt().identity());
    for corrupted in changed {
        assert_ne!(baseline.identity(), corrupted.identity());
    }
}

#[test]
fn source_custody_is_an_exact_final_or_unreachable_partition() {
    let initial = custody_unit();
    let location = NodeLocation {
        machine: initial.functions[0].machine,
        block: initial.functions[0].blocks[0].id,
        node: 0,
    };
    let source = initial.functions[0].blocks[0].nodes[0].provenance[0];
    let mut final_unit = initial.clone();
    final_unit.functions[0].blocks[0].nodes.clear();
    final_unit.identity =
        omega_optimization_unit::recompute_psi_optimization_unit_identity(&final_unit);
    let record = custody_record(
        initial.identity,
        final_unit.identity,
        ProvenanceDisposition::ProvenUnreachableAt(PsiRealizationSite::Node(location)),
        source,
    );
    let ledger = PsiTransformationLedger::new(
        initial.psi,
        initial.fuel_schedule,
        initial.identity,
        final_unit.identity,
        vec![record.clone()],
    )
    .unwrap();
    validate_source_custody(&initial, &final_unit, &ledger).unwrap();

    assert_eq!(
        validate_source_custody(&initial, &initial, &ledger),
        Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
    );

    let mut wrong_units = record;
    wrong_units.provenance[0].fuel[0].units = 2;
    let wrong_ledger = PsiTransformationLedger::new(
        initial.psi,
        initial.fuel_schedule,
        initial.identity,
        final_unit.identity,
        vec![wrong_units],
    )
    .unwrap();
    assert_eq!(
        validate_source_custody(&initial, &final_unit, &wrong_ledger),
        Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
    );
}

#[test]
fn source_custody_rejects_resurrection_after_unreachability() {
    let initial = custody_unit();
    let location = NodeLocation {
        machine: initial.functions[0].machine,
        block: initial.functions[0].blocks[0].id,
        node: 0,
    };
    let source = initial.functions[0].blocks[0].nodes[0].provenance[0];
    let mut final_unit = initial.clone();
    final_unit.functions[0].blocks[0].nodes.clear();
    final_unit.identity =
        omega_optimization_unit::recompute_psi_optimization_unit_identity(&final_unit);
    let middle = OptimizationUnitIdentity::from_canonical_bytes(b"custody-middle");
    let removed = custody_record(
        initial.identity,
        middle,
        ProvenanceDisposition::ProvenUnreachableAt(PsiRealizationSite::Node(location)),
        source,
    );
    let resurrected = custody_record(
        middle,
        final_unit.identity,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
        source,
    );
    let ledger = PsiTransformationLedger::new(
        initial.psi,
        initial.fuel_schedule,
        initial.identity,
        final_unit.identity,
        vec![removed, resurrected],
    )
    .unwrap();
    assert_eq!(
        validate_source_custody(&initial, &final_unit, &ledger),
        Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
    );
}
