//! Follow nonescaping borrowed call paths through their current live descriptors.
//! Every inner coordinate must rejoin concrete typed storage, while intermediate
//! descriptor dependencies remain separate from the final borrowed origins.

use super::{
    Custody, LegalizationError, PsiOptimizationFunction, Root, StructuralAccess,
    StructuralArgument, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    invalid, leaf_type, shared_source_compatible, source_contract,
};

/// A nonescaping call follows live descriptor custody at every crossing.
/// Intermediate origins become concrete cursors only after their current
/// storage contract matches the declared referent. Opaque ingress permissions
/// remain valid final origins, but cannot invent another stored descriptor.
pub(super) struct CallReferenceCustody {
    pub(super) origins: Vec<Root>,
    pub(super) dependencies: Vec<Root>,
}

pub(super) fn call_reference_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
) -> Result<CallReferenceCustody, LegalizationError> {
    let invalid = || invalid();
    if argument.access == StructuralAccess::Owned {
        return Err(invalid());
    }
    let source_access = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.structural_parameters.iter()),
        )
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.access)
        .unwrap_or(StructuralAccess::Owned);
    if source_access == StructuralAccess::WriteOnlyBorrow
        || source_access == StructuralAccess::SharedBorrow
            && argument.access != StructuralAccess::SharedBorrow
    {
        return Err(invalid());
    }
    let mut current = source_contract(function, custody, argument.place).ok_or_else(invalid)?;
    let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
    let cursor_budget = custody
        .leaves
        .len()
        .checked_mul(argument.path.len())
        .ok_or_else(invalid)?;
    let mut dependencies = Vec::new();
    let mut prefix_start = 0;
    for (position, segment) in argument.path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let prefix = &argument.path[prefix_start..position];
        let reference_type = leaf_type(types, current, prefix).ok_or_else(invalid)?;
        let StructuralTypeShape::Reference { referent, access } = types
            .iter()
            .find(|declaration| declaration.id == reference_type)
            .ok_or_else(invalid)?
            .shape
        else {
            return Err(invalid());
        };
        let further_crossing =
            argument.path[position + 1..].contains(&StructuralPathSegment::Referent);
        if !(access == argument.access
            || access == StructuralAccess::MutableBorrow
                && matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
            || further_crossing && access == StructuralAccess::WriteOnlyBorrow
        {
            return Err(invalid());
        }
        let mut origins = Vec::new();
        let mut next_cursors = Vec::new();
        for (place, mut path, ancestors) in cursors {
            path.extend_from_slice(prefix);
            if source_contract(function, custody, place)
                .and_then(|contract| leaf_type(types, contract, &path))
                != Some(reference_type)
            {
                return Err(invalid());
            }
            let leaf = custody
                .leaves
                .get(&(place, path.clone()))
                .ok_or_else(invalid)?;
            if leaf.referent != referent
                || custody.leaves.iter().any(|((carrier, path), child)| {
                    child.parent.as_ref() == Some(&leaf.identity)
                        && !shared_source_compatible(
                            function,
                            custody,
                            types,
                            carrier,
                            path,
                            argument.access,
                        )
                })
            {
                return Err(invalid());
            }
            // A descriptor is stored data too. Direct access cannot bypass
            // a live enclosing loan; traversing that exact ancestor is the
            // only non-shared exemption, retained separately on each cursor.
            if custody
                .leaves
                .iter()
                .any(|((carrier, carrier_path), enclosing)| {
                    !ancestors.contains(&enclosing.identity)
                        && !shared_source_compatible(
                            function,
                            custody,
                            types,
                            carrier,
                            carrier_path,
                            argument.access,
                        )
                        && enclosing.root.paths().iter().any(|origin_path| {
                            let origin_place = enclosing.root.place();
                            origin_place == place
                                && source_contract(function, custody, origin_place)
                                    .and_then(|contract| leaf_type(types, contract, origin_path))
                                    == Some(enclosing.referent)
                                && terminal_semantics::structural_paths_may_overlap(
                                    origin_path,
                                    &path,
                                )
                        })
                })
            {
                return Err(invalid());
            }
            dependencies.push(Root::Ingress(place, path));
            origins.push(leaf.root.clone());
            if further_crossing {
                dependencies.push(leaf.root.clone());
                let mut ancestors = ancestors;
                ancestors.push(leaf.identity.clone());
                for path in leaf.root.paths() {
                    let place = leaf.root.place();
                    if source_contract(function, custody, place)
                        .and_then(|contract| leaf_type(types, contract, &path))
                        != Some(referent)
                    {
                        return Err(invalid());
                    }
                    let cursor = (place, path, ancestors.clone());
                    if !next_cursors.contains(&cursor) {
                        if next_cursors.len() >= cursor_budget {
                            return Err(invalid());
                        }
                        next_cursors.push(cursor);
                    }
                }
            }
        }
        if !further_crossing {
            if origins.is_empty()
                || leaf_type(types, referent, &argument.path[position + 1..]).is_none()
            {
                return Err(invalid());
            }
            return Ok(CallReferenceCustody {
                origins,
                dependencies,
            });
        }
        cursors = next_cursors;
        current = referent;
        prefix_start = position + 1;
    }
    Err(invalid())
}
