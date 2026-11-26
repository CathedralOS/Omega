use super::{DescriptorFlowStep, DescriptorLocation, DescriptorPosition};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct DescriptorResidenceRules {
    pub captures: BTreeSet<(DescriptorPosition, DescriptorLocation, DescriptorLocation)>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ResidenceName {
    Storage(DescriptorLocation),
    AddressContents(DescriptorLocation),
}

impl ResidenceName {
    fn location(&self) -> &DescriptorLocation {
        match self {
            Self::Storage(location) | Self::AddressContents(location) => location,
        }
    }
}

pub fn descriptor_residence_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    rules: &DescriptorResidenceRules,
    birth: DescriptorPosition,
    residence: &DescriptorLocation,
    at_entry: bool,
    observes: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> BTreeSet<DescriptorPosition> {
    type Occurrence = (DescriptorPosition, ResidenceName);
    let mut pending = Vec::new();
    if at_entry {
        pending.push((birth, ResidenceName::Storage(residence.clone())));
    } else if let Some(step) = steps.get(&birth) {
        for edge in &step.successors {
            pending.push((edge.target, ResidenceName::Storage(residence.clone())));
        }
    }
    let mut visited = BTreeSet::new();
    let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
    let mut observations = Vec::new();
    let mut escaped = Vec::new();
    while let Some(occurrence) = pending.pop() {
        if !visited.insert(occurrence.clone()) {
            continue;
        }
        let (position, name) = &occurrence;
        let Some(step) = steps.get(position) else {
            continue;
        };
        let location = name.location();
        if step
            .observed_inputs()
            .any(|input| observes(*position, input) && input.overlaps(location))
        {
            observations.push(occurrence.clone());
            if step.may_escape {
                escaped.push(*position);
            }
        }
        let mut outputs = Vec::new();
        match name {
            ResidenceName::Storage(_) => {
                if at_entry || *position != birth || location != residence {
                    outputs.push(name.clone());
                }
                for (source, destination) in &step.derivations {
                    if source.overlaps(location)
                        && rules.captures.contains(&(
                            *position,
                            source.clone(),
                            destination.clone(),
                        ))
                    {
                        outputs.push(ResidenceName::AddressContents(destination.clone()));
                    }
                }
            }
            ResidenceName::AddressContents(_) => {
                if !step
                    .definitions
                    .iter()
                    .any(|definition| definition.contains(location))
                {
                    outputs.push(name.clone());
                }
                for (source, destination) in &step.derivations {
                    if !rules.captures.contains(&(
                        *position,
                        source.clone(),
                        destination.clone(),
                    )) && let Some(target) = location.transported(source, destination)
                    {
                        outputs.push(ResidenceName::AddressContents(target));
                    }
                }
            }
        }
        for edge in &step.successors {
            for output in &outputs {
                let location = output.location();
                let mut destinations = Vec::new();
                if !edge
                    .parameters
                    .iter()
                    .chain(&edge.definitions)
                    .any(|parameter| parameter.contains(location))
                {
                    destinations.push(output.clone());
                }
                for binding in &edge.bindings {
                    if binding.indexed_borrow_source().is_some_and(|source| source.overlaps(location)) {
                        destinations.push(ResidenceName::Storage(binding.parameter.clone()));
                    }
                    let Some((source, destination)) = binding.locations() else {
                        continue;
                    };
                    match output {
                        ResidenceName::Storage(_)
                            if binding.access != terminal_psi::StructuralAccess::Owned
                                && source.overlaps(location) =>
                        {
                            destinations.push(ResidenceName::Storage(destination.clone()));
                        }
                        ResidenceName::AddressContents(_) => {
                            if let Some(target) = location.transported(source, destination)
                            {
                                destinations.push(ResidenceName::AddressContents(target));
                            }
                        }
                        _ => {}
                    }
                }
                for destination in destinations {
                    let next = (edge.target, destination);
                    predecessors
                        .entry(next.clone())
                        .or_default()
                        .insert(occurrence.clone());
                    pending.push(next);
                }
            }
        }
    }
    let mut observed = BTreeSet::new();
    let mut live = BTreeSet::new();
    while let Some(occurrence) = observations.pop() {
        if observed.insert(occurrence.clone()) {
            live.insert(occurrence.0);
            observations
                .extend(predecessors.get(&occurrence).into_iter().flatten().cloned());
        }
    }
    let mut escaped_positions = BTreeSet::new();
    while let Some(position) = escaped.pop() {
        if escaped_positions.insert(position) {
            live.insert(position);
            if let Some(step) = steps.get(&position) {
                escaped.extend(step.successors.iter().map(|edge| edge.target));
            }
        }
    }
    live
}
