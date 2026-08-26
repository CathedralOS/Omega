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
    ExecutableEdgeKnowledge, ExitKind, FunctionControlFlow, FunctionEffectSummary, LoopAnalysis,
    LoopRegion, NodeEffectSummary, NodeLiveness, OwnershipFrontierAnalysis,
    OwnershipFrontierAnalysisFact, ScalarConstant, ScalarConstantAnalysis, ScalarConstantFact,
    ScalarConstantSupport, StronglyConnectedComponentAnalysis, UseDefinitionAnalysis,
    ValueFactRegion, ValueLivenessAnalysis, ValueLivenessBlock, ValueRangeAnalysis, ValueRangeFact,
    analysis_dependencies, compute_analysis,
};
pub use pass_manager::{
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    VerifiedPsiOptimizationSession, baseline_psi_cost_model_identity, run_psi_pipeline,
    run_psi_registry,
};
pub use registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, RuleScheduleKey,
};
pub use rules::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, ConstantConditionalFoldRule,
    ExactIntegerAddConstantsRule, ExactIntegerCastConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, IntegerBitwiseAndConstantsRule,
    IntegerBitwiseNotConstantsRule, IntegerBitwiseOrConstantsRule, IntegerBitwiseXorConstantsRule,
    IntegerEqualConstantsRule, IntegerLessOrEqualConstantsRule, IntegerLessThanConstantsRule,
    IntegerWidenConstantsRule, RedundantBlockParameterRule, SaturatingIntegerAddConstantsRule,
    SaturatingIntegerDivideConstantsRule, SaturatingIntegerMultiplyConstantsRule,
    SaturatingIntegerRemainderConstantsRule, SaturatingIntegerSubtractConstantsRule,
    WrappingIntegerAddConstantsRule, WrappingIntegerDivideConstantsRule,
    WrappingIntegerMultiplyConstantsRule, WrappingIntegerRemainderConstantsRule,
    WrappingIntegerShiftLeftConstantsRule, WrappingIntegerShiftRightConstantsRule,
    WrappingIntegerSubtractConstantsRule, built_in_psi_registries, built_in_psi_registry,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use omega_optimization_core::{AnalysisInvalidationSet, AnalysisKind, AnalysisSet};
    use omega_optimization_unit::{
        EffectLink, OptimizationBlock, OptimizationEdge, OptimizationFact, OptimizationNode,
        OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinition,
        ValueDefinitionSite, ValueUse, recompute_psi_optimization_unit_identity,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractOperation as O, TerminalAbstractSuccessor, TerminalValueBinding,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, OperationId, ScalarType,
        ServiceId, ValueId,
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
            attachment: None,
            entry: id(entry, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit,
            declared_places: BTreeSet::new(),
            entry_claim_declarations: Vec::new(),
            entry_claims: BTreeSet::new(),
            published_service_ceiling: Vec::new(),
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

    fn unit(functions: Vec<PsiOptimizationFunction>, _revision: &[u8]) -> PsiOptimizationUnit {
        let mut unit = PsiOptimizationUnit {
            identity: omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
                b"pending test content",
            ),
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            entry: functions[0].machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            accepted_obligation_facts: Vec::new(),
            ownership_frontier_facts: Vec::new(),
            functions,
        };
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    fn block_parameter_constant_unit(
        known_condition: Option<bool>,
        right_constant: IntegerValue,
    ) -> (PsiOptimizationUnit, ValueId, [OperationId; 3], [EdgeId; 4]) {
        let mut function = function(
            100,
            1,
            vec![
                (1, Terminator::Branch(2, 3)),
                (2, Terminator::Jump(4)),
                (3, Terminator::Jump(4)),
                (4, Terminator::Return),
            ],
        );
        let condition = id(12, ValueId::new);
        let left = id(70, ValueId::new);
        let right = id(71, ValueId::new);
        let parameter = id(72, ValueId::new);
        let scalar_type = ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        );
        function.parameters = vec![ValueDefinition {
            value: condition,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::FunctionParameter(0),
        }];
        function.blocks[3].parameters = vec![ValueDefinition {
            value: parameter,
            scalar_type,
            site: ValueDefinitionSite::BlockParameter {
                block: id(4, BlockId::new),
                position: 0,
            },
        }];
        let true_edge = id(13, EdgeId::new);
        let false_edge = id(14, EdgeId::new);
        function.blocks[0].nodes[0].successors = vec![
            OptimizationEdge {
                psi_edge: true_edge,
                target: id(2, BlockId::new),
                bindings: Vec::new(),
            },
            OptimizationEdge {
                psi_edge: false_edge,
                target: id(3, BlockId::new),
                bindings: Vec::new(),
            },
        ];
        let left_edge = id(21, EdgeId::new);
        let right_edge = id(31, EdgeId::new);
        for (block_index, edge, argument) in [(1, left_edge, left), (2, right_edge, right)] {
            let binding = TerminalValueBinding {
                parameter,
                argument,
                scalar_type,
            };
            let O::Jump { bindings, .. } = &mut function.blocks[block_index].nodes[0].operation
            else {
                unreachable!()
            };
            *bindings = vec![binding];
            function.blocks[block_index].nodes[0].successors = vec![OptimizationEdge {
                psi_edge: edge,
                target: id(4, BlockId::new),
                bindings: vec![binding],
            }];
        }
        let condition_support = id(600, OperationId::new);
        let left_support = id(601, OperationId::new);
        let right_support = id(602, OperationId::new);
        function.facts = vec![
            OptimizationFact::IntegerConstant {
                value: left,
                constant: IntegerValue::Unsigned(7),
                support: left_support,
            },
            OptimizationFact::IntegerConstant {
                value: right,
                constant: right_constant,
                support: right_support,
            },
        ];
        if let Some(condition) = known_condition {
            function.facts.push(OptimizationFact::BooleanConstant {
                value: id(12, ValueId::new),
                constant: condition,
                support: condition_support,
            });
        }
        (
            unit(vec![function], b"block-parameter-constants"),
            parameter,
            [condition_support, left_support, right_support],
            [true_edge, false_edge, left_edge, right_edge],
        )
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
    fn function_effects_propagate_services_and_crashes_through_calls() {
        let mut caller = function(100, 1, vec![(1, Terminator::Return)]);
        let mut callee = function(200, 2, vec![(2, Terminator::Crash)]);
        let call_support = id(510, OperationId::new);
        let mut call = node(O::CallUnit {
            psi_operation: call_support,
            callee: callee.machine,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
        });
        call.provenance = vec![PsiProvenance::Operation(call_support)];
        caller.blocks[0].nodes.insert(0, call);
        let service_support = id(511, OperationId::new);
        let service = id(512, ServiceId::new);
        let mut write = node(O::PortWrite {
            psi_operation: service_support,
            service,
            port: 7,
            value: 9,
        });
        write.provenance = vec![PsiProvenance::Operation(service_support)];
        callee.blocks[0].nodes.insert(0, write);
        let unit = unit(vec![caller, callee], b"transitive-effects");

        let AnalysisProduct::EffectSummaries(effects) =
            compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(effects.functions.len(), 2);
        for summary in &effects.functions {
            assert_eq!(summary.observable, EffectKnowledge::Yes);
            assert_eq!(summary.crash, EffectKnowledge::Yes);
            assert_eq!(summary.services, vec![service]);
            assert_eq!(summary.revision, unit.identity);
            assert!(
                summary
                    .support
                    .contains(&PsiProvenance::Operation(service_support))
            );
        }
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
            AnalysisKind::OwnershipFrontiers,
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
    fn ownership_frontiers_are_exact_and_never_retained_across_revisions() {
        let mut original = unit(
            vec![function(100, 1, vec![(1, Terminator::Return)])],
            b"ownership-original",
        );
        let fact = OwnershipFrontierFact::new(
            original.terminal_psi,
            original.functions[0].machine,
            OwnershipFrontierSite::BlockEntry(original.functions[0].entry),
            OwnershipFrontierSnapshot {
                claims: Vec::new(),
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        );
        original.ownership_frontier_facts = vec![fact.clone()];
        original.identity = recompute_psi_optimization_unit_identity(&original);

        let AnalysisProduct::OwnershipFrontiers(frontiers) =
            compute_analysis(&original, AnalysisKind::OwnershipFrontiers).unwrap()
        else {
            unreachable!()
        };
        let projected = frontiers
            .fact(fact.machine, fact.site)
            .expect("exact source site is queryable");
        assert_eq!(projected.identity, fact.identity);
        assert_eq!(projected.snapshot, fact.snapshot);
        assert_eq!(projected.revision, original.identity);

        let mut changed = unit(
            vec![function(
                100,
                1,
                vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
            )],
            b"ownership-changed",
        );
        changed.ownership_frontier_facts = vec![fact];
        changed.identity = recompute_psi_optimization_unit_identity(&changed);

        let mut manager = AnalysisManager::new(&original);
        manager
            .require(&original, AnalysisKind::OwnershipFrontiers)
            .unwrap();
        let commit = manager
            .commit_revision(&changed, AnalysisInvalidationSet::default(), true)
            .unwrap();
        assert_eq!(commit.invalidated, vec![AnalysisKind::OwnershipFrontiers]);
        assert!(commit.retained.is_empty());
        let AnalysisProduct::OwnershipFrontiers(rebound) = manager
            .require(&changed, AnalysisKind::OwnershipFrontiers)
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(rebound.facts[0].revision, changed.identity);
    }

    #[test]
    fn analysis_manager_rejects_stale_content_on_require_cold_and_commit_paths() {
        let valid = unit(
            vec![function(100, 1, vec![(1, Terminator::Return)])],
            b"valid-analysis-content",
        );
        let mut stale = valid.clone();
        stale.functions[0].blocks[0].nodes[0].effect.output += 1;
        let recomputed = recompute_psi_optimization_unit_identity(&stale);
        let is_stale = |error: AnalysisManagerError| {
            matches!(
                error,
                AnalysisManagerError::StaleUnitIdentity {
                    stored,
                    recomputed: actual,
                } if stored == stale.identity && actual == recomputed
            )
        };

        let mut require_manager = AnalysisManager::new(&valid);
        assert!(is_stale(
            require_manager
                .require(&stale, AnalysisKind::ControlFlowGraph)
                .unwrap_err()
        ));
        assert!(is_stale(
            AnalysisManager::compute_cold_parallel(
                &stale,
                AnalysisSet::new([AnalysisKind::ControlFlowGraph]),
            )
            .unwrap_err()
        ));
        let mut commit_manager = AnalysisManager::new(&valid);
        assert!(is_stale(
            commit_manager
                .commit_revision(&stale, AnalysisInvalidationSet::default(), false)
                .unwrap_err()
        ));
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
                    ScalarConstantSupport {
                        operations: vec![boolean_support],
                        edges: vec![id(13, EdgeId::new)],
                    }
                ),
                (
                    ExecutableEdgeKnowledge::KnownInexecutable,
                    ScalarConstantSupport {
                        operations: vec![boolean_support],
                        edges: Vec::new(),
                    }
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
        assert_eq!(
            ranges.facts[0].support.literal_operation(),
            Some(integer_support)
        );
    }

    #[test]
    fn scalar_constants_merge_only_feasible_block_parameter_bindings() {
        let (selected, parameter, supports, edges) =
            block_parameter_constant_unit(Some(true), IntegerValue::Unsigned(8));
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&selected, AnalysisKind::ScalarConstants).unwrap()
        else {
            unreachable!()
        };
        let fact = constants
            .facts
            .iter()
            .find(|fact| fact.value == parameter)
            .expect("selected incoming constant reaches block parameter");
        assert_eq!(
            fact.constant,
            ScalarConstant::Integer(IntegerValue::Unsigned(7))
        );
        assert!(fact.identity.is_some());
        assert_eq!(fact.support.operations, vec![supports[0], supports[1]]);
        assert_eq!(fact.support.edges, vec![edges[0], edges[2]]);
        let AnalysisProduct::ExecutableEdges(executable) =
            compute_analysis(&selected, AnalysisKind::ExecutableEdges).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            executable
                .edges
                .iter()
                .map(|fact| (fact.edge, fact.knowledge))
                .collect::<Vec<_>>(),
            vec![
                (edges[0], ExecutableEdgeKnowledge::KnownExecutable),
                (edges[1], ExecutableEdgeKnowledge::KnownInexecutable),
                (edges[2], ExecutableEdgeKnowledge::KnownExecutable),
                (edges[3], ExecutableEdgeKnowledge::KnownInexecutable),
            ]
        );

        let (same, parameter, supports, edges) =
            block_parameter_constant_unit(None, IntegerValue::Unsigned(7));
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&same, AnalysisKind::ScalarConstants).unwrap()
        else {
            unreachable!()
        };
        let fact = constants
            .facts
            .iter()
            .find(|fact| fact.value == parameter)
            .expect("equal feasible incoming values meet to one constant");
        assert_eq!(
            fact.constant,
            ScalarConstant::Integer(IntegerValue::Unsigned(7))
        );
        assert!(fact.identity.is_some());
        assert_eq!(fact.support.operations, vec![supports[1], supports[2]]);
        assert_eq!(fact.support.edges, edges);
        let AnalysisProduct::ExecutableEdges(executable) =
            compute_analysis(&same, AnalysisKind::ExecutableEdges).unwrap()
        else {
            unreachable!()
        };
        assert!(
            executable
                .edges
                .iter()
                .all(|fact| { fact.knowledge == ExecutableEdgeKnowledge::KnownExecutable })
        );

        let (different, parameter, _, _) =
            block_parameter_constant_unit(None, IntegerValue::Unsigned(8));
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&different, AnalysisKind::ScalarConstants).unwrap()
        else {
            unreachable!()
        };
        assert!(constants.facts.iter().all(|fact| fact.value != parameter));
        assert!(
            analysis_dependencies(AnalysisKind::ScalarConstants)
                .unwrap()
                .contains(AnalysisKind::ControlFlowGraph)
        );
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
        let independent = omega_optimization_validation::reconstruct_closed_scalar_node_boundary(
            &unit,
            omega_optimization_unit::NodeLocation {
                machine: id(100, MachineId::new),
                block: id(1, BlockId::new),
                node: 1,
            },
        )
        .unwrap();
        assert_eq!(independent.live_in, liveness.blocks[0].nodes[1].entry);
        assert_eq!(independent.live_out, liveness.blocks[0].nodes[1].exit);
    }
}
