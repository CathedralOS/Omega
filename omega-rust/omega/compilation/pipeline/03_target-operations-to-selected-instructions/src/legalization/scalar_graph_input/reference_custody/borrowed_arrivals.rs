//! Borrowed addresses travel with the actual edge's owned permission roster.
//! These aliases authenticate addresses; they neither select pointer bits nor
//! grant a new observation of a suspended parent.
use super::*;
use abstract_operations::AbstractStructuralBinding;
use terminal_psi::StructuralParameterDeclaration;

type Sources = BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>;
type Identity = (PlaceId, Vec<StructuralPathSegment>);

fn canonicalize(sources: &mut Vec<StructuralArgument>) {
    sources.sort_by(|left, right| {
        (&left.place, &left.path, left.access).cmp(&(&right.place, &right.path, right.access))
    });
    sources.dedup();
}

/// Flatten against the pre-transfer state. Never reinterpret an unresolved
/// borrowed parameter as an activation-local owning home.
fn flatten(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Vec<StructuralArgument>> {
    let declared = function.structural_parameter(source.place).or_else(|| {
        function
            .block_structural_parameters()
            .find(|parameter| parameter.place == source.place)
    });
    if declared.is_some_and(|parameter| !source.access.admits(parameter.access)) {
        return None;
    }
    let expected = storage_projection(function, custody, types, source)?;
    let Some(origins) = custody.borrowed_sources.get(&source.place) else {
        return Some(vec![source.clone()]);
    };
    let mut resolved = Vec::new();
    for origin in origins.as_ref()?.iter() {
        if !source.access.admits(origin.access) {
            return None;
        }
        let mut actual = origin.clone();
        actual.path.extend_from_slice(&source.path);
        actual.access = source.access;
        if !storage_projection(function, custody, types, &actual)?.matches(types.iter(), expected) {
            return None;
        }
        resolved.push(actual);
    }
    (!resolved.is_empty()).then_some(resolved)
}

fn transport(
    before: &Custody,
    after: &Custody,
    source: &StructuralArgument,
    locations: &BTreeMap<Identity, Identity>,
    identities: &BTreeMap<OccurrenceIdentity, Vec<OccurrenceIdentity>>,
    parameters: &[StructuralParameterDeclaration],
    bindings: &[AbstractStructuralBinding],
) -> Option<StructuralArgument> {
    let mut actual = source.clone();
    if source
        .path
        .iter()
        .any(|segment| segment.runtime_index().is_some())
    {
        // A raw selector is not an operation-owned captured address. Dynamic
        // backing is admitted through the live carrier, never this geometry.
        return None;
    }
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let leaf = before.leaf(source.place, &source.path[..crossing])?;
        let location = (source.place, source.path[..crossing].to_vec());
        let destination = locations.get(&location).unwrap_or(&location);
        let arrived = after.leaf(destination.0, &destination.1)?;
        let identity = if locations.contains_key(&location) {
            OccurrenceIdentity::parameter(destination)
        } else {
            leaf.identity.clone()
        };
        if locations.contains_key(&location) {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == destination.0)?;
            if arrived.contract != CarrierContract::Parameter(parameter.clone()) {
                return None;
            }
        }
        let mut parents: Vec<_> = leaf
            .parents
            .iter()
            .flat_map(|parent| {
                identities
                    .get(parent)
                    .cloned()
                    .unwrap_or_else(|| vec![parent.clone()])
            })
            .collect();
        parents.sort();
        parents.dedup();
        if arrived.identity != identity
            || arrived.root != leaf.root
            || arrived.referent != leaf.referent
            || arrived.formations != leaf.formations
            || arrived.parents != parents
        {
            return None;
        }
        actual.place = destination.0;
        actual.path = destination.1.clone();
        actual.path.extend_from_slice(&source.path[crossing..]);
    } else if bindings.iter().any(|binding| {
        binding.argument.access == StructuralAccess::Owned
            && binding.argument.place == source.place
            && source.path.starts_with(&binding.argument.path)
    }) {
        // Moving a backing home is not moving a reference permission. The
        // carrier rule cannot manufacture authority for its relocated storage.
        return None;
    }
    Some(actual)
}

pub(super) fn bind(
    function: &PsiOptimizationFunction,
    before: &Custody,
    after: &mut Custody,
    bindings: &[AbstractStructuralBinding],
    parameters: &[StructuralParameterDeclaration],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    if bindings.len() != parameters.len() {
        return Err(invalid());
    }
    // Case alternatives of one carrier hold one occurrence, which arrives at
    // one parameter coordinate per alternative.
    let mut identities = BTreeMap::<_, Vec<_>>::new();
    let mut selected = BTreeMap::new();
    let mut locations = BTreeMap::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if binding.argument.access != StructuralAccess::Owned
            || parameter.access != StructuralAccess::Owned
        {
            continue;
        }
        for (location, leaf) in &before.leaves {
            if location.0 != binding.argument.place {
                continue;
            }
            if let Some(relative) = owned_relative_path(&location.1, &binding.argument.path) {
                let destination = (parameter.place, relative.to_vec());
                let repeated = selected
                    .insert(leaf.identity.clone(), (location, leaf))
                    .is_some_and(|(previous, previous_leaf)| {
                        previous_leaf != leaf
                            || !super::retirements::alternative_residency(previous, location)
                    });
                identities
                    .entry(leaf.identity.clone())
                    .or_default()
                    .push(OccurrenceIdentity::parameter(&destination));
                if repeated || locations.insert(location.clone(), destination).is_some() {
                    return Err(invalid());
                }
            }
        }
    }
    let mut sources: Sources = before
        .borrowed_sources
        .iter()
        .map(|(place, origins)| {
            let transported = origins.as_ref().and_then(|origins| {
                origins
                    .iter()
                    .map(|origin| {
                        transport(
                            before,
                            after,
                            origin,
                            &locations,
                            &identities,
                            parameters,
                            bindings,
                        )
                    })
                    .collect()
            });
            (*place, transported)
        })
        .collect();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if !sources.contains_key(&parameter.place) {
            continue;
        }
        if !parameter.access.admits(binding.argument.access) {
            return Err(invalid());
        }
        let expected = optimization_unit_semantics::StructuralStorageProjection::Endpoint(
            parameter.structural_type,
        );
        let mut origins = flatten(function, before, types, &binding.argument).and_then(|origins| {
            origins
                .into_iter()
                .map(|origin| {
                    let mut actual = transport(
                        before,
                        after,
                        &origin,
                        &locations,
                        &identities,
                        parameters,
                        bindings,
                    )?;
                    actual.access = parameter.access;
                    storage_projection(function, after, types, &actual)?
                        .matches(types.iter(), expected)
                        .then_some(actual)
                })
                .collect::<Option<Vec<_>>>()
        });
        if let Some(origins) = &mut origins {
            canonicalize(origins);
        }
        sources.insert(parameter.place, origins);
    }
    after.borrowed_sources = std::sync::Arc::new(sources);
    Ok(())
}

/// Every arrival contributes; unknown on any path stays unknown. Selecting a
/// carrier preserves its correlation, while direct backing alternatives union.
pub(super) fn merge(left: &mut Custody, right: &Custody) -> Result<(), LegalizationError> {
    if !left
        .borrowed_sources
        .keys()
        .eq(right.borrowed_sources.keys())
    {
        return Err(invalid());
    }
    let sources = std::sync::Arc::make_mut(&mut left.borrowed_sources);
    for (place, incoming) in right.borrowed_sources.iter() {
        let existing = sources.get_mut(place).ok_or_else(invalid)?;
        if let (Some(existing), Some(incoming)) = (existing.as_mut(), incoming.as_ref()) {
            existing.extend_from_slice(incoming);
            canonicalize(existing);
        } else {
            *existing = None;
        }
    }
    Ok(())
}

/// Operation-local carrier moves preserve permission identity. Rewrite only
/// aliases crossing a descriptor at an exactly authenticated moved location.
pub(super) fn relocate(custody: &mut Custody, locations: &BTreeMap<Identity, Identity>) {
    if locations.is_empty() {
        return;
    }
    for origins in std::sync::Arc::make_mut(&mut custody.borrowed_sources)
        .values_mut()
        .flatten()
    {
        for origin in origins.iter_mut() {
            let Some(crossing) = origin
                .path
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent)
            else {
                continue;
            };
            let source = (origin.place, origin.path[..crossing].to_vec());
            if let Some(destination) = locations.get(&source) {
                let suffix = origin.path[crossing..].to_vec();
                origin.place = destination.0;
                origin.path = destination.1.clone();
                origin.path.extend(suffix);
            }
        }
        canonicalize(origins);
    }
}
