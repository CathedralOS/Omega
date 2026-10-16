//! Received dynamic rows must refine the requirement's public result regions.
//!
//! Source checking owns this relation during production. Reconstruct descriptor
//! origins only at decoding, where a foreign artifact has no trusted producer;
//! encoding a module we just produced must not replay its bodies.

use crate::codec_error::{CodecError, malformed};
use terminal_psi::{BoundaryMachineResult, TerminalModule};

/// Validate every table row, including rows with no local call site. Replay is
/// prepared once at this trust boundary and shared across selected row queries.
pub(super) fn validate_dynamic_descriptor_results(
    module: &TerminalModule,
) -> Result<(), CodecError> {
    let mut rows = Vec::new();
    for table in &module.dynamic_dispatch.tables {
        let interface = module.dynamic_dispatch.interface(table.interface).ok_or(
            CodecError::MalformedStructuralFoundation("dynamic table has no interface"),
        )?;
        if table.rows.len() != interface.slots.len() {
            return malformed("dynamic table row count disagrees with its interface");
        }
        for (row, slot) in table.rows.iter().zip(&interface.slots) {
            let Some(row) = row else {
                continue;
            };
            let BoundaryMachineResult::Structural(result) = &slot.result else {
                continue;
            };
            let leaves = terminal_semantics::borrowed_result_leaves(
                module.structural_types.iter(),
                result.structural_type,
                result.multiplicity,
                slot.result_reference_regions.len(),
            )
            .map_err(CodecError::MalformedStructuralFoundation)?;
            if leaves
                .iter()
                .any(|leaf| matches!(leaf.kind, terminal_semantics::BorrowedResultLeafKind::View))
            {
                rows.push((*row, slot));
            }
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
        &module.structural_types,
        &graph,
    );
    for (row, slot) in rows {
        let callee = module
            .machine(row)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "dynamic table row has no machine",
            ))?;
        let result =
            callee
                .result
                .structural()
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "dynamic view result row has no structural result",
                ))?;
        if !slot.result.matches_machine_result(&callee.result) {
            return malformed("dynamic view result row disagrees with its slot");
        }
        let slots = analysis
            .result_slots(row)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "dynamic row has unknown returned descriptor contents",
            ))?;
        terminal_semantics::validate_descriptor_result_regions(
            &module.structural_types,
            &callee.structural_parameters,
            result,
            &slot.result_reference_regions,
            &slots,
        )
        .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    Ok(())
}
