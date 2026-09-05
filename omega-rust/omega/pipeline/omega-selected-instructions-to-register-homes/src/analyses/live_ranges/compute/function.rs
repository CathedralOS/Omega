//! Ordinary and structural function live-range construction.

use super::*;

pub(super) fn compute_structural_function(
    function_index: usize,
    machine: psi_core::MachineId,
    liveness: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    if liveness.machine != machine
        || !liveness.entry_definitions.is_empty()
        || !liveness.operand_positions.is_empty()
    {
        return Err(LiveRangeError::FunctionMismatch {
            function: function_index,
        });
    }
    let block_domains = liveness
        .blocks
        .iter()
        .map(|block| block_domain(function_index, block))
        .collect::<Result<Vec<_>, _>>()?;
    let architectural_units = architectural_units(function_index, liveness)?;
    Ok(FunctionLiveRanges {
        machine,
        block_domains,
        virtual_registers: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units,
        interference: Vec::new(),
    })
}

pub(super) fn compute_function(
    function_index: usize,
    selected: &omega_selected_instructions::SelectedFunction,
    liveness: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    reject_unsupported(function_index, liveness)?;
    let block_domains = liveness
        .blocks
        .iter()
        .map(|block| block_domain(function_index, block))
        .collect::<Result<Vec<_>, _>>()?;
    let tied_pairs = derive_tied_pairs(function_index, liveness)?;
    let early_clobbers = derive_early_clobbers(function_index, liveness)?;

    let mut virtual_rows = Vec::with_capacity(selected.virtual_registers.len());
    for register in &selected.virtual_registers {
        let occurrences = liveness
            .operand_positions
            .iter()
            .filter(|row| row.virtual_register == register.id)
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
        for row in liveness
            .operand_positions
            .iter()
            .filter(|row| row.virtual_register == register.id)
        {
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
        let fragments = liveness
            .blocks
            .iter()
            .map(|block| virtual_fragments(function_index, block, register.id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        let edge_connectors = liveness
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .successors
                    .iter()
                    .filter(move |edge| edge.virtual_live.contains(&register.id))
                    .map(move |edge| connector(block.block, edge))
            })
            .collect();
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

    let mut interference = BTreeSet::new();
    for (left_index, left) in virtual_rows.iter().enumerate() {
        for right in virtual_rows.iter().skip(left_index + 1) {
            if fragments_overlap(&left.fragments, &right.fragments) {
                interference.insert(VirtualInterference {
                    lower: left.virtual_register,
                    higher: right.virtual_register,
                });
            }
        }
    }
    Ok(FunctionLiveRanges {
        machine: selected.machine,
        block_domains,
        virtual_registers: virtual_rows,
        tied_pairs,
        early_clobbers,
        architectural_units,
        interference: interference.into_iter().collect(),
    })
}

fn reject_unsupported(
    function: usize,
    liveness: &crate::FunctionLiveness,
) -> Result<(), LiveRangeError> {
    for operand in &liveness.operand_positions {
        let error = if operand.access == RegisterOperandAccess::UseDef {
            Some(LiveRangeError::UnsupportedUseDef {
                function,
                instruction: operand.instruction.0,
                operand: operand.operand,
            })
        } else {
            None
        };
        if let Some(error) = error {
            return Err(error);
        }
    }
    Ok(())
}
