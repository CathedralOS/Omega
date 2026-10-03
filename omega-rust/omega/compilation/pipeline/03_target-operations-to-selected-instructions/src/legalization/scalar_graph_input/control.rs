//! Exact source transfers, with edge-local fuel and no discarded ownership work.
use super::{
    AbstractFunctionResult, AbstractOperation, OptimizationNode, PsiOptimizationFunction,
    PsiProvenance, ScalarType,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::value_type;
pub(super) fn validate(
    node: &OptimizationNode,
    _body: &[OptimizationNode],
    function: &PsiOptimizationFunction,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    match (&node.operation, &function.result) {
        (AbstractOperation::Crash { psi_edge, .. }, _) => terminal_edge(node, *psi_edge),
        (AbstractOperation::StructuralCase { .. }, _) => {
            super::structural_case::validate(node, function)
        }
        (
            AbstractOperation::ReturnStructural {
                psi_edge,
                source,
                returned_claims,
                trivial_affine_locals,
                trivial_affine_discards,
            },
            AbstractFunctionResult::Structural(declared),
        ) => {
            let cleanup_actions = trivial_affine_discards
                .iter()
                .copied()
                .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                .collect::<Vec<_>>();
            if terminal_semantics::descriptor_results::is_shared_descriptor(
                structural_types.iter(),
                declared.structural_type,
            ) {
                if !super::byte_views::shared_return(function, *source, structural_types)
                    || !returned_claims.is_empty()
                    || !trivial_affine_locals.is_empty()
                    || !super::aggregate_results::cleanup(function, &cleanup_actions)
                {
                    return Err(LegalizationError::custody());
                }
                return terminal_edge(node, *psi_edge);
            }
            let (structural_type, multiplicity, source_roster_matches) = if let Some(parameter) =
                function
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *source)
            {
                if parameter.access != terminal_psi::StructuralAccess::Owned
                    || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                    || parameter.is_self
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(LegalizationError::custody());
                }
                (
                    parameter.structural_type,
                    parameter.multiplicity,
                    parameter.qualifications == declared.qualifications
                        && parameter.projected_qualifications == declared.projected_qualifications,
                )
            } else {
                match super::structural_case::source_owner(function, *source)? {
                    legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                        result,
                        ..
                    } if result.claims.is_empty() => (
                        result.structural_type,
                        result.multiplicity,
                        returned_roster_matches(&result, declared),
                    ),
                    legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                        ..
                    } => return Err(LegalizationError::custody()),
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        declaration,
                        ..
                    } => (
                        declaration.structural_type,
                        declaration.multiplicity,
                        declaration.qualifications == declared.qualifications
                            && declaration.projected_qualifications
                                == declared.projected_qualifications,
                    ),
                    // `source_owner` never resolves a function parameter.
                    legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                    | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                        ..
                    } => {
                        return Err(LegalizationError::custody());
                    }
                }
            };
            if structural_type != declared.structural_type
                || multiplicity != declared.multiplicity
                || !source_roster_matches
                || (!declared.qualifications.is_empty()
                    && !bounded_owned_byte_result(declared.structural_type, structural_types))
                || !declared.projected_qualifications.is_empty()
                || !returned_claims.is_empty()
                || !trivial_affine_locals.is_empty()
                || !super::aggregate_results::cleanup(function, &cleanup_actions)
            {
                return Err(LegalizationError::custody());
            }
            terminal_edge(node, *psi_edge)
        }
        (
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            },
            AbstractFunctionResult::Unit,
        ) if cleanup_actions.is_empty()
            || (super::aggregate_results::cleanup(function, cleanup_actions)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
            || (super::read_byte::cleanup(function, cleanup_actions)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )])
            || (super::unobserved_owned::body(function)
                && node.ownership
                    == [optimization_unit::OwnershipEvent::Cleanup(
                        cleanup_actions.clone(),
                    )]) =>
        {
            terminal_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            },
            AbstractFunctionResult::Scalar(declared),
        ) if *result == declared.value
            && *scalar_type == declared.scalar_type
            && value_type(function, *value) == Some(*scalar_type)
            && (cleanup_actions.is_empty()
                || (super::aggregate_results::cleanup(function, cleanup_actions)
                    && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )])
                || (super::unobserved_owned::body(function)
                    && node.ownership
                        == [optimization_unit::OwnershipEvent::Cleanup(
                            cleanup_actions.clone(),
                        )])) =>
        {
            terminal_edge(node, *psi_edge)
        }
        (
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
            },
            _,
        ) => {
            let [edge] = node.successors.as_slice() else {
                return Err(LegalizationError::custody());
            };
            if edge.psi_edge != *psi_edge
                || edge.target != *target
                || edge.bindings != *bindings
                || edge.structural_bindings != *structural_bindings
                || edge.trivial_affine_discards != *trivial_affine_discards
                || edge.residual_affine_discards != *residual_affine_discards
            {
                return Err(LegalizationError::custody());
            }
            branch_edges(node, function)
        }
        (
            AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            },
            _,
        ) => {
            if value_type(function, *condition) != Some(ScalarType::Boolean) {
                return Err(LegalizationError::custody());
            }
            if node.successors.len() != 2 {
                return Err(LegalizationError::custody());
            }
            for (actual, expected) in node.successors.iter().zip([when_true, when_false]) {
                if actual.psi_edge != expected.psi_edge
                    || actual.target != expected.target
                    || actual.bindings != expected.bindings
                    || actual.structural_bindings != expected.structural_bindings
                    || actual.trivial_affine_discards != expected.trivial_affine_discards
                {
                    return Err(LegalizationError::custody());
                }
            }
            branch_edges(node, function)
        }
        _ => Err(LegalizationError::custody()),
    }
}

/// Domains minted by the producing call's own establishment rows are
/// discharged evidence that sheds at the return edge, as Terminal return
/// admission sheds them. The roster left after shedding must be exactly the
/// declared result's roster; a retained domain the function does not declare
/// still refuses.
fn returned_roster_matches(
    result: &terminal_psi::StructuralOperationResult,
    declared: &terminal_psi::StructuralResultDeclaration,
) -> bool {
    let minted = |domain: semantic_vocabulary::StructuralDomainId| {
        result
            .qualification_establishments
            .iter()
            .any(|establishment| establishment.domain == domain)
    };
    result
        .qualifications
        .iter()
        .copied()
        .filter(|domain| !minted(*domain))
        .eq(declared.qualifications.iter().copied())
        && result
            .projected_qualifications
            .iter()
            .filter(|projection| !minted(projection.domain))
            .eq(declared.projected_qualifications.iter())
}

/// Root qualifications are semantic custody on the inline owner and do not
/// change its aggregate return shape. A borrowed byte descriptor is a separate
/// ABI family and never satisfies this result judgment.
fn bounded_owned_byte_result(
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                )
            )
    })
}
fn terminal_edge(
    node: &OptimizationNode,
    edge: semantic_vocabulary::EdgeId,
) -> Result<(), LegalizationError> {
    if !node.successors.is_empty()
        || node.provenance != [PsiProvenance::Edge(edge)]
        || node.fuel.is_empty()
        || node
            .fuel
            .iter()
            .any(|fuel| fuel.site != PsiProvenance::Edge(edge))
    {
        return Err(LegalizationError::custody());
    }
    Ok(())
}
fn branch_edges(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    if !node.provenance.is_empty()
        || !node.fuel.is_empty()
        || node.successors.iter().any(|edge| {
            !super::aggregate_results::cleanup(
                function,
                &edge
                    .trivial_affine_discards
                    .iter()
                    .copied()
                    .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                    .chain(
                        edge.residual_affine_discards
                            .iter()
                            .cloned()
                            .map(terminal_psi::TerminalAffineCleanupAction::DiscardResidual),
                    )
                    .collect::<Vec<_>>(),
            ) || edge.provenance != [PsiProvenance::Edge(edge.psi_edge)]
                || edge.fuel.is_empty()
                || edge
                    .fuel
                    .iter()
                    .any(|fuel| fuel.site != PsiProvenance::Edge(edge.psi_edge))
        })
    {
        return Err(LegalizationError::custody());
    }
    Ok(())
}
