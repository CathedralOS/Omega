//! Optimizer module role: admission leaf. Dispatch-shape and incoming-edge
//! predicates the producer enumeration and the independent validator share.
//!
//! Proposal decides *which* edges enter a plan; validation must never rerun
//! that decision through the producer's own plan — a matcher cannot attest to
//! itself. Both sides share only these predicates: the dispatch's block-shape
//! evidence and the per-edge admissibility that turns one qualifying incoming
//! traversal into its fused `SpecializedStateEdge` row.

use super::{
    BlockId, NodeLocation, O, OptimizationBlock, OptimizationEdge, OptimizationNode,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarConstant, ScalarConstantAnalysis,
    SpecializedStateEdge,
};
use semantic_vocabulary::{EdgeId, IntegerType, IntegerValue, MachineId, ScalarType, ValueId};
use std::cmp::Ordering;

/// The comparison an in-block dispatch condition computes against its bound
/// operand. Mirrors the three integer comparison operations lowered literal
/// guards and ordering guards produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerComparisonOrdering {
    Equal,
    LessThan,
    LessOrEqual,
}

/// How a dispatch `Conditional` resolves one bound argument's proven constant
/// into the arm that edge takes. Two admitted shapes share the same fused
/// traversal — "this edge, then the resolved arm" — differing only in how the
/// condition reads the block's own scalar parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DispatchCondition {
    /// The condition is the block's own scalar parameter directly — a
    /// Boolean state argument resolves on a proven `ScalarConstant::Boolean`.
    DirectParameter,
    /// The condition is produced in-block by `parameter CMP bound`, where
    /// `bound` is the literal an in-block `IntegerConstant` carries — an
    /// integer state argument resolves on a proven `ScalarConstant::Integer`
    /// evaluated under the operand type's own ordering via
    /// `IntegerType::compare`.
    IntegerComparison {
        ordering: IntegerComparisonOrdering,
        bound: IntegerValue,
        parameter_on_left: bool,
        integer_type: IntegerType,
    },
}

impl DispatchCondition {
    /// The Boolean the dispatch condition takes when the bound argument is
    /// `constant`, or `None` when the constant is not the kind this condition
    /// resolves — a Boolean argument can never answer an integer comparison
    /// and vice versa.
    fn resolve(&self, constant: &ScalarConstant) -> Option<bool> {
        match (self, constant) {
            (Self::DirectParameter, ScalarConstant::Boolean(value)) => Some(*value),
            (
                Self::IntegerComparison {
                    ordering,
                    bound,
                    parameter_on_left,
                    integer_type,
                },
                ScalarConstant::Integer(argument),
            ) => {
                let (left, right) = if *parameter_on_left {
                    (*argument, *bound)
                } else {
                    (*bound, *argument)
                };
                let order = integer_type.compare(left, right)?;
                Some(match ordering {
                    IntegerComparisonOrdering::Equal => order == Ordering::Equal,
                    IntegerComparisonOrdering::LessThan => order == Ordering::Less,
                    IntegerComparisonOrdering::LessOrEqual => order != Ordering::Greater,
                })
            }
            _ => None,
        }
    }
}

/// The dispatch context every admitted edge resolves against: the owning
/// function, the block's own state-argument parameter, the condition shape
/// that parameter feeds, and the conditional's two arm edges. `None` from
/// [`dispatch_evidence`] when `dispatch` is not an eligible dispatch state —
/// an entry block, a block with structural parameters, a block whose
/// terminator is not a `Conditional` reading one of its own scalar
/// parameters (directly, or through an in-block integer comparison against a
/// literal), a conditional with an unresolved arm edge, or a state argument
/// the sparse constant analysis already proves globally constant
/// (constant-conditional-fold custody, not specialization).
pub(super) struct DispatchEvidence<'a> {
    pub(super) unit: &'a PsiOptimizationUnit,
    pub(super) function: &'a PsiOptimizationFunction,
    pub(super) machine: MachineId,
    pub(super) dispatch: BlockId,
    pub(super) parameter: ValueId,
    pub(super) condition: DispatchCondition,
    pub(super) when_true: &'a OptimizationEdge,
    pub(super) when_false: &'a OptimizationEdge,
}

/// The dispatch context for one block, or `None` when it is not an
/// admissible dispatch state.
pub(super) fn dispatch_evidence<'a>(
    unit: &'a PsiOptimizationUnit,
    function: &'a PsiOptimizationFunction,
    dispatch: BlockId,
    constants: &ScalarConstantAnalysis,
) -> Option<DispatchEvidence<'a>> {
    let machine = function.machine;
    let block = function.blocks.iter().find(|block| block.id == dispatch)?;
    if block.id == function.entry || !block.structural_parameters.is_empty() {
        return None;
    }
    let (node, prefix) = block.nodes.split_last()?;
    let O::Conditional {
        condition,
        when_true,
        when_false,
    } = &node.operation
    else {
        return None;
    };
    let (parameter, condition_kind) = if prefix.is_empty() {
        // The dispatch reads a state argument directly: the condition must be
        // one of this block's own scalar parameters.
        let parameter = block
            .parameters
            .iter()
            .find(|parameter| parameter.value == *condition)?;
        (parameter.value, DispatchCondition::DirectParameter)
    } else {
        // A non-Boolean state argument reaches the conditional through an
        // in-block comparison: `parameter CMP literal`. Every other node must
        // be a pure scalar constant — no side effects, calls, or custody may
        // hide inside the fused traversal.
        integer_comparison_condition(block, prefix, *condition)?
    };
    // A globally constant state argument is constant-conditional-fold custody,
    // not specialization. This family owns the edge-exact case where the
    // function-wide lattice cannot prove the parameter.
    if constants.facts.iter().any(|fact| {
        fact.valid_in.machine == machine
            && fact.valid_in.revision == unit.identity
            && fact.value == parameter
    }) {
        return None;
    }
    let arm_edge = |edge: EdgeId| {
        node.successors
            .iter()
            .find(|successor| successor.psi_edge == edge)
    };
    Some(DispatchEvidence {
        unit,
        function,
        machine,
        dispatch,
        parameter,
        condition: condition_kind,
        when_true: arm_edge(when_true.psi_edge)?,
        when_false: arm_edge(when_false.psi_edge)?,
    })
}

/// The integer-comparison dispatch shape, or `None` when `prefix` is not a
/// pure scalar computation of `condition` as `parameter CMP literal`: every
/// prefix node must be a scalar constant, except exactly one integer
/// comparison producing `condition` whose operands pair one of the block's
/// own scalar parameters with an in-block `IntegerConstant`. Returns the
/// parameter and the condition descriptor the bound argument resolves
/// against.
fn integer_comparison_condition(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    condition: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let mut resolved = None;
    for node in prefix {
        let (ordering, result, left, right) = match &node.operation {
            O::IntegerConstant { .. } | O::BooleanConstant { .. } => continue,
            O::IntegerEqual {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::Equal, *result, *left, *right),
            O::IntegerLessThan {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::LessThan, *result, *left, *right),
            O::IntegerLessOrEqual {
                result,
                left,
                right,
                ..
            } => (
                IntegerComparisonOrdering::LessOrEqual,
                *result,
                *left,
                *right,
            ),
            _ => return None,
        };
        if result != condition || resolved.is_some() {
            return None;
        }
        resolved = Some(comparison_operands(block, prefix, ordering, left, right)?);
    }
    resolved
}

/// Split one comparison's operands into the dispatch parameter (one of the
/// block's own scalar parameters, on exactly one side) and the bound literal
/// (an in-block `IntegerConstant`, on the other). The operand's declared
/// integer type — shared by both sides under checked semantics — is the type
/// the comparison evaluates under.
fn comparison_operands(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    ordering: IntegerComparisonOrdering,
    left: ValueId,
    right: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let left_is_parameter = block.parameters.iter().any(|p| p.value == left);
    let right_is_parameter = block.parameters.iter().any(|p| p.value == right);
    let (parameter, bound, parameter_on_left) = match (left_is_parameter, right_is_parameter) {
        (true, false) => (left, right, true),
        (false, true) => (right, left, false),
        _ => return None,
    };
    let (bound_value, integer_type) = prefix.iter().find_map(|node| {
        let O::IntegerConstant {
            result,
            scalar_type,
            value,
            ..
        } = &node.operation
        else {
            return None;
        };
        if *result != bound {
            return None;
        }
        let ScalarType::Integer(integer_type) = scalar_type else {
            return None;
        };
        Some((*value, *integer_type))
    })?;
    Some((
        parameter,
        DispatchCondition::IntegerComparison {
            ordering,
            bound: bound_value,
            parameter_on_left,
            integer_type,
        },
    ))
}

/// Every edge entering `dispatch`, with its owner coordinates — the owner
/// block, the node index inside it, the node, and the edge itself. Proposal
/// enumerates this roster to select candidates; validation counts it to
/// reject a declared set that would fuse every incoming edge and leave the
/// dispatch state unreachable.
pub(super) fn incoming_edges(
    function: &PsiOptimizationFunction,
    dispatch: BlockId,
) -> Vec<(BlockId, usize, &OptimizationNode, &OptimizationEdge)> {
    let mut incoming = Vec::new();
    for owner_block in &function.blocks {
        for (node_index, owner_node) in owner_block.nodes.iter().enumerate() {
            for edge in &owner_node.successors {
                if edge.target == dispatch {
                    incoming.push((owner_block.id, node_index, owner_node, edge));
                }
            }
        }
    }
    incoming
}

/// The admissibility of one incoming edge under `evidence`, or `None` when
/// the edge may not specialize. Two predecessor shapes may fuse: an
/// unconditional `Jump` whose single successor is this exact edge, or one arm
/// of a `Conditional` predecessor — the sibling arm then executes exactly as
/// before, so the dispatch stays reachable along it or another unfused edge.
/// No affine or structural custody may ride either side of the fused
/// traversal, the edge must bind the state argument to a value proven to be
/// one exact constant the dispatch condition can resolve — a Boolean for a
/// direct parameter read, an integer for an in-block literal comparison —
/// and the arm that constant resolves must carry no custody and reach a
/// block without structural parameters. Two constant-proof sources admit the
/// bound argument: the sparse constant analysis proves the argument's own
/// value, or — after the bound argument is resolved through
/// single-predecessor forwarding-block parameters to the delivered value —
/// the delivered value is the scalar result of an in-function direct `Call`
/// whose callee's single `Return` carries a value the callee's own lattice
/// proves constant — the result specialization.
pub(super) fn admit_incoming_edge(
    evidence: &DispatchEvidence<'_>,
    owner_block: BlockId,
    node_index: usize,
    owner_node: &OptimizationNode,
    edge: &OptimizationEdge,
    constants: &ScalarConstantAnalysis,
) -> Option<SpecializedStateEdge> {
    let predecessor = NodeLocation {
        machine: evidence.machine,
        block: owner_block,
        node: u32::try_from(node_index).ok()?,
    };
    match &owner_node.operation {
        O::Jump {
            psi_edge,
            target,
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            if *psi_edge != edge.psi_edge
                || *target != evidence.dispatch
                || !structural_bindings.is_empty()
                || !trivial_affine_discards.is_empty()
                || !residual_affine_discards.is_empty()
            {
                return None;
            }
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            // The admitted edge is exactly one arm of the conditional; the
            // sibling arm is not part of this traversal and keeps its own
            // custody untouched. A conditional successor carries no residual
            // affine discards, so only the arm's own fields can refuse.
            let arm = [when_true, when_false]
                .into_iter()
                .find(|arm| arm.psi_edge == edge.psi_edge)?;
            if arm.target != evidence.dispatch
                || !arm.structural_bindings.is_empty()
                || !arm.trivial_affine_discards.is_empty()
            {
                return None;
            }
        }
        _ => return None,
    }
    if !edge.structural_bindings.is_empty()
        || !edge.trivial_affine_discards.is_empty()
        || !edge.residual_affine_discards.is_empty()
    {
        return None;
    }
    let binding = edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == evidence.parameter)?;
    let fact = constants.facts.iter().find(|fact| {
        fact.valid_in.machine == evidence.machine
            && fact.valid_in.revision == evidence.unit.identity
            && fact.value == binding.argument
    });
    // The sparse lattice proves the argument's own value, or — when it
    // cannot — the argument may still resolve to the proven-constant result
    // of an in-function direct call, which the lattice leaves overdefined.
    // An evaluated state-call argument is delivered through forwarding-block
    // parameters, so the delivered value is resolved before the call proof.
    let argument_constant = match fact {
        Some(fact) => fact.constant,
        None => {
            let delivered = delivered_argument(evidence, owner_block, binding.argument);
            call_result_constant(evidence, delivered, constants)?
        }
    };
    let constant = evidence.condition.resolve(&argument_constant)?;
    let (resolved, rejected) = if constant {
        (evidence.when_true, evidence.when_false)
    } else {
        (evidence.when_false, evidence.when_true)
    };
    if !resolved.structural_bindings.is_empty()
        || !resolved.trivial_affine_discards.is_empty()
        || !resolved.residual_affine_discards.is_empty()
    {
        return None;
    }
    let resolved_block = evidence
        .function
        .blocks
        .iter()
        .find(|candidate| candidate.id == resolved.target)?;
    if !resolved_block.structural_parameters.is_empty() {
        return None;
    }
    Some(SpecializedStateEdge {
        incoming_edge: edge.psi_edge,
        predecessor,
        parameter: evidence.parameter,
        argument: binding.argument,
        constant,
        taken_edge: resolved.psi_edge,
        rejected_edge: rejected.psi_edge,
        resolved_target: resolved.target,
    })
}

/// The value actually delivered to the dispatch parameter by `argument`.
/// The abstract-operations lowering evaluates a state-call argument in the
/// caller's continuation and delivers it through single-predecessor
/// forwarding blocks, so the bound argument is often a parameter of the
/// edge's owner block rather than the produced value itself. While the
/// argument is a parameter of its owner block and that block has exactly
/// one incoming edge, follow the unique binding to the predecessor's
/// supplied value — a merge-free trace: a parameter with several incoming
/// edges is a genuine join and stops the walk unresolved, and a value that
/// is not a parameter of its owner block is the produced result itself.
/// The walk is bounded by the block count — the machine is acyclic when it
/// reaches admission (cyclic components are declined first), so following
/// single-predecessor edges visits each block at most once.
fn delivered_argument(
    evidence: &DispatchEvidence<'_>,
    owner_block: BlockId,
    argument: ValueId,
) -> ValueId {
    let mut owner_block = owner_block;
    let mut argument = argument;
    for _ in 0..evidence.function.blocks.len() {
        let Some(block) = evidence
            .function
            .blocks
            .iter()
            .find(|block| block.id == owner_block)
        else {
            break;
        };
        if !block
            .parameters
            .iter()
            .any(|parameter| parameter.value == argument)
        {
            break;
        }
        // `argument` is a parameter of `owner_block`: it is exactly the value
        // the block's single incoming edge binds to it, or — when several
        // edges reach the block — a genuine join the walk cannot resolve.
        let mut incoming = evidence.function.blocks.iter().flat_map(|candidate| {
            candidate.nodes.iter().flat_map(move |node| {
                node.successors
                    .iter()
                    .filter(move |successor| successor.target == owner_block)
                    .map(move |successor| (candidate.id, successor))
            })
        });
        let (Some((predecessor_block, predecessor_edge)), None) =
            (incoming.next(), incoming.next())
        else {
            break;
        };
        let Some(binding) = predecessor_edge
            .bindings
            .iter()
            .find(|binding| binding.parameter == argument)
        else {
            break;
        };
        argument = binding.argument;
        owner_block = predecessor_block;
    }
    argument
}

/// The result-specialization proof for one delivered argument: the argument
/// is the scalar `result` of an in-function direct `Call` whose callee's
/// function contains exactly one `Return` node, and the sparse constant
/// lattice proves that returned value constant inside the callee's own
/// machine. The call itself still executes at the predecessor — only its
/// proven result resolves the dispatch — so no callee purity is required,
/// and a cyclic callee is harmless because every produced result is the
/// constant no matter which path reached the single return. Dynamic,
/// structural, and boundary calls produce their results through different
/// operations and never match; a callee with several `Return` nodes or a
/// non-constant return admits nothing.
fn call_result_constant(
    evidence: &DispatchEvidence<'_>,
    argument: ValueId,
    constants: &ScalarConstantAnalysis,
) -> Option<ScalarConstant> {
    let callee = evidence
        .function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::Call { result, callee, .. } if *result == argument => Some(*callee),
            _ => None,
        })?;
    let callee_function = evidence
        .unit
        .functions
        .iter()
        .find(|function| function.machine == callee)?;
    let mut returned = callee_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::Return { value, .. } => Some(*value),
            _ => None,
        });
    let value = returned.next()?;
    if returned.next().is_some() {
        return None;
    }
    let fact = constants.facts.iter().find(|fact| {
        fact.valid_in.machine == callee
            && fact.valid_in.revision == evidence.unit.identity
            && fact.value == value
    })?;
    Some(fact.constant)
}
