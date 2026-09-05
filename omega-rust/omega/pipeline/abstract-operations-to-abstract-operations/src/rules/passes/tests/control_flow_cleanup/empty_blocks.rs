//! Linear and path-qualified empty-block threading.

use super::*;

#[test]
fn linear_empty_block_thread_composes_bindings_and_realizes_both_edges() {
    let unit = linear_empty_block_unit();
    let contract = LinearEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = LinearEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("linear jump block is threadable");
    assert_eq!(
        candidate.affected_blocks(),
        [
            id(902, BlockId::new),
            id(903, BlockId::new),
            id(904, BlockId::new),
        ]
    );
    assert_eq!(candidate.provenance().len(), 3);
    assert!(
        candidate
            .provenance()
            .iter()
            .all(|row| row.disposition.is_realized())
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| {
                matches!(row.input, PsiRealizationSite::Edge { .. })
                    && row.disposition.site()
                        == PsiRealizationSite::Edge {
                            machine: id(901, MachineId::new),
                            edge: id(911, semantic_vocabulary::EdgeId::new),
                        }
            })
            .count(),
        2
    );

    let accepted = validate_linear_empty_block_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.linear-empty-block-thread.v2"
        )
    );
    let output = accepted.unit();
    assert_eq!(
        output.functions[0]
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        [id(902, BlockId::new), id(904, BlockId::new)]
    );
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &output.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    assert_eq!(*psi_edge, id(911, EdgeId::new));
    assert_eq!(*target, id(904, BlockId::new));
    assert_eq!(bindings[0].argument, id(906, ValueId::new));
    assert_eq!(bindings[1].argument, id(905, ValueId::new));
    assert!(output.functions[0].blocks[0].nodes[0].provenance.is_empty());
    assert!(output.functions[0].blocks[0].nodes[0].fuel.is_empty());
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].successors[0]
            .provenance
            .len(),
        2
    );
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].successors[0]
            .fuel
            .len(),
        2
    );
    assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 1);
    assert_eq!(output.functions[0].blocks[1].nodes[0].effect.output, 2);
}

#[test]
fn linear_empty_block_validator_rejects_incomplete_fused_custody() {
    let unit = linear_empty_block_unit();
    let contract = LinearEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = LinearEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let optimization_unit::PsiRewritePatch::ThreadLinearEmptyBlock(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let mut provenance = candidate.provenance().to_vec();
    let incoming = provenance
        .iter()
        .find(|row| {
            row.input
                == PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                }
        })
        .expect("incoming occurrence is present")
        .clone();
    let outgoing = provenance
        .iter_mut()
        .find(|row| {
            row.input
                == PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.outgoing_edge,
                }
        })
        .expect("outgoing occurrence is present");
    outgoing.sources = incoming.sources;
    outgoing.fuel = incoming.fuel;
    let incomplete = PsiRewriteCandidate::new_linear_empty_block(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        -3,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_linear_empty_block_candidate(&unit, &incomplete),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    ));
}

#[test]
fn path_qualified_empty_block_thread_fans_out_only_on_incoming_edge_antichain() {
    let unit = path_qualified_empty_block_unit();
    let contract = PathQualifiedEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = PathQualifiedEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("two mutually exclusive incoming edges are threadable");
    let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
        unreachable!()
    };
    let outgoing_site = PsiRealizationSite::Edge {
        machine: patch.empty.machine,
        edge: patch.outgoing_edge,
    };
    let fanout = candidate
        .provenance()
        .iter()
        .filter(|row| row.input == outgoing_site)
        .collect::<Vec<_>>();
    assert_eq!(fanout.len(), 2);
    assert_ne!(fanout[0].disposition.site(), fanout[1].disposition.site());
    assert!(fanout.iter().all(|row| row.disposition.is_realized()));

    let accepted = validate_path_qualified_empty_block_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.path-qualified-empty-block-thread.v1"
        )
    );
    let function = &accepted.unit().functions[0];
    assert_eq!(function.blocks.len(), 4);
    assert!(
        !function
            .blocks
            .iter()
            .any(|block| block.id == patch.empty.block)
    );
    for edge_id in [id(933, EdgeId::new), id(934, EdgeId::new)] {
        let edge = function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter())
            .flat_map(|node| node.successors.iter())
            .find(|edge| edge.psi_edge == edge_id)
            .expect("incoming edge survives");
        assert_eq!(edge.target, patch.target);
        assert_eq!(
            edge.provenance,
            [
                PsiProvenance::Edge(edge_id),
                PsiProvenance::Edge(patch.outgoing_edge),
            ]
        );
    }

    let mut coexecuted = accepted.unit().clone();
    let source = PsiProvenance::Edge(patch.outgoing_edge);
    coexecuted.functions[0].blocks[0].nodes[0].successors[0]
        .provenance
        .push(source);
    coexecuted.functions[0].blocks[0].nodes[0].successors[0]
        .fuel
        .push(optimization_unit::FuelSettlement {
            site: source,
            units: 1,
        });
    coexecuted.identity = recompute_psi_optimization_unit_identity(&coexecuted);
    assert_eq!(
        optimization_validation::validate_psi_optimization_unit(&coexecuted),
        Err(OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source))
    );
}
