//! Apply ordinary-call pointer-slot writes from current executable bodies.
//! Carrier locations and permission identities are independent: replacing a
//! slot retires its resident binding, while an existing child keeps the old
//! captured root and parent. Incoming transfers retain their own identity.
use super::*;
use semantic_vocabulary::MachineId;
use std::borrow::Cow;
use terminal_semantics::descriptor_lifetimes::descriptor_path;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorStoredOrigin, writable_reference_slots,
};

type Location = (PlaceId, Vec<StructuralPathSegment>);
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    IncomingSlot(StructuralArgument),
    BorrowedStorage(StructuralArgument),
}
#[derive(Debug)]
struct Effect {
    destination: StructuralArgument,
    sources: Vec<Source>,
}
#[derive(Debug, Default)]
pub(super) struct Effects {
    machines: BTreeMap<MachineId, Option<Vec<Effect>>>,
}

fn writable_slots(function: &PsiOptimizationFunction, types: &[StructuralTypeDeclaration]) -> bool {
    function.structural_parameters.iter().any(|parameter| {
        writable_reference_slots(
            types,
            parameter.place,
            parameter.structural_type,
            parameter.access,
        )
        .is_none_or(|slots| !slots.is_empty())
    })
}

pub(super) fn prepare(
    native: &target_operations::TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Effects {
    let types = &plan.structural_types;
    let candidates = unit
        .functions
        .iter()
        .filter(|function| writable_slots(function, types))
        .collect::<Vec<_>>();
    let mut effects = Effects {
        machines: candidates
            .iter()
            .map(|function| (function.machine, None))
            .collect(),
    };
    if candidates.is_empty() {
        return effects;
    }
    let Some(graph) =
        optimization_unit_semantics::descriptor_results::optimized_reference_catalog_graph_with_boundary_preservation(
            &unit.functions,
            &unit.boundary_machines,
            types,
            |caller, node| {
                super::super::call_origin::selected_boundary_operation(node, caller, native, plan)
                    .ok()
                    .map(|selected| match selected {
                        Some((operation, _)) => Cow::Owned(operation),
                        None => Cow::Borrowed(&node.operation),
                    })
            },
            |caller, node| {
                let AbstractOperation::BoundaryCall {
                    psi_operation,
                    boundary,
                    arguments,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                    ..
                } = &node.operation
                else {
                    return false;
                };
                if !completion_claim_sources.is_empty() || !completion_receipts.is_empty() {
                    return false;
                }
                let Ok(Some(target_operations::TargetUnitOperation::NormalizedForeignCall {
                    boundary: selected_boundary,
                    binding,
                    ..
                })) = super::super::normalized_foreign::row(native, caller.machine, *psi_operation)
                else {
                    return false;
                };
                let mut declarations = unit.boundary_machines.iter()
                    .filter(|declaration| declaration.id == *boundary);
                let Some(declaration) = declarations.next() else { return false; };
                *selected_boundary == *boundary
                    && declarations.next().is_none()
                    && arguments.len() == declaration.scalar_parameters.len()
                    && structural_arguments.len() == declaration.structural_parameters.len()
                    && structural_arguments.iter().zip(&declaration.structural_parameters)
                        .all(|(actual, formal)| actual.access == formal.access)
                    // The selected import preserves pointers only under this
                    // guard obligation. Image replay independently requires
                    // the exact bound and count before invoking the import.
                    && abstract_operations_to_target_operations::descriptor_input_guard(
                        declaration, binding, types,
                    ).is_ok_and(|guard| guard.is_some())
            },
        )
    else {
        return effects;
    };
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, &graph);
    for function in candidates {
        let summary = analysis.stored_effects(function.machine).and_then(|rows| {
            rows.into_iter()
                .map(|row| {
                    let parameter = function.structural_parameters.get(row.parameter)?;
                    let path = descriptor_path(types, parameter.structural_type, &row.path)?;
                    let destination = StructuralArgument {
                        place: parameter.place,
                        path,
                        access: parameter.access,
                    };
                    let mut sources = Vec::new();
                    for origin in row.origins {
                        match origin {
                            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                let parameter = function.structural_parameters.get(position)?;
                                sources.push(Source::IncomingSlot(StructuralArgument {
                                    place: parameter.place,
                                    path: descriptor_path(types, parameter.structural_type, &path)?,
                                    access: parameter.access,
                                }));
                            }
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
                                let ty =
                                    leaf_type(types, parameter.structural_type, &destination.path)?;
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
                            DescriptorStoredOrigin::IncomingContents { .. }
                            | DescriptorStoredOrigin::Literal { .. } => return None,
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
    effects
}

pub(super) struct Staged {
    operation: OperationId,
    replacements: Vec<(Location, Option<Leaf>)>,
    relocations: BTreeMap<Location, Location>,
}

fn receiving_contract(
    function: &PsiOptimizationFunction,
    destination: &Location,
) -> Result<CarrierContract, LegalizationError> {
    if let Ok((operation, result)) =
        super::super::structural_case::source_result(function, destination.0)
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

pub(super) fn stage(
    function: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    types: &[StructuralTypeDeclaration],
    custody: &Custody,
    arguments: &[StructuralArgument],
    operation: OperationId,
    moved: &BTreeMap<Location, Leaf>,
) -> Result<Staged, LegalizationError> {
    let mut staged = Staged {
        operation,
        replacements: Vec::new(),
        relocations: BTreeMap::new(),
    };
    let Some(callee) = callee else {
        if arguments.iter().any(|argument| {
            source_contract(function, custody, argument.place)
                .and_then(|ty| leaf_type(types, ty, &argument.path))
                .is_none_or(|ty| {
                    writable_reference_slots(types, argument.place, ty, argument.access)
                        .is_none_or(|slots| !slots.is_empty())
                })
        }) {
            return Err(invalid());
        }
        return Ok(staged);
    };
    let Some(effects) = custody.stored_calls.machines.get(&callee.machine) else {
        return Ok(staged);
    };
    let effects = effects.as_ref().ok_or_else(|| invalid())?;
    let actual = |formal: &StructuralArgument| {
        call_source(&callee.structural_parameters, arguments, formal).ok_or_else(|| invalid())
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
        let StructuralTypeShape::Reference { referent, access } = find_structural_type(types, ty)
            .ok_or_else(|| invalid())?
            .shape
        else {
            return Err(invalid());
        };
        let mut candidates = Vec::new();
        for origin in &effect.sources {
            let formal = match origin {
                Source::IncomingSlot(formal) => formal,
                Source::BorrowedStorage(formal) => {
                    let source = actual(formal)?;
                    if source.access != access
                        || reference_source_type(function, custody, types, &source)
                            != Some(referent)
                    {
                        return Err(invalid());
                    }
                    let (root, mut parents) = normalized_source(function, custody, types, &source)?;
                    let (backing, backing_parents) = descriptor_backing::capture(
                        function, custody, types, &source, operation, &root,
                    )?;
                    parents.extend(backing_parents);
                    parents.sort();
                    parents.dedup();
                    let contract = receiving_contract(function, &destination)?;
                    candidates.push(Leaf {
                        identity: OccurrenceIdentity::operation(&destination, operation),
                        root,
                        backing,
                        parents,
                        formations: vec![Formation {
                            identity: OccurrenceIdentity::operation(&destination, operation),
                            contract: contract.clone(),
                        }],
                        contract,
                        referent,
                    });
                    continue;
                }
            };
            let source = if formal.place == effect.destination.place
                && formal.path == effect.destination.path
            {
                // Preserving an incoming binding does not read its pointer.
                destination.clone()
            } else {
                let source = actual(formal)?;
                window_storage(
                    function,
                    custody,
                    types,
                    source.place,
                    &source.path,
                    StructuralAccess::SharedBorrow,
                )?
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
                if moved.contains_key(&source) {
                    if staged
                        .relocations
                        .insert(source.clone(), destination.clone())
                        .is_some()
                    {
                        return Err(invalid());
                    }
                } else {
                    if access != StructuralAccess::SharedBorrow {
                        return Err(invalid());
                    }
                    receiving_identity(function, &destination, operation, &mut leaf)?;
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
            }
            // A reused storage coordinate must never turn historical children
            // into children of the newly installed permission.
            if custody
                .leaves
                .get(&destination)
                .is_some_and(|old| old.identity == leaf.identity && old != &leaf)
                && custody
                    .leaves
                    .values()
                    .any(|child| custody.descends_from(child, &leaf.identity))
            {
                return Err(invalid());
            }
            Some(leaf)
        };
        staged.replacements.push((destination, replacement));
    }
    Ok(staged)
}

pub(super) fn commit(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    result: Option<PlaceId>,
    custody: &mut Custody,
    staged: Staged,
) -> Result<(), LegalizationError> {
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
        .map(|(key, leaf)| Some((leaf.root.clone(), access_at(key)?)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| invalid())?;
    for (destination, leaf) in &staged.replacements {
        let Some(leaf) = leaf else {
            continue;
        };
        let access = access_at(destination).ok_or_else(|| invalid())?;
        if published.iter().any(|(root, previous)| {
            roots_overlap(root, &leaf.root)
                && (access != StructuralAccess::SharedBorrow
                    || *previous != StructuralAccess::SharedBorrow)
        }) {
            return Err(invalid());
        }
        published.push((leaf.root.clone(), access));
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
            // A transferred permission cannot simultaneously be installed in
            // another slot or returned as a second live occurrence.
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
    borrowed_arrivals::relocate(custody, &staged.relocations);
    Ok(())
}
