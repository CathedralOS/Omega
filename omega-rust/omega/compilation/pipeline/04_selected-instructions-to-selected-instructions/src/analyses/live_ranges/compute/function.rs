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
    let registers = selected
        .virtual_registers
        .iter()
        .map(|register| register.id)
        .collect::<BTreeSet<_>>();
    let mut fragment_rows = register_fragments(function_index, liveness, &registers)?;
    let mut operand_rows = std::collections::BTreeMap::<_, Vec<_>>::new();
    for row in &liveness.operand_positions {
        operand_rows
            .entry(row.virtual_register)
            .or_default()
            .push(row);
    }
    let mut edge_rows = std::collections::BTreeMap::<_, Vec<_>>::new();
    for block in &liveness.blocks {
        for edge in &block.successors {
            for register in edge.virtual_live.iter().collect::<BTreeSet<_>>() {
                edge_rows
                    .entry(*register)
                    .or_default()
                    .push(connector(block.block, edge));
            }
        }
    }

    let mut virtual_rows = Vec::with_capacity(selected.virtual_registers.len());
    for register in &selected.virtual_registers {
        let rows = operand_rows
            .get(&register.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
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
        let fragments = fragment_rows.remove(&register.id).unwrap_or_default();
        let edge_connectors = edge_rows.get(&register.id).cloned().unwrap_or_default();
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
        interference: interference.into_iter().collect(),
    })
}

fn reject_unsupported(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
) -> Result<(), LiveRangeError> {
    for operand in &liveness.operand_positions {
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
