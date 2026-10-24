//! Instantiate ownership continuations before publishing Terminal identities.
//!
//! The source-authorized cleanup graph supplies the candidate actions and the
//! actual control graph selects their surviving owners. Different frontiers
//! keep separate continuations. Equal frontiers share a continuation exactly
//! when their live captured namespaces agree; the existing block parameters
//! still join ordinary scalar arguments. No source expression is evaluated a
//! second time and no generated runtime liveness state is needed.

use super::continuation_identities::{IdentityMap, IdentityUses};
use super::selected_continuations::{SelectedCleanupGroup, Specialization};
use crate::lowering_error::LoweringError;
use crate::terminal_identities::{
    allocate_dense, block_id, claim_id, edge_id, obligation_id, operation_id, place_id, value_id,
};
use lowered_psi::{LoweredSelectedComparisonOccurrence, LoweredSourceCallOccurrence};
use semantic_vocabulary::{BlockId, EdgeId, ObligationId, StructuralPlaceKind};
use terminal_psi::{
    ScalarBlockInvariant, ScalarQualificationCoercion, StructuralTypeDeclaration, TerminalMachine,
};

mod companions;
mod dependencies;
use dependencies::Definitions;

pub(crate) struct CompanionRows<'a> {
    pub(crate) source_returns:
        &'a mut Vec<crate::expression_preparation::qualifications::SourceScalarReturn>,
    pub(crate) continuation_value_uses: &'a [(BlockId, semantic_vocabulary::ValueId)],
    pub(crate) source_calls: &'a mut Vec<LoweredSourceCallOccurrence>,
    pub(crate) scalar_block_invariants: &'a mut Vec<ScalarBlockInvariant>,
    pub(crate) qualification_coercions: &'a mut Vec<ScalarQualificationCoercion>,
    pub(crate) selected_comparisons: &'a mut Vec<LoweredSelectedComparisonOccurrence>,
    pub(crate) source_checked_obligations: &'a mut Vec<ObligationId>,
    pub(crate) claim_introductions: &'a [super::claim_bindings::ClaimIntroduction],
    pub(crate) claim_edge_bindings: &'a [super::claim_bindings::ClaimEdgeBinding],
}

pub(crate) struct Counters<'a> {
    pub(crate) next_place: &'a mut u64,
    pub(crate) next_value: &'a mut u64,
    pub(crate) next_block: &'a mut u64,
    pub(crate) next_edge: &'a mut u64,
    pub(crate) next_operation: &'a mut u64,
    pub(crate) next_obligation: &'a mut u64,
}

struct Instance {
    continuation: usize,
    incoming: IdentityMap,
    namespace: IdentityMap,
    successors: Vec<(EdgeId, usize)>,
}

/// Returns one namespace per emitted block. `blocks` contains its sole owner
/// pair and `operations`/`edges` contain only its local definitions; inherited
/// source-loan producer names live in `inherited_operations` separately.
pub(crate) fn complete(
    machine: &mut TerminalMachine,
    groups: &[SelectedCleanupGroup],
    types: &[StructuralTypeDeclaration],
    mut rows: CompanionRows<'_>,
    mut counters: Counters<'_>,
) -> Result<Vec<IdentityMap>, LoweringError> {
    let Some(topology) = super::selected_continuations::prepare(
        machine.id,
        machine.entry,
        &machine.structural_parameters,
        &machine.structural_places,
        &machine.blocks,
        types,
        groups,
        !rows.claim_edge_bindings.is_empty(),
    )?
    else {
        return Ok(Vec::new());
    };
    // A projected frontier that already has one continuation per source block
    // needs no identity work. Candidate groups can still redirect edges, so
    // their ordinary specialization remains necessary even without clones.
    if groups.is_empty() && rows.claim_edge_bindings.is_empty() {
        let mut retained = vec![false; machine.blocks.len()];
        if topology.continuations.iter().all(|continuation| {
            !std::mem::replace(&mut retained[continuation.original_block], true)
        }) {
            return Ok(Vec::new());
        }
    }
    let definitions = machine
        .blocks
        .iter()
        .map(|block| Definitions::for_block(machine, block, &rows))
        .collect::<Result<Vec<_>, _>>()?;
    let live = dependencies::live_inputs(machine, &topology, &definitions, &rows)?;
    let mut next_claim = machine
        .entry_claims
        .iter()
        .chain(&machine.block_entry_claims)
        .map(|row| row.claim.get())
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.structural())
                .flat_map(|result| result.claims.iter().map(|row| row.claim.get())),
        )
        .chain(
            machine
                .loan_completions
                .iter()
                .flat_map(|completion| completion.claims.iter().map(|row| row.claim.get())),
        )
        .chain(rows.claim_introductions.iter().map(|row| row.claim.get()))
        .chain(
            rows.claim_edge_bindings
                .iter()
                .flat_map(|row| [row.destination.get(), row.source.get()]),
        )
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "continuation claim namespace exceeds u64",
        ))?;
    let mut instantiated = vec![false; machine.blocks.len()];
    let incoming = captured_namespace(&mut IdentityMap::default(), &live[topology.entry]);
    let mut instances = vec![new_instance(
        topology.entry,
        incoming,
        machine,
        &topology,
        &definitions,
        &mut instantiated,
        &mut counters,
        &mut next_claim,
    )?];
    let mut position = 0;
    while position < instances.len() {
        let continuation = instances[position].continuation;
        let mut namespace = instances[position].namespace.clone();
        let mut successors = Vec::new();
        for (edge, target) in &topology.continuations[continuation].successors {
            // All substitutions read the predecessor namespace before any
            // destination is installed. Swaps and correlated sibling claims
            // therefore cannot observe a partially rebound frontier.
            let substitutions = rows
                .claim_edge_bindings
                .iter()
                .filter(|binding| binding.edge == *edge)
                .map(|binding| (binding.destination, namespace.claim(binding.source)))
                .collect::<Vec<_>>();
            let mut incoming_namespace = namespace.clone();
            for (destination, source) in substitutions {
                incoming_namespace
                    .claims
                    .retain(|(name, _)| *name != destination);
                incoming_namespace.claims.push((destination, source));
            }
            let incoming = captured_namespace(&mut incoming_namespace, &live[*target]);
            let target_instance = if let Some(existing) = instances.iter().position(|instance| {
                instance.continuation == *target && instance.incoming == incoming
            }) {
                existing
            } else {
                let target_instance = instances.len();
                instances.push(new_instance(
                    *target,
                    incoming,
                    machine,
                    &topology,
                    &definitions,
                    &mut instantiated,
                    &mut counters,
                    &mut next_claim,
                )?);
                target_instance
            };
            successors.push((*edge, target_instance));
        }
        instances[position].successors = successors;
        position += 1;
    }
    for position in 0..instances.len() {
        instances[position].namespace.edge_targets = instances[position]
            .successors
            .iter()
            .map(|(edge, target)| (*edge, instances[*target].namespace.blocks[0].1))
            .collect();
    }
    let mut blocks = Vec::with_capacity(instances.len());
    for instance in &mut instances {
        let original = topology.continuations[instance.continuation].original_block;
        let mut block = machine.blocks[original].clone();
        instance.namespace.block(&mut block)?;
        blocks.push(block);
    }
    companions::instantiate(machine, &topology, &mut instances, &mut rows, &mut counters)?;
    let mut places = machine
        .structural_places
        .iter()
        .filter(|place| {
            !definitions
                .iter()
                .any(|definition| definition.places.contains(&place.id))
        })
        .copied()
        .collect::<Vec<_>>();
    let mut block_entry_claims = Vec::new();
    for instance in &mut instances {
        let original = topology.continuations[instance.continuation].original_block;
        for source in &definitions[original].places {
            let mut declaration = machine
                .structural_places
                .iter()
                .find(|place| place.id == *source)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "continuation definition has no structural declaration",
                ))?;
            instance.namespace.place_declaration(&mut declaration);
            places.push(declaration);
        }
        for claim in machine.block_entry_claims.iter().filter(|claim| {
            machine.blocks[original]
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == claim.input)
        }) {
            let mut claim = claim.clone();
            claim.input = instance.namespace.place(claim.input);
            claim.claim = instance.namespace.claim(claim.claim);
            instance.namespace.path(&mut claim.path);
            block_entry_claims.push(claim);
        }
    }
    places.sort_by_key(|place| place.id);
    let mut literal_ordinal = 0;
    for place in &mut places {
        if let StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            ..
        } = &mut place.kind
        {
            *declaration_ordinal = literal_ordinal;
            literal_ordinal += 1;
        }
    }
    block_entry_claims.sort_by_key(|claim| claim.claim);
    blocks.sort_by_key(|block| block.id);
    machine.entry = instances[0].namespace.blocks[0].1;
    machine.blocks = blocks;
    machine.structural_places = places;
    machine.block_entry_claims = block_entry_claims;
    Ok(instances
        .into_iter()
        .map(|instance| instance.namespace)
        .collect())
}

fn captured_namespace(namespace: &mut IdentityMap, live: &IdentityUses) -> IdentityMap {
    IdentityMap {
        values: live
            .values
            .iter()
            .map(|original| (*original, namespace.value(*original)))
            .collect(),
        places: live
            .places
            .iter()
            .map(|original| (*original, namespace.place(*original)))
            .collect(),
        inherited_operations: live
            .operations
            .iter()
            .map(|original| (*original, namespace.operation(*original)))
            .collect(),
        claims: live
            .claims
            .iter()
            .map(|original| (*original, namespace.claim(*original)))
            .collect(),
        obligations: live
            .obligations
            .iter()
            .map(|original| (*original, namespace.obligation(*original)))
            .collect(),
        ..IdentityMap::default()
    }
}

fn new_instance(
    continuation: usize,
    incoming: IdentityMap,
    machine: &TerminalMachine,
    topology: &Specialization,
    definitions: &[Definitions],
    instantiated: &mut [bool],
    counters: &mut Counters<'_>,
    next_claim: &mut u64,
) -> Result<Instance, LoweringError> {
    let original = topology.continuations[continuation].original_block;
    let block = &machine.blocks[original];
    let duplicate = std::mem::replace(&mut instantiated[original], true);
    let mut namespace = incoming.clone();
    namespace.blocks.push((
        block.id,
        if duplicate {
            block_id(allocate_dense(counters.next_block)?)
        } else {
            block.id
        },
    ));
    let definitions = &definitions[original];
    for original in &definitions.values {
        namespace.values.push((
            *original,
            if duplicate {
                value_id(allocate_dense(counters.next_value)?)
            } else {
                *original
            },
        ));
    }
    for original in &definitions.places {
        namespace.places.push((
            *original,
            if duplicate {
                place_id(allocate_dense(counters.next_place)?)
            } else {
                *original
            },
        ));
    }
    for original in &definitions.operations {
        namespace.operations.push((
            *original,
            if duplicate {
                operation_id(allocate_dense(counters.next_operation)?)
            } else {
                *original
            },
        ));
    }
    for original in &definitions.edges {
        namespace.edges.push((
            *original,
            if duplicate {
                edge_id(allocate_dense(counters.next_edge)?)
            } else {
                *original
            },
        ));
    }
    for original in &definitions.obligations {
        namespace.obligations.push((
            *original,
            if duplicate {
                obligation_id(allocate_dense(counters.next_obligation)?)
            } else {
                *original
            },
        ));
    }
    for original in &definitions.claims {
        namespace.claims.push((
            *original,
            if duplicate {
                claim_id(allocate_dense(next_claim)?)
            } else {
                *original
            },
        ));
    }
    Ok(Instance {
        continuation,
        incoming,
        namespace,
        successors: Vec::new(),
    })
}
