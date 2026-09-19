//! Coordinates a machine's retained ranking evidence names by identity.
//!
//! A `TerminalRankedScc` row covers exactly the cyclic components the
//! verifier derived for its machine: the member blocks, and the scalar values
//! the rows pin — rank values and successor ranks. Rewrites keep covered
//! block contents and named value identities exact; regions outside the
//! covered components optimize under the ordinary rules.

use semantic_vocabulary::{BlockId, ValueId};
use std::collections::BTreeSet;
use terminal_psi::{TerminalMachine, TerminalRankedScc};

/// The block-local coordinates a machine's `ranked_scc` evidence covers.
#[derive(Debug, Default)]
pub(crate) struct RankedCoverage {
    /// Member blocks of the covered cyclic components. Their parameter tables
    /// feed rank-substitution positions and their contents stay exact.
    pub blocks: BTreeSet<BlockId>,
    /// Scalar values the rows name by identity: rank values and successor
    /// ranks.
    pub values: BTreeSet<ValueId>,
    /// Scalar values used inside member blocks. Collapsing or removing one
    /// would rewrite covered contents through the use substitution, so its
    /// identity survives even though the evidence does not name it.
    pub member_uses: BTreeSet<ValueId>,
}

/// Inventory the coordinates `machine`'s retained ranking evidence covers.
/// An unranked machine has empty coverage and optimizes everywhere.
pub(crate) fn ranked_coverage(machine: &TerminalMachine) -> RankedCoverage {
    let mut coverage = RankedCoverage::default();
    let Some(ranking) = &machine.ranked_scc else {
        return coverage;
    };
    let TerminalRankedScc::Natural(components) = ranking;
    for component in components {
        for rank in &component.ranks {
            coverage.blocks.insert(rank.block);
            coverage.values.insert(rank.value);
        }
        for edge in &component.edges {
            coverage.blocks.extend([edge.source, edge.target]);
            coverage.values.insert(edge.successor_rank);
        }
    }
    // The read-side of `map_scalar_uses` is a mutating traversal: walk clones
    // and keep every visited identity.
    for block in &machine.blocks {
        if !coverage.blocks.contains(&block.id) {
            continue;
        }
        for operation in &block.operations {
            let mut kind = operation.kind.clone();
            kind.map_scalar_uses(&mut |value| {
                coverage.member_uses.insert(value);
                value
            });
        }
        let mut terminator = block.terminator.clone();
        terminator.map_scalar_uses(&mut |value| {
            coverage.member_uses.insert(value);
            value
        });
    }
    coverage
}

#[cfg(test)]
mod tests {
    //! Coverage inventory legs: an unranked machine covers nothing, a
    //! `Natural` row covers exactly its member blocks and named values, and
    //! member uses collect scalar identities used inside member contents —
    //! operation operands and terminator positions — and nowhere else.
    use super::ranked_coverage;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalBlockNaturalRank, TerminalMachine, TerminalMachineResult, TerminalNaturalCycle,
        TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
        ValueDeclaration,
    };

    fn value(ordinal: u64) -> ValueId {
        ValueId::new(ordinal).unwrap()
    }

    fn block_id(ordinal: u64) -> BlockId {
        BlockId::new(ordinal).unwrap()
    }

    fn u32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    fn declaration(ordinal: u64, scalar_type: ScalarType) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: value(ordinal),
            scalar_type,
        }
    }

    fn machine(blocks: Vec<Block>) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: vec![
                declaration(1, u32_type()),
                declaration(2, ScalarType::Boolean),
            ],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks,
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn block(
        ordinal: u64,
        parameters: Vec<ValueDeclaration>,
        operations: Vec<Operation>,
        terminator: Terminator,
    ) -> Block {
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(ordinal),
            parameters,
            operations,
            terminator,
        }
    }

    fn constant(ordinal: u64, result: u64, value_bits: u128) -> Operation {
        Operation {
            static_reach_binding: None,
            id: OperationId::new(ordinal).unwrap(),
            result: OperationResult::Scalar(declaration(result, u32_type())),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value_bits),
            },
        }
    }

    fn subtract(ordinal: u64, result: u64, left: u64, right: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            id: OperationId::new(ordinal).unwrap(),
            result: OperationResult::Scalar(declaration(result, u32_type())),
            kind: OperationKind::WrappingIntegerSubtract {
                left: value(left),
                right: value(right),
            },
        }
    }

    fn edge(ordinal: u64, target: u64, arguments: Vec<u64>) -> SuccessorEdge {
        SuccessorEdge {
            edge: EdgeId::new(ordinal).unwrap(),
            target: block_id(target),
            arguments: arguments.into_iter().map(value).collect(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        }
    }

    /// The shared shape: entry b1 branches into member b2, whose backedge and
    /// exit edge carry rank arguments; b3 is the uncovered exit.
    ///
    /// ```text
    /// b1 (entry): cond v2 ──e1:t──▶ b2 [v1]
    ///                    └──e2:f──▶ b3 [v1]
    /// b2 (v10, v11): v40 = 1u32; v41 = v10 - v40;
    ///                cond v11 ──e3:[v41, v11]──▶ b2 (self-edge)
    ///                       └──e4:[v1]─────────▶ b3
    /// b3 (v30): ──e5:[v30]──▶ b4
    /// b4 (v50): return v50
    /// ```
    fn member_cycle_machine() -> TerminalMachine {
        machine(vec![
            block(
                1,
                Vec::new(),
                Vec::new(),
                Terminator::Conditional {
                    condition: value(2),
                    when_true: edge(1, 2, vec![1]),
                    when_false: edge(2, 3, vec![1]),
                },
            ),
            block(
                2,
                vec![
                    declaration(10, u32_type()),
                    declaration(11, ScalarType::Boolean),
                ],
                vec![constant(40, 40, 1), subtract(41, 41, 10, 40)],
                Terminator::Conditional {
                    condition: value(11),
                    when_true: edge(3, 2, vec![41, 11]),
                    when_false: edge(4, 3, vec![1]),
                },
            ),
            block(
                3,
                vec![declaration(30, u32_type())],
                Vec::new(),
                Terminator::Jump {
                    edge: EdgeId::new(5).unwrap(),
                    target: block_id(4),
                    arguments: vec![value(30)],
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            ),
            block(
                4,
                vec![declaration(50, u32_type())],
                Vec::new(),
                Terminator::Return {
                    edge: EdgeId::new(6).unwrap(),
                    value: value(50),
                    cleanup_actions: Vec::new(),
                },
            ),
        ])
    }

    #[test]
    fn an_unranked_machine_covers_nothing() {
        // Boundary: without retained ranking evidence every coordinate
        // optimizes under the ordinary rules.
        let coverage = ranked_coverage(&member_cycle_machine());
        assert!(coverage.blocks.is_empty());
        assert!(coverage.values.is_empty());
        assert!(coverage.member_uses.is_empty());
    }

    #[test]
    fn covered_blocks_values_and_member_uses_follow_the_rows() {
        // Positive: the row names member block b2, rank v10, and successor
        // rank v41; edge endpoints join the covered blocks. Member uses are
        // exactly the scalar identities b2's contents touch — operands v10,
        // v40, the condition v11, and the edge arguments — while identical
        // uses outside the component (b1's condition and arguments, b3's and
        // b4's contents) stay uncovered.
        let mut machine = member_cycle_machine();
        machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            ranks: vec![TerminalBlockNaturalRank {
                block: block_id(2),
                value: value(10),
            }],
            edges: vec![TerminalNaturalRankEdge {
                edge: EdgeId::new(3).unwrap(),
                source: block_id(2),
                target: block_id(2),
                successor_rank: value(41),
                comparison: TerminalNaturalRankComparison::Strict,
            }],
        }]));
        let coverage = ranked_coverage(&machine);
        let again = ranked_coverage(&machine);
        assert_eq!(coverage.blocks, again.blocks);
        assert_eq!(coverage.values, again.values);
        assert_eq!(coverage.member_uses, again.member_uses);

        assert_eq!(
            coverage.blocks,
            std::collections::BTreeSet::from([block_id(2)])
        );
        assert_eq!(
            coverage.values,
            std::collections::BTreeSet::from([value(10), value(41)])
        );
        // Member contents use v10 and v40 as operands, v11 as the condition,
        // v41/v11 on the covered self-edge, and v1 on the exit edge.
        assert_eq!(
            coverage.member_uses,
            std::collections::BTreeSet::from([
                value(1),
                value(10),
                value(11),
                value(40),
                value(41),
            ])
        );
        // The uncovered regions' uses — v2, v30, v50 — never enter the set.
        assert!(!coverage.member_uses.contains(&value(2)));
        assert!(!coverage.member_uses.contains(&value(30)));
        assert!(!coverage.member_uses.contains(&value(50)));
    }

    #[test]
    fn an_edge_endpoint_without_a_rank_row_still_joins_covered_blocks() {
        // Boundary: edge endpoints extend the covered block set beyond the
        // rank rows — a cycle member reached by a covered edge keeps its
        // contents exact even when no row pins its own rank.
        let mut machine = member_cycle_machine();
        machine.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            ranks: vec![TerminalBlockNaturalRank {
                block: block_id(2),
                value: value(10),
            }],
            edges: vec![TerminalNaturalRankEdge {
                edge: EdgeId::new(4).unwrap(),
                source: block_id(2),
                target: block_id(3),
                successor_rank: value(1),
                comparison: TerminalNaturalRankComparison::Strict,
            }],
        }]));
        let coverage = ranked_coverage(&machine);
        assert_eq!(
            coverage.blocks,
            std::collections::BTreeSet::from([block_id(2), block_id(3)]),
            "the edge's uncovered-row target still counts as a member block"
        );
        assert_eq!(
            coverage.values,
            std::collections::BTreeSet::from([value(10), value(1)])
        );
        // b3 is now a member: its parameter's jump use joins member_uses.
        assert!(coverage.member_uses.contains(&value(30)));
        assert!(!coverage.member_uses.contains(&value(50)));
    }
}
