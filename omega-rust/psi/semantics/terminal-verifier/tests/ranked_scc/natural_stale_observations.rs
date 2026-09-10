use super::*;
use terminal_psi::{
    StructuralArgument, TerminalBlockNaturalRank, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge,
};

fn length(operation: u64, value: u64, source: u64) -> Operation {
    Operation {
        id: id(operation, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(value, ValueId::new),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::ByteSequenceLength {
            source: id(source, PlaceId::new),
        },
    }
}

fn successor(edge: u64, target: u64, view: Option<u64>) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: view
            .into_iter()
            .map(|place| StructuralArgument {
                place: id(place, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            })
            .collect(),
        trivial_affine_discards: Vec::new(),
    }
}

fn ranking_edge(
    edge: u64,
    source: u64,
    target: u64,
    rank: u64,
    strict: bool,
) -> TerminalNaturalRankEdge {
    TerminalNaturalRankEdge {
        edge: id(edge, EdgeId::new),
        source: id(source, BlockId::new),
        target: id(target, BlockId::new),
        successor_rank: id(rank, ValueId::new),
        comparison: if strict {
            TerminalNaturalRankComparison::Strict
        } else {
            TerminalNaturalRankComparison::Preserving
        },
    }
}

fn natural(module: &mut TerminalModule) -> &mut TerminalNaturalCycle {
    let Some(TerminalRankedScc::Natural(components)) = &mut module.machines[0].ranked_scc else {
        panic!("natural fixture");
    };
    &mut components[0]
}

/// Header 2 binds the arriving view and chooses observation block 5 or bypasses
/// it to latch 3. The latch observes the current descriptor independently and
/// returns its tail to the header. Value 30 from block 5 can therefore survive
/// an earlier traversal but cannot describe every subsequent arrival.
fn current_observations() -> TerminalModule {
    let mut module = super::unranked_views::view_cycle();
    let machine = &mut module.machines[0];
    machine.blocks[1].operations.push(Operation {
        id: id(32, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(32, ValueId::new),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessThan {
            left: id(1, ValueId::new),
            right: id(10, ValueId::new),
        },
    });
    machine.blocks[1].terminator = Terminator::Conditional {
        // This guard changes as tails arrive; unlike an invariant Boolean
        // parameter, it can select observation first and bypass it later.
        condition: id(32, ValueId::new),
        when_true: successor(2, 5, None),
        when_false: successor(3, 3, None),
    };
    machine.blocks[2].operations.insert(0, length(31, 31, 2));
    // Operational bounds use the current latch observation, so a metadata
    // mutation cannot fail early because an executable operand became stale.
    for operation in &mut machine.blocks[2].operations {
        match &mut operation.kind {
            OperationKind::ByteSequenceRead { length, .. } => *length = id(31, ValueId::new),
            OperationKind::ByteSequenceSubslice { length, end, .. } => {
                *length = id(31, ValueId::new);
                *end = id(31, ValueId::new);
            }
            _ => {}
        }
    }
    machine.blocks[2].terminator = Terminator::Conditional {
        condition: id(20, ValueId::new),
        when_true: successor(4, 2, Some(3)),
        when_false: successor(6, 4, None),
    };
    machine.blocks.push(Block {
        id: id(5, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![length(30, 30, 2)],
        terminator: Terminator::Jump {
            edge: id(7, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        rank_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ranks: [(2, 10), (3, 31), (5, 30)]
            .into_iter()
            .map(|(block, value)| TerminalBlockNaturalRank {
                block: id(block, BlockId::new),
                value: id(value, ValueId::new),
            })
            .collect(),
        edges: vec![
            ranking_edge(2, 2, 5, 10, false),
            ranking_edge(3, 2, 3, 10, false),
            ranking_edge(4, 3, 2, 13, true),
            ranking_edge(7, 5, 3, 30, false),
        ],
    }]));
    module
}

#[test]
fn natural_cycle_rejects_bypassed_observation_as_block_or_successor_rank() {
    let original = current_observations();
    validate_module(&original).expect("current ranks and exact tail transfer validate");
    for (stale_block, stale_successor) in [(true, false), (false, true), (true, true)] {
        let mut changed = original.clone();
        let component = natural(&mut changed);
        if stale_block {
            component.ranks[1].value = id(30, ValueId::new);
        }
        if stale_successor {
            component.edges[1].successor_rank = id(30, ValueId::new);
        }
        assert_eq!(
            validate_module(&changed).map(|_| ()),
            Err(ModuleError::InvalidRankedScc(id(1, MachineId::new))),
            "stale block {stale_block}, stale successor {stale_successor}"
        );
        changed.machines[0].ranked_scc = None;
        validate_module(&changed).expect("only the ranking metadata was invalid");
    }
}

/// H(n) constructs bytes[0..n], observes its length, and takes H(n + 1).
/// This fixture asks only whether rank substitution is well formed. Its strict
/// scalar comparison has no proof and grants no termination authority.
fn target_local_descriptor() -> TerminalModule {
    let mut module = super::unranked_views::view_cycle();
    let machine = &mut module.machines[0];
    let mut subslice = machine.blocks[2].operations[1].clone();
    let OperationKind::ByteSequenceSubslice {
        source, start, end, ..
    } = &mut subslice.kind
    else {
        panic!("tail descriptor fixture");
    };
    *source = id(1, PlaceId::new);
    *start = id(40, ValueId::new);
    *end = id(2, ValueId::new);
    machine
        .structural_places
        .retain(|place| place.id != id(2, PlaceId::new));
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let header = &mut machine.blocks[1];
    header.structural_parameters.clear();
    header.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(2, ValueId::new),
        scalar_type: scalar,
    }];
    header.operations = vec![
        length(10, 10, 1),
        Operation {
            id: id(40, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(40, ValueId::new),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        },
        Operation {
            id: id(41, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(41, ValueId::new),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        },
        subslice,
        length(13, 13, 3),
        Operation {
            id: id(42, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(42, ValueId::new),
                scalar_type: scalar,
            }),
            kind: OperationKind::WrappingIntegerAdd {
                left: id(2, ValueId::new),
                right: id(41, ValueId::new),
            },
        },
    ];
    let mut backedge = successor(2, 2, None);
    backedge.arguments.push(id(42, ValueId::new));
    header.terminator = Terminator::Conditional {
        condition: id(20, ValueId::new),
        when_true: backedge,
        when_false: successor(3, 4, None),
    };
    let Terminator::Jump {
        arguments,
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("entry jump");
    };
    *arguments = vec![id(1, ValueId::new)];
    structural_arguments.clear();
    machine
        .blocks
        .retain(|block| block.id != id(3, BlockId::new));
    machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        rank_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ranks: vec![TerminalBlockNaturalRank {
            block: id(2, BlockId::new),
            value: id(2, ValueId::new),
        }],
        edges: vec![ranking_edge(2, 2, 2, 42, true)],
    }]));
    module
}

#[test]
fn natural_cycle_rejects_previous_target_local_descriptor_as_arriving_rank() {
    let mut module = target_local_descriptor();
    validate_module(&module)
        .expect("scalar successor substitution and typed descriptor operations");
    let component = natural(&mut module);
    component.ranks[0].value = id(13, ValueId::new);
    component.edges[0].successor_rank = id(13, ValueId::new);
    // Keep the self-edge strict so rejection cannot be attributed to a
    // preserving cycle. The current view's length is not the next H(n)'s rank.
    assert_eq!(
        component.edges[0].comparison,
        TerminalNaturalRankComparison::Strict
    );
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidRankedScc(id(1, MachineId::new)))
    );
    module.machines[0].ranked_scc = None;
    validate_module(&module).expect("target-local operations retain valid dominance and types");
}
