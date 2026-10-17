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
mod boundary_calls;
pub use boundary_calls::{
    BoundaryCompletionResult, boundary_call_preserves_descriptor_storage,
    boundary_completion_preserves_descriptor_storage,
};
mod compatibility;
mod contents;
mod payload_entry;
mod payload_history;
mod payload_predicates;
mod scalar_contracts;
pub use contents::{
    DescriptorContentsOperand, DescriptorContentsQuery, DescriptorContentsStep,
    DescriptorContentsTransfer, DescriptorPayloadTarget, ElementPayloadMemberships,
    PayloadMembershipContract, descriptor_contents_query, descriptor_contents_transfers,
    element_payload_memberships, element_payload_memberships_in, payload_memberships,
    terminal_payload_contract,
};
pub use payload_history::{current_payload_contract, current_selected_payload_contract};
pub use payload_predicates::{PayloadScalarObservation, scalar_payload_expression};
pub use scalar_contracts::instantiate_payload_scalar_guarantees;
mod owned_shape;
mod replay;
pub use compatibility::{DescriptorCompatibilityFailure, DescriptorCompatibilityRequirement};
pub use owned_shape::{owned_shared_byte_descriptor_shape, plain_owned_shape};
mod invocation;
mod terminal_flow;
pub use invocation::{public_invocation, public_result_slots, writable_descriptor_slots};
pub use replay::{
    DescriptorAnalysis, DescriptorSlotInventory, DescriptorSlotKind, DescriptorStoredCallEffect,
    DescriptorStoredEffect, DescriptorStoredOrigin, descriptor_slot_inventory, descriptor_slots,
    reconstruct_source_origins, reconstruct_stored_effects, writable_reference_slots,
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
    /// Provenance token used only by the placed-loan analysis. This names a
    /// checked formation, never physical storage or permission to access it.
    PlacedFormation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        place: PlaceId,
        carrier: StructuralTypeId,
    },
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
    IncomingContents(DescriptorContentsQuery),
}

/// Descriptor storage, independently of the permission it carries. Native
/// transport uses this query; shared observation must use is_shared_descriptor
/// and still establish current access and custody.
pub fn is_descriptor<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    declarations.next().is_none()
        && (declaration.shape.is_borrowed_byte_carrier()
            || matches!(declaration.shape, StructuralTypeShape::ElementView { .. }))
}

/// The backing permission carried by a descriptor value, not permission over
/// its storage slot. Affine retirement never widens a declared access ceiling.
/// Canonical descriptors keep access on their occurrence, where unrestricted
/// results are shared and affine results are exclusive. Provenance and current
/// loan availability remain independent checks.
pub fn descriptor_value_access<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
) -> Option<StructuralAccess> {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let declaration = declarations.next()?;
    if declarations.next().is_some() || multiplicity == StructuralMultiplicity::Linear {
        return None;
    }
    let access = match declaration.shape {
        StructuralTypeShape::ElementView { access, .. }
        | StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BorrowedView { access }
            | ByteSequenceCarrier::BoundedView { access, .. },
        ) => access,
        _ => return None,
    };
    let access = access.unwrap_or(if multiplicity == StructuralMultiplicity::Affine {
        StructuralAccess::MutableBorrow
    } else {
        StructuralAccess::SharedBorrow
    });
    match access {
        StructuralAccess::SharedBorrow => Some(access),
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            if multiplicity == StructuralMultiplicity::Affine =>
        {
            Some(access)
        }
        _ => None,
    }
}

/// Shared descriptor shape; multiplicity and origin remain separate judgments.
pub fn is_shared_descriptor<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    declarations.next().is_none()
        && (declaration.shape.is_shared_readable_byte_view()
            || matches!(
                declaration.shape,
                StructuralTypeShape::ElementView {
                    access: None | Some(StructuralAccess::SharedBorrow),
                    ..
                }
            ))
}

/// Backing access of a readable result, selected by the same declaration and
/// multiplicity judgment as other descriptor values. Inventory grants no access.
pub fn result_access<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    result: &terminal_psi::StructuralOperationResult,
) -> Option<StructuralAccess> {
    descriptor_value_access(types, result.structural_type, result.multiplicity)
        .filter(|access| matches!(access, terminal_psi::reading_borrow_pair_pattern!()))
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
    let result = machine.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }
    let graph = terminal_graph(module);
    let row = descriptor_machine(&graph, machine.id)?;
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

/// Replay an exact current return position in a caller-provided executable
/// catalog. Both Terminal and Omega adapters retain the same declared slots.
/// This supplies backing provenance, never carrier ownership or access.
pub fn reconstruct_return_slots_external(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    source: PlaceId,
) -> bool {
    let Some(machine) = descriptor_machine(machines, identity) else {
        return false;
    };
    if !machine.returns.as_ref().is_some_and(|returns| {
        returns.iter().any(|(returned_position, location)| {
            *returned_position == position && *location == DescriptorLocation::whole(source)
        })
    }) {
        return false;
    }
    replay::return_slots_have_external_backing(types, machines, identity, position, source)
        .is_some()
}

/// Every descriptor slot inside this exact returned aggregate has independently
/// reconstructed nonlocal backing, or is absent under a retained constructor
/// case. This proves provenance only; source ownership, slot access, claims and
/// the result signature remain the return validator's separate obligations.
pub fn return_slots_have_external_backing(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
) -> bool {
    let graph = terminal_graph(module);
    let Some(row) = descriptor_machine(&graph, machine.id) else {
        return false;
    };
    let Some(returns) = &row.returns else {
        return false;
    };
    let mut found = false;
    for (position, location) in returns
        .iter()
        .filter(|(_, location)| location.root == source && location.path.is_empty())
    {
        found = true;
        if replay::return_slots_have_external_backing(
            &module.structural_types,
            &graph,
            machine.id,
            *position,
            location.root,
        )
        .is_none()
        {
            return false;
        }
    }
    found
}

/// Substitute the callee's formal backing with this call's actual projections.
/// Literal origins retain their callee namespace and never become caller places.
pub fn call_origins(
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorCallOrigin>> {
    let graph = terminal_graph(module);
    call_origins_in(
        &mut DescriptorAnalysis::new(&module.structural_types, &graph),
        module,
        caller,
        operation,
    )
}

/// `call_origins` over an analysis of this module's `terminal_graph` that the
/// caller already holds, so asking about many calls builds the graph once.
pub fn call_origins_in(
    analysis: &mut DescriptorAnalysis<'_>,
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorCallOrigin>> {
    if !matches!(
        operation.kind,
        terminal_psi::call_structural_pair_pattern!(..) | OperationKind::CallDynamic { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
    {
        return None;
    }

    analysis.call_origins(caller.id, result.place)
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
    pub contents_queries: BTreeMap<
        (DescriptorPosition, DescriptorLocation, DescriptorLocation),
        DescriptorContentsQuery,
    >,
    pub contents_transfers: BTreeMap<DescriptorPosition, Vec<DescriptorContentsTransfer>>,
    /// Payload writes not described by structural value derivations. These
    /// retire value memberships independently of descriptor pointer contents.
    pub payload_producers: BTreeMap<DescriptorPosition, PlaceId>,
    /// Caller-owed entry propositions in their original formal namespace.
    pub payload_entry_predicates: Option<(
        semantic_vocabulary::PropositionContext,
        Vec<semantic_vocabulary::Proposition>,
    )>,
    /// Closed scalar contents captured from actual constructor/store operands.
    /// Unknown operands supply no observation and still invalidate old contents.
    /// New dynamic instances retire premises about earlier instances of these
    /// SSA identities, including block formals on loop arrivals.
    pub payload_scalar_definitions: BTreeMap<DescriptorPosition, Vec<semantic_vocabulary::ValueId>>,
    /// Independently checked scalar guarantees become available only after
    /// this exact normal-return operation; no structural storage facts enter.
    pub payload_scalar_postconditions:
        BTreeMap<DescriptorPosition, Vec<semantic_vocabulary::Proposition>>,
    pub payload_scalar_parameters:
        BTreeMap<semantic_vocabulary::BlockId, Vec<semantic_vocabulary::ValueId>>,
    /// Exact typed scalar reads capture immutable values from current storage.
    pub scalar_payload_reads:
        BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, semantic_vocabulary::ScalarTerm)>>,
    /// Selected Boolean facts, in the flow step's exact successor order.
    pub payload_edge_predicates:
        BTreeMap<DescriptorPosition, Vec<Option<semantic_vocabulary::Proposition>>>,
    /// Scalar arrivals in successor order. Simultaneous binding observes the
    /// old SSA instances before target formals acquire their new instances.
    pub payload_scalar_bindings: BTreeMap<
        DescriptorPosition,
        Vec<
            Vec<(
                semantic_vocabulary::ValueId,
                semantic_vocabulary::ScalarTerm,
            )>,
        >,
    >,
    pub scalar_payload_writes:
        BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, semantic_vocabulary::ScalarTerm)>>,
    pub payload_invalidations: BTreeMap<DescriptorPosition, Vec<Option<DescriptorContentsQuery>>>,
    /// Nonexact value arrivals retain their actual typed read query, indexed
    /// by successor ordinal. They supplement membership transport only: exact
    /// address/custody bindings in `flow` remain unchanged.
    pub payload_arrivals: BTreeMap<DescriptorPosition, Vec<Vec<PayloadMembershipBinding>>>,
    /// Unresolved writes cannot leave entry contents available for a later
    /// query. These operations require contents effects before replay is usable.
    pub unresolved_contents_writes: BTreeSet<DescriptorPosition>,
    /// Reference construction captures the source slot address, not its current
    /// pointer contents. Copies and moves remain ordinary value derivations.
    pub storage_derivations: BTreeSet<(DescriptorPosition, DescriptorLocation, DescriptorLocation)>,
    pub calls: BTreeMap<DescriptorPosition, DescriptorInvocation>,
    pub normal_exits: Vec<DescriptorPosition>,
}

pub struct PayloadMembershipBinding {
    pub parameter: PlaceId,
    pub source: Option<DescriptorContentsQuery>,
}

/// The graph row for `id`, when the catalog covers that machine.
pub fn descriptor_machine<'graph>(
    graph: &'graph [DescriptorMachine],
    id: MachineId,
) -> Option<&'graph DescriptorMachine> {
    graph.iter().find(|current| current.id == id)
}

pub struct DescriptorInvocation {
    pub target: DescriptorInvocationTarget,
    /// Every formal position is retained. None means its actual storage
    /// coordinate is unresolved, never that the invocation is absent.
    pub arguments: Vec<Option<DescriptorLocation>>,
    /// Complete readable backing candidates at the invocation snapshot.
    /// Runtime selections remain typed queries, never exact write addresses.
    pub argument_contents: Vec<Option<DescriptorContentsQuery>>,
    pub result: Option<DescriptorLocation>,
}

/// A direct body and a public requirement provide distinct invocation contracts.
/// Public regions bound backing reach, never the selected runtime descriptor.
pub enum DescriptorInvocationTarget {
    Machine(MachineId),
    Public {
        extent: semantic_vocabulary::ReferenceOriginExtent,
        results: Vec<DescriptorResultSlot>,
        invalidates_contents: bool,
    },
}

impl DescriptorInvocation {
    pub fn callee(&self) -> Option<MachineId> {
        match self.target {
            DescriptorInvocationTarget::Machine(machine) => Some(machine),
            DescriptorInvocationTarget::Public { .. } => None,
        }
    }
}

/// One typed returned descriptor slot and its complete possible backing origins.
#[derive(Clone)]
pub struct DescriptorResultSlot {
    pub path: Vec<crate::descriptor_lifetimes::DescriptorPathSegment>,
    pub origins: Vec<DescriptorStoredOrigin>,
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
                access: terminal_psi::reading_borrow_pair_pattern!(),
                ..
            } | StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView {
                        access: None | Some(terminal_psi::reading_borrow_pair_pattern!())
                    } | ByteSequenceCarrier::BoundedView {
                        access: None | Some(terminal_psi::reading_borrow_pair_pattern!()),
                        ..
                    }
                )
        ) {
            return true;
        }
        let Some(StructuralPathSegment::Field(identity)) = path.get(length) else {
            return false;
        };
        let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape else {
            return false;
        };
        terminal_psi::live_field_by_identity(fields, *identity)
            .and_then(|field| field.field_type.byte_sequence_carrier())
            .is_some_and(|carrier| {
                matches!(
                    carrier,
                    ByteSequenceCarrier::BorrowedView {
                        access: None | Some(terminal_psi::reading_borrow_pair_pattern!())
                    } | ByteSequenceCarrier::BoundedView {
                        access: None | Some(terminal_psi::reading_borrow_pair_pattern!()),
                        ..
                    }
                )
            })
    })
}

fn place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .signature_structural_parameter(place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .block_structural_parameters()
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
        })
        .or_else(|| {
            machine.structural_places.iter().find_map(|row| {
                if row.id != place {
                    return None;
                }
                match row.kind {
                    StructuralPlaceKind::Result => machine
                        .result
                        .structural()
                        .filter(|result| result.place == place)
                        .map(|result| result.structural_type),
                    _ => row.kind.produced_structural_type(),
                }
            })
        })
}

/// Return custody applies to nested descriptor slots of an owned aggregate,
/// regardless of which operation constructed it. Direct references and views
/// retain their separate return contracts. Missing or ambiguous types fail closed.
pub fn aggregate_descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    let mut matching = types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    if matches!(
        declaration.shape,
        StructuralTypeShape::Record { .. }
            | terminal_psi::sum_or_mixed_shape_pattern!()
            | StructuralTypeShape::FixedArray { .. }
    ) {
        DescriptorSlotKind::Views.owned_slots(types, root, structural_type)
    } else {
        Some(Vec::new())
    }
}

pub fn terminal_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    module
        .machines
        .iter()
        .filter_map(|machine| terminal_flow::adapt(module, machine, DescriptorSlotKind::Views))
        .collect()
}

/// Current Terminal reference-slot graph; pair with `with_reference_slots`.
pub fn terminal_reference_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    module
        .machines
        .iter()
        .filter_map(|machine| terminal_flow::adapt(module, machine, DescriptorSlotKind::References))
        .collect()
}

/// Placed-loan provenance uses the same exact descriptor transport graph,
/// retaining formation tokens separately from ordinary backing addresses.
pub fn terminal_placed_origin_graph(module: &TerminalModule) -> Option<Vec<DescriptorMachine>> {
    module
        .machines
        .iter()
        .map(|machine| terminal_flow::adapt(module, machine, DescriptorSlotKind::PlacedOrigins))
        .collect()
}

fn terminal_definition(
    _module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    kind: StructuralPlaceKind,
) -> Option<DescriptorDefinition> {
    match kind {
        StructuralPlaceKind::Parameter { position } => {
            let mut matching =
                machine
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        parameter.place == place && parameter.position == position
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
            let mut establishments = machine.operations().filter(|operation| matches!(operation.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == place));
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
                .operations()
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
    let graph = terminal_graph(module);
    call_stored_effects_in(
        &mut DescriptorAnalysis::new(&module.structural_types, &graph),
        machine,
        operation,
    )
}

/// `call_stored_effects` over an analysis of this module's `terminal_graph`
/// that the caller already holds.
pub fn call_stored_effects_in(
    analysis: &mut DescriptorAnalysis<'_>,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<Vec<DescriptorStoredCallEffect>> {
    analysis.call_stored_effects(machine.id, operation_position(machine, operation)?)
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
