//! Authenticate lineage against executable ownership, not a claimed root ID.
//!
//! The ordered witness retains the exact producer operations across rewriting.
//! Internal calls bind returned paths to callee-local claims and follow their
//! derivations back to exact entry claims, then substitute the actual owned
//! transfers. Numeric content equality is not used for this correspondence.

use semantic_vocabulary::{ClaimId, ContentProjectionIdentity, MachineId, OperationId};
use terminal_psi::{
    ContentLineageStep, ContentTransformationDerivation, Operation, OperationKind, TerminalMachine,
    TerminalModule,
};

mod retirement;

type OriginKey = (MachineId, ClaimId, ContentProjectionIdentity);

/// Invocation-local receiving work, shared across all ancestry steps. Completed
/// origins are reused at reconvergent edges; active origins detect cycles. The
/// finite work and stack ceilings are verifier resource limits, not evidence
/// that a larger graph is invalid or that an unchecked prefix is sufficient.
struct OriginTraversal {
    active: Vec<OriginKey>,
    completed: Vec<(OriginKey, Vec<ClaimId>)>,
    remaining: usize,
    retirement_active: Vec<(MachineId, ClaimId)>,
    retired: Vec<((MachineId, ClaimId), bool)>,
}

impl OriginTraversal {
    fn new() -> Self {
        Self {
            active: Vec::new(),
            completed: Vec::new(),
            remaining: 65_536,
            retirement_active: Vec::new(),
            retired: Vec::new(),
        }
    }

    fn visit(&mut self) -> Result<(), &'static str> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("content ancestry receiving work budget exhausted")?;
        Ok(())
    }
}

pub(super) fn validate_frontier(
    derived: &ContentTransformationDerivation,
) -> Result<(), &'static str> {
    if derived.lineage_roots.is_empty()
        || derived
            .lineage_roots
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err("content ancestry root roster is empty or not strictly canonical");
    }
    let mut frontier = derived.lineage_roots.clone();
    let mut producers = Vec::new();
    for step in &derived.ancestry {
        if step.inputs.is_empty()
            || step.outputs.is_empty()
            || step.inputs.windows(2).any(|pair| pair[0] >= pair[1])
            || step.outputs.windows(2).any(|pair| pair[0] >= pair[1])
            || producers.contains(&step.producer_operation)
            || !step.inputs.iter().all(|input| frontier.contains(input))
        {
            return Err("content ancestry does not replace a complete known parent roster");
        }
        producers.push(step.producer_operation);
        frontier.retain(|claim| !step.inputs.contains(claim));
        if step.outputs.iter().any(|output| frontier.contains(output)) {
            return Err("content ancestry duplicates a live descendant");
        }
        frontier.extend_from_slice(&step.outputs);
    }
    frontier.sort();
    let mut inputs = derived
        .inputs
        .iter()
        .map(|input| input.claim)
        .collect::<Vec<_>>();
    inputs.sort();
    if frontier != inputs {
        return Err("content ancestry does not end at the consumed claim frontier");
    }
    Ok(())
}

pub(super) fn validate_producers(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    derivations: &[ContentTransformationDerivation],
) -> Result<(), &'static str> {
    let position = |identity: OperationId| {
        machine.blocks.iter().find_map(|block| {
            block
                .operations
                .iter()
                .position(|operation| operation.id == identity)
                .map(|ordinal| (block.id, ordinal))
        })
    };
    let consumer = position(operation.id).ok_or("content consumer position is absent")?;
    let dominators = crate::control_graph::dominators(machine);
    let mut traversal = OriginTraversal::new();
    for derived in derivations {
        let projection = derived
            .inputs
            .first()
            .ok_or("content ancestry has no input")?
            .projection;
        for step in &derived.ancestry {
            let producer =
                position(step.producer_operation).ok_or("content ancestry producer is absent")?;
            if (producer.0 == consumer.0 && producer.1 >= consumer.1)
                || !dominators.dominates(producer.0, consumer.0)
            {
                return Err("content ancestry producer does not precede the consuming completion");
            }
            let producer = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == step.producer_operation)
                .ok_or("content ancestry producer is absent")?;
            authenticate_step(module, producer, projection, step, &mut traversal)?;
        }
    }
    Ok(())
}

fn authenticate_step(
    module: &TerminalModule,
    producer: &Operation,
    projection: ContentProjectionIdentity,
    step: &ContentLineageStep,
    traversal: &mut OriginTraversal,
) -> Result<(), &'static str> {
    if let OperationKind::TransformContent { derivations, .. } = &producer.kind {
        if derivations.iter().any(|derived| {
            let mut inputs = derived
                .inputs
                .iter()
                .filter(|input| input.projection == projection)
                .map(|input| input.claim)
                .collect::<Vec<_>>();
            let mut outputs = derived
                .outputs
                .iter()
                .filter(|output| output.projection == projection)
                .map(|output| output.claim)
                .collect::<Vec<_>>();
            inputs.sort();
            outputs.sort();
            inputs == step.inputs && outputs == step.outputs
        }) {
            return Ok(());
        }
        return Err("content ancestry differs from its completed transformation");
    }
    let mut inputs = Vec::new();
    for output in &step.outputs {
        for input in call_input_origins(module, producer, *output, projection, traversal)? {
            if !inputs.contains(&input) {
                inputs.push(input);
            }
        }
    }
    for input in &step.inputs {
        if !inputs.contains(input)
            && retirement::call_input_is_retired(module, producer, *input, projection, traversal)?
        {
            inputs.push(*input);
        }
    }
    inputs.sort();
    if inputs != step.inputs {
        return Err("content ancestry call inputs differ from its exact returned claim origins");
    }
    Ok(())
}

fn call_input_origins(
    module: &TerminalModule,
    operation: &Operation,
    output: ClaimId,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<Vec<ClaimId>, &'static str> {
    let callee = operation
        .kind
        .direct_callee()
        .and_then(|identity| {
            module
                .machines
                .iter()
                .find(|machine| machine.id == identity)
        })
        .ok_or("content ancestry requires an authenticated returning callable")?;
    let transfers = operation.kind.claim_transfers();
    let result = operation
        .result
        .as_structural()
        .ok_or("content ancestry call has no structural result")?;
    let output = result
        .claims
        .iter()
        .find(|binding| binding.claim == output)
        .ok_or("content ancestry output is absent from its call result")?;
    let mut origin = None;
    for block in &callee.blocks {
        traversal.visit()?;
        let terminal_psi::Terminator::ReturnStructural {
            returned_claims, ..
        } = &block.terminator
        else {
            continue;
        };
        let returned = returned_claims
            .iter()
            .find(|binding| binding.path == output.path)
            .ok_or("content ancestry result lacks an exact returned-claim correspondence")?;
        let roots = local_roots(module, callee, returned.claim, projection, traversal)?;
        let mut candidate = Vec::new();
        for root in roots {
            let entry = callee
                .entry_claims
                .iter()
                .find(|entry| entry.claim == root)
                .ok_or("content ancestry callee root is not an owned input")?;
            let parameter = callee
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == entry.input)
                .ok_or("content ancestry entry has no structural parameter")?;
            let mut matching = transfers.iter().filter(|transfer| {
                transfer.argument_index as usize == parameter && transfer.path == entry.path
            });
            let transfer = matching
                .next()
                .ok_or("content ancestry entry has no exact actual claim transfer")?;
            if matching.next().is_some() {
                return Err("content ancestry entry has ambiguous actual claim transfers");
            }
            if !candidate.contains(&transfer.claim) {
                candidate.push(transfer.claim);
            }
        }
        candidate.sort();
        if candidate.is_empty()
            || origin
                .as_ref()
                .is_some_and(|previous| previous != &candidate)
        {
            return Err("content ancestry has ambiguous normal-return input rosters");
        }
        origin = Some(candidate);
    }
    origin.ok_or("content ancestry callee has no grounded normal result")
}

fn local_roots(
    module: &TerminalModule,
    machine: &TerminalMachine,
    claim: ClaimId,
    projection: ContentProjectionIdentity,
    traversal: &mut OriginTraversal,
) -> Result<Vec<ClaimId>, &'static str> {
    traversal.visit()?;
    let key = (machine.id, claim, projection);
    if let Some((_, roots)) = traversal
        .completed
        .iter()
        .find(|(completed, _)| *completed == key)
    {
        return Ok(roots.clone());
    }
    if machine.content_entry_claims.iter().any(|entry| {
        entry.claim == claim
            && entry
                .projections
                .iter()
                .any(|row| row.projection == projection)
    }) {
        return Ok(vec![claim]);
    }
    if traversal.active.contains(&key) {
        return Err("content ancestry is circular rather than grounded in an input");
    }
    if traversal.active.len() >= 128 {
        return Err("content ancestry receiving depth budget exhausted");
    }
    traversal.active.push(key);
    let result = (|| {
        let mut roots: Option<Vec<ClaimId>> = None;
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            traversal.visit()?;
            let mut candidates = Vec::new();
            if let OperationKind::TransformContent { derivations, .. } = &operation.kind {
                for derived in derivations.iter().filter(|derived| {
                    derived
                        .outputs
                        .iter()
                        .any(|output| output.claim == claim && output.projection == projection)
                }) {
                    if derived.inputs.is_empty() {
                        return Err("content ancestry transformation has no parent");
                    }
                    let mut parents = Vec::new();
                    for input in &derived.inputs {
                        if input.projection != projection {
                            return Err("content ancestry changes qualification owner");
                        }
                        parents.push(input.claim);
                    }
                    candidates.push(parents);
                }
            } else if operation.kind.direct_callee().is_some()
                && operation
                    .result
                    .as_structural()
                    .is_some_and(|result| result.claims.iter().any(|output| output.claim == claim))
            {
                candidates.push(call_input_origins(
                    module, operation, claim, projection, traversal,
                )?);
            }
            for parents in candidates {
                let mut candidate = Vec::new();
                for parent in parents {
                    for ancestor in local_roots(module, machine, parent, projection, traversal)? {
                        if !candidate.contains(&ancestor) {
                            candidate.push(ancestor);
                        }
                    }
                }
                candidate.sort();
                if candidate.is_empty()
                    || roots
                        .as_ref()
                        .is_some_and(|previous| previous != &candidate)
                {
                    return Err("content ancestry has conflicting producer input rosters");
                }
                roots = Some(candidate);
            }
        }
        roots.ok_or("content ancestry cannot relate the returned qualification to an input")
    })();
    traversal.active.pop();
    if let Ok(roots) = &result {
        traversal.completed.push((key, roots.clone()));
    }
    result
}
