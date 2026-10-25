//! Current value equality is a relation between live storage occurrences, not
//! an origin label. Replacing either occurrence removes its links. A copied
//! snapshot keeps its own tag when another member is replaced; re-executing a
//! producer cannot join its new value to a surviving old snapshot.
use super::*;
use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};

/// Normalized current equivalence classes. Every exact member belongs to one
/// roster; the derived index is rebuilt from those rosters after a change. Class
/// positions are analysis scratch, never producer identities or version tokens.
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Copies {
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

    pub(super) fn equivalents(
        &self,
        subject: &DescriptorLocation,
        limit: usize,
    ) -> Option<BTreeSet<DescriptorLocation>> {
        let mut found = BTreeSet::from([subject.clone()]);
        let mut pending = vec![subject.clone()];
        let mut applied = BTreeSet::new();
        while let Some(location) = pending.pop() {
            // Consult only exact prefixes in the member index. One whole-value
            // class with N copies is expanded once for this suffix, rather than
            // rescanning N(N-1)/2 pairs for every reached member.
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

    /// The members were captured from one pre-step value component. Exact
    /// overlap merges whole rosters, retaining independent survivors when a
    /// former source is later overwritten. No clique of pair edges is stored.
    pub(super) fn insert_class(
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

    pub(super) fn retain(&mut self, keep: impl Fn(&DescriptorLocation) -> bool) {
        let classes = std::mem::take(&mut self.classes)
            .into_iter()
            .map(|mut members| {
                members.retain(&keep);
                members
            })
            .collect();
        *self = Self::normalized(classes);
    }

    pub(super) fn meet(&self, other: &Self) -> Self {
        // Two members remain equal only when they share a class on BOTH
        // arrivals. Grouping by the two current indexes intersects partitions
        // without materializing their pairwise relation. Normalization below
        // derives the resulting identities solely from the surviving members.
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

/// A typed operation assignment differs from an actual block arrival. Borrowed
/// arrivals name storage aliases; only Owned arrivals copy an independent value.
#[derive(Clone, Copy)]
pub(in crate::descriptor_results::replay) enum Transfer<'edge> {
    Operation(DescriptorPosition),
    Arrival(&'edge DescriptorFlowEdge),
}

impl Transfer<'_> {
    pub(in crate::descriptor_results::replay) fn position(self) -> Option<DescriptorPosition> {
        match self {
            Self::Operation(position) => Some(position),
            Self::Arrival(_) => None,
        }
    }

    /// A derivation may transport backing geometry rather than a value. Only
    /// an identity payload assignment, or an exact actual-location arrival,
    /// carries tag facts. Calls produce their results after their effects and
    /// must use completed summaries, never a pre-call derivation shortcut.
    pub(super) fn transports_value(
        self,
        machine: &DescriptorMachine,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> bool {
        match self {
            Self::Operation(position) => {
                let key = (position, source.clone(), destination.clone());
                !machine.calls.contains_key(&position)
                    && !machine.storage_derivations.contains(&key)
                    && !machine.projections.contains_key(&key)
                    && !machine.contents_queries.contains_key(&key)
            }
            Self::Arrival(edge) => edge
                .bindings
                .iter()
                .any(|binding| binding.locations() == Some((source, destination))),
        }
    }

    pub(super) fn copies_value(
        self,
        machine: &DescriptorMachine,
        source: &DescriptorLocation,
        destination: &DescriptorLocation,
    ) -> bool {
        self.transports_value(machine, source, destination)
            && match self {
                Self::Operation(_) => true,
                Self::Arrival(edge) => edge.bindings.iter().any(|binding| {
                    binding.access == StructuralAccess::Owned
                        && binding.locations() == Some((source, destination))
                }),
            }
    }
}

/// Only ordinary owned homes prove disjointness. Incoming loans, placed views,
/// descriptors and unresolved reference crossings never acquire allocation
/// identity from their type or from a different PlaceId.
pub(in crate::descriptor_results::replay) struct Storage {
    roots: BTreeSet<PlaceId>,
    case_types: BTreeSet<StructuralTypeId>,
    pub(super) limit: usize,
}

impl Storage {
    pub(in crate::descriptor_results::replay) fn new(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
    ) -> Self {
        let mut roots = BTreeSet::new();
        let mut ordinary_types = BTreeMap::new();
        let mut case_types = BTreeMap::new();
        for row in &machine.places {
            let Some(structural_type) = machine.place_types.get(&row.place) else {
                continue;
            };
            if !*ordinary_types
                .entry(*structural_type)
                .or_insert_with(|| ordinary_type(replay, *structural_type, &mut BTreeSet::new()))
            {
                continue;
            }
            contains_case(
                replay,
                *structural_type,
                &mut case_types,
                &mut BTreeSet::new(),
            );
            let owned = match row.definition {
                DescriptorDefinition::Parameter { access, .. } => access == StructuralAccess::Owned,
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

    pub(super) fn contains(&self, location: &DescriptorLocation) -> bool {
        self.roots.contains(&location.root)
            && !location.path.contains(&DescriptorPathSegment::Referent)
    }

    pub(super) fn copy_type(
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
            && find_structural_type(replay.types, source_type).is_some_and(|row| row.copyable)
    }
}

fn location_type(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    location: &DescriptorLocation,
) -> Option<StructuralTypeId> {
    crate::descriptor_lifetimes::descriptor_location_type(
        replay.types,
        *machine.place_types.get(&location.root)?,
        &location.path,
    )
}

/// A copied/stored/moved carrier transports its inline payload, not its
/// referent's contents. Validate the actual tag endpoints after substitution;
/// equal field numbers or backing projections cannot establish value equality.
pub(super) fn transported_subject(
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

/// Prepare only types whose value contains a tag. Scalar-only copies cannot
/// affect a case query and must not grow replay's equality work domain.
fn contains_case(
    replay: &Replay<'_>,
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
    let Some(declaration) = find_structural_type(replay.types, structural_type) else {
        return false;
    };
    let mut child = |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
        terminal_psi::StructuralFieldType::Structural(child) => {
            contains_case(replay, child, complete, active)
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
            contains_case(replay, *element, complete, active)
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
    if !super::super::super::owned_shape::plain_owned_shape(replay.types.iter(), structural_type)
        || !active.insert(structural_type)
    {
        return false;
    }
    let Some(declaration) = find_structural_type(replay.types, structural_type) else {
        return false;
    };
    let mut child = |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
        terminal_psi::StructuralFieldType::Structural(child) => {
            ordinary_type(replay, child, active)
        }
        _ => true,
    };
    let ordinary = match &declaration.shape {
        StructuralTypeShape::Record {
            storage, fields, ..
        } => {
            !matches!(
                storage,
                terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_)
            ) && fields.iter().all(&mut child)
        }
        StructuralTypeShape::Mixed { fields, cases } => fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
            .all(&mut child),
        StructuralTypeShape::Sum { cases } => {
            cases.iter().flat_map(|case| &case.fields).all(&mut child)
        }
        StructuralTypeShape::FixedArray { element, .. } => ordinary_type(replay, *element, active),
        _ => true,
    };
    active.remove(&structural_type);
    ordinary
}

/// Different case payloads may overlay. Only exact inline field/index siblings
/// prove non-overlap; a containing write also retires a contained tag fact.
pub(super) fn overlaps(left: &DescriptorLocation, right: &DescriptorLocation) -> bool {
    if left.root != right.root {
        return false;
    }
    for (left, right) in left.path.iter().zip(&right.path) {
        if left == right {
            continue;
        }
        return !matches!(
            (left, right),
            (
                DescriptorPathSegment::Field(_),
                DescriptorPathSegment::Field(_)
            ) | (
                DescriptorPathSegment::FixedIndex(_),
                DescriptorPathSegment::FixedIndex(_)
            )
        );
    }
    true
}
