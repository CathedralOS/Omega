//! Current value equality links live storage occurrences, not origin labels.
//! Replacing a member retires its links; independent copied snapshots survive.
//! Immutable rosters and their member index share ordered pages across cuts.
//! A roster's least member indexes that CURRENT relation; it is not a value
//! version or producer identity, and retirement recomputes it when necessary.
use super::super::origin_maps::OriginMap;
use super::*;
use crate::descriptors::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};
use terminal_psi::StructuralFieldType;

#[cfg(test)]
mod tests;

type Members = OriginMap<DescriptorLocation, ()>;
type Classes = OriginMap<DescriptorLocation, Arc<Members>>;
type MemberClasses = OriginMap<DescriptorLocation, DescriptorLocation>;
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Copies {
    classes: Arc<Classes>,
    member_classes: Arc<MemberClasses>,
}
impl Copies {
    pub(super) fn counts(&self) -> (usize, usize) {
        (self.classes.len(), self.member_classes.len())
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
            for length in 0..=location.path.len() {
                let prefix = DescriptorLocation {
                    root: location.root,
                    path: location.path[..length].to_vec(),
                };
                let Some(representative) = self.member_classes.get(&prefix) else {
                    continue;
                };
                let suffix = &location.path[length..];
                if suffix.contains(&DescriptorPathSegment::Referent)
                    || !applied.insert((representative.clone(), suffix.to_vec()))
                {
                    continue;
                }
                if applied.len() > limit {
                    return None;
                }
                for member in self.classes.get(representative)?.keys() {
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
    pub(super) fn insert_class(
        &mut self,
        members: BTreeSet<DescriptorLocation>,
        limit: usize,
    ) -> Option<()> {
        if members.len() < 2 {
            return Some(());
        }
        let affected = members
            .iter()
            .filter_map(|member| self.member_classes.get(member).cloned())
            .collect::<BTreeSet<_>>();
        let base = affected
            .iter()
            .filter_map(|representative| self.classes.get(representative))
            .max_by_key(|members| members.len())
            .cloned();
        let mut combined = base.unwrap_or_default();
        for member in members.iter().chain(
            affected
                .iter()
                .filter_map(|representative| self.classes.get(representative))
                .flat_map(|members| members.keys()),
        ) {
            if !combined.contains_key(member) {
                Arc::make_mut(&mut combined).insert(member.clone(), ());
            }
        }
        let representative = combined.keys().next()?.clone();
        for old in &affected {
            if old != &representative {
                Arc::make_mut(&mut self.classes).remove(old);
            }
        }
        if self
            .classes
            .get(&representative)
            .is_none_or(|old| !Arc::ptr_eq(old, &combined))
        {
            Arc::make_mut(&mut self.classes).insert(representative.clone(), combined.clone());
        }
        for member in combined.keys() {
            if self.member_classes.get(member) != Some(&representative) {
                Arc::make_mut(&mut self.member_classes)
                    .insert(member.clone(), representative.clone());
            }
        }
        (self.member_classes.len() <= limit).then_some(())
    }
    pub(super) fn retain(&mut self, keep: impl Fn(&DescriptorLocation) -> bool) {
        let changes = self
            .classes
            .iter()
            .filter_map(|(representative, members)| {
                let removed = members
                    .keys()
                    .filter(|member| !keep(member))
                    .cloned()
                    .collect::<Vec<_>>();
                (!removed.is_empty()).then(|| (representative.clone(), members.clone(), removed))
            })
            .collect::<Vec<_>>();
        for (old, mut members, removed) in changes {
            for member in removed {
                Arc::make_mut(&mut members).remove(&member);
                Arc::make_mut(&mut self.member_classes).remove(&member);
            }
            Arc::make_mut(&mut self.classes).remove(&old);
            if members.len() < 2 {
                for member in members.keys() {
                    Arc::make_mut(&mut self.member_classes).remove(member);
                }
                continue;
            }
            let representative = members.keys().next().unwrap().clone();
            if representative != old {
                for member in members.keys() {
                    Arc::make_mut(&mut self.member_classes)
                        .insert(member.clone(), representative.clone());
                }
            }
            Arc::make_mut(&mut self.classes).insert(representative, members);
        }
    }
    pub(super) fn meet(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }
        let mut paired = BTreeMap::<
            (DescriptorLocation, DescriptorLocation),
            BTreeSet<DescriptorLocation>,
        >::new();
        for (member, left) in self.member_classes.iter() {
            if let Some(right) = other.member_classes.get(member) {
                paired
                    .entry((left.clone(), right.clone()))
                    .or_default()
                    .insert(member.clone());
            }
        }
        let groups = paired
            .into_values()
            .filter(|members| members.len() > 1)
            .map(|members| (members.first().unwrap().clone(), members))
            .collect::<BTreeMap<_, _>>();
        let mut result = self.clone();
        Arc::make_mut(&mut result.classes)
            .retain(|representative, _| groups.contains_key(representative));
        let mut assignments = BTreeMap::new();
        for (representative, members) in groups {
            let existing = self.classes.get(&representative).cloned();
            let roster = if let Some(existing) =
                existing.filter(|existing| existing.keys().eq(members.iter()))
            {
                existing
            } else {
                let mut roster = self
                    .member_classes
                    .get(&representative)
                    .and_then(|old| self.classes.get(old))
                    .cloned()
                    .unwrap_or_default();
                Arc::make_mut(&mut roster).retain(|member, _| members.contains(member));
                for member in &members {
                    if !roster.contains_key(member) {
                        Arc::make_mut(&mut roster).insert(member.clone(), ());
                    }
                }
                roster
            };
            if result
                .classes
                .get(&representative)
                .is_none_or(|old| !Arc::ptr_eq(old, &roster))
            {
                Arc::make_mut(&mut result.classes).insert(representative.clone(), roster);
            }
            assignments.extend(
                members
                    .into_iter()
                    .map(|member| (member, representative.clone())),
            );
        }
        Arc::make_mut(&mut result.member_classes)
            .retain(|member, _| assignments.contains_key(member));
        for (member, representative) in assignments {
            if result.member_classes.get(&member) != Some(&representative) {
                Arc::make_mut(&mut result.member_classes).insert(member, representative);
            }
        }
        result
    }
}

/// A typed operation assignment differs from an actual block arrival. Borrowed
/// arrivals name storage aliases; only Owned arrivals copy an independent value.
#[derive(Clone, Copy)]
pub(in crate::descriptors::descriptor_results::replay) enum Transfer<'edge> {
    Operation(DescriptorPosition),
    Arrival(&'edge DescriptorFlowEdge),
}

impl Transfer<'_> {
    pub(in crate::descriptors::descriptor_results::replay) fn position(
        self,
    ) -> Option<DescriptorPosition> {
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

/// Only ordinary owned homes prove disjointness. Incoming loans, access views,
/// descriptors and unresolved reference crossings never acquire allocation
/// identity from their type or from a different PlaceId.
pub(in crate::descriptors::descriptor_results::replay) struct Storage {
    roots: BTreeSet<PlaceId>,
    case_types: BTreeSet<StructuralTypeId>,
    pub(super) limit: usize,
}

impl Storage {
    pub(in crate::descriptors::descriptor_results::replay) fn new(
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
                replay.types,
                *structural_type,
                &mut case_types,
                &mut BTreeSet::new(),
            );
            let owned = match row.definition {
                DescriptorDefinition::Parameter { access, .. }
                | DescriptorDefinition::Arrival { access } => access == StructuralAccess::Owned,
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
    crate::descriptors::descriptor_lifetimes::descriptor_location_type(
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
pub(in crate::descriptors::descriptor_results::replay) fn contains_case(
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
    let mut child = |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
        StructuralFieldType::Structural(child) => contains_case(types, child, complete, active),
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
    if !super::super::super::owned_shape::plain_owned_shape(replay.types.iter(), structural_type)
        || !active.insert(structural_type)
    {
        return false;
    }
    let Some(declaration) = find_structural_type(replay.types, structural_type) else {
        return false;
    };
    let mut child = |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
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
