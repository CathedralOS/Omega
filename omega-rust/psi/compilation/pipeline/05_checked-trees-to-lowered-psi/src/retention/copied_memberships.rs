//! Complete-graph membership transport for copied payloads.
//! Copying through a descriptor observes its captured backing at that operation,
//! not the descriptor's own empty roster. Intersect every possible source and
//! propagate completed copies until dependencies stop growing. Owned arrival
//! declarations retain their intended receiving roster: a source can supply
//! additional facts without strengthening an explicitly weakened value home.
//! Receiving declarations supply no new rows. Independent Terminal validation
//! replays each copy and every edge/store after this producer elaboration.
use crate::lowering_error::LoweringError;
mod construction_memberships;
mod view_memberships;
use terminal_psi::{OperationKind, OperationResult, TerminalModule};
use terminal_semantics::descriptor_results::{self as descriptors, PayloadMembershipContract};

pub(crate) fn retain(module: &mut TerminalModule) -> Result<(), LoweringError> {
    let types = module.structural_types.clone();
    let graph = descriptors::terminal_graph(module);
    let mut analysis = descriptors::DescriptorAnalysis::new(&types, &graph);
    loop {
        let mut copies = Vec::new();
        for (machine_index, machine) in module.machines.iter().enumerate() {
            for (block_index, block) in machine.blocks.iter().enumerate() {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("membership operation ordinal overflow")
                        })?,
                    );
                    if let Some(contract) = view_memberships::retained(
                        module,
                        machine,
                        operation,
                        position,
                        &mut analysis,
                    ) {
                        copies.push((machine_index, block_index, ordinal, contract));
                    }
                    if let Some(contract) = construction_memberships::retained(
                        module, machine, operation, position, &graph,
                    ) {
                        copies.push((machine_index, block_index, ordinal, contract));
                    }
                    let OperationKind::StructuralLeafCopy { source, path } = &operation.kind else {
                        continue;
                    };
                    let Some(result) = operation.result.structural() else {
                        continue;
                    };
                    let Some(root) = descriptors::terminal_payload_contract(machine, *source)
                    else {
                        continue;
                    };
                    let Some(query) = descriptors::descriptor_contents_query(
                        &types,
                        *source,
                        root.carrier,
                        path,
                        None,
                    ) else {
                        continue;
                    };
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("copy operation ordinal overflow")
                        })?,
                    );
                    let Some(targets) = analysis.payload_targets(machine.id, position, &query)
                    else {
                        continue;
                    };
                    let Some(contract) = descriptors::payload_memberships(
                        &types,
                        &module.structural_domains,
                        targets,
                        |place| descriptors::terminal_payload_contract(machine, place),
                    ) else {
                        continue;
                    };
                    if contract.carrier == result.structural_type {
                        copies.push((machine_index, block_index, ordinal, contract));
                    }
                }
            }
        }
        let mut changed = false;
        for (machine, block, operation, contract) in copies {
            let OperationResult::Structural(result) =
                &mut module.machines[machine].blocks[block].operations[operation].result
            else {
                continue;
            };
            changed |= merge(&mut result.qualifications, contract.qualifications);
            changed |= merge(
                &mut result.projected_qualifications,
                contract.projected_qualifications,
            );
        }
        if !changed {
            return Ok(());
        }
    }
}

fn merge<T: Ord>(destination: &mut Vec<T>, source: Vec<T>) -> bool {
    let old_length = destination.len();
    destination.extend(source);
    destination.sort();
    destination.dedup();
    destination.len() != old_length
}
