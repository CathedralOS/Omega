//! Scalar source availability is indexed by source CFG dominance, not target block order.
use super::super::{BlockId, ValueId};
use super::{PsiOptimizationFunction, TargetControlGraph, TargetUnitOperation};
use target_operations::TargetUnitScalarArgumentSource as Source;

/// Resolve projection selectors at the call, excluding later definitions.
pub(in crate::legalization::scalar_graph_input) fn call_indices(
    native: &target_operations::TargetOperationPlan,
    caller: &PsiOptimizationFunction,
    call_operation: semantic_vocabulary::OperationId,
    elements: &[crate::structural_inputs::structural_reference_input::RuntimeElement],
) -> Result<Vec<target_operations::TargetStructuralRuntimeIndex>, crate::LegalizationError> {
    if elements.is_empty() {
        return Ok(Vec::new());
    }
    let function = native
        .function(caller.machine)
        .ok_or(crate::LegalizationError::custody())?;
    let (call_block, call_position) = function
        .graph
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .position(|operation| {
                    matches!(operation, TargetUnitOperation::Call { psi_operation, .. }
                        | TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                    if *psi_operation == call_operation)
                })
                .map(|position| (block, position))
        })
        .ok_or(crate::LegalizationError::custody())?;
    let dominators = SourceDominators::new(caller);
    let mut sources = available(&function.graph, caller, &dominators, call_block.block);
    sources.extend(
        call_block.operations[..call_position]
            .iter()
            .filter_map(definition),
    );
    elements
        .iter()
        .map(|element| {
            let (_, operand) = sources
                .iter()
                .find(|(value, _)| *value == element.index)
                .ok_or(crate::LegalizationError::custody())?;
            Ok(target_operations::TargetStructuralRuntimeIndex {
                operand: *operand,
                stride: element.stride,
            })
        })
        .collect()
}

/// Values available at entry to `block`: the function's parameters, `block`'s
/// own parameters, and the parameters and definitions of every block that
/// dominates it, in target block order.
pub(in crate::legalization::scalar_graph_input) fn available(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    dominators: &SourceDominators,
    block: BlockId,
) -> Vec<(ValueId, Source)> {
    let mut result = optimized
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.value,
                Source::Parameter {
                    parameter_index: index as u32,
                    source_value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            )
        })
        .collect::<Vec<_>>();
    for candidate in &graph.blocks {
        if dominators.dominates(candidate.block, block) {
            result.extend(candidate.parameters.iter().map(|parameter| {
                (
                    parameter.value,
                    Source::BlockParameter(target_operations::TargetScalarBlockValue {
                        block: candidate.block,
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    }),
                )
            }));
            if candidate.block != block {
                result.extend(candidate.operations.iter().filter_map(definition));
            }
        }
    }
    result
}

// Candidate rows are independently replayed against their exact source nodes by
// the caller; collecting their descriptions does not authorize a producer or use.
pub(in crate::legalization::scalar_graph_input) fn definition(
    operation: &TargetUnitOperation,
) -> Option<(ValueId, Source)> {
    match operation {
        TargetUnitOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => Some((
            *result,
            Source::IeeeFloatImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                value: *value,
            },
        )),
        TargetUnitOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralScalarFieldRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralCaseMembership {
            psi_operation,
            result,
            ..
        } => Some((
            result.value,
            Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: result.value,
                scalar_type: result.scalar_type,
                shape: super::super::super::scalar_shape(result.scalar_type)?,
            }),
        )),
        TargetUnitOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => Some((
            *result,
            Source::BooleanImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                value: *value,
            },
        )),
        TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => Some((
            *result,
            Source::IntegerImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: *scalar_type,
                value: *value,
            },
        )),
        TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            ..
        } => Some((
            *result,
            Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(*format),
                shape: super::super::super::scalar_shape(
                    semantic_vocabulary::ScalarType::IeeeFloat(*format),
                )?,
            }),
        )),
        TargetUnitOperation::SelectImplementation { result_home, .. }
        // An instruction-observed atomic result is an immutable SSA value,
        // even when a later call can modify the original resident. Reconstruct
        // its definition like every other ordered result home; unit replay
        // independently authenticates the event, address and exact result.
        | TargetUnitOperation::AtomicAccess { result_home: Some(result_home), .. }
        | TargetUnitOperation::PlacedScalarAccess { result_home: Some(result_home), .. }
        | TargetUnitOperation::IeeeFloatCompare { result_home, .. }
        | TargetUnitOperation::ScalarRepresentationView { result_home, .. }
        | TargetUnitOperation::FloatResultPolicy { result_home, .. }
        | TargetUnitOperation::NearestIeeeFloatArithmetic { result_home, .. }
        | TargetUnitOperation::ScalarDefinition { result_home, .. }
        | TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Scalar(result_home),
            ..
        }
        | TargetUnitOperation::NormalizedForeignCall {
            result: target_operations::TargetBoundaryResult::Scalar(result_home),
            ..
        } => Some((result_home.source_value, Source::Home(*result_home))),
        _ => None,
    }
}

/// Whether every path from `function`'s entry to `block` passes `candidate`.
/// A block dominates itself, and every block dominates one the entry cannot
/// reach. Callers asking about many blocks of one function build
/// [`SourceDominators`] once instead.
pub(in crate::legalization::scalar_graph_input) fn dominates(
    function: &PsiOptimizationFunction,
    candidate: BlockId,
    block: BlockId,
) -> bool {
    SourceDominators::new(function).dominates(candidate, block)
}

/// The dominator tree of one source function's control-flow graph, computed
/// once so dominance queries stop re-walking the graph.
///
/// Built with the iterative algorithm of Cooper, Harvey and Kennedy ("A Simple,
/// Fast Dominance Algorithm") over reverse postorder from the entry. Blocks
/// are the function's own blocks; an edge target that names no block has no
/// successors, as in a walk of `nodes[..].successors`.
pub(in crate::legalization::scalar_graph_input) struct SourceDominators {
    /// Reverse-postorder position of each reachable block.
    order: std::collections::BTreeMap<BlockId, usize>,
    /// Immediate dominator of each reachable block, by reverse-postorder
    /// position; the entry is its own.
    immediate: Vec<usize>,
}

impl SourceDominators {
    pub(in crate::legalization::scalar_graph_input) fn new(
        function: &PsiOptimizationFunction,
    ) -> Self {
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id, index))
            .collect::<std::collections::BTreeMap<_, _>>();
        let successors = |block: BlockId| {
            blocks
                .get(&block)
                .into_iter()
                .flat_map(|index| function.blocks[*index].successors().map(|edge| edge.target))
        };
        // Postorder by an explicit stack: (block, successors not yet visited).
        let mut postorder = Vec::new();
        let mut visited = std::collections::BTreeSet::from([function.entry]);
        let mut stack = vec![(
            function.entry,
            successors(function.entry).collect::<Vec<_>>(),
        )];
        while let Some((block, pending)) = stack.last_mut() {
            if let Some(next) = pending.pop() {
                if visited.insert(next) {
                    let next_successors = successors(next).collect();
                    stack.push((next, next_successors));
                }
            } else {
                postorder.push(*block);
                stack.pop();
            }
        }
        postorder.reverse();
        let order = postorder
            .iter()
            .enumerate()
            .map(|(position, block)| (*block, position))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut predecessors = vec![Vec::new(); postorder.len()];
        for (position, block) in postorder.iter().enumerate() {
            for successor in successors(*block) {
                if let Some(target) = order.get(&successor) {
                    predecessors[*target].push(position);
                }
            }
        }
        const UNDEFINED: usize = usize::MAX;
        let mut immediate = vec![UNDEFINED; postorder.len()];
        if !immediate.is_empty() {
            immediate[0] = 0;
        }
        let intersect = |immediate: &[usize], mut left: usize, mut right: usize| {
            while left != right {
                while left > right {
                    left = immediate[left];
                }
                while right > left {
                    right = immediate[right];
                }
            }
            left
        };
        let mut changed = true;
        while changed {
            changed = false;
            for position in 1..postorder.len() {
                let mut processed = predecessors[position]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor] != UNDEFINED);
                let Some(first) = processed.next() else {
                    continue;
                };
                let candidate = processed.fold(first, |current, predecessor| {
                    intersect(&immediate, current, predecessor)
                });
                if immediate[position] != candidate {
                    immediate[position] = candidate;
                    changed = true;
                }
            }
        }
        Self { order, immediate }
    }

    /// Whether every path from the entry to `block` passes `candidate`; see
    /// [`dominates`].
    pub(in crate::legalization::scalar_graph_input) fn dominates(
        &self,
        candidate: BlockId,
        block: BlockId,
    ) -> bool {
        if candidate == block {
            return true;
        }
        let Some(mut position) = self.order.get(&block).copied() else {
            return true;
        };
        let Some(candidate) = self.order.get(&candidate).copied() else {
            return false;
        };
        while position != 0 {
            position = self.immediate[position];
            if position == candidate {
                return true;
            }
        }
        candidate == 0
    }
}
