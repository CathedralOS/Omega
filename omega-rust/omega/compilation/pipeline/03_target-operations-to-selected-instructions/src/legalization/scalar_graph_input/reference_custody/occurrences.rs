//! Preserve actual receiving occurrences separately from transported formations.
//! Only authenticated owned bindings select parameter occurrences. Their roots,
//! immediate parents and complete formation alternatives converge; a missing or
//! unrelated surviving leaf never disappears through an availability intersection.

use super::{
    Custody, Leaf, LegalizationError, OperationId, PlaceId, PsiOptimizationFunction, Root,
    StructuralAccess, StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, invalid, leaf_referent, owned_relative_path,
};
use semantic_vocabulary::{BlockId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralParameterDeclaration;

/// A permission occurrence is not its current storage slot. In particular a
/// store can install a fresh shared copy in a slot whose former child survives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::legalization) struct OccurrenceIdentity {
    pub(super) place: PlaceId,
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) formation: OccurrenceFormation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum OccurrenceFormation {
    Parameter,
    Operation(OperationId),
    ResidentJoin(BlockId),
}

impl OccurrenceIdentity {
    pub(super) fn resident_join(
        block: BlockId,
        location: &(PlaceId, Vec<StructuralPathSegment>),
    ) -> Self {
        Self {
            place: location.0,
            path: location.1.clone(),
            formation: OccurrenceFormation::ResidentJoin(block),
        }
    }

    pub(super) fn parameter(location: &(PlaceId, Vec<StructuralPathSegment>)) -> Self {
        Self {
            place: location.0,
            path: location.1.clone(),
            formation: OccurrenceFormation::Parameter,
        }
    }

    pub(super) fn operation(
        location: &(PlaceId, Vec<StructuralPathSegment>),
        operation: OperationId,
    ) -> Self {
        Self {
            place: location.0,
            path: location.1.clone(),
            formation: OccurrenceFormation::Operation(operation),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CarrierContract {
    Operation {
        operation: OperationId,
        result: StructuralOperationResult,
    },
    Parameter(StructuralParameterDeclaration),
    ResidentJoin {
        block: BlockId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

impl CarrierContract {
    pub(super) fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Operation { result, .. } => result.structural_type,
            Self::Parameter(parameter) => parameter.structural_type,
            Self::ResidentJoin {
                structural_type, ..
            } => *structural_type,
        }
    }

    pub(super) fn matches_declaration(
        &self,
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> bool {
        match self {
            Self::ResidentJoin {
                block,
                place: resident,
                structural_type,
            } => {
                *resident == place
                    && function.block(*block).is_some()
                    && resident_type(function, place) == Some(*structural_type)
            }
            Self::Operation { operation, result } => {
                result.place == place
                    && function.structural_places.iter().any(|declaration| {
                        declaration.id == place
                            && declaration.kind
                                == StructuralPlaceKind::OperationResult {
                                    producer: *operation,
                                    structural_type: result.structural_type,
                                }
                    })
                    && super::super::structural_case::source_result(function, place).is_ok_and(
                        |(actual_operation, actual)| {
                            actual_operation == *operation && actual == result
                        },
                    )
            }
            Self::Parameter(parameter) => {
                parameter.place == place
                    && function.structural_places.iter().any(|declaration| {
                        declaration.id == place
                            && (function
                                .structural_parameters
                                .iter()
                                .any(|actual| actual == parameter)
                                && declaration.kind
                                    == StructuralPlaceKind::Parameter {
                                        position: parameter.position,
                                    }
                                || function.blocks.iter().any(|block| {
                                    block
                                        .structural_parameters
                                        .iter()
                                        .any(|actual| actual == parameter)
                                        && declaration.kind
                                            == StructuralPlaceKind::BlockParameter {
                                                block: block.id,
                                                position: parameter.position,
                                            }
                                }))
                    })
            }
        }
    }
}

/// The slot is already declared storage; a joined loan cannot declare its own
/// home by supplying a convenient type in its custody row.
pub(super) fn resident_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    function
        .structural_parameter(place)
        .or_else(|| {
            function
                .block_structural_parameters()
                .find(|row| row.place == place)
        })
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            super::super::structural_case::source_result(function, place)
                .ok()
                .map(|(_, result)| result.structural_type)
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Formation {
    pub(super) identity: OccurrenceIdentity,
    pub(super) contract: CarrierContract,
}

impl Leaf {
    pub(super) fn formed(
        identity: (PlaceId, Vec<StructuralPathSegment>),
        root: Root,
        parents: Vec<OccurrenceIdentity>,
        operation: OperationId,
        result: &StructuralOperationResult,
        referent: StructuralTypeId,
    ) -> Self {
        let contract = CarrierContract::Operation {
            operation,
            result: result.clone(),
        };
        let identity = OccurrenceIdentity::operation(&identity, operation);
        Self {
            identity: identity.clone(),
            backing: vec![super::descriptor_backing::Backing::Source(root.clone())],
            root,
            parents,
            contract: contract.clone(),
            formations: vec![Formation { identity, contract }],
            referent,
            placed: None,
        }
    }

    /// A copy or newly selected call result forms a new permission occurrence.
    /// Its backing and parent obligations remain separate from this declaration.
    pub(super) fn reform(&mut self, operation: OperationId, result: &StructuralOperationResult) {
        self.identity.formation = OccurrenceFormation::Operation(operation);
        self.contract = CarrierContract::Operation {
            operation,
            result: result.clone(),
        };
        self.formations = vec![Formation {
            identity: self.identity.clone(),
            contract: self.contract.clone(),
        }];
    }

    pub(in crate::legalization) fn matches_operation_result(
        &self,
        result: &StructuralOperationResult,
    ) -> bool {
        matches!(&self.contract, CarrierContract::Operation { result: actual, .. } if actual == result)
    }
}

pub(super) fn normalize_formations(
    formations: &mut Vec<Formation>,
) -> Result<(), LegalizationError> {
    formations.sort_by(|left, right| left.identity.cmp(&right.identity));
    if formations
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity && pair[0].contract != pair[1].contract)
    {
        return Err(invalid());
    }
    formations.dedup();
    Ok(())
}

pub(super) fn merge_arrivals(
    function: &PsiOptimizationFunction,
    target: BlockId,
    types: &[StructuralTypeDeclaration],
    left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Result<Option<()>, LegalizationError> {
    // An owned sum arrival carries only the selected payload. Authenticate
    // every asymmetric slot against the receiving declaration and its case
    // namespace; unconditional and unrelated missing loans still reject.
    for (key, leaf) in left.iter().chain(right) {
        if left.contains_key(key) == right.contains_key(key) {
            continue;
        }
        let parameter = function
            .block(target)
            .and_then(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == key.0)
            })
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
            .ok_or_else(invalid)?;
        if !key
            .1
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
            || leaf.identity != OccurrenceIdentity::parameter(key)
            || leaf.contract != CarrierContract::Parameter(parameter.clone())
            || leaf.formations.is_empty()
            || leaf_referent(types, parameter.structural_type, &key.1) != Some(leaf.referent)
        {
            return Err(invalid());
        }
    }
    for (key, incoming) in right {
        let Some(existing) = left.get_mut(key) else {
            left.insert(key.clone(), incoming.clone());
            continue;
        };
        if existing == incoming {
            continue;
        }
        let parameter = function
            .block(target)
            .and_then(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == key.0)
            })
            .filter(|parameter| parameter.access == StructuralAccess::Owned);
        let (identity, contract) = if let Some(parameter) = parameter {
            (
                OccurrenceIdentity::parameter(key),
                CarrierContract::Parameter(parameter.clone()),
            )
        } else if matches!(&existing.contract, CarrierContract::ResidentJoin { block, place, .. }
            if *block == target && *place == key.0)
            && existing.contract.matches_declaration(function, key.0)
        {
            (
                OccurrenceIdentity::resident_join(target, key),
                existing.contract.clone(),
            )
        } else {
            // Nonselected metadata can arrive at different worklist times.
            // Defer rather than union it; convergence still requires equality.
            return Ok(None);
        };
        if existing.identity != identity
            || incoming.identity != identity
            || existing.contract != contract
            || incoming.contract != contract
            || existing.referent != incoming.referent
            || existing.formations.is_empty()
            || incoming.formations.is_empty()
        {
            return Err(invalid());
        }
        if existing.root != incoming.root {
            let mut origins = existing.root.origins();
            origins.extend(incoming.root.origins());
            origins.sort();
            origins.dedup();
            existing.root = Root::Alternatives(origins);
        }
        existing.backing.extend_from_slice(&incoming.backing);
        existing.backing.sort();
        existing.backing.dedup();
        existing.parents.extend_from_slice(&incoming.parents);
        existing.parents.sort();
        existing.parents.dedup();
        existing.formations.extend_from_slice(&incoming.formations);
        normalize_formations(&mut existing.formations)?;
    }
    Ok(Some(()))
}

/// Select complete owned leaf rosters before modifying storage. Parameter
/// bindings and their parent links move simultaneously, including swaps.
pub(super) fn re_home_bound_leaves(
    custody: &mut Custody,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[StructuralParameterDeclaration],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    if bindings.len() != parameters.len() {
        return Err(invalid());
    }
    let mut moved = Vec::new();
    let mut selected_sources = BTreeSet::new();
    let mut selected_identities = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if parameter.access != StructuralAccess::Owned
            || binding.argument.access != StructuralAccess::Owned
        {
            continue;
        }
        for (source, leaf) in &custody.leaves {
            if source.0 != binding.argument.place {
                continue;
            }
            let Some(relative) = owned_relative_path(&source.1, &binding.argument.path) else {
                continue;
            };
            let destination = (parameter.place, relative.to_vec());
            // Case alternatives of one carrier hold one occurrence; each
            // alternative arrives at its own parameter coordinate.
            let repeated = selected_identities
                .insert(leaf.identity.clone(), (source.clone(), leaf.clone()))
                .is_some_and(|(previous, previous_leaf)| {
                    previous_leaf != *leaf
                        || !super::retirements::alternative_residency(&previous, source)
                });
            if leaf_referent(types, parameter.structural_type, relative) != Some(leaf.referent)
                || !selected_sources.insert(source.clone())
                || repeated
                || !destinations.insert(destination.clone())
            {
                return Err(invalid());
            }
            moved.push((source.clone(), destination, leaf.clone(), parameter.clone()));
        }
    }
    if destinations.iter().any(|destination| {
        custody
            .retired
            .contains_key(&OccurrenceIdentity::parameter(destination))
            || custody.leaves.contains_key(destination) && !selected_sources.contains(destination)
    }) {
        return Err(invalid());
    }
    let identities: Vec<_> = moved
        .iter()
        .map(|(_, destination, leaf, _)| {
            (
                leaf.identity.clone(),
                OccurrenceIdentity::parameter(destination),
            )
        })
        .collect();
    for (source, _, _, _) in &moved {
        custody.leaves.remove(source);
    }
    for (_, destination, mut leaf, parameter) in moved {
        leaf.identity = OccurrenceIdentity::parameter(&destination);
        leaf.contract = CarrierContract::Parameter(parameter);
        custody.leaves.insert(destination, leaf);
    }
    remap_parents(custody, &identities);
    Ok(())
}

/// Only live occurrences are selected. Retired keys keep their generation;
/// their parent links follow an independently selected live ancestor. A parent
/// re-homed as several case alternatives links to every alternative.
pub(super) fn remap_parents(
    custody: &mut Custody,
    identities: &[(OccurrenceIdentity, OccurrenceIdentity)],
) {
    for leaf in custody.leaves.values_mut().chain(
        custody
            .retired
            .values_mut()
            .map(|retired| &mut retired.leaf),
    ) {
        let mut parents = Vec::with_capacity(leaf.parents.len());
        for parent in &leaf.parents {
            let len = parents.len();
            parents.extend(
                identities
                    .iter()
                    .filter(|(source, _)| source == parent)
                    .map(|(_, destination)| destination.clone()),
            );
            if parents.len() == len {
                parents.push(parent.clone());
            }
        }
        parents.sort();
        parents.dedup();
        leaf.parents = parents;
    }
}
