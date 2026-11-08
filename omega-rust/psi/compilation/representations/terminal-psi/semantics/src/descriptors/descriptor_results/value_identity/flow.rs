//! Adapt one observation once; all seeds read the same immutable flow.
use super::*;

pub(super) fn adapt(
    module: &TerminalModule,
    machine: &DescriptorMachine,
    operations: &BTreeMap<DescriptorPosition, &Operation>,
    analysis: &mut DescriptorAnalysis<'_>,
    relevant: &payload_reachability::Reachability,
    observation: DescriptorPosition,
) -> Result<BTreeMap<DescriptorPosition, DescriptorFlowStep>, &'static str> {
    let identity = machine.id;
    let mut flow = BTreeMap::new();
    for (&position, original) in &machine.flow {
        if !relevant.contains(position) {
            continue;
        }
        // Observation is a pre-operation cut. Its writes and successors cannot
        // establish or invalidate the value being consumed there.
        if position == observation {
            flow.insert(position, DescriptorFlowStep::default());
            continue;
        }
        if machine.unresolved_contents_writes.contains(&position) {
            return Err("value identity crosses an unresolved write");
        }
        let mut step = DescriptorFlowStep::default();
        for definition in &original.definitions {
            step.definitions.push(
                analysis
                    .location(identity, position, definition)
                    .ok_or("value identity write has unresolved backing")?,
            );
        }
        for query in machine
            .payload_invalidations
            .get(&position)
            .into_iter()
            .flatten()
        {
            let query = query
                .as_ref()
                .ok_or("value identity crosses an unknown payload write")?;
            let targets = analysis
                .payload_targets(identity, position, query)
                .ok_or("value identity payload write has unresolved backing")?;
            if targets.is_empty() {
                return Err("value identity payload write has no complete targets");
            }
            for target in targets {
                let DescriptorPayloadTarget::Storage(target) = target else {
                    return Err("value identity payload write may reach unknown incoming storage");
                };
                step.definitions.push(target);
            }
        }
        if operations
            .get(&position)
            .is_some_and(|operation| identity_operation(&operation.kind))
        {
            for (source, destination) in &original.derivations {
                let source = analysis
                    .location(identity, position, source)
                    .ok_or("value identity source has unresolved backing")?;
                let destination = analysis
                    .location(identity, position, destination)
                    .ok_or("value identity destination has unresolved backing")?;
                if location_type(module, machine, &source)?
                    != location_type(module, machine, &destination)?
                {
                    return Err("value identity transfer changes the retained type");
                }
                step.derivations.push((source.clone(), destination.clone()));
                step.authority_derivations.push((source, destination));
            }
        }
        // Inputs and escapes are only used by the existing renewal guard.
        step.inputs = original.inputs.clone();
        step.may_escape = original.may_escape;
        for (ordinal, edge) in original.successors.iter().enumerate() {
            if !relevant.includes_edge(position, ordinal) {
                continue;
            }
            let mut edge = edge.clone();
            for binding in &mut edge.bindings {
                let source = match &binding.source {
                    DescriptorFlowSource::Location(source) => {
                        analysis.location(identity, position, source)
                    }
                    DescriptorFlowSource::Contents(query) => {
                        let targets = analysis
                            .arrival_payload_targets(identity, position, query)
                            .ok_or("value identity arrival has unavailable contents")?;
                        match targets.as_slice() {
                            [DescriptorPayloadTarget::Storage(location)] => Some(location.clone()),
                            _ => None,
                        }
                    }
                    DescriptorFlowSource::Unavailable(_) => None,
                }
                .ok_or("value identity arrival does not name one exact value")?;
                if location_type(module, machine, &source)?
                    != location_type(module, machine, &binding.parameter)?
                {
                    return Err("value identity arrival changes the retained type");
                }
                binding.source = DescriptorFlowSource::Location(source);
            }
            step.successors.push(edge);
        }
        flow.insert(position, step);
    }
    require_acyclic(&flow, machine.entry)?;
    Ok(flow)
}
pub(super) fn location_type(
    module: &TerminalModule,
    machine: &DescriptorMachine,
    location: &DescriptorLocation,
) -> Result<StructuralTypeId, &'static str> {
    descriptor_location_type(
        &module.structural_types,
        *machine
            .place_types
            .get(&location.root)
            .ok_or("value identity location has no retained type")?,
        &location.path,
    )
    .ok_or("value identity location has an unresolved retained type")
}

pub(super) fn identity_operation(operation: &OperationKind) -> bool {
    matches!(
        operation,
        OperationKind::MoveStructuralValue { .. }
            | OperationKind::MoveStructuralField { .. }
            | OperationKind::StructuralLeafCopy { .. }
            | OperationKind::StructuralCaseLeafCopy { .. }
            | OperationKind::StoreStructuralField { .. }
    )
}

pub(super) fn require_acyclic(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
) -> Result<(), &'static str> {
    let mut incoming = flow
        .keys()
        .map(|position| (*position, 0usize))
        .collect::<BTreeMap<_, _>>();
    for step in flow.values() {
        for edge in &step.successors {
            *incoming
                .get_mut(&edge.target)
                .ok_or("value identity edge has no target")? += 1;
        }
    }
    if incoming.get(&entry) != Some(&0) {
        return Err("value identity entry is cyclic or absent");
    }
    let mut pending = vec![entry];
    let mut count = 0;
    while let Some(position) = pending.pop() {
        count += 1;
        for edge in &flow[&position].successors {
            let remaining = incoming
                .get_mut(&edge.target)
                .ok_or("value identity edge has no target")?;
            *remaining -= 1;
            if *remaining == 0 {
                pending.push(edge.target);
            }
        }
    }
    if count != flow.len() {
        return Err("value identity crosses cyclic or unavailable incoming flow");
    }
    Ok(())
}
