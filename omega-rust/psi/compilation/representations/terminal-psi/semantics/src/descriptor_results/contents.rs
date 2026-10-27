//! Typed queries for descriptor contents, separate from exact carrier addresses.
//!
//! A runtime selector denotes every possible element here. This analysis never
//! exports an activation's scalar IDs into another activation, nor grants write
//! or disjointness authority from an uncertain selection. Reads must account for
//! every selected slot; one unknown candidate makes the contents unknown.
use super::*;
mod incoming_memberships;
use crate::descriptor_lifetimes::{descriptor_location, descriptor_location_type, descriptor_path};

pub use crate::descriptor_lifetimes::{DescriptorContentsQuery, DescriptorContentsStep};

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
    pub scalar_observations: Vec<super::PayloadScalarObservation>,
    pub scalar_requirements: Vec<semantic_vocabulary::Proposition>,
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

/// Rejoin a reference occurrence to every actual backing producer's declared
/// roster at its pre-operation cut. This supplies restoration obligations, not
/// inferred current-value predicates, a physical address or access authority.
/// The receiving result is excluded even when a loop reuses its static place.
/// An explicit stored representation requires its separate interpretation proof;
/// this canonical-payload query cannot substitute for its physical offset.
pub fn reference_backing_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
    analysis: &mut DescriptorAnalysis<'_>,
    machine: MachineId,
    position: DescriptorPosition,
    source: &StructuralArgument,
    storage: Option<&terminal_psi::ReferenceStorageView>,
    result: &terminal_psi::StructuralOperationResult,
    mut source_contract: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<Vec<terminal_psi::StructuralPathQualification>, &'static str> {
    let StructuralTypeShape::Reference { referent, access } =
        terminal_psi::find_structural_type(types, result.structural_type)
            .ok_or("reference result has no declared carrier")?
            .shape
    else {
        return Err("reference result has no reference carrier");
    };
    if storage.is_some() {
        return Err("reference backing requires its explicit storage interpretation");
    }
    if access != source.access || source.place == result.place {
        return Err("reference backing has no independent matching source");
    }
    let root =
        source_contract(source.place).ok_or("reference backing has no source declaration")?;
    let path = crate::reference_source_storage_path(types.iter(), root.carrier, &source.path);
    let query = descriptor_contents_query(types, source.place, root.carrier, path, None)
        .ok_or("reference backing has no typed contents query")?;
    let targets = analysis
        .payload_targets(machine, position, &query)
        .ok_or("reference backing has an unresolved source alternative")?;
    reference_payload_memberships(
        types,
        domains,
        identities,
        targets,
        referent,
        access,
        |place| {
            (place != result.place)
                .then(|| source_contract(place))
                .flatten()
        },
    )
}

/// Retain a reference's complete declared source roster after descriptor
/// resolution. Unlike an observation of current scalar contents, this reads
/// only the actual producers/ingresses supplied by `source`. Every alternative
/// must resolve. Shared loans meet their observations; exclusive loans cannot
/// silently discard a candidate's distinct restoration obligations.
fn reference_payload_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
    targets: impl IntoIterator<Item = DescriptorPayloadTarget>,
    referent: StructuralTypeId,
    access: StructuralAccess,
    mut source: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<Vec<terminal_psi::StructuralPathQualification>, &'static str> {
    let mut common: Option<Vec<terminal_psi::StructuralPathQualification>> = None;
    for target in targets {
        let query = match target {
            DescriptorPayloadTarget::Storage(location) => {
                DescriptorContentsQuery::from_location(&location)
            }
            DescriptorPayloadTarget::Incoming(query) => query,
        };
        let contract = source(query.root).ok_or("reference payload has no source declaration")?;
        query
            .validate(types, contract.carrier)
            .ok_or("reference payload has no typed source query")?;
        let path = reference_membership_path(types, contract.carrier, &query.steps)
            .ok_or("reference payload has no declared membership path")?;
        let selected = crate::qualification_path_carrier(types.iter(), contract.carrier, &path)
            .ok_or("reference payload has no declared selected carrier")?;
        if crate::borrowed_referent_presentation(types.iter(), selected, referent)
            != Some(crate::BorrowedReferentPresentation::Unchanged)
        {
            return Err("reference payload changes its selected source carrier");
        }
        if !crate::reference_preserves_ancestor_qualifications(
            types.iter(),
            domains.iter(),
            identities,
            &contract.qualifications,
            &contract.projected_qualifications,
            &path,
            access,
        ) {
            return Err("reference payload loses an ancestor membership obligation");
        }
        let captured = crate::reference_source_memberships(
            types.iter(),
            contract.carrier,
            &contract.qualifications,
            &contract.projected_qualifications,
            &path,
            access,
        )?;
        let (mut whole, mut projected) = terminal_psi::StructuralPathQualification::select(
            &[],
            &captured,
            &[StructuralPathSegment::Referent],
        );
        crate::present_structural_memberships(
            types.iter(),
            domains.iter(),
            referent,
            &mut whole,
            &mut projected,
        )?;
        let mut captured = Vec::new();
        terminal_psi::StructuralPathQualification::append_under(
            &whole,
            &projected,
            &[StructuralPathSegment::Referent],
            &mut captured,
        );
        captured.sort();
        captured.dedup();
        if let Some(common) = &mut common {
            if access != StructuralAccess::SharedBorrow && *common != captured {
                return Err("reference payload cannot retain every source membership obligation");
            }
            common.retain(|row| captured.binary_search(row).is_ok());
        } else {
            common = Some(captured);
        }
    }
    common.ok_or("reference payload has no source candidate")
}

/// Convert the typed payload-query grammar to membership metadata. A universal
/// step stays `AllElements`: no dummy SSA value, executable index or address is
/// manufactured merely to reuse the membership selection operation.
fn reference_membership_path(
    types: &[StructuralTypeDeclaration],
    mut carrier: StructuralTypeId,
    steps: &[DescriptorContentsStep],
) -> Option<Vec<StructuralPathSegment>> {
    let mut path = Vec::new();
    let mut address = Vec::new();
    for step in steps {
        match step {
            DescriptorContentsStep::Address(segment) => address.push(segment.clone()),
            DescriptorContentsStep::AnyElement { element } => {
                let prefix = descriptor_path(types, carrier, &address)?;
                path.extend(prefix.iter().cloned());
                carrier = crate::qualification_path_carrier(types.iter(), carrier, &prefix)?;
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

/// Predicate implication classifies the same current bytes, never a route or
/// content witness. Close each candidate before intersection: distinct incoming
/// theories may imply the same predicate-only membership. Unknown payloads still
/// decline above, and type, access, availability and custody remain independent.
fn close_predicate_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
    super::payload_predicates::close(types, domains, contract);
    // A refined primitive carrier owes its declared predicate at every
    // completed store. Literal observations are not required: the carrier
    // identity is that constrained type, and interval bounds stay on the
    // scalar restriction instead of becoming another nominal row. Only the
    // refinement's own theory is admitted, so a neighboring domain on the
    // same carrier does not become an element membership.
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

/// The carrier's refined-scalar declaration, when one domain states that
/// exact predicate and nothing else. Establishment routes and content
/// projections stay occurrence-bound.
fn admit_declared_scalar_refinement(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    contract: &mut PayloadMembershipContract,
) {
    let Some(refinement) = types
        .iter()
        .find(|row| row.id == contract.carrier)
        .and_then(|row| match &row.scalar_restriction {
            Some(terminal_psi::StructuralFieldType::RefinedScalar(refinement)) => Some(refinement),
            _ => None,
        })
    else {
        return;
    };
    for domain in domains {
        if domain.carrier != contract.carrier
            || domain.content_projection.is_some()
            || !domain.establishment_routes.is_empty()
            || contract.qualifications.contains(&domain.id)
        {
            continue;
        }
        let Some(predicate) = super::payload_predicates::introduction_predicate(
            types,
            domains,
            domain,
            &contract.qualifications,
        ) else {
            continue;
        };
        let mut proposition = predicate.proposition.clone();
        rewrite_subject_as_refinement_value(
            &mut proposition,
            predicate.subject,
            refinement.carrier(),
        );
        // Domain rows lower a comparison as `true = (left ~ right)`. The
        // carrier refinement states that comparison directly.
        normalize_boolean_comparison(&mut proposition);
        if proposition.same_commutative_predicate(refinement.predicate()) {
            contract.qualifications.push(domain.id);
        }
    }
}

fn rewrite_subject_as_refinement_value(
    proposition: &mut semantic_vocabulary::Proposition,
    subject: semantic_vocabulary::PlaceId,
    carrier: semantic_vocabulary::ScalarType,
) {
    fn rewrite_term(
        term: &mut semantic_vocabulary::ScalarTerm,
        subject: semantic_vocabulary::PlaceId,
        carrier: semantic_vocabulary::ScalarType,
    ) {
        use semantic_vocabulary::ScalarTerm;
        let replacement = match term {
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } if *root == subject
                && path.is_empty()
                && semantic_vocabulary::ScalarType::Integer(*scalar_type) == carrier =>
            {
                Some(ScalarTerm::value(
                    semantic_vocabulary::ScalarRefinement::SUBJECT,
                    carrier,
                ))
            }
            ScalarTerm::BooleanField { root, path }
                if *root == subject
                    && path.is_empty()
                    && carrier == semantic_vocabulary::ScalarType::Boolean =>
            {
                Some(ScalarTerm::value(
                    semantic_vocabulary::ScalarRefinement::SUBJECT,
                    carrier,
                ))
            }
            ScalarTerm::IeeeFloatField { root, path, format }
                if *root == subject
                    && path.is_empty()
                    && carrier == semantic_vocabulary::ScalarType::IeeeFloat(*format) =>
            {
                Some(ScalarTerm::value(
                    semantic_vocabulary::ScalarRefinement::SUBJECT,
                    carrier,
                ))
            }
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } if *root == subject && path.is_empty() && *scalar_type == carrier => Some(
                ScalarTerm::value(semantic_vocabulary::ScalarRefinement::SUBJECT, carrier),
            ),
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
        carrier: semantic_vocabulary::ScalarType,
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
        carrier: semantic_vocabulary::ScalarType,
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

/// Turn `true = integer comparison` into the comparison proposition. Operand
/// order inside the comparison stays as lowered.
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
                ScalarTerm::IntegerLessThan { left, right, .. } => Some(Proposition::LessThan(
                    left.as_ref().clone(),
                    right.as_ref().clone(),
                )),
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
    // A closed empty theory classifies every value of this exact carrier.
    // Routed and Content memberships remain occurrence-bound even when
    // their logical predicate is Truth.
    let mut implied = domains
        .iter()
        .filter(|domain| {
            domain.carrier == carrier
                && (domain
                    .missing_membership_requirements(domains, qualifications)
                    .is_some_and(|missing| missing.is_empty())
                    || super::payload_predicates::introduction_predicate(
                        types,
                        domains,
                        domain,
                        qualifications,
                    )
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

/// Current declared memberships of actual Terminal storage, including literal
/// views. This reads no receiving annotation and grants no availability.
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
