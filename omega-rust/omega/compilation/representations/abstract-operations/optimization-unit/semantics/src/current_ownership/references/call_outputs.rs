//! A completed body supplies the loan graph, while each call supplies its own
//! live input occurrences. Bind parents before publishing any output so a
//! replaced slot cannot redirect a child to its new resident pointer.
use super::*;
use crate::reference_outputs::{
    ReferenceBodyOutputs, ReferenceOutputInterpretation, ReferenceOutputOrigin,
    ReferenceOutputParent, ReferenceOutputResidence, ReferenceOutputSupply,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn bind(
    inputs: &CallReferenceInputs<'_, '_>,
    callee: &PsiOptimizationFunction,
    body: &ReferenceBodyOutputs,
    borrowed_sources: &BorrowedReferenceSources,
    arguments: &[StructuralArgument],
    result: Option<&StructuralOperationResult>,
    moved: &BTreeSet<ReferenceOccurrenceIdentity>,
) -> Result<call_effects::StoredReferences, OptimizationUnitValidationError> {
    let CallReferenceInputs {
        function,
        types,
        block,
        node,
        live,
    } = *inputs;
    let fail = |reason| invalid(function, block, Some(node), reason);
    let actual = |parameter: usize, path: &[StructuralPathSegment]| {
        let formal = callee
            .structural_parameters
            .get(parameter)
            .ok_or_else(|| fail("completed reference output parameter is absent"))?;
        abstract_operations::calls::call_source_at(arguments, parameter, path, formal.access)
            .ok_or_else(|| fail("completed reference output actual is absent"))
    };
    // Capture once, before any destination is replaced. An entry anchor is an
    // actual live pointer word; its type or root spelling cannot supply it.
    let mut residents = live.iter().collect::<Vec<_>>();
    residents.sort_by(|left, right| {
        (left.carrier, &left.carrier_path).cmp(&(right.carrier, &right.carrier_path))
    });
    let mut captured = vec![None; body.entry().len()];
    for &index in body.required_entries() {
        let entry = &body.entry()[index];
        let parameter = body
            .entry_parameter(index)
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
        let first = residents.partition_point(|reference| {
            (reference.carrier, reference.carrier_path.as_slice())
                < (slot.place, slot.path.as_slice())
        });
        let after = residents.partition_point(|reference| {
            (reference.carrier, reference.carrier_path.as_slice())
                <= (slot.place, slot.path.as_slice())
        });
        if after - first != 1 {
            return Err(fail(
                "completed reference entry is absent or ambiguous at this call",
            ));
        }
        let resident = residents[first].clone();
        captured[index] = Some(resident);
    }
    let destinations = body
        .slots()
        .iter()
        .map(|slot| {
            let actual = abstract_operations::calls::call_source_at(
                arguments,
                slot.parameter(),
                slot.path(),
                slot.access(),
            )
            .ok_or_else(|| fail("completed writable output actual is absent"))?;
            let destination =
                call_effects::resolve_slot(function, types, block, node, live, &actual)?;
            if call_effects::slot_type(function, types, &destination) != Some(slot.reference_type())
            {
                return Err(fail(
                    "completed writable output changes its actual descriptor type",
                ));
            }
            Ok(destination)
        })
        .collect::<Result<Vec<_>, OptimizationUnitValidationError>>()?;
    let mut destination_keys = destinations.iter().collect::<Vec<_>>();
    destination_keys.sort();
    if destination_keys.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(fail(
            "completed outputs map distinct writable slots to the same actual slot",
        ));
    }
    let mut completed = None;
    for exit in body.exits() {
        let mut nodes: Vec<LiveReference> = Vec::with_capacity(exit.graph().len());
        let mut replacements = Vec::new();
        let mut returned = Vec::new();
        let mut established = BTreeSet::new();
        let mut covered = vec![false; destinations.len()];
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
                ReferenceOutputResidence::Parameter { slot } => {
                    let present = covered
                        .get_mut(*slot)
                        .ok_or_else(|| fail("completed writable slot index is absent"))?;
                    if *present {
                        return Err(fail("completed body repeats a writable output slot"));
                    }
                    *present = true;
                    (destinations[*slot].clone(), true, false)
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
                        && !body.entry_is_writable(*index)
                    {
                        return Err(fail(
                            "completed reference transfer did not consume its actual argument",
                        ));
                    }
                    if (reference.carrier != destination.place
                        || reference.carrier_path != destination.path)
                        && !moved.contains(&reference.identity)
                        && live.iter().any(|child| {
                            matches!(&child.parent,
                            ReferenceParent::Reference(parent) if parent == &reference.identity)
                        })
                    {
                        return Err(fail("completed reference transfer source has a live child"));
                    }
                    reference.clone()
                }
                ReferenceOutputSupply::Constructed { origins, parent } => {
                    let root = bind_origins(
                        inputs,
                        borrowed_sources,
                        arguments,
                        callee,
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
                            bind_parent(inputs, borrowed_sources, arguments, callee, &captured, origins)?
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
                    let interpretation = match output.interpretation() {
                        ReferenceOutputInterpretation::Unplaced => {
                            ReferenceInterpretation::Unplaced
                        }
                        ReferenceOutputInterpretation::Entry(index) => captured
                            .get(*index)
                            .and_then(Option::as_ref)
                            .ok_or_else(|| fail("completed interpretation entry is unavailable"))?
                            .interpretation
                            .clone(),
                        ReferenceOutputInterpretation::Captured(backing) => {
                            ReferenceInterpretation::Captured(backing.clone())
                        }
                        ReferenceOutputInterpretation::Source {
                            parameter,
                            path,
                            access,
                        } => {
                            let source = abstract_operations::calls::call_source_at(
                                arguments, *parameter, path, *access,
                            )
                            .ok_or_else(|| {
                                fail("completed interpretation source actual is absent")
                            })?;
                            whole_source_interpretation(
                                function,
                                types,
                                live,
                                &source,
                                output.reference_type(),
                                &root,
                                &parent,
                                true,
                            )
                        }
                    };
                    LiveReference {
                        identity,
                        carrier: destination.place,
                        carrier_path: destination.path.clone(),
                        root,
                        parent,
                        interpretation,
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
        let cleared = exit
            .cleared_slots()
            .ok_or_else(|| fail("completed output clear roster is unavailable"))?;
        for &slot in cleared {
            let present = covered
                .get_mut(slot)
                .ok_or_else(|| fail("completed output clear index is absent"))?;
            if *present {
                return Err(fail("completed output clear overlaps a live output"));
            }
            *present = true;
        }
        if covered.iter().any(|present| !present) {
            return Err(fail("completed body omits a typed writable output slot"));
        }
        let current = (replacements, returned, established, cleared.to_vec());
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
    let (replacements, returned, established, _) =
        completed.ok_or_else(|| fail("reference-output body has no completed normal exit"))?;
    Ok(call_effects::StoredReferences {
        replacements,
        destinations,
        returned: Some(returned),
        established,
        historical_parents: live
            .iter()
            .filter_map(|reference| match &reference.parent {
                ReferenceParent::Reference(parent) => Some(parent.clone()),
                ReferenceParent::Root(_) => None,
            })
            .collect(),
        retained_identities: live
            .iter()
            .filter(|reference| !moved.contains(&reference.identity))
            .map(|reference| reference.identity.clone())
            .collect(),
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_origin(
    inputs: &CallReferenceInputs<'_, '_>,
    borrowed: &BorrowedReferenceSources,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    captured: &[Option<LiveReference>],
    origin: &ReferenceOutputOrigin,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    let CallReferenceInputs {
        function,
        types,
        block,
        node,
        live,
    } = *inputs;
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
            let actual = abstract_operations::calls::call_source_at(
                arguments,
                *parameter,
                &path,
                formal.access,
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
    inputs: &CallReferenceInputs<'_, '_>,
    borrowed: &BorrowedReferenceSources,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    captured: &[Option<LiveReference>],
    origins: &[ReferenceOutputOrigin],
) -> Result<ReferenceOrigin, OptimizationUnitValidationError> {
    let CallReferenceInputs {
        function,
        block,
        node,
        ..
    } = *inputs;
    let alternatives = origins
        .iter()
        .map(|origin| bind_origin(inputs, borrowed, arguments, callee, captured, origin))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(combine_alternative_origins(function, block, node, alternatives)?.0)
}

#[allow(clippy::too_many_arguments)]
fn bind_parent(
    inputs: &CallReferenceInputs<'_, '_>,
    borrowed: &BorrowedReferenceSources,
    arguments: &[StructuralArgument],
    callee: &PsiOptimizationFunction,
    captured: &[Option<LiveReference>],
    origins: &[ReferenceOutputOrigin],
) -> Result<ReferenceParent, OptimizationUnitValidationError> {
    let CallReferenceInputs {
        function,
        block,
        node,
        ..
    } = *inputs;
    let alternatives = origins
        .iter()
        .map(|origin| bind_origin(inputs, borrowed, arguments, callee, captured, origin))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(combine_alternative_origins(function, block, node, alternatives)?.1)
}
