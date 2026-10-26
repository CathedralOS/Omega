use super::super::{AbstractOperation, ObligationId, OperationId, OptimizationFact};

pub(super) fn collect_fact(operation: &AbstractOperation, facts: &mut Vec<OptimizationFact>) {
    if let Some((obligation, support)) = operation_obligation(operation) {
        facts.push(OptimizationFact::OperationObligationReference {
            obligation,
            support,
        });
    }
    // Every runtime-selected path element carries a bound the operation
    // owns; the surviving-frontier check rejoins each to its verified owner.
    if let Some(support) = operation.psi_operation() {
        facts.extend(
            operation
                .runtime_indices()
                .into_iter()
                .map(
                    |(_, obligation)| OptimizationFact::OperationObligationReference {
                        obligation,
                        support,
                    },
                ),
        );
    }
    match operation {
        abstract_operations::establish_record_or_case_pattern!(psi_operation, fields, ..) => {
            facts.extend(fields.iter().filter_map(|initializer| {
                let terminal_psi::RecordFieldValue::Scalar {
                    range_obligation, ..
                } = &initializer.value
                else {
                    return None;
                };
                range_obligation.map(
                    |obligation| OptimizationFact::OperationObligationReference {
                        obligation,
                        support: *psi_operation,
                    },
                )
            }));
        }

        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => facts.push(OptimizationFact::BooleanConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            value,
            ..
        } => facts.push(OptimizationFact::IntegerConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        _ => {}
    }
}

fn operation_obligation(operation: &AbstractOperation) -> Option<(ObligationId, OperationId)> {
    use AbstractOperation as O;
    match operation {
        O::EstablishPrimitiveLocal {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::WriteOnlyPrimitiveStore {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::StructuralScalarFieldStore {
            psi_operation,
            range_obligation: Some(obligation),
            ..
        }
        | O::AtomicEvent {
            psi_operation,
            resident_obligation: Some(obligation),
            ..
        }
        | O::StructuralByteViewFieldStore {
            psi_operation,
            predicate_obligation: Some(obligation),
            ..
        }
        | O::EstablishByteSequenceConcatenation {
            psi_operation,
            capacity_obligation: obligation,
            ..
        }
        | O::ByteSequenceSubslice {
            psi_operation,
            obligation,
            ..
        }
        | O::ByteSequenceWrite {
            psi_operation,
            obligation,
            ..
        }
        | abstract_operations::byte_sequence_store_pair_pattern!(psi_operation, obligation, ..)
        | O::StructuralByteSequenceFieldByteStore {
            psi_operation,
            obligation,
            ..
        }
        | abstract_operations::byte_read_or_field_read_pattern!(psi_operation, obligation, ..)
        | O::ElementViewSubslice {
            psi_operation,
            obligation,
            ..
        }
        | O::EstablishElementView {
            psi_operation,
            region: terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. },
            ..
        }
        | abstract_operations::element_view_read_write_pair_pattern!(
            psi_operation,
            obligation,
            ..
        )
        | O::IntegerExactCast {
            psi_operation,
            obligation,
            ..
        } => Some((*obligation, *psi_operation)),
        O::IntegerShift {
            psi_operation,
            policy,
            ..
        } => policy
            .obligation()
            .map(|obligation| (obligation, *psi_operation)),
        O::IntegerArithmetic {
            psi_operation,
            arithmetic,
            ..
        } => arithmetic
            .obligation()
            .map(|obligation| (obligation, *psi_operation)),
        _ => None,
    }
}
