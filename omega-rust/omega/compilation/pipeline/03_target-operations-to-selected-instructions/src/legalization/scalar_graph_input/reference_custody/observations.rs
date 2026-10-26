//! Borrowed pointer contents are reconstructed independently of owned leaves.
//! The source graph supplies current values; declarations supply types only.
//! Observing external contents creates no resident permission occurrence.

use super::*;
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::DescriptorSourceOrigin;

pub(super) struct Observed {
    pub(super) root: Root,
    pub(super) referent: StructuralTypeId,
    pub(super) parents: Vec<OccurrenceIdentity>,
    /// Exact current carrier for copies and permission checks. Copying this
    /// occurrence preserves its parents rather than reborrowing the carrier.
    pub(super) resident: Option<(PlaceId, Vec<StructuralPathSegment>)>,
}

/// Normalize every possible selected descriptor without merging its resident
/// identity. Every candidate must observe successfully; its original refusal
/// retains the exact concrete path that failed. Calls still check each
/// resident's permissions independently.
pub(super) fn observe_alternatives(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<Vec<Observed>, LegalizationError> {
    let contract = source_contract(function, custody, place).ok_or_else(|| {
        Refusal::MissingSourceContract.into_error(function, custody, types, place, path)
    })?;
    if custody.leaf(place, path).is_some()
        || descriptor_location(types, place, contract, path, None).is_some()
    {
        return observe(function, custody, types, place, path)
            .map(|observed| vec![observed])
            .map_err(|reason| reason.into_error(function, custody, types, place, path));
    }
    let locations =
        fixed_runtime_index_alternatives(types, place, contract, path).ok_or_else(|| {
            Refusal::InvalidFixedIndexAlternatives.into_error(function, custody, types, place, path)
        })?;
    locations
        .iter()
        .map(|location| {
            let concrete = descriptor_path(types, contract, &location.path).ok_or_else(|| {
                Refusal::InvalidFixedIndexDescriptorPath
                    .into_error(function, custody, types, place, path)
            })?;
            observe(function, custody, types, place, &concrete)
                .map_err(|reason| reason.into_error(function, custody, types, place, &concrete))
        })
        .collect()
}

/// A failed current-storage join is distinct from an inactive case. Reasons
/// describe the original refusal; none supplies an alternate observation.
#[derive(Clone, Copy)]
pub(super) enum Refusal {
    MissingSourceContract,
    CyclicResidentObservation,
    InvalidCarrierPath,
    CarrierIsNotReference,
    InvalidDescriptorLocation,
    InvalidFixedIndexAlternatives,
    InvalidFixedIndexDescriptorPath,
    MissingMachineObservations,
    MissingOperationPosition,
    MissingPositionObservations,
    MissingLocationObservation,
    MissingResidentStorageContract,
    InvalidResidentStoragePath,
    ResidentCarrierTypeMismatch,
    ResidentReferentMismatch,
    UnknownOrigins,
    OwnedIngressWithoutResident,
    InvalidIngressStoragePath,
    IngressCarrierTypeMismatch,
    LocalOriginWithoutResident,
    EmptyOrigins,
}

impl Refusal {
    fn description(self) -> &'static str {
        match self {
            Self::CyclicResidentObservation => "resident reference observation is cyclic",
            Self::MissingSourceContract => "source storage contract is unavailable",
            Self::InvalidCarrierPath => "descriptor path does not select a declared carrier",
            Self::CarrierIsNotReference => "selected carrier is not an ordinary reference",
            Self::InvalidDescriptorLocation => "descriptor location cannot be reconstructed",
            Self::InvalidFixedIndexAlternatives => {
                "descriptor path has no finite fixed-index alternatives"
            }
            Self::InvalidFixedIndexDescriptorPath => {
                "fixed-index alternative has no declared descriptor path"
            }
            Self::MissingMachineObservations => "machine reference observations are unavailable",
            Self::MissingOperationPosition => "current operation position is unavailable",
            Self::MissingPositionObservations => "current position has no reference observations",
            Self::MissingLocationObservation => "current location has no reference observation",
            Self::MissingResidentStorageContract => "observed carrier storage has no contract",
            Self::InvalidResidentStoragePath => "observed carrier storage path is invalid",
            Self::ResidentCarrierTypeMismatch => "observed storage has a different carrier type",
            Self::ResidentReferentMismatch => "resident reference has a different referent type",
            Self::UnknownOrigins => "observed reference origins are unknown",
            Self::OwnedIngressWithoutResident => {
                "owned ingress descriptor has no resident occurrence"
            }
            Self::InvalidIngressStoragePath => "incoming descriptor storage path is invalid",
            Self::IngressCarrierTypeMismatch => "incoming slot has a different carrier type",
            Self::LocalOriginWithoutResident => "local reference origin has no resident occurrence",
            Self::EmptyOrigins => "observed reference origins are empty",
        }
    }

    /// Capture diagnostic context only after the original observation refused.
    /// These read-only lookups neither replay the graph nor repair custody.
    #[track_caller]
    pub(super) fn into_error(
        self,
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &[StructuralTypeDeclaration],
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> LegalizationError {
        let contract = source_contract(function, custody, place);
        let carrier = contract.and_then(|contract| leaf_type(types, contract, path));
        let location =
            contract.and_then(|contract| descriptor_location(types, place, contract, path, None));
        let observation = custody
            .stored_calls
            .observations
            .get(&function.machine)
            .and_then(|positions| positions.get(&custody.reference_position?))
            .and_then(|locations| locations.get(location.as_ref()?));
        let storage = observation.and_then(|observation| observation.storage.clone());
        let resident_path = storage.as_ref().and_then(|storage| {
            descriptor_path(
                types,
                source_contract(function, custody, storage.root)?,
                &storage.path,
            )
        });
        let resident_referent = storage.as_ref().and_then(|storage| {
            custody
                .leaf(storage.root, resident_path.as_deref()?)
                .map(|leaf| leaf.referent)
        });
        if matches!(self, Self::LocalOriginWithoutResident)
            && std::env::var_os("OMEGA_RESIDENT_OCCURRENCE_DIAGNOSTIC").is_some()
        {
            eprintln!(
                "resident-occurrence-diagnostic machine={:?} position={:?} place={place:?} path={path:?} storage={storage:?} resident_path={resident_path:?} origins={:?} parameters={:?} places={:?} blocks={:#?} residents={:#?} retired={:#?} borrowed_arrivals={:#?}",
                function.machine,
                custody.reference_position,
                observation.and_then(|observation| observation.origins.as_ref()),
                function.structural_parameters,
                function.structural_places,
                function.blocks,
                custody.leaves,
                custody.retired,
                custody.borrowed_sources,
            );
        }
        LegalizationError::ReferenceObservation {
            machine: function.machine,
            operation: None,
            site: optimization_core::CompilerCheckSite::caller(),
            reason: self.description(),
            context: Box::new(crate::legalization::model::ReferenceObservationContext {
                position: custody.reference_position,
                place,
                path: path.to_vec(),
                carrier,
                catalog_available: custody.stored_calls.reference_catalog_available,
                location,
                observation_available: observation.is_some(),
                storage,
                resident_path,
                resident_referent,
                origin_count: observation
                    .and_then(|observation| observation.origins.as_ref().map(Vec::len)),
            }),
        }
    }
}

pub(super) fn observe(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<Observed, Refusal> {
    observe_following(function, custody, types, place, path, &mut Vec::new())
}

fn observe_following(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Result<Observed, Refusal> {
    if let Some(leaf) = custody.leaf(place, path) {
        return Ok(Observed {
            root: leaf.root.clone(),
            referent: leaf.referent,
            parents: vec![leaf.identity.clone()],
            resident: Some((place, path.to_vec())),
        });
    }
    let contract =
        source_contract(function, custody, place).ok_or(Refusal::MissingSourceContract)?;
    let carrier = leaf_type(types, contract, path).ok_or(Refusal::InvalidCarrierPath)?;
    let StructuralTypeShape::Reference { referent, .. } = find_structural_type(types, carrier)
        .ok_or(Refusal::InvalidCarrierPath)?
        .shape
    else {
        return Err(Refusal::CarrierIsNotReference);
    };
    let coordinate = (place, path.to_vec());
    if active.contains(&coordinate) {
        return Err(Refusal::CyclicResidentObservation);
    }
    active.push(coordinate);
    let resident = through_resident(function, custody, types, place, path, active);
    active.pop();
    if let Some(resident) = resident {
        return (resident.referent == referent)
            .then_some(resident)
            .ok_or(Refusal::ResidentReferentMismatch);
    }
    let location = descriptor_location(types, place, contract, path, None)
        .ok_or(Refusal::InvalidDescriptorLocation)?;
    let observation = custody
        .stored_calls
        .observations
        .get(&function.machine)
        .ok_or(Refusal::MissingMachineObservations)?
        .get(
            &custody
                .reference_position
                .ok_or(Refusal::MissingOperationPosition)?,
        )
        .ok_or(Refusal::MissingPositionObservations)?
        .get(&location)
        .ok_or(Refusal::MissingLocationObservation)?;
    // Join the carrier slot, not the pointee. The current resident leaf owns
    // receiving identity, original formations and immediate ancestry, including
    // copies and replacements. Reconstructing these from equal backing would
    // silently detach a copied child from its still-suspended parent.
    if let Some(storage) = &observation.storage {
        let storage_type = source_contract(function, custody, storage.root)
            .ok_or(Refusal::MissingResidentStorageContract)?;
        let storage_path = descriptor_path(types, storage_type, &storage.path)
            .ok_or(Refusal::InvalidResidentStoragePath)?;
        if leaf_type(types, storage_type, &storage_path) != Some(carrier) {
            return Err(Refusal::ResidentCarrierTypeMismatch);
        }
        if let Some(leaf) = custody.leaf(storage.root, &storage_path) {
            if leaf.referent != referent {
                return Err(Refusal::ResidentReferentMismatch);
            }
            return Ok(Observed {
                root: leaf.root.clone(),
                referent,
                parents: vec![leaf.identity.clone()],
                resident: Some((storage.root, storage_path)),
            });
        }
    }
    let sources = observation
        .origins
        .as_ref()
        .ok_or(Refusal::UnknownOrigins)?;
    let mut origins = Vec::new();
    for source in sources {
        let origin = match source {
            DescriptorSourceOrigin::IncomingSlot(slot) => {
                let parameter = function
                    .structural_parameter(slot.root)
                    .ok_or(Refusal::InvalidIngressStoragePath)?;
                let slot_path = descriptor_path(types, parameter.structural_type, &slot.path)
                    .ok_or(Refusal::InvalidIngressStoragePath)?;
                if parameter.access == StructuralAccess::Owned {
                    // An owned ingress descriptor has a real resident
                    // occurrence; its absence cannot become external custody.
                    // A slot behind that descriptor is external storage, but
                    // only the authenticated original incoming pointer may
                    // supply its incoming contents at this observation epoch.
                    authenticate_indirect_ingress(
                        function, custody, types, place, path, slot.root, &slot_path,
                    )
                    .ok_or(Refusal::OwnedIngressWithoutResident)?;
                }
                if leaf_type(types, parameter.structural_type, &slot_path) != Some(carrier) {
                    return Err(Refusal::IngressCarrierTypeMismatch);
                }
                target_operations::TargetReferenceOrigin {
                    base: target_operations::TargetReferenceOriginBase::IncomingContents {
                        root: slot.root,
                        slot: slot_path,
                    },
                    path: Vec::new(),
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                }
            }
            // Exact backing coordinates do not prove absence of a local
            // parent. Locally formed values require the resident join above.
            _ => return Err(Refusal::LocalOriginWithoutResident),
        };
        origins.push(origin);
    }
    if origins.is_empty() {
        return Err(Refusal::EmptyOrigins);
    }
    origins.sort();
    origins.dedup();
    // Traversing a containing pointer reads the slot, not a reborrow of the
    // copied pointer's pointee. Those traversal dependencies belong to the
    // operation; external slot contents have no owned local parent to invent.
    Ok(Observed {
        root: Root::Alternatives(origins),
        referent,
        parents: Vec::new(),
        resident: None,
    })
}

/// Follow the completed descriptor's actual storage origin before consulting
/// symbolic incoming contents. This preserves the inner resident occurrence
/// through owned joins without manufacturing custody from its backing type.
fn through_resident(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Option<Observed> {
    let crossing = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier =
        observe_following(function, custody, types, place, &path[..crossing], active).ok()?;
    // Retain the ordinary prefix permission/access check, including ancestor
    // conflicts. Origin geometry alone does not authorize loading the slot.
    super::call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path[..=crossing].to_vec(),
            access: StructuralAccess::SharedBorrow,
        },
    )
    .ok()?;
    let origins =
        super::storage_origins(function, custody, types, &carrier.root, carrier.referent).ok()?;
    let mut selected: Option<Observed> = None;
    for origin in origins {
        let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
            return None;
        };
        let mut projected = origin.exact_path()?;
        projected.extend_from_slice(&path[crossing + 1..]);
        let observed =
            observe_following(function, custody, types, root, &projected, active).ok()?;
        if let Some(previous) = &selected {
            // A scalar observation cannot collapse different permission
            // occurrences merely because their referents have equal layout.
            if previous.root != observed.root
                || previous.referent != observed.referent
                || previous.parents != observed.parents
                || previous.resident != observed.resident
            {
                return None;
            }
        } else {
            selected = Some(observed);
        }
    }
    selected
}

/// Inline owned slots require resident custody above. An indirect ingress
/// slot instead requires every earlier pointer crossing to retain its exact
/// incoming provenance. The shared call walk checks readable access, current
/// occurrences, ancestor conflicts and declaration geometry; recursion here
/// strictly shortens the descriptor path at its last pointer crossing.
fn authenticate_indirect_ingress(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    current_place: PlaceId,
    current_path: &[StructuralPathSegment],
    root: PlaceId,
    incoming_path: &[StructuralPathSegment],
) -> Option<()> {
    let crossing = current_path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let incoming_crossing = incoming_path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &incoming_path[..incoming_crossing];
    let observed = super::call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place: current_place,
            path: current_path[..=crossing].to_vec(),
            access: StructuralAccess::SharedBorrow,
        },
    )
    .ok()?;
    let incoming = if carrier_path.contains(&StructuralPathSegment::Referent) {
        Root::Alternatives(vec![target_operations::TargetReferenceOrigin {
            base: target_operations::TargetReferenceOriginBase::IncomingContents {
                root,
                slot: carrier_path.to_vec(),
            },
            path: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        }])
    } else {
        Root::Ingress(root, carrier_path.to_vec())
    };
    (!observed.origins.is_empty() && observed.origins.iter().all(|origin| *origin == incoming))
        .then_some(())
}
