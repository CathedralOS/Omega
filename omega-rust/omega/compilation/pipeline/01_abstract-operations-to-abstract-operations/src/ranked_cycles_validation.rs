/*
Verified-cycle topology and countdown ranking-certificate replay
(`ValidatedOptimizerCycleComponents`, `ValidatedOptimizerRankingCertificates`,
`validate_exact_ranked_cycles`) across Terminal Psi inputs and transformed
abstract-optimization units.
*/

use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::MachineId;

pub(crate) mod components {
    use crate::validation::BTreeMap;
    use semantic_vocabulary::BlockId;

    use crate::cycles::strongly_connected_components;

    pub(crate) fn cyclic_components(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
        strongly_connected_components(graph)
            .into_iter()
            .filter(|component| {
                component.len() > 1
                    || component
                        .first()
                        .is_some_and(|block| graph[block].contains(block))
            })
            .collect()
    }
}
pub(crate) mod countdown_ranking {
    use super::{
        OptimizationUnitValidationError, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
        OptimizerRankingCertificateSnapshot, PsiOptimizationUnit,
    };
    pub(crate) mod current {
        use crate::validation::O;
        use abstract_operations::{IntegerArithmeticOperator, IntegerOverflowPolicy};
        use optimization_unit::{OptimizationBlock, OptimizationNode, PsiOptimizationFunction};
        use semantic_vocabulary::{
            BlockId, IntegerCarrier, IntegerSign, IntegerValue, ScalarType, ValueId,
        };

        use super::super::{
            OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
            OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
        };
        use super::{
            OptimizationUnitValidationError, OptimizerCycleComponentSnapshot, PsiOptimizationUnit,
        };
        pub(crate) mod invariant_constants {
            use semantic_vocabulary::{IntegerType, OperationId};

            use optimization_unit::{NodeLocation, PsiProvenance, ValueDefinitionSite};

            use super::{
                BlockId, IntegerValue, O, OptimizerCycleComponent, PsiOptimizationFunction,
                ScalarType, ValueId,
            };
            #[derive(Clone, Copy)]
            pub(crate) struct ResolvedInvariantConstant {
                pub(crate) result: ValueId,
                pub(crate) operation: OperationId,
                pub(crate) location: NodeLocation,
            }

            pub(crate) fn resolve(
                function: &PsiOptimizationFunction,
                component: &OptimizerCycleComponent,
                original_block: BlockId,
                result: ValueId,
                rank_type: IntegerType,
                expected_value: IntegerValue,
            ) -> Result<ResolvedInvariantConstant, ()> {
                let mut definitions = function.blocks.iter().flat_map(|block| {
                    block
                        .nodes
                        .iter()
                        .enumerate()
                        .filter(move |(_, node)| {
                            node.definitions
                                .iter()
                                .any(|definition| definition.value == result)
                        })
                        .map(move |(node, value)| (block.id, node, value))
                });
                let (block, node_index, node) = definitions.next().ok_or(())?;
                if definitions.next().is_some() {
                    return Err(());
                }
                let node_index = u32::try_from(node_index).map_err(|_| ())?;
                let preheader = sole_preheader(function, component).map(|(block, _)| block.id);
                if block != original_block && Some(block) != preheader {
                    return Err(());
                }
                let O::IntegerConstant {
                    psi_operation,
                    result: actual_result,
                    scalar_type,
                    value,
                } = node.operation
                else {
                    return Err(());
                };
                let [definition] = node.definitions.as_slice() else {
                    return Err(());
                };
                if actual_result != result
                    || scalar_type != ScalarType::Integer(rank_type)
                    || value != expected_value
                    || node.provenance.first() != Some(&PsiProvenance::Operation(psi_operation))
                    || definition.value != result
                    || definition.scalar_type != ScalarType::Integer(rank_type)
                    || definition.site
                        != (ValueDefinitionSite::Node {
                            block,
                            node: node_index,
                        })
                    || !node.uses.is_empty()
                    || !node.successors.is_empty()
                    || !node.ownership.is_empty()
                {
                    return Err(());
                }
                Ok(ResolvedInvariantConstant {
                    result,
                    operation: psi_operation,
                    location: NodeLocation {
                        machine: function.machine,
                        block,
                        node: node_index,
                    },
                })
            }

            pub(crate) fn validate_canonical_preheader_suffix(
                function: &PsiOptimizationFunction,
                component: &OptimizerCycleComponent,
                header: BlockId,
                original_blocks_in_role_order: [BlockId; 2],
                locations_in_role_order: [NodeLocation; 2],
            ) -> Result<(), ()> {
                if locations_in_role_order
                    .iter()
                    .zip(original_blocks_in_role_order)
                    .all(|(location, original)| location.block == original)
                {
                    return Ok(());
                }
                let (preheader, jump_index) = sole_preheader(function, component).ok_or(())?;
                let moved = locations_in_role_order
                    .into_iter()
                    .zip(original_blocks_in_role_order)
                    .filter_map(|(location, original)| {
                        (location.block != original).then_some(location)
                    })
                    .collect::<Vec<_>>();
                if moved.iter().any(|location| location.block != preheader.id) {
                    return Err(());
                }
                let O::Jump {
                    psi_edge, target, ..
                } = preheader.nodes[jump_index].operation
                else {
                    return Err(());
                };
                let [entry] = component.entries.as_slice() else {
                    return Err(());
                };
                if psi_edge != entry.edge || target != header || entry.target != header {
                    return Err(());
                }
                let suffix_start = jump_index.checked_sub(moved.len()).ok_or(())?;
                if moved.iter().enumerate().all(|(offset, location)| {
                    location.machine == function.machine
                        && usize::try_from(location.node).ok() == Some(suffix_start + offset)
                }) {
                    Ok(())
                } else {
                    Err(())
                }
            }

            fn sole_preheader<'function>(
                function: &'function PsiOptimizationFunction,
                component: &OptimizerCycleComponent,
            ) -> Option<(&'function optimization_unit::OptimizationBlock, usize)> {
                let [entry] = component.entries.as_slice() else {
                    return None;
                };
                if component.members.contains(&entry.source) {
                    return None;
                }
                let preheader = function.block(entry.source)?;
                preheader
                    .nodes
                    .iter()
                    .enumerate()
                    .next_back()
                    .map(|(jump, _)| (preheader, jump))
            }
        }

        pub(crate) fn derive(
            unit: &PsiOptimizationUnit,
            snapshot: &OptimizerCycleComponentSnapshot,
        ) -> Result<
            Vec<OptimizerUnsignedCountdownRankingCertificate>,
            OptimizationUnitValidationError,
        > {
            let mut certificates = Vec::new();
            for component in &snapshot.components {
                let function = unit
                    .function(component.id.machine)
                    .ok_or_else(|| super::mismatch(component))?;
                certificates.push(derive_one(function, component)?);
            }
            certificates.sort_by(|left, right| left.component.cmp(&right.component));
            Ok(certificates)
        }

        fn derive_one(
            function: &PsiOptimizationFunction,
            component: &OptimizerCycleComponent,
        ) -> Result<OptimizerUnsignedCountdownRankingCertificate, OptimizationUnitValidationError>
        {
            let mut candidates = component
                .members
                .iter()
                .filter_map(|header| derive_from_header(function, component, *header).ok());
            let certificate = candidates
                .next()
                .ok_or_else(|| super::mismatch(component))?;
            if candidates.next().is_some() {
                return Err(super::mismatch(component));
            }
            Ok(certificate)
        }

        fn derive_from_header(
            function: &PsiOptimizationFunction,
            component: &OptimizerCycleComponent,
            header_id: BlockId,
        ) -> Result<OptimizerUnsignedCountdownRankingCertificate, ()> {
            let header = block(function, header_id).ok_or(())?;
            let terminator = header.nodes.last().ok_or(())?;
            let O::Conditional {
                condition,
                when_true,
                ..
            } = &terminator.operation
            else {
                return Err(());
            };
            let comparison = scalar_node(header, *condition).ok_or(())?;
            let O::IntegerLessThan {
                psi_operation: comparison_operation,
                result,
                left: zero,
                right: rank_parameter,
            } = comparison.operation
            else {
                return Err(());
            };
            if result != *condition {
                return Err(());
            }
            let rank_type = header
                .parameters
                .iter()
                .find_map(|parameter| {
                    (parameter.value == rank_parameter).then_some(parameter.scalar_type)
                })
                .and_then(|scalar_type| match scalar_type {
                    ScalarType::Integer(integer) => Some(integer),
                    _ => None,
                })
                .filter(|integer| {
                    integer.carrier() == IntegerCarrier::Fixed
                        && integer.sign() == IntegerSign::Unsigned
                })
                .ok_or(())?;
            let zero = invariant_constants::resolve(
                function,
                component,
                header_id,
                zero,
                rank_type,
                IntegerValue::Unsigned(0),
            )?;
            let backedge = component
                .id
                .internal_edges
                .iter()
                .copied()
                .find(|edge| edge.target == header_id && edge.source == when_true.target)
                .ok_or(())?;
            if when_true.psi_edge == backedge.edge {
                return Err(());
            }
            let decrement = block(function, backedge.source).ok_or(())?;
            let jump = decrement.nodes.last().ok_or(())?;
            let O::Jump {
                psi_edge,
                target,
                bindings,
                ..
            } = &jump.operation
            else {
                return Err(());
            };
            if *psi_edge != backedge.edge || *target != header_id {
                return Err(());
            }
            let (argument_index, binding) = bindings
                .iter()
                .enumerate()
                .find(|(_, binding)| binding.parameter == rank_parameter)
                .ok_or(())?;
            if binding.scalar_type != ScalarType::Integer(rank_type) {
                return Err(());
            }
            let subtract = scalar_node(decrement, binding.argument).ok_or(())?;
            let operands = subtract.operation.integer_arithmetic().ok_or(())?;
            let (
                IntegerOverflowPolicy::Exact,
                IntegerArithmeticOperator::Subtract,
                Some(subtract_obligation),
            ) = (
                operands.arithmetic.policy(),
                operands.arithmetic.operator(),
                operands.arithmetic.obligation(),
            )
            else {
                return Err(());
            };
            let subtract_operation = operands.psi_operation;
            let argument = operands.result;
            let subtract_type = operands.scalar_type;
            let source_parameter = operands.left;
            let one = operands.right;
            let reads_counter = source_parameter == rank_parameter
                || when_true.bindings.iter().any(|guard_binding| {
                    guard_binding.parameter == source_parameter
                        && guard_binding.argument == rank_parameter
                        && guard_binding.scalar_type == ScalarType::Integer(rank_type)
                });
            if argument != binding.argument || subtract_type != rank_type || !reads_counter {
                return Err(());
            }
            let one = invariant_constants::resolve(
                function,
                component,
                backedge.source,
                one,
                rank_type,
                IntegerValue::Unsigned(1),
            )?;
            invariant_constants::validate_canonical_preheader_suffix(
                function,
                component,
                header_id,
                [header_id, backedge.source],
                [zero.location, one.location],
            )?;
            Ok(OptimizerUnsignedCountdownRankingCertificate {
                component: component.id.clone(),
                header: header_id,
                rank_parameter,
                rank_type,
                lower_bound: rank_type.minimum_value(),
                upper_bound: rank_type.maximum_value(),
                guard: OptimizerUnsignedPositiveGuard {
                    block: header_id,
                    edge: when_true.psi_edge,
                    condition: *condition,
                    parameter: rank_parameter,
                    zero: zero.result,
                    zero_operation: zero.operation,
                    comparison_operation,
                },
                descent: OptimizerUnsignedMinusOneDescent {
                    backedge,
                    argument_index: u32::try_from(argument_index).map_err(|_| ())?,
                    argument,
                    source_parameter,
                    target_parameter: binding.parameter,
                    one: one.result,
                    one_operation: one.operation,
                    subtract_operation,
                    subtract_obligation,
                },
            })
        }

        fn block(function: &PsiOptimizationFunction, id: BlockId) -> Option<&OptimizationBlock> {
            function.block(id)
        }

        fn scalar_node(block: &OptimizationBlock, value: ValueId) -> Option<&OptimizationNode> {
            block.nodes.iter().find(|node| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == value)
            })
        }
    }
    pub(crate) mod terminal {
        use crate::validation::{BTreeMap, BTreeSet};
        use semantic_vocabulary::IntegerValue;
        use semantic_vocabulary::ValueId;

        use super::super::{
            CycleComponentEdge, OptimizerCycleComponent,
            OptimizerUnsignedCountdownRankingCertificate, OptimizerUnsignedMinusOneDescent,
            OptimizerUnsignedPositiveGuard,
        };
        use super::OptimizationUnitValidationError;
        pub(crate) fn project(
            module: &::terminal_psi::TerminalModule,
            component: &OptimizerCycleComponent,
        ) -> Result<
            Option<OptimizerUnsignedCountdownRankingCertificate>,
            OptimizationUnitValidationError,
        > {
            let machine = module
                .machine(component.id.machine)
                .ok_or_else(|| super::mismatch(component))?;
            Ok(project_one(machine, component))
        }

        fn project_one(
            machine: &::terminal_psi::TerminalMachine,
            component: &OptimizerCycleComponent,
        ) -> Option<OptimizerUnsignedCountdownRankingCertificate> {
            let ::terminal_psi::TerminalRankedScc::Natural(naturals) =
                machine.ranked_scc.as_ref()?;
            let internal_edges = component
                .id
                .internal_edges
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let natural = naturals.iter().find(|natural| {
                natural
                    .ranks
                    .iter()
                    .map(|rank| rank.block)
                    .collect::<Vec<_>>()
                    == component.members
                    && natural
                        .edges
                        .iter()
                        .map(|edge| CycleComponentEdge {
                            edge: edge.edge,
                            source: edge.source,
                            target: edge.target,
                        })
                        .collect::<BTreeSet<_>>()
                        == internal_edges
            })?;
            let rank_type = natural
                .scalar_rank_value(natural.ranks.first()?.values)?
                .integer_type;
            if rank_type.sign() != semantic_vocabulary::IntegerSign::Unsigned {
                return None;
            }
            let ranks = natural
                .ranks
                .iter()
                .map(|rank| Some((rank.block, natural.scalar_rank_value(rank.values)?.value)))
                .collect::<Option<BTreeMap<_, _>>>()?;

            let strict = natural
                .edges
                .iter()
                .filter(|edge| {
                    edge.comparison == ::terminal_psi::TerminalNaturalRankComparison::Strict
                })
                .collect::<Vec<_>>();
            let [backedge] = strict.as_slice() else {
                return None;
            };
            let successor_rank = natural.scalar_rank_value(backedge.successor_ranks)?.value;
            let header_id = backedge.target;
            let decrement_id = backedge.source;
            let rank_parameter = *ranks.get(&header_id)?;
            let source_parameter = *ranks.get(&decrement_id)?;

            let header = machine.block(&header_id)?;
            let ::terminal_psi::Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &header.terminator
            else {
                return None;
            };
            let guard = when_true;
            if guard.target != decrement_id
                || component.members.contains(&when_false.target)
                || !internal_edges.contains(&CycleComponentEdge {
                    edge: guard.edge,
                    source: header_id,
                    target: decrement_id,
                })
            {
                return None;
            }
            let comparison = scalar_operation(header, *condition)?;
            let ::terminal_psi::OperationKind::IntegerLessThan { left: zero, right } =
                comparison.kind
            else {
                return None;
            };
            if right != rank_parameter {
                return None;
            }
            let zero_operation = scalar_operation(header, zero)?;
            if zero_operation.kind
                != (::terminal_psi::OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0),
                })
            {
                return None;
            }

            let argument_index = header
                .parameters
                .iter()
                .position(|parameter| parameter.id == rank_parameter)?;
            let decrement = machine.block(&decrement_id)?;
            let ::terminal_psi::Terminator::Jump {
                edge: jump_edge,
                target,
                arguments,
                ..
            } = &decrement.terminator
            else {
                return None;
            };
            if *jump_edge != backedge.edge
                || *target != header_id
                || arguments.get(argument_index).copied() != Some(successor_rank)
            {
                return None;
            }
            let subtract = scalar_operation(decrement, successor_rank)?;
            let ::terminal_psi::OperationKind::ExactIntegerSubtract {
                left,
                right: one,
                obligation,
            } = subtract.kind
            else {
                return None;
            };
            if left != source_parameter {
                return None;
            }
            let one_operation = scalar_operation(decrement, one)?;
            if one_operation.kind
                != (::terminal_psi::OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(1),
                })
            {
                return None;
            }
            Some(OptimizerUnsignedCountdownRankingCertificate {
                component: component.id.clone(),
                header: header_id,
                rank_parameter,
                rank_type,
                lower_bound: rank_type.minimum_value(),
                upper_bound: rank_type.maximum_value(),
                guard: OptimizerUnsignedPositiveGuard {
                    block: header_id,
                    edge: guard.edge,
                    condition: *condition,
                    parameter: rank_parameter,
                    zero,
                    zero_operation: zero_operation.id,
                    comparison_operation: comparison.id,
                },
                descent: OptimizerUnsignedMinusOneDescent {
                    backedge: CycleComponentEdge {
                        edge: backedge.edge,
                        source: decrement_id,
                        target: header_id,
                    },
                    argument_index: u32::try_from(argument_index).ok()?,
                    argument: successor_rank,
                    source_parameter,
                    target_parameter: rank_parameter,
                    one,
                    one_operation: one_operation.id,
                    subtract_operation: subtract.id,
                    subtract_obligation: obligation,
                },
            })
        }

        fn scalar_operation(
            block: &::terminal_psi::Block,
            value: ValueId,
        ) -> Option<&::terminal_psi::Operation> {
            block
                .operations
                .iter()
                .find(|operation| operation.result.scalar().map(|result| result.id) == Some(value))
        }
    }

    pub(crate) fn rederive_exact_certificates(
        module: &::terminal_psi::TerminalModule,
        unit: &PsiOptimizationUnit,
        components: &OptimizerCycleComponentSnapshot,
    ) -> Result<OptimizerRankingCertificateSnapshot, OptimizationUnitValidationError> {
        let mut recognized = Vec::new();
        let mut terminal = Vec::new();
        for component in &components.components {
            if let Some(certificate) = self::terminal::project(module, component)? {
                recognized.push(component.clone());
                terminal.push(certificate);
            }
        }
        terminal.sort_by(|left, right| left.component.cmp(&right.component));
        let countdown = OptimizerCycleComponentSnapshot {
            terminal_psi: components.terminal_psi,
            components: recognized,
        };
        let current = current::derive(unit, &countdown)?;
        if terminal != current {
            let machine = components
                .components
                .first()
                .map_or(unit.entry, |component| component.id.machine);
            return Err(
                OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch { machine },
            );
        }
        Ok(OptimizerRankingCertificateSnapshot {
            terminal_psi: components.terminal_psi,
            certificates: current,
        })
    }

    fn mismatch(component: &OptimizerCycleComponent) -> OptimizationUnitValidationError {
        OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch {
            machine: component.id.machine,
        }
    }
}
pub(crate) mod graph {
    use crate::validation::{BTreeMap, BTreeSet, O};
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::BlockId;

    use super::{CycleComponentEdge, MachineId};
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CanonicalControlGraph {
        pub(crate) machine: MachineId,
        pub(crate) blocks: BTreeSet<BlockId>,
        pub(crate) edges: Vec<CycleComponentEdge>,
    }

    impl CanonicalControlGraph {
        pub(crate) fn successors(&self) -> BTreeMap<BlockId, Vec<BlockId>> {
            let mut graph = self
                .blocks
                .iter()
                .copied()
                .map(|block| (block, Vec::new()))
                .collect::<BTreeMap<_, _>>();
            for edge in &self.edges {
                if let Some(successors) = graph.get_mut(&edge.source) {
                    successors.push(edge.target);
                }
            }
            for successors in graph.values_mut() {
                successors.sort_unstable();
                successors.dedup();
            }
            graph
        }
    }

    pub(crate) fn terminal_graph(machine: &terminal_psi::TerminalMachine) -> CanonicalControlGraph {
        let mut edges = Vec::new();
        for block in &machine.blocks {
            match &block.terminator {
                terminal_psi::Terminator::Jump { edge, target, .. } => {
                    edges.push(CycleComponentEdge {
                        edge: *edge,
                        source: block.id,
                        target: *target,
                    });
                }
                terminal_psi::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    edges.extend([when_true, when_false].map(|successor| CycleComponentEdge {
                        edge: successor.edge,
                        source: block.id,
                        target: successor.target,
                    }));
                }
                terminal_psi::Terminator::StructuralCase { cases, .. } => {
                    edges.extend(cases.iter().map(|successor| CycleComponentEdge {
                        edge: successor.edge,
                        source: block.id,
                        target: successor.target,
                    }));
                }
                terminal_psi::Terminator::Return { .. }
                | terminal_psi::Terminator::ReturnUnit { .. }
                | terminal_psi::Terminator::ReturnStructural { .. }
                | terminal_psi::Terminator::ExternalCompletion { .. }
                | terminal_psi::Terminator::InvokeNoNormal { .. }
                | terminal_psi::Terminator::Crash { .. } => {}
            }
        }
        edges.sort_unstable();
        CanonicalControlGraph {
            machine: machine.id,
            blocks: machine.blocks.iter().map(|block| block.id).collect(),
            edges,
        }
    }

    pub(crate) fn optimization_graph(function: &PsiOptimizationFunction) -> CanonicalControlGraph {
        let mut edges = Vec::new();
        for block in &function.blocks {
            let Some(operation) = block.nodes.last().map(|node| &node.operation) else {
                continue;
            };
            match operation {
                O::Jump {
                    psi_edge, target, ..
                } => edges.push(CycleComponentEdge {
                    edge: *psi_edge,
                    source: block.id,
                    target: *target,
                }),
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => edges.extend([when_true, when_false].map(|successor| CycleComponentEdge {
                    edge: successor.psi_edge,
                    source: block.id,
                    target: successor.target,
                })),
                O::StructuralCase { cases, .. } => {
                    edges.extend(cases.iter().map(|successor| CycleComponentEdge {
                        edge: successor.psi_edge,
                        source: block.id,
                        target: successor.target,
                    }));
                }
                _ => {}
            }
        }
        edges.sort_unstable();
        CanonicalControlGraph {
            machine: function.machine,
            blocks: function.blocks.iter().map(|block| block.id).collect(),
            edges,
        }
    }
}
pub(crate) mod ordinary {
    use super::{
        OptimizationUnitValidationError, OptimizerCycleComponentSnapshot, PsiOptimizationUnit,
        graph, topology,
    };
    pub(crate) fn rederive_components(
        module: &terminal_psi::TerminalModule,
        seed_functions: &[abstract_operations::AbstractFunction],
        unit: &PsiOptimizationUnit,
        terminal_psi: terminal_psi::TerminalPsiIdentity,
    ) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
        let mut components = Vec::new();
        for seed in seed_functions {
            let machine = module.machine(seed.machine).ok_or(
                OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                    machine: seed.machine,
                },
            )?;
            let invalid = || OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                machine: machine.id,
            };
            let terminal = topology::derive_terminal_components(machine)?;
            if terminal.is_empty() {
                continue;
            }
            let function = unit.function(machine.id).ok_or_else(invalid)?;
            let current = topology::derive_components(&graph::optimization_graph(function));
            if current != terminal {
                return Err(invalid());
            }
            components.extend(current);
        }
        components.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(OptimizerCycleComponentSnapshot {
            terminal_psi,
            components,
        })
    }
}
pub(crate) mod replay {
    use super::{
        OptimizationUnitValidationError, OptimizerCycleComponentSnapshot, PsiOptimizationUnit,
    };
    pub(crate) fn rederive_exact_components(
        module: &terminal_psi::TerminalModule,
        seed_functions: &[abstract_operations::AbstractFunction],
        unit: &PsiOptimizationUnit,
    ) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
        let terminal_psi = terminal_codec::terminal_psi_identity(module)
            .map_err(OptimizationUnitValidationError::ContextIdentity)?;
        super::ordinary::rederive_components(module, seed_functions, unit, terminal_psi)
    }
}
pub(crate) mod topology {
    use crate::validation::BTreeSet;
    use semantic_vocabulary::BlockId;

    use super::{
        CycleComponentId, OptimizationUnitValidationError, OptimizerCycleComponent, components,
        graph,
    };

    pub(crate) fn derive_components(
        graph: &graph::CanonicalControlGraph,
    ) -> Vec<OptimizerCycleComponent> {
        assemble(graph, components::cyclic_components(&graph.successors()))
    }

    pub(crate) fn derive_terminal_components(
        machine: &terminal_psi::TerminalMachine,
    ) -> Result<Vec<OptimizerCycleComponent>, OptimizationUnitValidationError> {
        let members =
            terminal_semantics::control_graph::control_cycle_members(machine).map_err(|_| {
                OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                    machine: machine.id,
                }
            })?;
        Ok(assemble(&graph::terminal_graph(machine), members))
    }

    fn assemble(
        graph: &graph::CanonicalControlGraph,
        members: Vec<Vec<BlockId>>,
    ) -> Vec<OptimizerCycleComponent> {
        let mut components = members
            .into_iter()
            .map(|members| derive_component(graph, members))
            .collect::<Vec<_>>();
        components.sort_by(|left, right| left.id.cmp(&right.id));
        components
    }

    fn derive_component(
        graph: &graph::CanonicalControlGraph,
        members: Vec<BlockId>,
    ) -> OptimizerCycleComponent {
        let member_set = members.iter().copied().collect::<BTreeSet<_>>();
        let mut internal_edges = Vec::new();
        let mut entries = Vec::new();
        let mut exits = Vec::new();
        for edge in &graph.edges {
            match (
                member_set.contains(&edge.source),
                member_set.contains(&edge.target),
            ) {
                (true, true) => internal_edges.push(*edge),
                (false, true) => entries.push(*edge),
                (true, false) => exits.push(*edge),
                (false, false) => {}
            }
        }
        OptimizerCycleComponent {
            id: CycleComponentId {
                machine: graph.machine,
                internal_edges,
            },
            members,
            entries,
            exits,
        }
    }
}

use optimization_unit::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
};

pub(crate) struct RankedCycleAdmission {
    pub(crate) snapshot: OptimizerCycleComponentSnapshot,
    pub(crate) rankings: OptimizerRankingCertificateSnapshot,
}

pub(crate) fn validate_exact_ranked_cycles(
    input: &verified_optimization_input::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<RankedCycleAdmission, OptimizationUnitValidationError> {
    let snapshot =
        replay::rederive_exact_components(input.context().module(), &input.plan().functions, unit)?;
    let rankings =
        countdown_ranking::rederive_exact_certificates(input.context().module(), unit, &snapshot)?;
    Ok(RankedCycleAdmission { snapshot, rankings })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizerCycleComponents {
    snapshot: OptimizerCycleComponentSnapshot,
    rankings: ValidatedOptimizerRankingCertificates,
}

impl ValidatedOptimizerCycleComponents {
    pub(crate) const fn new(
        snapshot: OptimizerCycleComponentSnapshot,
        rankings: ValidatedOptimizerRankingCertificates,
    ) -> Self {
        Self { snapshot, rankings }
    }

    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.snapshot.terminal_psi
    }

    pub fn components(&self) -> &[OptimizerCycleComponent] {
        &self.snapshot.components
    }

    pub const fn ranking_certificates(&self) -> &ValidatedOptimizerRankingCertificates {
        &self.rankings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizerRankingCertificates {
    snapshot: OptimizerRankingCertificateSnapshot,
}

impl ValidatedOptimizerRankingCertificates {
    pub(crate) const fn new(snapshot: OptimizerRankingCertificateSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn certificates(&self) -> &[OptimizerUnsignedCountdownRankingCertificate] {
        &self.snapshot.certificates
    }

    pub const fn snapshot(&self) -> &OptimizerRankingCertificateSnapshot {
        &self.snapshot
    }
}
