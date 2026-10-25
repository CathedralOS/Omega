//! Ordinary and structural function live-range construction.
use super::{
    BTreeSet, FunctionLiveRanges, RegisterOperandAccess, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualLiveRange, VirtualOccurrence,
};
use super::{
    LiveRangeError, architectural_units, block_domain, connector, derive_early_clobbers,
    derive_tied_pairs, operand_point, register_fragments,
};

pub(super) fn compute_function(
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
    let mut operand_rows = vec![Vec::new(); register_count];
    for row in liveness.operand_positions.storage_slice() {
        if let Some(&idx) = register_index.get(row.virtual_register.0 as usize)
            && idx != u32::MAX
        {
            operand_rows[idx as usize].push(row);
        }
    }
    let mut edge_rows = vec![Vec::new(); register_count];
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

    let mut virtual_rows = Vec::with_capacity(register_count);
    for (pos, register) in selected.virtual_registers.iter().enumerate() {
        let canonical_idx = register_index[register.id.0 as usize] as usize;
        let rows = operand_rows[canonical_idx].as_slice();
        let occurrences = rows
            .iter()
            .map(|row| {
                Ok(VirtualOccurrence {
                    position: row.position,
                    point: operand_point(function_index, row.position, row.access)?,
                    instruction: row.instruction,
                    operand: row.operand,
                    access: row.access,
                })
            })
            .collect::<Result<Vec<_>, LiveRangeError>>()?;
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for row in rows {
            if let Some(view) = row.fixed_view {
                fixed_constraints.push(VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Operand {
                        position: row.position,
                        point: operand_point(function_index, row.position, row.access)?,
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
        let edge_connectors = edge_rows[canonical_idx].clone();
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
