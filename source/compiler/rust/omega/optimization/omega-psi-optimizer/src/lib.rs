#![forbid(unsafe_code)]

//! Deterministic target-neutral analyses and rewrite orchestration for verified
//! Psi optimization units.
//!
//! This crate is not constructed by the ordinary empty-selection compiler
//! path. Callers must explicitly enter the verified optimizer pipeline before
//! creating an [`AnalysisManager`].

mod analyses;
mod pass_manager;
mod registry;
mod rules;

pub use analyses::{
    AnalysisManager, AnalysisManagerError, AnalysisProduct, AnalysisRevisionCommit,
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, EffectClass,
    EffectKnowledge, EffectSummaryAnalysis, ExecutableEdgeAnalysis, ExecutableEdgeFact,
    ExecutableEdgeKnowledge, ExitKind, FunctionControlFlow, LoopAnalysis, LoopRegion,
    NodeEffectSummary, NodeLiveness, ScalarConstant, ScalarConstantAnalysis, ScalarConstantFact,
    StronglyConnectedComponentAnalysis, UseDefinitionAnalysis, ValueFactRegion,
    ValueLivenessAnalysis, ValueLivenessBlock, ValueRangeAnalysis, ValueRangeFact,
    analysis_dependencies, compute_analysis,
};
pub use pass_manager::{
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    VerifiedPsiOptimizationSession, run_psi_registry,
};
pub use registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, RuleScheduleKey,
};
pub use rules::{
    ExactIntegerAddConstantsRule, ExactIntegerMultiplyConstantsRule,
    ExactIntegerSubtractConstantsRule, built_in_psi_registry,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use omega_optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationUnitIdentity,
    };
    use omega_optimization_unit::{
        EffectLink, OptimizationBlock, OptimizationFact, OptimizationNode, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiProvenance, ValueDefinition, ValueDefinitionSite, ValueUse,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractOperation as O, TerminalAbstractSuccessor,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, OperationId, ScalarType,
        ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    #[derive(Clone)]
    enum Terminator {
        Return,
        Crash,
        Jump(u64),
        Branch(u64, u64),
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn edge(raw: u64, target: u64) -> TerminalAbstractSuccessor {
        TerminalAbstractSuccessor {
            psi_edge: id(raw, EdgeId::new),
            target: id(target, BlockId::new),
            bindings: Vec::new(),
        }
    }

    fn operation(raw: u64, terminator: Terminator) -> O {
        match terminator {
            Terminator::Return => O::ReturnUnit {
                psi_edge: id(raw * 10 + 1, EdgeId::new),
                cleanup_actions: Vec::new(),
            },
            Terminator::Crash => O::Crash {
                psi_edge: id(raw * 10 + 1, EdgeId::new),
                cause: psi_terminal::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            Terminator::Jump(target) => O::Jump {
                psi_edge: id(raw * 10 + 1, EdgeId::new),
                target: id(target, BlockId::new),
                bindings: Vec::new(),
            },
            Terminator::Branch(when_true, when_false) => O::Conditional {
                condition: id(raw * 10 + 2, ValueId::new),
                when_true: edge(raw * 10 + 3, when_true),
                when_false: edge(raw * 10 + 4, when_false),
            },
        }
    }

    fn node(operation: O) -> OptimizationNode {
        OptimizationNode {
            operation,
            provenance: Vec::new(),
            fuel: Vec::new(),
            effect: EffectLink {
                input: 0,
                output: 1,
            },
            definitions: Vec::new(),
            uses: Vec::new(),
            successors: Vec::new(),
            ownership: Vec::new(),
        }
    }

    fn function(
        machine: u64,
        entry: u64,
        blocks: Vec<(u64, Terminator)>,
    ) -> PsiOptimizationFunction {
        PsiOptimizationFunction {
            machine: id(machine, MachineId::new),
            entry: id(entry, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            declared_places: BTreeSet::new(),
            entry_claims: BTreeSet::new(),
            facts: Vec::new(),
            blocks: blocks
                .into_iter()
                .map(|(block, terminator)| OptimizationBlock {
                    id: id(block, BlockId::new),
                    parameters: Vec::new(),
                    nodes: vec![node(operation(block, terminator))],
                })
                .collect(),
        }
    }

    fn unit(functions: Vec<PsiOptimizationFunction>, revision: &[u8]) -> PsiOptimizationUnit {
        PsiOptimizationUnit {
            identity: OptimizationUnitIdentity::from_canonical_bytes(revision),
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            entry: functions[0].machine,
            functions,
        }
    }

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
                        (4, Terminator::Crash),
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
        assert_eq!(cfg.functions[0].blocks[3].exits, vec![ExitKind::Crash]);
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
        assert!(post.functions[0].1[0].1.contains(&id(4, BlockId::new)));
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

    #[test]
    fn cache_audits_undeclared_invalidation_atomically() {
        let original = unit(
            vec![function(100, 1, vec![(1, Terminator::Return)])],
            b"original",
        );
        let changed = unit(
            vec![function(
                100,
                1,
                vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
            )],
            b"changed",
        );
        let mut manager = AnalysisManager::new(&original);
        manager
            .require(&original, AnalysisKind::Dominators)
            .unwrap();
        let prior_revision = manager.revision();
        let prior_cache = manager.cached_kinds().collect::<Vec<_>>();
        assert_eq!(
            manager.commit_revision(&changed, AnalysisInvalidationSet::default(), true,),
            Err(AnalysisManagerError::UndeclaredInvalidation(
                AnalysisKind::ControlFlowGraph
            ))
        );
        assert_eq!(manager.revision(), prior_revision);
        assert_eq!(manager.cached_kinds().collect::<Vec<_>>(), prior_cache);

        let committed = manager
            .commit_revision(
                &changed,
                AnalysisInvalidationSet::new([AnalysisKind::ControlFlowGraph]),
                true,
            )
            .unwrap();
        assert_eq!(committed.current, changed.identity);
        assert!(committed.invalidated.contains(&AnalysisKind::Dominators));
        assert!(manager.cached_kinds().next().is_none());
    }

    #[test]
    fn cached_cold_and_parallel_schedules_have_canonical_output() {
        let unit = unit(
            vec![function(
                100,
                1,
                vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
            )],
            b"parallel",
        );
        let requested = AnalysisSet::new([
            AnalysisKind::LoopForest,
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::StronglyConnectedComponents,
            AnalysisKind::Dominators,
        ]);
        let cold = AnalysisManager::compute_cold_parallel(&unit, requested).unwrap();
        assert_eq!(
            cold.iter().map(AnalysisProduct::kind).collect::<Vec<_>>(),
            requested.iter().collect::<Vec<_>>()
        );
        let mut manager = AnalysisManager::new(&unit);
        let cached = manager
            .require_all(&unit, requested)
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(cached, cold);
    }

    #[test]
    fn literal_semantic_facts_keep_support_and_revision_regions() {
        let mut function = function(
            100,
            1,
            vec![
                (1, Terminator::Branch(2, 3)),
                (2, Terminator::Return),
                (3, Terminator::Crash),
            ],
        );
        let condition = id(12, ValueId::new);
        let integer = id(99, ValueId::new);
        let boolean_support = id(600, OperationId::new);
        let integer_support = id(601, OperationId::new);
        function.facts = vec![
            OptimizationFact::BooleanConstant {
                value: condition,
                constant: true,
                support: boolean_support,
            },
            OptimizationFact::IntegerConstant {
                value: integer,
                constant: IntegerValue::Unsigned(7),
                support: integer_support,
            },
        ];
        let unit = unit(vec![function], b"semantic-facts");

        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap()
        else {
            unreachable!()
        };
        assert!(constants.facts.iter().all(|fact| {
            fact.valid_in.revision == unit.identity
                && fact.valid_in.machine == unit.entry
                && fact.valid_in.value == fact.value
        }));

        let AnalysisProduct::ExecutableEdges(edges) =
            compute_analysis(&unit, AnalysisKind::ExecutableEdges).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            edges
                .edges
                .iter()
                .map(|edge| (edge.knowledge, edge.support.clone()))
                .collect::<Vec<_>>(),
            vec![
                (
                    ExecutableEdgeKnowledge::KnownExecutable,
                    vec![boolean_support]
                ),
                (
                    ExecutableEdgeKnowledge::KnownInexecutable,
                    vec![boolean_support]
                ),
            ]
        );

        let AnalysisProduct::ValueRanges(ranges) =
            compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(ranges.facts.len(), 1);
        assert_eq!(ranges.facts[0].minimum, IntegerValue::Unsigned(7));
        assert_eq!(ranges.facts[0].maximum, IntegerValue::Unsigned(7));
        assert_eq!(ranges.facts[0].support, integer_support);
    }

    #[test]
    fn effects_are_conservative_and_liveness_reaches_fixed_point() {
        let mut function = function(
            100,
            1,
            vec![
                (1, Terminator::Branch(2, 3)),
                (2, Terminator::Jump(4)),
                (3, Terminator::Jump(4)),
                (4, Terminator::Crash),
            ],
        );
        let condition = id(12, ValueId::new);
        let support = id(700, OperationId::new);
        let mut constant = node(O::BooleanConstant {
            psi_operation: support,
            result: condition,
            value: true,
        });
        constant.provenance = vec![PsiProvenance::Operation(support)];
        constant.definitions = vec![ValueDefinition {
            value: condition,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::Node {
                block: id(1, BlockId::new),
                node: 0,
            },
        }];
        function.blocks[0].nodes.insert(0, constant);
        function.blocks[0].nodes[1].uses = vec![ValueUse {
            value: condition,
            block: id(1, BlockId::new),
            node: 1,
        }];
        let unit = unit(vec![function], b"effects-liveness");

        let AnalysisProduct::EffectSummaries(effects) =
            compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(effects.nodes[0].class, EffectClass::PureScalar);
        assert_eq!(effects.nodes[0].observable, EffectKnowledge::No);
        assert_eq!(
            effects.nodes[0].support,
            vec![PsiProvenance::Operation(support)]
        );
        let crash = effects.nodes.last().unwrap();
        assert_eq!(crash.crash, EffectKnowledge::Yes);
        assert_eq!(crash.observable, EffectKnowledge::Yes);

        let AnalysisProduct::ValueLiveness(liveness) =
            compute_analysis(&unit, AnalysisKind::ValueLiveness).unwrap()
        else {
            unreachable!()
        };
        assert!(liveness.blocks[0].entry.is_empty());
        assert_eq!(liveness.blocks[0].nodes[0].exit, vec![condition]);
        assert_eq!(liveness.blocks[0].nodes[1].entry, vec![condition]);
        assert!(liveness.blocks[0].nodes[1].exit.is_empty());
    }
}
