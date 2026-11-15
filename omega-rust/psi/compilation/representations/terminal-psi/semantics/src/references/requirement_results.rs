//! A generic check form calls a requirement through a rigid implementation
//! parameter, and that requirement has no body. Its declared lifetime groups
//! therefore stand for the callee's origins: within a selected input, every
//! leaf carrying the selected lifetime remains a possible source
//! (lifetimes.md, "Returned views"). Each publishes as a within-region
//! origin in the requirement header's existing result column, so a caller
//! keeps the whole declared region borrowed instead of one exact projection
//! that no body chose.

use std::borrow::Cow;

use semantic_vocabulary::{MachineId, ReferenceOriginExtent, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineResult, DeclaredResultLifetime, MachineContract, ReferenceOriginProjection,
    ReferenceOriginSource, ReferenceResultShape, StructuralAccess, StructuralPathSegment,
    StructuralReferenceOrigin, StructuralReferenceResultSource, TerminalDynamicReferenceOrigin,
    TerminalDynamicReferenceRegion, TerminalModule,
};

/// The header column of a bodiless requirement: one within-region source per
/// declared input of each reference leaf's lifetime. A caller-chosen leaf
/// (no input carries its lifetime) has no admitted backing and is refused.
pub fn declared_reference_result_sources(
    module: &TerminalModule,
    contract: &MachineContract,
    groups: &[DeclaredResultLifetime],
) -> Result<Vec<StructuralReferenceResultSource>, &'static str> {
    super::validate_result_lifetime_groups(groups)?;
    let result = contract
        .result
        .structural()
        .ok_or("requirement reference result has no structural declaration")?;
    let leaves = crate::borrowed_result_leaves(
        module.structural_types.iter(),
        result.structural_type,
        result.multiplicity,
        groups.iter().map(|group| group.results.len()).sum(),
    )?;
    let mut mappings = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        if !matches!(leaf.kind, crate::BorrowedResultLeafKind::Reference(_)) {
            return Err("requirement view result needs its admitted completion resource");
        }
        let mut owners = groups
            .iter()
            .filter(|group| group.results.binary_search(&leaf.path).is_ok());
        let group = owners
            .next()
            .ok_or("requirement lifetime groups omit a reference result leaf")?;
        if owners.next().is_some() {
            return Err("requirement result leaf belongs to multiple declared lifetimes");
        }
        if group.inputs.is_empty() {
            return Err(
                "requirement caller-chosen result needs its admitted backing and retention",
            );
        }
        let mut sources = Vec::with_capacity(group.inputs.len());
        for input in &group.inputs {
            let region =
                super::selected_results::declared_region(module, contract, input, &leaf.path)?;
            let TerminalDynamicReferenceOrigin::Parameter { argument_index } = region.origin else {
                return Err("requirement lifetime input unexpectedly uses an invocation origin");
            };
            let parameter = usize::try_from(argument_index)
                .ok()
                .and_then(|position| contract.structural_parameters.get(position))
                .ok_or("requirement lifetime input names no public structural formal")?;
            sources.push(StructuralReferenceOrigin::Parameter(
                ReferenceOriginSource {
                    place: parameter.place,
                    path: region
                        .source_path
                        .into_iter()
                        .map(ReferenceOriginProjection::Exact)
                        .collect(),
                    extent: ReferenceOriginExtent::WithinRegion,
                    access: region.access,
                },
            ));
        }
        sources.sort();
        sources.dedup();
        mappings.push(StructuralReferenceResultSource {
            path: leaf.path,
            sources,
        });
    }
    mappings.sort();
    Ok(mappings)
}

/// A call's public result when its callee is a bodiless requirement. The
/// header column is the requirement's declared relation; there is no body
/// whose actual return could narrow it.
pub struct RequirementReferenceResult<'module> {
    contract: &'module MachineContract,
    mappings: &'module [StructuralReferenceResultSource],
    regions: Vec<TerminalDynamicReferenceRegion>,
}

/// `None` when `callee` has a body (its actual origins govern) or returns no
/// reference. A bodiless callee's header must be a complete within-region
/// relation over its own formals; anything else is refused, never adopted.
pub fn requirement_reference_result(
    module: &TerminalModule,
    callee: MachineId,
) -> Result<Option<RequirementReferenceResult<'_>>, &'static str> {
    let machine = module
        .machine(callee)
        .ok_or("requirement reference call has no callee declaration")?;
    if !machine.blocks.is_empty() {
        return Ok(None);
    }
    let Some(result) = machine.contract.result.structural().filter(|result| {
        terminal_psi::structural_type_contains_reference_by(
            |id| module.structural_type(id),
            result.structural_type,
        )
    }) else {
        return Ok(None);
    };
    let contract = &machine.contract;
    let mappings = result.reference_sources.as_slice();
    let leaves = crate::borrowed_result_leaves(
        module.structural_types.iter(),
        result.structural_type,
        result.multiplicity,
        mappings.len(),
    )?;
    if leaves.len() != mappings.len()
        || mappings.windows(2).any(|pair| pair[0].path >= pair[1].path)
        || leaves.iter().zip(mappings).any(|(leaf, mapping)| {
            leaf.path != mapping.path
                || !matches!(leaf.kind, crate::BorrowedResultLeafKind::Reference(_))
        })
    {
        return Err("requirement reference header has no complete declared result frontier");
    }
    let mut regions = Vec::new();
    for (leaf, mapping) in leaves.iter().zip(mappings) {
        if mapping.sources.is_empty() {
            return Err("requirement reference header omits its declared inputs");
        }
        for source in &mapping.sources {
            let source = source
                .parameter()
                .filter(|source| {
                    source.extent == ReferenceOriginExtent::WithinRegion
                        && source.access == leaf.access
                })
                .ok_or("requirement reference header is not a declared input region")?;
            let position = terminal_psi::structural_parameter_position(
                &contract.structural_parameters,
                source.place,
            )
            .ok_or("requirement reference header names no public formal")?;
            let source_path = source
                .path
                .iter()
                .map(|projection| match projection {
                    ReferenceOriginProjection::Exact(segment) => Ok(segment.clone()),
                    ReferenceOriginProjection::Element => {
                        Err("requirement reference header has no static input region")
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let parameter = &contract.structural_parameters[position];
            super::reference_origins::result_regions::validate_input_region(
                module.structural_types.iter(),
                parameter.structural_type,
                parameter.access,
                &source_path,
                source.access,
            )?;
            regions.push(TerminalDynamicReferenceRegion {
                path: mapping.path.clone(),
                origin: TerminalDynamicReferenceOrigin::Parameter {
                    argument_index: u32::try_from(position)
                        .map_err(|_| "requirement reference input ordinal exceeds u32")?,
                },
                source_path,
                access: source.access,
            });
        }
    }
    Ok(Some(RequirementReferenceResult {
        contract,
        mappings,
        regions,
    }))
}

impl RequirementReferenceResult<'_> {
    pub fn contract(&self) -> &MachineContract {
        self.contract
    }

    /// The declared relation is the callee's only origin column.
    pub fn sources(&self) -> &[StructuralReferenceResultSource] {
        self.mappings
    }
}

impl ReferenceResultShape for RequirementReferenceResult<'_> {
    fn result(&self) -> Cow<'_, BoundaryMachineResult> {
        Cow::Owned(super::selected_results::public_result(self.contract))
    }
    fn maximum_reference_leaves(&self) -> usize {
        self.mappings.len()
    }
    fn reference_regions<'a>(
        &'a self,
        path: &'a [StructuralPathSegment],
    ) -> impl Iterator<Item = Result<Cow<'a, TerminalDynamicReferenceRegion>, &'static str>> + 'a
    {
        self.regions
            .iter()
            .filter(move |region| region.path == path)
            .map(|region| Ok(Cow::Borrowed(region)))
    }
    fn structural_parameter_count(&self) -> usize {
        self.contract.structural_parameters.len()
    }
    fn structural_parameter(&self, index: usize) -> Option<(StructuralTypeId, StructuralAccess)> {
        self.contract
            .structural_parameters
            .get(index)
            .map(|parameter| (parameter.structural_type, parameter.access))
    }
}
