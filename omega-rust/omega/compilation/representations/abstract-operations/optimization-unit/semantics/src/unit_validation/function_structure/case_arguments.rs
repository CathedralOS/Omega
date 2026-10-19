//! Active-case evidence for payload loans and projected dispatch in the current CFG.
//! Layout selects bytes; only an unchanged observation on every arrival may
//! authorize interpreting those bytes as the requested case's payload.

use crate::unit_validation::catalog_mismatch;
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
            let validate_projection = |root, path: &[StructuralPathSegment]| {
                for (position, segment) in path.iter().enumerate() {
                    let StructuralPathSegment::Case(case) = segment else {
                        continue;
                    };
                    if !active_on_every_path(
                        function,
                        types,
                        predecessors,
                        (block_position, node_position),
                        root,
                        &path[..position],
                        *case,
                    ) {
                        return Err(catalog_mismatch(function));
                    }
                }
                Ok(())
            };
            match &node.operation {
                O::CallStructural {
                    structural_arguments,
                    ..
                } => {
                    for argument in structural_arguments {
                        validate_projection(argument.place, &argument.path)?;
                    }
                }
                O::StructuralCase { source, path, .. } => {
                    validate_projection(*source, path)?;
                }
                _ => {}
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
    // The roster depends on the declared endpoint type, not a canonical fact
    // key. Reference crossings and runtime selectors retain their exact path
    // in the freshness judgment rather than gaining canonical storage identity.
    let Some(declaration) = terminal_semantics::runtime_structural_path_tip(
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

pub(crate) fn membership_on_every_path(
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
    super::structural_freshness::established_on_every_path_with_entry(
        function,
        predecessors,
        use_site,
        root,
        |_, operation, root| {
            matches!(operation, O::EstablishCase { result, result_case, .. }
            if result.place == root && path.is_empty() && (*result_case == case) == expected)
        },
        |site, operation, edge, before| match operation {
            O::StructuralCase {
                source,
                path: selected_path,
                cases,
            } => {
                *source == before
                    && path == selected_path
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
        |root| entry_membership(function, types, root, path, case, expected),
        invalidates,
    )
}

/// The caller owes verified entry preconditions. They establish a tag fact at
/// entry only; the common freshness walk rejects intervening writes, unknown
/// aliases, or arrivals that do not preserve that same storage.
fn entry_membership(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    case: StructuralCaseId,
    expected: bool,
) -> bool {
    use semantic_vocabulary::Proposition;
    let Some(contract) = &function.verified_contract else {
        return false;
    };
    let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)
    else {
        return false;
    };
    let matches_subject = |subject: &semantic_vocabulary::StructuralCaseSubject| {
        subject.root() == root
            && terminal_semantics::canonical_structural_storage_path(
                types.values().copied(),
                parameter.structural_type,
                subject.path(),
            )
            .as_deref()
                == Some(path)
    };
    fn entails(
        proposition: &Proposition,
        matches_subject: &impl Fn(&semantic_vocabulary::StructuralCaseSubject) -> bool,
        case: StructuralCaseId,
        expected: bool,
    ) -> bool {
        match proposition {
            Proposition::StructuralCaseMembership {
                subject,
                case: selected,
            } => matches_subject(subject) && (*selected == case) == expected,
            Proposition::Conjunction(children) => children
                .iter()
                .any(|child| entails(child, matches_subject, case, expected)),
            Proposition::Disjunction(children) => {
                !children.is_empty()
                    && children
                        .iter()
                        .all(|child| entails(child, matches_subject, case, expected))
            }
            Proposition::Implication {
                premise,
                conclusion,
            } if matches!(conclusion.as_ref(), Proposition::Falsehood) => {
                !expected
                    && matches!(premise.as_ref(),
                Proposition::StructuralCaseMembership { subject, case: excluded }
                    if *excluded == case && matches_subject(subject))
            }
            _ => false,
        }
    }
    contract
        .requires
        .iter()
        .any(|proposition| entails(proposition, &matches_subject, case, expected))
}
