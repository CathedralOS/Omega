//! Ordinary and structural function live-range construction.
use super::{
    FunctionLiveRanges, RegisterOperandAccess, VirtualFixedConstraint, VirtualFixedConstraintSite,
    VirtualLiveRange, VirtualOccurrence,
};
use super::{
    LiveRangeError, architectural_units, block_domain, connector, derive_early_clobbers,
    derive_tied_pairs, operand_point, register_fragments,
};

pub(crate) fn compute_function(
    function_index: usize,
    selected: &selected_instructions::SelectedFunction,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    reject_unsupported(function_index, liveness)?;
    let block_domains = liveness
        .blocks
        .iter()
        .map(|block| block_domain(function_index, block))
        .collect::<Result<Vec<_>, _>>()?;
    let tied_pairs = derive_tied_pairs(function_index, liveness)?;
    let edge_transfers = super::edge_transfers::derive(function_index, selected, liveness)?;
    let copy_affinities = super::copy_affinities::derive(function_index, selected)?;
    let early_clobbers = derive_early_clobbers(function_index, liveness)?;

    // Group the function's operand rows, live fragments and live edges by
    // register once; each register's range then reads only its own rows, in
    // the order a scan of the whole function would have met them.
    let register_count = selected.virtual_registers.len();
    let max_reg_id = selected
        .virtual_registers
        .iter()
        .map(|register| register.id.0 as usize)
        .max()
        .unwrap_or(0);
    let mut register_index = vec![u32::MAX; max_reg_id + 1];
    for (idx, register) in selected.virtual_registers.iter().enumerate() {
        let slot = &mut register_index[register.id.0 as usize];
        if *slot == u32::MAX {
            *slot = idx as u32;
        }
    }
    let mut fragment_rows =
        register_fragments(function_index, liveness, register_count, &register_index)?;
    let operand_slice = liveness.operand_positions.storage_slice();
    let mut operand_counts = vec![0u32; register_count];
    for row in operand_slice {
        if let Some(&idx) = register_index.get(row.virtual_register.0 as usize)
            && idx != u32::MAX
        {
            operand_counts[idx as usize] += 1;
        }
    }
    let mut operand_offsets = vec![0u32; register_count + 1];
    for i in 0..register_count {
        operand_offsets[i + 1] = operand_offsets[i] + operand_counts[i];
    }
    let mut operand_cursors = operand_offsets.clone();
    let mut operand_indices = vec![0u32; operand_offsets[register_count] as usize];
    for (row_idx, row) in operand_slice.iter().enumerate() {
        if let Some(&idx) = register_index.get(row.virtual_register.0 as usize)
            && idx != u32::MAX
        {
            let cursor = &mut operand_cursors[idx as usize];
            operand_indices[*cursor as usize] = row_idx as u32;
            *cursor += 1;
        }
    }
    let has_edges = liveness.blocks.iter().any(|b| !b.successors.is_empty());
    let mut edge_rows = if has_edges {
        vec![Vec::new(); register_count]
    } else {
        Vec::new()
    };
    if has_edges {
        for block in &liveness.blocks {
            for edge in &block.successors {
                let mut prev = None;
                for &register in &edge.virtual_live {
                    if prev == Some(register) {
                        continue;
                    }
                    prev = Some(register);
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                    {
                        edge_rows[idx as usize].push(connector(block.block, edge));
                    }
                }
            }
        }
    }

    let mut virtual_rows = Vec::with_capacity(register_count);
    for (pos, register) in selected.virtual_registers.iter().enumerate() {
        let canonical_idx = register_index[register.id.0 as usize] as usize;
        let row_start = operand_offsets[canonical_idx] as usize;
        let row_end = operand_offsets[canonical_idx + 1] as usize;
        let row_indices = &operand_indices[row_start..row_end];
        let mut occurrences = Vec::with_capacity(row_indices.len());
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for &row_idx in row_indices {
            let row = &operand_slice[row_idx as usize];
            let point = operand_point(function_index, row.position, row.access)?;
            occurrences.push(VirtualOccurrence {
                position: row.position,
                point,
                instruction: row.instruction,
                operand: row.operand,
                access: row.access,
            });
            if let Some(view) = row.fixed_view {
                fixed_constraints.push(VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Operand {
                        position: row.position,
                        point,
                        instruction: row.instruction,
                        operand: row.operand,
                        access: row.access,
                    },
                    view,
                });
            }
        }
        let fragments = if pos == canonical_idx {
            std::mem::take(&mut fragment_rows[canonical_idx])
        } else {
            Vec::new()
        };
        let edge_connectors = if has_edges {
            edge_rows[canonical_idx].clone()
        } else {
            Vec::new()
        };
        virtual_rows.push(VirtualLiveRange {
            virtual_register: register.id,
            class: register.class,
            occurrences,
            fixed_constraints,
            fragments,
            edge_connectors,
        });
    }

    let architectural_units = architectural_units(function_index, liveness)?;

    let interference = super::interference(&virtual_rows);
    Ok(FunctionLiveRanges {
        machine: selected.machine,
        block_domains,
        virtual_registers: virtual_rows,
        tied_pairs,
        edge_transfers,
        copy_affinities,
        early_clobbers,
        architectural_units,
        interference,
    })
}

fn reject_unsupported(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<(), LiveRangeError> {
    for operand in liveness.operand_positions.storage_slice() {
        if operand.access == RegisterOperandAccess::UseDef {
            return Err(LiveRangeError::UnsupportedUseDef {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            });
        }
    }
    Ok(())
}
