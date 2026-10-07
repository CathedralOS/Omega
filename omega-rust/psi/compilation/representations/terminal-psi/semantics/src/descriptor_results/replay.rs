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
use replay::{Origin, Replay, ResultSlot, State, States, Summary, Value};

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
    types: &[StructuralTypeDeclaration],
    ty: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !active.insert(ty) {
        return true;
    }
    let Some(declaration) = types.iter().find(|declaration| declaration.id == ty) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(ty) => {
                may_contain_descriptor(types, ty, active)
            }
            _ => matches!(
                field.field_type.byte_sequence_carrier(),
                Some(ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::SharedBorrow)
                })
            ),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::Record { fields } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::Reference { referent, .. } => {
            may_contain_descriptor(types, *referent, active)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            may_contain_descriptor(types, *element, active)
        }
        _ => is_shared_descriptor(types.iter(), ty),
    };
    active.remove(&ty);
    contains
}

/// Enumerate actual shared carrier slots, retaining reference boundaries.
pub fn descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    fn walk(
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
        location: DescriptorLocation,
        active: &mut BTreeSet<StructuralTypeId>,
        result: &mut Vec<DescriptorLocation>,
    ) -> Option<()> {
        if !active.insert(ty) || result.len() > 4096 {
            return None;
        }
        let mut matching = types.iter().filter(|declaration| declaration.id == ty);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        if is_shared_descriptor(types.iter(), ty) {
            result.push(location);
            active.remove(&ty);
            return Some(());
        }
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, .. } => {
                let mut child = location.clone();
                child.path.push(DescriptorPathSegment::Referent);
                walk(types, *referent, child, active, result)?;
            }
            StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::Field(field.id));
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(types, ty, child, active, result)?
                        }
                        _ if matches!(
                            field.field_type.byte_sequence_carrier(),
                            Some(ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow)
                            })
                        ) =>
                        {
                            result.push(child)
                        }
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !may_contain_descriptor(types, *element, &mut BTreeSet::new()) {
                    active.remove(&ty);
                    return Some(());
                }
                if *length > 4096 {
                    return None;
                }
                for index in 0..*length {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::FixedIndex(index));
                    walk(types, *element, child, active, result)?;
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
                            walk(types, ty, child, active, result)?
                        }
                        _ if matches!(
                            field.field_type.byte_sequence_carrier(),
                            Some(ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow)
                            })
                        ) =>
                        {
                            result.push(child)
                        }
                        _ => {}
                    }
                }
            }
        }
        active.remove(&ty);
        Some(())
    }
    let mut result = Vec::new();
    walk(
        types,
        structural_type,
        DescriptorLocation::whole(root),
        &mut BTreeSet::new(),
        &mut result,
    )?;
    Some(result)
}
