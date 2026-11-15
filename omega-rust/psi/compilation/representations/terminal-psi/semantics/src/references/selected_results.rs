//! Selected calls keep every backing source allowed by the public lifetime,
//! even when the implementing body always returns one input. This is a
//! borrowed query over the existing header and application, not another
//! published result or a source of backing permission.

use std::borrow::Cow;

use semantic_vocabulary::{ContractId, MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    DeclaredResultLifetime, MachineContract, ReferenceResultShape, StructuralAccess,
    StructuralPathSegment, StructuralTypeShape, TerminalDynamicReferenceOrigin,
    TerminalDynamicReferenceRegion, TerminalMachineResult, TerminalModule,
};

mod definition;

pub struct SelectedReferenceResult<'module> {
    module: &'module TerminalModule,
    contract: &'module MachineContract,
    groups: &'module [DeclaredResultLifetime],
    leaves: usize,
}

pub fn selected_reference_result(
    module: &TerminalModule,
    caller: MachineId,
    callee: MachineId,
    required: ContractId,
) -> Result<SelectedReferenceResult<'_>, &'static str> {
    let contract = module
        .invocation_machine_contract(callee, Some(required))
        .ok_or("selected reference call has no exact public header and body association")?;
    let dispatch = contract
        .selected_application
        .as_ref()
        .ok_or("selected reference call has no original public application")?;
    let applications = crate::closed_conformances::dispatch_applications(module);
    let application =
        crate::closed_conformances::dispatch_application(&applications, caller, dispatch, callee)?;
    let method = application
        .static_applications
        .iter()
        .find(|method| {
            method.public_requirement_identity == dispatch.public_requirement_identity
                && method.realization_identity == dispatch.realization_identity
                && method.source_callable_identity == dispatch.realization_callable_identity
                && method.callable == terminal_psi::TargetMachineCallable::Machine(callee)
        })
        .ok_or("selected reference call lost its exact declared method tuple")?;
    let groups = method
        .result_lifetimes
        .as_deref()
        .ok_or("selected reference result has an unresolved declared lifetime frontier")?;
    let schema = application
        .schemas
        .iter()
        .find(|schema| {
            schema.declaring_trait_identity == dispatch.declaring_trait_identity
                && schema.public_requirement_identity == dispatch.public_requirement_identity
                && schema.requirement_identity == dispatch.requirement_identity
                && schema.realization_identity == dispatch.realization_identity
        })
        .ok_or("selected reference result lost its original public definition")?;
    if definition::result_lifetimes(module, contract, schema)? != groups {
        return Err("selected result lifetimes differ from the original public definition");
    }
    let leaves = validate_result_lifetime_groups(groups)?;
    let selected = SelectedReferenceResult {
        module,
        contract,
        groups,
        leaves,
    };
    selected.validate()?;
    Ok(selected)
}

pub(crate) fn validate_result_lifetime_groups(
    groups: &[DeclaredResultLifetime],
) -> Result<usize, &'static str> {
    if groups.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("selected result lifetime groups are repeated or not canonical");
    }
    let path = |path: &[StructuralPathSegment]| {
        path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Referent
                    | StructuralPathSegment::AllElements
            )
        })
    };
    groups.iter().try_fold(0_usize, |count, group| {
        if group.results.is_empty()
            || group.inputs.windows(2).any(|pair| pair[0] >= pair[1])
            || group.results.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("selected result lifetime membership is repeated or not canonical");
        }
        if group.results.iter().any(|result| !path(result))
            || group.inputs.iter().any(|input| match input {
                BoundaryLifetimeDependency::Referent { access, .. } => {
                    *access == StructuralAccess::Owned
                }
                BoundaryLifetimeDependency::Stored {
                    access,
                    path: stored,
                    ..
                } => *access == StructuralAccess::Owned || !path(stored),
            })
        {
            return Err("selected result lifetime membership is not a static borrowed frontier");
        }
        count
            .checked_add(group.results.len())
            .ok_or("selected result lifetime frontier exceeds host positions")
    })
}

impl SelectedReferenceResult<'_> {
    pub fn contract(&self) -> &MachineContract {
        self.contract
    }

    /// The body may choose one public source, but cannot return storage that
    /// the public lifetime never permits. Compare actual formation separately
    /// from the conservative input union retained by callers.
    pub fn validate_actual_sources(
        &self,
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        mappings: &[terminal_psi::StructuralReferenceResultSource],
    ) -> Result<(), &'static str> {
        let result = self
            .contract
            .result
            .structural()
            .ok_or("selected reference body has no structural public result")?;
        let leaves = super::reference_origins::reference_leaves(
            self.module.structural_types.iter(),
            result.structural_type,
            mappings.len(),
        )
        .ok_or("selected reference body omits a complete actual result frontier")?;
        if leaves.len() != mappings.len()
            || mappings.windows(2).any(|pair| pair[0].path >= pair[1].path)
            || leaves
                .iter()
                .zip(mappings)
                .any(|(leaf, mapping)| leaf.path != mapping.path)
        {
            return Err("selected reference body has another actual result frontier");
        }
        for mapping in mappings {
            if mapping.sources.is_empty() {
                return Err("selected reference body omits actual backing sources");
            }
            for source in &mapping.sources {
                let actual = source
                    .parameter()
                    .ok_or("selected reference backing needs its admitted producer resource")?;
                let position =
                    terminal_psi::structural_parameter_position(parameters, actual.place).ok_or(
                        "selected reference body returns storage outside its formal roster",
                    )?;
                let root = parameters[position].structural_type;
                let mut covered = false;
                for region in self.reference_regions(&mapping.path) {
                    let region = region?;
                    if region_argument(&region)? != position {
                        continue;
                    }
                    let allowed = terminal_psi::ReferenceOriginSource {
                        place: actual.place,
                        path: region
                            .source_path
                            .iter()
                            .cloned()
                            .map(terminal_psi::ReferenceOriginProjection::Exact)
                            .collect(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                        access: region.access,
                    };
                    if super::reference_origins::reference_origin_is_covered(
                        self.module.structural_types.iter(),
                        root,
                        actual,
                        std::slice::from_ref(&allowed),
                    ) {
                        covered = true;
                        break;
                    }
                }
                if !covered {
                    return Err(
                        "selected implementation returns a reference outside its declared public lifetime",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), &'static str> {
        let Some(result) = self.contract.result.structural() else {
            return if self.groups.is_empty() {
                Ok(())
            } else {
                Err("nonstructural public result advertises borrowed lifetime leaves")
            };
        };
        // This column records a checked body's actual origins. A bodyless
        // requirement uses its independent declared lifetime groups instead;
        // copying the implementation's narrower row here is not authority.
        if !result.reference_sources.is_empty() {
            return Err("selected public result replaces declared lifetimes with body origins");
        }
        let leaves = crate::borrowed_result_leaves(
            self.module.structural_types.iter(),
            result.structural_type,
            result.multiplicity,
            self.leaves,
        )?;
        if leaves.len() != self.leaves {
            return Err(
                "selected lifetime groups do not cover the complete public result frontier",
            );
        }
        for leaf in leaves {
            let mut groups = self
                .groups
                .iter()
                .filter(|group| group.results.binary_search(&leaf.path).is_ok());
            let group = groups
                .next()
                .ok_or("selected lifetime groups omit a public result leaf")?;
            if groups.next().is_some() {
                return Err("selected result leaf belongs to multiple declared lifetimes");
            }
            if group.inputs.is_empty() {
                return Err(
                    "selected caller-chosen result needs its admitted backing and retention",
                );
            }
            for input in &group.inputs {
                let region = self.region(input, &leaf.path)?;
                super::reference_origins::result_regions::validate_input_region(
                    self.module.structural_types.iter(),
                    self.contract.structural_parameters[region_argument(&region)?].structural_type,
                    self.contract.structural_parameters[region_argument(&region)?].access,
                    &region.source_path,
                    region.access,
                )?;
            }
        }
        Ok(())
    }

    fn region(
        &self,
        input: &BoundaryLifetimeDependency,
        path: &[StructuralPathSegment],
    ) -> Result<TerminalDynamicReferenceRegion, &'static str> {
        let result = self
            .contract
            .result
            .structural()
            .ok_or("selected result lifetime has no structural result")?;
        let leaf = crate::runtime_structural_path_tip(
            self.module.structural_types.iter(),
            result.structural_type,
            path,
        )
        .ok_or("selected lifetime output path has no canonical borrowed carrier")?;
        let access = match leaf.shape {
            StructuralTypeShape::Reference { access, .. } => access,
            StructuralTypeShape::ElementView {
                access: Some(access),
                ..
            }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(access),
                }
                | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(access),
                    ..
                },
            ) => access,
            _ => return Err("selected lifetime output is not a declared borrowed descriptor"),
        };
        let (argument_index, mut source_path, supplied) = match input {
            BoundaryLifetimeDependency::Referent {
                argument_index,
                access,
            } => (*argument_index, Vec::new(), *access),
            BoundaryLifetimeDependency::Stored {
                argument_index,
                path,
                access,
            } => (*argument_index, path.clone(), *access),
        };
        let position = usize::try_from(argument_index)
            .map_err(|_| "selected lifetime input ordinal exceeds host positions")?;
        let parameter = self
            .contract
            .structural_parameters
            .get(position)
            .ok_or("selected lifetime input names no public structural formal")?;
        if !access.admits_referent(supplied) {
            return Err("selected reference result exceeds a possible input's declared access");
        }
        match input {
            BoundaryLifetimeDependency::Referent { .. } => {
                if parameter.access != supplied || parameter.access == StructuralAccess::Owned {
                    return Err("selected lifetime referent is not that public borrowed formal");
                }
            }
            BoundaryLifetimeDependency::Stored { .. } => {
                let source = crate::runtime_structural_path_tip(
                    self.module.structural_types.iter(),
                    parameter.structural_type,
                    &source_path,
                )
                .ok_or("selected lifetime stored input has no canonical carrier")?;
                match source.shape {
                    StructuralTypeShape::Reference { access, .. } if access == supplied => {
                        source_path.push(StructuralPathSegment::Referent);
                    }
                    StructuralTypeShape::ElementView {
                        access: Some(access),
                        ..
                    }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(access),
                        }
                        | terminal_psi::ByteSequenceCarrier::BoundedView {
                            access: Some(access),
                            ..
                        },
                    ) if access == supplied => {}
                    _ => {
                        return Err(
                            "selected lifetime stored input differs from its declared descriptor",
                        );
                    }
                }
            }
        }
        Ok(TerminalDynamicReferenceRegion {
            path: path.to_vec(),
            origin: TerminalDynamicReferenceOrigin::Parameter { argument_index },
            source_path,
            access,
        })
    }
}

fn region_argument(region: &TerminalDynamicReferenceRegion) -> Result<usize, &'static str> {
    let TerminalDynamicReferenceOrigin::Parameter { argument_index } = region.origin else {
        return Err("declared lifetime input unexpectedly uses an invocation origin");
    };
    usize::try_from(argument_index)
        .map_err(|_| "selected lifetime input ordinal exceeds host positions")
}

impl ReferenceResultShape for SelectedReferenceResult<'_> {
    fn result(&self) -> Cow<'_, BoundaryMachineResult> {
        Cow::Owned(match &self.contract.result {
            TerminalMachineResult::Unit => BoundaryMachineResult::Unit,
            TerminalMachineResult::Scalar(result) => {
                BoundaryMachineResult::Scalar(result.value_type())
            }
            TerminalMachineResult::Structural(result) => {
                BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                })
            }
        })
    }
    fn maximum_reference_leaves(&self) -> usize {
        self.leaves
    }
    fn reference_regions<'a>(
        &'a self,
        path: &'a [StructuralPathSegment],
    ) -> impl Iterator<Item = Result<Cow<'a, TerminalDynamicReferenceRegion>, &'static str>> + 'a
    {
        self.groups
            .iter()
            .filter(move |group| {
                group
                    .results
                    .binary_search_by(|known| known.as_slice().cmp(path))
                    .is_ok()
            })
            .flat_map(move |group| {
                group
                    .inputs
                    .iter()
                    .map(move |input| self.region(input, path).map(Cow::Owned))
            })
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
