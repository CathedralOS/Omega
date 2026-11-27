
use semantic_vocabulary::{
    ClaimId, ContentProjectionIdentity, ContentTerm, MachineId, OperationId,
};
use terminal_psi::{
    ContentLineageStep, ContentTransformationDerivation, Operation, OperationKind,
    TerminalMachine, TerminalModule,
};

pub(crate) mod arrivals;
pub(crate) mod retirement;

type OriginKey = (MachineId, ClaimId, ContentProjectionIdentity);

/// `'a` borrows the bundle whose affine walks check each callee's claims;
/// `'m` is that bundle's module.
pub(crate) struct OriginTraversal<'a, 'm> {
    forwarding: bool,
    /// A bundle over the traversed module, so each callee's claim walk reuses
    /// its shared descriptor rows (`forwarding::validate_claims`).
    analyses: Option<&'a crate::module_analyses::ModuleAnalyses<'m>>,
    active: Vec<OriginKey>,
    completed: Vec<(OriginKey, Vec<ClaimId>)>,
    remaining: usize,
    retirement_active: Vec<(MachineId, ClaimId)>,
    retired: Vec<((MachineId, ClaimId), bool)>,
}

impl<'a, 'm> OriginTraversal<'a, 'm> {
    pub(super) fn new() -> Self {
        Self {
            forwarding: false,
            analyses: None,
            active: Vec::new(),
            completed: Vec::new(),
            remaining: 65_536,
            retirement_active: Vec::new(),
            retired: Vec::new(),
        }
    }

    pub(super) fn unchanged_content() -> Self {
        Self {
            forwarding: true,
            ..Self::new()
        }
    }

    /// Check callee claims with `analyses`' walks; `None` walks each alone.
    pub(super) fn with_analyses(
        self,
        analyses: Option<&'a crate::module_analyses::ModuleAnalyses<'m>>,
    ) -> Self {
        Self { analyses, ..self }
    }

    pub(super) fn analyses(&self) -> Option<&'a crate::module_analyses::ModuleAnalyses<'m>> {
        self.analyses
    }

    fn visit(&mut self) -> Result<(), &'static str> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("content ancestry receiving work budget exhausted")?;
        Ok(())
    }
}

pub(crate) fn validate_frontier(
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

pub(crate) fn validate_producers(
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
            let producer = position(step.producer_operation)
                .ok_or("content ancestry producer is absent")?;
            if (producer.0 == consumer.0 && producer.1 >= consumer.1)
                || !dominators.dominates(producer.0, consumer.0)
            {
                return Err(
                    "content ancestry producer does not precede the consuming completion",
                );
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
            && retirement::call_input_is_retired(
                module, producer, *input, projection, traversal,
            )?
        {
            inputs.push(*input);
        }
    }
    inputs.sort();
    if inputs != step.inputs {
        return Err(
            "content ancestry call inputs differ from its exact returned claim origins",
        );
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
    let result = operation
        .result
        .as_structural()
        .ok_or("content ancestry call has no structural result")?;
    let output = result
        .claims
        .iter()
        .find(|binding| binding.claim == output)
        .ok_or("content ancestry output is absent from its call result")?;
    if traversal.forwarding {
        let input =
            super::forwarding::call_input(module, callee, &output.path, projection, traversal)?;
        return Ok(vec![transferred_input(callee, operation, input)?]);
    }
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
            let transfer = transferred_input(callee, operation, root)?;
            if !candidate.contains(&transfer) {
                candidate.push(transfer);
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

/// A boundary call's result claim descends from the claim it consumed only
/// where the boundary declares that this projection is unchanged: an
/// equation `P(old(@input)) == P(@result)` instantiated at this call
/// (`boundary_calls::compositions`). The equation is the boundary's
/// published contract; its provider is checked against it separately. A
/// call with no such equation for this projection and result claim relates
/// nothing, so the caller cannot restate the content as conserved.
fn boundary_input_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    output: ClaimId,
    projection: ContentProjectionIdentity,
) -> Result<Vec<ClaimId>, &'static str> {
    let result = operation
        .result
        .as_structural()
        .ok_or("content ancestry call has no structural result")?;
    let binding = result
        .claims
        .iter()
        .find(|binding| binding.claim == output)
        .ok_or("content ancestry output is absent from its call result")?;
    let mut origin = None;
    for row in super::boundary_calls::compositions(module, machine, operation)? {
        let (
            ContentTerm::Projection {
                projection: left,
                subject: left_subject,
            },
            ContentTerm::Projection {
                projection: right,
                subject: right_subject,
            },
        ) = (row.derived.left(), row.derived.right())
        else {
            continue;
        };
        if *left != projection || *right != projection {
            continue;
        }
        let returned = match (
            left_subject.root == result.place,
            right_subject.root == result.place,
        ) {
            (true, false) => left_subject,
            (false, true) => right_subject,
            _ => continue,
        };
        let path = super::structural_path(
            module.types.structural_types.iter(),
            &module.identities,
            result.structural_type,
            &returned.segments,
        )
        .ok_or("content ancestry boundary output path is not a declared projection")?;
        if path != binding.path {
            continue;
        }
        let [input] = row.input_claims.as_slice() else {
            return Err("content ancestry boundary equation consumes more than one claim");
        };
        if origin
            .replace(*input)
            .is_some_and(|previous| previous != *input)
        {
            return Err("content ancestry boundary call has conflicting input claims");
        }
    }
    origin
        .map(|input| vec![input])
        .ok_or("content ancestry boundary call declares no unchanged projection for its result")
}

fn transferred_input(
    callee: &TerminalMachine,
    operation: &Operation,
    root: ClaimId,
) -> Result<ClaimId, &'static str> {
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
    let mut matching = operation.kind.claim_transfers().iter().filter(|transfer| {
        transfer.argument_index as usize == parameter && transfer.path == entry.path
    });
    let transfer = matching
        .next()
        .ok_or("content ancestry entry has no exact actual claim transfer")?;
    if matching.next().is_some() {
        return Err("content ancestry entry has ambiguous actual claim transfers");
    }
    Ok(transfer.claim)
}

pub(super) fn local_roots(
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
        let mut roots = arrivals::roots(module, machine, claim, projection, traversal)?;
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
                && operation.result.as_structural().is_some_and(|result| {
                    result.claims.iter().any(|output| output.claim == claim)
                })
            {
                candidates.push(call_input_origins(
                    module, operation, claim, projection, traversal,
                )?);
            } else if matches!(operation.kind, OperationKind::BoundaryCall { .. })
                && operation
                    .result
                    .as_structural()
                    .is_some_and(|result| result.claims.iter().any(|output| output.claim == claim))
            {
                candidates.push(boundary_input_origins(
                    module, machine, operation, claim, projection,
                )?);
            }
            for parents in candidates {
                let mut candidate = Vec::new();
                for parent in parents {
                    for ancestor in local_roots(module, machine, parent, projection, traversal)?
                    {
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
