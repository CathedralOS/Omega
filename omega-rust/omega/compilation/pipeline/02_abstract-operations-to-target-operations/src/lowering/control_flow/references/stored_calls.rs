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
    Provider(target_operations::TargetReferenceOrigin),
}

pub(in crate::lowering) struct StoredEffects {
    machines: BTreeMap<MachineId, Option<Vec<Effect>>>,
    provider_outputs: BTreeMap<(BoundaryMachineId, u32), (StructuralTypeId, StructuralAccess)>,
    pub(super) observations:
        BTreeMap<MachineId, terminal_semantics::descriptor_results::DescriptorSourceObservations>,
}
pub(in crate::lowering) type StoredEffectCatalog = std::sync::Arc<StoredEffects>;

impl StoredEffects {
    /// Physical metadata comes from the admitted boundary's promised leaf,
    /// never from a lifetime input's storage. The custody audit independently
    /// captures and enforces the provider's lifetime dependencies.
    pub(super) fn provider_origin<Dependency>(
        &self,
        provider: &terminal_semantics::descriptor_results::DescriptorProviderOrigin<Dependency>,
    ) -> Option<target_operations::TargetReferenceOrigin> {
        let (referent, access) = *self
            .provider_outputs
            .get(&(provider.boundary, provider.output))?;
        if provider.invocations.is_empty()
            || !provider.projection.exact
            || provider.projection.indirect
            || provider.projection.window.is_some()
            || provider.projection.captured_at.is_some()
        {
            return None;
        }
        Some(target_operations::TargetReferenceOrigin {
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
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        })
    }
}

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
        provider_outputs: BTreeMap::new(),
        observations: BTreeMap::new(),
        machines: candidates
            .iter()
            .map(|function| (function.machine, None))
            .collect(),
    };
    for boundary in boundaries {
        if !settlements.get(&boundary.id).is_some_and(|settlement| {
            matches!(
                settlement.realization,
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
            )
        }) {
            continue;
        }
        for (position, output) in boundary.reference_outputs.iter().enumerate() {
            let root = match output.destination {
                terminal_psi::BoundaryReferenceDestination::Parameter(position) => boundary
                    .structural_parameters
                    .get(position as usize)
                    .map(|parameter| parameter.structural_type),
            };
            let Some(StructuralTypeShape::Reference { referent, access }) = root
                .and_then(|root| leaf_type(types, root, &output.path))
                .and_then(|carrier| types.get(&carrier).map(|declaration| &declaration.shape))
            else {
                continue;
            };
            if *access == output.access
                && let Ok(position) = u32::try_from(position)
            {
                effects
                    .provider_outputs
                    .insert((boundary.id, position), (*referent, *access));
            }
        }
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
    // Read-only functions also observe nested reference contents. Restricting
    // this roster to functions with write effects loses those observations.
    for machine in &graph {
        effects
            .observations
            .insert(machine.id, analysis.source_observations(machine.id));
    }
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
                            DescriptorStoredOrigin::Provider(provider) => {
                                sources.push(Source::Provider(effects.provider_origin(&provider)?));
                            }
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
    operation: semantic_vocabulary::OperationId,
    callee: Option<&AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    arguments: &[StructuralArgument],
) -> Result<Staged, LoweringError> {
    let invalid = |reason| LoweringError::StoredReferenceCall {
        caller: function.machine,
        callee: callee.map(|callee| callee.machine),
        operation,
        reason,
    };
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
            return Err(invalid(
                "dynamic call has no stored-reference effect summary",
            ));
        }
        return Ok(staged);
    };
    let Some(effects) = live.stored_reference_effects.machines.get(&callee.machine) else {
        return Ok(staged);
    };
    let effects = effects
        .as_ref()
        .ok_or_else(|| invalid("callee stored-reference effects are unresolved"))?;
    let actual = |formal: &StructuralArgument| {
        let argument = call_source(&callee.structural_parameters, arguments, formal)
            .ok_or_else(|| invalid("effect formal has no corresponding actual argument"))?;
        window_storage(function, types, live, argument.place, &argument.path)
    };
    let mut destinations = BTreeSet::new();
    for effect in effects {
        let destination = actual(&effect.destination)?;
        if !destinations.insert(destination.clone()) {
            return Err(invalid(
                "stored-reference effects overlap at one destination",
            ));
        }
        let carrier_type = source_contract(function, live, destination.0)
            .and_then(|ty| leaf_type(types, ty, &destination.1))
            .ok_or_else(|| invalid("destination has no structural carrier type"))?;
        let referent = referent(types, carrier_type)
            .ok_or_else(|| invalid("destination carrier is not a reference"))?;
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
                        return Err(invalid("incoming reference slot has no live provenance"));
                    };
                    leaf.clone()
                }
                Source::Backing(formal) => {
                    let source = call_source(&callee.structural_parameters, arguments, formal)
                        .ok_or_else(|| {
                            invalid("backing formal has no corresponding actual argument")
                        })?;
                    if reference_source_type(function, live, types, &source) != Some(referent) {
                        return Err(invalid(
                            "backing argument does not have the destination referent type",
                        ));
                    }
                    ReferenceStorage {
                        root: normalized_source(function, live, types, &source)?,
                        carrier_type,
                        referent,
                        placed: super::placed_source(live, &source),
                    }
                }
                Source::Provider(origin) => {
                    let mut origin = origin.clone();
                    let target_operations::TargetReferenceOriginBase::Provider {
                        referent: promised,
                        access,
                        invocations,
                        ..
                    } = &mut origin.base
                    else {
                        return Err(invalid("provider output has no admitted origin"));
                    };
                    let projected = origin
                        .path
                        .iter()
                        .map(|segment| match segment {
                            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                                Some(segment.clone())
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| {
                            invalid("provider output has unresolved referent projection")
                        })?;
                    let StructuralTypeShape::Reference {
                        access: required, ..
                    } = types
                        .get(&carrier_type)
                        .ok_or_else(|| invalid("reference carrier is undeclared"))?
                        .shape
                    else {
                        return Err(invalid("provider output destination is not a reference"));
                    };
                    if leaf_type(types, *promised, &projected) != Some(referent)
                        || !access_covers(*access, required)
                    {
                        return Err(invalid(
                            "provider promise does not cover the destination reference",
                        ));
                    }
                    invocations.insert(0, (function.machine, operation));
                    ReferenceStorage {
                        root: ReferenceRoot::Alternatives(vec![origin]),
                        carrier_type,
                        referent,
                        placed: None,
                    }
                }
            };
            if leaf.carrier_type != carrier_type || leaf.referent != referent {
                return Err(invalid("source and destination reference types disagree"));
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
