use super::*;

/// Preserve the same entry-argument sources that ordinary body replay proved
/// for each result slot. A worker can return those loans after its owner has
/// parked and resumed in other callers. Preparation only takes custody.
pub(super) fn prepared_worker_summary(
    target: &DescriptorInvocationTarget,
    body: &Summary,
) -> Option<Summary> {
    let DescriptorInvocationTarget::PrepareWorker {
        results, contents, ..
    } = target
    else {
        return None;
    };
    let mut contents = contents.clone();
    let packet = contents
        .iter_mut()
        .flat_map(|slot| &mut slot.origins)
        .find_map(|origin| {
            let DescriptorStoredOrigin::ProducedContents(owner) = origin else {
                return None;
            };
            owner.activation_backing.as_mut()
        })?;
    let slots = body
        .result
        .slots()
        .into_iter()
        .flatten()
        .chain(body.result.contents());
    for slot in slots {
        let binding = packet.worker_result.get_mut(&slot.path)?;
        for origin in &slot.origins {
            // These denote actual entry values, or independent literal/empty
            // storage. Other origins can be created or modified while running.
            match origin {
                DescriptorStoredOrigin::IncomingSlot { position: 0, .. }
                | DescriptorStoredOrigin::Literal { .. }
                | DescriptorStoredOrigin::Empty => binding.push(origin.clone()),
                _ => return None,
            }
        }
    }
    if packet.worker_result.values().any(Vec::is_empty) || !body.result.unknown().is_empty() {
        return None;
    }
    Some(Summary {
        result: ResultObservations::Observed {
            slots: Some(results.clone()),
            contents,
            unknown: Vec::new(),
        },
        result_cases: Default::default(),
        effects: EffectObservations::Observed(Vec::new()),
        invalidates_contents: false,
        storage_frame: StorageFrame::unknown(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BoundaryMachineId, OperationId};

    #[test]
    fn only_the_selected_workers_entry_loans_can_be_returned_by_preparation() {
        let worker = MachineId::new(1).unwrap();
        let value_path = Vec::new();
        let owner = ProducedContents {
            producer: DescriptorInvocationSite {
                machine: MachineId::new(2).unwrap(),
                owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
            },
            path: vec![DescriptorPathSegment::UnknownContents],
            structural_type: StructuralTypeId::new(1).unwrap(),
            lifetime_dependencies: Vec::new(),
            activation_backing: Some(ActivationBacking {
                preparation: BoundaryMachineId::new(1).unwrap(),
                sources: vec![],
                worker_result: [(value_path.clone(), Vec::new())].into(),
            }),
        };
        let target = DescriptorInvocationTarget::PrepareWorker {
            worker,
            results: Vec::new(),
            contents: vec![DescriptorResultSlot {
                path: owner.path.clone(),
                origins: vec![DescriptorStoredOrigin::ProducedContents(owner)],
            }],
        };
        let body = |position| Summary {
            result: ResultObservations::Observed {
                slots: Some(vec![DescriptorResultSlot {
                    path: value_path.clone(),
                    origins: vec![DescriptorStoredOrigin::IncomingSlot {
                        position,
                        path: vec![],
                    }],
                }]),
                contents: vec![],
                unknown: vec![],
            },
            result_cases: Default::default(),
            effects: EffectObservations::Incomplete {
                observed: vec![],
                unobserved: None,
            },
            invalidates_contents: true,
            storage_frame: StorageFrame::unknown(),
        };
        let summary = prepared_worker_summary(&target, &body(0)).unwrap();
        let [result] = summary.result.contents() else {
            panic!("one prepared owner")
        };
        let [DescriptorStoredOrigin::ProducedContents(owner)] = result.origins.as_slice() else {
            panic!("owner keeps its inputs")
        };
        let sources = &owner.activation_backing.as_ref().unwrap().worker_result[&value_path];
        assert!(matches!(
            sources.as_slice(),
            [DescriptorStoredOrigin::IncomingSlot { position: 0, .. }]
        ));
        assert!(summary.effects.observed().unwrap().is_empty());
        assert!(!summary.invalidates_contents);
        assert!(prepared_worker_summary(&target, &body(1)).is_none());
    }
}
