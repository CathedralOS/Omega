//! A stored result needs the loan actually formed by the callee. Bind its
//! completed parent graph to this invocation's captured descriptors; pointer
//! origins alone cannot authorize moving or creating a permission.
use super::*;
use optimization_unit_semantics::reference_outputs::{
    ReferenceBodyOutputs, ReferenceOutputInterpretation, ReferenceOutputOrigin,
    ReferenceOutputParent, ReferenceOutputResidence, ReferenceOutputSupply,
};

#[derive(Clone)]
struct Captured {
    location: Location,
    leaf: Leaf,
}

struct Origin {
    root: Root,
    backing: Vec<descriptor_backing::Backing>,
    parents: Vec<OccurrenceIdentity>,
    debts: Vec<lifetime_debts::Debt>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage(
    function: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    body: &ReferenceBodyOutputs,
    types: &[StructuralTypeDeclaration],
    custody: &Custody,
    arguments: &[StructuralArgument],
    operation: OperationId,
    moved: &BTreeMap<Location, Leaf>,
    result: Option<&StructuralOperationResult>,
) -> Result<Staged, LegalizationError> {
    let actual = |parameter: usize, path: &[StructuralPathSegment]| {
        let formal = callee
            .structural_parameters
            .get(parameter)
            .ok_or_else(|| invalid())?;
        abstract_operations::calls::call_source_at(arguments, parameter, path, formal.access)
            .ok_or_else(|| invalid())
    };
    let mut captured = vec![None; body.entry().len()];
    for &index in body.required_entries() {
        let entry = &body.entry()[index];
        let parameter = body.entry_parameter(index).ok_or_else(|| invalid())?;
        let actual = actual(parameter, &entry.carrier_path)?;
        let location = window_storage(
            function,
            custody,
            types,
            actual.place,
            &actual.path,
            StructuralAccess::SharedBorrow,
        )?;
        let leaf = custody
            .leaves
            .get(&location)
            .cloned()
            .ok_or_else(|| invalid())?;
        let expected = leaf_type(
            types,
            callee.structural_parameters[parameter].structural_type,
            &entry.carrier_path,
        )
        .ok_or_else(|| invalid())?;
        if source_contract(function, custody, location.0)
            .and_then(|root| leaf_type(types, root, &location.1))
            != Some(expected)
        {
            return Err(invalid());
        }
        captured[index] = Some(Captured { location, leaf });
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
            .ok_or_else(|| invalid())?;
            let destination = window_storage(
                function,
                custody,
                types,
                actual.place,
                &actual.path,
                StructuralAccess::WriteOnlyBorrow,
            )?;
            if source_contract(function, custody, destination.0)
                .and_then(|root| leaf_type(types, root, &destination.1))
                != Some(slot.reference_type())
            {
                return Err(invalid());
            }
            Ok(destination)
        })
        .collect::<Result<Vec<_>, LegalizationError>>()?;
    let mut destination_keys = destinations.iter().collect::<Vec<_>>();
    destination_keys.sort();
    if destination_keys.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(invalid());
    }
    let mut completed = None;
    for exit in body.exits() {
        let mut staged = Staged {
            operation,
            replacements: Vec::new(),
            relocations: BTreeMap::new(),
            established: BTreeSet::new(),
            returned: Some(Vec::new()),
        };
        let mut nodes: Vec<Leaf> = Vec::with_capacity(exit.graph().len());
        let mut covered = vec![false; destinations.len()];
        for output in exit.graph() {
            let (destination, publish, returned) = match output.residence() {
                ReferenceOutputResidence::Entry(entry) => (
                    captured
                        .get(*entry)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| invalid())?
                        .location
                        .clone(),
                    false,
                    false,
                ),
                ReferenceOutputResidence::Parameter { slot } => {
                    let present = covered.get_mut(*slot).ok_or_else(|| invalid())?;
                    if *present {
                        return Err(invalid());
                    }
                    *present = true;
                    (destinations[*slot].clone(), true, false)
                }
                ReferenceOutputResidence::Returned { path } => (
                    (result.ok_or_else(|| invalid())?.place, path.clone()),
                    true,
                    true,
                ),
            };
            if source_contract(function, custody, destination.0)
                .and_then(|root| leaf_type(types, root, &destination.1))
                != Some(output.reference_type())
            {
                return Err(invalid());
            }
            let mut leaf = match output.supply() {
                ReferenceOutputSupply::Entry(index) => {
                    let entry = captured
                        .get(*index)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| invalid())?;
                    if entry.location != destination {
                        let moved_by_argument = moved
                            .get(&entry.location)
                            .is_some_and(|leaf| leaf == &entry.leaf);
                        if (!moved_by_argument && !body.entry_is_writable(*index))
                            || (!moved_by_argument
                                && custody.leaves.values().any(|child| {
                                    custody.descends_from(child, &entry.leaf.identity)
                                        || custody.lifetime_depends_on(child, &entry.leaf.identity)
                                }))
                            || staged
                                .relocations
                                .insert(entry.location.clone(), destination.clone())
                                .is_some()
                        {
                            return Err(invalid());
                        }
                    }
                    entry.leaf.clone()
                }
                ReferenceOutputSupply::Constructed { origins, parent } => {
                    let origin = bind_origins(
                        function, callee, types, custody, arguments, operation, &captured, origins,
                    )?;
                    let mut parents = match parent {
                        ReferenceOutputParent::Entry(index) => vec![
                            captured
                                .get(*index)
                                .and_then(Option::as_ref)
                                .ok_or_else(|| invalid())?
                                .leaf
                                .identity
                                .clone(),
                        ],
                        ReferenceOutputParent::Node(index) => {
                            vec![nodes.get(*index).ok_or_else(|| invalid())?.identity.clone()]
                        }
                        ReferenceOutputParent::Root(origins) => {
                            bind_origins(
                                function, callee, types, custody, arguments, operation, &captured,
                                origins,
                            )?
                            .parents
                        }
                    };
                    parents.sort();
                    parents.dedup();
                    let contract = receiving_contract(function, &destination)?;
                    let identity = OccurrenceIdentity::operation(&destination, operation);
                    let referent = leaf_referent(types, output.reference_type(), &[])
                        .ok_or_else(|| invalid())?;
                    let placed = match output.interpretation() {
                        ReferenceOutputInterpretation::Unplaced => None,
                        ReferenceOutputInterpretation::Entry(index) => captured
                            .get(*index)
                            .and_then(Option::as_ref)
                            .ok_or_else(|| invalid())?
                            .leaf
                            .placed
                            .clone(),
                        ReferenceOutputInterpretation::Captured(backing) => {
                            if backing.carrier() != referent {
                                return Err(invalid());
                            }
                            Some(target_operations::TargetAccessInterpretation::Captured(
                                backing.clone(),
                            ))
                        }
                        ReferenceOutputInterpretation::Source {
                            parameter,
                            path,
                            access,
                        } => {
                            let source = abstract_operations::calls::call_source_at(
                                arguments, *parameter, path, *access,
                            )
                            .ok_or_else(|| invalid())?;
                            if reference_source_type(function, custody, types, &source)
                                != Some(referent)
                            {
                                return Err(invalid());
                            }
                            super::super::access_source(function, custody, types, &source)
                        }
                    };
                    staged.established.insert(destination.clone());
                    Leaf {
                        identity: identity.clone(),
                        root: origin.root,
                        backing: origin.backing,
                        parents,
                        lifetime_debts: origin.debts,
                        referent,
                        placed,
                        formations: vec![Formation {
                            identity,
                            contract: contract.clone(),
                        }],
                        contract,
                    }
                }
            };
            // A moved entry keeps its formation and contract. A new node has
            // the actual receiving declaration and this call's birth event.
            if publish {
                if returned {
                    staged
                        .returned
                        .as_mut()
                        .ok_or_else(|| invalid())?
                        .push((destination, leaf.clone()));
                } else {
                    staged.replacements.push((destination, Some(leaf.clone())));
                }
            }
            leaf.parents.sort();
            leaf.parents.dedup();
            nodes.push(leaf);
        }
        for &index in exit.cleared_slots().ok_or_else(|| invalid())? {
            let present = covered.get_mut(index).ok_or_else(|| invalid())?;
            if *present {
                return Err(invalid());
            }
            *present = true;
            staged.replacements.push((
                destinations.get(index).cloned().ok_or_else(|| invalid())?,
                None,
            ));
        }
        if covered.iter().any(|present| !present) {
            return Err(invalid());
        }
        staged
            .replacements
            .sort_by(|left, right| left.0.cmp(&right.0));
        staged
            .returned
            .as_mut()
            .ok_or_else(|| invalid())?
            .sort_by(|left, right| left.0.cmp(&right.0));
        if staged
            .replacements
            .windows(2)
            .any(|rows| rows[0].0 == rows[1].0)
        {
            return Err(invalid());
        }
        if completed.as_ref().is_some_and(|previous: &Staged| {
            previous.replacements != staged.replacements
                || previous.returned != staged.returned
                || previous.relocations != staged.relocations
                || previous.established != staged.established
        }) {
            // Current custody has no union of divergent parent graphs. Keep
            // that limit explicit rather than choosing one normal exit.
            return Err(invalid());
        }
        completed = Some(staged);
    }
    completed.ok_or_else(|| invalid())
}

#[allow(clippy::too_many_arguments)]
fn bind_origins(
    function: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    custody: &Custody,
    arguments: &[StructuralArgument],
    operation: OperationId,
    captured: &[Option<Captured>],
    origins: &[ReferenceOutputOrigin],
) -> Result<Origin, LegalizationError> {
    let mut candidates = Vec::with_capacity(origins.len());
    let mut backing = Vec::new();
    let mut debts = Vec::new();
    for origin in origins {
        let (root, parents, observed_backing, observed_debts) = match origin {
            ReferenceOutputOrigin::Entry {
                entry,
                suffix,
                extent,
            } => {
                let entry = &captured
                    .get(*entry)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| invalid())?
                    .leaf;
                (
                    entry.root.projected_family(suffix, *extent),
                    entry.parents.clone(),
                    entry.backing.clone(),
                    custody
                        .captured_lifetime_debts(entry)
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            }
            ReferenceOutputOrigin::Storage {
                parameter,
                path,
                extent,
            } => {
                let formal = callee
                    .structural_parameters
                    .get(*parameter)
                    .ok_or_else(|| invalid())?;
                let path = path
                    .iter()
                    .map(|step| match step {
                        ReferenceOriginProjection::Exact(step) => Some(step.clone()),
                        ReferenceOriginProjection::Element => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| invalid())?;
                let actual = abstract_operations::calls::call_source_at(
                    arguments,
                    *parameter,
                    &path,
                    formal.access,
                )
                .ok_or_else(|| invalid())?;
                let (root, mut parents) = normalized_source(function, custody, types, &actual)?;
                let (backing, captured_parents) = descriptor_backing::capture(
                    function, custody, types, &actual, operation, &root,
                )?;
                parents.extend(captured_parents);
                parents.sort();
                parents.dedup();
                (
                    root.projected_family(&[], *extent),
                    parents,
                    backing,
                    Vec::new(),
                )
            }
        };
        candidates.push((root, parents));
        backing.extend(observed_backing);
        debts.extend(observed_debts);
    }
    let (root, parents) = combine_candidate_origins(candidates)?;
    backing.sort();
    backing.dedup();
    debts.sort();
    debts.dedup();
    Ok(Origin {
        root,
        parents,
        backing,
        debts,
    })
}
