//! Expand edges that dispose an unmatched sum or mixed value into explicit
//! `Terminator::StructuralCase` dispatches so each arm establishes the active
//! case before component cleanup runs (ownership.md: "only the active sum
//! payload").

use crate::lower::machine_order::ModuleWalks;
use arena::IdTable;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, PlaceId, StructuralTypeId};
use terminal_check_tables::edges::{Destination, EdgeRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{CheckTables, Checked, MachineTables};
use terminal_psi::{
    Block, StructuralCaseSuccessorEdge, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalModule, TerminalRankedScc, Terminator,
};
use terminal_semantics::plain_affine::{Error, Site};

/// Bound per-machine expansions so a malformed cyclic graph cannot spin.
const MAX_EXPANSIONS_PER_MACHINE: usize = 64;

/// Expand every edge whose affine disposal needs an active sum case.
pub(crate) fn expand_unmatched_sums(
    module: &mut TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
) -> Checked<()> {
    let has_case_cleanup = module.types.structural_types.iter().any(|declaration| {
        matches!(
            declaration.shape,
            StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. }
        ) && declaration.nominal_cleanup.is_none()
            && (!declaration.no_code_affine_cleanup || !declaration.descriptor_cleanup)
    });
    if !has_case_cleanup {
        return Ok(());
    }
    // Machines are walked in order, each up to `MAX_EXPANSIONS_PER_MACHINE`
    // times, until a walk finds no edge to expand. Walks over the unchanged
    // module share one `ModuleWalks` round; only an expansion changes the
    // module, so only an expansion ends that round (`machine_order`).
    let machine_count = module.machines.len();
    let mut machine_index = 0;
    let mut walks_of_machine = 0;
    while machine_index < machine_count {
        let missing = {
            let module = &*module;
            let walks = ModuleWalks::new(module);
            loop {
                let Some(machine) = module.machines.get(machine_index) else {
                    break None;
                };
                if !machine.structural_places.is_empty()
                    && walks_of_machine < MAX_EXPANSIONS_PER_MACHINE
                {
                    walks_of_machine += 1;
                    if let Some(missing) = missing_active_case(&walks, machine.id) {
                        break Some(missing);
                    }
                }
                machine_index += 1;
                walks_of_machine = 0;
            }
        };
        let Some((edge, place, path, structural_type)) = missing else {
            break;
        };
        let machine_id = module.machines[machine_index].id;
        let expanded = match (
            tables.machines.get_mut(machine_id),
            sources.get_mut(machine_id),
        ) {
            (Some(machine_tables), Some(machine_sources)) => expand_edge(
                &module.types.structural_types,
                &mut module.machines[machine_index],
                machine_tables,
                machine_sources,
                edge,
                place,
                path,
                structural_type,
            ),
            _ => false,
        };
        if !expanded {
            machine_index += 1;
            walks_of_machine = 0;
        }
    }
    Ok(())
}

/// An edge whose disposal needs an active sum case, with the place, path
/// and sum type it disposes.
type MissingCase = (
    EdgeId,
    PlaceId,
    Vec<StructuralPathSegment>,
    StructuralTypeId,
);

/// The edge of `machine` whose disposal needs an active sum case, or `None`
/// when its ownership walk finds no such edge (or refuses for any other
/// reason, which planning reports later).
fn missing_active_case(walks: &ModuleWalks<'_>, machine: MachineId) -> Option<MissingCase> {
    match walks.affine(machine).and_then(|walks| walks.plan()) {
        Err(Error::MissingActiveCase {
            place,
            path,
            structural_type,
            site: Some(Site::Edge(edge)),
        }) => Some((edge, place, path, structural_type)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn expand_edge(
    types: &[StructuralTypeDeclaration],
    machine: &mut TerminalMachine,
    tables: &mut MachineTables,
    sources: &mut MachineSources,
    edge: EdgeId,
    place: PlaceId,
    path: Vec<StructuralPathSegment>,
    structural_type: StructuralTypeId,
) -> bool {
    let Some(declaration) = types.get(structural_type.get().saturating_sub(1) as usize) else {
        return false;
    };
    let cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }
            if !cases.is_empty() =>
        {
            cases.as_slice()
        }
        _ => return false,
    };
    let Some(block_index) = machine.blocks.iter().position(|block| {
        block
            .terminator
            .cleanup_rosters()
            .into_iter()
            .any(|(candidate, _)| candidate == edge)
    }) else {
        return false;
    };
    let block_id = machine.blocks[block_index].id;
    let Some(origin) = sources
        .edges
        .get(edge)
        .copied()
        .or_else(|| sources.blocks.get(block_id).copied())
    else {
        return false;
    };

    let mint_block = |machine: &mut TerminalMachine,
                      sources: &mut MachineSources,
                      terminator: Terminator|
     -> Option<BlockId> {
        let next = u64::try_from(machine.blocks.len() + 1).ok()?;
        let id = BlockId::new(next)?;
        let pushed = sources.blocks.push(origin);
        debug_assert_eq!(pushed, id);
        machine.blocks.push(Block {
            assembly_exit: None,
            id,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator,
        });
        Some(id)
    };
    let mint_edge = |tables: &mut MachineTables,
                     sources: &mut MachineSources,
                     source: BlockId,
                     destination: Destination|
     -> EdgeId {
        let id = tables.edges.push(EdgeRow {
            source,
            destination,
            dispositions: Vec::new(),
        });
        let pushed = sources.edges.push(origin);
        debug_assert_eq!(pushed, id);
        id
    };

    match &machine.blocks[block_index].terminator {
        Terminator::ReturnUnit { edge: e, .. }
        | Terminator::Return { edge: e, .. }
        | Terminator::ReturnStructural { edge: e, .. }
        | Terminator::Jump { edge: e, .. }
            if *e == edge =>
        {
            let original = std::mem::replace(
                &mut machine.blocks[block_index].terminator,
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions: Vec::new(),
                },
            );
            let Some(cont_block) = mint_block(machine, sources, original) else {
                return false;
            };
            if let Some(row) = tables.edges.get_mut(edge) {
                row.source = cont_block;
            }
            update_ranked_scc(machine, edge, cont_block, &[cont_block]);
            let case_edges = cases
                .iter()
                .map(|case| StructuralCaseSuccessorEdge {
                    edge: mint_edge(tables, sources, block_id, Destination::Block(cont_block)),
                    target: cont_block,
                    case: case.id,
                    payload_fields: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                })
                .collect();
            machine.blocks[block_index].terminator = Terminator::StructuralCase {
                source: place,
                path,
                cases: case_edges,
            };
            true
        }
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } if when_true.edge == edge || when_false.edge == edge => {
            let is_true = when_true.edge == edge;
            let (orig_target, orig_args, orig_erased, orig_proof, orig_structural, orig_cleanup) = {
                let Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut machine.blocks[block_index].terminator
                else {
                    unreachable!()
                };
                let branch = if is_true { when_true } else { when_false };
                (
                    branch.target,
                    std::mem::take(&mut branch.arguments),
                    std::mem::take(&mut branch.erased_arguments),
                    std::mem::take(&mut branch.erased_proof_arguments),
                    std::mem::take(&mut branch.structural_arguments),
                    std::mem::take(&mut branch.cleanup_actions),
                )
            };
            let Some(cont_block) = mint_block(
                machine,
                sources,
                Terminator::Jump {
                    edge,
                    target: orig_target,
                    arguments: orig_args,
                    erased_arguments: orig_erased,
                    erased_proof_arguments: orig_proof,
                    structural_arguments: orig_structural,
                    cleanup_actions: orig_cleanup,
                },
            ) else {
                return false;
            };
            if let Some(row) = tables.edges.get_mut(edge) {
                row.source = cont_block;
            }
            let Some(dispatch_block) = mint_block(
                machine,
                sources,
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions: Vec::new(),
                },
            ) else {
                return false;
            };
            let case_edges = cases
                .iter()
                .map(|case| StructuralCaseSuccessorEdge {
                    edge: mint_edge(
                        tables,
                        sources,
                        dispatch_block,
                        Destination::Block(cont_block),
                    ),
                    target: cont_block,
                    case: case.id,
                    payload_fields: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                })
                .collect();
            machine.blocks[(dispatch_block.get() - 1) as usize].terminator =
                Terminator::StructuralCase {
                    source: place,
                    path,
                    cases: case_edges,
                };
            let to_dispatch = mint_edge(
                tables,
                sources,
                block_id,
                Destination::Block(dispatch_block),
            );
            let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut machine.blocks[block_index].terminator
            else {
                unreachable!()
            };
            let branch = if is_true { when_true } else { when_false };
            branch.edge = to_dispatch;
            branch.target = dispatch_block;
            update_ranked_scc(machine, edge, cont_block, &[dispatch_block, cont_block]);
            true
        }
        Terminator::StructuralCase {
            cases: existing_cases,
            ..
        } => {
            let Some(arm_index) = existing_cases.iter().position(|arm| arm.edge == edge) else {
                return false;
            };
            if !existing_cases[arm_index].payload_fields.is_empty()
                || !existing_cases[arm_index].structural_arguments.is_empty()
            {
                return false;
            }
            let orig_target = existing_cases[arm_index].target;
            let Some(dispatch_block) = mint_block(
                machine,
                sources,
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions: Vec::new(),
                },
            ) else {
                return false;
            };
            let case_edges = cases
                .iter()
                .map(|case| StructuralCaseSuccessorEdge {
                    edge: mint_edge(
                        tables,
                        sources,
                        dispatch_block,
                        Destination::Block(orig_target),
                    ),
                    target: orig_target,
                    case: case.id,
                    payload_fields: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                })
                .collect();
            machine.blocks[(dispatch_block.get() - 1) as usize].terminator =
                Terminator::StructuralCase {
                    source: place,
                    path,
                    cases: case_edges,
                };
            let Terminator::StructuralCase {
                cases: existing_cases,
                ..
            } = &mut machine.blocks[block_index].terminator
            else {
                unreachable!()
            };
            existing_cases[arm_index].target = dispatch_block;
            if let Some(row) = tables.edges.get_mut(edge) {
                row.destination = Destination::Block(dispatch_block);
            }
            update_ranked_scc(machine, edge, block_id, &[dispatch_block]);
            true
        }
        _ => false,
    }
}

fn update_ranked_scc(
    machine: &mut TerminalMachine,
    edge: EdgeId,
    new_source: BlockId,
    _added_blocks: &[BlockId],
) {
    let Some(TerminalRankedScc::Natural(cycles)) = &mut machine.ranked_scc else {
        return;
    };
    for cycle in cycles {
        for cycle_edge in &mut cycle.edges {
            if cycle_edge.edge == edge {
                cycle_edge.source = new_source;
            }
        }
    }
}
