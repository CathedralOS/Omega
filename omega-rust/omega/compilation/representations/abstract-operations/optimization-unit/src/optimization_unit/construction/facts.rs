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
        AbstractOperation::EstablishCase {
            psi_operation,
            fields,
            ..
        }
        | AbstractOperation::EstablishRecord {
            psi_operation,
            fields,
            ..
        } => {
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
        O::StructuralScalarFieldStore {
            psi_operation,
            range_obligation: Some(obligation),
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
        | O::ByteSequenceStore {
            psi_operation,
            obligation,
            ..
        }
        | O::StructuralByteSequenceFieldStore {
            psi_operation,
            obligation,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            psi_operation,
            obligation,
            ..
        }
        | O::ByteSequenceRead {
            psi_operation,
            obligation,
            ..
        }
        | O::StructuralByteSequenceFieldRead {
            psi_operation,
            obligation,
            ..
        }
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
        | O::ElementViewRead {
            psi_operation,
            obligation,
            ..
        }
        | O::ElementViewWrite {
            psi_operation,
            obligation,
            ..
        }
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
