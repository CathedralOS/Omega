//! Active-case evidence for call-scoped payload loans in the current CFG.
//! Layout selects bytes; only an unchanged observation on every arrival may
//! authorize interpreting those bytes as the requested case's payload.

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralCaseId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

use crate::OptimizationUnitValidationError;

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            let O::CallStructural {
                structural_arguments,
                ..
            } = &node.operation
            else {
                continue;
            };
            for argument in structural_arguments {
                for (position, segment) in argument.path.iter().enumerate() {
                    let StructuralPathSegment::Case(case) = segment else {
                        continue;
                    };
                    if !active_on_every_path(
                        function,
                        types,
                        predecessors,
                        (block_position, node_position),
                        argument.place,
                        &argument.path[..position],
                        *case,
                    ) {
                        return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                            machine: Some(function.machine),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn active_on_every_path(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    path: &[StructuralPathSegment],
    case: StructuralCaseId,
) -> bool {
    if membership_on_every_path(
        function,
        types,
        predecessors,
        use_site,
        root,
        path,
        case,
        true,
    ) {
        return true;
    }
    // Exhaustive exclusion is over the complete declared roster, not a
    // special two-case complement. Every alternative must remain disproved
    // on every current arrival, even when different guards supply the facts.
    let Some(source) =
        crate::unit_validation::operation_contracts::structural_source_contract(function, root)
    else {
        return false;
    };
    let Some((_, declaration)) = terminal_semantics::canonical_static_projection(
        types.values().copied(),
        source.structural_type,
        path,
    ) else {
        return false;
    };
    let (terminal_psi::StructuralTypeShape::Sum { cases }
    | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
    else {
        return false;
    };
    cases.iter().any(|candidate| candidate.id == case)
        && cases
            .iter()
            .filter(|candidate| candidate.id != case)
            .all(|candidate| {
                membership_on_every_path(
                    function,
                    types,
                    predecessors,
                    use_site,
                    root,
                    path,
                    candidate.id,
                    false,
                )
            })
}

fn membership_on_every_path(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    path: &[StructuralPathSegment],
    case: StructuralCaseId,
    expected: bool,
) -> bool {
    let invalidates = |operation: &O, root| {
        super::structural_freshness::structural_metadata_may_change(
            function, types, root, path, operation,
        )
    };
    super::structural_freshness::established_on_every_path_with_edges(
        function,
        predecessors,
        use_site,
        root,
        |_, operation, root| {
            matches!(operation, O::EstablishCase { result, result_case, .. }
            if result.place == root && path.is_empty() && (*result_case == case) == expected)
        },
        |site, operation, edge, before| match operation {
            O::StructuralCase { source, cases } => {
                *source == before
                    && path.is_empty()
                    && cases.iter().any(|candidate| {
                        candidate.psi_edge == edge.psi_edge && (candidate.case == case) == expected
                    })
            }
            O::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let truth = if when_true.psi_edge == edge.psi_edge {
                    true
                } else if when_false.psi_edge == edge.psi_edge {
                    false
                } else {
                    return false;
                };
                function
                    .blocks
                    .iter()
                    .enumerate()
                    .any(|(observed_block, block)| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .any(|(observed_node, observed)| {
                                let O::StructuralCaseMembership {
                                    result,
                                    source,
                                    path: observed_path,
                                    case: observed_case,
                                    ..
                                } = &observed.operation
                                else {
                                    return false;
                                };
                                if result.value != *condition
                                    || *source != before
                                    || observed_path != path
                                {
                                    return false;
                                }
                                let matches_case = if truth {
                                    (*observed_case == case) == expected
                                } else {
                                    *observed_case == case && !expected
                                };
                                matches_case
                                    && super::structural_freshness::established_on_every_path(
                                        function,
                                        predecessors,
                                        site,
                                        before,
                                        |position, _, current_root| {
                                            position == (observed_block, observed_node)
                                                && current_root == before
                                        },
                                        invalidates,
                                    )
                            })
                    })
            }
            _ => false,
        },
        invalidates,
    )
}
