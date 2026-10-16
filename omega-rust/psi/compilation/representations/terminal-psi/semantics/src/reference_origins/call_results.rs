//! Bind returned provenance to an invocation's actual structural arguments.
//! Direct declarations name private formal places; requirements name public
//! input ordinals. Normalize that distinction here, before ordinary custody
//! transports the actual carrier. Neither route selects a runtime address.

use semantic_vocabulary::{ReferenceOriginExtent, StructuralTypeId};
use terminal_psi::{
    ReferenceOriginProjection, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralReferenceResultSource, StructuralTypeDeclaration,
    TerminalDynamicSlot,
};

pub enum ReferenceCallContract<'declaration> {
    Machine {
        parameters: &'declaration [StructuralParameterDeclaration],
        sources: &'declaration [StructuralReferenceResultSource],
    },
    Requirement(&'declaration TerminalDynamicSlot),
}

pub struct ReferenceCallSource {
    pub source: StructuralArgument,
    pub suffix: Vec<ReferenceOriginProjection>,
    pub endpoint: StructuralTypeId,
    pub extent: ReferenceOriginExtent,
    pub transfers_existing: bool,
}

pub struct ReferenceCallResult {
    pub path: Vec<StructuralPathSegment>,
    pub sources: Vec<ReferenceCallSource>,
}

/// Declaration binding only. Call admission establishes actual access and type
/// compatibility; completion still checks the returned descriptor's formation,
/// original backing and parent disposition.
pub fn bind_reference_call_results<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    contract: ReferenceCallContract<'_>,
    arguments: &[StructuralArgument],
) -> Result<Vec<ReferenceCallResult>, &'static str> {
    let mut results = Vec::new();
    match contract {
        ReferenceCallContract::Machine {
            parameters,
            sources,
        } => {
            for mapping in sources {
                let mut candidates = Vec::with_capacity(mapping.sources.len());
                for origin in &mapping.sources {
                    let argument_index =
                        terminal_psi::structural_parameter_position(parameters, origin.place)
                            .ok_or("returned origin names no formal input")?;
                    let parameter = &parameters[argument_index];
                    let binding = super::bind_reference_origin(
                        types.clone(),
                        parameter.structural_type,
                        origin,
                    )?;
                    let transfers_existing = binding.transfers_existing(
                        types.clone(),
                        parameter.structural_type,
                        parameter.access,
                    );
                    let mut source = arguments
                        .get(argument_index)
                        .ok_or("returned origin has no actual structural argument")?
                        .clone();
                    source.path.extend(binding.source.path);
                    source.access = origin.access;
                    candidates.push(ReferenceCallSource {
                        source,
                        suffix: binding.suffix.to_vec(),
                        endpoint: binding.endpoint,
                        extent: binding.extent,
                        transfers_existing,
                    });
                }
                results.push(ReferenceCallResult {
                    path: mapping.path.clone(),
                    sources: candidates,
                });
            }
        }
        ReferenceCallContract::Requirement(slot) => {
            let leaves = match &slot.result {
                terminal_psi::BoundaryMachineResult::Structural(result) => {
                    super::borrowed_result_leaves(
                        types.clone(),
                        result.structural_type,
                        result.multiplicity,
                        slot.result_reference_regions.len(),
                    )?
                }
                _ => Vec::new(),
            };
            for region in &slot.result_reference_regions {
                // Views carry pointer/extent provenance through descriptor
                // replay. They must not manufacture a pointer-reference loan.
                if !leaves.iter().any(|leaf| {
                    leaf.path == region.path
                        && matches!(leaf.kind, super::BorrowedResultLeafKind::Reference(_))
                }) {
                    continue;
                }
                let argument_index = usize::try_from(region.argument_index)
                    .map_err(|_| "dynamic returned region input ordinal overflows")?;
                let parameter = slot
                    .structural_parameters
                    .get(argument_index)
                    .ok_or("dynamic returned region names no formal input")?;
                let origin_path = region
                    .source_path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>();
                let endpoint = super::reference_origin_type(
                    types.clone(),
                    parameter.structural_type,
                    &origin_path,
                )
                .ok_or("dynamic returned region has no typed input endpoint")?;
                let source_path = crate::reference_source_storage_path(
                    types.clone(),
                    parameter.structural_type,
                    &region.source_path,
                );
                let mut source = arguments
                    .get(argument_index)
                    .ok_or("dynamic returned region has no actual structural argument")?
                    .clone();
                source.path.extend_from_slice(source_path);
                source.access = region.access;
                let candidate = ReferenceCallSource {
                    source,
                    suffix: Vec::new(),
                    endpoint,
                    extent: ReferenceOriginExtent::WithinRegion,
                    transfers_existing: false,
                };
                if let Some(result) = results.iter_mut().find(|result| result.path == region.path) {
                    result.sources.push(candidate);
                } else {
                    results.push(ReferenceCallResult {
                        path: region.path.clone(),
                        sources: vec![candidate],
                    });
                }
            }
        }
    }
    Ok(results)
}
