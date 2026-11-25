use super::*;
use crate::descriptors::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};
use terminal_psi::StructuralFieldType;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Copies {
    classes: Vec<BTreeSet<DescriptorLocation>>,
    member_classes: BTreeMap<DescriptorLocation, usize>,
}

impl Copies {
    fn normalized(mut classes: Vec<BTreeSet<DescriptorLocation>>) -> Self {
        classes.retain(|members| members.len() > 1);
        classes.sort();
        let mut member_classes = BTreeMap::new();
        for (index, members) in classes.iter().enumerate() {
            for member in members {
                member_classes.insert(member.clone(), index);
            }
        }
        Self {
            classes,
            member_classes,
        }
    }

    pub(crate) fn equivalents(
        &self,
        subject: &DescriptorLocation,
        limit: usize,
    ) -> Option<BTreeSet<DescriptorLocation>> {
        let mut found = BTreeSet::from([subject.clone()]);
        let mut pending = vec![subject.clone()];
        let mut applied = BTreeSet::new();
        while let Some(location) = pending.pop() {
            for length in 0..=location.path.len() {
                let prefix = DescriptorLocation {
                    root: location.root,
                    path: location.path[..length].to_vec(),
                };
                let Some(index) = self.member_classes.get(&prefix) else {
                    continue;
                };
                let suffix = &location.path[length..];
                if suffix.contains(&DescriptorPathSegment::Referent)
                    || !applied.insert((*index, suffix.to_vec()))
                {
                    continue;
                }
                if applied.len() > limit {
                    return None;
                }
                for member in &self.classes[*index] {
                    let mut target = member.clone();
                    target.path.extend_from_slice(suffix);
                    if found.insert(target.clone()) {
                        if found.len() > limit {
                            return None;
                        }
                        pending.push(target);
                    }
                }
            }
        }
        Some(found)
    }

    pub(crate) fn insert_class(
        &mut self,
        mut members: BTreeSet<DescriptorLocation>,
        limit: usize,
    ) -> Option<()> {
        if members.len() < 2 {
            return Some(());
        }
        let affected = members
            .iter()
            .filter_map(|member| self.member_classes.get(member).copied())
            .collect::<BTreeSet<_>>();
        let mut classes = Vec::with_capacity(self.classes.len() + 1);
        for (index, existing) in std::mem::take(&mut self.classes).into_iter().enumerate() {
            if affected.contains(&index) {
                members.extend(existing);
            } else {
                classes.push(existing);
            }
        }
        classes.push(members);
        *self = Self::normalized(classes);
        (self.member_classes.len() <= limit).then_some(())
    }

    pub(crate) fn retain(&mut self, keep: impl Fn(&DescriptorLocation) -> bool) {
        let classes = std::mem::take(&mut self.classes)
            .into_iter()
            .map(|mut members| {
                members.retain(&keep);
                members
            })
            .collect();
        *self = Self::normalized(classes);
    }

    pub(crate) fn meet(&self, other: &Self) -> Self {
        let mut groups = BTreeMap::<(usize, usize), BTreeSet<DescriptorLocation>>::new();
        for (left, members) in self.classes.iter().enumerate() {
            for member in members {
                if let Some(right) = other.member_classes.get(member) {
                    groups
                        .entry((left, *right))
                        .or_default()
                        .insert(member.clone());
                }
            }
        }
        Self::normalized(groups.into_values().collect())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Transfer<'read> {
    Operation {
        position: DescriptorPosition,
        reads: &'read DescriptorReadDepth<'read>,
    },
    Arrival {
        edge: &'read DescriptorFlowEdge,
        reads: &'read DescriptorReadDepth<'read>,
    },
}

impl<'read> Transfer<'read> {
    pub(crate) fn reads(self) -> &'read DescriptorReadDepth<'read> {
        match self {
            Self::Operation { reads, .. } | Self::Arrival { reads, .. } => reads,
        }
    }

    pub(crate) fn position(self) -> Option<DescriptorPosition> {
        match self {
            Self::Operation { position, .. } => Some(position),
            Self::Arrival { .. } => None,
        }
    }

    pub(crate) fn transports_value(
        self,
        machine: &DescriptorMachine,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> bool {
        match self {
            Self::Operation { position, .. } => {
                let key = (position, source.clone(), destination.clone());
                !machine.calls.contains_key(&position)
                    && !machine.storage_derivations.contains(&key)
                    && !machine.projections.contains_key(&key)
                    && !machine.contents_queries.contains_key(&key)
            }
            Self::Arrival { edge, .. } => edge
                .bindings
                .iter()
                .any(|binding| binding.locations() == Some((source, destination))),
        }
    }

    pub(crate) fn copies_value(
        self,
        machine: &DescriptorMachine,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> bool {
        self.transports_value(machine, source, destination)
            && match self {
                Self::Operation { .. } => true,
                Self::Arrival { edge, .. } => edge.bindings.iter().any(|binding| {
                    binding.access == StructuralAccess::Owned
                        && binding.locations() == Some((source, destination))
                }),
            }
    }
}

pub(crate) struct Storage {
    roots: BTreeSet<PlaceId>,
    case_types: BTreeSet<StructuralTypeId>,
    pub(crate) limit: usize,
}

impl Storage {
    pub(crate) fn new(replay: &Replay<'_>, machine: &DescriptorMachine) -> Self {
        let mut roots = BTreeSet::new();
        let mut ordinary_types = BTreeMap::new();
        let mut case_types = BTreeMap::new();
        for row in &machine.places {
            let Some(structural_type) = machine.place_types.get(&row.place) else {
                continue;
            };
            if !*ordinary_types.entry(*structural_type).or_insert_with(|| {
                ordinary_type(replay, *structural_type, &mut BTreeSet::new())
            }) {
                continue;
            }
            contains_case(
                replay.types,
                *structural_type,
                &mut case_types,
                &mut BTreeSet::new(),
            );
            let owned = match row.definition {
                DescriptorDefinition::Parameter { access, .. }
                | DescriptorDefinition::Arrival { access } => {
                    access == StructuralAccess::Owned
                }
                DescriptorDefinition::Local => {
                    machine
                        .payload_producers
                        .values()
                        .any(|root| *root == row.place)
                        || owned_arrival(machine, row.place)
                }
                DescriptorDefinition::Literal => true,
            };
            if owned {
                roots.insert(row.place);
            }
        }
        Self {
            roots,
            case_types: case_types
                .into_iter()
                .filter_map(|(identity, tagged)| tagged.then_some(identity))
                .collect(),
            limit: machine.flow.len().saturating_mul(4096).max(4096),
        }
    }

    pub(crate) fn contains(&self, location: &DescriptorLocation) -> bool {
        self.roots.contains(&location.root)
            && !location.path.contains(&DescriptorPathSegment::Referent)
    }

    pub(crate) fn copy_type(
        &self,
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> bool {
        if !self.contains(source) || !self.contains(destination) {
            return false;
        }
        let Some(source_type) = location_type(replay, machine, source) else {
            return false;
        };
        self.case_types.contains(&source_type)
            && location_type(replay, machine, destination) == Some(source_type)
            && replay
                .type_index
                .find(source_type)
                .is_some_and(|row| row.is_copyable())
    }
}

fn location_type(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    location: &DescriptorLocation,
) -> Option<StructuralTypeId> {
    machine.location_type_indexed(&replay.type_index, location, None)
}

pub(crate) fn transported_subject(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    subject: &DescriptorLocation,
    source: &DescriptorLocation,
    destination: &DescriptorLocation,
) -> Option<DescriptorLocation> {
    let target = subject.transported(source, destination)?;
    if subject.path[source.path.len()..].contains(&DescriptorPathSegment::Referent) {
        return None;
    }
    let source_type = location_type(replay, machine, subject)?;
    (location_type(replay, machine, &target) == Some(source_type)).then_some(target)
}

fn owned_arrival(machine: &DescriptorMachine, root: PlaceId) -> bool {
    let parameter = DescriptorLocation::whole(root);
    let mut found = false;
    for edge in machine.flow.values().flat_map(|step| &step.successors) {
        if !edge.parameters.contains(&parameter) {
            continue;
        }
        found = true;
        let mut bindings = edge
            .bindings
            .iter()
            .filter(|binding| binding.parameter == parameter);
        let Some(binding) = bindings.next() else {
            return false;
        };
        if bindings.next().is_some() || binding.access != StructuralAccess::Owned {
            return false;
        }
    }
    found
}

pub(crate) fn contains_case(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    complete: &mut BTreeMap<StructuralTypeId, bool>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if let Some(result) = complete.get(&structural_type) {
        return *result;
    }
    if !active.insert(structural_type) {
        return false;
    }
    let Some(declaration) = find_structural_type(types, structural_type) else {
        return false;
    };
    let mut child =
        |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
            StructuralFieldType::Structural(child) => {
                contains_case(types, child, complete, active)
            }
            _ => false,
        };
    let result = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields
            .iter()
            .fold(false, |found, field| child(field) | found),
        StructuralTypeShape::Mixed { fields, cases } => fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
            .fold(!cases.is_empty(), |found, field| child(field) | found),
        StructuralTypeShape::Sum { cases } => cases
            .iter()
            .flat_map(|case| &case.fields)
            .fold(!cases.is_empty(), |found, field| child(field) | found),
        StructuralTypeShape::FixedArray { element, .. } => {
            contains_case(types, *element, complete, active)
        }
        _ => false,
    };
    active.remove(&structural_type);
    complete.insert(structural_type, result);
    result
}

fn ordinary_type(
    replay: &Replay<'_>,
    structural_type: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !plain_owned_shape(replay.types.iter(), structural_type)
        || !active.insert(structural_type)
    {
        return false;
    }
    let Some(declaration) = replay.type_index.find(structural_type) else {
        return false;
    };
    let mut child =
        |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
            StructuralFieldType::Structural(child) => ordinary_type(replay, child, active),
            _ => true,
        };
    let ordinary = match &declaration.shape {
        StructuralTypeShape::Record {
            storage, fields, ..
        } => {
            !matches!(
                storage,
                terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_)
            ) && fields.iter().all(&mut child)
        }
        StructuralTypeShape::Mixed { fields, cases } => fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
            .all(&mut child),
        StructuralTypeShape::Sum { cases } => {
            cases.iter().flat_map(|case| &case.fields).all(&mut child)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            ordinary_type(replay, *element, active)
        }
        _ => true,
    };
    active.remove(&structural_type);
    ordinary
}

pub(crate) fn overlaps(left: &DescriptorLocation, right: &DescriptorLocation) -> bool {
    if left.root != right.root {
        return false;
    }
    crate::storage::place_paths::may_overlap(&left.path, &right.path)
}
