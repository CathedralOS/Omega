//! Exact consuming occurrence coordinates shared by ordinary and no-normal calls.
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, InvocationSite};
use terminal_psi::StructuralArgument;

pub(super) fn matches(operation: &AbstractOperation, site: InvocationSite) -> bool {
    match site {
        InvocationSite::Operation(identity) => operation.psi_operation() == Some(identity),
        InvocationSite::InvokeNoNormal(edge) => matches!(operation,
            AbstractOperation::InvokeNoNormal { psi_edge, .. } if *psi_edge == edge),
        InvocationSite::ExternalCompletion(edge) => matches!(operation,
            AbstractOperation::ExternalCompletion { psi_edge, .. } if *psi_edge == edge),
    }
}

pub(super) fn position(
    function: &PsiOptimizationFunction,
    site: InvocationSite,
    argument: Option<&StructuralArgument>,
) -> Option<(BlockId, usize)> {
    let mut sites = function.blocks.iter().flat_map(|block| {
        block
            .nodes
            .iter()
            .enumerate()
            .filter_map(move |(position, node)| {
                if !matches(&node.operation, site) {
                    return None;
                }
                let arguments = node
                    .operation
                    .module_invocation()
                    .map(|call| call.structural_arguments)
                    .or_else(|| match &node.operation {
                        abstract_operations::boundary_or_structural_call_pair_pattern!(
                            structural_arguments,
                            ..
                        ) => Some(structural_arguments.as_slice()),
                        _ => None,
                    });
                if argument.is_some_and(|argument| {
                    !arguments.is_some_and(|arguments| arguments.contains(argument))
                }) {
                    return None;
                }
                Some((block.id, position))
            })
    });
    let position = sites.next()?;
    sites.next().is_none().then_some(position)
}
