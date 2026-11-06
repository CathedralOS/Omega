//! Export permission occurrences and their immediate parents, keeping equal
//! pointer origins separate from the identity of a transported loan.
use super::*;

pub(super) fn output_graph(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    entry: &[LiveReference],
    slots: &[ReferenceOutputSlot],
    exit: &ReferenceOutputExit,
) -> Result<Vec<ReferenceOutputNode>, &'static str> {
    let mut current = exit.returned.iter().chain(&exit.stored).collect::<Vec<_>>();
    current.sort_by(|left, right| left.identity.cmp(&right.identity));
    current.dedup_by(|left, right| left.identity == right.identity);
    let mut entry_order = (0..entry.len()).collect::<Vec<_>>();
    entry_order.sort_by(|left, right| entry[*left].identity.cmp(&entry[*right].identity));
    let entry_index = |identity: &ReferenceOccurrenceIdentity| {
        entry_order
            .binary_search_by(|index| entry[*index].identity.cmp(identity))
            .ok()
            .map(|position| entry_order[position])
    };
    let mut returned_order = exit.returned.iter().collect::<Vec<_>>();
    returned_order.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut parameter_order = (0..function.structural_parameters.len()).collect::<Vec<_>>();
    parameter_order.sort_by_key(|index| function.structural_parameters[*index].place);
    let mut slot_order = slots.iter().collect::<Vec<_>>();
    slot_order
        .sort_by(|left, right| (left.parameter, &left.path).cmp(&(right.parameter, &right.path)));
    let mut origin_order = entry
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| match &reference.root {
            ReferenceOrigin::IngressLeaf(origin) => Some((origin, index)),
            _ => None,
        })
        .collect::<Vec<_>>();
    origin_order
        .sort_by(|left, right| (left.0.place, &left.0.path).cmp(&(right.0.place, &right.0.path)));
    let mut graph = Vec::new();
    let mut positions = vec![None; current.len()];
    let mut active = vec![false; current.len()];
    #[allow(clippy::too_many_arguments)]
    fn visit(
        index: usize,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        entry: &[LiveReference],
        current: &[&LiveReference],
        entry_index: &impl Fn(&ReferenceOccurrenceIdentity) -> Option<usize>,
        returned_order: &[&LiveReference],
        parameter_order: &[usize],
        slot_order: &[&ReferenceOutputSlot],
        origin_order: &[(&ReferenceOriginIdentity, usize)],
        graph: &mut Vec<ReferenceOutputNode>,
        positions: &mut [Option<usize>],
        active: &mut [bool],
    ) -> Result<usize, &'static str> {
        if let Some(position) = positions[index] {
            return Ok(position);
        }
        if active[index] {
            return Err("stored reference output ancestry is cyclic");
        }
        active[index] = true;
        let reference = current[index];
        let existing = entry_index(&reference.identity).filter(|position| {
            entry[*position].root == reference.root && entry[*position].parent == reference.parent
        });
        let returned = returned_order
            .binary_search_by(|candidate| candidate.identity.cmp(&reference.identity))
            .is_ok_and(|index| {
                returned_order[index].carrier == reference.carrier
                    && returned_order[index].carrier_path == reference.carrier_path
            });
        let (residence, root_type) = if returned {
            (
                ReferenceOutputResidence::Returned {
                    path: reference.carrier_path.clone(),
                },
                function
                    .result
                    .structural()
                    .ok_or("returned reference has no structural result")?
                    .structural_type,
            )
        } else {
            let parameter = parameter_order
                .binary_search_by_key(&reference.carrier, |index| {
                    function.structural_parameters[*index].place
                })
                .map(|position| parameter_order[position])
                .map_err(|_| "a stored output still depends on an unexported local carrier")?;
            let root_type = function.structural_parameters[parameter].structural_type;
            let writable = slot_order
                .binary_search_by(|slot| {
                    (slot.parameter, slot.path.as_slice())
                        .cmp(&(parameter, reference.carrier_path.as_slice()))
                })
                .is_ok();
            let residence = if writable {
                ReferenceOutputResidence::Parameter {
                    parameter,
                    path: reference.carrier_path.clone(),
                }
            } else {
                ReferenceOutputResidence::Entry(
                    existing.ok_or("a read-only input changed its reference occurrence")?,
                )
            };
            (residence, root_type)
        };
        let reference_type = crate::unit_validation::references::access_path_type(
            types,
            root_type,
            &reference.carrier_path,
        )
        .ok_or("an exported reference has no typed receiving slot")?;
        let supply = if let Some(entry) = existing {
            ReferenceOutputSupply::Entry(entry)
        } else {
            let origins = output_origins(function, origin_order, &reference.root)?;
            let parent = match &reference.parent {
                ReferenceParent::Root(root) => {
                    ReferenceOutputParent::Root(output_origins(function, origin_order, root)?)
                }
                ReferenceParent::Reference(identity) => {
                    if let Ok(parent) =
                        current.binary_search_by(|candidate| candidate.identity.cmp(identity))
                    {
                        ReferenceOutputParent::Node(visit(
                            parent,
                            function,
                            types,
                            entry,
                            current,
                            entry_index,
                            returned_order,
                            parameter_order,
                            slot_order,
                            origin_order,
                            graph,
                            positions,
                            active,
                        )?)
                    } else {
                        ReferenceOutputParent::Entry(
                            entry_index(identity)
                                .ok_or("a stored output lost its captured parent generation")?,
                        )
                    }
                }
            };
            ReferenceOutputSupply::Constructed { origins, parent }
        };
        let position = graph.len();
        graph.push(ReferenceOutputNode {
            residence,
            reference_type,
            supply,
        });
        positions[index] = Some(position);
        active[index] = false;
        Ok(position)
    }
    for index in 0..current.len() {
        visit(
            index,
            function,
            types,
            entry,
            &current,
            &entry_index,
            &returned_order,
            &parameter_order,
            &slot_order,
            &origin_order,
            &mut graph,
            &mut positions,
            &mut active,
        )?;
    }
    Ok(graph)
}

fn output_origins(
    function: &PsiOptimizationFunction,
    origin_order: &[(&ReferenceOriginIdentity, usize)],
    root: &ReferenceOrigin,
) -> Result<Vec<ReferenceOutputOrigin>, &'static str> {
    fn ingress(
        function: &PsiOptimizationFunction,
        origin_order: &[(&ReferenceOriginIdentity, usize)],
        origin: &ReferenceOriginIdentity,
    ) -> Result<ReferenceOutputOrigin, &'static str> {
        for prefix in (0..=origin.path.len()).rev() {
            let key = (origin.place, &origin.path[..prefix]);
            let first = origin_order
                .partition_point(|(incoming, _)| (incoming.place, incoming.path.as_slice()) < key);
            let after = origin_order
                .partition_point(|(incoming, _)| (incoming.place, incoming.path.as_slice()) <= key);
            if after - first > 1 {
                return Err("an output origin has ambiguous entry anchors");
            }
            if after > first {
                return Ok(ReferenceOutputOrigin::Entry {
                    entry: origin_order[first].1,
                    suffix: origin.path[prefix..].to_vec(),
                    extent: origin.extent,
                });
            }
        }
        let parameter = function
            .structural_parameters
            .iter()
            .position(|parameter| {
                parameter.place == origin.place && parameter.access != StructuralAccess::Owned
            })
            .ok_or("an output origin has no external entry storage")?;
        Ok(ReferenceOutputOrigin::Storage {
            parameter,
            path: origin.path.clone(),
            extent: origin.extent,
        })
    }
    match root {
        ReferenceOrigin::Primitive(place) => {
            let parameter = function
                .structural_parameters
                .iter()
                .position(|parameter| {
                    parameter.place == *place && parameter.access != StructuralAccess::Owned
                })
                .ok_or("an output refers to activation-local storage")?;
            Ok(vec![ReferenceOutputOrigin::Storage {
                parameter,
                path: Vec::new(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            }])
        }
        ReferenceOrigin::IngressLeaf(origin) => Ok(vec![ingress(function, origin_order, origin)?]),
        ReferenceOrigin::Alternatives { origins, .. } => origins
            .iter()
            .map(|origin| ingress(function, origin_order, origin))
            .collect(),
        ReferenceOrigin::Invocation { .. } => {
            Err("an output has no represented completed-call anchor")
        }
    }
}

pub(super) fn external_root(
    function: &PsiOptimizationFunction,
    entry_roots: &[&ReferenceOrigin],
    root: &ReferenceOrigin,
) -> bool {
    if entry_roots.binary_search(&root).is_ok() {
        return true;
    }
    match root {
        ReferenceOrigin::Primitive(place) => is_entry_storage(function, *place),
        ReferenceOrigin::IngressLeaf(origin) => is_entry_storage(function, origin.place),
        ReferenceOrigin::Alternatives { origins, .. } => {
            !origins.is_empty()
                && origins
                    .iter()
                    .all(|origin| is_entry_storage(function, origin.place))
        }
        ReferenceOrigin::Invocation { .. } => false,
    }
}

pub(super) fn is_entry_storage(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function
        .structural_parameter(place)
        .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
}
