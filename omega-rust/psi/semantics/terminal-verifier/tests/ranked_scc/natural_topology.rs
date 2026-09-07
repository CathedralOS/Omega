use super::*;
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge,
};

#[test]
fn natural_topology_rejects_a_preserving_cross_cycle() {
    let mut module = ranked_countdown();
    let machine = &mut module.machines[0];
    let edge = |number, target, arguments| SuccessorEdge {
        edge: id(number, EdgeId::new),
        target: id(target, BlockId::new),
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[2].terminator = Terminator::Conditional {
        condition: id(4, ValueId::new),
        when_true: edge(4, 2, vec![id(6, ValueId::new)]),
        when_false: edge(6, 5, Vec::new()),
    };
    machine.blocks.push(Block {
        id: id(5, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: id(7, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    let comparison = |number, source, target, value, strict| TerminalNaturalRankEdge {
        edge: id(number, EdgeId::new),
        source: id(source, BlockId::new),
        target: id(target, BlockId::new),
        successor_rank: id(value, ValueId::new),
        comparison: if strict {
            TerminalNaturalRankComparison::Strict
        } else {
            TerminalNaturalRankComparison::Preserving
        },
    };
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        ranks: [2, 3, 5]
            .into_iter()
            .map(|block| TerminalBlockNaturalRank {
                block: id(block, BlockId::new),
                value: id(2, ValueId::new),
            })
            .collect(),
        edges: vec![
            comparison(2, 2, 3, 2, true),
            comparison(4, 3, 2, 6, false),
            comparison(6, 3, 5, 2, false),
            comparison(7, 5, 3, 2, true),
        ],
    }]));
    // Shape validation does not establish the asserted strict comparisons.
    // Both cycles have a strict edge here; no proof bundle is supplied.
    validate_module(&module).expect("complete topology and exact scalar substitutions");
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let Some(TerminalRankedScc::Natural(components)) = &mut module.machines[0].ranked_scc else {
        panic!("natural fixture");
    };
    components[0].edges[3].comparison = TerminalNaturalRankComparison::Preserving;
    // The header cycle still has strict edge 2, but 3 -> 5 -> 3 preserves.
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidRankedScc(id(1, MachineId::new)))
    );
    module.machines[0].ranked_scc = None;
    validate_module(&module).expect("ordinary cyclic safety is independent of ranking");
}
