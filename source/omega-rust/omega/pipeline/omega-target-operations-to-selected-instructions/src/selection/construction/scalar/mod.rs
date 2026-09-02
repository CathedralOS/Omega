//! Optimizer module role: executable entrance. Scalar selection: reconstruct context, classify once, build one whole body.
//!
//! `catalog.rs` is the sole family inventory. Each selected leaf returns its
//! virtual-register roster and blocks together so those projections cannot
//! drift through separate source-shape matches.

mod active_resident_exact_add_bridge_chain;
mod active_resident_exact_add_chain;
mod active_resident_exact_add_original_victim_chain;
mod blocks;
mod catalog;
mod comparison_immediate_pair;
mod context;
mod equal_zero_immediate_pair;
mod exact_binary_pair;
mod immediate_pair;
mod model;
mod parameter_pair;
mod registers;

use crate::selection::shared::*;

pub(super) fn build(
    function: usize,
    source: &SourceFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    register_catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let context = context::reconstruct(function, source, constraints, physical, register_catalog)?;
    let body = catalog::build(&context)?;
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: body.virtual_registers,
        blocks: body.blocks,
    })
}
