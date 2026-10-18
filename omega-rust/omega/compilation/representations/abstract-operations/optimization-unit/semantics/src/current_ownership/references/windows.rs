//! Restoration moves and stores transfer descriptor custody with the selected
//! storage. Resolve references through the same live access walk as calls;
//! relocating a carrier preserves its identity, parent and captured referent.
//! A may-alias region cannot authorize moving every candidate's resources.
use super::*;
use terminal_psi::StructuralPathSegment;

#[allow(clippy::too_many_arguments)]
pub(super) fn transfer(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &mut Vec<LiveReference>,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
    access: StructuralAccess,
    value_type: StructuralTypeId,
    value: PlaceId,
    extracting: bool,
    replace_existing: bool,
) -> Result<(), OptimizationUnitValidationError> {
    let fail = |reason| invalid(function, block, Some(node), reason);
    let hole = window_hole(
        function,
        types,
        root.place(),
        root.structural_type(),
        path,
        field,
        access,
    )
    .ok_or_else(|| fail("restoration window does not resolve to a declared field"))?;
    let crossing = hole
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent);
    let (origins, dependencies, suffix) = if let Some(position) = crossing {
        let custody = access_reference_custody(
            function,
            types,
            block,
            node,
            live,
            &StructuralArgument {
                place: root.place(),
                path: hole.clone(),
                access,
            },
        )?;
        if !terminal_semantics::structural_window_selects_child(path, field)
            && !terminal_semantics::structural_window_origins(
                function.structural_parameters.iter().chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                ),
                custody.origins.iter().map(origin_place),
                access,
                |place| crate::structural_place_supports_window_restoration(function, place),
            )
        {
            return Err(fail(
                "whole-referent restoration requires live owned or exclusive ingress storage",
            ));
        }
        (custody.origins, custody.dependencies, &hole[position + 1..])
    } else {
        (
            vec![ReferenceOrigin::Primitive(root.place())],
            Vec::new(),
            hole.as_slice(),
        )
    };
    let mut regions = Vec::new();
    for origin in origins {
        let candidates = match origin {
            ReferenceOrigin::Invocation { .. } => {
                return Err(fail("external reference has no owned restoration cursor"));
            }
            ReferenceOrigin::Primitive(place) => vec![ReferenceIdentity {
                place,
                path: Vec::new(),
            }],
            ReferenceOrigin::IngressLeaf(identity) => vec![
                identity
                    .exact_identity()
                    .ok_or_else(|| fail("window reference origin has no exact storage cursor"))?,
            ],
            ReferenceOrigin::Alternatives { origins, .. } => origins
                .into_iter()
                .map(|origin| {
                    origin
                        .exact_identity()
                        .ok_or_else(|| fail("window reference origin has no exact storage cursor"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        for mut region in candidates {
            if let Some(position) = crossing {
                let reference = access_path_type(types, root.structural_type(), &hole[..position])
                    .ok_or_else(|| fail("window reference crossing has no type"))?;
                let referent = leaf_referent(types, reference, &[])
                    .ok_or_else(|| fail("window reference crossing has no referent"))?;
                region = super::storage_cursor(function, types, region, referent)
                    .ok_or_else(|| fail("window reference origin has no exact storage cursor"))?;
            }
            region.path.extend_from_slice(suffix);
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
    }
    let expected = leaf_paths(types, value_type, live.len())
        .ok_or_else(|| fail("window reference roster exceeds live custody"))?;
    let is_dependency = |reference: &LiveReference| {
        dependencies.iter().any(|dependency| {
            matches!(dependency, ReferenceOrigin::IngressLeaf(identity)
            if identity.exact_identity().is_some_and(|identity| identity.place == reference.carrier && identity.path == reference.carrier_path))
        })
    };
    let ancestors = live
        .iter()
        .filter(|reference| is_dependency(reference))
        .map(|reference| reference.identity.clone())
        .collect::<Vec<_>>();
    for region in &regions {
        if !access_paths::descriptor_storage_available(
            function,
            types,
            live,
            region.place,
            &region.path,
            &ancestors,
            access,
        ) {
            return Err(fail("restoration window accesses suspended storage"));
        }
    }
    if expected.is_empty() {
        if regions.iter().any(|region| {
            live.iter().any(|reference| {
                reference.carrier == region.place
                    && (!reference.carrier_path.starts_with(&region.path)
                        || super::owned_resident(&reference.carrier_path, &region.path))
                    && terminal_semantics::structural_paths_may_overlap(
                        &reference.carrier_path,
                        &region.path,
                    )
            })
        }) {
            return Err(fail("resource-free window overlaps a live descriptor"));
        }
        return Ok(());
    }
    let [region] = regions.as_slice() else {
        return Err(fail(
            "reference-bearing window requires one exact storage region",
        ));
    };
    let whole_value = ReferenceIdentity {
        place: value,
        path: Vec::new(),
    };
    let (source, destination) = if extracting {
        (region, &whole_value)
    } else {
        (&whole_value, region)
    };
    let copies_storage = !extracting
        && structural_source_contract(function, value).is_some_and(|contract| {
            contract.access == StructuralAccess::Owned
                && contract.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        });
    // Discarding reference descriptors settles only this resident cohort. Stage
    // closure and transfer together so a failed incoming roster changes none
    // of the caller's custody. Captured backing and external copies stay live.
    let mut staged = live.clone();
    if replace_existing {
        discard_subtree(
            function,
            types,
            block,
            &mut staged,
            destination.place,
            &destination.path,
        )?;
    }
    let staged_live = &mut staged;
    if staged_live.iter().any(|reference| {
        reference.carrier == destination.place
            && (!reference.carrier_path.starts_with(&destination.path)
                || super::owned_resident(&reference.carrier_path, &destination.path))
            && terminal_semantics::structural_paths_may_overlap(
                &reference.carrier_path,
                &destination.path,
            )
    }) {
        return Err(fail("window destination still owns reference custody"));
    }
    let mut relocations = Vec::new();
    for leaf in expected {
        if copies_storage
            && leaf_type(types, value_type, &leaf)
                .and_then(|identity| reference_access(types, identity))
                != Some(StructuralAccess::SharedBorrow)
        {
            return Err(fail("window copy requires shared descriptor leaves"));
        }
        let mut source_path = source.path.clone();
        source_path.extend_from_slice(&leaf);
        let mut matching = staged_live.iter().enumerate().filter(|(_, reference)| {
            reference.carrier == source.place && reference.carrier_path == source_path
        });
        let (position, reference) = matching
            .next()
            .ok_or_else(|| fail("window source reference leaf is absent"))?;
        if matching.next().is_some()
            || staged_live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &reference.identity)) {
            return Err(fail("window source reference leaf is ambiguous or suspended"));
        }
        let mut destination_path = destination.path.clone();
        destination_path.extend(leaf);
        relocations.push((position, destination_path));
    }
    if staged_live.iter().enumerate().any(|(position, reference)| {
        reference.carrier == source.place
            && super::owned_resident(&reference.carrier_path, &source.path)
            && !relocations
                .iter()
                .any(|(selected, _)| *selected == position)
    }) {
        return Err(fail("window source reference roster differs from its type"));
    }
    if !access_paths::descriptor_storage_available(
        function,
        types,
        staged_live,
        source.place,
        &source.path,
        &ancestors,
        if copies_storage {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::Owned
        },
    ) {
        return Err(fail("window source storage has an outstanding loan"));
    }
    // Publish only after the complete roster and both regions were checked.
    for (position, path) in relocations {
        if copies_storage {
            let mut copied = staged_live[position].clone();
            copied.identity = ReferenceIdentity {
                place: destination.place,
                path: path.clone(),
            };
            if staged_live
                .iter()
                .any(|reference| reference.identity == copied.identity)
            {
                return Err(fail("window copy destination identity is already live"));
            }
            copied.carrier = destination.place;
            copied.carrier_path = path;
            staged_live.push(copied);
        } else {
            staged_live[position].carrier = destination.place;
            staged_live[position].carrier_path = path;
        }
    }
    *live = staged;
    Ok(())
}
