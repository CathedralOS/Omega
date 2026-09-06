//! Publish scalar leaves from already validated selected fragments. Encoding,
//! allocation and frame application have finished before this projection.

use super::{LeafEvidence, leaf_function};
use machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, MachineCodeFunction,
    ScalarControlFlowEvidence, SemanticCodeAttribution, SemanticCodeSite,
};
use target_operations::FixedIntegerScalarFunctionAbi;

pub(super) fn project_function(
    fragment: &FunctionFragment,
    abi: &FixedIntegerScalarFunctionAbi,
    architecture: target::Architecture,
) -> Result<MachineCodeFunction, &'static str> {
    let [block] = fragment.blocks.as_slice() else {
        return Err("scalar native publication requires one straight-line block");
    };
    let Some((last, body)) = block.instructions.split_last() else {
        return Err("scalar native publication requires a return");
    };
    let FunctionFragmentControlProvenance::Return { psi_return_edge } = last.control else {
        return Err("scalar native publication requires a final return edge");
    };
    if fragment.provenance.edges.as_slice() != [psi_return_edge]
        || body
            .iter()
            .any(|instruction| instruction.control != FunctionFragmentControlProvenance::None)
        || block.instructions.iter().any(|instruction| {
            instruction.branch.is_some() || instruction.internal_machine_fixup.is_some()
        })
    {
        return Err("scalar native publication does not admit calls or branching");
    }
    let mut attribution = Vec::new();
    for instruction in &block.instructions {
        let code_offset = usize::try_from(instruction.offset)
            .map_err(|_| "scalar instruction offset exceeds host size")?;
        for operation in &instruction.provenance.operations {
            let operation_ordinal = fragment
                .provenance
                .operations
                .iter()
                .position(|candidate| candidate == operation)
                .ok_or("scalar instruction names a foreign semantic operation")?;
            attribution.push(SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(*operation),
                operation_ordinal,
                code_offset,
                byte_count: instruction.bytes.len(),
            });
        }
    }
    let return_offset =
        usize::try_from(last.offset).map_err(|_| "scalar return offset exceeds host size")?;
    attribution.push(SemanticCodeAttribution {
        site: SemanticCodeSite::Edge(psi_return_edge),
        operation_ordinal: fragment.provenance.operations.len(),
        code_offset: return_offset,
        byte_count: fragment
            .bytes
            .len()
            .checked_sub(return_offset)
            .ok_or("scalar return exceeds function bytes")?,
    });
    let stack = crate::scalar::collect_scalar_stack_evidence(
        architecture,
        &fragment.bytes,
        ScalarControlFlowEvidence::Linear,
        None,
    )
    .map_err(|_| "scalar fragment stack instructions are invalid")?;
    Ok(leaf_function(
        fragment,
        LeafEvidence::Scalar {
            abi: abi.clone(),
            stack,
            attribution,
        },
    ))
}
