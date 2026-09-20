//! Reads-from coherence of retained atomic-event edges under the bounded
//! `happens_before` derivation.

use crate::tests::{id, refresh_identity};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractBlockEntry, AbstractFunction,
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, AbstractResult,
    AtomicReadsFrom, AtomicReadsFromViolation,
};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn stored_value() -> ValueId {
    id(33, ValueId::new)
}

fn observed_value() -> ValueId {
    id(34, ValueId::new)
}

fn location() -> PlaceId {
    id(35, PlaceId::new)
}

fn integer_constant(psi_operation: u64, result: ValueId) -> AbstractOperation {
    AbstractOperation::IntegerConstant {
        psi_operation: id(psi_operation, OperationId::new),
        result,
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 32).expect("valid width"),
        ),
        value: IntegerValue::Unsigned(7),
    }
}

fn store(psi_operation: u64, place: PlaceId, value: ValueId) -> AbstractOperation {
    AbstractOperation::AtomicEvent {
        psi_operation: id(psi_operation, OperationId::new),
        event: AbstractAtomicEvent::Store {
            place,
            ordering: language_core::atomic::MemoryOrdering::NoOrdering,
            value,
        },
        reads_from: None,
    }
}

fn load(
    psi_operation: u64,
    place: PlaceId,
    result: ValueId,
    reads_from: Option<AtomicReadsFrom>,
) -> AbstractOperation {
    AbstractOperation::AtomicEvent {
        psi_operation: id(psi_operation, OperationId::new),
        event: AbstractAtomicEvent::Load {
            place,
            ordering: language_core::atomic::MemoryOrdering::NoOrdering,
            result: AbstractResult {
                value: result,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 32).expect("valid width"),
                ),
            },
        },
        reads_from,
    }
}

fn fence(psi_operation: u64, ordering: AbstractAtomicFenceOrdering) -> AbstractOperation {
    AbstractOperation::AtomicEvent {
        psi_operation: id(psi_operation, OperationId::new),
        event: AbstractAtomicEvent::Fence { ordering },
        reads_from: None,
    }
}

fn jump(psi_edge: u64, target: u64) -> AbstractOperation {
    AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(psi_edge, EdgeId::new),
        target: id(target, BlockId::new),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn boolean_constant(psi_operation: u64, result: ValueId) -> AbstractOperation {
    AbstractOperation::BooleanConstant {
        psi_operation: id(psi_operation, OperationId::new),
        result,
        value: true,
    }
}

fn conditional(
    condition: ValueId,
    true_edge: u64,
    true_target: u64,
    false_edge: u64,
    false_target: u64,
) -> AbstractOperation {
    let successor = |edge, target| abstract_operations::AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    AbstractOperation::Conditional {
        condition,
        when_true: successor(true_edge, true_target),
        when_false: successor(false_edge, false_target),
    }
}

fn return_unit(psi_edge: u64) -> AbstractOperation {
    AbstractOperation::ReturnUnit {
        psi_edge: id(psi_edge, EdgeId::new),
        cleanup_actions: Vec::new(),
    }
}

/// One function whose `block_entries` partition `operations` into the
/// `(raw block id, operation count)` spans, entry first.
fn atomic_cfg_unit(
    entry: u64,
    blocks: &[(u64, usize)],
    operations: Vec<AbstractOperation>,
) -> crate::PsiOptimizationUnit {
    let machine = id(31, MachineId::new);
    let mut operation_offset = 0;
    let block_entries = blocks
        .iter()
        .map(|(raw, count)| {
            let entry = AbstractBlockEntry {
                structural_parameters: Vec::new(),
                block: id(*raw, BlockId::new),
                parameters: Vec::new(),
                operation_offset,
            };
            operation_offset += count;
            entry
        })
        .collect();
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
            entry: id(entry, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries,
            operations,
        }],
    };
    reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("valid unit")
}

fn atomic_unit(load_witness: Option<AtomicReadsFrom>) -> crate::PsiOptimizationUnit {
    atomic_cfg_unit(
        32,
        &[(32, 4)],
        vec![
            integer_constant(36, stored_value()),
            store(37, location(), stored_value()),
            load(38, location(), observed_value(), load_witness),
            return_unit(39),
        ],
    )
}

fn coherent_witness() -> Option<AtomicReadsFrom> {
    Some(AtomicReadsFrom::Write {
        operation: id(37, OperationId::new),
    })
}

fn coherence_mismatch(
    block: u64,
    node: u32,
    violation: AtomicReadsFromViolation,
) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::AtomicEventCoherenceMismatch {
        machine: id(31, MachineId::new),
        block: id(block, BlockId::new),
        node,
        violation,
    }
}

#[test]
fn a_coherent_reads_from_edge_validates() {
    let unit = atomic_unit(coherent_witness());
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
}

#[test]
fn refused_edges_fail_validation_with_the_coherence_error() {
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
            AtomicReadsFromViolation::WriteOutsideModificationOrder {
                claimed: id(38, OperationId::new),
            },
        ),
    ] {
        let unit = atomic_unit(witness);
        assert_eq!(
            validate_psi_optimization_unit(&unit),
            Err(coherence_mismatch(32, 2, violation)),
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
        Err(coherence_mismatch(
            32,
            1,
            AtomicReadsFromViolation::NonObservingWitness
        ))
    );
}

#[test]
fn a_coherent_reads_from_edge_resolves_across_blocks() {
    let unit = atomic_cfg_unit(
        32,
        &[(32, 3), (40, 2)],
        vec![
            integer_constant(36, stored_value()),
            store(37, location(), stored_value()),
            jump(41, 40),
            load(38, location(), observed_value(), coherent_witness()),
            return_unit(39),
        ],
    );
    assert_eq!(
        validate_psi_optimization_unit(&unit),
        Ok(()),
        "a write in the dominating predecessor happens before the load"
    );
}

#[test]
fn a_witness_must_happen_before_the_observation() {
    // A write on one branch of a diamond never happens before the join's
    // observation; nor does a write in the observation's own successor.
    let condition = id(42, ValueId::new);
    for (witness, violation) in [
        (
            Some(AtomicReadsFrom::Write {
                operation: id(44, OperationId::new),
            }),
            AtomicReadsFromViolation::ObservedWriteNotHappensBefore {
                claimed: id(44, OperationId::new),
            },
        ),
        (
            Some(AtomicReadsFrom::InitialResidency),
            AtomicReadsFromViolation::InitialResidencyAfterWrite,
        ),
    ] {
        let unit = atomic_cfg_unit(
            50,
            &[(50, 2), (51, 2), (52, 1), (53, 2)],
            vec![
                boolean_constant(43, condition),
                conditional(condition, 54, 51, 55, 52),
                store(44, location(), stored_value()),
                jump(56, 53),
                jump(57, 53),
                load(38, location(), observed_value(), witness),
                return_unit(39),
            ],
        );
        assert_eq!(
            validate_psi_optimization_unit(&unit),
            Err(coherence_mismatch(53, 0, violation)),
            "witness {witness:?} must refuse as {violation:?}"
        );
    }
    let successor_write = atomic_cfg_unit(
        32,
        &[(32, 3), (40, 2)],
        vec![
            integer_constant(36, stored_value()),
            load(
                38,
                location(),
                observed_value(),
                Some(AtomicReadsFrom::Write {
                    operation: id(58, OperationId::new),
                }),
            ),
            jump(41, 40),
            store(58, location(), stored_value()),
            return_unit(39),
        ],
    );
    assert_eq!(
        validate_psi_optimization_unit(&successor_write),
        Err(coherence_mismatch(
            32,
            1,
            AtomicReadsFromViolation::ObservedWriteNotHappensBefore {
                claimed: id(58, OperationId::new),
            }
        )),
        "a write in the observer's successor block never happens before it"
    );
}

#[test]
fn only_the_modification_order_latest_write_on_every_path_may_be_observed() {
    // A linear chain writes the place twice; a claim on the first write is
    // stale even though it happens before the observation.
    let chain = |witness| {
        atomic_cfg_unit(
            60,
            &[(60, 3), (61, 2), (62, 2)],
            vec![
                integer_constant(36, stored_value()),
                store(63, location(), stored_value()),
                jump(66, 61),
                store(64, location(), stored_value()),
                jump(67, 62),
                load(38, location(), observed_value(), witness),
                return_unit(39),
            ],
        )
    };
    assert_eq!(
        validate_psi_optimization_unit(&chain(coherent_witness_for(64))),
        Ok(()),
        "the latest write on the only path is coherent"
    );
    for (witness, violation) in [
        (
            Some(AtomicReadsFrom::Write {
                operation: id(63, OperationId::new),
            }),
            AtomicReadsFromViolation::ObservedWriteOverwritten {
                claimed: id(63, OperationId::new),
            },
        ),
        (
            Some(AtomicReadsFrom::InitialResidency),
            AtomicReadsFromViolation::InitialResidencyAfterWrite,
        ),
    ] {
        assert_eq!(
            validate_psi_optimization_unit(&chain(witness)),
            Err(coherence_mismatch(62, 0, violation)),
            "witness {witness:?} must refuse as {violation:?}"
        );
    }
}

fn coherent_witness_for(operation: u64) -> Option<AtomicReadsFrom> {
    Some(AtomicReadsFrom::Write {
        operation: id(operation, OperationId::new),
    })
}

#[test]
fn writes_converging_on_distinct_branches_admit_no_edge() {
    // Both branches write the place: neither is happens-before the join,
    // a dominating write is stale, and the residency is never initial.
    let condition = id(42, ValueId::new);
    let diamond = |witness| {
        atomic_cfg_unit(
            50,
            &[(50, 3), (51, 2), (52, 2), (53, 2)],
            vec![
                store(68, location(), stored_value()),
                boolean_constant(43, condition),
                conditional(condition, 54, 51, 55, 52),
                store(69, location(), stored_value()),
                jump(56, 53),
                store(70, location(), stored_value()),
                jump(57, 53),
                load(38, location(), observed_value(), witness),
                return_unit(39),
            ],
        )
    };
    for (claimed, violation) in [
        (
            68,
            AtomicReadsFromViolation::ObservedWriteOverwritten {
                claimed: id(68, OperationId::new),
            },
        ),
        (
            69,
            AtomicReadsFromViolation::ObservedWriteNotHappensBefore {
                claimed: id(69, OperationId::new),
            },
        ),
        (
            70,
            AtomicReadsFromViolation::ObservedWriteNotHappensBefore {
                claimed: id(70, OperationId::new),
            },
        ),
    ] {
        let witness = coherent_witness_for(claimed);
        assert_eq!(
            validate_psi_optimization_unit(&diamond(witness)),
            Err(coherence_mismatch(53, 0, violation)),
            "witness {witness:?} must refuse as {violation:?}"
        );
    }
    assert_eq!(
        validate_psi_optimization_unit(&diamond(Some(AtomicReadsFrom::InitialResidency))),
        Err(coherence_mismatch(
            53,
            0,
            AtomicReadsFromViolation::InitialResidencyAfterWrite
        )),
        "every path wrote the place before the join"
    );
}

#[test]
fn a_fence_neither_writes_nor_disturbs_the_order() {
    // A sequenced fence between the write and the observer leaves the
    // claimed write the modification-order tail on every path.
    let unit = atomic_cfg_unit(
        32,
        &[(32, 4), (40, 2)],
        vec![
            integer_constant(36, stored_value()),
            store(37, location(), stored_value()),
            fence(72, AbstractAtomicFenceOrdering::Publish),
            jump(41, 40),
            load(38, location(), observed_value(), coherent_witness()),
            return_unit(39),
        ],
    );
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
    // A fence resolves as an event but writes nothing, so a witness
    // naming it stands outside the modification order.
    let claims_fence = atomic_cfg_unit(
        32,
        &[(32, 4), (40, 2)],
        vec![
            integer_constant(36, stored_value()),
            store(37, location(), stored_value()),
            fence(72, AbstractAtomicFenceOrdering::Receive),
            jump(41, 40),
            load(38, location(), observed_value(), coherent_witness_for(72)),
            return_unit(39),
        ],
    );
    assert_eq!(
        validate_psi_optimization_unit(&claims_fence),
        Err(coherence_mismatch(
            40,
            0,
            AtomicReadsFromViolation::WriteOutsideModificationOrder {
                claimed: id(72, OperationId::new),
            }
        ))
    );
    // And a fence observes nothing, so it cannot carry an edge either.
    let mut witnessing_fence = unit;
    let AbstractOperation::AtomicEvent { reads_from, .. } =
        &mut witnessing_fence.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("fixture third node is the fence");
    };
    *reads_from = coherent_witness();
    refresh_identity(&mut witnessing_fence);
    assert_eq!(
        validate_psi_optimization_unit(&witnessing_fence),
        Err(coherence_mismatch(
            32,
            2,
            AtomicReadsFromViolation::NonObservingWitness
        ))
    );
}
