//! Preserve actual receiving occurrences separately from transported formations.
//! Only authenticated owned bindings select parameter occurrences. Their roots,
//! immediate parents and complete formation alternatives converge; a missing or
//! unrelated surviving leaf never disappears through an availability intersection.

use super::{
    Custody, Leaf, LegalizationError, OperationId, PlaceId, PsiOptimizationFunction, Root,
    StructuralAccess, StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, invalid, leaf_referent,
};
use semantic_vocabulary::{BlockId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralParameterDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CarrierContract {
    Operation {
        operation: OperationId,
        result: StructuralOperationResult,
    },
    Parameter(StructuralParameterDeclaration),
}

impl CarrierContract {
    pub(super) fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Operation { result, .. } => result.structural_type,
            Self::Parameter(parameter) => parameter.structural_type,
        }
    }

    pub(super) fn matches_declaration(
        &self,
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> bool {
        match self {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Formation {
    pub(super) identity: (PlaceId, Vec<StructuralPathSegment>),
    pub(super) contract: CarrierContract,
}

impl Leaf {
    pub(super) fn formed(
        identity: (PlaceId, Vec<StructuralPathSegment>),
        root: Root,
        parents: Vec<(PlaceId, Vec<StructuralPathSegment>)>,
        operation: OperationId,
        result: &StructuralOperationResult,
        referent: StructuralTypeId,
    ) -> Self {
        let contract = CarrierContract::Operation {
            operation,
            result: result.clone(),
        };
        Self {
            identity: identity.clone(),
            root,
            parents,
            contract: contract.clone(),
            formations: vec![Formation { identity, contract }],
            referent,
        }
    }

    /// A copy or newly selected call result forms a new permission occurrence.
    /// Its backing and parent obligations remain separate from this declaration.
    pub(super) fn reform(&mut self, operation: OperationId, result: &StructuralOperationResult) {
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
    left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Result<Option<()>, LegalizationError> {
    if !left.keys().eq(right.keys()) {
        return Err(invalid());
    }
    for (key, incoming) in right {
        let existing = left.get_mut(key).ok_or_else(invalid)?;
        if existing == incoming {
            continue;
        }
        let Some(parameter) = function
            .block(target)
            .and_then(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == key.0)
            })
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
        else {
            // Nonselected metadata can arrive at different worklist times.
            // Defer rather than union it; convergence still requires equality.
            return Ok(None);
        };
        let contract = CarrierContract::Parameter(parameter.clone());
        if existing.identity != *key
            || incoming.identity != *key
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
    let mut selected_identities = BTreeSet::new();
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
            let Some(relative) = source.1.strip_prefix(binding.argument.path.as_slice()) else {
                continue;
            };
            let destination = (parameter.place, relative.to_vec());
            if leaf_referent(types, parameter.structural_type, relative) != Some(leaf.referent)
                || !selected_sources.insert(source.clone())
                || !selected_identities.insert(leaf.identity.clone())
                || !destinations.insert(destination.clone())
            {
                return Err(invalid());
            }
            moved.push((source.clone(), destination, leaf.clone(), parameter.clone()));
        }
    }
    if destinations.iter().any(|destination| {
        custody.leaves.contains_key(destination) && !selected_sources.contains(destination)
    }) {
        return Err(invalid());
    }
    let identities: Vec<_> = moved
        .iter()
        .map(|(_, destination, leaf, _)| (leaf.identity.clone(), destination.clone()))
        .collect();
    for (source, _, _, _) in &moved {
        custody.leaves.remove(source);
    }
    for (_, destination, mut leaf, parameter) in moved {
        leaf.identity = destination.clone();
        leaf.contract = CarrierContract::Parameter(parameter);
        custody.leaves.insert(destination, leaf);
    }
    for leaf in custody.leaves.values_mut() {
        for parent in &mut leaf.parents {
            if let Some((_, destination)) = identities.iter().find(|(source, _)| source == parent) {
                *parent = destination.clone();
            }
        }
        leaf.parents.sort();
        leaf.parents.dedup();
    }
    Ok(())
}
