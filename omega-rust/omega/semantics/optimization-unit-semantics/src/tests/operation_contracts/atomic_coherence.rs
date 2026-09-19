//! Serial reads-from coherence of retained atomic-event edges.

use crate::tests::{id, refresh_identity};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{
    AbstractAtomicEvent, AbstractBlockEntry, AbstractFunction, AbstractFunctionResult,
    AbstractOperation, AbstractOperationPlan, AbstractResult, AtomicReadsFrom,
    AtomicReadsFromViolation,
};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn atomic_unit(load_witness: Option<AtomicReadsFrom>) -> crate::PsiOptimizationUnit {
    let machine = id(31, MachineId::new);
    let block = id(32, BlockId::new);
    let stored = id(33, ValueId::new);
    let observed = id(34, ValueId::new);
    let location = id(35, PlaceId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).expect("valid width");
    let scalar_type = ScalarType::Integer(integer);
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
        },
        entry: machine,
        structural_types: Vec::new().into(),
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
                structural_parameters: Vec::new(),
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: id(36, OperationId::new),
                    result: stored,
                    scalar_type,
                    value: IntegerValue::Unsigned(7),
                },
                AbstractOperation::AtomicEvent {
                    psi_operation: id(37, OperationId::new),
                    event: AbstractAtomicEvent::Store {
                        place: location,
                        ordering: language_core::atomic::MemoryOrdering::NoOrdering,
                        value: stored,
                    },
                    reads_from: None,
                },
                AbstractOperation::AtomicEvent {
                    psi_operation: id(38, OperationId::new),
                    event: AbstractAtomicEvent::Load {
                        place: location,
                        ordering: language_core::atomic::MemoryOrdering::NoOrdering,
                        result: AbstractResult {
                            value: observed,
                            scalar_type,
                        },
                    },
                    reads_from: load_witness,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: id(39, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("valid unit")
}

fn coherent_witness() -> Option<AtomicReadsFrom> {
    Some(AtomicReadsFrom::Write {
        operation: id(37, OperationId::new),
    })
}

#[test]
fn a_coherent_reads_from_edge_validates() {
    let unit = atomic_unit(coherent_witness());
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
}

#[test]
fn refused_edges_fail_validation_with_the_serial_coherence_error() {
    for (witness, violation) in [
        (None, AtomicReadsFromViolation::MissingWitness),
        (
            Some(AtomicReadsFrom::InitialResidency),
            AtomicReadsFromViolation::InitialResidencyAfterWrite,
        ),
        (
            Some(AtomicReadsFrom::Write {
                operation: id(36, OperationId::new),
            }),
            AtomicReadsFromViolation::UnresolvedObservedWrite {
                claimed: id(36, OperationId::new),
            },
        ),
        (
            Some(AtomicReadsFrom::Write {
                operation: id(38, OperationId::new),
            }),
            AtomicReadsFromViolation::UnresolvedObservedWrite {
                claimed: id(38, OperationId::new),
            },
        ),
    ] {
        let unit = atomic_unit(witness);
        assert_eq!(
            validate_psi_optimization_unit(&unit),
            Err(
                OptimizationUnitValidationError::AtomicEventCoherenceMismatch {
                    machine: id(31, MachineId::new),
                    block: id(32, BlockId::new),
                    node: 2,
                    violation,
                }
            ),
            "witness {witness:?} must refuse as {violation:?}"
        );
    }
}

#[test]
fn a_store_cannot_carry_a_reads_from_edge() {
    let mut unit = atomic_unit(coherent_witness());
    let AbstractOperation::AtomicEvent { reads_from, .. } =
        &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture second node is the store");
    };
    *reads_from = Some(AtomicReadsFrom::InitialResidency);
    refresh_identity(&mut unit);
    assert_eq!(
        validate_psi_optimization_unit(&unit),
        Err(
            OptimizationUnitValidationError::AtomicEventCoherenceMismatch {
                machine: id(31, MachineId::new),
                block: id(32, BlockId::new),
                node: 1,
                violation: AtomicReadsFromViolation::NonObservingWitness,
            }
        )
    );
}
