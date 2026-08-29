//! Independently reconstructed definitions, uses, edges, provenance, and ownership.

use super::*;

pub(crate) fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = block_ids.collect::<BTreeSet<_>>();
    let mut result = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                [entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != entry) {
            let incoming = predecessors.get(&block).expect("all blocks indexed");
            let mut next = if let Some(first) = incoming.first() {
                result[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

pub(crate) fn validate_places_and_claims(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let known_places = reconstruct_declared_places(function)?;
    for parameter in &function.structural_parameters {
        if !function.declared_places.contains(&parameter.place) {
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place: parameter.place,
            });
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    omega_optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | omega_optimization_unit::OwnershipEvent::ClaimCompletion(claims)
                    | omega_optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | omega_optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    omega_optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                };
                for claim in claims {
                    if !function_has_claim(function, *claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
    }
    if known_places != function.declared_places {
        let place = known_places
            .symmetric_difference(&function.declared_places)
            .next()
            .copied()
            .expect("different place sets have a difference");
        return Err(OptimizationUnitValidationError::UnknownPlace {
            machine: function.machine,
            place,
        });
    }
    Ok(())
}

/// Terminal ownership treats ordinary and content entry claims as one live
/// claim namespace while retaining their declarations as distinct authority.
/// `entry_claims` remains the independently checked ordinary-claim index;
/// content-only claims are resolved from their complete retained catalog.
pub(crate) fn function_has_claim(function: &PsiOptimizationFunction, claim: ClaimId) -> bool {
    function.entry_claims.contains(&claim)
        || function
            .content_entry_claims
            .iter()
            .any(|candidate| candidate.claim == claim)
}

pub(crate) fn reconstruct_declared_places(
    function: &PsiOptimizationFunction,
) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        for node in &block.nodes {
            match &node.operation {
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => {
                    known_places.insert(place.id);
                }
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    known_places.insert(result.place);
                }
                _ => {}
            }
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            validate_operation_places(function.machine, &node.operation, &known_places)?;
        }
    }
    Ok(known_places)
}

pub(crate) fn validate_operation_places(
    machine: MachineId,
    operation: &omega_abstract_operations::AbstractOperation,
    known: &BTreeSet<PlaceId>,
) -> Result<(), OptimizationUnitValidationError> {
    use omega_abstract_operations::AbstractOperation as O;
    let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
        if known.contains(&place) {
            Ok(())
        } else {
            Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
        }
    };
    match operation {
        O::EstablishByteSequenceLiteral { .. } | O::EstablishTrivialAffineLocal { .. } => {}
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            require(*source, known)?;
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                let place = match cleanup {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        discard.place
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        cleanup.place
                    }
                };
                require(place, known)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn expected_definitions(
    operation: &omega_abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    use omega_abstract_operations::AbstractOperation as O;
    let definition = match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    };
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

pub(crate) fn expected_uses(
    operation: &omega_abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use omega_abstract_operations::AbstractOperation as O;
    let values = match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}

pub(crate) fn expected_provenance(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<PsiProvenance> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump { .. } | O::Conditional { .. } => Vec::new(),
        O::Return { psi_edge, .. } | O::ReturnUnit { psi_edge, .. } | O::Crash { psi_edge, .. } => {
            vec![PsiProvenance::Edge(*psi_edge)]
        }
        O::ReturnStructural {
            psi_edge,
            trivial_affine_locals,
            ..
        } => {
            // This is deliberately primary-site-first custody order rather
            // than execution order. The return edge anchors the node; hidden
            // establishments follow in their exact tuple order.
            std::iter::once(PsiProvenance::Edge(*psi_edge))
                .chain(
                    trivial_affine_locals
                        .iter()
                        .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                )
                .collect()
        }
        O::EstablishPayloadlessCase { psi_operation, .. }
        | O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::CallUnit { psi_operation, .. }
        | O::CallStructuralScalar { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::BooleanStructuralField { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. } => {
            vec![PsiProvenance::Operation(*psi_operation)]
        }
    }
}

pub(crate) fn provenance_matches_operation(
    operation: &omega_abstract_operations::AbstractOperation,
    provenance: &[PsiProvenance],
) -> bool {
    let expected = expected_provenance(operation);
    if expected.is_empty() {
        matches!(operation, O::Jump { .. } | O::Conditional { .. }) || provenance.is_empty()
    } else {
        provenance.starts_with(&expected)
    }
}

pub(crate) fn successors_match_operation(
    operation: &omega_abstract_operations::AbstractOperation,
    actual: &[OptimizationEdge],
) -> bool {
    let expected = expected_edges(operation);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.psi_edge == expected.psi_edge
                && actual.target == expected.target
                && actual.bindings == expected.bindings
                && actual.trivial_affine_discards == expected.trivial_affine_discards
                && actual.provenance.first() == Some(&PsiProvenance::Edge(actual.psi_edge))
                && actual
                    .provenance
                    .iter()
                    .all(|source| matches!(source, PsiProvenance::Edge(_)))
        })
}

pub(crate) fn expected_edges(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![omega_optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| OptimizationEdge {
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                trivial_affine_discards: edge.trivial_affine_discards.clone(),
                provenance: vec![PsiProvenance::Edge(edge.psi_edge)],
                fuel: vec![omega_optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(edge.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn expected_ownership(
    operation: &omega_abstract_operations::AbstractOperation,
) -> Vec<OwnershipEvent> {
    use omega_abstract_operations::AbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        }
        | O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => vec![OwnershipEvent::Cleanup(cleanup_actions.clone())],
        O::ReturnStructural {
            returned_claims, ..
        } => vec![OwnershipEvent::StructuralReturn(returned_claims.clone())],
        O::Crash {
            frontier_lower_bound,
            ..
        } => vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())],
        _ => Vec::new(),
    }
}

pub(crate) fn is_terminator(operation: &omega_abstract_operations::AbstractOperation) -> bool {
    use omega_abstract_operations::AbstractOperation as O;
    matches!(
        operation,
        O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::ReturnStructural { .. }
            | O::Crash { .. }
    )
}
