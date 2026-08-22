use super::*;

pub(super) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), ModuleError> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &module.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralType(declaration.id));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
        }
        if let StructuralTypeShape::Record { fields } = &declaration.shape {
            let mut field_ids = BTreeSet::new();
            let mut field_names = BTreeSet::new();
            for field in fields {
                if !field_ids.insert(field.id)
                    || field.identity.is_empty()
                    || !field_names.insert(field.identity.as_str())
                {
                    return Err(ModuleError::InvalidStructuralFieldIdentity {
                        structural_type: declaration.id,
                        field: field.id,
                    });
                }
                match &field.field_type {
                    StructuralFieldType::Erased { type_identity }
                        if !field.relevance.is_erased() || type_identity.is_empty() =>
                    {
                        return Err(ModuleError::InvalidErasedStructuralField {
                            structural_type: declaration.id,
                            field: field.id,
                        });
                    }
                    StructuralFieldType::Scalar(_) | StructuralFieldType::Structural(_)
                        if field.relevance.is_erased() =>
                    {
                        return Err(ModuleError::InvalidErasedStructuralField {
                            structural_type: declaration.id,
                            field: field.id,
                        });
                    }
                    _ => {}
                }
            }
        } else if let StructuralTypeShape::Sum { cases } = &declaration.shape {
            if cases.is_empty() {
                return Err(ModuleError::EmptyStructuralSum(declaration.id));
            }
            let mut case_ids = BTreeSet::new();
            let mut case_names = BTreeSet::new();
            for case in cases {
                if !case_ids.insert(case.id)
                    || case.identity.is_empty()
                    || !case_names.insert(case.identity.as_str())
                {
                    return Err(ModuleError::InvalidStructuralCaseIdentity {
                        structural_type: declaration.id,
                        case: case.id,
                    });
                }
                let mut field_ids = BTreeSet::new();
                let mut field_names = BTreeSet::new();
                for field in &case.fields {
                    if !field_ids.insert(field.id)
                        || field.identity.is_empty()
                        || !field_names.insert(field.identity.as_str())
                    {
                        return Err(ModuleError::InvalidStructuralFieldIdentity {
                            structural_type: declaration.id,
                            field: field.id,
                        });
                    }
                    match &field.field_type {
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_empty() =>
                        {
                            return Err(ModuleError::InvalidErasedStructuralField {
                                structural_type: declaration.id,
                                field: field.id,
                            });
                        }
                        StructuralFieldType::Scalar(_) | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return Err(ModuleError::InvalidErasedStructuralField {
                                structural_type: declaration.id,
                                field: field.id,
                            });
                        }
                        _ => {}
                    }
                }
            }
        } else if matches!(
            declaration.shape,
            StructuralTypeShape::FixedArray { length: 0, .. }
        ) {
            return Err(ModuleError::InvalidStructuralArrayLength(declaration.id));
        }
    }
    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type
                        && !types.contains_key(target)
                    {
                        return Err(ModuleError::UnknownStructuralType(*target));
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                if !types.contains_key(element) {
                    return Err(ModuleError::UnknownStructuralType(*element));
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type
                        && !types.contains_key(target)
                    {
                        return Err(ModuleError::UnknownStructuralType(*target));
                    }
                }
            }
        }
    }
    validate_structural_type_graph(&types)?;

    let mut domains = BTreeMap::new();
    let mut domain_names = BTreeSet::new();
    for declaration in &module.structural_domains {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralDomain(declaration.id));
        }
        if declaration.identity.is_empty() || !domain_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidStructuralDomainIdentity(declaration.id));
        }
        if !types.contains_key(&declaration.carrier) {
            return Err(ModuleError::UnknownStructuralType(declaration.carrier));
        }
    }

    let mut services = BTreeMap::new();
    let mut service_names = BTreeSet::new();
    for declaration in &module.services {
        if services.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateService(declaration.id));
        }
        if declaration.identity.is_empty() || !service_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidServiceIdentity(declaration.id));
        }
    }
    for declaration in &module.services {
        let mut parents = BTreeSet::new();
        for parent in &declaration.parents {
            if *parent == declaration.id
                || !parents.insert(*parent)
                || !services.contains_key(parent)
            {
                return Err(ModuleError::InvalidServiceParent {
                    service: declaration.id,
                    parent: *parent,
                });
            }
        }
        if declaration
            .parents
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalServiceParents(declaration.id));
        }
    }
    validate_service_graph(&services)?;

    validate_service_ceiling(
        &module.root_service_reach.concrete,
        &services,
        ServiceCeilingOwner::RootConcrete,
    )?;

    let mut installation_requirements = BTreeSet::new();
    for (index, dependency) in module
        .root_service_reach
        .installation_dependencies
        .iter()
        .enumerate()
    {
        if dependency.requirement_identity.is_empty()
            || !installation_requirements.insert(dependency.requirement_identity.as_str())
        {
            return Err(ModuleError::InvalidInstallationReachDependency(index));
        }
        validate_service_ceiling(
            &dependency.upper_bound,
            &services,
            ServiceCeilingOwner::InstallationReach(index),
        )?;
    }
    if module
        .root_service_reach
        .installation_dependencies
        .windows(2)
        .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
    {
        return Err(ModuleError::NonCanonicalInstallationReachDependencies);
    }

    let mut boundary_ids = BTreeSet::new();
    let mut boundary_names = BTreeSet::new();
    for boundary in &module.boundary_machines {
        if !boundary_ids.insert(boundary.id) {
            return Err(ModuleError::DuplicateBoundaryMachine(boundary.id));
        }
        if boundary.identity.is_empty() || !boundary_names.insert(boundary.identity.as_str()) {
            return Err(ModuleError::InvalidBoundaryMachineIdentity(boundary.id));
        }
        validate_attachment(boundary.id, boundary.attachment, &types)?;
        validate_structural_signature(
            &boundary.structural_parameters,
            boundary.attachment,
            &types,
            &domains,
            StructuralSignatureOwner::Boundary(boundary.id),
        )?;
        validate_service_ceiling(
            &boundary.published_service_ceiling,
            &services,
            ServiceCeilingOwner::Boundary(boundary.id),
        )?;
        let mut requirements = BTreeSet::new();
        for requirement in &boundary.requires {
            if !requirements.insert(*requirement) {
                return Err(ModuleError::DuplicateBoundaryRequirement {
                    boundary: boundary.id,
                    argument_index: requirement.argument_index,
                    domain: requirement.domain,
                });
            }
            let Some(parameter) = boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
            else {
                return Err(ModuleError::BoundaryRequirementArgumentOutOfRange {
                    boundary: boundary.id,
                    argument_index: requirement.argument_index,
                });
            };
            let Some(domain) = domains.get(&requirement.domain) else {
                return Err(ModuleError::UnknownStructuralDomain(requirement.domain));
            };
            if domain.carrier != parameter.structural_type {
                return Err(ModuleError::StructuralDomainCarrierMismatch {
                    domain: domain.id,
                    expected: parameter.structural_type,
                    actual: domain.carrier,
                });
            }
        }
        if boundary.requires.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalBoundaryRequirements(boundary.id));
        }
    }

    if let Some(pair) = module.provider_candidates.windows(2).find(|pair| {
        (
            pair[0].boundary,
            pair[0].provider_identity.as_str(),
            pair[0].candidate_identity.as_str(),
            pair[0].candidate,
        ) >= (
            pair[1].boundary,
            pair[1].provider_identity.as_str(),
            pair[1].candidate_identity.as_str(),
            pair[1].candidate,
        )
    }) {
        let row = &pair[1];
        return Err(ModuleError::InvalidProviderCandidate {
            boundary: row.boundary,
            candidate: row.candidate,
        });
    }
    let mut provider_subjects = BTreeSet::new();
    let mut requirement_identities = BTreeMap::<BoundaryMachineId, &str>::new();
    for row in &module.provider_candidates {
        let Some(boundary) = module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.id == row.boundary)
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        let Some(candidate) = module
            .machines
            .iter()
            .find(|candidate| candidate.id == row.candidate)
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        if row.requirement_identity.is_empty()
            || row.provider_identity.is_empty()
            || row.candidate_identity.is_empty()
            || boundary.identity != row.requirement_identity
            || !provider_subjects.insert((
                row.boundary,
                row.provider_identity.as_str(),
                row.candidate_identity.as_str(),
                row.candidate,
            ))
            || requirement_identities
                .insert(row.boundary, row.requirement_identity.as_str())
                .is_some_and(|identity| identity != row.requirement_identity)
        {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        }
        let Some(attachment) = candidate
            .attachment
            .and_then(|attachment| types.get(&attachment))
        else {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        };
        let boundary_signature = boundary
            .structural_parameters
            .iter()
            .map(provider_signature_parameter)
            .collect::<Vec<_>>();
        let candidate_signature = candidate
            .structural_parameters
            .iter()
            .map(provider_signature_parameter)
            .collect::<Vec<_>>();
        let expected_positions = (0..boundary_signature.len())
            .map(|index| psi_terminal::ProviderParameterRefinement {
                boundary_index: u32::try_from(index).unwrap_or(u32::MAX),
                candidate_index: u32::try_from(index).unwrap_or(u32::MAX),
            })
            .collect::<Vec<_>>();
        if attachment.identity.is_empty()
            || !boundary_signature.is_empty()
            || !boundary.scalar_parameters.is_empty()
            || boundary.result.is_some()
            || !candidate.parameters.is_empty()
            || candidate.result != TerminalMachineResult::Unit
            || row.signature.parameters != boundary_signature
            || row.signature.parameters != candidate_signature
            || row.refinement.positional_parameters != expected_positions
            || row.refinement.required_domains != boundary.requires
            || row.refinement.realized_service_ceiling != candidate.published_service_ceiling
            || row
                .refinement
                .realized_service_ceiling
                .iter()
                .any(|service| !boundary.published_service_ceiling.contains(service))
        {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: row.boundary,
                candidate: row.candidate,
            });
        }
    }

    for machine in &module.machines {
        validate_attachment(machine.id, machine.attachment, &types)?;
        validate_structural_signature(
            &machine.structural_parameters,
            machine.attachment,
            &types,
            &domains,
            StructuralSignatureOwner::Machine(machine.id),
        )?;
        let mut trivial_affine_locals = machine
            .structural_places
            .iter()
            .filter_map(|place| match place.kind {
                StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type,
                } => Some((place.id, declaration_ordinal, structural_type)),
                _ => None,
            })
            .collect::<Vec<_>>();
        trivial_affine_locals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
        if trivial_affine_locals.iter().enumerate().any(
            |(expected, (_, declaration_ordinal, _))| {
                u32::try_from(expected).ok() != Some(*declaration_ordinal)
            },
        ) {
            return Err(ModuleError::NonCanonicalTrivialAffineLocals(machine.id));
        }
        for (place, _, structural_type) in &trivial_affine_locals {
            let Some(declaration) = types.get(structural_type) else {
                return Err(ModuleError::UnknownStructuralType(*structural_type));
            };
            if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(
                    ModuleError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                        machine: machine.id,
                        place: *place,
                    },
                );
            }
        }
        let establishments = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                OperationKind::EstablishTrivialAffineLocal { destination } => Some(destination),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_establishments = trivial_affine_locals
            .iter()
            .map(|(place, _, _)| *place)
            .collect::<Vec<_>>();
        if !trivial_affine_locals.is_empty()
            && (machine.blocks.len() != 1
                || !matches!(
                    machine.blocks[0].terminator,
                    Terminator::ReturnStructural { .. } | Terminator::ReturnUnit { .. }
                )
                || establishments != expected_establishments)
        {
            return Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(
                machine.id,
            ));
        }
        match &machine.result {
            TerminalMachineResult::Unit => {
                if let Some(place) = machine
                    .structural_places
                    .iter()
                    .find(|place| place.kind == StructuralPlaceKind::Result)
                {
                    return Err(ModuleError::UnitMachineHasResultStructuralPlace {
                        machine: machine.id,
                        place: place.id,
                    });
                }
            }
            TerminalMachineResult::Scalar(_) => {
                if let Some(place) = machine
                    .structural_places
                    .iter()
                    .find(|place| place.kind == StructuralPlaceKind::Result)
                {
                    return Err(ModuleError::ScalarMachineHasResultStructuralPlace {
                        machine: machine.id,
                        place: place.id,
                    });
                }
            }
            TerminalMachineResult::Structural(result) => {
                if result.multiplicity == StructuralMultiplicity::Unrestricted {
                    return Err(ModuleError::StructuralResultMustBeOwned(machine.id));
                }
                if !types.contains_key(&result.structural_type) {
                    return Err(ModuleError::UnknownStructuralType(result.structural_type));
                }
                let mut qualifications = BTreeSet::new();
                for qualification in &result.qualifications {
                    if !qualifications.insert(*qualification) {
                        return Err(ModuleError::DuplicateStructuralQualification {
                            place: result.place,
                            domain: *qualification,
                        });
                    }
                    let Some(domain) = domains.get(qualification) else {
                        return Err(ModuleError::UnknownStructuralDomain(*qualification));
                    };
                    if domain.carrier != result.structural_type {
                        return Err(ModuleError::StructuralDomainCarrierMismatch {
                            domain: domain.id,
                            expected: result.structural_type,
                            actual: domain.carrier,
                        });
                    }
                }
                if result
                    .qualifications
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                {
                    return Err(ModuleError::NonCanonicalStructuralQualifications(
                        result.place,
                    ));
                }
                if !machine.structural_places.iter().any(|place| {
                    place.id == result.place && place.kind == StructuralPlaceKind::Result
                }) {
                    return Err(ModuleError::StructuralResultPlaceMismatch {
                        machine: machine.id,
                        place: result.place,
                    });
                }
            }
        }
        if !machine.structural_parameters.is_empty() {
            for parameter in &machine.structural_parameters {
                let expected = StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                };
                if !machine
                    .structural_places
                    .iter()
                    .any(|place| place.id == parameter.place && place.kind == expected)
                {
                    return Err(ModuleError::StructuralParameterPlaceMismatch {
                        machine: machine.id,
                        place: parameter.place,
                    });
                }
            }
            for place in &machine.structural_places {
                if matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                    && !machine
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == place.id)
                {
                    return Err(ModuleError::StructuralPlaceHasNoParameter {
                        machine: machine.id,
                        place: place.id,
                    });
                }
            }
        }
        validate_service_ceiling(
            &machine.published_service_ceiling,
            &services,
            ServiceCeilingOwner::Machine(machine.id),
        )?;
        validate_machine_entry_claims(module, machine)?;
    }
    Ok(())
}

fn provider_signature_parameter(
    parameter: &StructuralParameterDeclaration,
) -> psi_terminal::ProviderSignatureParameter {
    psi_terminal::ProviderSignatureParameter {
        position: parameter.position,
        is_self: parameter.is_self,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: parameter.qualifications.clone(),
    }
}

fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), ModuleError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(ModuleError::RecursiveStructuralType(id));
        }
        let declaration = types[&id];
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(*target, types, active, complete)?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(*element, types, active, complete)?;
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type {
                        visit(*target, types, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in types.keys().copied() {
        visit(id, types, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_service_graph(
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> Result<(), ModuleError> {
    fn visit(
        id: ServiceId,
        services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), ModuleError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(ModuleError::RecursiveServiceHierarchy(id));
        }
        for parent in &services[&id].parents {
            visit(*parent, services, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in services.keys().copied() {
        visit(id, services, &mut active, &mut complete)?;
    }
    for declaration in services.values() {
        for parent in &declaration.parents {
            if let Some(ancestor) = services[parent]
                .parents
                .iter()
                .find(|ancestor| !declaration.parents.contains(ancestor))
            {
                return Err(ModuleError::IncompleteServiceParentClosure {
                    service: declaration.id,
                    ancestor: *ancestor,
                });
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralSignatureOwner {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCeilingOwner {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
    RootConcrete,
    InstallationReach(usize),
}

trait AttachmentIdentity: Copy {
    fn unknown_attachment(self, attachment: StructuralTypeId) -> ModuleError;
}

impl AttachmentIdentity for MachineId {
    fn unknown_attachment(self, attachment: StructuralTypeId) -> ModuleError {
        ModuleError::UnknownMachineAttachment {
            machine: self,
            attachment,
        }
    }
}

impl AttachmentIdentity for BoundaryMachineId {
    fn unknown_attachment(self, attachment: StructuralTypeId) -> ModuleError {
        ModuleError::UnknownBoundaryAttachment {
            boundary: self,
            attachment,
        }
    }
}

fn validate_attachment<Id: AttachmentIdentity>(
    owner: Id,
    attachment: Option<StructuralTypeId>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    if let Some(attachment) = attachment
        && !types.contains_key(&attachment)
    {
        return Err(owner.unknown_attachment(attachment));
    }
    Ok(())
}

fn validate_structural_signature(
    parameters: &[StructuralParameterDeclaration],
    attachment: Option<StructuralTypeId>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
    owner: StructuralSignatureOwner,
) -> Result<(), ModuleError> {
    let mut places = BTreeSet::new();
    let mut saw_self = false;
    for (index, parameter) in parameters.iter().enumerate() {
        if parameter.position != index as u32 {
            return Err(ModuleError::NonDenseStructuralParameter {
                owner,
                expected: index as u32,
                actual: parameter.position,
            });
        }
        if !places.insert(parameter.place) {
            return Err(ModuleError::DuplicateStructuralParameterPlace(
                parameter.place,
            ));
        }
        if !types.contains_key(&parameter.structural_type) {
            return Err(ModuleError::UnknownStructuralType(
                parameter.structural_type,
            ));
        }
        if parameter.is_self {
            if saw_self || attachment != Some(parameter.structural_type) {
                return Err(ModuleError::InvalidStructuralSelfParameter { owner });
            }
            saw_self = true;
        }
        let mut qualifications = BTreeSet::new();
        for qualification in &parameter.qualifications {
            if !qualifications.insert(*qualification) {
                return Err(ModuleError::DuplicateStructuralQualification {
                    place: parameter.place,
                    domain: *qualification,
                });
            }
            let Some(domain) = domains.get(qualification) else {
                return Err(ModuleError::UnknownStructuralDomain(*qualification));
            };
            if domain.carrier != parameter.structural_type {
                return Err(ModuleError::StructuralDomainCarrierMismatch {
                    domain: domain.id,
                    expected: parameter.structural_type,
                    actual: domain.carrier,
                });
            }
        }
        if parameter
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalStructuralQualifications(
                parameter.place,
            ));
        }
    }
    Ok(())
}

fn validate_service_ceiling(
    ceiling: &[ServiceId],
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    owner: ServiceCeilingOwner,
) -> Result<(), ModuleError> {
    let mut seen = BTreeSet::new();
    for service in ceiling {
        if !seen.insert(*service) {
            return Err(ModuleError::DuplicatePublishedService {
                owner,
                service: *service,
            });
        }
        let Some(declaration) = services.get(service) else {
            return Err(ModuleError::UnknownPublishedService {
                owner,
                service: *service,
            });
        };
        if declaration
            .parents
            .iter()
            .any(|parent| !ceiling.contains(parent))
        {
            return Err(ModuleError::IncompletePublishedServiceClosure {
                owner,
                service: *service,
            });
        }
    }
    if ceiling.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalPublishedServiceCeiling(owner));
    }
    Ok(())
}

fn validate_machine_entry_claims(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ModuleError> {
    let mut claims = BTreeSet::new();
    let mut inputs = Vec::<&EntryClaim>::new();
    for (index, claim) in machine.entry_claims.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory claim count fits u64")
                .checked_add(1)
                .expect("an in-memory claim count cannot exhaust u64"),
        )
        .expect("dense claim identities begin at one");
        if claim.claim != expected {
            return Err(ModuleError::NonDenseStructuralEntryClaim {
                machine: machine.id,
                expected,
                actual: claim.claim,
            });
        }
        if !claims.insert(claim.claim) {
            return Err(ModuleError::DuplicateClaim(claim.claim));
        }
        if inputs
            .iter()
            .any(|previous| previous.input == claim.input && previous.path == claim.path)
        {
            return Err(ModuleError::DuplicateEntryClaimInput(claim.input));
        }
        let Some(parameter) = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
        else {
            return Err(ModuleError::EntryClaimRequiresStructuralParameter(
                claim.claim,
            ));
        };
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
            return Err(ModuleError::EntryClaimRequiresOwnedParameter(claim.claim));
        }
        if resolve_structural_path(module, parameter.structural_type, &claim.path).is_none() {
            return Err(ModuleError::InvalidEntryClaimFieldPath(claim.claim));
        }
        if inputs.iter().any(|previous| {
            previous.input == claim.input
                && (previous.path.starts_with(&claim.path)
                    || claim.path.starts_with(&previous.path))
        }) {
            return Err(ModuleError::OverlappingEntryClaimInput {
                first: inputs
                    .iter()
                    .find(|previous| {
                        previous.input == claim.input
                            && (previous.path.starts_with(&claim.path)
                                || claim.path.starts_with(&previous.path))
                    })
                    .expect("overlap predicate found a prior claim")
                    .claim,
                second: claim.claim,
            });
        }
        inputs.push(claim);
    }
    for parameter in &machine.structural_parameters {
        if parameter.multiplicity == StructuralMultiplicity::Linear
            && !machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return Err(ModuleError::LinearParameterHasNoEntryClaim {
                machine: machine.id,
                place: parameter.place,
            });
        }
        let Some(StructuralTypeShape::FixedArray { length, .. }) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            continue;
        };
        if parameter.multiplicity != StructuralMultiplicity::Linear {
            continue;
        }
        let actual = machine
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        let complete = usize::try_from(*length).ok().is_some_and(|length| {
            actual.len() == length
                && actual.iter().enumerate().all(|(index, path)| {
                    **path
                        == [StructuralPathSegment::FixedIndex(
                            u64::try_from(index).expect("a usize index fits u64"),
                        )]
                })
        });
        if !complete {
            return Err(ModuleError::IncompleteFixedArrayEntryClaims {
                machine: machine.id,
                place: parameter.place,
            });
        }
    }
    if machine.entry_claims.windows(2).any(|pair| {
        let key = |claim: &EntryClaim| {
            let position = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == claim.input)
                .expect("entry claim parameter was validated")
                .position;
            (position, claim.path.clone())
        };
        key(&pair[0]) >= key(&pair[1])
    }) {
        return Err(ModuleError::NonCanonicalEntryClaimOrder(machine.id));
    }
    Ok(())
}

pub(super) fn resolve_structural_path(
    module: &TerminalModule,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let Some(declaration) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
        else {
            return None;
        };
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

pub(super) fn is_nonempty_field_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
}

pub(super) fn partial_affine_residuals(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty() || moved_paths.iter().any(|path| !is_nonempty_field_path(path)) {
        return None;
    }
    if moved_paths.iter().enumerate().any(|(index, path)| {
        moved_paths
            .iter()
            .enumerate()
            .any(|(other_index, other)| index != other_index && path.starts_with(other))
    }) {
        return None;
    }
    let moved_paths = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut residuals = Vec::new();
    collect_partial_affine_residuals(
        module,
        root_type,
        &moved_paths,
        &mut Vec::new(),
        &mut residuals,
    )?;
    Some(residuals)
}

fn collect_partial_affine_residuals(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(field.field_type, StructuralFieldType::Structural(_))
        })
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            unreachable!("record field shape was checked above")
        };
        prefix.push(StructuralPathSegment::Field(field.identity.clone()));
        let descendants = moved_paths
            .iter()
            .filter_map(|path| match path {
                [StructuralPathSegment::Field(identity), remaining @ ..]
                    if *identity == field.identity =>
                {
                    Some(remaining)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        matched += descendants.len();
        if descendants.is_empty() {
            residuals.push((prefix.clone(), field_type));
        } else if descendants.iter().all(|path| !path.is_empty()) {
            collect_partial_affine_residuals(module, field_type, &descendants, prefix, residuals)?;
        } else if descendants.len() != 1 {
            return None;
        } else {
            debug_assert!(descendants[0].is_empty());
        }
        prefix.pop();
    }
    (matched == moved_paths.len()).then_some(())
}
