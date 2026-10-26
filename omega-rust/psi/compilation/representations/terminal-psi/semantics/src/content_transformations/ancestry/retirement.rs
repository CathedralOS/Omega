//! Exact nominal disposal of a call input that does not survive in its result.
//!
//! This is a bounded disposition summary, not another ownership verifier.
//! It follows the complete tracked claim frontier through ordinary transfers
//! and content replacements, preserves CFG alternatives, and accepts only an
//! owner-designated leaf cleanup of each remaining claim. Numeric emptiness,
//! a retaining hook, an arbitrary owned call and RetireHome prove no disposal.

use super::{OriginTraversal, call_input_origins};
use semantic_vocabulary::{BlockId, ClaimId, ContentProjectionIdentity, PlaceId};
use terminal_psi::{
    ClaimTransfer, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralPathSegment, TerminalAffineCleanupAction, TerminalMachine, TerminalModule,
    Terminator,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ClaimLocation {
    identity: ClaimId,
    place: PlaceId,
    path: Vec<StructuralPathSegment>,
}

fn owned_call<'a>(
    module: &'a TerminalModule,
    operation: &'a Operation,
) -> Result<(&'a TerminalMachine, &'a [ClaimTransfer]), &'static str> {
    let callee = operation
        .kind
        .direct_callee()
        .and_then(|identity| {
            module
                .machines
                .iter()
                .find(|machine| machine.id == identity)
        })
        .ok_or("nominal retirement requires an exact in-module callable")?;
    let transfers = operation.kind.claim_transfers();
    Ok((callee, transfers))
}

fn transferred_entry_claim(
    callee: &TerminalMachine,
    transfer: &ClaimTransfer,
) -> Result<ClaimId, &'static str> {
    let parameter = callee
        .structural_parameters
        .get(transfer.argument_index as usize)
        .ok_or("nominal retirement transfer has no structural formal")?;
    if parameter.access != StructuralAccess::Owned {
        return Err("nominal retirement transfer does not enter an owned formal");
    }
    let mut matching = callee
        .entry_claims
        .iter()
        .filter(|entry| entry.input == parameter.place && entry.path == transfer.path);
    let entry = matching
        .next()
        .ok_or("nominal retirement transfer has no exact entry claim")?;
    if matching.next().is_some() {
        return Err("nominal retirement transfer has ambiguous entry claims");
    }
    Ok(entry.claim)
}

pub(super) fn call_input_is_retired(
    module: &TerminalModule,
    operation: &Operation,
    input: ClaimId,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<bool, &'static str> {
    let (callee, transfers) = owned_call(module, operation)?;
    let mut matching = transfers.iter().filter(|transfer| transfer.claim == input);
    let transfer = matching
        .next()
        .ok_or("nominal retirement input has no exact call transfer")?;
    if matching.next().is_some() {
        return Err("nominal retirement input transfers more than once");
    }
    entry_claim_is_retired(
        module,
        callee,
        transferred_entry_claim(callee, transfer)?,
        projection,
        traversal,
    )
}

fn retain_claim(
    frontier: &mut Vec<ClaimLocation>,
    claim: ClaimLocation,
) -> Result<(), &'static str> {
    if let Some(previous) = frontier
        .iter()
        .find(|previous| previous.identity == claim.identity)
    {
        if previous != &claim {
            return Err("nominal retirement claim has ambiguous current storage");
        }
    } else {
        frontier.push(claim);
    }
    frontier.sort_by_key(|claim| claim.identity);
    Ok(())
}

fn produced_claim(operation: &Operation, identity: ClaimId) -> Result<ClaimLocation, &'static str> {
    let result = operation
        .result
        .as_structural()
        .ok_or("nominal retirement result has no structural carrier")?;
    let mut matching = result.claims.iter().filter(|claim| claim.claim == identity);
    let claim = matching
        .next()
        .ok_or("nominal retirement result has no exact claim path")?;
    if matching.next().is_some() {
        return Err("nominal retirement result repeats a claim");
    }
    Ok(ClaimLocation {
        identity,
        place: result.place,
        path: claim.path.clone(),
    })
}

fn advance_owned_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    frontier: &mut Vec<ClaimLocation>,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<bool, &'static str> {
    if frontier.is_empty() {
        return Ok(true);
    }
    if let OperationKind::TransformContent { derivations, .. } = &operation.kind {
        let mut consumed = Vec::new();
        let mut outputs = Vec::new();
        for derived in derivations {
            traversal.visit()?;
            if !derived
                .inputs
                .iter()
                .any(|input| frontier.iter().any(|claim| claim.identity == input.claim))
            {
                continue;
            }
            for input in &derived.inputs {
                traversal.visit()?;
                if let Some(claim) = frontier.iter().find(|claim| claim.identity == input.claim) {
                    let root = super::super::place_type(machine, input.subject.root)
                        .ok_or("nominal retirement transformation lacks its current carrier")?;
                    let path = super::super::structural_path(
                        module.structural_types.iter(),
                        &module.identities,
                        root,
                        &input.subject.segments,
                    )
                    .ok_or("nominal retirement transformation lacks its exact typed path")?;
                    if claim.place != input.subject.root || claim.path != path {
                        return Err(
                            "nominal retirement transformation names another current subject",
                        );
                    }
                }
            }
            for input in &derived.inputs {
                traversal.visit()?;
                if !consumed.contains(&input.claim) {
                    consumed.push(input.claim);
                }
            }
            for output in &derived.outputs {
                traversal.visit()?;
                if !outputs.contains(&output.claim) {
                    outputs.push(output.claim);
                }
            }
        }
        frontier.retain(|claim| !consumed.contains(&claim.identity));
        for output in outputs {
            retain_claim(frontier, produced_claim(operation, output)?)?;
        }
    } else if operation.kind.direct_callee().is_some() {
        let (callee, transfers) = owned_call(module, operation)?;
        let moving = frontier
            .iter()
            .filter(|claim| {
                transfers
                    .iter()
                    .any(|transfer| transfer.claim == claim.identity)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !moving.is_empty() {
            let mut returned = Vec::new();
            if let Some(result) = operation.result.as_structural() {
                for output in &result.claims {
                    let inputs =
                        call_input_origins(module, operation, output.claim, projection, traversal)?;
                    returned.push((output.claim, inputs));
                }
            }
            for claim in moving {
                traversal.visit()?;
                let mut matching = transfers
                    .iter()
                    .filter(|transfer| transfer.claim == claim.identity);
                let transfer = matching
                    .next()
                    .ok_or("nominal retirement lost a selected call transfer")?;
                if matching.next().is_some() {
                    return Err("nominal retirement call duplicates an owned input");
                }
                let argument = operation
                    .kind
                    .structural_arguments()
                    .get(transfer.argument_index as usize)
                    .ok_or("nominal retirement transfer has no actual structural operand")?;
                let mut actual_path = argument.path.clone();
                actual_path.extend_from_slice(&transfer.path);
                if argument.access != StructuralAccess::Owned
                    || argument.place != claim.place
                    || actual_path != claim.path
                {
                    return Err(
                        "nominal retirement call transfers a different current claim subject",
                    );
                }
                let outputs = returned
                    .iter()
                    .filter(|(_, inputs)| inputs.contains(&claim.identity))
                    .map(|(output, _)| *output)
                    .collect::<Vec<_>>();
                if outputs.is_empty()
                    && !entry_claim_is_retired(
                        module,
                        callee,
                        transferred_entry_claim(callee, transfer)?,
                        projection,
                        traversal,
                    )?
                {
                    return Ok(false);
                }
                frontier.retain(|current| current.identity != claim.identity);
                for output in outputs {
                    retain_claim(frontier, produced_claim(operation, output)?)?;
                }
            }
        }
    } else {
        if operation.kind.claim_transfers().iter().any(|transfer| {
            frontier
                .iter()
                .any(|claim| claim.identity == transfer.claim)
        }) {
            return Err(
                "opaque or dynamic custody transfer is not an authenticated nominal retirement",
            );
        }
        if let OperationKind::StoreStructuralField { value, .. } = &operation.kind {
            if value.access == StructuralAccess::Owned
                && frontier
                    .iter()
                    .any(|claim| claim.place == value.place && claim.path.starts_with(&value.path))
            {
                return Err(
                    "borrowed-storage custody exit needs its own exact restoration evidence, not a nominal-disposal inference",
                );
            }
        }
    }
    // Moves, constructions and selected extractions retain the nominal identity
    // while relocating its exact path. New unrelated claims are never pulled in.
    if let Some(result) = operation.result.as_structural() {
        for claim in frontier.iter_mut() {
            if result
                .claims
                .iter()
                .any(|binding| binding.claim == claim.identity)
            {
                *claim = produced_claim(operation, claim.identity)?;
            }
        }
    }
    Ok(true)
}

fn apply_nominal_disposals(
    module: &TerminalModule,
    machine: &TerminalMachine,
    actions: &[TerminalAffineCleanupAction],
    frontier: &mut Vec<ClaimLocation>,
) -> Result<(), &'static str> {
    for action in actions {
        match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                let selected = frontier
                    .iter()
                    .filter(|claim| cleanup.consumed_claims.contains(&claim.identity))
                    .cloned()
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    continue;
                }
                crate::nominal_cleanup::target(
                    module.structural_types.iter(),
                    module.machines.iter(),
                    cleanup,
                )?;
                if cleanup.retains_descendants {
                    return Err(
                        "nominal retirement needs the completed descendant partition, not a retaining hook",
                    );
                }
                let declaration = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == cleanup.structural_type)
                    .ok_or("nominal retirement subject type is absent")?;
                if !declaration.owns_linear_claim
                    || cleanup.consumed_claims.len() != 1
                    || selected.len() != 1
                {
                    return Err("nominal retirement requires an exact whole leaf claim");
                }
                let claim = &selected[0];
                if claim.place != cleanup.place || claim.path != cleanup.path {
                    return Err("nominal retirement cleanup names different current storage");
                }
                let root = super::super::place_type(machine, cleanup.place)
                    .ok_or("nominal retirement current root has no structural type")?;
                let selected = crate::runtime_structural_path_tip(
                    module.structural_types.iter(),
                    root,
                    &cleanup.path,
                )
                .ok_or("nominal retirement current path has no structural type")?;
                if selected.id != cleanup.structural_type {
                    return Err(
                        "nominal retirement cleanup differs from its current typed subject",
                    );
                }
                // A leaf owns no hidden by-value child debt. The runtime and
                // independent ownership replay separately check the full action.
                let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape
                else {
                    return Err("nominal retirement subject has an unsupported descendant shape");
                };
                if fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .any(|field| {
                        matches!(
                            field.field_type,
                            terminal_psi::StructuralFieldType::Structural(_)
                                | terminal_psi::StructuralFieldType::ByteSequence(_)
                                | terminal_psi::StructuralFieldType::PredicateByteSequence { .. }
                        )
                    })
                {
                    return Err(
                        "nominal retirement subject has an unaccounted structural descendant",
                    );
                }
                frontier.retain(|current| current.identity != claim.identity);
            }
            TerminalAffineCleanupAction::DiscardRoot(place)
            | TerminalAffineCleanupAction::RetireHome(place) => {
                if frontier.iter().any(|claim| claim.place == *place) {
                    return Err(
                        "home retirement or claim-free discard cannot discharge a nominal claim",
                    );
                }
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                if frontier.iter().any(|claim| {
                    claim.place == discard.place
                        && crate::structural_paths_may_overlap(&claim.path, &discard.path)
                }) {
                    return Err("claim-free residual disposal cannot discharge a nominal claim");
                }
            }
        }
    }
    Ok(())
}

fn bind_successor_claims(
    machine: &TerminalMachine,
    target: BlockId,
    arguments: &[StructuralArgument],
    frontier: &mut [ClaimLocation],
) -> Result<(), &'static str> {
    let block = machine
        .block(&target)
        .ok_or("nominal retirement successor block is absent")?;
    for claim in frontier {
        let mut matching = arguments.iter().enumerate().filter(|(_, argument)| {
            argument.access == StructuralAccess::Owned
                && argument.place == claim.place
                && claim.path.starts_with(&argument.path)
        });
        let Some((position, argument)) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return Err("nominal retirement edge duplicates a live claim");
        }
        let parameter = block
            .structural_parameters
            .get(position)
            .ok_or("nominal retirement edge lacks its target formal")?;
        if parameter.access != StructuralAccess::Owned {
            return Err("nominal retirement edge changes owned access");
        }
        let path = claim.path[argument.path.len()..].to_vec();
        let mut bindings = machine
            .block_entry_claims
            .iter()
            .filter(|entry| entry.input == parameter.place && entry.path == path);
        if let Some(binding) = bindings.next() {
            if bindings.next().is_some() {
                return Err("nominal retirement arrival has ambiguous claim names");
            }
            claim.identity = binding.claim;
        }
        claim.place = parameter.place;
        claim.path = path;
    }
    Ok(())
}

fn entry_claim_is_retired(
    module: &TerminalModule,
    machine: &TerminalMachine,
    input: ClaimId,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<bool, &'static str> {
    traversal.visit()?;
    let key = (machine.id, input);
    if let Some((_, result)) = traversal
        .retired
        .iter()
        .find(|(candidate, _)| *candidate == key)
    {
        return Ok(*result);
    }
    if traversal.retirement_active.contains(&key) || traversal.retirement_active.len() >= 128 {
        return Err("nominal retirement call graph is circular or exceeds its receiving depth");
    }
    let declaration = machine
        .entry_claims
        .iter()
        .find(|entry| entry.claim == input)
        .ok_or("nominal retirement has no exact original entry claim")?;
    traversal.retirement_active.push(key);
    let result = (|| {
        let mut pending = vec![(
            machine.entry,
            vec![ClaimLocation {
                identity: input,
                place: declaration.input,
                path: declaration.path.clone(),
            }],
        )];
        let mut entered: Vec<(BlockId, Vec<ClaimLocation>)> = Vec::new();
        let mut normal = false;
        while let Some((block, mut frontier)) = pending.pop() {
            traversal.visit()?;
            frontier.sort_by_key(|claim| claim.identity);
            if let Some((_, previous)) = entered.iter().find(|(candidate, _)| *candidate == block) {
                if previous != &frontier {
                    return Err("nominal retirement frontier differs across incoming paths");
                }
                continue;
            }
            entered.push((block, frontier.clone()));
            let body = machine
                .block(&block)
                .ok_or("nominal retirement control block is absent")?;
            for current in &body.operations {
                traversal.visit()?;
                if !advance_owned_frontier(
                    module,
                    machine,
                    current,
                    &mut frontier,
                    projection,
                    traversal,
                )? {
                    return Ok(false);
                }
            }
            match &body.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                }
                | Terminator::ReturnUnit {
                    cleanup_actions, ..
                }
                | Terminator::ReturnStructural {
                    cleanup_actions, ..
                } => {
                    if let Terminator::ReturnStructural {
                        returned_claims, ..
                    } = &body.terminator
                    {
                        if returned_claims.iter().any(|returned| {
                            frontier
                                .iter()
                                .any(|claim| claim.identity == returned.claim)
                        }) {
                            return Ok(false);
                        }
                    }
                    apply_nominal_disposals(module, machine, cleanup_actions, &mut frontier)?;
                    normal = true;
                    if !frontier.is_empty() {
                        return Ok(false);
                    }
                }
                Terminator::Jump {
                    target,
                    structural_arguments,
                    cleanup_actions,
                    ..
                } => {
                    apply_nominal_disposals(module, machine, cleanup_actions, &mut frontier)?;
                    bind_successor_claims(machine, *target, structural_arguments, &mut frontier)?;
                    pending.push((*target, frontier));
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        let mut selected = frontier.clone();
                        apply_nominal_disposals(
                            module,
                            machine,
                            &edge.cleanup_actions,
                            &mut selected,
                        )?;
                        bind_successor_claims(
                            machine,
                            edge.target,
                            &edge.structural_arguments,
                            &mut selected,
                        )?;
                        pending.push((edge.target, selected));
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    for edge in cases {
                        let mut selected = frontier.clone();
                        apply_nominal_disposals(
                            module,
                            machine,
                            &edge.cleanup_actions,
                            &mut selected,
                        )?;
                        // Structural-case payloads are scalar-only; no owned
                        // structural parameter is introduced on these edges.
                        pending.push((edge.target, selected));
                    }
                }
                Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
            }
        }
        if !normal {
            return Err("nominal retirement has no grounded normal return");
        }
        Ok(true)
    })();
    traversal.retirement_active.pop();
    if let Ok(result) = result {
        traversal.retired.push((key, result));
    }
    result
}
