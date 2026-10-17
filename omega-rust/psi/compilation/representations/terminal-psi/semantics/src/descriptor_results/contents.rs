//! Typed queries for descriptor contents, separate from exact carrier addresses.
//!
//! A runtime selector denotes every possible element here. This analysis never
//! exports an activation's scalar IDs into another activation, nor grants write
//! or disjointness authority from an uncertain selection. Reads must account for
//! every selected slot; one unknown candidate makes the contents unknown.
use super::*;
mod incoming_memberships;
use crate::descriptor_lifetimes::{
    DescriptorPathSegment, descriptor_location, descriptor_location_type, descriptor_path,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorContentsStep {
    Address(DescriptorPathSegment),
    AnyElement { element: StructuralTypeId },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorContentsQuery {
    pub root: PlaceId,
    pub steps: Vec<DescriptorContentsStep>,
}

/// Leaf-wise value transfer. Carrier custody still uses the exact flow graph;
/// these queries describe possible payload locations, not write authority.
pub struct DescriptorContentsTransfer {
    pub source: DescriptorContentsQuery,
    pub destination: DescriptorContentsQuery,
    pub removes_source: bool,
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
    let slots = slot_kind.slots(types, source.root, value_type)?;
    if slots.is_empty() {
        return Some(Vec::new());
    }
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
    let mut transfers = Vec::new();
    for slot in slots {
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

impl DescriptorContentsQuery {
    /// A family is not an executable address, even when it has one typed root.
    pub fn exact_location(&self) -> Option<DescriptorLocation> {
        Some(DescriptorLocation {
            root: self.root,
            path: self
                .steps
                .iter()
                .map(|step| match step {
                    DescriptorContentsStep::Address(segment) => Some(segment.clone()),
                    DescriptorContentsStep::AnyElement { .. } => None,
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

    /// The exact carrier prefix observed while evaluating this query. Keeping
    /// that prefix live is conservative; it does not equate it with the result.
    pub fn observation(&self) -> DescriptorLocation {
        DescriptorLocation {
            root: self.root,
            path: self
                .steps
                .iter()
                .map_while(|step| match step {
                    DescriptorContentsStep::Address(segment) => Some(segment.clone()),
                    DescriptorContentsStep::AnyElement { .. } => None,
                })
                .collect(),
        }
    }

    /// Validate the full typed route, including inline field carriers that
    /// have no standalone structural declaration. Only collection crossings
    /// require a structural type; the final leaf uses its actual field identity.
    pub fn validate(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<()> {
        let mut tip = root_type;
        let mut path = Vec::new();
        for step in &self.steps {
            match step {
                DescriptorContentsStep::Address(segment) => path.push(segment.clone()),
                DescriptorContentsStep::AnyElement { element } => {
                    tip = descriptor_location_type(types, tip, &path)?;
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
                    path.clear();
                }
            }
        }
        descriptor_path(types, tip, &path).map(|_| ())
    }

    /// Geometry consumers that cannot retain symbolic contents get an indirect
    /// enclosing region, never a fabricated exact payload address.
    pub fn conservative_projection(
        &self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
    ) -> Option<Projection> {
        let prefix = self.observation();
        let mut projection = Projection::new(&descriptor_path(types, root_type, &prefix.path)?);
        projection.exact = false;
        projection.indirect = true;
        Some(projection.without_captured_address())
    }
}

/// Resolve declaration-local field identities before erasing runtime selectors.
/// The executable operation still owns its bounds, case and access obligations.
pub fn descriptor_contents_query(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<DescriptorContentsQuery> {
    let mut query = DescriptorContentsQuery {
        root,
        steps: Vec::new(),
    };
    let mut tip = root_type;
    let mut static_path = Vec::new();
    for segment in path {
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
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
            query
                .steps
                .push(DescriptorContentsStep::AnyElement { element });
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

/// Read-only possible payload locations. Symbolic incoming contents are not
/// exact addresses and grant neither write nor disjointness authority.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorPayloadTarget {
    Storage(DescriptorLocation),
    Incoming(DescriptorContentsQuery),
}

/// Disposable membership metadata selected from a current storage contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayloadMembershipContract {
    pub carrier: StructuralTypeId,
    pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

impl PayloadMembershipContract {
    /// Present the same completed payload without introducing a membership.
    /// Geometry permits descriptor rebinding only; source history and access
    /// stay with the caller, and every domain retains its full semantic theory.
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
            types,
            domains,
            destination,
            &mut self.qualifications,
            &mut self.projected_qualifications,
        )
        .ok()?;
        self.carrier = destination;
        Some(self)
    }
}

/// Select a value contract using its payload carrier, not its address shape.
/// Inline byte fields have a structural value type even when the descriptor
/// address walk ends at a primitive field rather than a nested declaration.
/// Current source rows and independently validated source-field predicates
/// supply the facts. Receiving annotations never supply evidence.
pub(super) fn select_payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &PayloadMembershipContract,
    address: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) -> Option<PayloadMembershipContract> {
    let path = descriptor_path(types, contract.carrier, address)?;
    let carrier = crate::qualification_path_carrier(types.iter(), contract.carrier, &path)?;
    let (mut qualifications, projected_qualifications) =
        terminal_psi::StructuralPathQualification::select(
            &contract.qualifications,
            &contract.projected_qualifications,
            &path,
        );
    // The actual source field's primitive invariants hold at each valid
    // establishment and replacement. They classify this read independently of
    // the receiving copy's requested domain or a previous initializer's facts.
    if let Some(predicates) =
        crate::shared_byte_field_copy_predicates(types.iter(), contract.carrier, &path)
    {
        qualifications.extend(byte_predicate_memberships(domains, carrier, predicates));
        qualifications.sort();
        qualifications.dedup();
    }
    Some(PayloadMembershipContract {
        carrier,
        qualifications,
        projected_qualifications,
    })
}

/// Copy only memberships held by every possible backing subtree. Unknown or
/// symbolic contents cannot be filled from the receiving type's declaration.
pub fn payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    targets: impl IntoIterator<Item = DescriptorPayloadTarget>,
    mut source: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
) -> Option<PayloadMembershipContract> {
    let mut common: Option<PayloadMembershipContract> = None;
    for target in targets {
        let mut selected = match target {
            DescriptorPayloadTarget::Storage(location) => {
                select_payload_memberships(types, domains, &source(location.root)?, &location.path)?
            }
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

/// Predicate implication classifies the same current bytes, never a route or
/// content witness. Close each candidate before intersection: distinct incoming
/// theories may imply the same predicate-only membership. Unknown payloads still
/// decline above, and type, access, availability and custody remain independent.
fn close_predicate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
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
        // Descendant rows belong to their own carrier and are closed separately.
        for domain in implied_predicate_memberships(types, domains, carrier, &whole) {
            contract
                .projected_qualifications
                .push(terminal_psi::StructuralPathQualification {
                    path: path.clone(),
                    domain,
                });
        }
    }
    contract.projected_qualifications.sort();
    contract.projected_qualifications.dedup();
}

fn implied_predicate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    carrier: StructuralTypeId,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
) -> Vec<semantic_vocabulary::StructuralDomainId> {
    // A closed empty theory classifies every value of this exact carrier.
    // Routed and Content memberships remain occurrence-bound even when
    // their logical predicate is Truth.
    let mut implied = domains
        .iter()
        .filter(|domain| {
            domain.carrier == carrier
                && domain.establishment_routes.is_empty()
                && domain.content_projection.is_none()
                && domain.predicate.as_ref().is_some_and(|predicate| {
                    predicate.proposition == semantic_vocabulary::Proposition::Truth
                })
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

/// Current declared memberships of actual Terminal storage, including literal
/// views. This reads no receiving annotation and grants no availability.
pub fn terminal_payload_contract(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<PayloadMembershipContract> {
    if let Some(parameter) = crate::arrival_parameter(machine, place) {
        return Some(PayloadMembershipContract {
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

/// Evidence about every possible element, including a proved empty source.
/// Unknown contents remain the outer `None`, never vacuous evidence.
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

/// Reconstruct every element from the current graph and its source contracts.
/// Optimized consumers supply their current declarations, never frozen input.
#[allow(clippy::too_many_arguments)]
pub fn element_payload_memberships_in(
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
        .push(DescriptorContentsStep::AnyElement { element });
    query.validate(types, root.carrier)?;
    let targets = analysis.payload_targets(machine, position, &query)?;
    if targets.is_empty() {
        return Some(ElementPayloadMemberships::Empty);
    }
    let contract = payload_memberships(types, domains, targets, source_contract)?;
    (contract.carrier == element).then_some(ElementPayloadMemberships::Present(contract))
}
