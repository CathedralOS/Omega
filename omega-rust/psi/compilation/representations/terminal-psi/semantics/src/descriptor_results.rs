//! Returned descriptors borrow their original backing, never their carrier home.
//!
//! Reconstruct origins from the current bodies, including ordinary callees and
//! simultaneous block bindings. These are analysis results, not producer claims.
//! Operation verification still checks access, initialization, bounds and order.
//! An unresolved or recursive origin supplies no permission to escape storage.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachine, TerminalModule, Terminator,
};

use crate::borrowed_storage::Projection;
use crate::descriptor_lifetimes::{DescriptorFlowStep, DescriptorLocation, DescriptorPosition};
mod compatibility;
mod replay;
pub use compatibility::{DescriptorCompatibilityFailure, DescriptorCompatibilityRequirement};
mod terminal_flow;
pub use replay::{
    DescriptorAnalysis, DescriptorStoredCallEffect, DescriptorStoredEffect, DescriptorStoredOrigin,
    descriptor_slots, reconstruct_call_stored_effects, reconstruct_flow,
    reconstruct_source_origins, reconstruct_stored_effects,
};

#[derive(Clone)]
pub enum DescriptorResultOrigin {
    Empty,
    Parameter {
        position: usize,
        projection: Projection,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorCallOrigin {
    Empty,
    Argument {
        source: PlaceId,
        projection: Projection,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}

/// Receiving-only identity of a descriptor's backing. An incoming slot denotes
/// the value captured on machine entry, even after that slot is overwritten.
/// It conveys no disjointness or authority until the invocation is checked.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorSourceOrigin {
    Backing(DescriptorCallOrigin),
    IncomingSlot(DescriptorLocation),
}

/// Shape only: access, multiplicity and origin remain separate judgments.
pub fn is_shared_descriptor<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    declarations.next().is_none()
        && matches!(
            declaration.shape,
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::SharedBorrow),
                })
        )
}

/// Every possible backing of a normal shared descriptor result. A literal has
/// module residence; a parameter position denotes the backing supplied there.
pub fn result_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<Vec<DescriptorResultOrigin>> {
    let result = machine.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }
    reconstruct_result_origins(
        &module.structural_types,
        &terminal_graph(module),
        machine.id,
    )
}

/// The exact returned occurrence must itself resolve to nonlocal storage.
pub fn return_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> Option<Vec<DescriptorResultOrigin>> {
    let graph = terminal_graph(module);
    let row = graph.iter().find(|row| row.id == machine.id)?;
    let mut origins = Vec::new();
    for (position, location) in row
        .returns
        .as_ref()?
        .iter()
        .filter(|(_, location)| location.root == source && location.path.is_empty())
    {
        origins.extend(replay::return_origins(
            &module.structural_types,
            &graph,
            machine.id,
            *position,
            location,
        )?);
    }
    (!origins.is_empty()).then_some(origins)
}

/// Substitute the callee's formal backing with this call's actual projections.
/// Literal origins retain their callee namespace and never become caller places.
pub fn call_origins(
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorCallOrigin>> {
    if !matches!(
        operation.kind,
        OperationKind::CallStructural { .. }
            | OperationKind::CallStructuralWithScalarArguments { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }

    reconstruct_call_origins(
        &module.structural_types,
        &terminal_graph(module),
        caller.id,
        result.place,
    )
}

/// Current-program adapter records. These are disposable analysis inputs, not
/// retained origin authority. Adapters authenticate signatures and producers.
pub struct DescriptorMachine {
    pub id: MachineId,
    pub returns: Option<Vec<(DescriptorPosition, DescriptorLocation)>>,
    pub places: Vec<DescriptorPlace>,
    pub place_types: BTreeMap<PlaceId, StructuralTypeId>,
    pub entry: DescriptorPosition,
    pub flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    pub originations: BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, DescriptorCallOrigin)>>,
    pub projections:
        BTreeMap<(DescriptorPosition, DescriptorLocation, DescriptorLocation), Projection>,
    pub calls: BTreeMap<DescriptorPosition, DescriptorInvocation>,
    pub normal_exits: Vec<DescriptorPosition>,
}

pub struct DescriptorInvocation {
    pub callee: MachineId,
    /// Every formal position is retained. None means its actual storage
    /// coordinate is unresolved, never that the invocation is absent.
    pub arguments: Vec<Option<DescriptorLocation>>,
    pub result: Option<DescriptorLocation>,
}

pub struct DescriptorPlace {
    pub place: PlaceId,
    pub definition: DescriptorDefinition,
}

pub enum DescriptorDefinition {
    Parameter {
        position: usize,
        access: StructuralAccess,
        structural_type: StructuralTypeId,
    },
    Local,
    Literal,
}

pub fn reconstruct_result_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    machine: MachineId,
) -> Option<Vec<DescriptorResultOrigin>> {
    replay::result_origins(types, machines, machine)
}

pub fn reconstruct_call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    replay::call_origins(types, machines, caller, result)
}

fn external_backing(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    (0..=path.len()).any(|length| {
        let Some(declaration) =
            crate::runtime_structural_path_tip(types.iter(), root, &path[..length])
        else {
            return false;
        };
        if matches!(
            declaration.shape,
            StructuralTypeShape::Reference {
                access: StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
                ..
            } | StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    } | ByteSequenceCarrier::BoundedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ),
                        ..
                    }
                )
        ) {
            return true;
        }
        let Some(StructuralPathSegment::Field(identity)) = path.get(length) else {
            return false;
        };
        let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
            &declaration.shape
        else {
            return false;
        };
        fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())
            .and_then(|field| field.field_type.byte_sequence_carrier())
            .is_some_and(|carrier| {
                matches!(
                    carrier,
                    ByteSequenceCarrier::BorrowedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            )
                    } | ByteSequenceCarrier::BoundedView {
                        access: None
                            | Some(
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ),
                        ..
                    }
                )
            })
    })
}

fn place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
        })
        .or_else(|| {
            machine
                .structural_places
                .iter()
                .find_map(|row| match row.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if row.id == place => Some(structural_type),
                    _ => None,
                })
        })
}

pub fn terminal_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    module
        .machines
        .iter()
        .filter_map(|machine| terminal_flow::adapt(module, machine))
        .collect()
}

fn terminal_definition(
    _module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    kind: StructuralPlaceKind,
) -> Option<DescriptorDefinition> {
    match kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let mut matching =
                machine
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        parameter.place == place
                            && parameter.position == position
                            && parameter.is_self == is_self
                    });
            let (position, parameter) = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            Some(DescriptorDefinition::Parameter {
                position,
                access: parameter.access,
                structural_type: parameter.structural_type,
            })
        }
        StructuralPlaceKind::ByteSequenceLiteral { .. } => {
            let mut establishments = machine.blocks.iter().flat_map(|block| &block.operations).filter(|operation| matches!(operation.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == place));
            establishments.next()?;
            establishments
                .next()
                .is_none()
                .then_some(DescriptorDefinition::Literal)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            let mut matching = machine
                .blocks
                .iter()
                .filter(|candidate| candidate.id == block);
            let block = matching.next()?;
            if matching.next().is_some()
                || block.structural_parameters.get(position as usize)?.place != place
            {
                return None;
            }
            Some(DescriptorDefinition::Local)
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let mut matching = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == producer);
            let operation = matching.next()?;
            let result = operation.result.structural()?;
            if matching.next().is_some()
                || result.place != place
                || result.structural_type != structural_type
            {
                return None;
            }
            Some(DescriptorDefinition::Local)
        }
        _ => Some(DescriptorDefinition::Local),
    }
}

pub fn source_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    position: DescriptorPosition,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorCallOrigin>> {
    reconstruct_source_origins(
        &module.structural_types,
        &terminal_graph(module),
        machine.id,
        position,
        location,
    )
}
pub fn stored_effects(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<Vec<DescriptorStoredEffect>> {
    reconstruct_stored_effects(
        &module.structural_types,
        &terminal_graph(module),
        machine.id,
    )
}
pub fn call_stored_effects(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorStoredCallEffect>> {
    reconstruct_call_stored_effects(
        &module.structural_types,
        &terminal_graph(module),
        machine.id,
        operation_position(machine, operation)?,
    )
}
pub fn stored_source_origins(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorCallOrigin>> {
    let OperationKind::StructuralByteViewFieldStore { source, .. } = operation.kind else {
        return None;
    };
    // Reconstruct all normal exits as well as this pre-store occurrence: a
    // locally born backing cannot survive in borrowed output storage.
    stored_effects(module, machine)?;
    source_origins(
        module,
        machine,
        operation_position(machine, operation)?,
        &DescriptorLocation::whole(source),
    )
}
fn operation_position(
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<DescriptorPosition> {
    let mut positions = machine.blocks.iter().flat_map(|block| {
        block
            .operations
            .iter()
            .enumerate()
            .filter_map(move |(index, candidate)| {
                (candidate.id == operation.id && candidate == operation)
                    .then(|| Some((block.id, u32::try_from(index).ok()?)))
                    .flatten()
            })
    });
    let position = positions.next()?;
    positions.next().is_none().then_some(position)
}

#[cfg(test)]
mod tests;
