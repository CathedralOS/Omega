//! A completed body supplies the loan graph, while each call supplies its own
//! live input occurrences. Bind parents before publishing any output so a
//! replaced slot cannot redirect a child to its new resident pointer.
use super::*;
use crate::reference_outputs::{
    ReferenceBodyOutputs, ReferenceOutputOrigin, ReferenceOutputParent, ReferenceOutputResidence,
    ReferenceOutputSupply,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn bind(
    function: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    body: &ReferenceBodyOutputs,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed_sources: &BorrowedReferenceSources,
    block: BlockId,
    node: u32,
    arguments: &[StructuralArgument],
    result: Option<&StructuralOperationResult>,
    live: &[LiveReference],
    moved: &BTreeSet<ReferenceOccurrenceIdentity>,
) -> Result<call_effects::StoredReferences, OptimizationUnitValidationError> {
    let fail = |reason| invalid(function, block, Some(node), reason);
    let actual = |parameter: usize, path: &[StructuralPathSegment]| {
        let formal = callee
            .structural_parameters
            .get(parameter)
            .ok_or_else(|| fail("completed reference output parameter is absent"))?;
        call_source(
            &callee.structural_parameters,
            arguments,
            &StructuralArgument {
                place: formal.place,
                path: path.to_vec(),
                access: formal.access,
            },
        )
        .ok_or_else(|| fail("completed reference output actual is absent"))
    };
    // Capture once, before any destination is replaced. An entry anchor is an
    // actual live pointer word; its type or root spelling cannot supply it.
    let mut captured = vec![None; body.entry().len()];
    for &index in body.required_entries() {
        let entry = &body.entry()[index];
        let parameter = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == entry.carrier)
            .ok_or_else(|| fail("completed reference entry has no formal"))?;
        let argument = actual(parameter, &entry.carrier_path)?;
        let slot = call_effects::resolve_slot(function, types, block, node, live, &argument)?;
        let expected = access_path_type(
            types,
            callee.structural_parameters[parameter].structural_type,
            &entry.carrier_path,
        )
        .ok_or_else(|| fail("completed reference entry has no type"))?;
        if call_effects::slot_type(function, types, &slot) != Some(expected) {
            return Err(fail(
                "completed reference entry changes the actual descriptor type",
            ));
        }
        let mut residents = live.iter().filter(|reference| {
            reference.carrier == slot.place && reference.carrier_path == slot.path
        });
        let resident = residents
            .next()
            .cloned()
            .ok_or_else(|| fail("completed reference entry is not live at this call"))?;
        if residents.next().is_some() {
            return Err(fail("completed reference entry is ambiguous at this call"));
        }
        captured[index] = Some(resident);
    }
    let mut completed = None;
    for exit in body.exits() {
        let mut nodes: Vec<LiveReference> = Vec::with_capacity(exit.graph().len());
        let mut replacements = Vec::new();
        let mut returned = Vec::new();
        let mut established = BTreeSet::new();
        for output in exit.graph() {
            let (destination, publish, is_returned) = match output.residence() {
                ReferenceOutputResidence::Entry(entry) => {
                    let entry = captured
                        .get(*entry)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| fail("completed reference entry index is absent"))?;
                    (
                        ReferenceIdentity {
                            place: entry.carrier,
                            path: entry.carrier_path.clone(),
                        },
                        false,
                        false,
                    )
                }
                ReferenceOutputResidence::Parameter { parameter, path } => {
                    let actual = actual(*parameter, path)?;
                    if !matches!(
                        actual.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        return Err(fail(
                            "completed reference destination lacks exclusive access",
                        ));
                    }
                    (
                        call_effects::resolve_slot(function, types, block, node, live, &actual)?,
                        true,
                        false,
                    )
                }
                ReferenceOutputResidence::Returned { path } => {
                    let result =
                        result.ok_or_else(|| fail("completed reference result is absent"))?;
                    (
                        ReferenceIdentity {
                            place: result.place,
                            path: path.clone(),
                        },
                        true,
                        true,
                    )
                }
            };
            if call_effects::slot_type(function, types, &destination)
                != Some(output.reference_type())
            {
                return Err(fail(
                    "completed reference output changes its receiving descriptor type",
                ));
            }
            if is_returned
                && live.iter().any(|reference| {
                    reference.carrier == destination.place
                        || reference.identity.place == destination.place
                })
            {
                return Err(fail("completed reference result storage is already live"));
            }
            let mut reference = match output.supply() {
                ReferenceOutputSupply::Entry(index) => {
                    let reference = captured
                        .get(*index)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| fail("completed reference transfer anchor is absent"))?;
                    if (reference.carrier != destination.place
                        || reference.carrier_path != destination.path)
                        && !moved.contains(&reference.identity)
                    {
                        return Err(fail(
                            "completed reference transfer did not consume its actual argument",
                        ));
                    }
                    reference.clone()
                }
                ReferenceOutputSupply::Constructed { origins, parent } => {
                    let root = bind_origins(
                        function,
                        types,
                        borrowed_sources,
                        block,
                        node,
                        arguments,
                        callee,
                        live,
                        &captured,
                        origins,
                    )?;
                    let parent = match parent {
                        ReferenceOutputParent::Entry(index) => ReferenceParent::Reference(
                            captured.get(*index).and_then(Option::as_ref).ok_or_else(|| fail("completed reference parent anchor is absent"))?.identity.clone()),
                        ReferenceOutputParent::Node(index) => ReferenceParent::Reference(
                            nodes.get(*index).ok_or_else(|| fail("completed reference parent is not earlier in the output graph"))?.identity.clone()),
                        ReferenceOutputParent::Root(origins) => {
                            // Resolving raw borrowed storage must retain the
                            // actual caller's parent, not create a root loan
                            // underneath an existing caller descriptor.
                            bind_parent(function, types, borrowed_sources, block, node,
                                arguments, callee, live, &captured, origins)?
                        }
                    };
                    let identity = formed_identity(
                        function,
                        block,
                        node,
                        destination.place,
                        destination.path.clone(),
                    )?;
                    established.insert(identity.clone());
                    LiveReference {
                        identity,
                        carrier: destination.place,
                        carrier_path: destination.path.clone(),
                        root,
                        parent,
                    }
                }
            };
            reference.carrier = destination.place;
            reference.carrier_path = destination.path;
            if publish {
                if is_returned {
                    returned.push(reference.clone());
                } else {
                    replacements.push(reference.clone());
                }
            }
            nodes.push(reference);
        }
        replacements.sort_by(|left, right| {
            (left.carrier, &left.carrier_path).cmp(&(right.carrier, &right.carrier_path))
        });
        returned.sort_by(|left, right| {
            (left.carrier, &left.carrier_path).cmp(&(right.carrier, &right.carrier_path))
        });
        let current = (replacements, returned, established);
        if completed
            .as_ref()
            .is_some_and(|previous| previous != &current)
        {
            return Err(fail(
                "completed body exits have different caller permission graphs",
            ));
        }
        completed = Some(current);
    }
    let (replacements, returned, established) =
        completed.ok_or_else(|| fail("reference-output body has no completed normal exit"))?;
    Ok(call_effects::StoredReferences {
        replacements,
        returned: Some(returned),
        established,
        retained_identities: live
            .iter()
            .filter(|reference| !moved.contains(&reference.identity))
            .map(|reference| reference.identity.clone())
            .collect(),
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_origin(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed: &BorrowedReferenceSources,
    block: BlockId,
    node: u32,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    live: &[LiveReference],
    captured: &[Option<LiveReference>],
    origin: &ReferenceOutputOrigin,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    let fail = |reason| invalid(function, block, Some(node), reason);
    match origin {
        ReferenceOutputOrigin::Entry {
            entry,
            suffix,
            extent,
        } => {
            let entry = captured
                .get(*entry)
                .and_then(Option::as_ref)
                .ok_or_else(|| fail("completed reference origin anchor is absent"))?;
            let root = projected_origin_family(&entry.root, suffix, *extent);
            let mut parent = entry.parent.clone();
            if parent == ReferenceParent::Root(entry.root.clone()) {
                parent = ReferenceParent::Root(root.clone());
            }
            Ok((root, parent))
        }
        ReferenceOutputOrigin::Storage {
            parameter,
            path,
            extent,
        } => {
            let formal = callee
                .structural_parameters
                .get(*parameter)
                .ok_or_else(|| fail("completed reference storage parameter is absent"))?;
            let path = path
                .iter()
                .map(|step| match step {
                    ReferenceOriginProjection::Exact(step) => Some(step.clone()),
                    ReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    fail("completed reference storage is not an exact projected path")
                })?;
            let actual = call_source(
                &callee.structural_parameters,
                arguments,
                &StructuralArgument {
                    place: formal.place,
                    path,
                    access: formal.access,
                },
            )
            .ok_or_else(|| fail("completed reference storage actual is absent"))?;
            let (root, mut parent) =
                normalized_source(function, types, borrowed, block, node, live, &actual)?;
            let projected = projected_origin_family(&root, &[], *extent);
            if parent == ReferenceParent::Root(root) {
                parent = ReferenceParent::Root(projected.clone());
            }
            Ok((projected, parent))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_origins(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed: &BorrowedReferenceSources,
    block: BlockId,
    node: u32,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    live: &[LiveReference],
    captured: &[Option<LiveReference>],
    origins: &[ReferenceOutputOrigin],
) -> Result<ReferenceOrigin, OptimizationUnitValidationError> {
    let alternatives = origins
        .iter()
        .map(|origin| {
            bind_origin(
                function, types, borrowed, block, node, arguments, callee, live, captured, origin,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(combine_alternative_origins(function, block, node, alternatives)?.0)
}

#[allow(clippy::too_many_arguments)]
fn bind_parent(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed: &BorrowedReferenceSources,
    block: BlockId,
    node: u32,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    live: &[LiveReference],
    captured: &[Option<LiveReference>],
    origins: &[ReferenceOutputOrigin],
) -> Result<ReferenceParent, OptimizationUnitValidationError> {
    let alternatives = origins
        .iter()
        .map(|origin| {
            bind_origin(
                function, types, borrowed, block, node, arguments, callee, live, captured, origin,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(combine_alternative_origins(function, block, node, alternatives)?.1)
}
