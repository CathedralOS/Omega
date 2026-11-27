pub(crate) use super::replay;

pub(crate) use crate::exactly_one::exactly_one;
pub(crate) use std::collections::{BTreeMap, BTreeSet};

pub(crate) use semantic_vocabulary::{
    MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId,
};
pub(crate) use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
};

pub(crate) use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
};
pub(crate) use crate::storage::borrowed_storage::Projection;
pub(crate) mod boundary_calls;
pub(crate) use crate::descriptors::descriptor_lifetimes;
pub(crate) use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
pub use boundary_calls::{
    BoundaryCompletionResult, boundary_call_preserves_descriptor_storage,
    boundary_completion_preserves_descriptor_storage,
};
pub(crate) use semantic_vocabulary::Proposition;
pub(crate) use semantic_vocabulary::PropositionContext;
pub(crate) use semantic_vocabulary::ScalarTerm;
pub(crate) use semantic_vocabulary::ValueId;
pub(crate) mod contents;
pub(crate) mod payload_entry;
pub(crate) use payload_entry::prove as prove_payload_predicate;
pub(crate) mod payload_history;
pub(crate) use payload_history::completed_payload_contract;
pub(crate) mod payload_predicates;
pub(crate) mod payload_reachability;
pub(crate) mod payload_versions;
pub(crate) mod scalar_captures;
pub(crate) mod value_identity;
pub use value_identity::{ValueIdentityAnalysis, ValueIdentitySeed};
pub(crate) mod scalar_contracts;
pub(crate) use contents::payload_memberships;
pub use contents::{
    DescriptorContentsOperand, DescriptorContentsQuery, DescriptorContentsStep,
    DescriptorContentsTransfer, DescriptorPayloadTarget, DescriptorStoreFlow,
    ElementPayloadMemberships, PayloadMembershipContract, descriptor_contents_query,
    descriptor_contents_query_with, descriptor_contents_transfers, descriptor_leaf_store,
    descriptor_scalar_arguments, element_payload_memberships, entry_selector,
    terminal_payload_contract,
};

pub(crate) use contents::{
    contents_membership_path, descriptor_storage_use_source, inline_selected_index,
};
pub use payload_history::PayloadMembershipHistory;
pub(crate) use payload_history::current_selected_payload_contract;
pub(crate) use payload_predicates::terminal_scalar_snapshots;
pub use payload_predicates::{
    PayloadEntryScalarIdentity, PayloadScalarObservation, TerminalScalarExpressions,
    scalar_payload_expression,
};
pub use payload_versions::{
    ProducedPayloadVersion, projected_stored_sources_in, require_unchanged_entry_storage,
    require_unchanged_observed_storage, unchanged_payload_producer,
    unchanged_stored_scalar_sources,
};
pub(crate) use payload_versions::{
    require_completed_scalar_capture, require_unchanged_produced_payload,
    require_unchanged_produced_payload_in, unchanged_stored_projection_sources_in,
};
pub(crate) use scalar_captures::scalar_storage_writes;
pub use scalar_captures::{
    CapturedScalarOperand, ResidentValueCapture, ResidentValueCaptures, ResidentValueTransfer,
    TerminalScalarCaptures, terminal_resident_value_transfers,
};
pub use scalar_contracts::{case_payload_scalar_types, instantiate_payload_scalar_guarantees};
pub(crate) mod owned_shape;
pub(crate) use owned_shape::{owned_shared_byte_descriptor_shape, plain_owned_shape};
pub(crate) mod invocation;
pub use invocation::selected_stored_effects_row;
mod private_parameters;
pub use private_parameters::PrivateObservationInputs;
pub(crate) mod provider_invocation;
pub use provider_invocation::provider_invocation;
pub(crate) use provider_invocation::provider_outputs_cover_writable_slots;
pub(crate) mod terminal_flow;
pub use invocation::{
    DescriptorMachineSignature, machine_invocation_for_signature, public_invocation,
    public_realizations, writable_descriptor_slots,
};
pub(crate) use invocation::{machine_invocation, public_result_slots};
pub use replay::{
    DescriptorAnalysis, DescriptorResultContents, DescriptorSlotInventory, DescriptorSlotKind,
    DescriptorSourceObservation, DescriptorSourceObservations, DescriptorStoredEffect,
    DescriptorStoredOrigin, ModuleSummaries, UnknownContentsKind, UnknownDescriptorContents,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorCallOrigin {
    Invocation {
        carrier_type: StructuralTypeId,
        projection: Projection,
    },
    AccessFormation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        place: PlaceId,
        carrier: StructuralTypeId,
        projection: Projection,
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

pub fn native_code_layout_regions(
    types: &[StructuralTypeDeclaration],
    result: &terminal_psi::StructuralOperationResult,
) -> Option<DescriptorLocation> {
    let StructuralTypeShape::Record { fields, .. } =
        &terminal_psi::find_structural_type(types, result.structural_type)?.shape
    else {
        return None;
    };
    if fields.len() != 2 {
        return None;
    }
    let terminal_psi::StructuralFieldType::Structural(view) = fields[1].field_type else {
        return None;
    };
    let StructuralTypeShape::ElementView { access, .. } =
        terminal_psi::find_structural_type(types, view)?.shape
    else {
        return None;
    };
    if access.is_some_and(|access| access != StructuralAccess::SharedBorrow) {
        return None;
    }
    Some(DescriptorLocation {
        root: result.place,
        path: vec![DescriptorPathSegment::Field(fields[1].id)],
    })
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorSourceOrigin {
    Provider(DescriptorProviderOrigin<DescriptorSourceOrigin>),
    Backing(DescriptorCallOrigin),
    IncomingSlot(DescriptorLocation),
    IncomingContents {
        query: DescriptorContentsQuery,
        projection: Projection,
    },
}

impl DescriptorSourceOrigin {
    pub fn captured_entry_slot(&self) -> Option<std::borrow::Cow<'_, DescriptorLocation>> {
        match self {
            Self::IncomingSlot(slot) => Some(std::borrow::Cow::Borrowed(slot)),
            Self::IncomingContents { query, projection }
                if *projection == Projection::new(&[]) =>
            {
                Some(std::borrow::Cow::Owned(query.exact_location()?))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorProviderOutput {
    /// An exact row in the boundary's `reference_outputs` table.
    Stored(u32),
    /// A canonical borrowed leaf in the declared result frontier.
    Result(Vec<StructuralPathSegment>),
}

impl DescriptorProviderOutput {
    pub fn stored_output(&self) -> Option<u32> {
        match self {
            Self::Stored(index) => Some(*index),
            Self::Result(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorProviderOrigin<Dependency> {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub output: DescriptorProviderOutput,
    pub invocations: Vec<DescriptorInvocationSite>,
    pub projection: Projection,
    pub lifetime_dependencies: Vec<DescriptorLifetimeDebt<Dependency>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorLifetimeDebt<Origin> {
    pub origin: Origin,
    pub access: StructuralAccess,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorInvocationSite {
    pub machine: MachineId,
    pub owner: DescriptorInvocationOwner,
}

pub use terminal_psi::InvocationOwner as DescriptorInvocationOwner;

pub fn is_descriptor<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    declarations.next().is_none() && declaration.shape.is_view_descriptor()
}

pub fn descriptor_value_access<'a>(
    types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
) -> Option<StructuralAccess> {
    let declaration = exactly_one(types.into_iter(), |item| item.id == structural_type)?;
    if multiplicity == StructuralMultiplicity::Linear {
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

pub(crate) fn is_shared_descriptor<'a>(
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

pub fn result_access<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    result: &terminal_psi::StructuralOperationResult,
) -> Option<StructuralAccess> {
    descriptor_value_access(types, result.structural_type, result.multiplicity)
        .filter(|access| matches!(access, terminal_psi::reading_borrow_pair_pattern!()))
}

pub(crate) type DescriptorReadQueries =
    BTreeMap<(DescriptorPosition, DescriptorLocation, DescriptorLocation), DescriptorContentsQuery>;

pub struct DescriptorMachine {
    pub id: MachineId,
    pub returns: Option<Vec<(DescriptorPosition, DescriptorLocation)>>,
    /// Sorted by place identity with no duplicate declarations.
    pub places: Vec<DescriptorPlace>,
    pub place_types: BTreeMap<PlaceId, StructuralTypeId>,
    pub entry: DescriptorPosition,
    pub flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    pub case_constructors: BTreeMap<DescriptorPosition, Vec<DescriptorCasePredicate>>,
    pub case_successors: BTreeMap<DescriptorPosition, Vec<(DescriptorCasePredicate, bool)>>,
    pub originations:
        BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, DescriptorCallOrigin)>>,
    pub projections:
        BTreeMap<(DescriptorPosition, DescriptorLocation, DescriptorLocation), Projection>,
    pub contents_queries: DescriptorReadQueries,
    pub contents_transfers: BTreeMap<DescriptorPosition, Vec<DescriptorContentsTransfer>>,
    pub payload_producers: BTreeMap<DescriptorPosition, PlaceId>,
    pub payload_entry_predicates: Option<(PropositionContext, Vec<Proposition>)>,
    /// Only literal definitions have rows. Their value IDs are sorted and
    /// unique within this machine. Their ordinals survive calls.
    pub unsigned_index_literals: Vec<(ValueId, u64)>,
    pub payload_scalar_definitions: BTreeMap<DescriptorPosition, Vec<ValueId>>,
    pub payload_scalar_postconditions: BTreeMap<DescriptorPosition, Vec<Proposition>>,
    pub payload_scalar_parameters: BTreeMap<semantic_vocabulary::BlockId, Vec<ValueId>>,
    pub scalar_payload_reads:
        BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, ScalarTerm)>>,
    pub payload_edge_predicates: BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
    pub payload_scalar_bindings: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
    pub scalar_payload_writes:
        BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, ScalarTerm)>>,
    pub payload_invalidations:
        BTreeMap<DescriptorPosition, Vec<Option<DescriptorContentsQuery>>>,
    /// Owned input occurrences whose concrete bytes and claims a closed boundary cannot reach.
    pub private_observation_inputs: PrivateObservationInputs,
    pub unresolved_contents_writes: BTreeSet<DescriptorPosition>,
    pub storage_derivations:
        BTreeSet<(DescriptorPosition, DescriptorLocation, DescriptorLocation)>,
    /// Missing is not a call; None is a real invocation whose effects
    /// cannot be represented. Known rows retain their exact ordered owners.
    pub calls: BTreeMap<DescriptorPosition, Option<Vec<DescriptorInvocation>>>,
    pub normal_exits: Vec<DescriptorPosition>,
}

pub fn insert_invocation(
    calls: &mut BTreeMap<DescriptorPosition, Option<Vec<DescriptorInvocation>>>,
    position: DescriptorPosition,
    call: DescriptorInvocation,
) -> Option<()> {
    let calls = calls
        .entry(position)
        .or_insert_with(|| Some(Vec::new()))
        .as_mut()?;
    if !calls.is_empty()
        && (matches!(
            call.owner,
            DescriptorInvocationOwner::Operation(_)
                | DescriptorInvocationOwner::InvokeNoNormal(_)
        ) || matches!(
            calls[0].owner,
            DescriptorInvocationOwner::Operation(_)
                | DescriptorInvocationOwner::InvokeNoNormal(_)
        ))
    {
        return None;
    }
    let insertion = calls
        .binary_search_by_key(&call.owner, |existing| existing.owner)
        .err()?;
    calls.insert(insertion, call);
    Some(())
}

impl DescriptorMachine {
    /// Both Psi and native operations can present blocks out of value order.
    /// A repeated definition cannot establish the ordinal of a captured index.
    pub fn finish_index_literals(mut self) -> Option<Self> {
        self.unsigned_index_literals
            .sort_unstable_by_key(|&(id, _)| id);
        self.unsigned_index_literals
            .windows(2)
            .all(|pair| pair[0].0 != pair[1].0)
            .then_some(self)
    }

    /// Resolve only indices captured from this machine's input storage. Other
    /// origins can carry coordinates formed in a different machine.
    pub(super) fn export_input_projection(&self, projection: &Projection) -> Projection {
        projection
            .clone()
            .without_runtime_coordinates_with(|index| {
                self.unsigned_index_literals
                    .binary_search_by_key(&index, |&(id, _)| id)
                    .ok()
                    .map(|row| self.unsigned_index_literals[row].1)
            }, |at| self.flow.contains_key(&at))
    }

    pub fn calls_at(&self, position: DescriptorPosition) -> &[DescriptorInvocation] {
        self.calls
            .get(&position)
            .and_then(Option::as_ref)
            .map_or(&[], Vec::as_slice)
    }

    /// One source call, including a call with no normal return. Cleanup
    /// actions have their own ordered owners and are queried separately.
    pub fn direct_call(&self, position: DescriptorPosition) -> Option<&DescriptorInvocation> {
        let [call] = self.calls_at(position) else {
            return None;
        };
        matches!(
            call.owner,
            DescriptorInvocationOwner::Operation(_)
                | DescriptorInvocationOwner::InvokeNoNormal(_)
        )
        .then_some(call)
    }

    pub fn invocation(
        &self,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
    ) -> Option<&DescriptorInvocation> {
        // Prepared replay validates the entire owner roster once before
        // publishing any state. Query its sorted rows without rewalking
        // every action, and retain local duplicate refusal for raw users.
        let calls = self.calls_at(position);
        let index = calls.binary_search_by_key(&owner, |call| call.owner).ok()?;
        if index
            .checked_sub(1)
            .and_then(|previous| calls.get(previous))
            .is_some_and(|call| call.owner == owner)
            || calls.get(index + 1).is_some_and(|call| call.owner == owner)
        {
            return None;
        }
        Some(&calls[index])
    }

    pub fn insert_call(
        &mut self,
        position: DescriptorPosition,
        call: DescriptorInvocation,
    ) -> Option<()> {
        insert_invocation(&mut self.calls, position, call)
    }

    pub(crate) fn validate_invocation_owners(&self) -> Option<()> {
        for (&position, calls) in &self.calls {
            let step = self.flow.get(&position)?;
            let Some(calls) = calls else {
                continue;
            };
            if calls.is_empty() || !calls.windows(2).all(|rows| rows[0].owner < rows[1].owner) {
                return None;
            }
            if matches!(
                calls[0].owner,
                DescriptorInvocationOwner::Operation(_)
                    | DescriptorInvocationOwner::InvokeNoNormal(_)
            ) {
                if calls.len() != 1 {
                    return None;
                }
                continue;
            }
            let mut exit_edge = None;
            let mut successor_edges = BTreeSet::new();
            for edge in &step.successors {
                if let Some(edge) = edge.edge
                    && !successor_edges.insert(edge)
                {
                    return None;
                }
            }
            for call in calls {
                let DescriptorInvocationOwner::Cleanup { edge, .. } = call.owner else {
                    return None;
                };
                if self.normal_exits.contains(&position) && step.successors.is_empty() {
                    if exit_edge.is_some_and(|selected| selected != edge) {
                        return None;
                    }
                    exit_edge = Some(edge);
                } else if !successor_edges.contains(&edge) {
                    return None;
                }
            }
        }
        Some(())
    }

    pub(crate) fn known_invocation(
        &self,
        position: &DescriptorPosition,
    ) -> Option<&DescriptorInvocation> {
        self.direct_call(*position)
    }

    pub fn known_invocations(
        &self,
    ) -> impl Iterator<Item = (&DescriptorPosition, &DescriptorInvocation)> {
        self.calls.iter().flat_map(|(position, calls)| {
            calls.iter().flatten().map(move |call| (position, call))
        })
    }

    pub(crate) fn producer(&self, root: PlaceId) -> Option<DescriptorPosition> {
        let index = self
            .places
            .binary_search_by_key(&root, |row| row.place)
            .ok()?;
        if index
            .checked_sub(1)
            .and_then(|index| self.places.get(index))
            .is_some_and(|row| row.place == root)
            || self
                .places
                .get(index + 1)
                .is_some_and(|row| row.place == root)
        {
            return None;
        }
        self.places[index].producer
    }

    pub(crate) fn place_definition(&self, place: PlaceId) -> Option<&DescriptorDefinition> {
        let index = self
            .places
            .binary_search_by_key(&place, |row| row.place)
            .ok()?;
        Some(&self.places[index].definition)
    }

    pub(crate) fn sequence_construction<'graph>(
        &'graph self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<(
        DescriptorPosition,
        StructuralTypeId,
        &'graph DescriptorFlowStep,
    )> {
        self.sequence_construction_with(
            |id| terminal_psi::find_structural_type(types, id),
            root,
            completed,
        )
    }

    pub(crate) fn sequence_construction_indexed<'graph, 'types>(
        &'graph self,
        types: &descriptor_lifetimes::locations::StructuralTypeIndex<'types>,
        root: PlaceId,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<(
        DescriptorPosition,
        StructuralTypeId,
        &'graph DescriptorFlowStep,
    )> {
        self.sequence_construction_with(|id| types.find(id), root, completed)
    }

    fn sequence_construction_with<'graph, 'types>(
        &'graph self,
        find_type: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
        root: PlaceId,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<(
        DescriptorPosition,
        StructuralTypeId,
        &'graph DescriptorFlowStep,
    )> {
        let ty = *self.place_types.get(&root)?;
        let StructuralTypeShape::ElementView {
            element,
            access: None | Some(StructuralAccess::SharedBorrow),
        } = find_type(ty)?.shape
        else {
            return None;
        };
        // The outer sequence read authenticates the constructor once.
        // Its inner ordinal reads already carry that completed cut, so
        // they can borrow the same roster by its retained coordinate.
        let captured = match completed {
            Some((owner, (block, index))) => {
                if owner != self.id {
                    return None;
                }
                let position = (block, index.checked_sub(1)?);
                (self.producer(root) == Some(position)).then_some(position)
            }
            None => None,
        };
        let position = if let Some(position) = captured {
            position
        } else {
            self.producer(root)?
        };
        let origin = DescriptorLocation::whole(root);
        let origins = self.originations.get(&position)?;
        if !origins.iter().any(|(location, value)| {
            *location == origin
                && matches!(value,
                DescriptorCallOrigin::Argument { source, projection }
                if *source == root && *projection == Projection::new(&[]))
        }) {
            return None;
        }
        let step = self.flow.get(&position)?;
        if step.inputs.iter().any(|(input, _)| input.is_none()) {
            return None;
        }
        // The adapter forms this ordered roster once from the actual
        // constructor operands. Reads borrow it rather than rebuilding or
        // rechecking every source element at every ordinal.
        Some((position, element, step))
    }

    fn resolve_location(
        &self,
        types: &[StructuralTypeDeclaration],
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
        descriptor_lifetimes::locations::resolved_path(
            types,
            *self.place_types.get(&location.root)?,
            &location.path,
            |prefix, ty, index| {
                self.sequence_element(
                    |id| terminal_psi::find_structural_type(types, id),
                    location,
                    completed,
                    (prefix, ty, index),
                )
            },
        )
    }

    /// The element type at `index` of a sequence the adapter constructed
    /// directly into `location.root`. Path resolvers use it to step through
    /// the ordinal reads of such a view.
    fn sequence_element<'types>(
        &self,
        find_type: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
        (prefix, ty, index): (&[DescriptorPathSegment], StructuralTypeId, u64),
    ) -> Option<StructuralTypeId> {
        if !prefix.is_empty() || self.place_types.get(&location.root) != Some(&ty) {
            return None;
        }
        let (_, element, step) =
            self.sequence_construction_with(find_type, location.root, completed)?;
        let (_, destination) = step.derivations.get(usize::try_from(index).ok()?)?;
        (destination.root == location.root
            && destination.path == [DescriptorPathSegment::FixedIndex(index)])
        .then_some(element)
    }

    pub(crate) fn location_capture(
        &self,
        types: &[StructuralTypeDeclaration],
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<(MachineId, DescriptorPosition)> {
        if !matches!(
            location.path.first(),
            Some(DescriptorPathSegment::FixedIndex(_))
        ) {
            return None;
        }
        self.resolve_location(types, location, completed)?;
        let (producer, _, _) = self.sequence_construction(types, location.root, completed)?;
        Some((self.id, (producer.0, producer.1.checked_add(1)?)))
    }

    pub(crate) fn location_type_indexed(
        &self,
        types: &descriptor_lifetimes::locations::StructuralTypeIndex<'_>,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<StructuralTypeId> {
        types.resolved_tip(
            *self.place_types.get(&location.root)?,
            &location.path,
            |prefix, ty, index| {
                self.sequence_element(
                    |id| types.find(id),
                    location,
                    completed,
                    (prefix, ty, index),
                )
            },
        )?
    }

    pub(crate) fn location_path(
        &self,
        types: &[StructuralTypeDeclaration],
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<Vec<StructuralPathSegment>> {
        Some(self.resolve_location(types, location, completed)?.0)
    }

    pub(crate) fn location_path_indexed(
        &self,
        types: &descriptor_lifetimes::locations::StructuralTypeIndex<'_>,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<Vec<StructuralPathSegment>> {
        Some(
            types
                .resolved_path(
                    *self.place_types.get(&location.root)?,
                    &location.path,
                    |prefix, ty, index| {
                        self.sequence_element(
                            |id| types.find(id),
                            location,
                            completed,
                            (prefix, ty, index),
                        )
                    },
                )?
                .0,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptorCasePredicate {
    pub subject: descriptor_lifetimes::DescriptorFlowSource,
    pub case: semantic_vocabulary::StructuralCaseId,
}

pub fn zeroed_case_predicates(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Vec<DescriptorCasePredicate> {
    let index = descriptor_lifetimes::locations::StructuralTypeIndex::new(types);
    let Some(selections) = crate::structural::zeroed_structural::zeroed_sum_selections_indexed(
        &index,
        structural_type,
    ) else {
        return Vec::new();
    };
    let mut predicates = Vec::with_capacity(selections.len());
    for (path, case) in selections {
        let Some(subject) = index.descriptor_location(root, structural_type, &path, None) else {
            return Vec::new();
        };
        predicates.push(DescriptorCasePredicate {
            subject: subject.into(),
            case,
        });
    }
    predicates
}

#[cfg(test)]
mod zeroed_case_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowFeasibility {
    ProvedImpossible,
    Possible,
    Unknown,
}

pub fn descriptor_machine(
    graph: &[DescriptorMachine],
    id: MachineId,
) -> Option<&DescriptorMachine> {
    graph.iter().find(|current| current.id == id)
}

#[derive(Clone, Copy)]
pub struct DescriptorActualInputs<'a> {
    pub scalar_arguments: &'a [(ValueId, semantic_vocabulary::ScalarTerm)],
    pub arguments: &'a [Option<DescriptorLocation>],
    pub argument_contents: &'a [Option<DescriptorContentsQuery>],
}

pub struct DescriptorInvocation {
    pub scalar_arguments: Vec<(ValueId, semantic_vocabulary::ScalarTerm)>,
    pub owner: DescriptorInvocationOwner,
    pub target: DescriptorInvocationTarget,
    pub arguments: Vec<Option<DescriptorLocation>>,
    pub argument_contents: Vec<Option<DescriptorContentsQuery>>,
    pub result: Option<DescriptorLocation>,
}

pub enum DescriptorInvocationTarget {
    Machine(MachineId),
    Provider {
        results: Vec<DescriptorResultSlot>,
        effects: Vec<DescriptorStoredEffect>,
        invalidates_contents: bool,
    },
    Public {
        effects_machine: Option<MachineId>,
        realizations: Vec<MachineId>,
        extent: semantic_vocabulary::ReferenceOriginExtent,
        results: Vec<DescriptorResultSlot>,
        contents: Vec<DescriptorResultSlot>,
        invalidates_contents: bool,
    },
}

impl DescriptorInvocation {
    /// A mutable reference into a single integer cell of the identical
    /// type occupies that whole cell. Its public lifetime still names the region.
    fn retain_whole_scalar_result_address(
        &mut self,
        module: &terminal_psi::TerminalModule,
        public: &crate::references::DeclaredReferenceResult<'_>,
    ) {
        let contract = public.contract();
        let DescriptorInvocationTarget::Public {
            results, extent, ..
        } = &mut self.target
        else {
            return;
        };
        let [slot] = results.as_mut_slice() else {
            return;
        };
        let [
            DescriptorStoredOrigin::Parameter {
                position,
                projection,
            },
        ] = slot.origins.as_mut_slice()
        else {
            return;
        };
        let Some(formal) = contract.structural_parameters.get(*position) else {
            return;
        };
        let result_shape = contract
            .result
            .structural()
            .and_then(|result| module.structural_type(result.structural_type))
            .map(|ty| &ty.shape);
        let Some(StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow,
        }) = result_shape
        else {
            return;
        };
        if !slot.path.is_empty()
            || formal.access != StructuralAccess::MutableBorrow
            || formal.structural_type != *referent
        {
            return;
        }
        let Some(StructuralTypeShape::PrimitiveScalar(
            semantic_vocabulary::ScalarType::Integer(integer),
        )) = module.structural_type(*referent).map(|ty| &ty.shape)
        else {
            return;
        };
        if integer.bits().is_some_and(|bits| bits > 0) {
            // Keep the supplied address during storage replay, and preserve
            // the public region when the result is forwarded or published.
            *projection = Projection::new(&[]);
            projection.exact = false;
            *extent = semantic_vocabulary::ReferenceOriginExtent::Projected;
        }
    }

    pub fn actual_inputs(&self) -> DescriptorActualInputs<'_> {
        DescriptorActualInputs {
            scalar_arguments: &self.scalar_arguments,
            arguments: &self.arguments,
            argument_contents: &self.argument_contents,
        }
    }

    pub(crate) fn stored_effects_callee(&self) -> Option<MachineId> {
        match self.target {
            DescriptorInvocationTarget::Public {
                effects_machine, ..
            } => effects_machine,
            _ => self.callee(),
        }
    }
    pub fn callee(&self) -> Option<MachineId> {
        match self.target {
            DescriptorInvocationTarget::Machine(machine) => Some(machine),
            DescriptorInvocationTarget::Public { .. }
            | DescriptorInvocationTarget::Provider { .. } => None,
        }
    }
}

#[derive(Clone)]
pub struct DescriptorResultSlot {
    pub path: Vec<DescriptorPathSegment>,
    pub origins: Vec<DescriptorStoredOrigin>,
}

impl DescriptorResultSlot {
    pub(crate) fn element_path(
        types: &[StructuralTypeDeclaration],
        account: (StructuralTypeId, &[Self]),
        path: &[DescriptorPathSegment],
    ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
        descriptor_lifetimes::locations::resolved_path(
            types,
            account.0,
            path,
            |prefix, view, ordinal| {
                let StructuralTypeShape::ElementView { element, .. } =
                    terminal_psi::find_structural_type(types, view)?.shape
                else {
                    return None;
                };
                let origins = Self::matching(account.1, prefix)?.origins_at(prefix)?;
                crate::descriptors::replay::DescriptorStoredOrigin::element_at(
                    &origins, element, ordinal,
                )?;
                Some(element)
            },
        )
    }

    pub(crate) fn canonicalize(slots: &mut [Self]) -> Option<()> {
        slots.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        if slots.windows(2).any(|rows| rows[0].path == rows[1].path) {
            return None;
        }
        for slot in slots {
            for origin in &mut slot.origins {
                if let DescriptorStoredOrigin::Carried { elements, .. } = origin {
                    for slots in elements {
                        Self::canonicalize(slots)?;
                    }
                }
            }
        }
        Some(())
    }

    pub(crate) fn matching<'slots>(
        slots: &'slots [Self],
        path: &[DescriptorPathSegment],
    ) -> Option<&'slots Self> {
        // Machine result accounts are ordered once by their owner. Query
        // the longest recorded prefix instead of rescanning every sibling.
        for length in (0..=path.len()).rev() {
            if let Ok(index) =
                slots.binary_search_by(|slot| slot.path.as_slice().cmp(&path[..length]))
            {
                return slots.get(index);
            }
        }
        None
    }

    pub(crate) fn origins_at(
        &self,
        path: &[DescriptorPathSegment],
    ) -> Option<Vec<&DescriptorStoredOrigin>> {
        let suffix = path.strip_prefix(self.path.as_slice())?;
        if suffix.is_empty() {
            return Some(self.origins.iter().collect());
        }
        let (DescriptorPathSegment::FixedIndex(ordinal), suffix) = suffix.split_first()? else {
            return None;
        };
        let mut origins = Vec::new();
        for origin in &self.origins {
            let DescriptorStoredOrigin::Carried { elements, .. } = origin else {
                return None;
            };
            let slots = elements.get(usize::try_from(*ordinal).ok()?)?;
            let slot = Self::matching(slots, suffix)?;
            origins.extend(slot.origins_at(suffix)?);
        }
        (!origins.is_empty()).then_some(origins)
    }

    pub(crate) fn merge(&mut self, other: Self) -> Option<()> {
        if self.path != other.path {
            return None;
        }
        self.origins.extend(other.origins);
        Some(())
    }

    pub(crate) fn visit<'a, Error>(
        &'a self,
        containing_type: StructuralTypeId,
        prefix: &mut Vec<DescriptorPathSegment>,
        visit: &mut impl FnMut(
            &[DescriptorPathSegment],
            StructuralTypeId,
            &'a Self,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let length = prefix.len();
        prefix.extend_from_slice(&self.path);
        let result = (|| {
            visit(prefix, containing_type, self)?;
            for origin in &self.origins {
                if let DescriptorStoredOrigin::Carried { element, elements } = origin {
                    for (ordinal, slots) in elements.iter().enumerate() {
                        prefix.push(DescriptorPathSegment::FixedIndex(ordinal as u64));
                        for slot in slots {
                            slot.visit(*element, prefix, visit)?;
                        }
                        prefix.pop();
                    }
                }
            }
            Ok(())
        })();
        prefix.truncate(length);
        result
    }
}

pub struct DescriptorPlace {
    /// The canonical declaration's actual producer, retained once while
    /// adapting its instruction. Roster queries need not rescan the body.
    pub producer: Option<DescriptorPosition>,
    pub place: PlaceId,
    pub definition: DescriptorDefinition,
}

pub enum DescriptorDefinition {
    Parameter {
        position: usize,
        access: StructuralAccess,
        structural_type: StructuralTypeId,
    },
    Arrival {
        access: StructuralAccess,
    },
    Local,
    Literal,
}

impl DescriptorDefinition {
    pub(crate) fn has_independent_storage(&self) -> bool {
        match self {
            Self::Parameter { access, .. } | Self::Arrival { access } => {
                *access == StructuralAccess::Owned
            }
            Self::Local | Self::Literal => true,
        }
    }
}

pub fn reconstruct_call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    replay::call_origins(types, machines, caller, result)
}

pub(crate) fn external_backing(
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

pub fn terminal_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    terminal_graph_for(module, DescriptorSlotKind::Views, &[]).unwrap_or_default()
}

pub fn terminal_borrowed_storage_graph_from(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
    terminal_graph_closure(
        module,
        roots,
        DescriptorSlotKind::BorrowedStorage,
        selected_calls,
    )
}

#[derive(Clone, Copy, Default)]
pub(crate) struct GenericCallInputs<'a> {
    pub nominal: &'a [terminal_psi::generic_calls::NominalCallType],
    pub scoped: Option<&'a arena::IdTable<MachineId, &'a [usize]>>,
}

pub fn terminal_borrowed_storage_graph_with_generic_calls(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    nominal: &[terminal_psi::generic_calls::NominalCallType],
    scoped: &arena::IdTable<MachineId, &[usize]>,
) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
    terminal_graph_rows_with_generic_calls(
        module,
        roots,
        DescriptorSlotKind::BorrowedStorage,
        selected_calls,
        GenericCallInputs { nominal, scoped: Some(scoped) },
    )
    .into_iter()
    .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorGraphError {
    MissingOrAmbiguousMachine(MachineId),
    IncompleteMachine {
        machine: MachineId,
        position: Option<DescriptorPosition>,
        successor: Option<(semantic_vocabulary::EdgeId, semantic_vocabulary::BlockId)>,
        detail: Option<&'static str>,
    },
}

pub(crate) const INEXACT_REFERENCE_BACKING: &str =
    "reference use without exact live backing correspondence";

impl DescriptorGraphError {
    /// Report a location only when the operation ID is unique and its
    /// instruction matches. Other failures retain only the machine.
    pub fn at_operation(
        module: &TerminalModule,
        machine: MachineId,
        operation: &Operation,
        detail: &'static str,
    ) -> Self {
        let position = exactly_one(module.machines.iter(), |row| row.id == machine)
            .and_then(|machine| {
                exactly_one(
                    machine.blocks.iter().flat_map(|block| {
                        block
                            .operations
                            .iter()
                            .enumerate()
                            .filter_map(move |(index, row)| {
                                (row.id == operation.id).then_some((block.id, index, row))
                            })
                    }),
                    |_| true,
                )
                .and_then(|(block, index, row)| (row == operation).then_some((block, index)))
            })
            .and_then(|(block, index)| u32::try_from(index).ok().map(|index| (block, index)));
        Self::IncompleteMachine {
            machine,
            position,
            successor: None,
            detail: Some(detail),
        }
    }

    pub const fn detail(self) -> Option<&'static str> {
        match self {
            Self::IncompleteMachine { detail, .. } => detail,
            Self::MissingOrAmbiguousMachine(_) => None,
        }
    }

    pub const fn machine(self) -> MachineId {
        match self {
            Self::MissingOrAmbiguousMachine(machine)
            | Self::IncompleteMachine { machine, .. } => machine,
        }
    }

    /// Resolve only an actual operation in the failed machine. Terminator
    /// failures retain their block and edge without becoming operations.
    pub fn operation(
        self,
        module: &TerminalModule,
    ) -> Option<semantic_vocabulary::OperationId> {
        self.operation_row(module).map(|operation| operation.id)
    }

    fn operation_row(self, module: &TerminalModule) -> Option<&Operation> {
        let Self::IncompleteMachine {
            machine,
            position: Some((block, index)),
            ..
        } = self
        else {
            return None;
        };
        let machine = exactly_one(module.machines.iter(), |row| row.id == machine)?;
        let block = exactly_one(machine.blocks.iter(), |row| row.id == block)?;
        block.operations.get(usize::try_from(index).ok()?)
    }

    /// A dependency may fail while another machine requests its closure.
    /// Keep that dependency's actual declaration in the diagnostic rather
    /// than blaming the first requested root or erasing the graph error.
    pub fn in_module(self, module: &TerminalModule) -> String {
        let name = exactly_one(module.machines.iter(), |machine| {
            machine.id == self.machine()
        })
        .and_then(|machine| machine.declaration.as_ref())
        .and_then(|declaration| module.identities.text(declaration.callable));
        let mut detail = self.to_string();
        if let Some(operation) = self.operation_row(module) {
            detail.push_str(&format!(
                "; checking {} ({:?})",
                crate::OperationSemanticTag::for_operation(&operation.kind).name(),
                operation.id
            ));
            if let Some(call) = operation.declared_invocation() {
                detail.push_str(&format!("; invocation target {:?}", call.callee));
                if let Some(required) = call.required_contract {
                    detail.push_str(&format!("; required contract {required:?}"));
                }
            }
        }
        if let Self::IncompleteMachine {
            position: Some((source, index)),
            ..
        } = self
            && let Some(machine) =
                exactly_one(module.machines.iter(), |row| row.id == self.machine())
            && let Some(block) = exactly_one(machine.blocks.iter(), |row| row.id == source)
            && usize::try_from(index).ok() == Some(block.operations.len())
        {
            let kind = match &block.terminator {
                Terminator::EliminateEmpty { .. } => "EliminateEmpty",
                Terminator::InvokeNoNormal { .. } => "InvokeNoNormal",
                Terminator::ExternalCompletion { .. } => "ExternalCompletion",
                Terminator::Jump { .. } => "Jump",
                Terminator::Conditional { .. } => "Conditional",
                Terminator::StructuralCase { .. } => "StructuralCase",
                Terminator::Return { .. } => "Return",
                Terminator::ReturnUnit { .. } => "ReturnUnit",
                Terminator::ReturnStructural { .. } => "ReturnStructural",
                Terminator::Crash { .. } => "Crash",
            };
            detail.push_str(&format!("; terminator {kind}"));
            if let Terminator::ReturnStructural { source, .. } = block.terminator {
                detail.push_str(&format!(
                    "; returned source {source:?} type {:?}; declared structural result {:?}",
                    machine.structural_place_type(source),
                    machine.result.structural(),
                ));
            }
        }
        if let Self::IncompleteMachine {
            position: Some((source, _)),
            successor: Some((identity, target)),
            ..
        } = self
            && let Some(machine) =
                exactly_one(module.machines.iter(), |row| row.id == self.machine())
        {
            if let Some(block) = exactly_one(machine.blocks.iter(), |row| row.id == source) {
                let mut scalar_arguments = None;
                let structural_arguments = match &block.terminator {
                    Terminator::Jump { edge, arguments, structural_arguments, .. }
                        if *edge == identity => {
                        scalar_arguments = Some(arguments.as_slice());
                        Some(structural_arguments.as_slice())
                    }
                    Terminator::Conditional { when_true, when_false, .. } => {
                        [when_true, when_false].into_iter()
                            .find(|row| row.edge == identity)
                            .map(|row| {
                                scalar_arguments = Some(row.arguments.as_slice());
                                row.structural_arguments.as_slice()
                            })
                    }
                    Terminator::StructuralCase { source, path, cases } => {
                        cases.iter().find(|row| row.edge == identity).map(|row| {
                            detail.push_str(&format!(
                                "; case payload actuals {:?} from {source:?} via {path:?} case {:?}",
                                row.payload_fields, row.case,
                            ));
                            row.structural_arguments.as_slice()
                        })
                    }
                    _ => None,
                };
                if let Some(arguments) = scalar_arguments {
                    let types = payload_predicates::TerminalScalarExpressions::new(machine);
                    let actual = arguments
                        .iter()
                        .map(|value| (*value, types.scalar_type(*value)))
                        .collect::<Vec<_>>();
                    detail.push_str(&format!("; scalar actuals {actual:?}"));
                }
                if let Some(arguments) = structural_arguments {
                    let actual = arguments
                        .iter()
                        .map(|argument| {
                            let root = machine.structural_place_type(argument.place);
                            let selected = root
                                .and_then(|root| {
                                    crate::runtime_structural_path_tip(
                                        module.types.structural_types.iter(),
                                        root,
                                        &argument.path,
                                    )
                                })
                                .map(|row| row.id);
                            (argument, selected)
                        })
                        .collect::<Vec<_>>();
                    detail.push_str(&format!("; structural actuals {actual:?}"));
                }
            }
            if let Some(target) = exactly_one(machine.blocks.iter(), |row| row.id == target) {
                detail.push_str(&format!(
                    "; target scalar parameters {:?}; target structural parameters {:?}",
                    target.parameters, target.structural_parameters,
                ));
            }
        }
        if let Some(name) = name {
            detail.push_str("; declaration ");
            detail.push_str(&readable_declaration_name(name));
        }
        detail
    }
}

/// Collapse an escaped canonical declaration identity string (which embeds
/// `package-owner(32:<hash>)` and nested backslash-escaped parameter lists)
/// into its readable `file.omg::member` path for diagnostics.
fn readable_declaration_name(raw: &str) -> String {
    let unescaped = raw.replace('\\', "");
    if let Some(rest) = unescaped.strip_prefix("named-callable(path(nominal(")
        && let Some((_, tail)) = rest.split_once(",path(")
    {
        if let Some(nested_marker) = tail.find("::nested-machine(") {
            let file = &tail[..nested_marker];
            let owner = tail[nested_marker..]
                .split(",path(")
                .nth(1)
                .and_then(|segment| segment.split(')').next());
            let leaf = tail
                .rsplit(",name(")
                .next()
                .and_then(|segment| segment.split(')').next());
            if let (Some(owner), Some(leaf)) = (owner, leaf) {
                let owner_name = owner.rsplit("::").next().unwrap_or(owner);
                return format!("{file}::{owner_name}::{leaf}");
            }
        }
        if let Some(path) = tail.split(')').next() {
            return path.to_owned();
        }
    }
    raw.to_owned()
}

impl std::fmt::Display for DescriptorGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOrAmbiguousMachine(machine) => write!(
                formatter,
                "descriptor storage closure has no unique machine {machine:?}"
            ),
            Self::IncompleteMachine {
                machine,
                position,
                successor,
                detail,
            } => {
                let detail =
                    detail.unwrap_or("descriptor storage flow cannot be reconstructed");
                write!(formatter, "{detail} for machine {machine:?}")?;
                if let Some((block, index)) = position {
                    write!(formatter, " at block {block:?}, position {index}")?;
                }
                if let Some((edge, target)) = successor {
                    write!(formatter, "; successor {edge:?} to block {target:?}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DescriptorGraphError {}

pub(crate) fn terminal_graph_closure(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
    terminal_graph_rows(module, roots, kind, selected_calls)
        .into_iter()
        .collect()
}

// Row adaptation reads the module's machines sorted by identity.
fn terminal_graph_machine<'a>(
    machines: &[&'a TerminalMachine],
    identity: MachineId,
) -> Option<&'a TerminalMachine> {
    let position = machines
        .binary_search_by_key(&identity, |machine| machine.id)
        .ok()?;
    Some(machines[position])
}

// An unsupported body must not erase useful analyses of independent
// bodies. Keep its failure in the same roster: complete-closure consumers
// still reject it, while optional summaries retain their unknown ceiling.
pub(crate) fn terminal_graph_rows(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
) -> Vec<Result<DescriptorMachine, DescriptorGraphError>> {
    terminal_graph_rows_with_generic_calls(
        module,
        roots,
        kind,
        selected_calls,
        GenericCallInputs::default(),
    )
}

fn terminal_graph_rows_with_generic_calls(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
    kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    generic_calls: GenericCallInputs<'_>,
) -> Vec<Result<DescriptorMachine, DescriptorGraphError>> {
    match crate::module_analyses::ModuleAnalyses::new(module) {
        Ok(analyses) => analyses.closure_rows(kind, roots, selected_calls, generic_calls),
        Err(error) => vec![Err(error)],
    }
}

pub fn terminal_reference_graph_from(
    module: &TerminalModule,
    roots: impl IntoIterator<Item = MachineId>,
) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
    terminal_graph_closure(module, roots, DescriptorSlotKind::References, &[])
}

pub fn terminal_reference_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
    terminal_graph_for(module, DescriptorSlotKind::References, &[]).unwrap_or_default()
}

pub(crate) fn terminal_residence_rules(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
) -> Option<descriptor_lifetimes::DescriptorResidenceRules> {
    terminal_flow::residence_rules(module, machine)
}

pub fn terminal_access_origin_graph(
    module: &TerminalModule,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
) -> Option<Vec<DescriptorMachine>> {
    terminal_graph_for(module, DescriptorSlotKind::AccessOrigins, selected_calls)
}

pub(crate) fn terminal_graph_for(
    module: &TerminalModule,
    kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
) -> Option<Vec<DescriptorMachine>> {
    crate::module_analyses::ModuleAnalyses::new(module)
        .ok()?
        .module_rows(kind, selected_calls)
}

pub(crate) fn terminal_definition(
    _module: &TerminalModule,
    machine: &TerminalMachine,
    place: PlaceId,
    kind: StructuralPlaceKind,
) -> Option<DescriptorDefinition> {
    match kind {
        StructuralPlaceKind::Parameter { position } => {
            let (position, parameter) = exactly_one(
                machine.structural_parameters.iter().enumerate(),
                |(_, parameter)| parameter.place == place && parameter.position == position,
            )?;
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
            Some(DescriptorDefinition::Arrival {
                access: block.structural_parameters.get(position as usize)?.access,
            })
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
