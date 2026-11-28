use super::*;
pub(crate) mod incoming_memberships;
use crate::descriptors::descriptor_lifetimes::{
    descriptor_location, descriptor_location_type, descriptor_path,
};

use super::payload_predicates::introduction_predicate;
pub use crate::descriptors::descriptor_lifetimes::{
    DescriptorContentsQuery, DescriptorContentsStep,
};
use semantic_vocabulary::ScalarRefinement;
use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::StructuralPathQualification;

pub struct DescriptorContentsTransfer {
    pub source: DescriptorContentsQuery,
    pub destination: DescriptorContentsQuery,
    pub removes_source: bool,
}

impl DescriptorFlowStep {
    /// A copy through an index carries the selected value's borrowed fields,
    /// including the opaque contents of a type parameter.
    pub fn record_selected_copy(
        &mut self,
        types: &[StructuralTypeDeclaration],
        kind: DescriptorSlotKind,
        position: DescriptorPosition,
        queries: &mut DescriptorReadQueries,
        (source, source_type): (Option<DescriptorContentsQuery>, StructuralTypeId),
        (destination, ty): (PlaceId, StructuralTypeId),
    ) {
        let known = source.and_then(|query| {
            if query.selected_type(types, source_type)? != Some(ty) {
                return None;
            }
            Some((query, kind.owned_inventory(types, destination, ty)?))
        });
        let Some((source, inventory)) = known else {
            self.may_escape = true;
            return;
        };
        let from = source.observation();
        for slot in inventory.into_observations() {
            // A copied value can outlive the selection only with the
            // authorities carried by its own concrete or opaque fields.
            self.may_escape = true;
            let mut query = source.clone();
            query.steps.extend(
                slot.path
                    .iter()
                    .copied()
                    .map(DescriptorContentsStep::Address),
            );
            self.derivations.push((from.clone(), slot.clone()));
            queries.insert((position, from.clone(), slot), query);
        }
    }
}

pub struct DescriptorContentsOperand<'a> {
    pub root: PlaceId,
    pub root_type: StructuralTypeId,
    pub path: &'a [StructuralPathSegment],
    pub field: Option<semantic_vocabulary::StructuralFieldId>,
}

pub fn descriptor_contents_transfers(
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    source: DescriptorContentsOperand<'_>,
    destination: DescriptorContentsOperand<'_>,
    value_type: StructuralTypeId,
    removes_source: bool,
) -> Option<Vec<DescriptorContentsTransfer>> {
    let inventory = slot_kind.owned_inventory(types, source.root, value_type)?;
    if inventory.slots.is_empty() && inventory.unknown_contents.is_empty() {
        return Some(Vec::new());
    }
    let source_type = source.root_type;
    let destination_type = destination.root_type;
    let source = descriptor_contents_query(
        types,
        source.root,
        source.root_type,
        source.path,
        source.field,
    )?;
    let destination = descriptor_contents_query(
        types,
        destination.root,
        destination.root_type,
        destination.path,
        destination.field,
    )?;
    if !inventory.unknown_contents.is_empty() {
        // The ordinary value derivation carries every descendant
        // packet. Require the same exact carrier at both locations;
        // an unknown payload cannot supply a descriptor-slot query
        // or justify a transfer through an unresolved address.
        let source_location = source.exact_location()?;
        let destination_location = destination.exact_location()?;
        if inventory.unknown_contents.iter().any(|packet| {
            packet.kind != super::UnknownContentsKind::Value
                || packet.location.path.last()
                    != Some(&DescriptorPathSegment::UnknownContents)
        }) || descriptor_location_type(types, source_type, &source_location.path)
            != Some(value_type)
            || descriptor_location_type(types, destination_type, &destination_location.path)
                != Some(value_type)
        {
            return None;
        }
    }
    let mut transfers = Vec::new();
    for slot in inventory.slots {
        let mut source = source.clone();
        let mut destination = destination.clone();
        let suffix = slot.path.into_iter().map(DescriptorContentsStep::Address);
        source.steps.extend(suffix.clone());
        destination.steps.extend(suffix);
        transfers.push(DescriptorContentsTransfer {
            source,
            destination,
            removes_source,
        });
    }
    Some(transfers)
}

#[cfg(test)]
#[test]
fn unknown_payload_transfers_require_the_same_exact_carrier() {
    let first = StructuralTypeId::new(1).unwrap();
    let second = StructuralTypeId::new(2).unwrap();
    let declaration = |id| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: terminal_psi::TerminalIdentityIndex::ABSENT,
        owns_linear_claim: true,
        no_code_affine_cleanup: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::TypeParameter,
        scalar_restriction: None,
    };
    let types = [declaration(first), declaration(second)];
    let operand = |root, root_type| DescriptorContentsOperand {
        root: PlaceId::new(root).unwrap(),
        root_type,
        path: &[],
        field: None,
    };
    assert!(
        descriptor_contents_transfers(
            &types,
            DescriptorSlotKind::BorrowedStorage,
            operand(1, first),
            operand(2, first),
            first,
            false
        )
        .is_some()
    );
    assert!(
        descriptor_contents_transfers(
            &types,
            DescriptorSlotKind::BorrowedStorage,
            operand(1, first),
            operand(2, second),
            first,
            false
        )
        .is_none()
    );
}

impl DescriptorContentsQuery {
    pub(crate) fn retains_selector(&self) -> bool {
        self.steps.iter().any(|step| {
            matches!(
                step,
                DescriptorContentsStep::Element {
                    selector: Some(_),
                    ..
                }
            )
        })
    }

    pub(crate) fn without_selectors(&self) -> Self {
        Self {
            root: self.root,
            steps: self
                .steps
                .iter()
                .map(|step| match step {
                    DescriptorContentsStep::Element { element, .. } => {
                        DescriptorContentsStep::any_element(*element)
                    }
                    step => step.clone(),
                })
                .collect(),
        }
    }

    pub(crate) fn is_symbolic_location(&self) -> bool {
        self.retains_selector()
            && self.steps.iter().all(|step| {
                !matches!(step, DescriptorContentsStep::Element { selector: None, .. })
            })
    }

    /// Whether each step stays inside the selected value's own
    /// storage. A reference, view backing or accessor crosses out.
    pub(crate) fn inline_steps(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<Vec<bool>> {
        let mut base = root_type;
        let mut path = Vec::new();
        let mut inline = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let container = || {
                let container = descriptor_location_type(types, base, &path)?;
                Some(&terminal_psi::find_structural_type(types, container)?.shape)
            };
            inline.push(match step {
                // Storage coordinates do not establish a scalar
                // value equation across a change of presentation.
                DescriptorContentsStep::Address(
                    DescriptorPathSegment::RepresentationView { .. }
                    | DescriptorPathSegment::RuntimeIndex { .. }
                    | DescriptorPathSegment::UnknownContents
                    | DescriptorPathSegment::Extent,
                ) => return None,
                DescriptorContentsStep::Address(DescriptorPathSegment::Referent) => false,
                DescriptorContentsStep::Address(DescriptorPathSegment::Case(_)) => true,
                DescriptorContentsStep::Address(DescriptorPathSegment::Field(_)) => {
                    matches!(path.last(), Some(DescriptorPathSegment::Case(_)))
                        || container().is_some_and(|shape| match shape {
                            StructuralTypeShape::Record { storage, .. } => !matches!(
                                storage,
                                terminal_psi::StructuralRecordStorage::AccessView(_)
                                    | terminal_psi::StructuralRecordStorage::Accessor(_)
                            ),
                            StructuralTypeShape::Mixed { .. } => true,
                            _ => false,
                        })
                }
                DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(_))
                | DescriptorContentsStep::Element { .. } => {
                    container().is_some_and(|shape| {
                        matches!(shape, StructuralTypeShape::FixedArray { .. })
                    })
                }
            });
            match step {
                DescriptorContentsStep::Address(segment) => path.push(*segment),
                DescriptorContentsStep::Element { element, .. } => {
                    base = *element;
                    path.clear();
                }
            }
        }
        Some(inline)
    }

    pub fn exact_location(&self) -> Option<DescriptorLocation> {
        Some(DescriptorLocation {
            root: self.root,
            path: self
                .steps
                .iter()
                .map(|step| match step {
                    DescriptorContentsStep::Address(segment) => Some(*segment),
                    DescriptorContentsStep::Element { .. } => None,
                })
                .collect::<Option<_>>()?,
        })
    }

    pub fn from_location(location: &DescriptorLocation) -> Self {
        Self {
            root: location.root,
            steps: location
                .path
                .iter()
                .cloned()
                .map(DescriptorContentsStep::Address)
                .collect(),
        }
    }

    pub fn observation(&self) -> DescriptorLocation {
        DescriptorLocation {
            root: self.root,
            path: self
                .steps
                .iter()
                .map_while(|step| match step {
                    DescriptorContentsStep::Address(segment) => Some(*segment),
                    DescriptorContentsStep::Element { .. } => None,
                })
                .collect(),
        }
    }

    pub(crate) fn selected_type(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<Option<StructuralTypeId>> {
        self.selected_type_using(types, root_type, |carrier, path, _| {
            descriptor_path(types, carrier, path)?;
            Some(descriptor_location_type(types, carrier, path))
        })
    }

    pub fn validate(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<()> {
        self.selected_type(types, root_type).map(|_| ())
    }

    /// `location_type` answers `Some` for a path that resolves from
    /// its carrier, holding the structural type at the path's end
    /// when the end has one. A leaf need not: a byte view held in a
    /// field resolves and has no declaration of its own. A query may
    /// end at such a leaf and may not step through it.
    pub(crate) fn validate_using(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        location_type: impl FnMut(
            StructuralTypeId,
            &[DescriptorPathSegment],
            bool,
        ) -> Option<Option<StructuralTypeId>>,
    ) -> Option<()> {
        self.selected_type_using(types, root_type, location_type)
            .map(|_| ())
    }

    pub(crate) fn selected_type_using(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        mut location_type: impl FnMut(
            StructuralTypeId,
            &[DescriptorPathSegment],
            bool,
        ) -> Option<Option<StructuralTypeId>>,
    ) -> Option<Option<StructuralTypeId>> {
        let mut tip = root_type;
        let mut original = true;
        let mut path = Vec::new();
        for step in &self.steps {
            match step {
                DescriptorContentsStep::Address(segment) => path.push(*segment),
                DescriptorContentsStep::Element { element, .. } => {
                    tip = location_type(tip, &path, original)??;
                    let declaration = terminal_psi::find_structural_type(types, tip)?;
                    match declaration.shape {
                        StructuralTypeShape::FixedArray {
                            element: actual, ..
                        }
                        | StructuralTypeShape::ElementView {
                            element: actual, ..
                        } if actual == *element => {}
                        _ => return None,
                    }
                    tip = *element;
                    original = false;
                    path.clear();
                }
            }
        }
        if path.last() == Some(&DescriptorPathSegment::UnknownContents) {
            path.pop();
            let carrier = location_type(tip, &path, original)??;
            return matches!(
                terminal_psi::find_structural_type(types, carrier)?.shape,
                StructuralTypeShape::TypeParameter
            )
            .then_some(Some(carrier));
        }
        location_type(tip, &path, original)
    }

    /// Append a checked inline projection using the selected value's
    /// own field declarations. Unknown addresses, windows and past
    /// captures cannot become an exact memory-access query here.
    pub(crate) fn projected(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        projection: &Projection,
    ) -> Option<Self> {
        if !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return None;
        }
        if projection.path.is_empty() {
            self.validate(types, root_type)?;
            return Some(self.clone());
        }
        let selected =
            self.selected_type_using(types, root_type, |carrier, path, _| {
                descriptor_path(types, carrier, path)?;
                Some(descriptor_location_type(types, carrier, path))
            })??;
        let suffix =
            descriptor_contents_query(types, self.root, selected, &projection.path, None)?;
        let mut query = self.clone();
        query.steps.extend(suffix.steps);
        query.validate(types, root_type)?;
        Some(query)
    }

    pub(crate) fn conservative_projection(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<Projection> {
        let prefix = self.observation();
        let mut projection =
            Projection::new(&descriptor_path(types, root_type, &prefix.path)?);
        projection.exact = false;
        projection.indirect = true;
        Some(projection.without_captured_address())
    }
}

pub fn descriptor_contents_query(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<DescriptorContentsQuery> {
    descriptor_contents_query_with(types, root, root_type, path, final_field, |_| None)
}

pub(crate) fn descriptor_storage_use_source(
    types: &[StructuralTypeDeclaration],
    root_type: Option<StructuralTypeId>,
    operation: &terminal_psi::OperationKind,
    used: terminal_psi::StructuralStorageUse<'_>,
    select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
) -> crate::descriptor_lifetimes::DescriptorFlowSource {
    use crate::descriptor_lifetimes::DescriptorFlowSource;
    use terminal_psi::StructuralStorageUse;

    let (root, query) = match used {
        StructuralStorageUse::Place(root) => (
            root,
            Some(DescriptorContentsQuery::from_location(
                &DescriptorLocation::whole(root),
            )),
        ),
        StructuralStorageUse::Projection(source) => (
            source.root,
            root_type.and_then(|ty| {
                descriptor_contents_query_with(
                    types,
                    source.root,
                    ty,
                    source.path,
                    source.field,
                    &select,
                )
            }),
        ),
        StructuralStorageUse::Canonical { root, path } => (
            root,
            root_type.and_then(|ty| {
                let location = crate::descriptor_lifetimes::canonical_descriptor_location(
                    types, root, ty, path,
                )?;
                Some(DescriptorContentsQuery::from_location(&location))
            }),
        ),
        StructuralStorageUse::ObservedField { root, path, field } => (
            root,
            root_type.and_then(|ty| {
                let mut selected = path.to_vec();
                selected.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    field,
                ));
                let location = crate::descriptor_lifetimes::canonical_descriptor_location(
                    types, root, ty, &selected,
                )?;
                Some(DescriptorContentsQuery::from_location(&location))
            }),
        ),
        StructuralStorageUse::Element { descriptor, path } => {
            let query = (|| {
                let (index, obligation) = match operation {
                    terminal_psi::OperationKind::ElementViewRead {
                        index,
                        obligation,
                        ..
                    }
                    | terminal_psi::OperationKind::ElementViewWrite {
                        index,
                        obligation,
                        ..
                    } => (*index, *obligation),
                    _ => return None,
                };
                let mut selected = descriptor.path.to_vec();
                if let Some(field) = descriptor.field {
                    let field = crate::projected_field_declaration(
                        types.iter(),
                        root_type?,
                        descriptor.path,
                        field,
                    )?;
                    selected.push(StructuralPathSegment::Field(field.identity));
                }
                selected.push(StructuralPathSegment::RuntimeIndex { index, obligation });
                selected.extend_from_slice(path);
                descriptor_contents_query_with(
                    types,
                    descriptor.root,
                    root_type?,
                    &selected,
                    None,
                    &select,
                )
            })();
            (descriptor.root, query)
        }
        StructuralStorageUse::Type(_) => unreachable!("a type is not an operation operand"),
    };
    // A byte descriptor's address does not identify the bytes an
    // instruction observes. Until its index/window and backing are
    // represented here, neither a header nor a payload query can be
    // used as an exact byte access. Ordinary liveness is unchanged.
    let query = query.filter(|query| {
        let Some(root_type) = root_type else {
            return false;
        };
        query
            .selected_type_using(types, root_type, |carrier, path, _| {
                let source_path = descriptor_path(types, carrier, path)?;
                let selected = descriptor_location_type(types, carrier, path);
                if selected.is_none()
                    && crate::primitive_read_projection_type(
                        types.iter(),
                        carrier,
                        &source_path,
                    )
                    .is_none()
                {
                    return None;
                }
                Some(selected)
            })
            .is_some_and(|selected| {
                selected.is_none_or(|selected| {
                    terminal_psi::find_structural_type(types, selected).is_some_and(
                        |declaration| {
                            !matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(_)
                            )
                        },
                    )
                })
            })
    });
    DescriptorFlowSource::from_query(root, query)
}

/// Capture a typed storage query and its already-selected index terms.
/// An unknown index remains an element query rather than a fixed cell.
pub fn descriptor_contents_query_with(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<semantic_vocabulary::StructuralFieldId>,
    select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
) -> Option<DescriptorContentsQuery> {
    let mut query = DescriptorContentsQuery {
        root,
        steps: Vec::new(),
    };
    let mut tip = root_type;
    let mut static_path = Vec::new();
    for segment in path {
        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
            let prefix = descriptor_location(types, root, tip, &static_path, None)?;
            tip = descriptor_location_type(types, tip, &prefix.path)?;
            query
                .steps
                .extend(prefix.path.into_iter().map(DescriptorContentsStep::Address));
            let declaration = terminal_psi::find_structural_type(types, tip)?;
            let element = match declaration.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => element,
                _ => return None,
            };
            let selector = select(*index);
            let fixed = inline_selected_index(&declaration.shape, selector.as_ref());
            query.steps.push(match fixed {
                Some(index) => DescriptorContentsStep::Address(
                    DescriptorPathSegment::FixedIndex(index),
                ),
                None => DescriptorContentsStep::Element {
                    element,
                    selector: selector.map(Box::new),
                },
            });
            tip = element;
            static_path.clear();
        } else {
            static_path.push(segment.clone());
        }
    }
    let suffix = descriptor_location(types, root, tip, &static_path, final_field)?;
    query
        .steps
        .extend(suffix.path.into_iter().map(DescriptorContentsStep::Address));
    query.validate(types, root_type)?;
    Some(query)
}

pub(crate) fn inline_selected_index(
    shape: &StructuralTypeShape,
    selector: Option<&semantic_vocabulary::ScalarTerm>,
) -> Option<u64> {
    let StructuralTypeShape::FixedArray {
        length: terminal_psi::ArrayExtent::Literal(extent),
        ..
    } = shape
    else {
        return None;
    };
    crate::runtime_index_selects(selector?.integer_value()?.1, *extent)
}

// Effects cross machine boundaries only in terms of stable scalar
// inputs. A read of local storage or a renewed block argument cannot
// accompany the descriptor into the caller.
pub fn entry_selector(
    term: semantic_vocabulary::ScalarTerm,
    is_parameter: impl Fn(semantic_vocabulary::ValueId) -> bool,
) -> Option<semantic_vocabulary::ScalarTerm> {
    use semantic_vocabulary::ScalarTerm;
    let mut pending = vec![&term];
    while let Some(value) = pending.pop() {
        match value {
            ScalarTerm::Value { id, .. } if is_parameter(*id) => (),
            ScalarTerm::Value { .. }
            | ScalarTerm::Application { .. }
            | ScalarTerm::StructuralRead { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::IeeeFloatField { .. }
            | ScalarTerm::ViewExtent { .. } => return None,
            _ => pending.extend(value.operands()),
        }
    }
    Some(term)
}

pub fn descriptor_scalar_arguments(
    formals: impl ExactSizeIterator<Item = (ValueId, semantic_vocabulary::ScalarType)>,
    actuals: &[ValueId],
    expression: impl Fn(ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
) -> Vec<(ValueId, semantic_vocabulary::ScalarTerm)> {
    if formals.len() != actuals.len() {
        return Vec::new();
    }
    let mut bindings = formals
        .zip(actuals)
        .filter_map(|((formal, scalar_type), actual)| {
            let term = expression(*actual)?;
            (term.scalar_type() == scalar_type).then_some((formal, term))
        })
        .collect::<Vec<_>>();
    bindings.sort_unstable_by_key(|(formal, _)| *formal);
    if bindings.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Vec::new();
    }
    bindings
}

pub struct DescriptorStoreFlow<'a> {
    pub position: crate::descriptor_lifetimes::DescriptorPosition,
    pub transfers: &'a mut std::collections::BTreeMap<
        crate::descriptor_lifetimes::DescriptorPosition,
        Vec<DescriptorContentsTransfer>,
    >,
    pub unresolved:
        &'a mut std::collections::BTreeSet<crate::descriptor_lifetimes::DescriptorPosition>,
}

pub fn descriptor_leaf_store(
    DescriptorStoreFlow {
        position,
        transfers,
        unresolved,
    }: DescriptorStoreFlow<'_>,
    step: &mut crate::descriptor_lifetimes::DescriptorFlowStep,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    destination: DescriptorContentsOperand<'_>,
    source: PlaceId,
    select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
) -> Option<()> {
    let from = DescriptorLocation::whole(source);
    let query = descriptor_contents_query_with(
        types,
        destination.root,
        destination.root_type,
        destination.path,
        destination.field,
        select,
    );
    let location = query
        .as_ref()
        .and_then(DescriptorContentsQuery::exact_location)
        .or_else(|| {
            descriptor_location(
                types,
                destination.root,
                destination.root_type,
                destination.path,
                destination.field,
            )
        });
    if let Some(location) = location {
        step.definitions.push(location.clone());
        step.derivations.push((from.clone(), location));
    } else {
        step.may_escape = true;
        if let Some(query) = query.filter(|query| {
            query.retains_selector()
                && query
                    .inline_steps(types, destination.root_type)
                    .is_some_and(|steps| steps.iter().all(|inline| *inline))
        }) {
            transfers
                .entry(position)
                .or_default()
                .push(DescriptorContentsTransfer {
                    source: DescriptorContentsQuery::from_location(&from),
                    destination: query,
                    removes_source: false,
                });
        } else {
            unresolved.insert(position);
            step.definitions.extend(slot_kind.overwritten_slots(
                types,
                destination.root,
                destination.root_type,
                destination.path,
                destination.field,
            )?);
        }
    }
    step.replace_value_inputs([from]);
    Some(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorPayloadTarget {
    Storage {
        location: DescriptorLocation,
        /// An immutable constructor keeps the value captured at its
        /// completed cut. This is not permission to write its current
        /// descriptor or to borrow a caller-selected result source.
        completed: Option<(MachineId, DescriptorPosition)>,
    },
    Incoming(DescriptorContentsQuery),
}

impl DescriptorPayloadTarget {
    pub fn current(location: DescriptorLocation) -> Self {
        Self::Storage {
            location,
            completed: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadMembershipContract {
    pub scalar_observations: Vec<super::PayloadScalarObservation>,
    pub scalar_requirements: Vec<semantic_vocabulary::Proposition>,
    pub carrier: StructuralTypeId,
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

impl PayloadMembershipContract {
    /// A move transports the selected place's declared qualifications. Predicate
    /// facts available while checking a transfer do not add displayed domains.
    pub fn move_preserves_qualifications(
        &self,
        path: &[terminal_psi::StructuralPathSegment],
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        let (whole, projected) = StructuralPathQualification::select(
            &self.qualifications,
            &self.projected_qualifications,
            path,
        );
        whole
            .iter()
            .all(|domain| result.qualifications.contains(domain))
            && projected
                .iter()
                .all(|row| result.projected_qualifications.contains(row))
    }

    pub fn presented(
        mut self,
        types: &[StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        destination: StructuralTypeId,
    ) -> Option<Self> {
        if crate::borrowed_referent_presentation(types.iter(), self.carrier, destination)
            != Some(crate::BorrowedReferentPresentation::Unchanged)
        {
            return None;
        }
        crate::present_structural_memberships(
            types.iter(),
            domains.iter(),
            destination,
            &mut self.qualifications,
            &mut self.projected_qualifications,
        )
        .ok()?;
        self.carrier = destination;
        Some(self)
    }
}

pub(crate) fn select_payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &PayloadMembershipContract,
    address: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) -> Option<PayloadMembershipContract> {
    let path = descriptor_path(types, contract.carrier, address)?;
    let carrier = crate::qualification_path_carrier(types.iter(), contract.carrier, &path)?;
    let (mut qualifications, projected_qualifications) =
        StructuralPathQualification::select(
            &contract.qualifications,
            &contract.projected_qualifications,
            &path,
        );
    if let Some(predicates) =
        crate::shared_byte_field_copy_predicates(types.iter(), contract.carrier, &path)
    {
        qualifications.extend(byte_predicate_memberships(domains, carrier, predicates));
        qualifications.sort();
        qualifications.dedup();
    }
    let mut selected = PayloadMembershipContract {
        scalar_requirements: contract.scalar_requirements.clone(),
        scalar_observations: contract
            .scalar_observations
            .iter()
            .filter_map(|observation| {
                Some(super::PayloadScalarObservation {
                    path: observation.path.strip_prefix(address)?.to_vec(),
                    value: observation.value.clone(),
                    entry: observation.entry.clone(),
                })
            })
            .collect(),
        carrier,
        qualifications,
        projected_qualifications,
    };
    super::payload_predicates::close(types, domains, &mut selected);
    Some(selected)
}

pub(crate) fn payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    targets: impl IntoIterator<Item = DescriptorPayloadTarget>,
    mut source: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
) -> Option<PayloadMembershipContract> {
    let mut common: Option<PayloadMembershipContract> = None;
    for target in targets {
        let mut selected = match target {
            DescriptorPayloadTarget::Storage {
                location,
                completed: None,
            } => select_payload_memberships(
                types,
                domains,
                &source(location.root)?,
                &location.path,
            )?,
            DescriptorPayloadTarget::Storage {
                completed: Some(_), ..
            } => return None,
            DescriptorPayloadTarget::Incoming(query) => {
                let contract = source(query.root)?;
                query.validate(types, contract.carrier)?;
                incoming_memberships::select(types, domains, contract, &query.steps)?
            }
        };
        let carrier = selected.carrier;
        close_predicate_memberships(types, domains, &mut selected);
        if let Some(common) = &mut common {
            if common.carrier != carrier {
                return None;
            }
            common
                .scalar_requirements
                .retain(|premise| selected.scalar_requirements.contains(premise));
            super::PayloadScalarObservation::intersect(
                &mut common.scalar_observations,
                &selected.scalar_observations,
            );
            common
                .qualifications
                .retain(|domain| selected.qualifications.contains(domain));
            common
                .projected_qualifications
                .retain(|row| selected.projected_qualifications.contains(row));
        } else {
            common = Some(selected);
        }
    }
    common
}

pub(crate) fn contents_membership_path(
    types: &[StructuralTypeDeclaration],
    mut carrier: StructuralTypeId,
    steps: &[DescriptorContentsStep],
) -> Option<Vec<StructuralPathSegment>> {
    let mut path = Vec::new();
    let mut address = Vec::new();
    for step in steps {
        match step {
            DescriptorContentsStep::Address(segment) => address.push(*segment),
            DescriptorContentsStep::Element { element, .. } => {
                let prefix = descriptor_path(types, carrier, &address)?;
                path.extend(prefix.iter().cloned());
                carrier =
                    crate::qualification_path_carrier(types.iter(), carrier, &prefix)?;
                match terminal_psi::find_structural_type(types, carrier)?.shape {
                    StructuralTypeShape::FixedArray {
                        element: actual, ..
                    }
                    | StructuralTypeShape::ElementView {
                        element: actual, ..
                    } if actual == *element => {}
                    _ => return None,
                }
                path.push(StructuralPathSegment::AllElements);
                carrier = *element;
                address.clear();
            }
        }
    }
    path.extend(descriptor_path(types, carrier, &address)?);
    Some(path)
}

fn close_predicate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
    super::payload_predicates::close(types, domains, contract);
    admit_declared_scalar_refinement(types, domains, contract);
    contract
        .qualifications
        .extend(implied_predicate_memberships(
            types,
            domains,
            contract.carrier,
            &contract.qualifications,
        ));
    contract.qualifications.sort();
    contract.qualifications.dedup();
    let mut paths: Vec<_> = contract
        .projected_qualifications
        .iter()
        .map(|row| row.path.clone())
        .collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let Some(carrier) =
            crate::qualification_path_carrier(types.iter(), contract.carrier, &path)
        else {
            continue;
        };
        let whole: Vec<_> = contract
            .projected_qualifications
            .iter()
            .filter(|row| row.path == path)
            .map(|row| row.domain)
            .collect();
        for domain in implied_predicate_memberships(types, domains, carrier, &whole) {
            contract
                .projected_qualifications
                .push(StructuralPathQualification {
                    path: path.clone(),
                    domain,
                });
        }
    }
    contract.projected_qualifications.sort();
    contract.projected_qualifications.dedup();
}

fn admit_declared_scalar_refinement(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
    let Some(refinement) = types
        .iter()
        .find(|row| row.id == contract.carrier)
        .and_then(|row| match &row.scalar_restriction {
            Some(terminal_psi::StructuralFieldType::RefinedScalar(refinement)) => {
                Some(refinement)
            }
            _ => None,
        })
    else {
        return;
    };
    for domain in domains {
        if domain.carrier != contract.carrier
            || domain.content_projection.is_some()
            || domain.establishment_routes.is_some()
            || contract.qualifications.contains(&domain.id)
        {
            continue;
        }
        let Some(predicate) =
            introduction_predicate(types, domains, domain, &contract.qualifications)
        else {
            continue;
        };
        let mut proposition = predicate.proposition.clone();
        rewrite_subject_as_refinement_value(
            &mut proposition,
            predicate.subject,
            refinement.carrier(),
        );
        normalize_boolean_comparison(&mut proposition);
        if proposition.same_commutative_predicate(refinement.predicate()) {
            contract.qualifications.push(domain.id);
        }
    }
}

fn rewrite_subject_as_refinement_value(
    proposition: &mut semantic_vocabulary::Proposition,
    subject: semantic_vocabulary::PlaceId,
    carrier: ScalarType,
) {
    fn rewrite_term(
        term: &mut semantic_vocabulary::ScalarTerm,
        subject: semantic_vocabulary::PlaceId,
        carrier: ScalarType,
    ) {
        use semantic_vocabulary::ScalarTerm;
        let replacement = match term {
            ScalarTerm::IntegerField {
                version: semantic_vocabulary::ContentPlaceVersion::Current,
                root,
                path,
                scalar_type,
            } if *root == subject
                && path.is_empty()
                && ScalarType::Integer(*scalar_type) == carrier =>
            {
                Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
            }
            ScalarTerm::BooleanField {
                version: semantic_vocabulary::ContentPlaceVersion::Current,
                root,
                path,
            } if *root == subject && path.is_empty() && carrier == ScalarType::Boolean => {
                Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
            }
            ScalarTerm::IeeeFloatField {
                version: semantic_vocabulary::ContentPlaceVersion::Current,
                root,
                path,
                format,
            } if *root == subject
                && path.is_empty()
                && carrier == ScalarType::IeeeFloat(*format) =>
            {
                Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
            }
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } if *root == subject && path.is_empty() && *scalar_type == carrier => {
                Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            *term = replacement;
            return;
        }
        for operand in term.operands_mut() {
            rewrite_term(operand, subject, carrier);
        }
    }
    fn rewrite_math(
        term: &mut semantic_vocabulary::IntegerMathTerm,
        subject: semantic_vocabulary::PlaceId,
        carrier: ScalarType,
    ) {
        if let semantic_vocabulary::IntegerMathTerm::Embed(inner) = term {
            rewrite_term(inner, subject, carrier);
        }
        for operand in term.operands_mut() {
            rewrite_math(operand, subject, carrier);
        }
    }
    fn rewrite_proposition(
        proposition: &mut semantic_vocabulary::Proposition,
        subject: semantic_vocabulary::PlaceId,
        carrier: ScalarType,
    ) {
        use semantic_vocabulary::Proposition;
        match proposition {
            semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                rewrite_term(left, subject, carrier);
                rewrite_term(right, subject, carrier);
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                rewrite_math(left, subject, carrier);
                rewrite_math(right, subject, carrier);
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    rewrite_proposition(child, subject, carrier);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                rewrite_proposition(premise, subject, carrier);
                rewrite_proposition(conclusion, subject, carrier);
            }
            _ => {}
        }
    }
    rewrite_proposition(proposition, subject, carrier);
}

fn normalize_boolean_comparison(proposition: &mut semantic_vocabulary::Proposition) {
    use semantic_vocabulary::{Proposition, ScalarTerm};
    match proposition {
        Proposition::Equal(left, right) => {
            let comparison = match (&*left, &*right) {
                (ScalarTerm::Boolean(true), comparison)
                | (comparison, ScalarTerm::Boolean(true)) => comparison.clone(),
                _ => return,
            };
            let Some(normalized) = (match &comparison {
                ScalarTerm::IntegerEqual { left, right, .. } => Some(Proposition::Equal(
                    left.as_ref().clone(),
                    right.as_ref().clone(),
                )),
                ScalarTerm::IntegerLessThan { left, right, .. } => Some(
                    Proposition::LessThan(left.as_ref().clone(), right.as_ref().clone()),
                ),
                ScalarTerm::IntegerLessOrEqual { left, right, .. } => Some(
                    Proposition::LessOrEqual(left.as_ref().clone(), right.as_ref().clone()),
                ),
                _ => None,
            }) else {
                return;
            };
            *proposition = normalized;
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            for child in children {
                normalize_boolean_comparison(child);
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            normalize_boolean_comparison(premise);
            normalize_boolean_comparison(conclusion);
        }
        _ => {}
    }
}

fn implied_predicate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    carrier: StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
) -> Vec<semantic_vocabulary::StructuralDomainId> {
    let mut implied = domains
        .iter()
        .filter(|domain| {
            domain.carrier == carrier
                && (domain
                    .missing_membership_requirements(domains, qualifications)
                    .is_some_and(|missing| missing.is_empty())
                    || introduction_predicate(types, domains, domain, qualifications)
                        .is_some_and(|predicate| {
                            predicate.proposition == semantic_vocabulary::Proposition::Truth
                        }))
        })
        .map(|domain| domain.id)
        .collect::<Vec<_>>();
    if !crate::byte_predicate_carrier(types.iter(), carrier) {
        return implied;
    }
    let mut supplied = terminal_psi::ByteSequencePredicateSet::EMPTY;
    for domain in qualifications {
        if let Some(predicates) =
            crate::byte_qualification_predicates_in(domains.iter(), carrier, &[*domain])
        {
            for predicate in predicates.predicates() {
                supplied.insert(predicate);
            }
        }
    }
    implied.extend(byte_predicate_memberships(domains, carrier, supplied));
    implied.sort();
    implied.dedup();
    implied
}

fn byte_predicate_memberships(
    domains: &[terminal_psi::StructuralDomainDeclaration],
    carrier: StructuralTypeId,
    supplied: terminal_psi::ByteSequencePredicateSet,
) -> Vec<semantic_vocabulary::StructuralDomainId> {
    domains
        .iter()
        .filter(|domain| {
            crate::byte_qualification_predicates_in(domains.iter(), carrier, &[domain.id])
                .is_some_and(|required| supplied.implies(required))
        })
        .map(|domain| domain.id)
        .collect()
}

pub fn terminal_payload_contract(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<PayloadMembershipContract> {
    if let Some(parameter) = crate::arrival_parameter(machine, place) {
        return Some(PayloadMembershipContract {
            scalar_observations: Vec::new(),
            scalar_requirements: Vec::new(),
            carrier: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
        });
    }
    for operation in machine.operations() {
        if let Some(result) = operation.result.structural()
            && result.place == place
        {
            return Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: result.structural_type,
                qualifications: result.qualifications.clone(),
                projected_qualifications: result.projected_qualifications.clone(),
            });
        }
        if let OperationKind::EstablishByteSequenceLiteral {
            destination,
            qualifications,
            ..
        } = &operation.kind
            && *destination == place
        {
            return Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: machine
                    .structural_place(place)?
                    .kind
                    .produced_structural_type()?,
                qualifications: qualifications.clone(),
                projected_qualifications: Vec::new(),
            });
        }
    }
    None
}

#[derive(Clone, Debug)]
pub enum ElementPayloadMemberships {
    Empty,
    Present(PayloadMembershipContract),
}

#[allow(clippy::too_many_arguments)]
pub fn element_payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    source: &StructuralArgument,
    element: StructuralTypeId,
) -> Option<ElementPayloadMemberships> {
    element_payload_memberships_in(
        types,
        domains,
        machine.id,
        analysis,
        position,
        source,
        element,
        |place| terminal_payload_contract(machine, place),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn element_payload_memberships_in(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    machine: MachineId,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    source: &StructuralArgument,
    element: StructuralTypeId,
    mut source_contract: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
) -> Option<ElementPayloadMemberships> {
    let root = source_contract(source.place)?;
    let mut query =
        descriptor_contents_query(types, source.place, root.carrier, &source.path, None)?;
    query
        .steps
        .push(DescriptorContentsStep::any_element(element));
    query.validate(types, root.carrier)?;
    let targets = analysis.payload_targets(machine, position, &query)?;
    if targets.is_empty() {
        return Some(ElementPayloadMemberships::Empty);
    }
    let contract = payload_memberships(types, domains, targets, source_contract)?;
    (contract.carrier == element).then_some(ElementPayloadMemberships::Present(contract))
}
