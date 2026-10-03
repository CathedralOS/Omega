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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::structural_catalog::direct_realization_boolean_structural_field_unit;
    use crate::tests::support::id;
    use abstract_operations::{AbstractResult, AbstractStructuralCallResult, AbstractSuccessor};
    use optimization_unit::{OptimizationBlock, OptimizationNode};
    use semantic_vocabulary::{EdgeId, OperationId, ScalarType, ValueId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralTypeShape,
    };

    fn predecessors(function: &PsiOptimizationFunction) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
        let mut predecessors = BTreeMap::<_, BTreeSet<_>>::new();
        for block in &function.blocks {
            for edge in block.nodes.iter().flat_map(|node| &node.successors) {
                predecessors
                    .entry(edge.target)
                    .or_default()
                    .insert(block.id);
            }
        }
        predecessors
    }

    #[test]
    fn case_facts_replay_true_edges_and_exhaustive_exclusion_without_stale_observations() {
        let unit = direct_realization_boolean_structural_field_unit();
        let mut function = unit.functions[0].clone();
        let root = function.structural_parameters[0].place;
        let root_type = function.structural_parameters[0].structural_type;
        let cases = [101, 102, 103].map(|raw| id(raw, StructuralCaseId::new));
        let declarations = [StructuralTypeDeclaration {
            id: root_type,
            identity: "ThreeCases".into(),
            shape: StructuralTypeShape::Sum {
                cases: cases
                    .iter()
                    .map(|case| StructuralCaseDeclaration {
                        id: *case,
                        identity: format!("Case{}", case.get()),
                        fields: Vec::new(),
                    })
                    .collect(),
            },
        }];
        let types = declarations
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let template = function.blocks[0].nodes[0].clone();
        let make_node = |operation: O| {
            let mut node: OptimizationNode = template.clone();
            node.successors = crate::unit_validation::derived_metadata::expected_edges(&operation);
            node.operation = operation;
            node
        };
        let block_ids = [201, 202, 203, 204, 205].map(|raw| id(raw, BlockId::new));
        let edge = |identity, target| AbstractSuccessor {
            psi_edge: id(identity, EdgeId::new),
            target,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let observation = |raw, case| {
            make_node(O::StructuralCaseMembership {
                psi_operation: id(raw, OperationId::new),
                result: AbstractResult {
                    value: id(raw, ValueId::new),
                    scalar_type: ScalarType::Boolean,
                },
                source: root,
                path: Vec::new(),
                case,
            })
        };
        let terminal = function.blocks[0].nodes.last().unwrap().clone();
        function.entry = block_ids[0];
        function.blocks = vec![
            OptimizationBlock {
                id: block_ids[0],
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                nodes: vec![
                    observation(301, cases[0]),
                    make_node(O::Conditional {
                        condition: id(301, ValueId::new),
                        when_true: edge(401, block_ids[1]),
                        when_false: edge(402, block_ids[2]),
                    }),
                ],
            },
            OptimizationBlock {
                id: block_ids[1],
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                nodes: vec![terminal.clone()],
            },
            OptimizationBlock {
                id: block_ids[2],
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                nodes: vec![
                    observation(302, cases[1]),
                    make_node(O::Conditional {
                        condition: id(302, ValueId::new),
                        when_true: edge(403, block_ids[3]),
                        when_false: edge(404, block_ids[4]),
                    }),
                ],
            },
            OptimizationBlock {
                id: block_ids[3],
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                nodes: vec![terminal.clone()],
            },
            OptimizationBlock {
                id: block_ids[4],
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                nodes: vec![terminal],
            },
        ];
        let admitted = |function: &PsiOptimizationFunction, block, case| {
            active_on_every_path(
                function,
                &types,
                &predecessors(function),
                (block, 0),
                root,
                &[],
                case,
            )
        };
        assert!(admitted(&function, 1, cases[0]));
        assert!(admitted(&function, 3, cases[1]));
        assert!(
            admitted(&function, 4, cases[2]),
            "two false guards exhaust a three-case roster"
        );
        assert!(!admitted(&function, 4, cases[0]));
        assert!(!admitted(&function, 4, cases[1]));

        let mut wrong_subject = function.clone();
        let O::StructuralCaseMembership { source, .. } =
            &mut wrong_subject.blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        *source = id(root.get() + 1000, PlaceId::new);
        assert!(!admitted(&wrong_subject, 4, cases[2]));
        let mut wrong_path = function.clone();
        let O::StructuralCaseMembership { path, .. } = &mut wrong_path.blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        path.push(StructuralPathSegment::Field("sibling".into()));
        assert!(!admitted(&wrong_path, 4, cases[2]));
        let mut wrong_edge = function.clone();
        let O::Conditional {
            when_true,
            when_false,
            ..
        } = &mut wrong_edge.blocks[2].nodes[1].operation
        else {
            unreachable!()
        };
        std::mem::swap(when_true, when_false);
        assert!(!admitted(&wrong_edge, 4, cases[2]));

        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut changed = function.clone();
            changed.blocks[0].nodes.insert(
                1,
                make_node(O::CallStructural {
                    psi_operation: id(501, OperationId::new),
                    callee: function.machine,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: root,
                        path: Vec::new(),
                        access,
                    }],
                    dynamic_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    result: AbstractStructuralCallResult::Unit,
                }),
            );
            assert_eq!(
                admitted(&changed, 4, cases[2]),
                access == StructuralAccess::SharedBorrow,
                "only an unchanged observation remains evidence after the call"
            );
        }
    }
}
