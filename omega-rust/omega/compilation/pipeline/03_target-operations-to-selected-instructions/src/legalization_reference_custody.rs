/*
We reconstruct and audit reference custody across scalar-graph operations,
tracking borrowed arrivals, liveness and restoration obligations, call paths,
carrier reads, descriptor backings, lifetime debts, observations, occurrences,
resident joins, retirements, and stored calls.
*/

use crate::LegalizationError;
use crate::legalization::scalar_graph_input::structural_case::source_result;
use crate::legalization::scalar_graph_input::{AbstractOperation, PsiOptimizationFunction};
use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::call_source;
use optimization_unit_semantics::CallRegion;
use optimization_unit_semantics::access_references::{borrowed_view_formation, validate_backing};
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terminal_psi::{
    ReferenceOriginProjection, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
};

pub(crate) mod borrowed_arrivals {
    use super::*;
    use abstract_operations::AbstractStructuralBinding;
    use optimization_unit_semantics::StructuralStorageProjection;
    use terminal_psi::StructuralParameterDeclaration;

    pub(crate) mod liveness {
        use super::{BlockId, Custody, PlaceId, PsiOptimizationFunction};
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Default)]
        struct BlockUses {
            parameters: BTreeSet<PlaceId>,
            inputs: BTreeSet<PlaceId>,
            successors: BTreeSet<BlockId>,
        }

        pub(crate) fn live_coordinates(
            function: &PsiOptimizationFunction,
        ) -> Option<BTreeMap<BlockId, BTreeSet<PlaceId>>> {
            let blocks = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        BlockUses {
                            parameters: block
                                .structural_parameters
                                .iter()
                                .map(|parameter| parameter.place)
                                .collect(),
                            inputs: block
                                .nodes
                                .iter()
                                .flat_map(|node| {
                                    optimization_unit_semantics::operation_place_inputs(
                                        &node.operation,
                                    )
                                })
                                .collect(),
                            successors: block.successors().map(|edge| edge.target).collect(),
                        },
                    )
                })
                .collect();
            live_blocks(&blocks)
        }

        fn live_blocks(
            blocks: &BTreeMap<BlockId, BlockUses>,
        ) -> Option<BTreeMap<BlockId, BTreeSet<PlaceId>>> {
            if blocks
                .values()
                .flat_map(|block| &block.successors)
                .any(|target| !blocks.contains_key(target))
            {
                return None;
            }
            let mut live = blocks
                .keys()
                .map(|block| (*block, BTreeSet::new()))
                .collect::<BTreeMap<_, _>>();
            loop {
                let mut changed = false;
                for (block, uses) in blocks.iter().rev() {
                    let mut incoming = uses.inputs.clone();
                    for target in &uses.successors {
                        incoming
                            .extend(live[target].difference(&blocks[target].parameters).copied());
                    }
                    let previous = live.get_mut(block).expect("block was initialized");
                    if *previous != incoming {
                        *previous = incoming;
                        changed = true;
                    }
                }
                if !changed {
                    return Some(live);
                }
            }
        }

        pub(crate) fn expire_coordinates(custody: &mut Custody, target: BlockId) {
            let Some(live) = custody
                .descriptor_backing
                .borrowed_live
                .as_ref()
                .and_then(|live| live.get(&target))
            else {
                return;
            };
            for (place, source) in std::sync::Arc::make_mut(&mut custody.borrowed_sources) {
                if !live.contains(place) {
                    *source = None;
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            fn block(number: u64) -> BlockId {
                BlockId::new(number).unwrap()
            }

            #[test]
            fn later_reads_remain_live_through_branches_and_backedges() {
                let place = PlaceId::new(7).unwrap();
                let blocks = BTreeMap::from([
                    (
                        block(1),
                        BlockUses {
                            successors: BTreeSet::from([block(2), block(4)]),
                            ..Default::default()
                        },
                    ),
                    (
                        block(2),
                        BlockUses {
                            successors: BTreeSet::from([block(3)]),
                            ..Default::default()
                        },
                    ),
                    (
                        block(3),
                        BlockUses {
                            inputs: BTreeSet::from([place]),
                            successors: BTreeSet::from([block(1)]),
                            ..Default::default()
                        },
                    ),
                    (block(4), BlockUses::default()),
                ]);
                let live = live_blocks(&blocks).unwrap();
                for number in [1, 2, 3] {
                    assert!(live[&block(number)].contains(&place));
                }
                assert!(!live[&block(4)].contains(&place));
            }

            #[test]
            fn incomplete_graph_proves_no_coordinate_dead() {
                let blocks = BTreeMap::from([(
                    block(1),
                    BlockUses {
                        successors: BTreeSet::from([block(2)]),
                        ..Default::default()
                    },
                )]);
                assert!(live_blocks(&blocks).is_none());
            }

            #[test]
            fn rebinding_a_coordinate_does_not_keep_its_previous_instance_live() {
                let place = PlaceId::new(7).unwrap();
                let blocks = BTreeMap::from([
                    (
                        block(1),
                        BlockUses {
                            successors: BTreeSet::from([block(2)]),
                            ..Default::default()
                        },
                    ),
                    (
                        block(2),
                        BlockUses {
                            parameters: BTreeSet::from([place]),
                            inputs: BTreeSet::from([place]),
                            ..Default::default()
                        },
                    ),
                ]);
                let live = live_blocks(&blocks).unwrap();
                assert!(!live[&block(1)].contains(&place));
                assert!(live[&block(2)].contains(&place));
            }
        }
    }
    pub(crate) mod restoration {
        use super::*;

        pub(crate) fn complete_cleanup(
            function: &PsiOptimizationFunction,
            custody: &mut Custody,
            types: &crate::native_geometry::NativeGeometry<'_>,
            event: retirements::DisposalEvent,
        ) -> Result<(), LegalizationError> {
            if !custody
                .retired
                .values()
                .any(|retired| retired.disposals.contains(&event))
            {
                return Ok(());
            }
            let mut sources = custody.borrowed_sources.as_ref().clone();
            for alternatives in sources.values_mut().flatten() {
                for source in alternatives.iter_mut() {
                    if source.parents.is_empty()
                        || source.placed.is_some()
                        || !entry_authorizes(function, custody, types, source)
                    {
                        continue;
                    }
                    source.parents = restored_parents(custody, &source.parents, event)?;
                }
                canonicalize(alternatives);
            }
            custody.borrowed_sources = std::sync::Arc::new(sources);
            Ok(())
        }

        fn restored_parents(
            custody: &Custody,
            parents: &[OccurrenceIdentity],
            event: retirements::DisposalEvent,
        ) -> Result<Vec<OccurrenceIdentity>, LegalizationError> {
            let mut restored = Vec::new();
            for grant in parents {
                let mut pending = vec![(grant, BTreeSet::new())];
                while let Some((grant, mut ancestry)) = pending.pop() {
                    if !ancestry.insert(grant) {
                        return Err(LegalizationError::ReferenceCompletion {
                            reason: terminal_semantics::ReferenceCompletionError::CyclicAncestry,
                        });
                    }
                    let closed = custody.retired.get(grant).filter(|retired| {
                        retired.disposals.contains(&event)
                            && retired.leaf.lifetime_debts.is_empty()
                            && retired.leaf.placed.is_none()
                            && !retired.leaf.backing.is_empty()
                            && retired.leaf.backing.iter().all(|backing| {
                                matches!(backing, descriptor_backing::Backing::Source(root)
                                if *root == retired.leaf.root)
                            })
                            && !custody.leaves.values().any(|child| {
                                child.identity == *grant
                                    || custody.descends_from(child, grant)
                                    || custody.lifetime_depends_on(child, grant)
                            })
                    });
                    if let Some(closed) = closed {
                        pending.extend(
                            closed
                                .leaf
                                .parents
                                .iter()
                                .map(|parent| (parent, ancestry.clone())),
                        );
                    } else {
                        restored.push(grant.clone());
                    }
                }
            }
            restored.sort();
            restored.dedup();
            Ok(restored)
        }

        fn entry_authorizes(
            function: &PsiOptimizationFunction,
            custody: &Custody,
            types: &crate::native_geometry::NativeGeometry<'_>,
            source: &Source,
        ) -> bool {
            let origins = source.root.origins();
            !origins.is_empty()
                && origins.iter().all(|origin| {
                    let target_operations::TargetReferenceOriginBase::Storage { root } =
                        origin.base
                    else {
                        return false;
                    };
                    let Some(parameter) = function.structural_parameter(root) else {
                        return false;
                    };
                    if parameter.access == StructuralAccess::Owned
                        || !access_covers(parameter.access, source.access)
                        || origin.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    {
                        return false;
                    }
                    let Some(path) = origin
                        .path
                        .iter()
                        .map(|part| match part {
                            terminal_psi::ReferenceOriginProjection::Exact(segment)
                                if *segment != StructuralPathSegment::Referent =>
                            {
                                Some(segment.clone())
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                    else {
                        return false;
                    };
                    storage_projection(
                        function,
                        custody,
                        types,
                        &StructuralArgument {
                            place: root,
                            path,
                            access: source.access,
                        },
                    )
                    .is_some_and(|projection| projection.matches(types.iter(), source.projection))
                })
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::*;

            fn leaf(number: u64, parents: Vec<OccurrenceIdentity>) -> Leaf {
                let result = StructuralOperationResult {
                    place: PlaceId::new(number).unwrap(),
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: vec![],
                    claims: vec![],
                };
                Leaf::formed(
                    (result.place, vec![]),
                    Root::Place(PlaceId::new(99).unwrap()),
                    parents,
                    OperationId::new(number).unwrap(),
                    &result,
                    result.structural_type,
                )
            }

            fn event(number: u64) -> retirements::DisposalEvent {
                retirements::DisposalEvent::Edge(EdgeId::new(number).unwrap())
            }

            #[test]
            fn only_selected_disposal_restores_original_parent() {
                let parent = leaf(1, vec![]).identity;
                let child = leaf(2, vec![parent.clone()]);
                let grant = child.identity.clone();
                let mut custody = Custody::default();
                custody.retire(child, event(1)).unwrap();
                assert_eq!(
                    restored_parents(&custody, std::slice::from_ref(&grant), event(2)).unwrap(),
                    std::slice::from_ref(&grant)
                );
                assert_eq!(
                    restored_parents(&custody, &[grant], event(1)).unwrap(),
                    [parent]
                );
            }

            #[test]
            fn live_descendants_and_independent_validity_keep_the_grant() {
                let closed = leaf(1, vec![]);
                let grant = closed.identity.clone();
                let mut custody = Custody::default();
                custody.retire(closed, event(1)).unwrap();
                let child = leaf(2, vec![grant.clone()]);
                custody.leaves.insert((child.identity.place, vec![]), child);
                assert_eq!(
                    restored_parents(&custody, std::slice::from_ref(&grant), event(1)).unwrap(),
                    std::slice::from_ref(&grant)
                );
                custody.leaves.clear();
                custody
                    .retired
                    .get_mut(&grant)
                    .unwrap()
                    .leaf
                    .lifetime_debts
                    .push(lifetime_debts::Debt {
                        backing: vec![],
                        access: StructuralAccess::SharedBorrow,
                        parents: vec![],
                    });
                assert_eq!(
                    restored_parents(&custody, std::slice::from_ref(&grant), event(1)).unwrap(),
                    [grant]
                );
            }

            #[test]
            fn different_backing_and_cyclic_ancestry_do_not_disappear() {
                let closed = leaf(1, vec![]);
                let grant = closed.identity.clone();
                let mut custody = Custody::default();
                custody.retire(closed, event(1)).unwrap();
                custody.retired.get_mut(&grant).unwrap().leaf.backing.push(
                    descriptor_backing::Backing::Source(Root::Place(PlaceId::new(98).unwrap())),
                );
                assert_eq!(
                    restored_parents(&custody, std::slice::from_ref(&grant), event(1)).unwrap(),
                    std::slice::from_ref(&grant)
                );
                let retired = custody.retired.get_mut(&grant).unwrap();
                retired.leaf.backing.pop();
                retired.leaf.parents.push(grant.clone());
                assert!(matches!(
                    restored_parents(&custody, &[grant], event(1)),
                    Err(LegalizationError::ReferenceCompletion {
                        reason: terminal_semantics::ReferenceCompletionError::CyclicAncestry
                    })
                ));
            }
        }
    }
    pub(crate) use liveness::{expire_coordinates, live_coordinates};
    pub(crate) use restoration::complete_cleanup;

    pub(crate) type Sources = BTreeMap<PlaceId, Option<Vec<Source>>>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Source {
        root: Root,
        projection: StructuralStorageProjection,
        access: StructuralAccess,
        pub(crate) parents: Vec<OccurrenceIdentity>,
        pub(crate) placed: Option<target_operations::TargetAccessInterpretation>,
    }

    fn canonicalize(sources: &mut Vec<Source>) {
        sources.sort_by(|left, right| {
            (&left.root, &left.parents, left.access).cmp(&(
                &right.root,
                &right.parents,
                right.access,
            ))
        });
        sources.dedup();
    }

    pub(crate) struct CapturedStorage {
        pub(crate) argument: StructuralArgument,
        pub(crate) grants: Vec<OccurrenceIdentity>,
    }

    pub(crate) fn captured_storage(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &StructuralArgument,
    ) -> Option<Vec<CapturedStorage>> {
        let Some(origins) = custody.borrowed_sources.get(&source.place) else {
            return Some(vec![CapturedStorage {
                argument: source.clone(),
                grants: Vec::new(),
            }]);
        };
        let mut resolved = Vec::new();
        for origin in origins.as_ref()? {
            if !source.access.admits(origin.access) {
                return None;
            }
            let StructuralStorageProjection::Endpoint(referent) = origin.projection else {
                return None;
            };
            for storage in storage_origins(function, custody, types, &origin.root, referent).ok()? {
                let mut path = storage.exact_path()?;
                path.extend_from_slice(&source.path);
                resolved.push(CapturedStorage {
                    argument: StructuralArgument {
                        place: storage.base.root_place()?,
                        path,
                        access: source.access,
                    },
                    grants: origin.parents.clone(),
                });
            }
        }
        (!resolved.is_empty()).then_some(resolved)
    }

    pub(crate) fn normalize(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &StructuralArgument,
    ) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
        let Some(origins) = custody.borrowed_sources.get(&source.place) else {
            return normalized_direct_source(function, custody, types, source);
        };
        let origins = origins
            .as_ref()
            .filter(|origins| !origins.is_empty())
            .ok_or_else(|| invalid())?;
        if source.path.contains(&StructuralPathSegment::Referent) {
            return normalized_direct_source(function, custody, types, source);
        }
        storage_projection(function, custody, types, source).ok_or_else(|| invalid())?;
        let mut candidates = Vec::with_capacity(origins.len());
        for origin in origins {
            if !source.access.admits(origin.access) {
                return Err(invalid());
            }
            candidates.push((origin.root.projected(&source.path), origin.parents.clone()));
        }
        combine_candidate_origins(candidates)
    }

    pub(crate) fn bind(
        function: &PsiOptimizationFunction,
        before: &Custody,
        after: &mut Custody,
        bindings: &[AbstractStructuralBinding],
        parameters: &[StructuralParameterDeclaration],
        types: &crate::native_geometry::NativeGeometry<'_>,
    ) -> Result<(), LegalizationError> {
        if bindings.len() != parameters.len() {
            return Err(invalid());
        }
        let mut identities = Vec::new();
        for (binding, parameter) in bindings.iter().zip(parameters) {
            if binding.argument.access != StructuralAccess::Owned
                || parameter.access != StructuralAccess::Owned
            {
                continue;
            }
            for (location, leaf) in &before.leaves {
                if location.0 == binding.argument.place
                    && let Some(relative) = owned_relative_path(&location.1, &binding.argument.path)
                {
                    let destination = (parameter.place, relative.to_vec());
                    let arrived = after.leaves.get(&destination).ok_or_else(|| invalid())?;
                    if arrived.identity != OccurrenceIdentity::parameter(&destination)
                        || arrived.contract != CarrierContract::Parameter(parameter.clone())
                    {
                        return Err(invalid());
                    }
                    identities.push((leaf.identity.clone(), arrived.identity.clone()));
                }
            }
        }
        let mut sources = before.borrowed_sources.as_ref().clone();
        for (binding, parameter) in bindings.iter().zip(parameters) {
            if !sources.contains_key(&parameter.place) {
                continue;
            }
            if !parameter.access.admits(binding.argument.access) {
                return Err(invalid());
            }
            let captured = (|| {
                let projection = storage_projection(function, before, types, &binding.argument)?;
                if !projection.matches(
                    types.iter(),
                    StructuralStorageProjection::Endpoint(parameter.structural_type),
                ) {
                    return None;
                }
                let (root, parents) = normalize(function, before, types, &binding.argument).ok()?;
                for binding in bindings
                    .iter()
                    .filter(|binding| binding.argument.access == StructuralAccess::Owned)
                {
                    for origin in root.origins() {
                        if !matches!(origin.base, target_operations::TargetReferenceOriginBase::Storage { root } if root == binding.argument.place)
                        {
                            continue;
                        }
                        let cursors = storage_origins(
                            function,
                            before,
                            types,
                            &Root::Alternatives(vec![origin]),
                            parameter.structural_type,
                        )
                        .ok()?;
                        for cursor in cursors {
                            let path = cursor.exact_path()?;
                            if owned_relative_path(&path, &binding.argument.path).is_some() {
                                return None;
                            }
                        }
                    }
                }
                Some(vec![Source {
                    root,
                    projection: StructuralStorageProjection::Endpoint(parameter.structural_type),
                    access: parameter.access,
                    parents,
                    placed: access_source(function, before, types, &binding.argument),
                }])
            })();
            sources.insert(parameter.place, captured);
        }
        after.borrowed_sources = std::sync::Arc::new(sources);
        remap_parents(after, &identities);
        Ok(())
    }

    pub(crate) fn lends_from(
        custody: &Custody,
        place: PlaceId,
        parent: &OccurrenceIdentity,
        requested: StructuralAccess,
    ) -> bool {
        custody
            .borrowed_sources
            .get(&place)
            .and_then(Option::as_ref)
            .is_some_and(|sources| {
                !sources.is_empty()
                    && sources.iter().all(|source| {
                        requested.admits(source.access)
                            && source
                                .parents
                                .iter()
                                .any(|grant| granting_ancestor(custody, grant, parent))
                    })
            })
    }

    pub(crate) fn granting_ancestor(
        custody: &Custody,
        grant: &OccurrenceIdentity,
        ancestor: &OccurrenceIdentity,
    ) -> bool {
        let mut pending = vec![grant];
        let mut visited = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if identity == ancestor {
                return true;
            }
            if !visited.insert(identity) {
                continue;
            }
            let leaf = custody
                .leaves
                .values()
                .find(|leaf| leaf.identity == *identity)
                .or_else(|| custody.retired.get(identity).map(|retired| &retired.leaf));
            if let Some(leaf) = leaf {
                pending.extend(&leaf.parents);
            }
        }
        false
    }

    pub(crate) fn obligations(custody: &Custody) -> impl Iterator<Item = &OccurrenceIdentity> {
        custody
            .borrowed_sources
            .values()
            .flatten()
            .flatten()
            .flat_map(|source| &source.parents)
    }

    pub(crate) fn remap_parents(
        custody: &mut Custody,
        identities: &[(OccurrenceIdentity, OccurrenceIdentity)],
    ) {
        if identities.is_empty() {
            return;
        }
        for sources in std::sync::Arc::make_mut(&mut custody.borrowed_sources)
            .values_mut()
            .flatten()
        {
            for source in sources.iter_mut() {
                source.parents = source
                    .parents
                    .iter()
                    .flat_map(|parent| {
                        let moved = identities
                            .iter()
                            .filter(|(original, _)| original == parent)
                            .map(|(_, destination)| destination.clone())
                            .collect::<Vec<_>>();
                        if moved.is_empty() {
                            vec![parent.clone()]
                        } else {
                            moved
                        }
                    })
                    .collect();
                source.parents.sort();
                source.parents.dedup();
            }
            canonicalize(sources);
        }
    }

    pub(crate) fn merge(left: &mut Custody, right: &Custody) -> Result<(), LegalizationError> {
        if !left
            .borrowed_sources
            .keys()
            .eq(right.borrowed_sources.keys())
        {
            return Err(invalid());
        }
        let sources = std::sync::Arc::make_mut(&mut left.borrowed_sources);
        for (place, incoming) in right.borrowed_sources.iter() {
            let existing = sources.get_mut(place).ok_or_else(|| invalid())?;
            if let (Some(existing), Some(incoming)) = (existing.as_mut(), incoming.as_ref()) {
                existing.extend_from_slice(incoming);
                canonicalize(existing);
            } else {
                *existing = None;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn occurrence(number: u64) -> OccurrenceIdentity {
            OccurrenceIdentity::parameter(&(PlaceId::new(number).unwrap(), Vec::new()))
        }

        fn source(parent: OccurrenceIdentity, access: StructuralAccess) -> Source {
            Source {
                root: Root::Ingress(PlaceId::new(1).unwrap(), Vec::new()),
                projection: StructuralStorageProjection::Endpoint(
                    StructuralTypeId::new(1).unwrap(),
                ),
                access,
                parents: vec![parent],
                placed: None,
            }
        }

        #[test]
        fn dead_lookup_expires_without_disposing_or_restoring_a_live_grant() {
            let target = BlockId::new(2).unwrap();
            let dead = PlaceId::new(3).unwrap();
            let live = PlaceId::new(4).unwrap();
            let parent = occurrence(5);
            let mut observations = descriptor_backing::Observations::default();
            observations.borrowed_live = Some(BTreeMap::from([(target, BTreeSet::from([live]))]));
            let mut custody = Custody {
                descriptor_backing: std::sync::Arc::new(observations),
                borrowed_sources: std::sync::Arc::new(BTreeMap::from([
                    (
                        dead,
                        Some(vec![source(parent.clone(), StructuralAccess::SharedBorrow)]),
                    ),
                    (
                        live,
                        Some(vec![source(parent, StructuralAccess::SharedBorrow)]),
                    ),
                ])),
                ..Custody::default()
            };
            expire_coordinates(&mut custody, target);
            assert_eq!(custody.borrowed_sources.get(&dead), Some(&None));
            assert!(custody.borrowed_sources[&live].is_some());
            assert!(matches!(
                custody.validate_occurrences(),
                Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::MissingParent,
                })
            ));
            assert!(custody.retired.is_empty());
            std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(live, None);
            assert!(custody.validate_occurrences().is_ok());
            let result = StructuralOperationResult {
                place: PlaceId::new(6).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            };
            let child = Leaf::formed(
                (result.place, Vec::new()),
                Root::Place(dead),
                vec![occurrence(5)],
                OperationId::new(7).unwrap(),
                &result,
                result.structural_type,
            );
            custody.leaves.insert((result.place, Vec::new()), child);
            expire_coordinates(&mut custody, target);
            assert!(matches!(
                custody.validate_occurrences(),
                Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::MissingParent,
                })
            ));
            assert!(custody.retired.is_empty());
        }

        #[test]
        fn unavailable_lookup_liveness_never_proves_a_coordinate_dead() {
            let target = BlockId::new(2).unwrap();
            let place = PlaceId::new(3).unwrap();
            let mut custody = Custody::default();
            std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(
                place,
                Some(vec![source(occurrence(4), StructuralAccess::SharedBorrow)]),
            );
            let before = custody.borrowed_sources.clone();
            expire_coordinates(&mut custody, target);
            assert_eq!(custody.borrowed_sources, before);
        }

        #[test]
        fn borrowed_arrival_requires_exact_parent_not_equal_backing() {
            let place = PlaceId::new(2).unwrap();
            let parent = occurrence(3);
            let mut custody = Custody::default();
            std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(
                place,
                Some(vec![source(
                    parent.clone(),
                    StructuralAccess::MutableBorrow,
                )]),
            );
            macro_rules! lends {
                ($parent:expr, $access:expr) => {
                    lends_from(&custody, place, $parent, $access)
                };
            }
            assert!(lends!(&parent, StructuralAccess::MutableBorrow));
            assert!(!lends!(&occurrence(4), StructuralAccess::MutableBorrow));
            let replaced_parent = OccurrenceIdentity::operation(
                &(PlaceId::new(3).unwrap(), Vec::new()),
                OperationId::new(7).unwrap(),
            );
            assert!(!lends!(&replaced_parent, StructuralAccess::MutableBorrow));
            assert!(!lends!(&parent, StructuralAccess::Owned));
            std::sync::Arc::make_mut(&mut custody.borrowed_sources)
                .get_mut(&place)
                .unwrap()
                .as_mut()
                .unwrap()
                .push(source(occurrence(4), StructuralAccess::MutableBorrow));
            assert!(!lends!(&parent, StructuralAccess::MutableBorrow));
        }

        #[test]
        fn borrowed_arrival_preserves_access_and_unknown_predecessors() {
            let place = PlaceId::new(2).unwrap();
            let parent = occurrence(3);
            let mut custody = Custody::default();
            macro_rules! lends {
                ($access:expr) => {
                    lends_from(&custody, place, &parent, $access)
                };
            }
            for sources in [None, Some(Vec::new())] {
                std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(place, sources);
                assert!(!lends!(StructuralAccess::SharedBorrow));
            }
            std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(
                place,
                Some(vec![source(parent.clone(), StructuralAccess::SharedBorrow)]),
            );
            assert!(lends!(StructuralAccess::SharedBorrow));
            assert!(!lends!(StructuralAccess::MutableBorrow));
            assert!(!lends!(StructuralAccess::WriteOnlyBorrow));
        }
    }
}
pub(crate) mod carrier_reads {
    use super::{
        Custody, Leaf, PlaceId, PsiOptimizationFunction, ReferenceOriginProjection,
        StructuralAccess, StructuralPathSegment, access_covers, backing_storage_access_conflicts,
        borrowed_arrivals, descriptor_backing, find_structural_type, leaf_reference_access,
        source_contract,
    };

    pub(crate) struct CarrierStorageRead<'a> {
        function: &'a PsiOptimizationFunction,
        custody: &'a Custody,
        types: &'a crate::native_geometry::NativeGeometry<'a>,
        place: PlaceId,
        path: &'a [StructuralPathSegment],
        inline_entry: bool,
        storage_path: Vec<ReferenceOriginProjection>,
    }

    impl<'a> CarrierStorageRead<'a> {
        pub(crate) fn new(
            function: &'a PsiOptimizationFunction,
            custody: &'a Custody,
            types: &'a crate::native_geometry::NativeGeometry<'a>,
            place: PlaceId,
            path: &'a [StructuralPathSegment],
        ) -> Self {
            let inline_entry = !custody.borrowed_sources.contains_key(&place)
                && function
                    .structural_parameter(place)
                    .is_some_and(|parameter| {
                        access_covers(parameter.access, StructuralAccess::SharedBorrow)
                            && terminal_semantics::projection_stays_in_inline_storage(
                                types.iter(),
                                parameter.structural_type,
                                path,
                            )
                    });
            let storage_path = if inline_entry {
                path.iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect()
            } else {
                Vec::new()
            };
            Self {
                function,
                custody,
                types,
                place,
                path,
                inline_entry,
                storage_path,
            }
        }

        pub(crate) fn conflicts(&self, reference: &Leaf) -> bool {
            if borrowed_arrivals::lends_from(
                self.custody,
                self.place,
                &reference.identity,
                StructuralAccess::SharedBorrow,
            ) || leaf_reference_access(self.types, reference)
                == Some(StructuralAccess::SharedBorrow)
            {
                return false;
            }
            if !self.inline_entry {
                return reference.backing.iter().any(|backing| {
                    backing_storage_access_conflicts(
                        self.function,
                        self.custody,
                        self.types,
                        backing,
                        self.place,
                        self.path,
                    )
                });
            }
            let origin_overlaps = |origin_path: &[ReferenceOriginProjection]| {
                if source_contract(self.function, self.custody, self.place)
                    .and_then(|root| {
                        terminal_semantics::captured_reference_origin_type(
                            self.types.iter(),
                            root,
                            origin_path,
                        )
                    })
                    .and_then(|identity| find_structural_type(self.types, identity))
                    .is_none()
                {
                    return true;
                }
                terminal_semantics::reference_origin_paths_may_overlap(
                    &self.storage_path,
                    origin_path,
                )
            };
            reference.backing.iter().any(|backing| {
                descriptor_backing::overlaps_inline_carrier_read(
                    backing,
                    self.function.machine,
                    self.place,
                    self.path,
                    &origin_overlaps,
                )
            })
        }
    }
}
use carrier_reads::CarrierStorageRead;
pub(crate) mod descriptor_backing {
    use super::*;
    use optimization_unit_semantics::descriptor_results::captured_scalar_ordering;
    use terminal_semantics::borrowed_storage::{Projection, Region};
    use terminal_semantics::descriptor_lifetimes::{DescriptorPosition, descriptor_path};
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorCallOrigin, DescriptorSlotKind, DescriptorSourceOrigin,
        descriptor_machine, descriptor_slot_inventory,
    };

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum Backing {
        Source(Root),
        Descriptor {
            origin: DescriptorSourceOrigin,
            geometry: DescriptorCallOrigin,
            return_path: Option<Vec<ReferenceOriginProjection>>,
        },
    }

    type Observation = (OperationId, PlaceId, Vec<StructuralPathSegment>);
    #[derive(Default)]
    pub(crate) struct Observations {
        pub(crate) borrowed_live: Option<BTreeMap<BlockId, BTreeSet<PlaceId>>>,
        pub(crate) flow:
            BTreeMap<DescriptorPosition, terminal_semantics::descriptor_results::FlowFeasibility>,
        pub(crate) edges: BTreeMap<
            semantic_vocabulary::EdgeId,
            terminal_semantics::descriptor_results::FlowFeasibility,
        >,
        inactive_cases: BTreeSet<(
            DescriptorPosition,
            terminal_semantics::descriptor_lifetimes::DescriptorLocation,
            semantic_vocabulary::StructuralCaseId,
        )>,
        sources: BTreeMap<Observation, Option<Vec<Backing>>>,
        positions: BTreeMap<OperationId, DescriptorPosition>,
        ordering: Option<terminal_semantics::scalar_ordering::ScalarOrdering>,
    }

    impl std::fmt::Debug for Observations {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("Observations")
                .field("sources", &self.sources)
                .field("positions", &self.positions)
                .finish_non_exhaustive()
        }
    }

    pub(crate) fn prepare(
        function: &PsiOptimizationFunction,
        descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
    ) -> Result<Observations, LegalizationError> {
        let plan = descriptor_inputs.plan();
        let unit = descriptor_inputs.unit();
        let types = plan.geometry();
        if !function
            .structural_parameters
            .iter()
            .any(|parameter| contains_reference(types, parameter.structural_type))
            && !function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    node.operation
                        .structural_result()
                        .is_some_and(|(_, result)| {
                            contains_reference(types, result.structural_type)
                        })
                })
        {
            return Ok(Observations::default());
        }
        let graph = descriptor_inputs.views();
        let mut observations = Observations::default();
        let functions = unit
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect();
        observations.ordering = Some(captured_scalar_ordering(function, &functions));
        let Some(graph) = graph else {
            return Ok(observations);
        };
        let current = descriptor_machine(graph, function.machine).ok_or_else(|| invalid())?;
        observations.borrowed_live = borrowed_arrivals::live_coordinates(function);
        let conditions = current
            .case_successors
            .values()
            .flatten()
            .map(|(predicate, _)| {
                (
                    (predicate.subject.clone(), predicate.case),
                    predicate.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect::<Vec<_>>();
        let mut analysis = DescriptorAnalysis::new(types, graph);
        observations.flow = analysis.flow_feasibility(function.machine);
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let position = (block.id, u32::try_from(node_index).map_err(|_| invalid())?);
                for predicate in &conditions {
                    capture_predicate_feasibility(
                        &mut observations,
                        &mut analysis,
                        function.machine,
                        position,
                        predicate,
                    );
                }
                for (ordinal, edge) in node.successors.iter().enumerate() {
                    if observations
                        .edges
                        .insert(
                            edge.psi_edge,
                            analysis.edge_feasibility(function.machine, position, ordinal),
                        )
                        .is_some()
                    {
                        return Err(invalid());
                    }
                }
                let Some(operation) = node.operation.psi_operation() else {
                    continue;
                };
                if observations.positions.insert(operation, position).is_some() {
                    return Err(invalid());
                }
                let step = current.flow.get(&position).ok_or_else(|| invalid())?;
                for input in step
                    .inputs
                    .iter()
                    .chain(step.derivations.iter().map(|(source, _)| source))
                    .chain(step.successors.iter().flat_map(|edge| {
                        edge.bindings
                            .iter()
                            .filter_map(|binding| binding.source.exact_location())
                    }))
                {
                    capture_case_feasibility(
                        &mut observations,
                        &mut analysis,
                        function.machine,
                        position,
                        input,
                    );
                }
                let mut slots = BTreeSet::new();
                for input in &step.inputs {
                    let Some(ty) = current.place_types.get(&input.root) else {
                        continue;
                    };
                    if let Some(inventory) =
                        DescriptorSlotKind::BorrowedStorage.inventory(types, input.root, *ty)
                    {
                        for location in inventory.slots {
                            capture_case_feasibility(
                                &mut observations,
                                &mut analysis,
                                function.machine,
                                position,
                                &location,
                            );
                        }
                    }
                    slots.extend(
                        descriptor_slot_inventory(types, input.root, *ty)
                            .map(|inventory| inventory.slots)
                            .unwrap_or_default(),
                    );
                }
                for location in slots {
                    capture_case_feasibility(
                        &mut observations,
                        &mut analysis,
                        function.machine,
                        position,
                        &location,
                    );
                    let ty = *current
                        .place_types
                        .get(&location.root)
                        .ok_or_else(|| invalid())?;
                    let path =
                        descriptor_path(types, ty, &location.path).ok_or_else(|| invalid())?;
                    let origins = analysis
                        .symbolic_source_origins(function.machine, position, &location)
                        .and_then(|origins| {
                            origins
                                .into_iter()
                                .map(|origin| {
                                    let geometry = analysis
                                        .flatten_source_origin(function.machine, &origin)?;
                                    Some(Backing::Descriptor {
                                        origin,
                                        geometry,
                                        return_path: None,
                                    })
                                })
                                .collect()
                        });
                    observations
                        .sources
                        .insert((operation, location.root, path), origins);
                }
            }
        }
        Ok(observations)
    }

    fn capture_case_feasibility(
        observations: &mut Observations,
        analysis: &mut DescriptorAnalysis<'_>,
        machine: semantic_vocabulary::MachineId,
        position: DescriptorPosition,
        input: &terminal_semantics::descriptor_lifetimes::DescriptorLocation,
    ) {
        for (case_position, segment) in input.path.iter().enumerate() {
            let terminal_semantics::descriptor_lifetimes::DescriptorPathSegment::Case(case) =
                segment
            else {
                continue;
            };
            let subject = terminal_semantics::descriptor_lifetimes::DescriptorLocation {
                root: input.root,
                path: input.path[..case_position].to_vec(),
            };
            let predicate = terminal_semantics::descriptor_results::DescriptorCasePredicate {
                subject: subject.clone(),
                case: *case,
            };
            capture_predicate_feasibility(observations, analysis, machine, position, &predicate);
        }
    }

    fn capture_predicate_feasibility(
        observations: &mut Observations,
        analysis: &mut DescriptorAnalysis<'_>,
        machine: semantic_vocabulary::MachineId,
        position: DescriptorPosition,
        predicate: &terminal_semantics::descriptor_results::DescriptorCasePredicate,
    ) {
        if analysis.case_feasibility(machine, position, predicate)
            == terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible
        {
            observations.inactive_cases.insert((
                position,
                predicate.subject.clone(),
                predicate.case,
            ));
        }
    }

    pub(crate) fn path_is_impossible(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> bool {
        let Some(position) = custody.reference_position else {
            return false;
        };
        let Some(root_type) = source_contract(function, custody, place) else {
            return false;
        };
        path.iter().enumerate().any(|(index, segment)| {
            let StructuralPathSegment::Case(case) = segment else {
                return false;
            };
            let Some(subject) = terminal_semantics::descriptor_lifetimes::descriptor_location(
                types,
                place,
                root_type,
                &path[..index],
                None,
            ) else {
                return false;
            };
            custody
                .descriptor_backing
                .inactive_cases
                .contains(&(position, subject, *case))
        })
    }

    pub(crate) fn capture(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &StructuralArgument,
        operation: OperationId,
        root: &Root,
    ) -> Result<(Vec<Backing>, Vec<super::OccurrenceIdentity>), LegalizationError> {
        let ty = source_contract(function, custody, source.place).ok_or_else(|| invalid())?;
        if let Some(crossing) = source
            .path
            .iter()
            .rposition(|part| *part == StructuralPathSegment::Referent)
            && let Some(leaf) = custody.leaf(source.place, &source.path[..crossing])
        {
            return Ok((leaf.backing.clone(), vec![leaf.identity.clone()]));
        }
        let descriptor = (0..source.path.len()).find_map(|length| {
            let path = &source.path[..length];
            let carrier = leaf_type(types, ty, path)?;
            terminal_semantics::descriptor_results::is_descriptor(types.iter(), carrier)
                .then(|| path.to_vec())
        });
        if let Some(path) = descriptor {
            let mut backing = custody
                .descriptor_backing
                .sources
                .get(&(operation, source.place, path.clone()))
                .and_then(Option::as_ref)
                .filter(|origins| !origins.is_empty())
                .cloned()
                .ok_or_else(|| invalid())?;
            let position = *custody
                .descriptor_backing
                .positions
                .get(&operation)
                .ok_or_else(|| invalid())?;
            let mut parents = Vec::new();
            for origin in &mut backing {
                if let Backing::Descriptor {
                    geometry:
                        DescriptorCallOrigin::Argument {
                            source: backing_root,
                            projection,
                        },
                    return_path,
                    ..
                } = origin
                {
                    *projection =
                        projection.then(&Projection::new(&source.path[path.len()..]).at(position));
                    *return_path = projection.reference_origin_path(
                        custody
                            .descriptor_backing
                            .ordering
                            .as_ref()
                            .ok_or_else(|| invalid())?,
                    );
                    parents.extend(custody.leaves.iter().filter_map(|(location, leaf)| {
                        (location.0 == *backing_root && projection.path.starts_with(&location.1))
                            .then_some(leaf.identity.clone())
                    }));
                }
            }
            return Ok((backing, parents));
        }
        Ok((vec![Backing::Source(root.clone())], Vec::new()))
    }

    pub(crate) fn overlaps(
        backing: &Backing,
        machine: semantic_vocabulary::MachineId,
        place: PlaceId,
        path: &[StructuralPathSegment],
        independent_carrier: bool,
        origin_overlaps: &impl Fn(&[terminal_psi::ReferenceOriginProjection]) -> bool,
    ) -> bool {
        match backing {
            Backing::Source(Root::Place(root)) => *root == place,
            Backing::Source(Root::Ingress(root, origin)) => {
                *root == place
                    && origin_overlaps(
                        &origin
                            .iter()
                            .cloned()
                            .map(terminal_psi::ReferenceOriginProjection::Exact)
                            .collect::<Vec<_>>(),
                    )
            }
            Backing::Source(Root::Alternatives(origins)) => origins.iter().any(|origin| {
                origin_overlaps_storage(origin, place, independent_carrier, origin_overlaps)
            }),
            Backing::Descriptor {
                origin:
                    DescriptorSourceOrigin::IncomingSlot(_)
                    | DescriptorSourceOrigin::IncomingContents { .. },
                ..
            } if independent_carrier => false,
            Backing::Descriptor { geometry, .. } => match geometry {
                DescriptorCallOrigin::Invocation { .. }
                | DescriptorCallOrigin::AccessFormation { .. } => true,
                DescriptorCallOrigin::Empty => false,
                DescriptorCallOrigin::Literal {
                    machine: owner,
                    place: root,
                } => *owner == machine && *root == place,
                DescriptorCallOrigin::Argument { source, projection } => Region {
                    root: Some(*source),
                    projection: projection.clone(),
                }
                .overlaps(&Region {
                    root: Some(place),
                    projection: Projection::new(path),
                }),
            },
        }
    }

    pub(crate) fn origin_overlaps_storage(
        origin: &target_operations::TargetReferenceOrigin,
        place: PlaceId,
        independent_carrier: bool,
        origin_overlaps: &impl Fn(&[terminal_psi::ReferenceOriginProjection]) -> bool,
    ) -> bool {
        match origin.base {
            target_operations::TargetReferenceOriginBase::Storage { root } => {
                root == place && origin_overlaps(&origin.path)
            }
            target_operations::TargetReferenceOriginBase::IncomingContents { .. } => {
                !independent_carrier
            }
            target_operations::TargetReferenceOriginBase::Invocation { .. }
            | target_operations::TargetReferenceOriginBase::Provider { .. } => true,
        }
    }

    pub(crate) fn overlaps_inline_carrier_read(
        backing: &Backing,
        machine: semantic_vocabulary::MachineId,
        place: PlaceId,
        path: &[StructuralPathSegment],
        origin_overlaps: &impl Fn(&[terminal_psi::ReferenceOriginProjection]) -> bool,
    ) -> bool {
        match backing {
            Backing::Source(Root::Alternatives(origins)) => origins.iter().any(|origin| {
                !matches!(
                    origin.base,
                    target_operations::TargetReferenceOriginBase::IncomingContents { .. }
                        | target_operations::TargetReferenceOriginBase::Provider { .. }
                ) && origin_overlaps_storage(origin, place, false, origin_overlaps)
            }),
            Backing::Descriptor {
                origin:
                    DescriptorSourceOrigin::IncomingSlot(_)
                    | DescriptorSourceOrigin::IncomingContents { .. },
                ..
            } => false,
            _ => overlaps(backing, machine, place, path, false, origin_overlaps),
        }
    }

    pub(crate) fn matches_return(backing: &[Backing], declared: &impl Fn(&Root) -> bool) -> bool {
        !backing.is_empty()
            && backing.iter().all(|origin| match origin {
                Backing::Source(root) => declared(root),
                Backing::Descriptor {
                    origin: DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument { .. }),
                    geometry: DescriptorCallOrigin::Argument { source, .. },
                    return_path: Some(path),
                } => declared(&Root::Alternatives(vec![
                    target_operations::TargetReferenceOrigin {
                        base: target_operations::TargetReferenceOriginBase::Storage {
                            root: *source,
                        },
                        path: path.clone(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                    },
                ])),
                _ => false,
            })
    }
}
pub(crate) mod lifetime_debts {
    use super::{Custody, Leaf, OccurrenceIdentity, StructuralAccess, descriptor_backing};
    use std::collections::BTreeSet;

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct Debt {
        pub(crate) backing: Vec<descriptor_backing::Backing>,
        pub(crate) access: StructuralAccess,
        pub(crate) parents: Vec<OccurrenceIdentity>,
    }

    impl Leaf {
        pub(crate) fn obligations(&self) -> impl Iterator<Item = &OccurrenceIdentity> {
            self.parents
                .iter()
                .chain(self.lifetime_debts.iter().flat_map(|debt| &debt.parents))
        }
    }

    impl Custody {
        fn occurrence(&self, identity: &OccurrenceIdentity) -> Option<&Leaf> {
            self.leaves
                .values()
                .find(|leaf| &leaf.identity == identity)
                .or_else(|| self.retired.get(identity).map(|retired| &retired.leaf))
        }

        pub(crate) fn captured_lifetime_debts<'a>(&'a self, leaf: &'a Leaf) -> Vec<&'a Debt> {
            let mut pending = vec![leaf];
            let mut visited = BTreeSet::new();
            let mut debts = Vec::new();
            while let Some(current) = pending.pop() {
                if !visited.insert(&current.identity) {
                    continue;
                }
                debts.extend(&current.lifetime_debts);
                pending.extend(
                    current
                        .obligations()
                        .filter_map(|parent| self.occurrence(parent)),
                );
            }
            debts
        }

        pub(crate) fn lifetime_depends_on(
            &self,
            leaf: &Leaf,
            ancestor: &OccurrenceIdentity,
        ) -> bool {
            self.lifetime_access_conflicts(leaf, ancestor, StructuralAccess::Owned)
        }

        pub(crate) fn lifetime_access_conflicts(
            &self,
            leaf: &Leaf,
            ancestor: &OccurrenceIdentity,
            requested: StructuralAccess,
        ) -> bool {
            self.captured_lifetime_debts(leaf).iter().any(|debt| {
                (requested != StructuralAccess::SharedBorrow
                    || debt.access != StructuralAccess::SharedBorrow)
                    && debt.parents.iter().any(|parent| {
                        parent == ancestor
                            || self
                                .occurrence(parent)
                                .is_some_and(|parent| self.descends_from(parent, ancestor))
                    })
            })
        }
    }
}
pub(crate) mod observations {
    use super::*;
    use terminal_semantics::descriptor_lifetimes::{
        descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
    };

    pub(crate) struct Observed {
        pub(crate) root: Root,
        pub(crate) referent: StructuralTypeId,
        pub(crate) parents: Vec<OccurrenceIdentity>,
        pub(crate) resident: Option<(PlaceId, Vec<StructuralPathSegment>)>,
    }

    pub(crate) fn observe_alternatives(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
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
                Refusal::InvalidFixedIndexAlternatives
                    .into_error(function, custody, types, place, path)
            })?;
        locations
            .iter()
            .map(|location| {
                let concrete =
                    descriptor_path(types, contract, &location.path).ok_or_else(|| {
                        Refusal::InvalidFixedIndexDescriptorPath
                            .into_error(function, custody, types, place, path)
                    })?;
                observe(function, custody, types, place, &concrete)
                    .map_err(|reason| reason.into_error(function, custody, types, place, &concrete))
            })
            .collect()
    }

    #[derive(Clone, Copy)]
    pub(crate) enum Refusal {
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
        StorageInterference,
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
                Self::MissingMachineObservations => {
                    "machine reference observations are unavailable"
                }
                Self::MissingOperationPosition => "current operation position is unavailable",
                Self::MissingPositionObservations => {
                    "current position has no reference observations"
                }
                Self::MissingLocationObservation => "current location has no reference observation",
                Self::MissingResidentStorageContract => "observed carrier storage has no contract",
                Self::InvalidResidentStoragePath => "observed carrier storage path is invalid",
                Self::ResidentCarrierTypeMismatch => {
                    "observed storage has a different carrier type"
                }
                Self::ResidentReferentMismatch => {
                    "resident reference has a different referent type"
                }
                Self::UnknownOrigins => "observed reference origins are unknown",
                Self::OwnedIngressWithoutResident => {
                    "owned ingress descriptor has no resident occurrence"
                }
                Self::InvalidIngressStoragePath => "incoming descriptor storage path is invalid",
                Self::IngressCarrierTypeMismatch => "incoming slot has a different carrier type",
                Self::LocalOriginWithoutResident => {
                    "local reference origin has no resident occurrence"
                }
                Self::EmptyOrigins => "observed reference origins are empty",
                Self::StorageInterference => {
                    "descriptor storage access conflicts with a retained loan"
                }
            }
        }

        #[track_caller]
        pub(crate) fn into_error(
            self,
            function: &PsiOptimizationFunction,
            custody: &Custody,
            types: &crate::native_geometry::NativeGeometry<'_>,
            place: PlaceId,
            path: &[StructuralPathSegment],
        ) -> LegalizationError {
            let contract = source_contract(function, custody, place);
            let carrier = contract.and_then(|contract| leaf_type(types, contract, path));
            let location = contract
                .and_then(|contract| descriptor_location(types, place, contract, path, None));
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
                    interference: None,
                    origin_count: observation
                        .and_then(|observation| observation.origins.as_ref().map(Vec::len)),
                }),
            }
        }
    }

    pub(crate) fn observe(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<Observed, Refusal> {
        observe_following(function, custody, types, place, path, &mut Vec::new())
    }

    fn observe_following(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
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
        let mut incoming_slots = Vec::new();
        let mut owns_ingress = false;
        for source in sources {
            let origin = if let Some(slot) = source.captured_entry_slot() {
                let parameter = function
                    .structural_parameter(slot.root)
                    .ok_or(Refusal::InvalidIngressStoragePath)?;
                let slot_path = descriptor_path(types, parameter.structural_type, &slot.path)
                    .ok_or(Refusal::InvalidIngressStoragePath)?;
                if parameter.access == StructuralAccess::Owned {
                    owns_ingress = true;
                }
                if leaf_type(types, parameter.structural_type, &slot_path) != Some(carrier) {
                    return Err(Refusal::IngressCarrierTypeMismatch);
                }
                incoming_slots.push((slot.root, slot_path.clone()));
                target_operations::TargetReferenceOrigin {
                    base: target_operations::TargetReferenceOriginBase::IncomingContents {
                        root: slot.root,
                        slot: slot_path,
                    },
                    path: Vec::new(),
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                }
            } else {
                return Err(Refusal::LocalOriginWithoutResident);
            };
            origins.push(origin);
        }
        if origins.is_empty() {
            return Err(Refusal::EmptyOrigins);
        }
        if owns_ingress {
            authenticate_indirect_ingress(function, custody, types, place, path, &incoming_slots)
                .ok_or(Refusal::OwnedIngressWithoutResident)?;
        }
        origins.sort();
        origins.dedup();
        Ok(Observed {
            root: Root::Alternatives(origins),
            referent,
            parents: Vec::new(),
            resident: None,
        })
    }

    fn through_resident(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        place: PlaceId,
        path: &[StructuralPathSegment],
        active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
    ) -> Option<Observed> {
        let crossing = path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)?;
        let carrier =
            observe_following(function, custody, types, place, &path[..crossing], active).ok()?;
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
            super::storage_origins(function, custody, types, &carrier.root, carrier.referent)
                .ok()?;
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

    fn authenticate_indirect_ingress(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        current_place: PlaceId,
        current_path: &[StructuralPathSegment],
        incoming_slots: &[(PlaceId, Vec<StructuralPathSegment>)],
    ) -> Option<()> {
        let crossing = current_path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
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
        let mut incoming = BTreeSet::new();
        for (root, incoming_path) in incoming_slots {
            let incoming_crossing = incoming_path
                .iter()
                .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
            let carrier_path = &incoming_path[..incoming_crossing];
            function.structural_parameter(*root)?;
            let origin = Root::incoming_contents(*root, carrier_path.to_vec());
            incoming.extend(origin.origins());
        }
        let observed = observed
            .origins
            .iter()
            .flat_map(Root::origins)
            .collect::<BTreeSet<_>>();
        (!observed.is_empty() && observed == incoming).then_some(())
    }
}
pub(crate) mod occurrences {
    use super::{
        Custody, Leaf, LegalizationError, OperationId, PlaceId, PsiOptimizationFunction, Root,
        StructuralAccess, StructuralOperationResult, StructuralPathSegment, StructuralTypeId,
        invalid, leaf_referent, owned_relative_path,
    };
    use crate::legalization::scalar_graph_input::structural_case::source_result;
    use semantic_vocabulary::{BlockId, StructuralPlaceKind};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralParameterDeclaration;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct OccurrenceIdentity {
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<StructuralPathSegment>,
        pub(crate) formation: OccurrenceFormation,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum OccurrenceFormation {
        Parameter,
        Operation(OperationId),
        ResidentJoin(BlockId),
    }

    impl OccurrenceIdentity {
        pub(crate) fn resident_join(
            block: BlockId,
            location: &(PlaceId, Vec<StructuralPathSegment>),
        ) -> Self {
            Self {
                place: location.0,
                path: location.1.clone(),
                formation: OccurrenceFormation::ResidentJoin(block),
            }
        }

        pub(crate) fn parameter(location: &(PlaceId, Vec<StructuralPathSegment>)) -> Self {
            Self {
                place: location.0,
                path: location.1.clone(),
                formation: OccurrenceFormation::Parameter,
            }
        }

        pub(crate) fn operation(
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
    pub(crate) enum CarrierContract {
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
        pub(crate) fn structural_type(&self) -> StructuralTypeId {
            match self {
                Self::Operation { result, .. } => result.structural_type,
                Self::Parameter(parameter) => parameter.structural_type,
                Self::ResidentJoin {
                    structural_type, ..
                } => *structural_type,
            }
        }

        pub(crate) fn matches_declaration(
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
                        && source_result(function, place).is_ok_and(|(actual_operation, actual)| {
                            actual_operation == *operation && actual == result
                        })
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

    pub(crate) fn resident_type(
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
                source_result(function, place)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Formation {
        pub(crate) identity: OccurrenceIdentity,
        pub(crate) contract: CarrierContract,
    }

    impl Leaf {
        pub(crate) fn in_caller_storage(
            &self,
            function: &PsiOptimizationFunction,
            types: &crate::native_geometry::NativeGeometry<'_>,
            carrier: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<bool, LegalizationError> {
            let Some(parameter) = function.structural_parameter(carrier) else {
                return Ok(false);
            };
            let Some(referent) = terminal_semantics::borrowed_entry_reference_referent(
                types.iter(),
                parameter,
                path,
            ) else {
                return Ok(false);
            };
            if referent != self.referent
                || !self
                    .contract
                    .matches_declaration(function, self.identity.place)
                || self.formations.is_empty()
                || self.formations.iter().any(|formation| {
                    !formation
                        .contract
                        .matches_declaration(function, formation.identity.place)
                })
            {
                return Err(invalid());
            }
            Ok(true)
        }

        pub(crate) fn formed(
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
                lifetime_debts: Vec::new(),
                root,
                parents,
                contract: contract.clone(),
                formations: vec![Formation { identity, contract }],
                referent,
                placed: None,
            }
        }

        pub(crate) fn reform(
            &mut self,
            operation: OperationId,
            result: &StructuralOperationResult,
        ) {
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

        pub(crate) fn matches_operation_result(&self, result: &StructuralOperationResult) -> bool {
            matches!(&self.contract, CarrierContract::Operation { result: actual, .. } if actual == result)
        }
    }

    pub(crate) fn normalize_formations(
        formations: &mut Vec<Formation>,
    ) -> Result<(), LegalizationError> {
        formations.sort_by(|left, right| left.identity.cmp(&right.identity));
        if formations.windows(2).any(|pair| {
            pair[0].identity == pair[1].identity && pair[0].contract != pair[1].contract
        }) {
            return Err(invalid());
        }
        formations.dedup();
        Ok(())
    }

    pub(crate) fn merge_arrivals(
        function: &PsiOptimizationFunction,
        target: BlockId,
        types: &crate::native_geometry::NativeGeometry<'_>,
        guarded_absence: &BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
        left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
        right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    ) -> Result<Option<()>, LegalizationError> {
        for (key, leaf) in left.iter().chain(right) {
            if left.contains_key(key) == right.contains_key(key) {
                continue;
            }
            if guarded_absence.contains(key) {
                let stored = resident_type(function, key.0)
                    .and_then(|root| super::leaf_type(types, root, &key.1))
                    .and_then(|ty| super::find_structural_type(types, ty));
                let carried =
                    super::leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
                        .and_then(|ty| super::find_structural_type(types, ty));
                if key.1.contains(&StructuralPathSegment::Referent)
                    && stored.zip(carried).is_some_and(|(stored, carried)| {
                        stored.shape == carried.shape
                            && matches!(stored.shape, super::StructuralTypeShape::Reference { referent, .. } if referent == leaf.referent)
                    })
                    && leaf.contract.matches_declaration(function, leaf.identity.place)
                    && !leaf.formations.is_empty()
                    && leaf.formations.iter().all(|formation| formation.contract.matches_declaration(function, formation.identity.place))
                {
                    continue;
                }
                return Err(invalid());
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
                .ok_or_else(|| invalid())?;
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
            existing
                .lifetime_debts
                .extend_from_slice(&incoming.lifetime_debts);
            existing.lifetime_debts.sort();
            existing.lifetime_debts.dedup();
            existing.parents.extend_from_slice(&incoming.parents);
            existing.parents.sort();
            existing.parents.dedup();
            existing.formations.extend_from_slice(&incoming.formations);
            normalize_formations(&mut existing.formations)?;
        }
        Ok(Some(()))
    }

    pub(crate) fn re_home_bound_leaves(
        custody: &mut Custody,
        bindings: &[abstract_operations::AbstractStructuralBinding],
        parameters: &[StructuralParameterDeclaration],
        types: &crate::native_geometry::NativeGeometry<'_>,
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
                || custody.leaves.contains_key(destination)
                    && !selected_sources.contains(destination)
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

    pub(crate) fn remap_parents(
        custody: &mut Custody,
        identities: &[(OccurrenceIdentity, OccurrenceIdentity)],
    ) {
        super::borrowed_arrivals::remap_parents(custody, identities);
        for leaf in custody.leaves.values_mut().chain(
            custody
                .retired
                .values_mut()
                .map(|retired| &mut retired.leaf),
        ) {
            for links in std::iter::once(&mut leaf.parents)
                .chain(leaf.lifetime_debts.iter_mut().map(|debt| &mut debt.parents))
            {
                let mut parents = Vec::with_capacity(links.len());
                for parent in links.iter() {
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
                *links = parents;
            }
        }
    }
}
pub(crate) mod resident_joins {
    use super::descriptor_backing::path_is_impossible;
    use super::occurrences::{remap_parents, resident_type};
    use super::{
        BlockId, CarrierContract, Custody, LegalizationError, OccurrenceIdentity, PlaceId,
        PsiOptimizationFunction, StructuralAccess, StructuralPathSegment, StructuralTypeShape,
        borrowed_arrivals, find_structural_type, invalid, leaf_type, merge_arrivals, retirements,
    };
    use std::collections::BTreeSet;

    type Location = (PlaceId, Vec<StructuralPathSegment>);

    pub(crate) fn merge(
        function: &PsiOptimizationFunction,
        target: BlockId,
        types: &crate::native_geometry::NativeGeometry<'_>,
        arrivals: impl Iterator<Item = Custody>,
    ) -> Result<Option<Custody>, LegalizationError> {
        let mut arrivals = arrivals.collect::<Vec<_>>();
        let first = arrivals.first().ok_or_else(|| invalid())?;
        let guarded_absence = arrivals
            .iter()
            .flat_map(|arrival| arrival.leaves.keys())
            .filter(|location| {
                location.1.contains(&StructuralPathSegment::Referent)
                    && arrivals
                        .iter()
                        .any(|arrival| !arrival.leaves.contains_key(*location))
                    && arrivals
                        .iter()
                        .filter(|arrival| !arrival.leaves.contains_key(*location))
                        .all(|arrival| {
                            path_is_impossible(function, arrival, types, location.0, &location.1)
                        })
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let is_parameter = |place| {
            function.block(target).is_some_and(|block| {
                block.structural_parameters.iter().any(|parameter| {
                    parameter.place == place && parameter.access == StructuralAccess::Owned
                })
            })
        };
        let common = first
            .leaves
            .keys()
            .filter(|location| {
                !is_parameter(location.0)
                    && arrivals
                        .iter()
                        .all(|arrival| arrival.leaves.contains_key(*location))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut selected = common
            .iter()
            .filter(|location| {
                arrivals
                    .iter()
                    .skip(1)
                    .any(|arrival| arrival.leaves.get(*location) != first.leaves.get(*location))
            })
            .cloned()
            .collect::<BTreeSet<Location>>();
        loop {
            let identities = arrivals
                .iter()
                .flat_map(|arrival| {
                    selected.iter().filter_map(|location| {
                        arrival
                            .leaves
                            .get(location)
                            .map(|leaf| leaf.identity.clone())
                    })
                })
                .collect::<BTreeSet<_>>();
            let descendants = common
                .iter()
                .filter(|location| {
                    !selected.contains(*location)
                        && arrivals.iter().any(|arrival| {
                            arrival.leaves.get(*location).is_some_and(|leaf| {
                                leaf.obligations().any(|parent| identities.contains(parent))
                            })
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            if descendants.is_empty() {
                break;
            }
            selected.extend(descendants);
        }
        for arrival in &mut arrivals {
            let mut identities = Vec::new();
            for location in &selected {
                let structural_type =
                    resident_type(function, location.0).ok_or_else(|| invalid())?;
                let declaration = leaf_type(types, structural_type, &location.1)
                    .and_then(|reference| find_structural_type(types, reference))
                    .ok_or_else(|| invalid())?;
                let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                    return Err(invalid());
                };
                let leaf = arrival.leaves.get_mut(location).ok_or_else(|| invalid())?;
                let carried =
                    leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
                        .and_then(|reference| find_structural_type(types, reference))
                        .ok_or_else(|| invalid())?;
                if carried.shape != declaration.shape
                    || leaf.referent != referent
                    || !matches!(
                        access,
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
                    || !leaf
                        .contract
                        .matches_declaration(function, leaf.identity.place)
                    || leaf.formations.is_empty()
                    || leaf.formations.iter().any(|formation| {
                        !formation
                            .contract
                            .matches_declaration(function, formation.identity.place)
                    })
                {
                    return Err(invalid());
                }
                let identity = OccurrenceIdentity::resident_join(target, location);
                if arrival.retired.contains_key(&identity) {
                    return Err(invalid());
                }
                identities.push((leaf.identity.clone(), identity.clone()));
                leaf.identity = identity;
                leaf.contract = CarrierContract::ResidentJoin {
                    block: target,
                    place: location.0,
                    structural_type,
                };
            }
            remap_parents(arrival, &identities);
            arrival.validate_occurrences()?;
        }
        let mut arrivals = arrivals.into_iter();
        let mut complete = arrivals.next().ok_or_else(|| invalid())?;
        for arrival in arrivals {
            if merge_arrivals(
                function,
                target,
                types,
                &guarded_absence,
                &mut complete.leaves,
                &arrival.leaves,
            )?
            .is_none()
                || retirements::merge(&mut complete.retired, &arrival.retired).is_none()
            {
                return Ok(None);
            }
            borrowed_arrivals::merge(&mut complete, &arrival)?;
        }
        complete.prune_retired();
        complete.validate_occurrences()?;
        Ok(Some(complete))
    }
}
pub(crate) mod retirements {
    use super::{Custody, Leaf, LegalizationError, OccurrenceIdentity, OperationId, invalid};
    use semantic_vocabulary::{EdgeId, PlaceId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralPathSegment;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum DisposalEvent {
        Operation(OperationId),
        NormalCall(OperationId),
        Edge(EdgeId),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct RetiredOccurrence {
        pub(crate) leaf: Leaf,
        pub(crate) disposals: Vec<DisposalEvent>,
    }

    impl Custody {
        pub(crate) fn retire(
            &mut self,
            leaf: Leaf,
            event: DisposalEvent,
        ) -> Result<(), LegalizationError> {
            if self.retired.contains_key(&leaf.identity) {
                return Err(invalid());
            }
            self.retired.insert(
                leaf.identity.clone(),
                RetiredOccurrence {
                    leaf,
                    disposals: vec![event],
                },
            );
            Ok(())
        }

        pub(crate) fn descends_from(&self, child: &Leaf, ancestor: &OccurrenceIdentity) -> bool {
            let mut pending = child.parents.iter().collect::<Vec<_>>();
            let mut visited = BTreeSet::new();
            while let Some(parent) = pending.pop() {
                if parent == ancestor {
                    return true;
                }
                if visited.insert(parent)
                    && let Some(retired) = self.retired.get(parent)
                {
                    pending.extend(&retired.leaf.parents);
                }
            }
            false
        }

        pub(crate) fn prune_retired(&mut self) {
            let mut pending = self
                .leaves
                .values()
                .flat_map(|leaf| leaf.obligations())
                .chain(super::borrowed_arrivals::obligations(self))
                .collect::<Vec<_>>();
            let mut retained = BTreeSet::new();
            while let Some(identity) = pending.pop() {
                if retained.insert(identity.clone())
                    && let Some(retired) = self.retired.get(identity)
                {
                    pending.extend(retired.leaf.obligations());
                }
            }
            self.retired
                .retain(|identity, _| retained.contains(identity));
        }

        pub(crate) fn validate_occurrences(&self) -> Result<(), LegalizationError> {
            let mut identities =
                BTreeMap::<_, (&(PlaceId, Vec<StructuralPathSegment>), &Leaf)>::new();
            for (location, leaf) in &self.leaves {
                let duplicate = identities
                    .insert(&leaf.identity, (location, leaf))
                    .is_some_and(|(previous, previous_leaf)| {
                        previous_leaf != leaf || !alternative_residency(previous, location)
                    });
                if duplicate || self.retired.contains_key(&leaf.identity) {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE custody-duplicate location={:?} identity={:?} live={:?} retired={:?}",
                            location, leaf.identity, self.leaves, self.retired
                        );
                    }
                    return Err(LegalizationError::ReferenceCompletion {
                        reason: terminal_semantics::ReferenceCompletionError::DuplicateOccurrence,
                    });
                }
            }
            for leaf in self
                .leaves
                .values()
                .chain(self.retired.values().map(|retired| &retired.leaf))
            {
                if let Some(parent) = leaf.obligations().find(|parent| {
                    !identities.contains_key(parent) && !self.retired.contains_key(parent)
                }) {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE custody-missing-parent occurrence={:?} parent={:?} live={:?} retired={:?}",
                            leaf.identity,
                            parent,
                            self.leaves
                                .iter()
                                .map(|(location, leaf)| (location, &leaf.identity))
                                .collect::<Vec<_>>(),
                            self.retired.keys().collect::<Vec<_>>()
                        );
                    }
                    return Err(LegalizationError::ReferenceCompletion {
                        reason: terminal_semantics::ReferenceCompletionError::MissingParent,
                    });
                }
            }
            if let Some(parent) = super::borrowed_arrivals::obligations(self).find(|parent| {
                !identities.contains_key(parent) && !self.retired.contains_key(parent)
            }) {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    eprintln!(
                        "CHECKSITE custody-missing-arrival-parent parent={:?} live={:?} retired={:?}",
                        parent,
                        self.leaves
                            .iter()
                            .map(|(location, leaf)| (location, &leaf.identity))
                            .collect::<Vec<_>>(),
                        self.retired.keys().collect::<Vec<_>>()
                    );
                }
                return Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::MissingParent,
                });
            }
            Ok(())
        }
    }

    pub(crate) fn alternative_residency(
        left: &(PlaceId, Vec<StructuralPathSegment>),
        right: &(PlaceId, Vec<StructuralPathSegment>),
    ) -> bool {
        left.0 == right.0
            && left
                .1
                .iter()
                .zip(&right.1)
                .find(|(left, right)| left != right)
                .is_some_and(|pair| {
                    matches!(
                        pair,
                        (
                            StructuralPathSegment::Case(_),
                            StructuralPathSegment::Case(_)
                        )
                    )
                })
    }

    pub(crate) fn merge(
        left: &mut BTreeMap<OccurrenceIdentity, RetiredOccurrence>,
        right: &BTreeMap<OccurrenceIdentity, RetiredOccurrence>,
    ) -> Option<()> {
        for (identity, incoming) in right {
            let Some(existing) = left.get_mut(identity) else {
                left.insert(identity.clone(), incoming.clone());
                continue;
            };
            if existing.leaf != incoming.leaf {
                return None;
            }
            existing.disposals.extend_from_slice(&incoming.disposals);
            existing.disposals.sort();
            existing.disposals.dedup();
        }
        Some(())
    }
}
pub(crate) mod stored_calls {
    use super::*;
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use terminal_semantics::descriptor_lifetimes::descriptor_path;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorSlotKind, DescriptorStoredOrigin,
    };

    type Location = (PlaceId, Vec<StructuralPathSegment>);

    fn destinations_may_coexist(left: &Location, right: &Location) -> bool {
        if left.0 != right.0 {
            return true;
        }
        for (left_segment, right_segment) in left.1.iter().zip(&right.1) {
            match (left_segment, right_segment) {
                (StructuralPathSegment::Case(left), StructuralPathSegment::Case(right))
                    if left != right =>
                {
                    return false;
                }
                (StructuralPathSegment::Case(_), StructuralPathSegment::Case(_))
                | (StructuralPathSegment::Field(_), StructuralPathSegment::Field(_))
                | (StructuralPathSegment::FixedIndex(_), StructuralPathSegment::FixedIndex(_))
                    if left_segment == right_segment => {}
                _ => return true,
            }
        }
        true
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Source {
        IncomingSlot(StructuralArgument),
        BorrowedStorage(StructuralArgument),
        Provider {
            origin: target_operations::TargetReferenceOrigin,
            debts: Vec<(Source, StructuralAccess)>,
        },
    }
    #[derive(Debug)]
    struct Effect {
        destination: StructuralArgument,
        sources: Vec<Source>,
    }
    #[derive(Debug, Default)]
    pub(crate) struct Effects {
        pub(crate) reference_catalog_available: bool,
        machines: BTreeMap<MachineId, Option<Vec<Effect>>>,
        boundaries: BTreeMap<BoundaryMachineId, Option<Vec<Effect>>>,
        pub(crate) observations: BTreeMap<
            MachineId,
            terminal_semantics::descriptor_results::DescriptorSourceObservations,
        >,
        pub(crate) access_results: BTreeMap<
            MachineId,
            Option<
                Vec<optimization_unit_semantics::descriptor_results::access_results::ResultSlot>,
            >,
        >,
    }

    fn writable_slots(
        function: &PsiOptimizationFunction,
        types: &crate::native_geometry::NativeGeometry<'_>,
    ) -> bool {
        function.structural_parameters.iter().any(|parameter| {
            DescriptorSlotKind::References
                .writable_slots(
                    types,
                    parameter.place,
                    parameter.structural_type,
                    parameter.access,
                )
                .is_none_or(|slots| !slots.is_empty())
        })
    }

    pub(crate) fn prepare(
        descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
    ) -> Effects {
        let plan = descriptor_inputs.plan();
        let unit = descriptor_inputs.unit();
        let types = plan.geometry();
        let candidates = unit
            .functions
            .iter()
            .filter(|function| writable_slots(function, types))
            .collect::<Vec<_>>();
        let mut effects = Effects {
            reference_catalog_available: false,
            boundaries: unit
                .boundary_machines
                .iter()
                .map(|boundary| (boundary.id, None))
                .collect(),
            observations: BTreeMap::new(),
            access_results: BTreeMap::new(),
            machines: candidates
                .iter()
                .map(|function| (function.machine, None))
                .collect(),
        };
        let Some(graph) = descriptor_inputs.references() else {
            return effects;
        };
        effects.reference_catalog_available = true;
        let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
        for machine in graph {
            effects
                .observations
                .insert(machine.id, analysis.source_observations(machine.id));
            let result = unit
                .functions
                .iter()
                .find(|function| function.machine == machine.id)
                .and_then(|function| {
                    let result = function.result.structural()?;
                    optimization_unit_semantics::descriptor_results::access_results::summarize(
                        &plan.structural_types,
                        &unit.boundary_machines,
                        &function.structural_parameters,
                        result.structural_type,
                        &analysis.result_slots(machine.id)?,
                        |machine, operation| {
                            unit.functions
                                .iter()
                                .find(|function| function.machine == machine)?
                                .block_nodes()
                                .find(|node| node.operation.psi_operation() == Some(operation))
                                .map(|node| &node.operation)
                        },
                    )
                });
            effects.access_results.insert(machine.id, result);
        }
        for function in candidates {
            let summary = analysis.stored_effects(function.machine).and_then(|rows| {
                rows.into_iter()
                    .map(|row| {
                        let parameter = function.structural_parameters.get(row.parameter)?;
                        let path =
                            descriptor_path(types, parameter.structural_type, &row.exact_path()?)?;
                        let destination = StructuralArgument {
                            place: parameter.place,
                            path,
                            access: parameter.access,
                        };
                        let mut sources = Vec::new();
                        for origin in row.origins {
                            if let Some((position, path)) = origin.captured_entry_slot() {
                                let parameter = function.structural_parameters.get(position)?;
                                sources.push(Source::IncomingSlot(StructuralArgument {
                                    place: parameter.place,
                                    path: descriptor_path(types, parameter.structural_type, &path)?,
                                    access: parameter.access,
                                }));
                                continue;
                            }
                            match origin {
                                DescriptorStoredOrigin::Empty => {}
                                DescriptorStoredOrigin::Parameter {
                                    position,
                                    projection,
                                } => {
                                    if projection
                                        != terminal_semantics::borrowed_storage::Projection::new(
                                            &projection.path,
                                        )
                                        || projection.indirect
                                        || projection.window.is_some()
                                        || projection.captured_at.is_some()
                                    {
                                        return None;
                                    }
                                    let source = function.structural_parameters.get(position)?;
                                    let ty = leaf_type(
                                        types,
                                        parameter.structural_type,
                                        &destination.path,
                                    )?;
                                    let StructuralTypeShape::Reference { access, .. } =
                                        find_structural_type(types, ty)?.shape
                                    else {
                                        return None;
                                    };
                                    if !access_covers(source.access, access) {
                                        return None;
                                    }
                                    sources.push(Source::BorrowedStorage(StructuralArgument {
                                        place: source.place,
                                        path: projection.path,
                                        access,
                                    }));
                                }
                                DescriptorStoredOrigin::Provider(provider) => {
                                    sources.push(provider_source(
                                        &provider,
                                        &function.structural_parameters,
                                        types,
                                        &unit.boundary_machines,
                                    )?);
                                }
                                DescriptorStoredOrigin::Invocation { .. }
                                | DescriptorStoredOrigin::IncomingSlot { .. }
                                | DescriptorStoredOrigin::IncomingContents { .. }
                                | DescriptorStoredOrigin::Literal { .. }
                                | DescriptorStoredOrigin::AccessFormation { .. } => return None,
                            }
                        }
                        sources.sort();
                        sources.dedup();
                        Some(Effect {
                            destination,
                            sources,
                        })
                    })
                    .collect()
            });
            effects.machines.insert(function.machine, summary);
        }
        for boundary in &unit.boundary_machines {
            if !terminal_semantics::descriptor_results::provider_outputs_cover_writable_slots(
                types, boundary,
            ) {
                continue;
            }
            let summary = boundary
                .reference_outputs
                .iter()
                .enumerate()
                .filter(|(_, output)| {
                    matches!(
                        output.destination,
                        terminal_psi::BoundaryReferenceDestination::Parameter(_)
                    )
                })
                .map(|(output_index, output)| {
                    let terminal_psi::BoundaryReferenceDestination::Parameter(position) =
                        output.destination;
                    let parameter = boundary.structural_parameters.get(position as usize)?;
                    let mut dependencies = Vec::new();
                    for debt in &output.dependencies {
                        let (origin, access) = match debt {
                            terminal_psi::BoundaryLifetimeDependency::Referent {
                                argument_index,
                                access,
                            } => (
                                DescriptorStoredOrigin::Parameter {
                                    position: *argument_index as usize,
                                    projection:
                                        terminal_semantics::borrowed_storage::Projection::new(&[]),
                                },
                                *access,
                            ),
                            terminal_psi::BoundaryLifetimeDependency::Stored {
                                argument_index,
                                path,
                                access,
                            } => {
                                let formal = boundary
                                    .structural_parameters
                                    .get(*argument_index as usize)?;
                                let location =
                                    terminal_semantics::descriptor_lifetimes::descriptor_location(
                                        types,
                                        formal.place,
                                        formal.structural_type,
                                        path,
                                        None,
                                    )?;
                                (
                                    DescriptorStoredOrigin::IncomingSlot {
                                        position: *argument_index as usize,
                                        path: location.path,
                                    },
                                    *access,
                                )
                            }
                        };
                        dependencies.push(
                            terminal_semantics::descriptor_results::DescriptorLifetimeDebt {
                                origin,
                                access,
                            },
                        );
                    }
                    let provider =
                        terminal_semantics::descriptor_results::DescriptorProviderOrigin {
                            boundary: boundary.id,
                            output: u32::try_from(output_index).ok()?,
                            invocations: Vec::new(),
                            projection: terminal_semantics::borrowed_storage::Projection::new(&[]),
                            lifetime_dependencies: dependencies,
                        };
                    Some(Effect {
                        destination: StructuralArgument {
                            place: parameter.place,
                            path: output.path.clone(),
                            access: parameter.access,
                        },
                        sources: vec![provider_source(
                            &provider,
                            &boundary.structural_parameters,
                            types,
                            &unit.boundary_machines,
                        )?],
                    })
                })
                .collect();
            effects.boundaries.insert(boundary.id, summary);
        }
        effects
    }

    fn provider_source(
        provider: &terminal_semantics::descriptor_results::DescriptorProviderOrigin<
            DescriptorStoredOrigin,
        >,
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        types: &crate::native_geometry::NativeGeometry<'_>,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    ) -> Option<Source> {
        let boundary = boundaries
            .iter()
            .find(|boundary| boundary.id == provider.boundary)?;
        let output = boundary.reference_outputs.get(provider.output as usize)?;
        let root_type = match output.destination {
            terminal_psi::BoundaryReferenceDestination::Parameter(position) => {
                boundary
                    .structural_parameters
                    .get(position as usize)?
                    .structural_type
            }
        };
        let StructuralTypeShape::Reference { referent, access } =
            find_structural_type(types, leaf_type(types, root_type, &output.path)?)?.shape
        else {
            return None;
        };
        if access != output.access
            || provider.projection
                != terminal_semantics::borrowed_storage::Projection::new(&provider.projection.path)
        {
            return None;
        }
        let mut debts = Vec::new();
        for debt in &provider.lifetime_dependencies {
            let source = match &debt.origin {
                DescriptorStoredOrigin::Parameter {
                    position,
                    projection,
                } => {
                    let parameter = parameters.get(*position)?;
                    if projection
                        != &terminal_semantics::borrowed_storage::Projection::new(&projection.path)
                        || !access_covers(parameter.access, debt.access)
                    {
                        return None;
                    }
                    Source::BorrowedStorage(StructuralArgument {
                        place: parameter.place,
                        path: projection.path.clone(),
                        access: debt.access,
                    })
                }
                DescriptorStoredOrigin::IncomingSlot { position, path } => {
                    let parameter = parameters.get(*position)?;
                    Source::IncomingSlot(StructuralArgument {
                        place: parameter.place,
                        path: descriptor_path(types, parameter.structural_type, path)?,
                        access: parameter.access,
                    })
                }
                _ => return None,
            };
            debts.push((source, debt.access));
        }
        Some(Source::Provider {
            origin: target_operations::TargetReferenceOrigin {
                base: target_operations::TargetReferenceOriginBase::Provider {
                    boundary: provider.boundary,
                    output: provider.output,
                    invocations: provider
                        .invocations
                        .iter()
                        .map(|site| (site.machine, site.operation))
                        .collect(),
                    referent,
                    access,
                },
                path: provider
                    .projection
                    .path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            },
            debts,
        })
    }

    pub(crate) enum Callee<'a> {
        Body(&'a PsiOptimizationFunction),
        Boundary(&'a terminal_psi::BoundaryMachineDeclaration),
    }

    pub(crate) struct Staged {
        operation: OperationId,
        replacements: Vec<(Location, Option<Leaf>)>,
        relocations: BTreeMap<Location, Location>,
        established: BTreeSet<Location>,
    }

    fn receiving_contract(
        function: &PsiOptimizationFunction,
        destination: &Location,
    ) -> Result<CarrierContract, LegalizationError> {
        if let Ok((operation, result)) =
            crate::legalization::scalar_graph_input::structural_case::source_result(
                function,
                destination.0,
            )
        {
            Ok(CarrierContract::Operation {
                operation,
                result: result.clone(),
            })
        } else {
            let parameter = function
                .structural_parameter(destination.0)
                .or_else(|| {
                    function
                        .block_structural_parameters()
                        .find(|parameter| parameter.place == destination.0)
                })
                .ok_or_else(|| invalid())?;
            Ok(CarrierContract::Parameter(parameter.clone()))
        }
    }

    fn receiving_identity(
        function: &PsiOptimizationFunction,
        destination: &Location,
        operation: OperationId,
        leaf: &mut Leaf,
    ) -> Result<(), LegalizationError> {
        leaf.identity = OccurrenceIdentity::operation(destination, operation);
        leaf.contract = receiving_contract(function, destination)?;
        leaf.formations = vec![Formation {
            identity: leaf.identity.clone(),
            contract: leaf.contract.clone(),
        }];
        Ok(())
    }

    pub(crate) fn stage(
        function: &PsiOptimizationFunction,
        callee: Option<Callee<'_>>,
        types: &crate::native_geometry::NativeGeometry<'_>,
        custody: &Custody,
        arguments: &[StructuralArgument],
        operation: OperationId,
        moved: &BTreeMap<Location, Leaf>,
    ) -> Result<Staged, LegalizationError> {
        let mut staged = Staged {
            operation,
            replacements: Vec::new(),
            relocations: BTreeMap::new(),
            established: BTreeSet::new(),
        };
        let Some(callee) = callee else {
            if arguments.iter().any(|argument| {
                source_contract(function, custody, argument.place)
                    .and_then(|ty| leaf_type(types, ty, &argument.path))
                    .is_none_or(|ty| {
                        DescriptorSlotKind::References
                            .writable_slots(types, argument.place, ty, argument.access)
                            .is_none_or(|slots| !slots.is_empty())
                    })
            }) {
                return Err(invalid());
            }
            return Ok(staged);
        };
        let (parameters, effects) = match callee {
            Callee::Body(callee) => (
                &callee.structural_parameters,
                custody.stored_calls.machines.get(&callee.machine),
            ),
            Callee::Boundary(boundary) => (
                &boundary.structural_parameters,
                custody.stored_calls.boundaries.get(&boundary.id),
            ),
        };
        let Some(effects) = effects else {
            return Ok(staged);
        };
        let effects = effects.as_ref().ok_or_else(|| invalid())?;
        let actual = |formal: &StructuralArgument| {
            call_source(parameters, arguments, formal).ok_or_else(|| invalid())
        };
        let mut destinations = BTreeSet::new();
        for effect in effects {
            let destination = actual(&effect.destination)?;
            let destination = window_storage(
                function,
                custody,
                types,
                destination.place,
                &destination.path,
                StructuralAccess::WriteOnlyBorrow,
            )?;
            if !destinations.insert(destination.clone()) {
                return Err(invalid());
            }
            let ty = source_contract(function, custody, destination.0)
                .and_then(|ty| leaf_type(types, ty, &destination.1))
                .ok_or_else(|| invalid())?;
            let StructuralTypeShape::Reference { referent, access } =
                find_structural_type(types, ty)
                    .ok_or_else(|| invalid())?
                    .shape
            else {
                return Err(invalid());
            };
            let borrowed_storage = |source: &StructuralArgument| {
                if source.access != access
                    || reference_source_type(function, custody, types, source) != Some(referent)
                {
                    return Err(invalid());
                }
                let (root, mut parents) = normalized_source(function, custody, types, source)?;
                let (backing, backing_parents) = descriptor_backing::capture(
                    function, custody, types, source, operation, &root,
                )?;
                parents.extend(backing_parents);
                parents.sort();
                parents.dedup();
                let contract = receiving_contract(function, &destination)?;
                let identity = OccurrenceIdentity::operation(&destination, operation);
                Ok(Leaf {
                    identity: identity.clone(),
                    root,
                    backing,
                    lifetime_debts: Vec::new(),
                    parents,
                    formations: vec![Formation {
                        identity,
                        contract: contract.clone(),
                    }],
                    contract,
                    referent,
                    placed: super::access_source(function, custody, types, source),
                })
            };
            let mut candidates = Vec::new();
            for origin in &effect.sources {
                let formal = match origin {
                    Source::Provider { origin, debts } => {
                        staged.established.insert(destination.clone());
                        let mut origin = origin.clone();
                        let target_operations::TargetReferenceOriginBase::Provider {
                            invocations,
                            referent: promised,
                            access: promised_access,
                            ..
                        } = &mut origin.base
                        else {
                            return Err(invalid());
                        };
                        let path = origin
                            .path
                            .iter()
                            .map(|part| match part {
                                ReferenceOriginProjection::Exact(part) => Some(part.clone()),
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| invalid())?;
                        if leaf_type(types, *promised, &path) != Some(referent)
                            || *promised_access != access
                        {
                            return Err(invalid());
                        }
                        invocations.insert(0, (function.machine, operation));
                        let mut lifetime_debts = Vec::new();
                        for (dependency, required_access) in debts {
                            match dependency {
                                Source::BorrowedStorage(formal) => {
                                    let mut source = actual(formal)?;
                                    source.access = *required_access;
                                    let (root, mut parents) =
                                        normalized_source(function, custody, types, &source)?;
                                    let (backing, captured_parents) = descriptor_backing::capture(
                                        function, custody, types, &source, operation, &root,
                                    )?;
                                    parents.extend(captured_parents);
                                    parents.sort();
                                    parents.dedup();
                                    lifetime_debts.push(lifetime_debts::Debt {
                                        backing,
                                        access: *required_access,
                                        parents,
                                    });
                                }
                                Source::IncomingSlot(formal) => {
                                    let source = actual(formal)?;
                                    let location = window_storage(
                                        function,
                                        custody,
                                        types,
                                        source.place,
                                        &source.path,
                                        StructuralAccess::SharedBorrow,
                                    )?;
                                    let leaf = custody
                                        .leaf(location.0, &location.1)
                                        .ok_or_else(|| invalid())?;
                                    let reference_type = leaf_type(
                                        types,
                                        leaf.contract.structural_type(),
                                        &leaf.identity.path,
                                    )
                                    .and_then(|ty| find_structural_type(types, ty))
                                    .ok_or_else(|| invalid())?;
                                    let StructuralTypeShape::Reference {
                                        access: available, ..
                                    } = reference_type.shape
                                    else {
                                        return Err(invalid());
                                    };
                                    if !access_covers(available, *required_access) {
                                        return Err(invalid());
                                    }
                                    let parents = if available == StructuralAccess::SharedBorrow {
                                        leaf.parents.clone()
                                    } else {
                                        vec![leaf.identity.clone()]
                                    };
                                    lifetime_debts.push(lifetime_debts::Debt {
                                        backing: leaf.backing.clone(),
                                        access: *required_access,
                                        parents,
                                    });
                                    lifetime_debts.extend(
                                        custody.captured_lifetime_debts(leaf).into_iter().cloned(),
                                    );
                                }
                                Source::Provider { .. } => return Err(invalid()),
                            }
                        }
                        lifetime_debts.sort();
                        lifetime_debts.dedup();
                        let root = Root::Alternatives(vec![origin]);
                        let contract = receiving_contract(function, &destination)?;
                        let identity = OccurrenceIdentity::operation(&destination, operation);
                        candidates.push(Leaf {
                            identity: identity.clone(),
                            backing: vec![descriptor_backing::Backing::Source(root.clone())],
                            root,
                            lifetime_debts,
                            parents: Vec::new(),
                            formations: vec![Formation {
                                identity,
                                contract: contract.clone(),
                            }],
                            contract,
                            referent,
                            placed: None,
                        });
                        continue;
                    }
                    Source::IncomingSlot(formal) => formal,
                    Source::BorrowedStorage(formal) => {
                        staged.established.insert(destination.clone());
                        let source = actual(formal)?;
                        candidates.push(borrowed_storage(&source)?);
                        continue;
                    }
                };
                let (source, argument) = if formal.place == effect.destination.place
                    && formal.path == effect.destination.path
                {
                    (destination.clone(), None)
                } else {
                    let argument = actual(formal)?;
                    let source = window_storage(
                        function,
                        custody,
                        types,
                        argument.place,
                        &argument.path,
                        StructuralAccess::SharedBorrow,
                    )?;
                    (source, Some(argument))
                };
                let Some(leaf) = custody.leaves.get(&source) else {
                    if source
                        .1
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    {
                        continue;
                    }
                    return Err(invalid());
                };
                if leaf.referent != referent {
                    return Err(invalid());
                }
                let mut leaf = leaf.clone();
                if source != destination {
                    if access == StructuralAccess::SharedBorrow {
                        let reference =
                            leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
                                .and_then(|ty| find_structural_type(types, ty))
                                .ok_or_else(|| invalid())?;
                        match reference.shape {
                            StructuralTypeShape::Reference {
                                access: StructuralAccess::SharedBorrow,
                                ..
                            } => {}
                            StructuralTypeShape::Reference {
                                referent: original,
                                access: StructuralAccess::MutableBorrow,
                            } if original == referent
                                && access_covers(formal.access, StructuralAccess::SharedBorrow) =>
                            {
                                /*
                                    Making a read-only view must leave the stored mutable
                                    reference in place. Keep the original substituted argument
                                    so its reborrow retains the backing and immediate parent.
                                */
                                let mut argument = argument.ok_or_else(|| invalid())?;
                                argument.path.push(StructuralPathSegment::Referent);
                                argument.access = StructuralAccess::SharedBorrow;
                                staged.established.insert(destination.clone());
                                candidates.push(borrowed_storage(&argument)?);
                                continue;
                            }
                            _ => return Err(invalid()),
                        }
                        receiving_identity(function, &destination, operation, &mut leaf)?;
                        staged.established.insert(destination.clone());
                    } else if moved.contains_key(&source) {
                        if staged
                            .relocations
                            .insert(source.clone(), destination.clone())
                            .is_some()
                        {
                            return Err(invalid());
                        }
                    } else {
                        return Err(invalid());
                    }
                }
                candidates.push(leaf);
            }
            let replacement = if candidates.is_empty() {
                None
            } else {
                let joined = candidates.len() > 1;
                let mut leaf = combine_alternatives(candidates)?;
                if joined {
                    receiving_identity(function, &destination, operation, &mut leaf)?;
                    staged.established.insert(destination.clone());
                }
                if custody.leaves.get(&destination).is_some_and(|old| {
                    old.identity == leaf.identity
                        && (staged.established.contains(&destination) || old != &leaf)
                }) && custody.leaves.values().any(|child| {
                    custody.descends_from(child, &leaf.identity)
                        || custody.lifetime_depends_on(child, &leaf.identity)
                }) {
                    return Err(invalid());
                }
                Some(leaf)
            };
            staged.replacements.push((destination, replacement));
        }
        Ok(staged)
    }

    pub(crate) fn commit(
        function: &PsiOptimizationFunction,
        types: &crate::native_geometry::NativeGeometry<'_>,
        result: Option<PlaceId>,
        custody: &mut Custody,
        staged: Staged,
    ) -> Result<(), LegalizationError> {
        for (destination, replacement) in &staged.replacements {
            if let (Some(old), Some(new)) = (custody.leaves.get(destination), replacement)
                && old.identity == new.identity
                && (staged.established.contains(destination) || old != new)
                && staged
                    .replacements
                    .iter()
                    .filter_map(|(_, leaf)| leaf.as_ref())
                    .any(|leaf| leaf.obligations().any(|parent| parent == &old.identity))
            {
                return Err(invalid());
            }
        }
        let access_at = |key: &Location| {
            let ty = source_contract(function, custody, key.0)
                .and_then(|ty| leaf_type(types, ty, &key.1))?;
            match find_structural_type(types, ty)?.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            }
        };
        let mut published = custody
            .leaves
            .iter()
            .filter(|((place, _), _)| Some(*place) == result)
            .map(|(key, leaf)| Some((key.clone(), leaf.root.clone(), access_at(key)?)))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid())?;
        for (destination, leaf) in &staged.replacements {
            let Some(leaf) = leaf else {
                continue;
            };
            let access = access_at(destination).ok_or_else(|| invalid())?;
            if published
                .iter()
                .any(|(previous_destination, root, previous)| {
                    destinations_may_coexist(previous_destination, destination)
                        && roots_overlap(root, &leaf.root)
                        && (access != StructuralAccess::SharedBorrow
                            || *previous != StructuralAccess::SharedBorrow)
                })
            {
                return Err(invalid());
            }
            published.push((destination.clone(), leaf.root.clone(), access));
        }
        for (destination, _) in &staged.replacements {
            if let Some(old) = custody.leaves.remove(destination)
                && !staged.replacements.iter().any(|(_, replacement)| {
                    replacement
                        .as_ref()
                        .is_some_and(|leaf| leaf.identity == old.identity)
                })
            {
                custody.retire(
                    old,
                    super::retirements::DisposalEvent::Operation(staged.operation),
                )?;
            }
        }
        for (destination, replacement) in staged.replacements {
            if let Some(leaf) = replacement {
                if custody
                    .leaves
                    .values()
                    .any(|existing| existing.identity == leaf.identity)
                {
                    return Err(invalid());
                }
                custody.leaves.insert(destination, leaf);
            }
        }
        Ok(())
    }
}
use occurrences::{
    CarrierContract, Formation, OccurrenceIdentity, merge_arrivals, re_home_bound_leaves,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Leaf {
    pub(crate) identity: OccurrenceIdentity,
    pub(crate) root: Root,
    backing: Vec<descriptor_backing::Backing>,
    lifetime_debts: Vec<lifetime_debts::Debt>,
    pub(crate) parents: Vec<OccurrenceIdentity>,
    contract: CarrierContract,
    formations: Vec<Formation>,
    pub(crate) referent: StructuralTypeId,
    placed: Option<target_operations::TargetAccessInterpretation>,
}

pub(crate) use target_operations::ReferenceRoot as Root;

fn storage_origins(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    root: &Root,
    referent: StructuralTypeId,
) -> Result<Vec<target_operations::TargetReferenceOrigin>, LegalizationError> {
    root.origins()
        .into_iter()
        .map(|mut origin| {
            let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
                return Err(invalid());
            };
            let selected = source_contract(function, custody, root)
                .and_then(|contract| {
                    terminal_semantics::captured_reference_origin_type(
                        types.iter(),
                        contract,
                        &origin.path,
                    )
                })
                .ok_or_else(|| invalid())?;
            if selected != referent {
                if function.structural_parameter(root).is_none()
                    || origin.path.iter().any(|segment| {
                        !matches!(
                            segment,
                            ReferenceOriginProjection::Exact(
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::Case(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        )
                    })
                    || !find_structural_type(types, selected).is_some_and(|declaration| {
                        matches!(declaration.shape, StructuralTypeShape::Reference {
                        referent: declared, ..
                    } if declared == referent)
                    })
                {
                    return Err(invalid());
                }
                origin.path.push(ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Referent,
                ));
            }
            Ok(origin)
        })
        .collect()
}

fn roots_overlap(left: &Root, right: &Root) -> bool {
    let left = left.origins();
    let right = right.origins();
    left.iter().any(|left| {
        right.iter().any(|right| {
            !matches!(
                left.base,
                target_operations::TargetReferenceOriginBase::Storage { .. }
            ) || !matches!(
                right.base,
                target_operations::TargetReferenceOriginBase::Storage { .. }
            ) || left.base.root_place() == right.base.root_place()
                && terminal_semantics::reference_origin_paths_may_overlap(&left.path, &right.path)
        })
    })
}

fn roots_definitely_overlap(left: &Root, right: &Root) -> bool {
    let left = left.origins();
    let right = right.origins();
    !left.is_empty()
        && !right.is_empty()
        && left.iter().all(|left| {
            right
                .iter()
                .all(|right| captured_origins_definitely_overlap(left, right))
        })
}

fn captured_origins_definitely_overlap(
    left: &target_operations::TargetReferenceOrigin,
    right: &target_operations::TargetReferenceOrigin,
) -> bool {
    if left.base != right.base
        || !matches!(
            left.base,
            target_operations::TargetReferenceOriginBase::Storage { .. }
                | target_operations::TargetReferenceOriginBase::IncomingContents { .. }
        )
        || left.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
        || right.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
    {
        return false;
    }
    let unknown_or_empty = |segment: &ReferenceOriginProjection| match segment {
        ReferenceOriginProjection::Element
        | ReferenceOriginProjection::Exact(StructuralPathSegment::AllElements) => true,
        ReferenceOriginProjection::Exact(StructuralPathSegment::FixedRange { start, end }) => {
            start >= end
        }
        _ => false,
    };
    if left.path.iter().chain(&right.path).any(unknown_or_empty) {
        return false;
    }
    for (position, (left_segment, right_segment)) in left.path.iter().zip(&right.path).enumerate() {
        let (
            ReferenceOriginProjection::Exact(left_segment),
            ReferenceOriginProjection::Exact(right_segment),
        ) = (left_segment, right_segment)
        else {
            return false;
        };
        match (left_segment, right_segment) {
            (
                StructuralPathSegment::RuntimeIndex { index: left, .. },
                StructuralPathSegment::RuntimeIndex { index: right, .. },
            ) if left == right => {}
            (
                StructuralPathSegment::FixedRange {
                    start: left_start,
                    end: left_end,
                },
                StructuralPathSegment::FixedRange {
                    start: right_start,
                    end: right_end,
                },
            ) => {
                return position + 1 == left.path.len()
                    && position + 1 == right.path.len()
                    && left_start < right_end
                    && right_start < left_end;
            }
            (StructuralPathSegment::AllElements, _) | (_, StructuralPathSegment::AllElements) => {
                return false;
            }
            (left, right) if left == right => {}
            _ => return false,
        }
    }
    true
}

fn duplicate_argument_storage(left: &StructuralArgument, right: &StructuralArgument) -> bool {
    left.place == right.place
        && left.path == right.path
        && !left.path.iter().any(|segment| {
            matches!(segment,
            StructuralPathSegment::FixedRange { start, end } if start >= end)
        })
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Custody {
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    retired: BTreeMap<OccurrenceIdentity, retirements::RetiredOccurrence>,
    edge_exits: Vec<(
        EdgeId,
        BlockId,
        BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
        BTreeMap<OccurrenceIdentity, retirements::RetiredOccurrence>,
        std::sync::Arc<borrowed_arrivals::Sources>,
    )>,
    borrowed_sources: std::sync::Arc<borrowed_arrivals::Sources>,
    descriptor_backing: std::sync::Arc<descriptor_backing::Observations>,
    stored_calls: std::sync::Arc<stored_calls::Effects>,
    reference_position: Option<terminal_semantics::descriptor_lifetimes::DescriptorPosition>,
    placement_catalog: abstract_operations::StructuralTypeCatalog,
    access_formals: BTreeMap<PlaceId, terminal_psi::TerminalAccessViewInput>,
}

impl Custody {
    fn position_is_impossible(&self) -> bool {
        self.reference_position
            .and_then(|position| self.descriptor_backing.flow.get(&position))
            == Some(&terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible)
    }

    fn edge_is_impossible(&self, edge: EdgeId) -> bool {
        self.descriptor_backing.edges.get(&edge)
            == Some(&terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible)
    }

    pub(crate) fn set_position(
        &mut self,
        block: BlockId,
        node_index: usize,
    ) -> Result<(), LegalizationError> {
        self.reference_position = Some((block, u32::try_from(node_index).map_err(|_| invalid())?));
        Ok(())
    }

    fn edge_custody(&self, edge: EdgeId, target: BlockId) -> Result<Self, LegalizationError> {
        let mut matching = self
            .edge_exits
            .iter()
            .filter(|(candidate, block, _, _, _)| *candidate == edge && *block == target);
        let (_, _, leaves, retired, borrowed_sources) = matching.next().ok_or_else(|| invalid())?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            leaves: leaves.clone(),
            retired: retired.clone(),
            edge_exits: Vec::new(),
            borrowed_sources: borrowed_sources.clone(),
            descriptor_backing: self.descriptor_backing.clone(),
            stored_calls: self.stored_calls.clone(),
            reference_position: self.reference_position,
            placement_catalog: self.placement_catalog.clone(),
            access_formals: self.access_formals.clone(),
        })
    }

    pub(crate) fn leaf(&self, place: PlaceId, path: &[StructuralPathSegment]) -> Option<&Leaf> {
        self.leaves.get(&(place, path.to_vec()))
    }

    fn form_leaf(
        &self,
        function: &PsiOptimizationFunction,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &StructuralArgument,
        operation: OperationId,
        root: Root,
        parents: Vec<OccurrenceIdentity>,
        result: &StructuralOperationResult,
    ) -> Result<Leaf, LegalizationError> {
        let referent = referent(types, result.structural_type).ok_or_else(|| invalid())?;
        let (backing, backing_parents) =
            descriptor_backing::capture(function, self, types, source, operation, &root)?;
        let mut leaf = Leaf::formed(
            (result.place, Vec::new()),
            root,
            parents,
            operation,
            result,
            referent,
        );
        leaf.backing = backing;
        leaf.parents.extend(backing_parents);
        leaf.parents.sort();
        leaf.parents.dedup();
        Ok(leaf)
    }

    fn copied_reference(
        &self,
        function: &PsiOptimizationFunction,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &StructuralArgument,
        operation: OperationId,
        result: &StructuralOperationResult,
        path: &[StructuralPathSegment],
    ) -> Result<Option<Leaf>, LegalizationError> {
        let observed = call_paths::observe_reference_custody(function, self, types, source)?;
        if observed.descriptors.is_empty() && observed.borrowed.is_empty() {
            return Ok(None);
        }
        if source.access != StructuralAccess::SharedBorrow
            || !leaf_type(types, result.structural_type, path)
                .and_then(|leaf| find_structural_type(types, leaf))
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                })
        {
            return Err(invalid());
        }
        let expected = leaf_referent(types, result.structural_type, path).ok_or(invalid())?;
        let mut alternatives = Vec::with_capacity(observed.descriptors.len());
        for descriptor in observed.descriptors {
            let source_leaf = self.leaves.get(&descriptor).ok_or(invalid())?;
            if source_leaf.referent != expected {
                return Err(invalid());
            }
            let mut leaf = source_leaf.clone();
            leaf.identity =
                OccurrenceIdentity::operation(&(result.place, path.to_vec()), operation);
            leaf.reform(operation, result);
            alternatives.push(leaf);
        }
        for borrowed in observed.borrowed {
            if borrowed.referent != expected {
                return Err(invalid());
            }
            alternatives.push(Leaf::formed(
                (result.place, path.to_vec()),
                borrowed.root,
                borrowed.parents,
                operation,
                result,
                expected,
            ));
        }
        combine_alternatives(alternatives).map(Some)
    }

    pub(crate) fn readable_leaf(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&Leaf> {
        let leaf = self.leaf(place, path)?;
        (!self.leaves.values().any(|other| {
            other.root.contains_place(place)
                || self.descends_from(other, &leaf.identity)
                || self.lifetime_depends_on(other, &leaf.identity)
        }))
        .then_some(leaf)
    }
}

pub(crate) struct ReferentDestination {
    pub(crate) carrier_type: StructuralTypeId,
    pub(crate) referent: StructuralTypeId,
    pub(crate) carrier_path: Vec<StructuralPathSegment>,
    pub(crate) path: Vec<StructuralPathSegment>,
}

pub(crate) fn referent_destination(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentDestination> {
    optimization_unit_semantics::reference_projection(
        function,
        types,
        carrier,
        path,
        required_access,
    )?;
    window_destination(function, custody, types, carrier, path, required_access)
}

pub(crate) fn window_destination(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentDestination> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &path[..crossing];
    let carrier_type = source_contract(function, custody, carrier)?;
    let descriptor = leaf_type(types, carrier_type, carrier_path)?;
    let StructuralTypeShape::Reference { referent, access } =
        find_structural_type(types, descriptor)?.shape
    else {
        return None;
    };
    if !access_covers(access, required_access) {
        return None;
    }
    call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place: carrier,
            path: path.to_vec(),
            access: required_access,
        },
    )
    .ok()?;
    Some(ReferentDestination {
        carrier_type,
        carrier_path: carrier_path.to_vec(),
        referent,
        path: path[crossing + 1..].to_vec(),
    })
}

pub(crate) fn window_backing_roots(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<PlaceId>, LegalizationError> {
    let observed = call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access,
        },
    )?;
    observed
        .origins
        .iter()
        .flat_map(Root::origins)
        .map(|origin| origin.base.root_place().ok_or_else(|| invalid()))
        .collect()
}

pub(crate) fn window_regions(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<(PlaceId, Vec<StructuralPathSegment>)>, LegalizationError> {
    let sources = borrowed_arrivals::captured_storage(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access,
        },
    )
    .ok_or_else(|| invalid())?;
    let mut regions = Vec::new();
    for source in sources {
        for region in window_regions_in_storage(function, custody, types, &source)? {
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
    }
    Ok(regions)
}

fn window_regions_in_storage(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &borrowed_arrivals::CapturedStorage,
) -> Result<Vec<(PlaceId, Vec<StructuralPathSegment>)>, LegalizationError> {
    let StructuralArgument { place, path, .. } = &source.argument;
    let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    else {
        return Ok(vec![(*place, path.to_vec())]);
    };
    let observed = call_paths::captured_reference_custody(function, custody, types, source)?;
    let mut regions = Vec::new();
    for root in &observed.origins {
        for origin in storage_origins(function, custody, types, root, observed.referent)? {
            let mut prefix = origin.exact_path().ok_or_else(|| invalid())?;
            prefix.extend_from_slice(&path[crossing + 1..]);
            let region = (origin.base.root_place().ok_or_else(|| invalid())?, prefix);
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
    }
    Ok(regions)
}

fn window_storage(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LegalizationError> {
    let mut regions = window_regions(function, custody, types, place, path, access)?;
    if regions.len() != 1 {
        return Err(invalid());
    }
    regions.pop().ok_or_else(|| invalid())
}

#[track_caller]
fn invalid() -> LegalizationError {
    LegalizationError::custody()
}

pub(crate) fn contains_reference(
    types: &crate::native_geometry::NativeGeometry<'_>,
    root: StructuralTypeId,
) -> bool {
    terminal_psi::structural_type_contains_reference(types, root)
}

pub(crate) fn shared_copy_type(
    types: &crate::native_geometry::NativeGeometry<'_>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let Some(declaration) = find_structural_type(types, current) else {
            return false;
        };
        let fields: Vec<_> = match &declaration.shape {
            StructuralTypeShape::Reference { access, .. } => {
                if *access != StructuralAccess::SharedBorrow {
                    return false;
                }
                continue;
            }
            StructuralTypeShape::Record { fields, .. } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .collect(),
            StructuralTypeShape::FixedArray { element, length } => {
                if length.literal() != Some(0) {
                    pending.push(*element);
                }
                continue;
            }
            _ => continue,
        };
        pending.extend(
            fields
                .into_iter()
                .filter(|field| !field.relevance.is_erased())
                .filter_map(|field| field.field_type.as_structural()),
        );
    }
    true
}

fn push_field_leaf(
    pending: &mut Vec<(StructuralTypeId, Vec<StructuralPathSegment>)>,
    path: &[StructuralPathSegment],
    leaves: usize,
    maximum_leaves: usize,
    types: &crate::native_geometry::NativeGeometry<'_>,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> Option<()> {
    if field.relevance.is_erased() {
        return Some(());
    }
    let Some(child) = field.field_type.as_structural() else {
        return Some(());
    };
    if !contains_reference(types, child) {
        return Some(());
    }
    if pending.len().checked_add(leaves)? >= maximum_leaves {
        return None;
    }
    let mut child_path = path.to_vec();
    child_path.push(StructuralPathSegment::Field(field.identity));
    pending.push((child, child_path));
    Some(())
}

fn leaf_paths(
    types: &crate::native_geometry::NativeGeometry<'_>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(types, current) {
            continue;
        }
        let Some(declaration) = find_structural_type(types, current) else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
                continue;
            }
            StructuralTypeShape::Record { fields, .. }
            | StructuralTypeShape::Mixed { fields, .. } => {
                for field in fields.iter().rev() {
                    push_field_leaf(
                        &mut pending,
                        &path,
                        output.len(),
                        maximum_leaves,
                        types,
                        field,
                    )?;
                }
            }
            StructuralTypeShape::FixedArray { element, length }
                if contains_reference(types, *element) =>
            {
                let length = usize::try_from(length.literal()?).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(length)?
                    > maximum_leaves
                {
                    return None;
                }
                for index in (0..length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, child_path));
                }
            }
            _ => {}
        }
        if let Some(cases) = declaration.shape.sum_cases() {
            for case in cases.iter().rev() {
                let mut guarded = path.clone();
                guarded.push(StructuralPathSegment::Case(case.id));
                for field in case.fields.iter().rev() {
                    push_field_leaf(
                        &mut pending,
                        &guarded,
                        output.len(),
                        maximum_leaves,
                        types,
                        field,
                    )?;
                }
            }
        }
    }
    output.sort();
    output.dedup();
    Some(output)
}

fn referent(
    types: &crate::native_geometry::NativeGeometry<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    match find_structural_type(types, structural_type)?.shape {
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } => Some(referent),
        _ => None,
    }
}

fn leaf_referent(
    types: &crate::native_geometry::NativeGeometry<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let leaf = leaf_type(types, current, fields)?;
    match &find_structural_type(types, leaf)?.shape {
        StructuralTypeShape::Reference { .. } => referent(types, leaf),
        _ => Some(leaf),
    }
}

fn leaf_type(
    types: &crate::native_geometry::NativeGeometry<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    terminal_semantics::runtime_structural_path_tip(types.iter(), current, fields)
        .map(|declaration| declaration.id)
        .or_else(|| {
            terminal_semantics::primitive_projection_declaration(types.iter(), current, fields)
        })
}

fn source_contract(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function.structural_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = function
        .block_structural_parameters()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Ok((_, result)) = source_result(function, place) {
        return Some(result.structural_type);
    }
    custody
        .leaf(place, &[])
        .map(|leaf| leaf.contract.structural_type())
}

fn reference_source_type(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !reference_source_accessible(function, custody, types, source) {
        return None;
    }
    let root = source_contract(function, custody, source.place)?;
    let path = terminal_semantics::reference_source_storage_path(types.iter(), root, &source.path);
    if source.access == StructuralAccess::SharedBorrow
        && let Some(observed) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.iter(),
            root,
            path,
            None,
        )
    {
        return Some(observed);
    }
    leaf_type(types, root, path)
}

fn reference_source_accessible(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> bool {
    let Some(root) = source_contract(function, custody, source.place) else {
        return false;
    };
    let (multiplicity, access) = if let Some(parameter) = function.arrival_parameter(source.place) {
        (parameter.multiplicity, parameter.access)
    } else if let Ok((_, result)) = source_result(function, source.place) {
        (result.multiplicity, StructuralAccess::Owned)
    } else {
        return false;
    };
    terminal_semantics::reference_source_projection_accessible(
        types.iter(),
        root,
        multiplicity,
        access,
        &source.path,
        source.access,
    )
}

fn shared_source_compatible(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    carrier: &PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> bool {
    access == StructuralAccess::SharedBorrow
        && source_contract(function, custody, *carrier)
            .and_then(|root| leaf_type(types, root, path))
            .and_then(|leaf| find_structural_type(types, leaf))
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                )
            })
}

fn storage_projection(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> Option<optimization_unit_semantics::StructuralStorageProjection> {
    let mut carrier = source_contract(function, custody, source.place)?;
    let mut path =
        terminal_semantics::reference_source_storage_path(types.iter(), carrier, &source.path);
    if let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let leaf_referent = match custody.leaf(source.place, &path[..crossing]) {
            Some(leaf) => leaf.referent,
            None => {
                let observed = observations::observe_alternatives(
                    function,
                    custody,
                    types,
                    source.place,
                    &path[..crossing],
                )
                .ok()?;
                let (first, rest) = observed.split_first()?;
                rest.iter()
                    .all(|alternative| alternative.referent == first.referent)
                    .then_some(first.referent)?
            }
        };
        let reference = leaf_type(types, carrier, &path[..crossing])?;
        let StructuralTypeShape::Reference { referent, access } =
            find_structural_type(types, reference)?.shape
        else {
            return None;
        };
        if referent != leaf_referent || !source.access.admits_referent(access) {
            return None;
        }
        carrier = leaf_referent;
        path = &path[crossing + 1..];
    }
    optimization_unit_semantics::structural_borrowed_storage_projection(
        types.iter(),
        carrier,
        path,
        source.access,
    )
}

fn access_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> Option<target_operations::TargetAccessInterpretation> {
    if source.path.is_empty() {
        if let Some(origins) = custody.borrowed_sources.get(&source.place) {
            return optimization_unit_semantics::descriptor_results::access_results::agree(
                origins.as_ref()?.iter().map(|origin| origin.placed.clone()),
            );
        }
        return custody
            .access_formals
            .get(&source.place)
            .cloned()
            .map(target_operations::TargetAccessInterpretation::Formal);
    }
    let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    if let Some(leaf) = custody.leaf(source.place, path) {
        return leaf.placed.clone();
    }
    let observed = observations::observe(function, custody, types, source.place, path).ok()?;
    let (place, path) = observed.resident?;
    custody.leaf(place, &path)?.placed.clone()
}

fn normalized_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    borrowed_arrivals::normalize(function, custody, types, source)
}

fn normalized_direct_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let path = source.path.as_slice();
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &path[..crossing];
        let suffix = &path[crossing + 1..];
        let observations = observations::observe_alternatives(
            function,
            custody,
            types,
            source.place,
            carrier_path,
        )?;
        let mut candidates = Vec::with_capacity(observations.len());
        for observed in observations {
            if optimization_unit_semantics::structural_borrowed_storage_projection(
                types.iter(),
                observed.referent,
                suffix,
                source.access,
            )
            .is_none()
            {
                return Err(invalid());
            }
            candidates.push((observed.root.projected(suffix), observed.parents));
        }
        combine_candidate_origins(candidates)
    } else if source.path.is_empty() {
        Ok((Root::Place(source.place), Vec::new()))
    } else if storage_projection(function, custody, types, source).is_some() {
        let root = Root::Ingress(source.place, path.to_vec());
        Ok((root, Vec::new()))
    } else {
        Err(invalid())
    }
}

use terminal_psi::structural_access_covers as access_covers;

pub(crate) mod call_paths {
    use super::borrowed_arrivals::{captured_storage, granting_ancestor};
    use super::{
        Custody, LegalizationError, PsiOptimizationFunction, Root, StructuralAccess,
        StructuralArgument, StructuralPathSegment, StructuralTypeShape, invalid, leaf_type,
        shared_source_compatible, source_contract,
    };
    use crate::structural_inputs::structural_reference_input::reference_formation_geometry;

    pub(crate) struct ReferenceObservation {
        pub(crate) origins: Vec<Root>,
        pub(crate) referent: semantic_vocabulary::StructuralTypeId,
        pub(crate) dependencies: Vec<Root>,
        pub(crate) descriptors: Vec<(semantic_vocabulary::PlaceId, Vec<StructuralPathSegment>)>,
        pub(crate) borrowed: Vec<super::observations::Observed>,
        absent_case_descriptors: bool,
    }

    pub(crate) fn call_reference_custody(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        argument: &StructuralArgument,
    ) -> Result<ReferenceObservation, LegalizationError> {
        let observed = observe_reference_custody(function, custody, types, argument)?;
        require_observation(observed)
    }

    pub(crate) fn captured_reference_custody(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        source: &super::borrowed_arrivals::CapturedStorage,
    ) -> Result<ReferenceObservation, LegalizationError> {
        require_observation(observe_direct_reference_custody(
            function,
            custody,
            types,
            &source.argument,
            &source.grants,
        )?)
    }

    fn require_observation(
        observed: ReferenceObservation,
    ) -> Result<ReferenceObservation, LegalizationError> {
        if observed.absent_case_descriptors || observed.origins.is_empty() {
            return Err(invalid());
        }
        Ok(observed)
    }

    pub(crate) fn observe_reference_custody(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        argument: &StructuralArgument,
    ) -> Result<ReferenceObservation, LegalizationError> {
        if !custody.borrowed_sources.contains_key(&argument.place) {
            return observe_direct_reference_custody(function, custody, types, argument, &[]);
        }
        let roots = captured_storage(
            function,
            custody,
            types,
            &StructuralArgument {
                place: argument.place,
                path: Vec::new(),
                access: argument.access,
            },
        )
        .ok_or_else(|| invalid())?;
        let mut complete: Option<ReferenceObservation> = None;
        for mut root in roots {
            root.argument.path.extend_from_slice(&argument.path);
            root.argument.access = argument.access;
            let observed = observe_direct_reference_custody(
                function,
                custody,
                types,
                &root.argument,
                &root.grants,
            )?;
            if let Some(complete) = &mut complete {
                if complete.referent != observed.referent {
                    return Err(invalid());
                }
                complete.absent_case_descriptors |= observed.absent_case_descriptors;
                for origin in observed.origins {
                    if !complete.origins.contains(&origin) {
                        complete.origins.push(origin);
                    }
                }
                for dependency in observed.dependencies {
                    if !complete.dependencies.contains(&dependency) {
                        complete.dependencies.push(dependency);
                    }
                }
                for descriptor in observed.descriptors {
                    if !complete.descriptors.contains(&descriptor) {
                        complete.descriptors.push(descriptor);
                    }
                }
                complete.borrowed.extend(observed.borrowed);
            } else {
                complete = Some(observed);
            }
        }
        complete.ok_or_else(|| invalid())
    }

    fn observe_direct_reference_custody(
        function: &PsiOptimizationFunction,
        custody: &Custody,
        types: &crate::native_geometry::NativeGeometry<'_>,
        argument: &StructuralArgument,
        grants: &[super::OccurrenceIdentity],
    ) -> Result<ReferenceObservation, LegalizationError> {
        if argument.access == StructuralAccess::Owned {
            return Err(invalid());
        }
        let source_access = function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .find(|parameter| parameter.place == argument.place)
            .map(|parameter| parameter.access)
            .unwrap_or(StructuralAccess::Owned);
        if source_access == StructuralAccess::WriteOnlyBorrow
            || source_access == StructuralAccess::SharedBorrow
                && argument.access != StructuralAccess::SharedBorrow
        {
            return Err(invalid());
        }
        let mut current = source_contract(function, custody, argument.place).ok_or(invalid())?;
        let mut cursors = vec![(argument.place, Vec::new(), grants.to_vec())];
        let cursor_budget = custody
            .leaves
            .len()
            .max(1)
            .checked_mul(argument.path.len())
            .ok_or(invalid())?;
        let mut dependencies = Vec::new();
        let mut prefix_start = 0;
        for (position, segment) in argument.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            let prefix = &argument.path[prefix_start..position];
            let reference_type = leaf_type(types, current, prefix).ok_or(invalid())?;
            let StructuralTypeShape::Reference { referent, access } =
                terminal_psi::find_structural_type(types, reference_type)
                    .ok_or(invalid())?
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
            let mut descriptors = Vec::new();
            let mut borrowed = Vec::new();
            let mut absent_case_descriptors = false;
            for (place, mut path, ancestors) in cursors {
                path.extend_from_slice(prefix);
                let root = source_contract(function, custody, place).ok_or(invalid())?;
                let candidates = terminal_semantics::stored_descriptor_paths(
                    types.iter(),
                    root,
                    &path,
                    cursor_budget,
                )
                .ok_or_else(|| invalid())?;
                for mut path in candidates {
                    if source_contract(function, custody, place)
                        .and_then(|contract| leaf_type(types, contract, &path))
                        != Some(reference_type)
                    {
                        return Err(invalid());
                    }
                    let observed = match super::observations::observe(
                        function, custody, types, place, &path,
                    ) {
                        Ok(observed) => observed,
                        Err(reason) => {
                            if super::descriptor_backing::path_is_impossible(
                                function, custody, types, place, &path,
                            ) {
                                absent_case_descriptors = true;
                                continue;
                            }
                            return Err(reason.into_error(function, custody, types, place, &path));
                        }
                    };
                    let leaf = observed
                        .resident
                        .as_ref()
                        .and_then(|slot| custody.leaves.get(slot));
                    let (observed_carrier_place, observed_carrier_path) = observed
                        .resident
                        .as_ref()
                        .map_or((place, path.as_slice()), |(place, path)| {
                            (*place, path.as_slice())
                        });
                    if observed.referent != referent
                        || leaf.is_some_and(|leaf| {
                            custody.leaves.iter().any(|((carrier, path), child)| {
                                custody.lifetime_access_conflicts(
                                    child,
                                    &leaf.identity,
                                    argument.access,
                                ) || custody.descends_from(child, &leaf.identity)
                                    && !grants.iter().any(|grant| {
                                        granting_ancestor(custody, grant, &child.identity)
                                    })
                                    && !shared_source_compatible(
                                        function,
                                        custody,
                                        types,
                                        carrier,
                                        path,
                                        argument.access,
                                    )
                            })
                        })
                    {
                        return Err(invalid());
                    }
                    let carrier_read = super::CarrierStorageRead::new(
                        function,
                        custody,
                        types,
                        observed_carrier_place,
                        observed_carrier_path,
                    );
                    if let Some((_, enclosing)) = custody.leaves.iter().find(|(_, enclosing)| {
                        super::lifetime_storage_access_conflicts(
                            function,
                            custody,
                            types,
                            enclosing,
                            observed_carrier_place,
                            observed_carrier_path,
                            StructuralAccess::SharedBorrow,
                        ) || !ancestors.contains(&enclosing.identity)
                            && carrier_read.conflicts(enclosing)
                    }) {
                        let mut error = super::observations::Refusal::StorageInterference
                            .into_error(function, custody, types, place, &path);
                        if let LegalizationError::ReferenceObservation { context, .. } = &mut error
                        {
                            context.interference = Some(format!(
                                "carrier={observed_carrier_place:?}/{observed_carrier_path:?}; ancestors={ancestors:?}; loan={enclosing:#?}; debts={:#?}",
                                custody.captured_lifetime_debts(enclosing),
                            ));
                        }
                        return Err(error);
                    }
                    if let Some(slot) = &observed.resident {
                        descriptors.push(slot.clone());
                    }
                    dependencies.push(Root::Ingress(place, path.clone()));
                    if observed_carrier_place != place || observed_carrier_path != path.as_slice() {
                        dependencies.push(Root::Ingress(
                            observed_carrier_place,
                            observed_carrier_path.to_vec(),
                        ));
                    }
                    origins.push(observed.root.clone());
                    if further_crossing {
                        dependencies.push(observed.root.clone());
                        let mut ancestors = ancestors.clone();
                        ancestors.extend(observed.parents.iter().cloned());
                        path.push(StructuralPathSegment::Referent);
                        let cursor = (place, path, ancestors);
                        if !next_cursors.contains(&cursor) {
                            if next_cursors.len() >= cursor_budget {
                                return Err(invalid());
                            }
                            next_cursors.push(cursor);
                        }
                    }
                    if leaf.is_none() {
                        borrowed.push(observed);
                    }
                }
            }
            if !further_crossing {
                if origins.is_empty() && !absent_case_descriptors
                    || reference_formation_geometry(referent, &argument.path[position + 1..], types)
                        .is_none()
                {
                    return Err(invalid());
                }
                return Ok(ReferenceObservation {
                    origins,
                    referent,
                    dependencies,
                    descriptors,
                    borrowed,
                    absent_case_descriptors,
                });
            }
            cursors = next_cursors;
            current = referent;
            prefix_start = position + 1;
        }
        Err(invalid())
    }
}

fn storage_access_conflicts(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    requested: StructuralAccess,
) -> bool {
    custody.leaves.iter().any(|(location, reference)| {
        let conflicts = leaf_storage_access_conflicts(
            function, custody, types, reference, place, path, requested,
        );
        if conflicts {
            trace_storage_conflict(
                function, custody, place, path, requested, location, reference,
            );
        }
        conflicts
    })
}

fn trace_storage_conflict(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    location: &(PlaceId, Vec<StructuralPathSegment>),
    reference: &Leaf,
) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
        eprintln!(
            "CHECKSITE custody-storage-conflict machine={:?} position={:?} place={place:?} path={path:?} access={access:?} conflicting_carrier={location:?} leaf={reference:?}",
            function.machine, custody.reference_position,
        );
    }
}

fn storage_access_conflicts_beneath_argument(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    argument: &StructuralArgument,
) -> Result<bool, LegalizationError> {
    let captured = custody
        .borrowed_sources
        .contains_key(&argument.place)
        .then(|| normalized_source(function, custody, types, argument))
        .transpose()?;
    Ok(custody.leaves.iter().any(|(location, reference)| {
        let (leaf_place, leaf_path) = location;
        let conflict = || {
            let own_key = *leaf_place == argument.place && leaf_path.as_slice() == argument.path;
            let owned_alias = leaf_path.as_slice() == argument.path
                && matches!(
                    &reference.contract,
                    CarrierContract::Parameter(parameter)
                        if parameter.access == StructuralAccess::Owned
                )
                && reference.backing.iter().all(|backing| {
                    matches!(backing, descriptor_backing::Backing::Source(Root::Place(_)))
                });
            if own_key || owned_alias {
                return false;
            }
            if let Some((root, parents)) = &captured {
                if lifetime_storage_access_conflicts(
                    function,
                    custody,
                    types,
                    reference,
                    argument.place,
                    &argument.path,
                    argument.access,
                ) {
                    return true;
                }
                if borrowed_arrivals::lends_from(
                    custody,
                    argument.place,
                    &reference.identity,
                    argument.access,
                ) || shared_source_compatible(
                    function,
                    custody,
                    types,
                    leaf_place,
                    leaf_path,
                    argument.access,
                ) {
                    return false;
                }
                if parents.iter().any(|parent| {
                    custody.descends_from(reference, parent)
                        || custody.lifetime_access_conflicts(reference, parent, argument.access)
                }) {
                    return true;
                }
                return reference.backing.iter().any(|backing| {
                    descriptor_backing::matches_return(std::slice::from_ref(backing), &|backing| {
                        roots_definitely_overlap(backing, root)
                    })
                });
            }
            leaf_storage_access_conflicts(
                function,
                custody,
                types,
                reference,
                argument.place,
                &argument.path,
                argument.access,
            )
        };
        let conflicts = conflict();
        if conflicts {
            trace_storage_conflict(
                function,
                custody,
                argument.place,
                &argument.path,
                argument.access,
                location,
                reference,
            );
        }
        conflicts
    }))
}

fn backing_storage_access_conflicts(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    backing: &descriptor_backing::Backing,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    if custody.borrowed_sources.contains_key(&place) {
        let Some(parameter) = function.arrival_parameter(place) else {
            return true;
        };
        let Ok((root, _)) = normalized_source(
            function,
            custody,
            types,
            &StructuralArgument {
                place,
                path: path.to_vec(),
                access: parameter.access,
            },
        ) else {
            return true;
        };
        return root.origins().iter().any(|origin| {
            let target_operations::TargetReferenceOriginBase::Storage { root: place } = origin.base
            else {
                return true;
            };
            let path = if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                let Some(path) = origin.exact_path() else {
                    return true;
                };
                path
            } else {
                let Some(region_path) = origin
                    .path
                    .iter()
                    .map(|projection| match projection {
                        ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                        ReferenceOriginProjection::Element => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return true;
                };
                if !source_contract(function, custody, place).is_some_and(|root| {
                    terminal_semantics::projection_stays_in_inline_storage(
                        types.iter(),
                        root,
                        &region_path,
                    )
                }) {
                    return true;
                }
                Vec::new()
            };
            custody.borrowed_sources.contains_key(&place)
                || backing_intersects_storage(function, custody, types, backing, place, &path)
        });
    }
    backing_intersects_storage(function, custody, types, backing, place, path)
}

fn backing_intersects_storage(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    backing: &descriptor_backing::Backing,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    let activation_local = function.arrival_parameter(place).map_or_else(
        || source_result(function, place).is_ok(),
        |parameter| parameter.access == StructuralAccess::Owned,
    ) && source_contract(function, custody, place).is_some_and(|root| {
        terminal_semantics::projection_stays_in_inline_storage(types.iter(), root, path)
    });
    let independent_carrier = activation_local
        || function
            .structural_parameter(place)
            .is_some_and(|parameter| {
                terminal_semantics::projection_stays_in_inline_storage(
                    types.iter(),
                    parameter.structural_type,
                    path,
                )
            });
    let origin_overlaps = |origin_path: &[ReferenceOriginProjection]| {
        let Some(origin) = source_contract(function, custody, place)
            .and_then(|root| {
                terminal_semantics::captured_reference_origin_type(types.iter(), root, origin_path)
            })
            .and_then(|identity| find_structural_type(types, identity))
        else {
            return true;
        };
        !matches!(origin.shape, StructuralTypeShape::Reference { .. })
            && terminal_semantics::reference_origin_paths_may_overlap(
                &path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>(),
                origin_path,
            )
    };
    if independent_carrier
        && let descriptor_backing::Backing::Source(Root::Alternatives(origins)) = backing
    {
        return origins.iter().any(|origin| {
            !matches!(
                origin.base,
                target_operations::TargetReferenceOriginBase::Provider { .. }
            ) && descriptor_backing::origin_overlaps_storage(
                origin,
                place,
                independent_carrier,
                &origin_overlaps,
            )
        });
    }
    descriptor_backing::overlaps(
        backing,
        function.machine,
        place,
        path,
        independent_carrier,
        &origin_overlaps,
    )
}

fn leaf_reference_access(
    types: &crate::native_geometry::NativeGeometry<'_>,
    reference: &Leaf,
) -> Option<StructuralAccess> {
    leaf_type(
        types,
        reference.contract.structural_type(),
        &reference.identity.path,
    )
    .and_then(|identity| find_structural_type(types, identity))
    .and_then(|declaration| match declaration.shape {
        StructuralTypeShape::Reference { access, .. } => Some(access),
        _ => None,
    })
}

fn lifetime_storage_access_conflicts(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    reference: &Leaf,
    place: PlaceId,
    path: &[StructuralPathSegment],
    requested: StructuralAccess,
) -> bool {
    if borrowed_arrivals::lends_from(custody, place, &reference.identity, requested) {
        return false;
    }
    custody
        .captured_lifetime_debts(reference)
        .iter()
        .any(|debt| {
            (requested != StructuralAccess::SharedBorrow
                || debt.access != StructuralAccess::SharedBorrow)
                && debt.backing.iter().any(|backing| {
                    backing_storage_access_conflicts(function, custody, types, backing, place, path)
                })
        })
}

fn leaf_storage_access_conflicts(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    reference: &Leaf,
    place: PlaceId,
    path: &[StructuralPathSegment],
    requested: StructuralAccess,
) -> bool {
    if lifetime_storage_access_conflicts(
        function, custody, types, reference, place, path, requested,
    ) {
        return true;
    }
    if borrowed_arrivals::lends_from(custody, place, &reference.identity, requested) {
        return false;
    }
    let permission = leaf_reference_access(types, reference);
    if requested == StructuralAccess::SharedBorrow
        && permission == Some(StructuralAccess::SharedBorrow)
    {
        return false;
    }
    reference.backing.iter().any(|backing| {
        backing_storage_access_conflicts(function, custody, types, backing, place, path)
    })
}

fn call_arguments(
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    custody: &Custody,
    arguments: &[StructuralArgument],
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>, LegalizationError> {
    let mut normalized = Vec::new();
    let mut dependencies = Vec::new();
    let mut moved = BTreeMap::new();
    for (argument_position, argument) in arguments.iter().enumerate() {
        if argument.path.contains(&StructuralPathSegment::Referent) {
            let resolved = call_paths::call_reference_custody(function, custody, types, argument)?;
            dependencies.push((argument_position, resolved.dependencies));
            normalized.push((
                argument_position,
                resolved.origins,
                argument.access,
                CallRegion::Referent,
                None,
            ));
            continue;
        }
        let argument_contract = source_contract(function, custody, argument.place);
        if argument_contract.is_some_and(|contract| contains_reference(types, contract)) {
            let authority = function
                .arrival_parameter(argument.place)
                .map(|parameter| parameter.access)
                .unwrap_or(StructuralAccess::Owned);
            if !access_covers(authority, argument.access) {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
            let selected = leaf_type(types, contract, &argument.path).ok_or_else(|| invalid())?;
            if contains_reference(types, selected)
                && argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
            {
                return Err(invalid());
            }
            if !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Case(_)
                        | StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            }) {
                return Err(invalid());
            }
            let storage = Root::Ingress(argument.place, argument.path.clone());
            if storage_access_conflicts(
                function,
                custody,
                types,
                argument.place,
                &argument.path,
                argument.access,
            ) {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    eprintln!(
                        "CHECKSITE custody-call-conflict machine={:?} position={:?} kind=carrier-storage argument={argument_position} place={:?} path={:?} access={:?} selected_type={selected:?}",
                        function.machine,
                        custody.reference_position,
                        argument.place,
                        argument.path,
                        argument.access,
                    );
                }
                return Err(invalid());
            }
            normalized.push((
                argument_position,
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
                None,
            ));
            if argument.access == StructuralAccess::WriteOnlyBorrow {
                continue;
            }
            for ((carrier, carrier_path), leaf) in &custody.leaves {
                if *carrier != argument.place {
                    continue;
                }
                let Some(path) = owned_relative_path(carrier_path, &argument.path) else {
                    continue;
                };
                let permission = leaf_type(types, selected, path)
                    .and_then(|leaf| {
                        find_structural_type(types, leaf).map(|declaration| &declaration.shape)
                    })
                    .and_then(|shape| match shape {
                        StructuralTypeShape::Reference { access, .. } => Some(*access),
                        _ => None,
                    })
                    .ok_or_else(|| invalid())?;
                let supplied = if access_covers(argument.access, permission) {
                    permission
                } else {
                    argument.access
                };
                if custody.leaves.iter().any(|((carrier, path), child)| {
                    custody.lifetime_access_conflicts(child, &leaf.identity, supplied)
                        || custody.descends_from(child, &leaf.identity)
                            && (argument.access == StructuralAccess::Owned
                                || !shared_source_compatible(
                                    function, custody, types, carrier, path, supplied,
                                ))
                }) || (argument.access == StructuralAccess::Owned
                    && moved
                        .insert((argument.place, carrier_path.clone()), leaf.clone())
                        .is_some())
                {
                    return Err(invalid());
                }
                normalized.push((
                    argument_position,
                    vec![leaf.root.clone()],
                    supplied,
                    CallRegion::Referent,
                    Some((argument.place, carrier_path.clone())),
                ));
            }
            continue;
        }
        if storage_access_conflicts_beneath_argument(function, custody, types, argument)? {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE custody-call-conflict machine={:?} position={:?} kind=argument-storage argument={argument_position} place={:?} path={:?} access={:?} root_type={argument_contract:?}",
                    function.machine,
                    custody.reference_position,
                    argument.place,
                    argument.path,
                    argument.access,
                );
            }
            return Err(invalid());
        }
        let (root, _) = normalized_source(function, custody, types, argument)?;
        normalized.push((
            argument_position,
            vec![root],
            argument.access,
            if custody.borrowed_sources.contains_key(&argument.place) {
                CallRegion::Referent
            } else {
                CallRegion::OrdinaryStorage
            },
            None,
        ));
    }
    for (position, (argument_position, roots, access, reference_access, descriptor)) in
        normalized.iter().enumerate()
    {
        if normalized[position + 1..].iter().any(
            |(other_position, others, other_access, other_reference_access, other_descriptor)| {
                let alternatives = descriptor
                    .as_ref()
                    .zip(other_descriptor.as_ref())
                    .is_some_and(|(left, right)| {
                        left.0 == right.0
                            && left
                                .1
                                .iter()
                                .zip(&right.1)
                                .find(|(left, right)| left != right)
                                .is_some_and(|(left, right)| {
                                    matches!(
                                        (left, right),
                                        (
                                            StructuralPathSegment::Case(_),
                                            StructuralPathSegment::Case(_)
                                        )
                                    )
                                })
                    });
                !alternatives
                    && (*reference_access != CallRegion::OrdinaryStorage
                        || *other_reference_access != CallRegion::OrdinaryStorage)
                    && (argument_position != other_position
                        || (*reference_access == CallRegion::Referent
                            && *other_reference_access == CallRegion::Referent))
                    && ((argument_position != other_position
                        && duplicate_argument_storage(
                            &arguments[*argument_position],
                            &arguments[*other_position],
                        ))
                        || roots.iter().any(|root| {
                            others
                                .iter()
                                .any(|other| roots_definitely_overlap(root, other))
                        }))
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
            },
        ) {
            return Err(invalid());
        }
    }
    for (argument_position, roots) in dependencies {
        if arguments.iter().enumerate().any(|(other_position, other)| {
            other_position != argument_position
                && other.access != StructuralAccess::SharedBorrow
                && !other.path.contains(&StructuralPathSegment::Referent)
                && roots.iter().any(|root| {
                    roots_overlap(root, &Root::Ingress(other.place, other.path.clone()))
                })
        }) {
            return Err(invalid());
        }
        if normalized
            .iter()
            .any(|(other_position, others, access, _, _)| {
                *other_position != argument_position
                    && *access != StructuralAccess::SharedBorrow
                    && roots
                        .iter()
                        .any(|root| others.iter().any(|other| roots_overlap(root, other)))
            })
        {
            return Err(invalid());
        }
    }
    Ok(moved)
}

fn retire_consumed_arguments(
    custody: &mut Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    operation: OperationId,
) -> Result<(), LegalizationError> {
    let mut occurrences = BTreeMap::new();
    for leaf in moved.values() {
        if occurrences
            .insert(&leaf.identity, leaf)
            .is_some_and(|previous| previous != leaf)
        {
            return Err(invalid());
        }
    }
    for leaf in occurrences.into_values() {
        if !custody
            .leaves
            .values()
            .any(|returned| returned.identity == leaf.identity)
        {
            custody.retire(
                leaf.clone(),
                retirements::DisposalEvent::NormalCall(operation),
            )?;
        }
    }
    Ok(())
}

fn call_results(
    function: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    contract: terminal_semantics::ReferenceCallContract<'_>,
    types: &crate::native_geometry::NativeGeometry<'_>,
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    structural_arguments: &[StructuralArgument],
    custody: &mut Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(Vec::new());
    }
    let mappings = terminal_semantics::bind_reference_call_results(
        types.iter(),
        contract,
        structural_arguments,
    )
    .map_err(|reason| LegalizationError::ReferenceResult { reason })?;
    if custody
        .leaves
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(invalid());
    }
    let mut established = Vec::new();
    let mut reference_results = Vec::new();
    for mapping in mappings {
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(|| invalid())?;
        let result_access = leaf_type(types, result.structural_type, &mapping.path)
            .and_then(|leaf| find_structural_type(types, leaf))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(|| invalid())?;
        if mapping.sources.is_empty() {
            return Err(invalid());
        }
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        let mut public_interpretations = Vec::with_capacity(mapping.sources.len());
        for binding in &mapping.sources {
            let (source, transfers_existing) = match &binding.origin {
                terminal_semantics::ReferenceCallOrigin::Parameter {
                    source,
                    transfers_existing,
                } => (source, *transfers_existing),
                terminal_semantics::ReferenceCallOrigin::Invocation {
                    carrier_type,
                    path: origin_path,
                    access,
                } => {
                    if !access_covers(*access, result_access)
                        || binding.endpoint != expected
                        || binding.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    {
                        return Err(invalid());
                    }
                    let root = Root::Alternatives(vec![target_operations::TargetReferenceOrigin {
                        base: target_operations::TargetReferenceOriginBase::Invocation {
                            root: result.place,
                            carrier_type: *carrier_type,
                        },
                        path: origin_path.clone(),
                        extent: binding.extent,
                    }]);
                    candidates.push(Leaf::formed(
                        (result.place, mapping.path.clone()),
                        root,
                        Vec::new(),
                        psi_operation,
                        result,
                        expected,
                    ));
                    public_interpretations.push(None);
                    continue;
                }
            };
            if !access_covers(source.access, result_access) {
                return Err(invalid());
            }
            let suffix = reference_source_type(function, custody, types, source)
                .and_then(|root| {
                    binding.projected_origin_suffix(types.iter(), root, expected, result_access)
                })
                .ok_or_else(|| invalid())?;
            let mut leaf = if transfers_existing {
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return Err(invalid());
                };
                if let Some(leaf) = moved.get(&(source.place, carrier_path.to_vec())) {
                    leaf.clone()
                } else if let Some(leaf) = custody.copied_reference(
                    function,
                    types,
                    source,
                    psi_operation,
                    result,
                    &mapping.path,
                )? {
                    leaf
                } else {
                    continue;
                }
            } else {
                let (root, parents) = normalized_source(function, custody, types, source)?;
                let root = root.projected_family(&suffix, binding.extent);
                let (backing, backing_parents) = descriptor_backing::capture(
                    function,
                    custody,
                    types,
                    source,
                    psi_operation,
                    &root,
                )?;
                let mut leaf = Leaf::formed(
                    (result.place, mapping.path.clone()),
                    root,
                    parents,
                    psi_operation,
                    result,
                    expected,
                );
                leaf.placed = access_source(function, custody, types, source);
                leaf.backing = backing;
                leaf.parents.extend(backing_parents);
                leaf.parents.sort();
                leaf.parents.dedup();
                leaf
            };
            public_interpretations.push(leaf.placed.take().filter(|_| {
                binding.suffix.is_empty()
                    && binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                    && leaf.referent == expected
            }));
            candidates.push(leaf);
        }
        if candidates.is_empty() {
            continue;
        }
        let joining = candidates.len() > 1;
        let mut leaf = combine_alternatives(candidates)?;
        if custody
            .placement_catalog
            .access_views()
            .iter()
            .any(|plan| plan.carrier == expected)
        {
            let private = callee.and_then(|callee| {
                let slots = custody
                    .stored_calls
                    .access_results
                    .get(&callee.machine)?
                    .as_ref()?;
                optimization_unit_semantics::descriptor_results::access_results::resolve(
                    slots,
                    &mapping.path,
                    |formal| {
                        let actual = abstract_operations::call_source(
                            &callee.structural_parameters,
                            structural_arguments,
                            formal,
                        )?;
                        if let Some((StructuralPathSegment::Referent, path)) =
                            actual.path.split_last()
                            && let Some(transferred) = moved.get(&(actual.place, path.to_vec()))
                        {
                            return transferred.placed.clone();
                        }
                        access_source(function, custody, types, &actual)
                    },
                    |correspondence| {
                        target_operations::TargetAccessInterpretation::Captured(
                            correspondence.clone(),
                        )
                    },
                )
            });
            let association =
                optimization_unit_semantics::descriptor_results::access_results::reconcile(
                    optimization_unit_semantics::descriptor_results::access_results::agree(
                        public_interpretations,
                    ),
                    private,
                )
                .ok_or(LegalizationError::ReferenceResult {
                    reason: "placed result has no complete whole-view interpretation",
                })?;
            leaf.placed = Some(association);
        }
        if joining {
            leaf.identity =
                OccurrenceIdentity::operation(&(result.place, mapping.path.clone()), psi_operation);
            leaf.reform(psi_operation, result);
        }
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: leaf.root.origins(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        custody.leaves.insert(key, leaf);
    }
    Ok(reference_results)
}

fn combine_alternatives(candidates: Vec<Leaf>) -> Result<Leaf, LegalizationError> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next().ok_or(invalid())?;
    let mut origins = vec![(leaf.root.clone(), leaf.parents.clone())];
    for candidate in candidates {
        if leaf.referent != candidate.referent || leaf.placed != candidate.placed {
            return Err(invalid());
        }
        origins.push((candidate.root, candidate.parents));
        leaf.backing.extend(candidate.backing);
        leaf.lifetime_debts.extend(candidate.lifetime_debts);
        leaf.formations.extend(candidate.formations);
    }
    leaf.backing.sort();
    leaf.backing.dedup();
    leaf.lifetime_debts.sort();
    leaf.lifetime_debts.dedup();
    occurrences::normalize_formations(&mut leaf.formations)?;
    (leaf.root, leaf.parents) = combine_candidate_origins(origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    candidates: Vec<(Root, Vec<OccurrenceIdentity>)>,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let mut candidates = candidates.into_iter();
    let (root, parents) = candidates.next().ok_or(invalid())?;
    if candidates.len() == 0 {
        return Ok((root, parents));
    }
    let mut origins = Vec::new();
    let mut immediate_parents = Vec::new();
    for (root, parents) in std::iter::once((root, parents)).chain(candidates) {
        origins.extend(root.origins());
        immediate_parents.extend(parents);
    }
    origins.sort();
    origins.dedup();
    immediate_parents.sort();
    immediate_parents.dedup();
    Ok((Root::Alternatives(origins), immediate_parents))
}

fn owned_relative_path<'a>(
    path: &'a [StructuralPathSegment],
    prefix: &[StructuralPathSegment],
) -> Option<&'a [StructuralPathSegment]> {
    path.strip_prefix(prefix)
        .filter(|relative| !relative.contains(&StructuralPathSegment::Referent))
}

fn discard_owned(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: PlaceId,
    event: retirements::DisposalEvent,
) -> Result<bool, LegalizationError> {
    discard_subtree(function, custody, types, source, &[], event)
}

fn discard_subtree(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: PlaceId,
    prefix: &[StructuralPathSegment],
    event: retirements::DisposalEvent,
) -> Result<bool, LegalizationError> {
    let Some(contract) = source_contract(function, custody, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix).ok_or(invalid())?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    let released: Vec<_> = custody
        .leaves
        .iter()
        .filter(|((carrier, path), _)| {
            *carrier == source && owned_relative_path(path, prefix).is_some()
        })
        .map(|(location, _)| location.clone())
        .collect();
    for location in released {
        let leaf = custody.leaves.remove(&location).ok_or_else(|| invalid())?;
        custody.retire(leaf, event)?;
    }
    custody.prune_retired();
    Ok(true)
}

fn discard_residual(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    discard: &terminal_psi::StructuralAffineDiscard,
    event: retirements::DisposalEvent,
) -> Result<(), LegalizationError> {
    let root = source_contract(function, custody, discard.place).ok_or(invalid())?;
    let selected = leaf_type(types, root, &discard.path);
    if selected != Some(discard.structural_type) {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE custody-discard-conflict machine={:?} position={:?} kind=type-path place={:?} path={:?} selected_type={selected:?} expected_type={:?} event={event:?}",
                function.machine,
                custody.reference_position,
                discard.place,
                discard.path,
                discard.structural_type,
            );
        }
        return Err(invalid());
    }
    if storage_access_conflicts(
        function,
        custody,
        types,
        discard.place,
        &discard.path,
        StructuralAccess::Owned,
    ) {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE custody-discard-conflict machine={:?} position={:?} kind=storage place={:?} path={:?} access=Owned selected_type={selected:?} event={event:?}",
                function.machine, custody.reference_position, discard.place, discard.path,
            );
        }
        return Err(invalid());
    }
    discard_subtree(
        function,
        custody,
        types,
        discard.place,
        &discard.path,
        event,
    )?;
    Ok(())
}

#[derive(Clone, Copy)]
enum OwnedOperandUse {
    CopyUnrestricted,
    Transfer,
}

fn relocate_subtree_leaves<'a>(
    custody: &mut Custody,
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    destination: PlaceId,
    operation: OperationId,
    operand_use: OwnedOperandUse,
    operands: impl IntoIterator<
        Item = (
            &'a StructuralArgument,
            StructuralTypeId,
            Vec<StructuralPathSegment>,
        ),
    >,
) -> Result<(), LegalizationError> {
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    for (argument, nested, prefix) in operands {
        if !contains_reference(types, nested) {
            continue;
        }
        if custody.leaves.keys().any(|(carrier, path)| {
            *carrier == destination && owned_relative_path(path, &prefix).is_some()
        }) {
            return Err(invalid());
        }
        let contract = source_contract(function, custody, argument.place).ok_or(invalid())?;
        let parameter = function.structural_parameter(argument.place).or_else(|| {
            function
                .block_structural_parameters()
                .find(|parameter| parameter.place == argument.place)
        });
        let copies_storage = matches!(operand_use, OwnedOperandUse::CopyUnrestricted)
            && parameter.map_or_else(
                || {
                    source_result(function, argument.place).is_ok_and(|(_, result)| {
                        result.multiplicity == StructuralMultiplicity::Unrestricted
                    })
                },
                |parameter| {
                    parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                },
            );
        if argument.access != StructuralAccess::Owned
            || argument.place == destination
            || leaf_type(types, contract, &argument.path) != Some(nested)
        {
            return Err(invalid());
        }
        for ((carrier, source_path), leaf) in &custody.leaves {
            if *carrier != argument.place {
                continue;
            }
            let Some(path) = owned_relative_path(source_path, &argument.path) else {
                continue;
            };
            if leaf_referent(types, nested, path) != Some(leaf.referent) {
                return Err(invalid());
            }
            let mut target_path = prefix.clone();
            target_path.extend(path.iter().cloned());
            let mut leaf = leaf.clone();
            if copies_storage {
                let reference_type = leaf_type(types, nested, path).ok_or(invalid())?;
                if !find_structural_type(types, reference_type).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                }) {
                    return Err(invalid());
                }
                let mut observation = source_path.clone();
                observation.push(StructuralPathSegment::Referent);
                call_paths::call_reference_custody(
                    function,
                    custody,
                    types,
                    &StructuralArgument {
                        place: argument.place,
                        path: observation,
                        access: StructuralAccess::SharedBorrow,
                    },
                )?;
                leaf.identity =
                    OccurrenceIdentity::operation(&(destination, target_path.clone()), operation);
                if let Ok((declaration_operation, result)) = source_result(function, destination) {
                    leaf.contract = CarrierContract::Operation {
                        operation: declaration_operation,
                        result: result.clone(),
                    };
                } else {
                    let parameter = function
                        .structural_parameter(destination)
                        .or_else(|| {
                            function
                                .block_structural_parameters()
                                .find(|parameter| parameter.place == destination)
                        })
                        .ok_or(invalid())?;
                    leaf.contract = CarrierContract::Parameter(parameter.clone());
                }
                leaf.formations = vec![Formation {
                    identity: leaf.identity.clone(),
                    contract: leaf.contract.clone(),
                }];
            } else {
                let source = (argument.place, source_path.clone());
                if !moved.insert(source.clone()) {
                    return Err(invalid());
                }
            }
            relocations.push((leaf, target_path));
        }
    }
    for source in moved {
        custody.leaves.remove(&source).ok_or(invalid())?;
    }
    for (leaf, to) in relocations {
        custody.leaves.insert((destination, to), leaf);
    }
    Ok(())
}

pub(crate) fn apply(
    custody: &mut Custody,
    node: &optimization_unit::OptimizationNode,
    function: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    plan: &crate::native_geometry::NativePlan<'_>,
    unit: &crate::native_geometry::NativeUnit<'_>,
) -> Result<(), LegalizationError> {
    if custody.position_is_impossible() {
        return Ok(());
    }
    let installed =
        crate::legalization::scalar_graph_input::call_origin::selected_boundary_operation(
            node, function, native, plan,
        )?;
    let operation = installed
        .as_ref()
        .map(|(operation, _)| operation)
        .unwrap_or(&node.operation);
    let types = plan.geometry();
    match operation {
        AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            structural_arguments,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } if plan.structural_types.access_lifecycles().iter().any(|row| {
            row.boundary == *boundary
                && row.operation == terminal_psi::TerminalAccessLifecycleOperation::View
        }) =>
        {
            let declaration = unit
                .boundary_machines
                .iter()
                .find(|row| row.id == *boundary)
                .ok_or_else(|| invalid())?;
            let (path, interpretation) =
                borrowed_view_formation(&plan.structural_types, declaration, result)
                    .ok_or_else(|| invalid())?;
            let [source, _profile] = structural_arguments.as_slice() else {
                return Err(invalid());
            };
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            if !moved.is_empty()
                || !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
                || declaration
                    .structural_parameters
                    .first()
                    .is_none_or(|parameter| parameter.access != source.access)
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
            {
                return Err(invalid());
            }
            let (root, parents) = normalized_source(function, custody, types, source)?;
            let (backing, backing_parents) = descriptor_backing::capture(
                function,
                custody,
                types,
                source,
                *psi_operation,
                &root,
            )?;
            let mut leaf = Leaf::formed(
                (result.place, path.clone()),
                root,
                parents,
                *psi_operation,
                result,
                interpretation.carrier(),
            );
            leaf.backing = backing;
            leaf.parents.extend(backing_parents);
            leaf.parents.sort();
            leaf.parents.dedup();
            leaf.placed = Some(target_operations::TargetAccessInterpretation::Captured(
                interpretation,
            ));
            custody.leaves.insert((result.place, path), leaf);
        }
        AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            source,
            path,
        } if contains_reference(types, result.structural_type) => {
            let source_type = source_contract(function, custody, *source).ok_or(invalid())?;
            let path: std::borrow::Cow<'_, [StructuralPathSegment]> = match path {
                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                    std::borrow::Cow::Borrowed(path)
                }
                abstract_operations::StructuralCopyPath::Case(path) => std::borrow::Cow::Owned(
                    terminal_semantics::canonical_structural_storage_path(
                        types.iter(),
                        source_type,
                        path,
                    )
                    .ok_or(invalid())?,
                ),
            };
            let shared_descriptor =
                terminal_semantics::is_copyable_carrier(types.iter(), result.structural_type);
            if !shared_copy_type(types, result.structural_type)
                || (result.multiplicity != StructuralMultiplicity::Unrestricted
                    && !(result.multiplicity == StructuralMultiplicity::Affine
                        && shared_descriptor))
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || source_contract(function, custody, *source)
                    .and_then(|root| leaf_type(types, root, &path))
                    != Some(result.structural_type)
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
            {
                return Err(invalid());
            }
            let paths = leaf_paths(types, result.structural_type, 4096)
                .filter(|paths| !paths.is_empty())
                .ok_or(invalid())?;
            let mut copied = Vec::with_capacity(paths.len());
            for relative in &paths {
                let mut selected = path.to_vec();
                selected.extend_from_slice(relative);
                let mut observation = selected.clone();
                observation.push(StructuralPathSegment::Referent);
                if let Some(leaf) = custody.copied_reference(
                    function,
                    types,
                    &StructuralArgument {
                        place: *source,
                        path: observation,
                        access: StructuralAccess::SharedBorrow,
                    },
                    *psi_operation,
                    result,
                    relative,
                )? {
                    copied.push(((result.place, relative.clone()), leaf));
                }
            }
            if custody.leaves.keys().any(|(carrier, selected)| {
                *carrier == *source
                    && owned_relative_path(selected, path.as_ref())
                        .is_some_and(|relative| !paths.iter().any(|expected| expected == relative))
            }) {
                return Err(invalid());
            }
            custody.leaves.extend(copied);
        }
        AbstractOperation::EstablishAccessReference {
            psi_operation,
            result,
            source,
            correspondence,
        } => {
            let referent = referent(types, result.structural_type).ok_or_else(|| invalid())?;
            let mut selected = plan
                .structural_types
                .access_views()
                .iter()
                .filter(|plan| plan.carrier == referent);
            let (Some(selected), None) = (selected.next(), selected.next()) else {
                return Err(invalid());
            };
            let actual =
                reference_source_type(function, custody, types, source).ok_or_else(|| invalid())?;
            let matches_backing = validate_backing(
                types,
                plan.structural_types.access_views(),
                selected,
                actual,
                correspondence,
            )
            .is_ok();
            if !matches_backing
                || !result.is_affine_plain()
                || !matches!(
                    source.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                || !find_structural_type(types, result.structural_type).is_some_and(|row| {
                    matches!(row.shape, StructuralTypeShape::Reference { access, .. }
                        if access.admits(source.access))
                })
                || custody.leaves.keys().any(|(carrier, path)| {
                    *carrier == result.place && owned_relative_path(path, &[]).is_some()
                })
                || custody
                    .leaves
                    .values()
                    .any(|leaf| leaf.identity.place == result.place)
            {
                return Err(invalid());
            }
            let (root, parents) = normalized_source(function, custody, types, source)?;
            let psi = *psi_operation;
            let mut leaf =
                custody.form_leaf(function, types, source, psi, root, parents, result)?;
            leaf.placed = Some(target_operations::TargetAccessInterpretation::Captured(
                correspondence.clone(),
            ));
            custody.leaves.insert((result.place, Vec::new()), leaf);
        }
        AbstractOperation::EstablishReference {
            psi_operation,
            result,
            source,
            storage,
        } => {
            let referent = referent(types, result.structural_type).ok_or_else(|| invalid())?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
                || !reference_source_accessible(function, custody, types, source)
                || !match storage {
                    Some(storage)
                        if matches!(
                            source.path.last(),
                            Some(StructuralPathSegment::FixedRange { .. })
                        ) =>
                    {
                        source_contract(function, custody, source.place).is_some_and(|root| {
                            terminal_semantics::reference_storage_region_source_compatible(
                                types.iter(),
                                root,
                                &source.path,
                                result.structural_type,
                                storage.stored_scalar,
                            )
                        })
                    }
                    Some(storage) => reference_source_type(function, custody, types, source)
                        .is_some_and(|actual| {
                            terminal_semantics::reference_storage_source_compatible(
                                types.iter(),
                                actual,
                                result.structural_type,
                                storage.stored_scalar,
                            )
                        }),
                    None => source_contract(function, custody, source.place).is_some_and(|root| {
                        terminal_semantics::reference_source_projection_compatible(
                            types.iter(),
                            root,
                            &source.path,
                            result.structural_type,
                        )
                    }),
                }
            {
                return Err(invalid());
            }
            let (mut root, parents) = normalized_source(function, custody, types, source)?;
            if let Some(storage) = storage {
                let mut offsets = function
                    .block_nodes()
                    .filter_map(|node| match node.operation {
                        AbstractOperation::IntegerConstant { result, value, .. }
                            if result == storage.byte_offset =>
                        {
                            Some(value)
                        }
                        _ => None,
                    });
                if offsets.next() != Some(semantic_vocabulary::IntegerValue::Unsigned(0))
                    || offsets.next().is_some()
                {
                    return Err(invalid());
                }
                terminal_semantics::representation_view_declaration(
                    types.iter(),
                    referent,
                    storage.stored_scalar,
                )
                .ok_or_else(|| invalid())?;
                root = root.projected(&[StructuralPathSegment::RepresentationView {
                    structural_type: referent,
                    stored_scalar: storage.stored_scalar,
                }]);
            }
            let psi = *psi_operation;
            let mut leaf =
                custody.form_leaf(function, types, source, psi, root, parents, result)?;
            leaf.placed = access_source(function, custody, types, source);
            custody.leaves.insert((result.place, Vec::new()), leaf);
        }
        AbstractOperation::ReleaseReference {
            psi_operation,
            source,
            path,
            ..
        } => {
            if custody
                .leaves
                .iter()
                .any(|((carrier, carrier_path), enclosing)| {
                    (*carrier != *source || carrier_path != path)
                        && leaf_storage_access_conflicts(
                            function,
                            custody,
                            types,
                            enclosing,
                            *source,
                            path,
                            StructuralAccess::Owned,
                        )
                })
            {
                return Err(invalid());
            }
            let key = (*source, path.clone());
            let leaf = custody.leaves.remove(&key);
            let Some(leaf) = leaf else {
                return Err(invalid());
            };
            custody.retire(leaf, retirements::DisposalEvent::Operation(*psi_operation))?;
        }
        AbstractOperation::MoveStructuralValue {
            psi_operation,
            result,
            source,
        }
        | AbstractOperation::TransformContent {
            psi_operation,
            result,
            source,
            ..
        } => {
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                *psi_operation,
                OwnedOperandUse::Transfer,
                std::iter::once((source, result.structural_type, Vec::new())),
            )?;
        }
        AbstractOperation::EstablishRecord { result, fields, .. }
        | AbstractOperation::EstablishCase { result, fields, .. } => {
            if custody.leaves.keys().any(|(carrier, path)| {
                *carrier == result.place && owned_relative_path(path, &[]).is_some()
            }) {
                return Err(invalid());
            }
            let shape = &find_structural_type(types, result.structural_type)
                .ok_or(invalid())?
                .shape;
            let selected_case = match operation {
                AbstractOperation::EstablishCase { result_case, .. } => Some(*result_case),
                _ => None,
            };
            let declarations = shape
                .direct_fields()
                .into_iter()
                .flatten()
                .map(|field| (None, field))
                .chain(
                    shape
                        .sum_cases()
                        .into_iter()
                        .flatten()
                        .filter(|case| Some(case.id) == selected_case)
                        .flat_map(|case| {
                            case.fields.iter().map(move |field| (Some(case.id), field))
                        }),
                )
                .filter(|(_, field)| !field.relevance.is_erased());
            if fields.len() != declarations.clone().count() {
                return Err(invalid());
            }
            let mut operands = Vec::new();
            for (field, (case, declaration)) in fields.iter().zip(declarations) {
                if field.field != declaration.id {
                    return Err(invalid());
                }
                if let (
                    terminal_psi::RecordFieldValue::Structural(argument),
                    StructuralFieldType::Structural(nested),
                ) = (&field.value, &declaration.field_type)
                {
                    let mut prefix = Vec::new();
                    if let Some(case) = case {
                        prefix.push(StructuralPathSegment::Case(case));
                    }
                    prefix.push(StructuralPathSegment::Field(declaration.identity));
                    operands.push((argument, *nested, prefix));
                }
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                OwnedOperandUse::CopyUnrestricted,
                operands,
            )?;
        }
        AbstractOperation::EstablishStructuralArray {
            result, elements, ..
        } => {
            if custody.leaves.keys().any(|(carrier, path)| {
                *carrier == result.place && owned_relative_path(path, &[]).is_some()
            }) {
                return Err(invalid());
            }
            let Some(StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                ..
            }) = find_structural_type(types, result.structural_type)
            else {
                return Err(invalid());
            };
            if u64::try_from(elements.len()).ok() != Some(*length) {
                return Err(invalid());
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                OwnedOperandUse::CopyUnrestricted,
                elements.iter().enumerate().map(|(index, argument)| {
                    (
                        argument,
                        *element,
                        vec![StructuralPathSegment::FixedIndex(index as u64)],
                    )
                }),
            )?;
        }
        AbstractOperation::MoveStructuralField {
            result,
            source,
            path,
            field,
            ..
        } => {
            let extent = crate::legalization::scalar_graph_input::borrowed_windows::moved(
                function, source, path, *field, result, plan, custody,
            )?;
            if !contains_reference(types, result.structural_type) {
                return Ok(());
            }
            let (place, path) = window_storage(
                function,
                custody,
                types,
                source.place(),
                &extent.path,
                StructuralAccess::MutableBorrow,
            )?;
            let argument = StructuralArgument {
                place,
                path,
                access: StructuralAccess::Owned,
            };
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                OwnedOperandUse::Transfer,
                [(&argument, result.structural_type, Vec::new())],
            )?;
        }
        AbstractOperation::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
            ..
        } => {
            let extent = crate::legalization::scalar_graph_input::borrowed_windows::extent(
                function,
                destination,
                path,
                *field,
                plan,
                custody,
                disposition.required_access(),
            )?;
            if !contains_reference(types, extent.field_type) {
                return Ok(());
            }
            let (place, path) = window_storage(
                function,
                custody,
                types,
                destination.place(),
                &extent.path,
                disposition.required_access(),
            )?;
            if *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable {
                discard_subtree(
                    function,
                    custody,
                    types,
                    place,
                    &path,
                    retirements::DisposalEvent::Operation(
                        operation.psi_operation().ok_or_else(|| invalid())?,
                    ),
                )?;
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                OwnedOperandUse::CopyUnrestricted,
                [(value, extent.field_type, path)],
            )?;
        }
        AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            ..
        } => {
            let (callee_function, result_contract) = match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    let callee = unit
                        .functions
                        .iter()
                        .find(|function| function.machine == *machine)
                        .ok_or_else(|| invalid())?;
                    let result = callee.result.structural().ok_or_else(|| invalid())?;
                    (
                        Some(callee),
                        terminal_semantics::ReferenceCallContract::Machine {
                            parameters: &callee.structural_parameters,
                            sources: &result.reference_sources,
                        },
                    )
                }
                abstract_operations::AbstractCallTarget::Callable { contract, .. } => {
                    let contract = plan
                        .callables
                        .contract(*contract)
                        .ok_or_else(|| invalid())?;
                    if !contract.result.matches_operation_result(
                        &terminal_psi::OperationResult::Structural(result.clone()),
                    ) {
                        return Err(invalid());
                    }
                    (
                        None,
                        terminal_semantics::ReferenceCallContract::Callable(contract),
                    )
                }
                abstract_operations::AbstractCallTarget::Dynamic { .. } => {
                    let abstract_operations::AbstractCallSignature::Requirement(requirement) =
                        crate::legalization::scalar_graph_input::call_signature::signature(
                            *callee, plan,
                        )?
                    else {
                        return Err(invalid());
                    };
                    if !requirement.matches_operation_result(
                        &terminal_psi::OperationResult::Structural(result.clone()),
                    ) {
                        return Err(invalid());
                    }
                    (
                        None,
                        terminal_semantics::ReferenceCallContract::Requirement(requirement),
                    )
                }
            };
            call_transition(
                function,
                callee_function,
                Some(result_contract),
                types,
                Some(result),
                *psi_operation,
                structural_arguments,
                custody,
            )?;
        }
        AbstractOperation::CallStructural {
            psi_operation,
            callee,
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } => {
            let callee_function = callee.machine().and_then(|machine| {
                unit.functions
                    .iter()
                    .find(|function| function.machine == machine)
            });
            call_transition(
                function,
                callee_function,
                None,
                types,
                None,
                *psi_operation,
                structural_arguments,
                custody,
            )?;
        }
        AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            structural_arguments,
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            if !moved.is_empty() {
                return Err(invalid());
            }
            let declaration = unit
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)
                .ok_or_else(|| invalid())?;
            if !declaration.reference_outputs.is_empty()
                && !matches!(crate::legalization::scalar_graph_input::normalized_foreign::row(native, function.machine, *psi_operation)?,
                    Some(target_operations::TargetUnitOperation::NormalizedForeignCall { boundary: selected, .. }) if selected == boundary)
            {
                return Err(invalid());
            }
            let stored = stored_calls::stage(
                function,
                Some(stored_calls::Callee::Boundary(declaration)),
                types,
                custody,
                structural_arguments,
                *psi_operation,
                &moved,
            )?;
            stored_calls::commit(function, types, None, custody, stored)?;
        }
        _ => {}
    }
    custody.prune_retired();
    custody.validate_occurrences().inspect_err(|_| {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE custody-operation-context machine={:?} position={:?} operation={:?} sources={:?}",
                function.machine, custody.reference_position, operation, custody.borrowed_sources
            );
        }
    })?;
    Ok(())
}

fn validate_return_custody(
    custody: &Custody,
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: PlaceId,
    result: &terminal_psi::StructuralResultDeclaration,
) -> Result<(), LegalizationError> {
    if !contains_reference(types, result.structural_type) {
        return Err(invalid());
    }
    let contract = source_contract(function, custody, source).ok_or_else(|| invalid())?;
    if contract != result.structural_type {
        return Err(invalid());
    }
    for ((carrier, path), leaf) in &custody.leaves {
        if *carrier != source || owned_relative_path(path, &[]).is_none() {
            continue;
        }
        let mapping = result
            .reference_sources
            .iter()
            .find(|mapping| mapping.path == *path)
            .ok_or_else(|| invalid())?;
        let access = leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
            .and_then(|identity| find_structural_type(types, identity))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(|| invalid())?;
        let declared = |root: &Root| {
            root.origins().iter().all(|origin| {
                if let target_operations::TargetReferenceOriginBase::Invocation { carrier_type, .. } = origin.base {
                    return mapping.sources.contains(&terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type, path: origin.path.clone(), extent: origin.extent, access,
                    });
                }
                let (root, source_path) = match &origin.base {
                    target_operations::TargetReferenceOriginBase::Storage { root } =>
                        (*root, std::borrow::Cow::Borrowed(origin.path.as_slice())),
                    target_operations::TargetReferenceOriginBase::IncomingContents { root, .. } =>
                        (*root, origin.type_projection()),
                    _ => return false,
                };
                let Some(parameter) = function.structural_parameter(root) else {
                    return false;
                };
                let mut path = source_path.iter().map(|projection| match projection {
                    ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex { .. }) =>
                        ReferenceOriginProjection::Element,
                    _ => projection.clone(),
                }).collect::<Vec<_>>();
                let Some(endpoint) = terminal_semantics::captured_reference_origin_type(
                    types.iter(), parameter.structural_type, &path,
                ) else { return false; };
                let mut extent = origin.extent;
                if extent == semantic_vocabulary::ReferenceOriginExtent::Projected && endpoint != leaf.referent {
                    if find_structural_type(types, endpoint).is_some_and(|declaration|
                        matches!(declaration.shape, StructuralTypeShape::Reference { referent, access: supplied }
                            if referent == leaf.referent && access_covers(supplied, access)))
                    {
                        path.push(ReferenceOriginProjection::Exact(StructuralPathSegment::Referent));
                    } else if matches!(&leaf.placed,
                        Some(target_operations::TargetAccessInterpretation::Captured(correspondence))
                            if correspondence.carrier() == leaf.referent)
                    {
                        extent = semantic_vocabulary::ReferenceOriginExtent::WithinRegion;
                    } else {
                        return false;
                    }
                }
                let actual = terminal_psi::ReferenceOriginSource {
                    place: root, path, access,
                    extent,
                };
                let supplying = mapping.sources.iter()
                    .filter_map(terminal_psi::StructuralReferenceOrigin::parameter)
                    .filter(|allowed| access_covers(allowed.access, actual.access))
                    .map(|allowed| terminal_psi::ReferenceOriginSource {
                        access: actual.access,
                        ..allowed.clone()
                    })
                    .collect::<Vec<_>>();
                terminal_semantics::reference_origin_is_covered(
                    types.iter(), parameter.structural_type, &actual, &supplying,
                )
            })
        };
        let admitted = declared(&leaf.root);
        if !admitted && !descriptor_backing::matches_return(&leaf.backing, &declared) {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE returned-reference-contract machine={:?} source={:?} path={:?} referent={:?} roots={:?} backing={:?} allowed={:?}",
                    function.machine,
                    source,
                    path,
                    leaf.referent,
                    leaf.root,
                    leaf.backing,
                    mapping.sources
                );
            }
            return Err(invalid());
        }
        if custody
            .leaves
            .values()
            .any(|child| custody.descends_from(child, &leaf.identity))
        {
            return Err(invalid());
        }
        if !leaf
            .contract
            .matches_declaration(function, leaf.identity.place)
            || leaf.formations.is_empty()
            || leaf.formations.iter().any(|formation| {
                !formation
                    .contract
                    .matches_declaration(function, formation.identity.place)
            })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn complete_reference_outputs(
    custody: &mut Custody,
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: Option<PlaceId>,
    cleanup_actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> Result<(), LegalizationError> {
    let mut occurrences = Vec::<terminal_semantics::ReferenceCompletionOccurrence<'_, _>>::new();
    for ((carrier, path), leaf) in &custody.leaves {
        let returned = source == Some(*carrier) && owned_relative_path(path, &[]).is_some();
        let in_caller_storage = leaf.in_caller_storage(function, types, *carrier, path)?;
        let disposed =
            terminal_semantics::cleanup_disposes_reference(cleanup_actions, *carrier, path);
        if let Some(alternative) = occurrences
            .iter_mut()
            .find(|occurrence| *occurrence.identity == leaf.identity)
        {
            alternative.returned |= returned;
            alternative.in_caller_storage &= in_caller_storage;
            alternative.disposed &= disposed;
            continue;
        }
        occurrences.push(terminal_semantics::ReferenceCompletionOccurrence {
            identity: &leaf.identity,
            parents: leaf.obligations().collect(),
            returned,
            in_caller_storage,
            disposed,
        });
    }
    occurrences.extend(custody.retired.values().map(|retired| {
        terminal_semantics::ReferenceCompletionOccurrence {
            identity: &retired.leaf.identity,
            parents: retired.leaf.obligations().collect(),
            returned: false,
            in_caller_storage: false,
            disposed: !retired.disposals.is_empty(),
        }
    }));
    terminal_semantics::validate_reference_result_completion(&occurrences)
        .map_err(|reason| {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE returned-reference-completion machine={:?} source={:?} reason={:?} occurrences={:?} cleanup={:?}",
                    function.machine,
                    source,
                    reason,
                    occurrences
                        .iter()
                        .map(|occurrence| (
                            occurrence.identity,
                            &occurrence.parents,
                            occurrence.returned,
                            occurrence.in_caller_storage,
                            occurrence.disposed,
                        ))
                        .collect::<Vec<_>>(),
                    cleanup_actions,
                );
            }
            LegalizationError::ReferenceCompletion { reason }
        })?;
    custody.leaves.retain(|(carrier, path), _| {
        source != Some(*carrier) || owned_relative_path(path, &[]).is_none()
    });
    custody.prune_retired();
    Ok(())
}

pub(crate) fn apply_terminator(
    custody: &mut Custody,
    operation: &AbstractOperation,
    function: &PsiOptimizationFunction,
    plan: &crate::native_geometry::NativePlan<'_>,
) -> Result<(), LegalizationError> {
    if custody.position_is_impossible() {
        return Ok(());
    }
    let types = plan.geometry();
    match operation {
        AbstractOperation::ReturnStructural {
            source,
            cleanup_actions,
            ..
        } => {
            let result = function.result.structural().ok_or_else(|| invalid())?;
            if contains_reference(types, result.structural_type) {
                validate_return_custody(custody, function, types, *source, result)?;
            }
            complete_reference_outputs(custody, function, types, Some(*source), cleanup_actions)?;
        }
        AbstractOperation::Return {
            cleanup_actions, ..
        }
        | AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } => {
            complete_reference_outputs(custody, function, types, None, cleanup_actions)?;
        }
        _ => {}
    }
    let mut exits = Vec::new();
    for (edge, actions) in operation.cleanup_rosters() {
        if custody.edge_is_impossible(edge) {
            continue;
        }
        let cleanup_failure = |error| {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE custody-cleanup-context machine={:?} edge={edge:?} actions={actions:?}",
                    function.machine,
                );
            }
            error
        };
        let owned_transfer = operation.owned_transfer(edge);
        let target = operation.successor_target(edge);
        if let Some(target) = target {
            let mut exit = custody.clone();
            if let Some((target, bindings)) = owned_transfer {
                let destination = function.block(target).ok_or_else(|| invalid())?;
                for binding in bindings {
                    if binding
                        .argument
                        .path
                        .contains(&StructuralPathSegment::Referent)
                    {
                        call_paths::call_reference_custody(
                            function,
                            &exit,
                            types,
                            &binding.argument,
                        )?;
                    }
                }
                re_home_bound_leaves(
                    &mut exit,
                    bindings,
                    &destination.structural_parameters,
                    types,
                )?;
                borrowed_arrivals::bind(
                    function,
                    custody,
                    &mut exit,
                    bindings,
                    &destination.structural_parameters,
                    types,
                )?;
            }
            apply_cleanup_actions(
                &mut exit,
                actions.iter().cloned(),
                function,
                types,
                retirements::DisposalEvent::Edge(edge),
            )
            .map_err(cleanup_failure)?;
            borrowed_arrivals::complete_cleanup(
                function,
                &mut exit,
                types,
                retirements::DisposalEvent::Edge(edge),
            )?;
            borrowed_arrivals::expire_coordinates(&mut exit, target);
            exit.prune_retired();
            exit.validate_occurrences()?;
            exits.push((
                edge,
                target,
                exit.leaves,
                exit.retired,
                exit.borrowed_sources,
            ));
        } else {
            apply_cleanup_actions(
                custody,
                actions.iter().cloned(),
                function,
                types,
                retirements::DisposalEvent::Edge(edge),
            )
            .map_err(cleanup_failure)?;
            custody.borrowed_sources = std::sync::Arc::default();
            custody.prune_retired();
            custody.validate_occurrences()?;
        }
    }
    if !exits.is_empty() {
        custody.edge_exits = exits;
    }
    Ok(())
}

fn apply_cleanup_actions(
    custody: &mut Custody,
    actions: impl IntoIterator<Item = terminal_psi::TerminalAffineCleanupAction>,
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    event: retirements::DisposalEvent,
) -> Result<(), LegalizationError> {
    use terminal_psi::TerminalAffineCleanupAction;
    for action in actions {
        let place = match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                if source_contract(function, custody, place).is_none() {
                    return Err(invalid());
                }
                custody.leaves.retain(|(carrier, path), _| {
                    *carrier != place || owned_relative_path(path, &[]).is_none()
                });
                continue;
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => place,
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                discard_residual(function, custody, types, &discard, event)?;
                continue;
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                if source_contract(function, custody, cleanup.place)
                    .and_then(|root| leaf_type(types, root, &cleanup.path))
                    != Some(cleanup.structural_type)
                {
                    return Err(invalid());
                }
                if storage_access_conflicts(
                    function,
                    custody,
                    types,
                    cleanup.place,
                    &cleanup.path,
                    StructuralAccess::Owned,
                ) {
                    return Err(LegalizationError::ReferenceDisposalConflict {
                        machine: function.machine,
                        place: cleanup.place,
                    });
                }
                discard_subtree(
                    function,
                    custody,
                    types,
                    cleanup.place,
                    &cleanup.path,
                    event,
                )?;
                continue;
            }
        };
        if storage_access_conflicts(
            function,
            custody,
            types,
            place,
            &[],
            StructuralAccess::Owned,
        ) {
            return Err(LegalizationError::ReferenceDisposalConflict {
                machine: function.machine,
                place,
            });
        }
        discard_owned(function, custody, types, place, event)?;
    }
    Ok(())
}

fn borrowed_sources(
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
) -> std::sync::Arc<borrowed_arrivals::Sources> {
    let sources = function
        .block_structural_parameters()
        .filter(|parameter| {
            parameter.access != StructuralAccess::Owned
                && !terminal_semantics::descriptor_results::is_descriptor(
                    types.iter(),
                    parameter.structural_type,
                )
        })
        .map(|parameter| (parameter.place, None))
        .collect();
    std::sync::Arc::new(sources)
}

pub(crate) fn block_entry_states(
    function: &PsiOptimizationFunction,
    descriptor_inputs: &crate::legalization_descriptor_inputs::DescriptorInputs<'_>,
) -> Result<BTreeMap<BlockId, Custody>, LegalizationError> {
    let native = descriptor_inputs.native();
    let plan = descriptor_inputs.plan();
    let unit = descriptor_inputs.unit();
    let types = plan.geometry();
    let blocks = &function.blocks;
    let positions: BTreeMap<BlockId, usize> = blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect();
    let entry_position = *positions.get(&function.entry).ok_or_else(|| invalid())?;
    if positions.len() != blocks.len() {
        return Err(invalid());
    }
    let descriptor_backing =
        std::sync::Arc::new(descriptor_backing::prepare(function, descriptor_inputs)?);
    let mut predecessors: BTreeMap<_, BTreeSet<_>> = blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect();
    let mut storage_definitions = BTreeMap::new();
    for block in blocks {
        for parameter in &block.structural_parameters {
            storage_definitions.insert(parameter.place, (block.id, true));
        }
        for node in &block.nodes {
            if let Some((_, result)) = node.operation.structural_result() {
                storage_definitions.insert(result.place, (block.id, false));
            }
            for edge in &node.successors {
                if descriptor_backing.edges.get(&edge.psi_edge)
                    == Some(
                        &terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible,
                    )
                {
                    continue;
                }
                predecessors
                    .get_mut(&edge.target)
                    .ok_or(invalid())?
                    .insert(block.id);
            }
        }
    }
    let dominators = optimization_unit::dominators(
        function.entry,
        blocks.iter().map(|block| block.id),
        &predecessors,
    );
    let borrowed_sources = borrowed_sources(function, types);
    let mut initial = entry(function, types)?;
    initial.borrowed_sources = borrowed_sources;
    initial.placement_catalog = plan.structural_types.clone();
    for input in plan
        .structural_types
        .access_view_inputs()
        .iter()
        .filter(|input| input.machine == function.machine)
    {
        let parameter = function
            .structural_parameters
            .get(input.runtime_position as usize)
            .ok_or_else(|| invalid())?;
        let (matched, _) = plan
            .structural_types
            .access_parameter_demand(function.machine, input.runtime_position as usize, parameter)
            .ok_or_else(|| invalid())?;
        if matched != input
            || initial
                .access_formals
                .insert(parameter.place, input.clone())
                .is_some()
        {
            return Err(invalid());
        }
    }
    initial.descriptor_backing = descriptor_backing;
    initial.stored_calls = std::sync::Arc::new(stored_calls::prepare(descriptor_inputs));
    let impossible_context = Custody {
        descriptor_backing: initial.descriptor_backing.clone(),
        stored_calls: initial.stored_calls.clone(),
        placement_catalog: initial.placement_catalog.clone(),
        ..Custody::default()
    };
    let mut entries = BTreeMap::from([(function.entry, initial)]);
    let mut pending = VecDeque::from([entry_position]);
    let mut queued = vec![false; blocks.len()];
    queued[entry_position] = true;
    // Keep source blocks in their original order and inspect every matching edge.
    // Unknown targets still fail at the existing edge lookup below.
    let mut incoming_sources = vec![Vec::new(); blocks.len()];
    for (source_position, block) in blocks.iter().enumerate() {
        let Some(terminator) = block.nodes.last() else {
            continue;
        };
        for edge in &terminator.successors {
            if let Some(&target_position) = positions.get(&edge.target) {
                let incoming = &mut incoming_sources[target_position];
                if incoming.last() != Some(&source_position) {
                    incoming.push(source_position);
                }
            }
        }
    }
    let mut final_arrivals = vec![Vec::new(); blocks.len()];
    let mut deferred = BTreeSet::new();
    let mut failures = BTreeMap::new();
    'worklist: while let Some(position) = pending.pop_front() {
        queued[position] = false;
        let block = &blocks[position];
        let mut custody = entries.get(&block.id).cloned().ok_or_else(|| invalid())?;
        for (node_index, node) in blocks[position].nodes.iter().enumerate() {
            custody.set_position(block.id, node_index)?;
            let result = if matches!(
                node.operation,
                abstract_operations::return_or_return_unit_pattern!(..)
                    | AbstractOperation::ReturnStructural { .. }
                    | AbstractOperation::Jump { .. }
                    | AbstractOperation::Conditional { .. }
                    | AbstractOperation::StructuralCase { .. }
                    | AbstractOperation::ExternalCompletion { .. }
            ) {
                apply_terminator(&mut custody, &node.operation, function, plan)
            } else {
                apply(&mut custody, node, function, native, plan, unit)
            };
            if let Err(error) = result {
                failures.insert(
                    block.id,
                    error.attributed(function.machine, node.operation.psi_operation()),
                );
                continue 'worklist;
            }
        }
        failures.remove(&block.id);
        let terminator = block.nodes.last().ok_or_else(|| invalid())?;
        let mut arrivals = Vec::with_capacity(terminator.successors.len());
        for edge in &terminator.successors {
            let target_position = *positions.get(&edge.target).ok_or_else(|| invalid())?;
            if target_position == entry_position {
                return Err(invalid());
            }
            if custody.edge_is_impossible(edge.psi_edge) {
                continue;
            }
            let mut arrival = custody.edge_custody(edge.psi_edge, edge.target)?;
            if matches!(
                terminator.operation,
                AbstractOperation::StructuralCase { .. }
            ) {
                re_home_bound_leaves(
                    &mut arrival,
                    &edge.structural_bindings,
                    &blocks[target_position].structural_parameters,
                    types,
                )?;
            }
            let scope = dominators.get(&edge.target).ok_or(invalid())?;
            for (place, source) in std::sync::Arc::make_mut(&mut arrival.borrowed_sources) {
                if storage_definitions
                    .get(place)
                    .is_some_and(|(definition, _)| !scope.contains(definition))
                {
                    *source = None;
                }
            }
            arrival.leaves.retain(|(carrier, path), _| {
                owned_relative_path(path, &[]).is_none()
                    || storage_definitions
                        .get(carrier)
                        .is_none_or(|(definition, parameter)| {
                            scope.contains(definition) && (*parameter || *definition != edge.target)
                        })
            });
            arrival.prune_retired();
            arrival.validate_occurrences().inspect_err(|_| {
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    eprintln!("CHECKSITE custody-arrival-context machine={:?} source_block={:?} edge={:?} target={:?} terminator={:?} scope={:?} sources={:?}", function.machine, block.id, edge.psi_edge, edge.target, terminator.operation, scope, arrival.borrowed_sources);
                }
            })?;
            arrivals.push((edge.psi_edge, edge.target, arrival));
        }
        final_arrivals[position] = arrivals;
        let targets: BTreeSet<_> = final_arrivals[position]
            .iter()
            .map(|(_, target, _)| *target)
            .collect();
        for target in targets {
            let target_position = *positions.get(&target).ok_or_else(|| invalid())?;
            let incoming = incoming_sources[target_position]
                .iter()
                .flat_map(|source| &final_arrivals[*source])
                .filter(|(_, candidate, _)| *candidate == target)
                .map(|(_, _, arrival)| arrival.clone());
            let Some(complete) = resident_joins::merge(function, target, types, incoming)? else {
                deferred.insert(target);
                pending.retain(|position| *position != target_position);
                queued[target_position] = false;
                continue;
            };
            let was_deferred = deferred.remove(&target);
            let changed = entries.get(&target).is_none_or(|entry| {
                entry.leaves != complete.leaves
                    || entry.retired != complete.retired
                    || entry.borrowed_sources != complete.borrowed_sources
            });
            if changed {
                entries.insert(target, complete);
            }
            if (changed || was_deferred) && !queued[target_position] {
                pending.push_back(target_position);
                queued[target_position] = true;
            }
        }
    }
    if let Some((_, error)) = failures.into_iter().next() {
        return Err(error);
    }
    for block in blocks {
        if impossible_context
            .descriptor_backing
            .flow
            .get(&(block.id, 0))
            == Some(&terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible)
        {
            if entries.contains_key(&block.id) {
                return Err(invalid());
            }
            entries.insert(block.id, impossible_context.clone());
        }
    }
    if entries.len() != blocks.len() || !deferred.is_empty() {
        return Err(invalid());
    }
    for (target_position, block) in blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.id != function.entry)
    {
        if impossible_context
            .descriptor_backing
            .flow
            .get(&(block.id, 0))
            == Some(&terminal_semantics::descriptor_results::FlowFeasibility::ProvedImpossible)
        {
            continue;
        }
        let arrivals = incoming_sources[target_position]
            .iter()
            .flat_map(|source| &final_arrivals[*source])
            .filter(|(_, target, _)| *target == block.id)
            .map(|(_, _, arrival)| arrival.clone());
        let complete =
            resident_joins::merge(function, block.id, types, arrivals)?.ok_or_else(|| invalid())?;
        if entries.get(&block.id).is_none_or(|entry| {
            entry.leaves != complete.leaves
                || entry.retired != complete.retired
                || entry.borrowed_sources != complete.borrowed_sources
        }) {
            return Err(invalid());
        }
    }
    Ok(entries)
}

fn entry(
    function: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
) -> Result<Custody, LegalizationError> {
    let mut custody = Custody::default();
    for parameter in &function.structural_parameters {
        if !contains_reference(types, parameter.structural_type) {
            continue;
        }
        let paths = leaf_paths(
            types,
            parameter.structural_type,
            4096usize.saturating_sub(custody.leaves.len()),
        )
        .ok_or_else(|| invalid())?;
        for path in paths {
            let identity = (parameter.place, path.clone());
            let root = Root::incoming_contents(parameter.place, path.clone());
            custody.leaves.insert(
                identity.clone(),
                Leaf {
                    identity: OccurrenceIdentity::parameter(&identity),
                    root: root.clone(),
                    backing: vec![descriptor_backing::Backing::Source(root)],
                    parents: Vec::new(),
                    lifetime_debts: Vec::new(),
                    contract: CarrierContract::Parameter(parameter.clone()),
                    formations: vec![Formation {
                        identity: OccurrenceIdentity::parameter(&identity),
                        contract: CarrierContract::Parameter(parameter.clone()),
                    }],
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| invalid())?,
                    placed: None,
                },
            );
        }
    }
    Ok(custody)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn referent_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &calling_conventions::ValuePlacement,
    caller: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    call_operation: semantic_vocabulary::InvocationSite,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return Err(invalid());
    }
    call_paths::call_reference_custody(caller, custody, types, argument)?;
    let root_type = source_contract(caller, custody, argument.place).ok_or(invalid())?;
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        root_type,
        &argument.path,
        argument.access,
        Some(declaration.structural_type),
        types,
    )
    .ok_or(invalid())?;
    let selected = projection.selected;
    let source_byte_offset = projection.referent_offset;
    if selected != declaration.structural_type
        || Some(destination.shape) != crate::structural_inputs::parameter_shape(declaration, types)
    {
        return Err(invalid());
    }
    let target_caller = native.function(caller.machine).ok_or(invalid())?;
    let source = if let Some(parameter) = caller.structural_parameter(argument.place) {
        let retained =
            crate::legalization::scalar_graph_input::structural_parameters(target_caller)
                .and_then(|parameters| {
                    parameters
                        .iter()
                        .find(|parameter| parameter.place == argument.place)
                })
                .ok_or(invalid())?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow
            || retained.structural_type != root_type
        {
            return Err(invalid());
        }
        retained.placement.clone().into()
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameter(argument.place)
            .map(|parameter| (block.id, parameter))
    }) {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity == StructuralMultiplicity::Linear
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter {
            block,
            place: argument.place,
        }
    } else {
        let (producer, result) = source_result(caller, argument.place)?;
        if result.structural_type != root_type
            || result.multiplicity == StructuralMultiplicity::Linear
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        }
    };
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices:
            crate::legalization::scalar_graph_input::target::control_flow::sources::call_indices(
                native,
                caller,
                target_operations::CallSiteOwner::from(call_operation),
                &projection.elements,
            )?,
        source,
        destination: destination.clone(),
    })
}

pub(crate) fn reference_results(
    caller: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    contract: terminal_semantics::ReferenceCallContract<'_>,
    arguments: &[StructuralArgument],
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    let mut returned = custody.clone();
    call_transition(
        caller,
        callee,
        Some(contract),
        types,
        Some(result),
        psi_operation,
        arguments,
        &mut returned,
    )
}

pub(crate) fn access_call_custody(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    types: &crate::native_geometry::NativeGeometry<'_>,
    arguments: &[StructuralArgument],
    custody: &Custody,
) -> Result<Vec<target_operations::TargetAccessCallBinding>, LegalizationError> {
    let catalog = &custody.placement_catalog;
    let mut bindings = Vec::new();
    for (runtime_position, parameter) in callee.structural_parameters.iter().enumerate() {
        if !catalog.access_view_inputs().iter().any(|input| {
            input.machine == callee.machine
                && usize::try_from(input.runtime_position).ok() == Some(runtime_position)
        }) {
            continue;
        }
        let (input, plan) = catalog
            .access_parameter_demand(callee.machine, runtime_position, parameter)
            .ok_or_else(|| invalid())?;
        let argument = arguments
            .get(runtime_position)
            .filter(|argument| argument.access == parameter.access)
            .ok_or_else(|| invalid())?;
        let association = match access_source(caller, custody, types, argument) {
            Some(target_operations::TargetAccessInterpretation::Captured(captured)) => {
                if reference_source_type(caller, custody, types, argument) != Some(plan.carrier) {
                    return Err(invalid());
                }
                if argument.path.contains(&StructuralPathSegment::Referent) {
                    let observed =
                        call_paths::call_reference_custody(caller, custody, types, argument)?;
                    if observed.referent != plan.carrier {
                        return Err(invalid());
                    }
                }
                validate_backing(
                    types,
                    catalog.access_views(),
                    plan,
                    captured.source_type(),
                    &captured,
                )
                .map_err(|_| invalid())?;
                target_operations::TargetAccessCallAssociation::Captured {
                    schema: captured.schema(),
                    carrier: captured.carrier(),
                }
            }
            Some(target_operations::TargetAccessInterpretation::Formal(formal)) => {
                if !parameter.access.admits(formal.access)
                    || !custody
                        .access_formals
                        .values()
                        .any(|input| input == &formal)
                {
                    return Err(invalid());
                }
                if argument.path.contains(&StructuralPathSegment::Referent) {
                    let observed =
                        call_paths::call_reference_custody(caller, custody, types, argument)?;
                    if observed.referent != plan.carrier {
                        return Err(invalid());
                    }
                }
                let source_parameter = caller
                    .structural_parameters
                    .get(formal.runtime_position as usize)
                    .ok_or_else(|| invalid())?;
                let (_, source_plan) = catalog
                    .access_parameter_demand(
                        caller.machine,
                        formal.runtime_position as usize,
                        source_parameter,
                    )
                    .ok_or_else(|| invalid())?;
                if source_plan != plan {
                    return Err(invalid());
                }
                target_operations::TargetAccessCallAssociation::CallerFormal(formal)
            }
            None => return Err(invalid()),
        };
        bindings.push(target_operations::TargetAccessCallBinding {
            input: input.clone(),
            argument_index: input.runtime_position,
            association,
        });
    }
    Ok(bindings)
}

fn call_transition(
    caller: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    result_contract: Option<terminal_semantics::ReferenceCallContract<'_>>,
    types: &crate::native_geometry::NativeGeometry<'_>,
    result: Option<&StructuralOperationResult>,
    operation: OperationId,
    arguments: &[StructuralArgument],
    custody: &mut Custody,
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    if let Some(callee) = callee {
        access_call_custody(caller, callee, types, arguments, custody)?;
    }
    let moved = call_arguments(caller, types, custody, arguments)?;
    let stored = stored_calls::stage(
        caller,
        callee.map(stored_calls::Callee::Body),
        types,
        custody,
        arguments,
        operation,
        &moved,
    )?;
    let results = match (result_contract, result) {
        (Some(contract), Some(result)) => call_results(
            caller, callee, contract, types, result, operation, arguments, custody, &moved,
        )?,
        (None, None) => Vec::new(),
        _ => return Err(invalid()),
    };
    for coordinate in moved.keys() {
        custody.leaves.remove(coordinate).ok_or_else(|| invalid())?;
    }
    stored_calls::commit(
        caller,
        types,
        result.map(|result| result.place),
        custody,
        stored,
    )?;
    retire_consumed_arguments(custody, &moved, operation)?;
    Ok(results)
}

pub(crate) fn accessor_scalar_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &crate::native_geometry::NativeGeometry<'_>,
    source: &StructuralArgument,
    accessor: StructuralTypeId,
) -> Result<(), LegalizationError> {
    if reference_source_type(function, custody, types, source) != Some(accessor) {
        return Err(invalid());
    }
    normalized_source(function, custody, types, source)?;
    Ok(())
}

#[cfg(test)]
mod call_origin_tests {
    use super::*;
    use semantic_vocabulary::{ObligationId, ValueId};

    fn selected(index: u64, obligation: u64) -> Root {
        Root::Ingress(
            PlaceId::new(1).unwrap(),
            vec![StructuralPathSegment::RuntimeIndex {
                index: ValueId::new(index).unwrap(),
                obligation: ObligationId::new(obligation).unwrap(),
            }],
        )
    }

    #[test]
    fn independent_bounds_obligations_do_not_make_one_selector_disjoint() {
        assert!(roots_definitely_overlap(&selected(2, 3), &selected(2, 4)));
    }

    #[test]
    fn distinct_captured_selectors_need_source_compatibility_not_duplicate_rejection() {
        let first = selected(2, 3);
        let second = selected(4, 5);
        assert!(roots_overlap(&first, &second));
        assert!(!roots_definitely_overlap(&first, &second));
    }

    #[test]
    fn exact_ancestor_and_static_projection_remain_demonstrated_overlap() {
        let child = Root::Ingress(
            PlaceId::new(1).unwrap(),
            vec![StructuralPathSegment::FixedIndex(2)],
        );
        assert!(roots_definitely_overlap(
            &Root::Place(PlaceId::new(1).unwrap()),
            &child
        ));
        assert!(roots_definitely_overlap(&child, &child));
        assert!(!roots_definitely_overlap(
            &child,
            &Root::Ingress(
                PlaceId::new(1).unwrap(),
                vec![StructuralPathSegment::FixedIndex(3)]
            )
        ));
    }

    #[test]
    fn empty_and_touching_windows_do_not_demonstrate_overlap() {
        let window = |start, end| {
            Root::Ingress(
                PlaceId::new(1).unwrap(),
                vec![StructuralPathSegment::FixedRange { start, end }],
            )
        };
        assert!(!roots_definitely_overlap(
            &window(0, 0),
            &Root::Place(PlaceId::new(1).unwrap())
        ));
        assert!(!roots_definitely_overlap(&window(0, 2), &window(2, 4)));
        assert!(roots_definitely_overlap(&window(0, 3), &window(2, 4)));
        let mut child = vec![StructuralPathSegment::FixedRange { start: 0, end: 3 }];
        child.push(StructuralPathSegment::FixedIndex(0));
        assert!(!roots_definitely_overlap(
            &Root::Ingress(PlaceId::new(1).unwrap(), child),
            &window(0, 3)
        ));
    }

    #[test]
    fn exact_argument_duplication_excludes_empty_storage_windows() {
        let whole =
            StructuralArgument::whole(PlaceId::new(1).unwrap(), StructuralAccess::MutableBorrow);
        assert!(duplicate_argument_storage(&whole, &whole));
        let empty = StructuralArgument {
            path: vec![StructuralPathSegment::FixedRange { start: 2, end: 2 }],
            ..whole
        };
        assert!(!duplicate_argument_storage(&empty, &empty));
    }

    #[test]
    fn forged_operand_reusing_another_exact_placement_is_duplicate() {
        let first =
            StructuralArgument::whole(PlaceId::new(1).unwrap(), StructuralAccess::MutableBorrow);
        let mut second =
            StructuralArgument::whole(PlaceId::new(2).unwrap(), StructuralAccess::MutableBorrow);
        assert!(!duplicate_argument_storage(&first, &second));
        second.place = first.place;
        assert!(duplicate_argument_storage(&first, &second));
    }

    #[test]
    fn element_family_is_not_one_captured_element() {
        let family = Root::Alternatives(vec![target_operations::TargetReferenceOrigin {
            base: target_operations::TargetReferenceOriginBase::Storage {
                root: PlaceId::new(1).unwrap(),
            },
            path: vec![ReferenceOriginProjection::Element],
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        }]);
        assert!(roots_overlap(&family, &family));
        assert!(!roots_definitely_overlap(&family, &family));
        assert!(!roots_definitely_overlap(
            &Root::Place(PlaceId::new(1).unwrap()),
            &family
        ));
        let wildcard = Root::Ingress(
            PlaceId::new(1).unwrap(),
            vec![StructuralPathSegment::AllElements],
        );
        assert!(!roots_definitely_overlap(
            &Root::Place(PlaceId::new(1).unwrap()),
            &wildcard
        ));
    }
}
