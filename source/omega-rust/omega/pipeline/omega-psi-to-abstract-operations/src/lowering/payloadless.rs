use crate::shared::*;

/// Reconstruct the same closed leaf producer recognized by Terminal. This is
/// used only after `VerifiedTerminalModule` admission, so proposition-root and
/// evidence-row validity have already been checked by Terminal; the optimizer
/// projection still retains those rows for its independent validator.
pub(super) fn exact_payloadless_case_return_exits(machine: &TerminalMachine) -> bool {
    let Some(result) = machine.result.structural() else {
        return false;
    };
    if !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            })
    {
        return false;
    }
    let mut exits = 0_usize;
    for block in &machine.blocks {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = machine.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == result.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(operation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
        else {
            return false;
        };
        let Some(operation_result) = operation.result.structural() else {
            return false;
        };
        if !matches!(
            operation.kind,
            OperationKind::EstablishPayloadlessCase { .. }
        ) || operation_result.place != *source
            || operation_result.structural_type != result.structural_type
            || operation_result.multiplicity != StructuralMultiplicity::Unrestricted
            || !operation_result.claims.is_empty()
            || !operation_result.qualifications.is_empty()
            || !operation_result.projected_qualifications.is_empty()
        {
            return false;
        }
        exits += 1;
    }
    exits != 0
}

/// Exact union of Terminal's `proposition_boolean_field_roots` and
/// `proposition_content_roots` projections.
pub(super) fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                roots.insert(*root);
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_roots(operand, roots),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                scalar_term_roots(value, roots);
                scalar_term_roots(count, roots);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn content_term_roots(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                roots.insert(subject.root);
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    content_term_roots(term, roots);
                }
            }
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::ByteSequenceEqual { left, right } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                roots.insert(subject.root());
            }
            Proposition::ContentConservation(conservation) => {
                content_term_roots(conservation.left(), roots);
                content_term_roots(conservation.right(), roots);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, roots);
                collect(conclusion, roots);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
}

pub(super) fn exact_payloadless_structural_call(
    module: &psi_terminal::TerminalModule,
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    let OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
    } = &operation.kind
    else {
        return false;
    };
    let Some(result) = operation.result.structural() else {
        return false;
    };
    let Some(callee) = machines.get(callee).copied() else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.requires.is_empty()
        && callee.contract.ensures.is_empty()
        && callee.contract.crash_routes.is_empty()
        && module
            .evidence_contract_lanes
            .iter()
            .all(|lane| lane.machine != callee.id)
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && result.qualifications.is_empty()
        && result.qualifications == callee_result.qualifications
        && result.projected_qualifications.is_empty()
        && result.projected_qualifications == callee_result.projected_qualifications
        && result.claims.is_empty()
        && callee.contract.outcome_specific_ensures.iter().all(|row| {
            proposition_structural_roots(&row.proposition)
                .into_iter()
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee)
}

pub(super) fn exact_unrestricted_payloadless_result(
    module: &psi_terminal::TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> bool {
    let Some(result) = machine.result.structural() else {
        return false;
    };
    result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && machine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::ReturnStructural { .. }))
        && machine.blocks.iter().all(|block| {
            let Terminator::ReturnStructural {
                source,
                returned_claims,
                ..
            } = &block.terminator
            else {
                return true;
            };
            returned_claims.is_empty()
                && machine
                    .structural_places
                    .iter()
                    .find(|place| place.id == *source)
                    .and_then(|place| match place.kind {
                        StructuralPlaceKind::OperationResult { producer, .. } => machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| operation.id == producer),
                        _ => None,
                    })
                    .is_some_and(|operation| {
                        (matches!(
                            operation.kind,
                            OperationKind::EstablishPayloadlessCase { .. }
                        ) || exact_payloadless_structural_call(module, operation, machines))
                            && operation
                                .result
                                .structural()
                                .is_some_and(|operation_result| {
                                    operation_result.place == *source
                                        && operation_result.structural_type
                                            == result.structural_type
                                        && operation_result.multiplicity
                                            == StructuralMultiplicity::Unrestricted
                                        && operation_result.qualifications.is_empty()
                                        && operation_result.projected_qualifications.is_empty()
                                        && operation_result.claims.is_empty()
                                })
                    })
        })
        && machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::BoundaryCall { .. }
                ) && (!matches!(operation.kind, OperationKind::CallStructural { .. })
                    || exact_payloadless_structural_call(module, operation, machines))
            })
}
