//! Exact invocation positions for rejoining retained argument producers.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarTerminator,
};
use semantic_vocabulary::{BlockId, InvocationSite, OperationId};

/// The input-use cut in the current function. A terminal invocation follows
/// its complete block prefix; it never borrows an instruction's identity.
pub(in crate::selection) fn use_cut(
    source: &LegalizedScalarFunction,
    site: InvocationSite,
) -> Option<(BlockId, usize)> {
    let mut found = None;
    for block in &source.blocks {
        match site {
            InvocationSite::Operation(operation) => {
                for (position, row) in block.instructions.iter().enumerate() {
                    if row.operation == operation && found.replace((block.id, position)).is_some() {
                        return None;
                    }
                }
            }
            InvocationSite::InvokeNoNormal(edge) => {
                if matches!(&block.terminator,
                    LegalizedScalarTerminator::InvokeNoNormal { psi_edge, .. } if *psi_edge == edge)
                    && found
                        .replace((block.id, block.instructions.len()))
                        .is_some()
                {
                    return None;
                }
            }
            InvocationSite::ExternalCompletion(edge) => {
                if matches!(&block.terminator,
                    LegalizedScalarTerminator::ExternalCompletion { psi_edge, .. } if *psi_edge == edge)
                    && found
                        .replace((block.id, block.instructions.len()))
                        .is_some()
                {
                    return None;
                }
            }
        }
    }
    found
}

/// The exact producer precedes the invocation inside its defining block.
/// Across blocks, selected def/use validation independently proves dominance
/// of its physical home; geometry cannot confer that lifetime authority.
pub(in crate::selection) fn producer_before_use(
    source: &LegalizedScalarFunction,
    producer: OperationId,
    site: InvocationSite,
) -> Option<(BlockId, usize, &LegalizedScalarInstruction)> {
    let used = use_cut(source, site)?;
    let mut producers = source.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .enumerate()
            .filter(move |(_, row)| row.operation == producer)
            .map(move |(position, row)| (block.id, position, row))
    });
    let produced = producers.next()?;
    if producers.next().is_some() || produced.0 == used.0 && produced.1 >= used.1 {
        return None;
    }
    Some(produced)
}

/// Rejoin the signature of the exact nonreceiving invocation. The receiving
/// result is absent; its declared result remains part of the source ABI.
pub(in crate::selection) fn no_normal_result<'source>(
    source: &'source LegalizedScalarFunction,
    site: InvocationSite,
    call: &legalized_operations::LegalizedScalarCall,
) -> Option<&'source abstract_operations::AbstractFunctionResult> {
    let InvocationSite::InvokeNoNormal(edge) = site else {
        return None;
    };
    let (block, position) = use_cut(source, site)?;
    let block = source
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?;
    if position != block.instructions.len() {
        return None;
    }
    match &block.terminator {
        LegalizedScalarTerminator::InvokeNoNormal {
            psi_edge,
            call: retained,
            declared_result,
            ..
        } if *psi_edge == edge && retained == call => Some(declared_result),
        _ => None,
    }
}
