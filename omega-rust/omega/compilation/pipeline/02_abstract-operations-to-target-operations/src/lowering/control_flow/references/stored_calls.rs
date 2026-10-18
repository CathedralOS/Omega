//! Ordinary calls replace descriptor metadata at the same exact slots whose
//! contents their current bodies replace. This records backing provenance only;
//! loan permissions and historical identities stay with the independent audits.
use super::*;
use abstract_operations::call_source;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::borrow::Cow;
use terminal_semantics::descriptor_lifetimes::descriptor_path;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorStoredOrigin, writable_reference_slots,
};

type Location = (PlaceId, Vec<StructuralPathSegment>);
struct Effect {
    destination: StructuralArgument,
    sources: Vec<Source>,
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Incoming(StructuralArgument),
    Backing(StructuralArgument),
}

pub(in crate::lowering) struct StoredEffects {
    machines: BTreeMap<MachineId, Option<Vec<Effect>>>,
}
pub(in crate::lowering) type StoredEffectCatalog = std::sync::Arc<StoredEffects>;

fn writable_slots(function: &AbstractFunction, types: &StructuralTypeLookup<'_>) -> bool {
    function.structural_parameters.iter().any(|parameter| {
        writable_reference_slots(
            types.catalog(),
            parameter.place,
            parameter.structural_type,
            parameter.access,
        )
        .is_none_or(|slots| !slots.is_empty())
    })
}

pub(in crate::lowering) fn catalog(
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    types: &StructuralTypeLookup<'_>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    selected_calls: &crate::lowering::selected_boundary_calls::SelectedCallsByCall,
    settlements: &BTreeMap<BoundaryMachineId, target_operations::BoundarySettlementBinding>,
) -> StoredEffectCatalog {
    let candidates = functions
        .values()
        .copied()
        .filter(|function| writable_slots(function, types))
        .collect::<Vec<_>>();
    let mut effects = StoredEffects {
        machines: candidates
            .iter()
            .map(|function| (function.machine, None))
            .collect(),
    };
    if candidates.is_empty() {
        return std::sync::Arc::new(effects);
    }
    let Some(graph) =
        optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph_with_boundary_preservation(
            functions.values().copied(),
            boundaries,
            types.catalog(),
        dynamic_dispatch,
            |function, node| {
                if let AbstractOperation::BoundaryCall { psi_operation, boundary, .. } = &node.operation
                    && let Some(selected) = selected_calls.get(&(function.machine, *psi_operation, *boundary))
                {
                    return super::super::selected_boundary_calls::resolve(&node.operation, selected, functions)
                        .ok().map(|(operation, _)| Cow::Owned(operation));
                }
                Some(Cow::Borrowed(&node.operation))
            },
            |function, node| {
                let AbstractOperation::BoundaryCall {
                    psi_operation, boundary, arguments, structural_arguments,
                    completion_claim_sources, completion_receipts, ..
                } = &node.operation else { return false; };
                if selected_calls.contains_key(&(function.machine, *psi_operation, *boundary))
                    || !completion_claim_sources.is_empty() || !completion_receipts.is_empty() {
                    return false;
                }
                let Some(settlement) = settlements.get(boundary) else { return false; };
                let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                    &settlement.realization else { return false; };
                let mut declarations = boundaries.iter().filter(|declaration| declaration.id == *boundary);
                let Some(declaration) = declarations.next() else { return false; };
                declarations.next().is_none()
                    && arguments.len() == declaration.scalar_parameters.len()
                    && structural_arguments.len() == declaration.structural_parameters.len()
                    && structural_arguments.iter().zip(&declaration.structural_parameters)
                        .all(|(actual, formal)| actual.access == formal.access)
                    // This is an emission obligation, not a signature-derived
                    // memory effect. Selection and image replay independently
                    // demand this exact bounded prefix guard before the import.
                    && crate::descriptor_input_guard(declaration, binding, types.catalog())
                        .is_ok_and(|guard| guard.is_some())
            },
        )
    else {
        return std::sync::Arc::new(effects);
    };
    let mut analysis = DescriptorAnalysis::with_reference_slots(types.catalog(), &graph);
    for function in candidates {
        let summary = analysis.stored_effects(function.machine).and_then(|rows| {
            rows.into_iter()
                .map(|row| {
                    let parameter = function.structural_parameters.get(row.parameter)?;
                    let destination = StructuralArgument {
                        place: parameter.place,
                        path: descriptor_path(
                            types.catalog(),
                            parameter.structural_type,
                            &row.exact_path()?,
                        )?,
                        access: parameter.access,
                    };
                    let mut sources = Vec::new();
                    for origin in row.origins {
                        match origin {
                            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                let parameter = function.structural_parameters.get(position)?;
                                sources.push(Source::Incoming(StructuralArgument {
                                    place: parameter.place,
                                    path: descriptor_path(
                                        types.catalog(),
                                        parameter.structural_type,
                                        &path,
                                    )?,
                                    access: parameter.access,
                                }));
                            }
                            DescriptorStoredOrigin::Parameter {
                                position,
                                projection,
                            } => {
                                if !projection.exact
                                    || projection.indirect
                                    || projection.window.is_some()
                                    || projection.captured_at.is_some()
                                {
                                    return None;
                                }
                                let source = function.structural_parameters.get(position)?;
                                let carrier =
                                    leaf_type(types, parameter.structural_type, &destination.path)?;
                                let StructuralTypeShape::Reference { access, .. } =
                                    types.get(&carrier)?.shape
                                else {
                                    return None;
                                };
                                if !access_covers(source.access, access) {
                                    return None;
                                }
                                sources.push(Source::Backing(StructuralArgument {
                                    place: source.place,
                                    path: projection.path,
                                    access,
                                }));
                            }
                            DescriptorStoredOrigin::Empty => {}
                            DescriptorStoredOrigin::Invocation { .. }
                            | DescriptorStoredOrigin::IncomingContents { .. }
                            | DescriptorStoredOrigin::Literal { .. }
                            | DescriptorStoredOrigin::PlacedFormation { .. } => return None,
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
    std::sync::Arc::new(effects)
}

pub(in super::super) struct Staged {
    replacements: Vec<(Location, Option<ReferenceStorage>)>,
}

pub(in super::super) fn stage(
    function: &AbstractFunction,
    callee: Option<&AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    arguments: &[StructuralArgument],
) -> Result<Staged, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let mut staged = Staged {
        replacements: Vec::new(),
    };
    let Some(callee) = callee else {
        if arguments.iter().any(|argument| {
            source_contract(function, live, argument.place)
                .and_then(|ty| leaf_type(types, ty, &argument.path))
                .is_some_and(|ty| {
                    writable_reference_slots(types.catalog(), argument.place, ty, argument.access)
                        .is_none_or(|slots| !slots.is_empty())
                })
        }) {
            return Err(invalid());
        }
        return Ok(staged);
    };
    let Some(effects) = live.stored_reference_effects.machines.get(&callee.machine) else {
        return Ok(staged);
    };
    let effects = effects.as_ref().ok_or_else(invalid)?;
    let actual = |formal: &StructuralArgument| {
        let argument =
            call_source(&callee.structural_parameters, arguments, formal).ok_or_else(invalid)?;
        window_storage(function, types, live, argument.place, &argument.path)
    };
    let mut destinations = BTreeSet::new();
    for effect in effects {
        let destination = actual(&effect.destination)?;
        if !destinations.insert(destination.clone()) {
            return Err(invalid());
        }
        let carrier_type = source_contract(function, live, destination.0)
            .and_then(|ty| leaf_type(types, ty, &destination.1))
            .ok_or_else(invalid)?;
        let referent = referent(types, carrier_type).ok_or_else(invalid)?;
        let mut candidates = Vec::new();
        for source in &effect.sources {
            let leaf = match source {
                Source::Incoming(formal) => {
                    let source = actual(formal)?;
                    let Some(leaf) = live.references.get(&source) else {
                        if source
                            .1
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                        {
                            continue;
                        }
                        return Err(invalid());
                    };
                    leaf.clone()
                }
                Source::Backing(formal) => {
                    let source = call_source(&callee.structural_parameters, arguments, formal)
                        .ok_or_else(invalid)?;
                    if reference_source_type(function, live, types, &source) != Some(referent) {
                        return Err(invalid());
                    }
                    ReferenceStorage {
                        root: normalized_source(function, live, types, &source)?,
                        carrier_type,
                        referent,
                        placed: super::placed_source(live, &source),
                    }
                }
            };
            if leaf.carrier_type != carrier_type || leaf.referent != referent {
                return Err(invalid());
            }
            candidates.push(leaf);
        }
        let replacement = if candidates.is_empty() {
            None
        } else {
            Some(combine_alternatives(function, candidates)?)
        };
        staged.replacements.push((destination, replacement));
    }
    Ok(staged)
}

pub(in super::super) fn commit(live: &mut LiveDefinitions, staged: Staged) {
    for (destination, replacement) in staged.replacements {
        if let Some(leaf) = replacement {
            live.references.insert(destination, leaf);
        } else {
            live.references.remove(&destination);
        }
    }
}
