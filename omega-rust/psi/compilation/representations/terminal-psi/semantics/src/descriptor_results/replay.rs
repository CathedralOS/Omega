//! One position-sensitive contents replay over the descriptor cohort graph.
//!
//! A returned aggregate owns its carrier but may borrow through several declared
//! slots. Summarize those slots separately and substitute their incoming origins
//! from the pre-call snapshot; treating the carrier as backing either loses the
//! caller loan or invents a loan on the temporary result. Missing active contents
//! remain unknown. Exact case constructors alone mark inactive slots empty.
use super::*;
use crate::descriptor_lifetimes::{DescriptorPathSegment, descriptor_path};

mod analysis;
mod replay;
pub use analysis::DescriptorAnalysis;
use replay::Replay;
use terminal_psi::find_structural_type;

#[derive(Clone)]
pub enum DescriptorStoredOrigin {
    Empty,
    Parameter {
        position: usize,
        projection: Projection,
    },
    IncomingSlot {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}
#[derive(Clone)]
pub struct DescriptorStoredEffect {
    pub parameter: usize,
    pub path: Vec<DescriptorPathSegment>,
    pub origins: Vec<DescriptorStoredOrigin>,
}
pub struct DescriptorStoredCallEffect<Origin = DescriptorCallOrigin> {
    pub destination: DescriptorLocation,
    /// All possible post-call origins, including the pre-call slot when preserved.
    pub origins: Vec<Origin>,
    pub preserves_incoming: bool,
}

pub fn reconstruct_source_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).source_origins(identity, position, location)
}
pub fn reconstruct_stored_effects(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<Vec<DescriptorStoredEffect>> {
    DescriptorAnalysis::new(types, machines).stored_effects(identity)
}
fn result_origin(
    types: &[StructuralTypeDeclaration],
    machine: &DescriptorMachine,
    origin: DescriptorStoredOrigin,
) -> Option<DescriptorResultOrigin> {
    match origin {
        DescriptorStoredOrigin::Empty => Some(DescriptorResultOrigin::Empty),
        DescriptorStoredOrigin::Parameter {
            position,
            projection,
        } => Some(DescriptorResultOrigin::Parameter {
            position,
            projection,
        }),
        DescriptorStoredOrigin::Literal { machine, place } => {
            Some(DescriptorResultOrigin::Literal { machine, place })
        }
        DescriptorStoredOrigin::IncomingSlot { position, path } => {
            let root_type = machine.places.iter().find_map(|row| match row.definition {
                DescriptorDefinition::Parameter {
                    position: p,
                    structural_type,
                    ..
                } if p == position => Some(structural_type),
                _ => None,
            })?;
            let mut projection = Projection::new(&descriptor_path(types, root_type, &path)?);
            projection.indirect = true;
            Some(DescriptorResultOrigin::Parameter {
                position,
                projection,
            })
        }
    }
}
pub(super) fn result_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<Vec<DescriptorResultOrigin>> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    replay
        .summary(identity, 0)?
        .result?
        .into_iter()
        .find(|slot| slot.path.is_empty())?
        .origins
        .into_iter()
        .map(|origin| result_origin(types, machine, origin))
        .collect()
}
pub(super) fn return_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorResultOrigin>> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    let states = replay.states(identity, 0)?;
    replay
        .read(machine, states.get(&position)?, location)?
        .iter()
        .map(|origin| result_origin(types, machine, replay.exported(machine, origin)?))
        .collect()
}
pub(super) fn return_slots_have_external_backing(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    source: PlaceId,
) -> Option<()> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    let slots = descriptor_slots(types, source, *machine.place_types.get(&source)?)?;
    if slots.is_empty() || slots.iter().any(|slot| slot.path.is_empty()) {
        return None;
    }
    let states = replay.states(identity, 0)?;
    let state = states.get(&position)?;
    for slot in slots {
        let origins = replay.read(machine, state, &slot)?;
        if origins.is_empty() {
            return None;
        }
        for origin in origins {
            replay.exported(machine, &origin)?;
        }
    }
    Some(())
}

pub(super) fn call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).call_origins(identity, result)
}

fn may_contain_descriptor(
    kind: DescriptorSlotKind,
    types: &[StructuralTypeDeclaration],
    ty: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !active.insert(ty) {
        return true;
    }
    let Some(declaration) = find_structural_type(types, ty) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(ty) => {
                may_contain_descriptor(kind, types, ty, active)
            }
            _ => {
                kind == DescriptorSlotKind::SharedViews
                    && field
                        .field_type
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| carrier.is_shared_readable_view())
            }
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::Reference { referent, .. } => {
            kind == DescriptorSlotKind::References
                || may_contain_descriptor(kind, types, *referent, active)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            may_contain_descriptor(kind, types, *element, active)
        }
        _ => kind.is_slot(types, ty),
    };
    active.remove(&ty);
    contains
}

/// Independent inventories for view backing and ordinary pointer custody.
/// Neither inventory grants copyability, observation, or access authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorSlotKind {
    SharedViews,
    References,
}
impl DescriptorSlotKind {
    pub fn is_slot(self, types: &[StructuralTypeDeclaration], ty: StructuralTypeId) -> bool {
        match self {
            Self::SharedViews => is_shared_descriptor(types.iter(), ty),
            Self::References => find_structural_type(types, ty).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            }),
        }
    }
    pub fn slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        let inventory = self.inventory(types, root, structural_type)?;
        inventory
            .opaque_referents
            .is_empty()
            .then_some(inventory.slots)
    }
    pub fn inventory(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<DescriptorSlotInventory> {
        slot_inventory(self, types, root, structural_type)
    }
}

/// Pointer slots whose storage a parameter can replace outside its owned
/// carrier. Owned aggregates may lend external storage through reference fields;
/// an outer ownership mode alone cannot classify their stored effects.
pub fn writable_reference_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
) -> Option<Vec<DescriptorLocation>> {
    // Shared carrier access cannot acquire stronger permission by reading an
    // embedded exclusive pointer. Its unavailable pointee layout is irrelevant
    // to pointer-slot writes, even for an open or growing recursive schema.
    if access == StructuralAccess::SharedBorrow {
        return Some(Vec::new());
    }
    let inventory = DescriptorSlotKind::References.inventory(types, root, structural_type)?;
    let writable = |path: &[DescriptorPathSegment]| -> Option<bool> {
        let mut external = access != StructuralAccess::Owned;
        let mut permission = access;
        for (index, segment) in path.iter().enumerate() {
            if *segment == DescriptorPathSegment::Referent {
                // A write-only carrier may replace its resident pointer, but
                // cannot read that pointer to reach another storage region.
                if matches!(
                    permission,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ) {
                    return Some(false);
                }
                let ty = crate::descriptor_lifetimes::descriptor_location_type(
                    types,
                    structural_type,
                    &path[..index],
                )?;
                let StructuralTypeShape::Reference { access, .. } =
                    find_structural_type(types, ty)?.shape
                else {
                    return None;
                };
                external = true;
                permission = access;
            }
        }
        Some(external && matches!(permission, terminal_psi::exclusive_borrow_pair_pattern!()))
    };
    // Only an inaccessible/read-only opaque subtree can be ignored. Unknown
    // storage reachable with write authority still prevents a complete effect.
    for opaque in inventory.opaque_referents {
        if writable(&opaque.path)? {
            return None;
        }
    }
    let mut slots = Vec::new();
    for slot in inventory.slots {
        if writable(&slot.path)? {
            slots.push(slot);
        }
    }
    Some(slots)
}

/// Materialized descriptor slots and referent subtrees whose layout is not
/// available. Reference identity alone does not demand a pointee definition;
/// recursive pointer closure is not a finite storage inventory. Unknown
/// subtrees remain explicit so reading them cannot invent a storage origin.
pub struct DescriptorSlotInventory {
    pub slots: Vec<DescriptorLocation>,
    pub opaque_referents: Vec<DescriptorLocation>,
}

/// Enumerate the complete slot roster when every reachable layout is available.
/// Consumers that require complete return/effect coverage must retain this gate.
pub fn descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    DescriptorSlotKind::SharedViews.slots(types, root, structural_type)
}

/// Inventory known slots without letting an unrelated opaque pointer erase
/// observations of materialized storage in the same machine.
pub fn descriptor_slot_inventory(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    DescriptorSlotKind::SharedViews.inventory(types, root, structural_type)
}

fn slot_inventory(
    kind: DescriptorSlotKind,
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    fn walk(
        kind: DescriptorSlotKind,
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
        location: DescriptorLocation,
        active: &mut BTreeSet<StructuralTypeId>,
        result: &mut DescriptorSlotInventory,
    ) -> Option<()> {
        if !active.insert(ty) || result.slots.len() + result.opaque_referents.len() > 4096 {
            return None;
        }
        let mut matching = types.iter().filter(|declaration| declaration.id == ty);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        if kind.is_slot(types, ty) {
            result.slots.push(location.clone());
            if kind == DescriptorSlotKind::SharedViews {
                active.remove(&ty);
                return Some(());
            }
        }
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, .. } => {
                let mut child = location.clone();
                child.path.push(DescriptorPathSegment::Referent);
                if active.contains(referent) || find_structural_type(types, *referent).is_none() {
                    result.opaque_referents.push(child);
                } else {
                    walk(kind, types, *referent, child, active, result)?;
                }
            }
            terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::Field(field.id));
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(kind, types, ty, child, active, result)?
                        }
                        _ if kind == DescriptorSlotKind::SharedViews
                            && field
                                .field_type
                                .byte_sequence_carrier()
                                .is_some_and(|carrier| carrier.is_shared_readable_view()) =>
                        {
                            result.slots.push(child)
                        }
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !may_contain_descriptor(kind, types, *element, &mut BTreeSet::new()) {
                    active.remove(&ty);
                    return Some(());
                }
                if *length > 4096 {
                    return None;
                }
                for index in 0..*length {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::FixedIndex(index));
                    walk(kind, types, *element, child, active, result)?;
                }
            }
            _ => {}
        }
        if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
            &declaration.shape
        {
            for case in cases {
                for field in case
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                {
                    let mut child = location.clone();
                    child.path.extend([
                        DescriptorPathSegment::Case(case.id),
                        DescriptorPathSegment::Field(field.id),
                    ]);
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(kind, types, ty, child, active, result)?
                        }
                        _ if kind == DescriptorSlotKind::SharedViews
                            && field
                                .field_type
                                .byte_sequence_carrier()
                                .is_some_and(|carrier| carrier.is_shared_readable_view()) =>
                        {
                            result.slots.push(child)
                        }
                        _ => {}
                    }
                }
            }
        }
        active.remove(&ty);
        Some(())
    }
    let mut result = DescriptorSlotInventory {
        slots: Vec::new(),
        opaque_referents: Vec::new(),
    };
    walk(
        kind,
        types,
        structural_type,
        DescriptorLocation::whole(root),
        &mut BTreeSet::new(),
        &mut result,
    )?;
    Some(result)
}
