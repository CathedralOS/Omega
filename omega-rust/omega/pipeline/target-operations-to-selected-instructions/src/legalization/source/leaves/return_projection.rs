use super::super::shared::*;
use super::context::LeafContext;
use super::fuel::exact_edge_fuel;

pub(super) fn finalize(
    context: &LeafContext<'_>,
    return_node: &optimization_unit::OptimizationNode,
    psi_return_edge: EdgeId,
    value: SourceLeafValue,
) -> Result<SourceLeaf, LegalizationError> {
    let AbstractOperation::Return {
        psi_edge,
        value: returned_value,
        scalar_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if *psi_edge != psi_return_edge
        || *returned_value != context.source_value
        || *scalar_type != context.u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let return_fuel = exact_edge_fuel(return_node, psi_return_edge, context.function)?;
    if return_node.fuel.len() != return_fuel.len() {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok(SourceLeaf {
        return_edge: psi_return_edge,
        source_value: context.source_value,
        return_fuel,
        value,
    })
}
