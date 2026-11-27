use super::{ReferenceCallContract, ReferenceCallOrigin, ReferenceCallResult};
use crate::descriptors::descriptor_lifetimes::DescriptorPosition;
use crate::descriptors::descriptor_results::{
    self as descriptors, DescriptorAnalysis, DescriptorMachine, PayloadMembershipContract,
};
use crate::descriptors::row_source::DescriptorRowSource;
use semantic_vocabulary::{MachineId, PlaceId, ReferenceOriginExtent};
use terminal_psi::{
    Operation, ReferenceOriginProjection, StructuralAccess, StructuralOperationResult,
    StructuralPathQualification, StructuralPathSegment, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub struct ReferenceResultMemberships<'module, 'r> {
    module: &'module TerminalModule,
    rows: DescriptorRowSource<'r, 'module>,
    graph: std::cell::OnceCell<Vec<DescriptorMachine>>,
}

impl<'module, 'r> ReferenceResultMemberships<'module, 'r> {
    pub fn new(module: &'module TerminalModule) -> Self {
        Self::with_rows(DescriptorRowSource::new(module))
    }

    /// `new`, building the module's borrowed-storage rows with its shared
    /// analyses' machine lookup and type index.
    pub fn new_in(analyses: &'r crate::ModuleAnalyses<'module>) -> Self {
        Self::with_rows(DescriptorRowSource::of(analyses))
    }

    fn with_rows(rows: DescriptorRowSource<'r, 'module>) -> Self {
        Self {
            module: rows.module(),
            rows,
            graph: std::cell::OnceCell::new(),
        }
    }

    pub fn for_operation(
        &self,
        caller: &TerminalMachine,
        operation: &Operation,
    ) -> Result<Vec<StructuralPathQualification>, &'static str> {
        let (terminal_psi::call_structural_pair_pattern!(callee, structural_arguments, ..)) =
            &operation.kind
        else {
            return Ok(Vec::new());
        };
        let caller = self
            .module
            .machine(caller.id)
            .ok_or("reference call has no caller")?;
        let result = operation
            .result
            .structural()
            .ok_or("reference call has no result")?;
        let callee = self
            .module
            .machine(*callee)
            .ok_or("reference call has no callee")?;
        let public = crate::public_reference_result(
            self.module,
            caller.id,
            callee.id,
            operation.required_contract,
        )?;
        let contract = public
            .as_ref()
            .map_or(&callee.contract, |public| public.contract());
        let signature = contract
            .result
            .structural()
            .ok_or("reference callee has no result")?;
        if public.is_none() && signature.reference_sources.is_empty() {
            return Ok(Vec::new());
        }
        if result.structural_type != signature.structural_type
            || result.multiplicity != signature.multiplicity
        {
            return Err("reference call result disagrees with its public carrier");
        }
        let mut positions =
            caller.blocks.iter().flat_map(|block| {
                block.operations.iter().enumerate().filter_map(
                    move |(ordinal, candidate)| {
                        (candidate.id == operation.id && candidate == operation)
                            .then_some((block.id, ordinal))
                    },
                )
            });
        let (block, ordinal) = positions
            .next()
            .ok_or("reference call has no current-body cut")?;
        if positions.next().is_some() {
            return Err("reference call has ambiguous current-body cuts");
        }
        let position = (
            block,
            u32::try_from(ordinal).map_err(|_| "call ordinal overflows")?,
        );
        let mappings = if let Some(public) = &public {
            super::bind_public_reference_call_results(
                self.module.types.structural_types.iter(),
                public,
                structural_arguments,
            )?
        } else {
            super::bind_reference_call_results(
                self.module.types.structural_types.iter(),
                ReferenceCallContract::Machine {
                    parameters: &contract.structural_parameters,
                    sources: &signature.reference_sources,
                },
                structural_arguments,
            )?
        };
        preserve_call_memberships(
            &self.module.types.structural_types,
            &self.module.types.structural_domains,
            || {
                self.graph
                    .get_or_init(|| {
                        self.rows
                            .module_graph(descriptors::DescriptorSlotKind::BorrowedStorage)
                            .unwrap_or_default()
                    })
                    .as_slice()
            },
            caller.id,
            position,
            result,
            signature,
            &mappings,
            &|place| descriptors::terminal_payload_contract(caller, place),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn preserve_call_memberships<'input>(
    types: &'input [StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: impl Fn() -> &'input [DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    result: &StructuralOperationResult,
    declaration: &StructuralResultDeclaration,
    mappings: &[ReferenceCallResult],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    if result.structural_type != declaration.structural_type
        || result.multiplicity != declaration.multiplicity
    {
        return Err("preserved reference result has a different declared carrier");
    }
    let replay_source = |place| {
        if place == result.place {
            Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: declaration.structural_type,
                qualifications: declaration.qualifications.clone(),
                projected_qualifications: declaration.projected_qualifications.clone(),
            })
        } else {
            source(place)
        }
    };
    let mut replay = None;
    let mut retained = Vec::new();
    for (ordinal, mapping) in mappings.iter().enumerate() {
        if mappings[..ordinal]
            .iter()
            .any(|previous| previous.path == mapping.path)
        {
            return Err("returned reference membership repeats an output leaf");
        }
        let Some(leaf) = crate::runtime_structural_path_tip(
            types.iter(),
            result.structural_type,
            &mapping.path,
        ) else {
            return Err("returned reference membership has no typed leaf");
        };
        let StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        } = leaf.shape
        else {
            continue;
        };
        let mut common: Option<PayloadMembershipContract> = None;
        let mut complete = !mapping.sources.is_empty();
        for candidate in &mapping.sources {
            let selected = (|| {
                if candidate.extent != ReferenceOriginExtent::Projected {
                    return None;
                }
                let ReferenceCallOrigin::Parameter { source: actual, .. } =
                    &candidate.origin
                else {
                    return None;
                };
                let root = replay_source(actual.place)?;
                let mut path = actual.path.clone();
                path.extend(candidate.suffix.iter().map(|projection| match projection {
                    ReferenceOriginProjection::Exact(segment) => segment.clone(),
                    ReferenceOriginProjection::Element => {
                        StructuralPathSegment::AllElements
                    }
                }));
                if let Some((StructuralPathSegment::Referent, prefix)) = path.split_last()
                    && crate::qualification_path_carrier(types.iter(), root.carrier, prefix)
                        .and_then(|carrier| {
                            terminal_psi::find_structural_type(types, carrier)
                        })
                        .is_some_and(|row| {
                            !matches!(row.shape, StructuralTypeShape::Reference { .. })
                        })
                {
                    path.pop();
                }
                let query = descriptors::descriptor_contents_query(
                    types,
                    actual.place,
                    root.carrier,
                    &path,
                    None,
                )?;
                let (graph, analysis) = replay.get_or_insert_with(|| {
                    let graph = graph();
                    (
                        graph,
                        DescriptorAnalysis::with_borrowed_storage_slots(types, graph),
                    )
                });
                let targets = analysis.payload_targets(identity, position, &query)?;
                descriptors::payload_memberships(types, domains, targets, |root| {
                    descriptors::completed_payload_contract(
                        types,
                        domains,
                        graph,
                        identity,
                        position,
                        root,
                        &replay_source,
                    )
                    .ok()
                })?
                .presented(types, domains, referent)
            })();
            let Some(selected) = selected else {
                complete = false;
                break;
            };
            if let Some(common) = &mut common {
                common
                    .qualifications
                    .retain(|domain| selected.qualifications.contains(domain));
                common
                    .projected_qualifications
                    .retain(|row| selected.projected_qualifications.contains(row));
            } else {
                common = Some(selected);
            }
        }
        if complete && let Some(common) = common {
            let mut destination = mapping.path.clone();
            destination.push(StructuralPathSegment::Referent);
            StructuralPathQualification::append_under(
                &common.qualifications,
                &common.projected_qualifications,
                &destination,
                &mut retained,
            );
        }
    }
    retained.sort();
    retained.dedup();
    Ok(retained)
}

#[cfg(test)]
mod tests;
