//! CFG, dominance, SCC, loop, and call-graph coverage.

use super::fixtures::*;
use crate::*;
use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::*;
use psi_core::*;

#[test]
fn cfg_products_cover_crash_exits_disconnected_machines_and_dominance() {
    let unit = unit(
        vec![
            function(
                100,
                1,
                vec![
                    (1, Terminator::Branch(2, 3)),
                    (2, Terminator::Jump(4)),
                    (3, Terminator::Jump(4)),
                    (4, Terminator::Branch(5, 6)),
                    (5, Terminator::Return),
                    (6, Terminator::Crash),
                ],
            ),
            function(200, 11, vec![(11, Terminator::Return)]),
        ],
        b"cfg",
    );
    let AnalysisProduct::ControlFlowGraph(cfg) =
        compute_analysis(&unit, AnalysisKind::ControlFlowGraph).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(cfg.functions.len(), 2);
    assert_eq!(cfg.functions[0].blocks[4].exits, vec![ExitKind::Normal]);
    assert_eq!(cfg.functions[0].blocks[5].exits, vec![ExitKind::Crash]);
    assert!(
        cfg.functions
            .iter()
            .all(|function| { function.blocks.iter().all(|block| block.reachable) })
    );

    let AnalysisProduct::Dominators(dominators) =
        compute_analysis(&unit, AnalysisKind::Dominators).unwrap()
    else {
        unreachable!()
    };
    let join = &dominators.functions[0].1[3];
    assert_eq!(join.0, id(4, BlockId::new));
    assert_eq!(join.1, vec![id(1, BlockId::new), id(4, BlockId::new)]);

    let AnalysisProduct::PostDominators(post) =
        compute_analysis(&unit, AnalysisKind::PostDominators).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        post.functions[0].1[0],
        (
            id(1, BlockId::new),
            vec![id(1, BlockId::new), id(4, BlockId::new)],
        )
    );
}

#[test]
fn irreducible_loop_and_scc_are_reported_canonically() {
    let unit = unit(
        vec![function(
            100,
            1,
            vec![
                (1, Terminator::Branch(2, 3)),
                (2, Terminator::Jump(4)),
                (3, Terminator::Jump(4)),
                (4, Terminator::Branch(2, 5)),
                (5, Terminator::Return),
            ],
        )],
        b"loop",
    );
    let AnalysisProduct::LoopForest(loops) =
        compute_analysis(&unit, AnalysisKind::LoopForest).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(loops.functions[0].1.len(), 1);
    assert_eq!(
        loops.functions[0].1[0].blocks,
        vec![id(2, BlockId::new), id(4, BlockId::new)]
    );
    assert!(loops.functions[0].1[0].irreducible);
    assert_eq!(loops.functions[0].1[0].header, None);
    let AnalysisProduct::StronglyConnectedComponents(components) =
        compute_analysis(&unit, AnalysisKind::StronglyConnectedComponents).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        components.functions[0].1,
        vec![
            vec![id(1, BlockId::new)],
            vec![id(2, BlockId::new), id(4, BlockId::new)],
            vec![id(3, BlockId::new)],
            vec![id(5, BlockId::new)],
        ]
    );
}

#[test]
fn call_graph_marks_mutual_recursion() {
    let mut first = function(100, 1, vec![(1, Terminator::Return)]);
    let mut second = function(200, 2, vec![(2, Terminator::Return)]);
    first.blocks[0].nodes.insert(
        0,
        node(O::CallUnit {
            psi_operation: id(501, psi_core::OperationId::new),
            callee: second.machine,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        }),
    );
    second.blocks[0].nodes.insert(
        0,
        node(O::CallUnit {
            psi_operation: id(502, psi_core::OperationId::new),
            callee: first.machine,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        }),
    );
    let unit = unit(vec![first, second], b"calls");
    let AnalysisProduct::EffectSummaries(effects) =
        compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(effects.nodes[0].class, EffectClass::InternalCall);
    assert_eq!(effects.nodes[0].observable, EffectKnowledge::May);
    assert_eq!(effects.nodes[0].suspension, EffectKnowledge::May);
    let AnalysisProduct::CallGraph(calls) =
        compute_analysis(&unit, AnalysisKind::CallGraph).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(calls.recursive_components, calls.components);
    assert_eq!(calls.recursive_components.len(), 1);
}
