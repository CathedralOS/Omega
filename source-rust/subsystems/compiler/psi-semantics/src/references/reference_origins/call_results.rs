use super::reference_origin_type;
use semantic_vocabulary::{ReferenceOriginExtent, StructuralTypeId};
use terminal_psi::TerminalDynamicReferenceOrigin;
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
    Callable(super::super::DeclaredReferenceResult<'declaration>),
}

pub enum ReferenceCallOrigin {
    Parameter {
        source: StructuralArgument,
        transfers_existing: bool,
    },
    Invocation {
        carrier_type: StructuralTypeId,
        path: Vec<ReferenceOriginProjection>,
        access: terminal_psi::StructuralAccess,
    },
}

pub struct ReferenceCallSource {
    pub origin: ReferenceCallOrigin,
    pub suffix: Vec<ReferenceOriginProjection>,
    pub endpoint: StructuralTypeId,
    pub extent: ReferenceOriginExtent,
}

pub struct ReferenceCallResult {
    pub path: Vec<StructuralPathSegment>,
    pub sources: Vec<ReferenceCallSource>,
}

impl ReferenceCallSource {
    /// The origin suffix a returned reference projects from its actual
    /// argument, or `None` when that storage cannot be the declared
    /// referent. `source_root` is the argument's resolved storage type.
    ///
    /// The returned reference keeps exactly the storage its argument lent.
    /// An argument may lend a scalar under another carrier: an exclusive
    /// loan of a restricted field stays a place loan, so its recast is not
    /// materialized before the call. The origin then records the presented
    /// view over the stored carrier, as an established reference view does,
    /// so later reads and writes still address the original backing.
    pub fn projected_origin_suffix<'types>(
        &self,
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        source_root: StructuralTypeId,
        expected: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> Option<std::borrow::Cow<'_, [ReferenceOriginProjection]>> {
        let endpoint = reference_origin_type(types.clone(), source_root, &self.suffix)?;
        if self.extent != ReferenceOriginExtent::Projected || endpoint == expected {
            return Some(std::borrow::Cow::Borrowed(&self.suffix));
        }
        let scalar = |identity| {
            crate::structural_declaration(types.clone(), identity)
                .filter(|declaration| declaration.scalar_restriction.is_none())
                .and_then(|declaration| match declaration.shape {
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => {
                        Some(scalar)
                    }
                    _ => None,
                })
        };
        if !crate::scalar_reference_view_admits(
            access,
            scalar(endpoint)?,
            scalar(expected)?,
        ) {
            return None;
        }
        let mut suffix = self.suffix.clone();
        suffix.push(ReferenceOriginProjection::Exact(
            StructuralPathSegment::RepresentationView {
                structural_type: expected,
                stored_scalar: None,
            },
        ));
        Some(std::borrow::Cow::Owned(suffix))
    }
}

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
                    let origin = match origin {
                        terminal_psi::StructuralReferenceOrigin::Parameter(origin) => {
                            origin
                        }
                        terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type,
                            path,
                            extent,
                            access,
                        } => {
                            let endpoint =
                                reference_origin_type(types.clone(), *carrier_type, path).ok_or(
                                    "invocation reference origin has no typed provider endpoint",
                                )?;
                            candidates.push(ReferenceCallSource {
                                origin: ReferenceCallOrigin::Invocation {
                                    carrier_type: *carrier_type,
                                    path: path.clone(),
                                    access: *access,
                                },
                                suffix: Vec::new(),
                                endpoint,
                                extent: *extent,
                            });
                            continue;
                        }
                    };
                    let argument_index = terminal_psi::structural_parameter_position(
                        parameters,
                        origin.place,
                    )
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
                        origin: ReferenceCallOrigin::Parameter {
                            source,
                            transfers_existing,
                        },
                        suffix: binding.suffix.to_vec(),
                        endpoint: binding.endpoint,
                        extent: binding.extent,
                    });
                }
                results.push(ReferenceCallResult {
                    path: mapping.path.clone(),
                    sources: candidates,
                });
            }
        }
        ReferenceCallContract::Requirement(slot) => {
            return bind_public_reference_call_results(types, slot, arguments);
        }
        ReferenceCallContract::Callable(contract) => {
            return bind_public_reference_call_results(types, &contract, arguments);
        }
    }
    Ok(results)
}

pub fn bind_public_reference_call_results<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    slot: &impl terminal_psi::ReferenceResultShape,
    arguments: &[StructuralArgument],
) -> Result<Vec<ReferenceCallResult>, &'static str> {
    let mut results: Vec<ReferenceCallResult> = Vec::new();
    let declared_result = slot.result();
    let leaves = match declared_result.as_ref() {
        terminal_psi::BoundaryMachineResult::Structural(result) => {
            super::borrowed_result_leaves(
                types.clone(),
                result.structural_type,
                result.multiplicity,
                slot.maximum_reference_leaves(),
            )?
        }
        _ => Vec::new(),
    };
    for leaf in leaves
        .iter()
        .filter(|leaf| matches!(leaf.kind, super::BorrowedResultLeafKind::Reference(_)))
    {
        for region in slot.reference_regions(&leaf.path) {
            let region = region?;
            let index = match region.origin {
                TerminalDynamicReferenceOrigin::Parameter { argument_index } => {
                    argument_index
                }
                TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                    let path = region
                        .source_path
                        .iter()
                        .cloned()
                        .map(ReferenceOriginProjection::Exact)
                        .collect::<Vec<_>>();
                    let endpoint =
                        reference_origin_type(types.clone(), carrier_type, &path).ok_or(
                            "dynamic invocation reference origin has no typed endpoint",
                        )?;
                    let candidate = ReferenceCallSource {
                        origin: ReferenceCallOrigin::Invocation {
                            carrier_type,
                            path,
                            access: region.access,
                        },
                        suffix: Vec::new(),
                        endpoint,
                        extent: ReferenceOriginExtent::Projected,
                    };
                    if let Some(result) =
                        results.iter_mut().find(|result| result.path == region.path)
                    {
                        result.sources.push(candidate);
                    } else {
                        results.push(ReferenceCallResult {
                            path: region.path.clone(),
                            sources: vec![candidate],
                        });
                    }
                    continue;
                }
            };
            let argument_index = usize::try_from(index)
                .map_err(|_| "dynamic returned region input ordinal overflows")?;
            let (parameter_type, _) = slot
                .structural_parameter(argument_index)
                .ok_or("dynamic returned region names no formal input")?;
            let origin_path = region
                .source_path
                .iter()
                .cloned()
                .map(ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>();
            let endpoint =
                reference_origin_type(types.clone(), parameter_type, &origin_path)
                    .ok_or("dynamic returned region has no typed input endpoint")?;
            let source_path = crate::reference_source_storage_path(
                types.clone(),
                parameter_type,
                &region.source_path,
            );
            let mut source = arguments
                .get(argument_index)
                .ok_or("dynamic returned region has no actual structural argument")?
                .clone();
            source.path.extend_from_slice(source_path);
            source.access = region.access;
            let candidate = ReferenceCallSource {
                origin: ReferenceCallOrigin::Parameter {
                    source,
                    transfers_existing: false,
                },
                suffix: Vec::new(),
                endpoint,
                extent: ReferenceOriginExtent::WithinRegion,
            };
            if let Some(result) =
                results.iter_mut().find(|result| result.path == region.path)
            {
                result.sources.push(candidate);
            } else {
                results.push(ReferenceCallResult {
                    path: region.path.clone(),
                    sources: vec![candidate],
                });
            }
        }
    }
    Ok(results)
}
