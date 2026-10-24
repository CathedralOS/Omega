//! Complete-graph membership transport through copied and moved payload operands.
//! Copying through a descriptor observes its captured backing at that operation,
//! not the descriptor's own empty roster. Intersect every possible source and
//! propagate completed copies until dependencies stop growing. Owned arrival
//! declarations retain their intended receiving roster: a source can supply
//! additional facts without strengthening an explicitly weakened value home.
//! Calls additionally preserve facts about the caller's same shared referent
//! after their normal effects, through the checked per-leaf origin relation.
//! This does not strengthen the public result declaration or expose private
//! implementation qualifications. Receiving declarations supply no new rows. Independent Terminal validation
//! replays each copy and every edge/store after this producer elaboration.
//! Late additions to an ordinary move come from its exact current selected
//! source occurrence, not its initial declaration. This does not repair stale
//! rows already seeded by earlier emission. Ordinary reference formation also
//! follows the actual completed backing at its operation cut: an exclusive loan
//! retains those declared restoration debts even when a private arrival's
//! annotation predates delayed view-membership completion. Equal storage
//! geometry supplies no membership, and block arrivals retain their intended
//! receiving contracts.
use crate::lowering_error::LoweringError;
mod membership_reachability;
mod operand_memberships;
mod view_memberships;
use terminal_psi::{OperationKind, OperationResult, TerminalModule};
use terminal_semantics::descriptor_results::{self as descriptors, PayloadMembershipContract};

pub(crate) fn retain(module: &mut TerminalModule) -> Result<(), LoweringError> {
    let memberships = membership_reachability::MembershipReachability::new(
        &module.structural_types,
        module
            .structural_domains
            .iter()
            .map(|domain| domain.carrier),
    );
    let types = module.structural_types.clone();
    let graph = descriptors::terminal_graph(module);
    let mut analysis = descriptors::DescriptorAnalysis::new(&types, &graph);
    loop {
        let mut copies = Vec::new();
        let reference_memberships = terminal_semantics::ReferenceResultMemberships::new(module);
        for (machine_index, machine) in module.machines.iter().enumerate() {
            let mut history = descriptors::PayloadMembershipHistory::new(
                &types,
                &module.structural_domains,
                &graph,
                machine,
            );
            for (block_index, block) in machine.blocks.iter().enumerate() {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("membership operation ordinal overflow")
                        })?,
                    );
                    if let Some(result) = operation.result.structural()
                        && let Ok(projected_qualifications) =
                            reference_memberships.for_operation(machine, operation)
                        && !projected_qualifications.is_empty()
                    {
                        copies.push((
                            machine_index,
                            block_index,
                            ordinal,
                            PayloadMembershipContract {
                                scalar_observations: Vec::new(),
                                scalar_requirements: Vec::new(),
                                carrier: result.structural_type,
                                qualifications: Vec::new(),
                                projected_qualifications,
                            },
                        ));
                    }
                    if let Some(contract) = view_memberships::retained(
                        module,
                        machine,
                        operation,
                        position,
                        &mut analysis,
                    ) {
                        copies.push((machine_index, block_index, ordinal, contract));
                    }
                    if let Some(contract) = operand_memberships::retained(
                        module,
                        machine,
                        operation,
                        position,
                        &mut history,
                        &mut analysis,
                        &memberships,
                    ) {
                        copies.push((machine_index, block_index, ordinal, contract));
                    }
                    if let OperationKind::MoveStructuralValue { source } = &operation.kind
                        && let Some(result) = operation.result.structural()
                        && let Ok(root) = history.current_payload_contract(position, source.place)
                        && terminal_semantics::qualification_path_carrier(
                            types.iter(),
                            root.carrier,
                            &source.path,
                        ) == Some(result.structural_type)
                    {
                        let (qualifications, projected_qualifications) =
                            terminal_psi::StructuralPathQualification::select(
                                &root.qualifications,
                                &root.projected_qualifications,
                                &source.path,
                            );
                        copies.push((
                            machine_index,
                            block_index,
                            ordinal,
                            PayloadMembershipContract {
                                scalar_observations: Vec::new(),
                                scalar_requirements: Vec::new(),
                                carrier: result.structural_type,
                                qualifications,
                                projected_qualifications,
                            },
                        ));
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
        drop(reference_memberships);
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
