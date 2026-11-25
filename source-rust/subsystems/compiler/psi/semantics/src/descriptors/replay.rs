/*
Position-sensitive descriptor contents replay engine (`DescriptorAnalysis`, `reconstruct_source_origins`,
`reconstruct_stored_effects`) tracking descriptor slot origins, case-flow narrowing, and symbolic
incoming-family joins across `TerminalMachine` control-flow graphs.
*/

use crate::descriptors::descriptor_lifetimes::{DescriptorPathSegment, descriptor_path};
use crate::descriptors::descriptor_results::*;
use crate::exactly_one::exactly_one;

mod input_contents;
mod location_map;
use location_map::LocationMap;

pub(crate) mod analysis;
pub(crate) mod case_flow;
pub(crate) mod contents_queries;
pub(crate) mod dispatch;
pub(crate) mod engine;
pub(crate) mod module_summaries;
pub(crate) mod provider;
pub(crate) mod summary_interface;
use crate::runtime_structural_path_tip;
pub use analysis::{DescriptorAnalysis, DescriptorSourceObservation, DescriptorSourceObservations};
use engine::Replay;
pub use module_summaries::ModuleSummaries;
use terminal_psi::find_structural_type;

/// Known reference origins and opaque value contents have different roles.
/// A contents packet records custody; it supplies no address or access mode.
#[derive(Clone)]
pub struct DescriptorResultContents {
    pub slots: Vec<DescriptorResultSlot>,
    pub unknown_contents: Vec<DescriptorResultSlot>,
    /// The flow retains all possible input loans for these descriptors.
    /// This lifetime account supplies no unique spatial origin.
    pub unresolved_slots: Vec<Vec<DescriptorPathSegment>>,
    pub(crate) case_facts: case_flow::ResultCases,
}

/// The owned contents supplied by a completed call whose result type is
/// still generic. The producing call and field identify the value without
/// giving the caller a reference to its hidden backing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProducedContents<Dependency> {
    pub(crate) producer: DescriptorInvocationSite,
    pub(crate) path: Vec<DescriptorPathSegment>,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) lifetime_dependencies: Vec<DescriptorLifetimeDebt<Dependency>>,
}

impl<Dependency> ProducedContents<Dependency> {
    fn with_dependencies<Mapped>(
        &self,
        lifetime_dependencies: Vec<DescriptorLifetimeDebt<Mapped>>,
    ) -> ProducedContents<Mapped> {
        ProducedContents {
            producer: self.producer.clone(),
            path: self.path.clone(),
            structural_type: self.structural_type,
            lifetime_dependencies,
        }
    }
}

#[derive(Clone)]
pub enum DescriptorStoredOrigin {
    Invocation {
        carrier_type: StructuralTypeId,
        projection: Projection,
    },
    Provider(DescriptorProviderOrigin<DescriptorStoredOrigin>),
    AccessFormation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        place: PlaceId,
        carrier: StructuralTypeId,
        projection: Projection,
    },
    Empty,
    Parameter {
        position: usize,
        projection: Projection,
    },
    IncomingSlot {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
    IncomingContents {
        position: usize,
        steps: Vec<DescriptorContentsStep>,
        projection: Projection,
    },
    ProducedContents(ProducedContents<DescriptorStoredOrigin>),
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
    /// The roster backing moves with this result origin. Its ordered element
    /// accounts retain the caller loans; carrying storage does not own their
    /// referents. Keep type and extent with this alternative, including empty
    /// descriptor inventories, rather than inferring ordinals from a nominal
    /// ElementView or forcing every return to have the same extent.
    Carried {
        element: StructuralTypeId,
        elements: Vec<Vec<DescriptorResultSlot>>,
    },
}
impl DescriptorStoredOrigin {
    pub(crate) fn element_at(
        origins: &[&Self],
        expected: StructuralTypeId,
        ordinal: u64,
    ) -> Option<()> {
        if origins.is_empty() {
            return None;
        }
        for origin in origins {
            let Self::Carried { element, elements } = origin else {
                return None;
            };
            if *element != expected || usize::try_from(ordinal).ok()? >= elements.len() {
                return None;
            }
        }
        Some(())
    }

    pub fn captured_entry_slot(
        &self,
    ) -> Option<(usize, std::borrow::Cow<'_, [DescriptorPathSegment]>)> {
        match self {
            Self::IncomingSlot { position, path } => {
                Some((*position, std::borrow::Cow::Borrowed(path)))
            }
            Self::IncomingContents {
                position,
                steps,
                projection,
            } if *projection == Projection::new(&[]) => Some((
                *position,
                std::borrow::Cow::Owned(
                    steps
                        .iter()
                        .map(|step| match step {
                            DescriptorContentsStep::Address(segment) => Some(*segment),
                            DescriptorContentsStep::Element { .. } => None,
                        })
                        .collect::<Option<Vec<_>>>()?,
                ),
            )),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct DescriptorStoredEffect {
    pub parameter: usize,
    pub steps: Vec<DescriptorContentsStep>,
    pub origins: Vec<DescriptorStoredOrigin>,
}
impl DescriptorStoredEffect {
    /// Whether `origin` is the slot this effect describes, as it was when
    /// the machine was entered.
    pub(crate) fn keeps_incoming_slot(&self, origin: &DescriptorStoredOrigin) -> bool {
        let DescriptorStoredOrigin::IncomingSlot { position, path } = origin else {
            return false;
        };
        *position == self.parameter
            && path.len() == self.steps.len()
            && path.iter().zip(&self.steps).all(|(segment, step)| {
                matches!(step, DescriptorContentsStep::Address(actual) if actual == segment)
            })
    }

    /// Whether `origin` says this effect's own storage still holds what it
    /// held when the machine was entered: the slot itself, or the same
    /// contents reached through a reference or view, unprojected.
    pub(crate) fn keeps_incoming(&self, origin: &DescriptorStoredOrigin) -> bool {
        self.keeps_incoming_slot(origin)
            || matches!(
                origin,
                DescriptorStoredOrigin::IncomingContents { position, steps, projection }
                    if *position == self.parameter
                        && *steps == self.steps
                        && *projection == Projection::new(&[])
            )
    }

    pub fn exact_path(&self) -> Option<Vec<DescriptorPathSegment>> {
        self.steps
            .iter()
            .map(|step| match step {
                DescriptorContentsStep::Address(segment) => Some(*segment),
                DescriptorContentsStep::Element { .. } => None,
            })
            .collect()
    }
}
pub struct DescriptorStoredCallEffect<Origin = DescriptorCallOrigin> {
    pub destination: DescriptorContentsQuery,
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
        DescriptorStoredOrigin::Invocation {
            carrier_type,
            projection,
        } => Some(DescriptorResultOrigin::Invocation {
            carrier_type,
            projection,
        }),
        DescriptorStoredOrigin::AccessFormation { .. }
        | DescriptorStoredOrigin::Provider(_)
        | DescriptorStoredOrigin::ProducedContents(_)
        | DescriptorStoredOrigin::Carried { .. } => None,
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
        DescriptorStoredOrigin::IncomingContents {
            position,
            steps,
            projection,
        } => {
            let parameter = machine.places.iter().find_map(|row| match row.definition {
                DescriptorDefinition::Parameter {
                    position: actual,
                    structural_type,
                    ..
                } if actual == position => Some((row.place, structural_type)),
                _ => None,
            })?;
            let query = DescriptorContentsQuery {
                root: parameter.0,
                steps,
            };
            Some(DescriptorResultOrigin::Parameter {
                position,
                projection: query
                    .conservative_projection(types, parameter.1)?
                    .then(&projection),
            })
        }
    }
}
pub(crate) fn result_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<Vec<DescriptorResultOrigin>> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    replay
        .summary(identity, 0)?
        .result
        .into_slots()?
        .into_iter()
        .find(|slot| slot.path.is_empty())?
        .origins
        .into_iter()
        .map(|origin| result_origin(types, machine, origin))
        .collect()
}
pub(crate) fn return_origins(
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
pub(crate) fn return_slots_have_external_backing(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    source: PlaceId,
) -> Option<()> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    let slots =
        DescriptorSlotKind::Views.owned_slots(types, source, *machine.place_types.get(&source)?)?;
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

pub(crate) fn call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).call_origins(identity, result)
}

fn may_contain_descriptor(
    kind: DescriptorSlotKind,
    scope: DescriptorStorageScope,
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
                may_contain_descriptor(kind, scope, types, ty, active)
            }
            _ => kind.is_view_field_slot(&field.field_type),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::Reference { referent, .. } => {
            kind.is_slot(types, ty)
                || (scope != DescriptorStorageScope::Owned
                    && may_contain_descriptor(kind, scope, types, *referent, active))
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            may_contain_descriptor(kind, scope, types, *element, active)
        }
        StructuralTypeShape::TypeParameter => true,
        _ => kind.is_slot(types, ty),
    };
    active.remove(&ty);
    contains
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorSlotKind {
    Views,
    References,
    BorrowedStorage,
    AccessOrigins,
}
impl DescriptorSlotKind {
    pub fn tracks_returned_value(
        self,
        types: &[StructuralTypeDeclaration],
        place: PlaceId,
        structural_type: StructuralTypeId,
    ) -> bool {
        self.owned_inventory(types, place, structural_type)
            .is_some_and(|inventory| {
                !inventory.slots.is_empty() || !inventory.unknown_contents.is_empty()
            })
            || case_flow::contains_case(
                types,
                structural_type,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
    }

    pub fn tracks_views(self) -> bool {
        matches!(self, Self::Views | Self::BorrowedStorage)
    }

    pub fn tracks_references(self) -> bool {
        matches!(self, Self::References | Self::BorrowedStorage)
    }

    pub fn is_slot(self, types: &[StructuralTypeDeclaration], ty: StructuralTypeId) -> bool {
        match self {
            Self::Views => is_descriptor(types.iter(), ty),
            Self::AccessOrigins => find_structural_type(types, ty).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference { .. }
                        | StructuralTypeShape::Record {
                            storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                                | terminal_psi::StructuralRecordStorage::Accessor(_),
                            ..
                        }
                )
            }),
            Self::References => find_structural_type(types, ty).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            }),
            Self::BorrowedStorage => find_structural_type(types, ty)
                .is_some_and(|declaration| declaration.shape.is_borrowed_descriptor()),
        }
    }

    pub(crate) fn parameter_is_incoming_slot(
        self,
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> bool {
        match self {
            Self::BorrowedStorage => Self::References.is_slot(types, ty),
            Self::Views => false,
            Self::References | Self::AccessOrigins => self.is_slot(types, ty),
        }
    }
    pub(crate) fn is_view_field_slot(self, field_type: &terminal_psi::StructuralFieldType) -> bool {
        self.tracks_views()
            && field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| carrier.is_borrowed())
    }
    pub fn slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        let inventory = self.inventory(types, root, structural_type)?;
        inventory
            .unknown_contents
            .is_empty()
            .then_some(inventory.slots)
    }
    pub fn owned_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        let inventory = self.owned_inventory(types, root, structural_type)?;
        inventory
            .unknown_contents
            .is_empty()
            .then_some(inventory.slots)
    }

    pub fn owned_inventory(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<DescriptorSlotInventory> {
        slot_inventory(
            self,
            DescriptorStorageScope::Owned,
            types,
            root,
            structural_type,
        )?
        .fully_enumerated()
    }

    pub fn inventory(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<DescriptorSlotInventory> {
        slot_inventory(
            self,
            DescriptorStorageScope::Reachable,
            types,
            root,
            structural_type,
        )?
        .fully_enumerated()
    }

    pub(crate) fn entry_inventory(
        self,
        types: &[StructuralTypeDeclaration],
        machine: &DescriptorMachine,
        root: PlaceId,
    ) -> Option<DescriptorSlotInventory> {
        let structural_type = *machine.place_types.get(&root)?;
        let shared_input = matches!(
            machine.place_definition(root)?,
            DescriptorDefinition::Parameter {
                access: StructuralAccess::SharedBorrow,
                ..
            }
        );
        let scope = if self.tracks_references() {
            DescriptorStorageScope::Owned
        } else {
            DescriptorStorageScope::Reachable
        };
        let inventory = slot_inventory(self, scope, types, root, structural_type)?;
        if shared_input {
            Some(inventory)
        } else {
            inventory.fully_enumerated()
        }
    }

    pub fn may_write_indirect_contents(
        self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
    ) -> bool {
        if argument.access == StructuralAccess::SharedBorrow {
            return false;
        }
        let Some(selected) = runtime_structural_path_tip(types.iter(), root_type, &argument.path)
        else {
            return true;
        };
        let behind_view = argument.path.iter().enumerate().any(|(position, segment)| {
            matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::FixedIndex(_)
            ) && runtime_structural_path_tip(types.iter(), root_type, &argument.path[..position])
                .is_none_or(|prefix| {
                    matches!(prefix.shape, StructuralTypeShape::ElementView { .. })
                })
        });
        indirect_contents_contain_slot(self, types, selected.id, behind_view, &mut BTreeSet::new())
    }

    pub fn overwritten_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Option<Vec<DescriptorLocation>> {
        let mut destination = path.to_vec();
        if let Some(field) = field {
            let declaration =
                crate::projected_field_declaration(types.iter(), structural_type, path, field)?;
            destination.push(StructuralPathSegment::Field(declaration.identity));
        } else {
            runtime_structural_path_tip(types.iter(), structural_type, path)?;
        }
        let mut overwritten = Vec::new();
        for slot in self.slots(types, root, structural_type)? {
            let resident = descriptor_path(types, structural_type, &slot.path)?;
            let mut overlaps = true;
            for (position, selected) in destination.iter().enumerate() {
                if matches!(selected, StructuralPathSegment::RepresentationView { .. }) {
                    break;
                }
                if !resident.get(position).is_some_and(|stored| {
                    stored == selected
                        || matches!(
                            (selected, stored),
                            (
                                StructuralPathSegment::RuntimeIndex { .. },
                                StructuralPathSegment::FixedIndex(_)
                            )
                        )
                }) {
                    overlaps = false;
                    break;
                }
            }
            if overlaps {
                overwritten.push(slot);
            }
        }
        Some(overwritten)
    }
}

fn indirect_contents_contain_slot(
    kind: DescriptorSlotKind,
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    behind_view: bool,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if behind_view && kind.is_slot(types, structural_type) {
        return true;
    }
    if !active.insert(structural_type) {
        return true;
    }
    let Some(declaration) = find_structural_type(types, structural_type) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(child) => {
                indirect_contents_contain_slot(kind, types, child, behind_view, active)
            }
            _ => behind_view && kind.is_view_field_slot(&field.field_type),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::ElementView { element, .. } => {
            indirect_contents_contain_slot(kind, types, *element, true, active)
        }
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::Reference {
            referent: element, ..
        } => indirect_contents_contain_slot(kind, types, *element, behind_view, active),
        StructuralTypeShape::Record { fields, .. } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::TypeParameter => true,
        _ => false,
    };
    active.remove(&structural_type);
    contains
}

impl DescriptorSlotKind {
    pub fn writable_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
        access: StructuralAccess,
    ) -> Option<Vec<DescriptorLocation>> {
        if access == StructuralAccess::SharedBorrow {
            return Some(Vec::new());
        }
        if self.tracks_views()
            && matches!(
                find_structural_type(types, structural_type)?.shape,
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { access: None }
                        | ByteSequenceCarrier::BoundedView { access: None, .. }
                ) | StructuralTypeShape::ElementView { access: None, .. }
            )
        {
            return Some(Vec::new());
        }
        let inventory = self.inventory(types, root, structural_type)?;
        let writable = |path: &[DescriptorPathSegment]| -> Option<bool> {
            let mut external = access != StructuralAccess::Owned;
            let mut permission = access;
            for (index, segment) in path.iter().enumerate() {
                if *segment == DescriptorPathSegment::Referent {
                    if matches!(
                        permission,
                        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        return Some(false);
                    }
                    let ty = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
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
        for opaque in inventory.unknown_contents {
            if writable(&opaque.location.path)? {
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
}

pub struct DescriptorSlotInventory {
    pub slots: Vec<DescriptorLocation>,
    pub unknown_contents: Vec<UnknownDescriptorContents>,
}

impl DescriptorSlotInventory {
    fn fully_enumerated(self) -> Option<Self> {
        self.unknown_contents
            .iter()
            .all(|item| item.kind != UnknownContentsKind::SymbolicArray)
            .then_some(self)
    }
}

/// An owned value's unknown contents travel with the value. A pointer's
/// unknown referent remains separate storage and is never an owned payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownContentsKind {
    Value,
    Referent,
    /// The shared input supplies an array whose length is a parameter. Its
    /// elements cannot be enumerated when the machine starts.
    SymbolicArray,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownDescriptorContents {
    pub location: DescriptorLocation,
    pub kind: UnknownContentsKind,
}

pub fn descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    DescriptorSlotKind::Views.slots(types, root, structural_type)
}

pub fn descriptor_slot_inventory(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    DescriptorSlotKind::Views.inventory(types, root, structural_type)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DescriptorStorageScope {
    Owned,
    Reachable,
    /// Traversal crossed a reference. Its descendants belong to the
    /// referent's storage even when their declarations are represented.
    Referent,
}

fn slot_inventory(
    kind: DescriptorSlotKind,
    scope: DescriptorStorageScope,
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    fn walk(
        kind: DescriptorSlotKind,
        scope: DescriptorStorageScope,
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
        location: DescriptorLocation,
        active: &mut BTreeSet<StructuralTypeId>,
        result: &mut DescriptorSlotInventory,
    ) -> Option<()> {
        if !active.insert(ty) || result.slots.len() + result.unknown_contents.len() > 4096 {
            return None;
        }
        let declaration = exactly_one(types.iter(), |declaration| declaration.id == ty)?;
        if kind.is_slot(types, ty) {
            result.slots.push(location.clone());
            if !kind.tracks_references() || scope == DescriptorStorageScope::Owned {
                active.remove(&ty);
                return Some(());
            }
        }
        match &declaration.shape {
            StructuralTypeShape::TypeParameter => {
                let mut contents = location.clone();
                contents.path.push(DescriptorPathSegment::UnknownContents);
                result.unknown_contents.push(UnknownDescriptorContents {
                    location: contents,
                    kind: if scope == DescriptorStorageScope::Referent {
                        UnknownContentsKind::Referent
                    } else {
                        UnknownContentsKind::Value
                    },
                });
            }
            StructuralTypeShape::Reference { referent, .. }
                if scope != DescriptorStorageScope::Owned =>
            {
                let mut child = location.clone();
                child.path.push(DescriptorPathSegment::Referent);
                if active.contains(referent) || find_structural_type(types, *referent).is_none() {
                    result.unknown_contents.push(UnknownDescriptorContents {
                        location: child,
                        kind: UnknownContentsKind::Referent,
                    });
                } else {
                    walk(
                        kind,
                        DescriptorStorageScope::Referent,
                        types,
                        *referent,
                        child,
                        active,
                        result,
                    )?;
                }
            }
            terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::Field(field.id));
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(kind, scope, types, ty, child, active, result)?
                        }
                        _ if kind.is_view_field_slot(&field.field_type) => result.slots.push(child),
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !may_contain_descriptor(kind, scope, types, *element, &mut BTreeSet::new()) {
                    active.remove(&ty);
                    return Some(());
                }
                let Some(length) = length.literal() else {
                    if scope == DescriptorStorageScope::Referent {
                        return None;
                    }
                    let mut contents = location;
                    contents.path.push(DescriptorPathSegment::UnknownContents);
                    result.unknown_contents.push(UnknownDescriptorContents {
                        location: contents,
                        kind: UnknownContentsKind::SymbolicArray,
                    });
                    active.remove(&ty);
                    return Some(());
                };
                if length > 4096 {
                    return None;
                }
                for index in 0..length {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::FixedIndex(index));
                    walk(kind, scope, types, *element, child, active, result)?;
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
                            walk(kind, scope, types, ty, child, active, result)?
                        }
                        _ if kind.is_view_field_slot(&field.field_type) => result.slots.push(child),
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
        unknown_contents: Vec::new(),
    };
    walk(
        kind,
        scope,
        types,
        structural_type,
        DescriptorLocation::whole(root),
        &mut BTreeSet::new(),
        &mut result,
    )?;
    Some(result)
}
