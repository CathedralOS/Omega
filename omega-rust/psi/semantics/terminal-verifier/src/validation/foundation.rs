//! The structural foundation: every declaration, signature, service ceiling
//! and structural path resolves against the module's types before any
//! machine is validated.
//!
//! `validate_structural_foundation` registers the structural types,
//! domains and services (`structural_types`, `domains`, `services`),
//! validates the boundary machines and provider candidates against them
//! (`boundary_machines`, `provider_candidates`), then each machine's
//! declarations (`machine_foundations`). The resolvers and graph checks
//! those owners share stay in this file.

mod boundary_machines;
mod domains;
mod machine_foundations;
mod provider_candidates;
mod services;
mod structural_types;

use super::structural_qualification_rosters::validate_projected_qualification_roster;
use super::{
    BTreeMap, BTreeSet, BoundaryMachineDeclaration, BoundaryMachineId,
    CanonicalStructuralPathSegment, ClaimId, ContentProjectionExpression, ContentProjectionScalar,
    EntryClaim, MachineId, ModuleError, OperationKind, PlaceId, ServiceId, StructuralDomainId,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    TerminalMachine, TerminalModule, program_local_root_introduction_compatibility_report_identity,
};

mod provider_result;

pub(crate) fn structural_leaf_type<'module>(
    module: &'module TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<&'module StructuralFieldType> {
    let mut structural_type = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?
        .structural_type;
    let mut selected_case_fields = None;
    if path.is_empty() {
        return None;
    }
    for (index, segment) in path.iter().enumerate() {
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)?;
        let is_last = index + 1 == path.len();
        if let CanonicalStructuralPathSegment::Case(case_id) = segment {
            if selected_case_fields.is_some() || is_last {
                return None;
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return None,
            };
            selected_case_fields = Some(
                &cases
                    .iter()
                    .find(|candidate| candidate.id == *case_id)?
                    .fields,
            );
            continue;
        }
        if let Some(fields) = selected_case_fields.take() {
            let CanonicalStructuralPathSegment::Field(field_id) = segment else {
                return None;
            };
            let field = fields
                .iter()
                .find(|candidate| candidate.id == *field_id)
                .filter(|field| !field.relevance.is_erased())?;
            if is_last {
                return Some(&field.field_type);
            }
            let StructuralFieldType::Structural(next) = field.field_type else {
                return None;
            };
            structural_type = next;
            continue;
        }
        match (segment, &declaration.shape) {
            (
                CanonicalStructuralPathSegment::Field(field_id),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields
                    .iter()
                    .find(|candidate| candidate.id == *field_id)
                    .filter(|field| !field.relevance.is_erased())?;
                if is_last {
                    return Some(&field.field_type);
                }
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                structural_type = next;
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(fixed_index),
                StructuralTypeShape::FixedArray { element, length },
            ) if !is_last && *fixed_index < *length => structural_type = *element,
            _ => return None,
        }
    }
    None
}

fn validate_structural_fields(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    fields: &[terminal_psi::StructuralFieldDeclaration],
    permit_provider_attachment: bool,
) -> Result<(), ModuleError> {
    let mut field_ids = BTreeSet::new();
    let mut field_names = BTreeSet::new();
    for field in fields {
        if !field_ids.insert(field.id)
            || field.identity.is_empty()
            || !field_names.insert(field.identity.as_str())
        {
            return Err(ModuleError::InvalidStructuralFieldIdentity {
                structural_type,
                field: field.id,
            });
        }
        let is_provider_attachment = permit_provider_attachment
            && module
                .machines
                .iter()
                .any(|machine| machine.attachment == Some(structural_type));
        match &field.field_type {
            StructuralFieldType::Erased { type_identity } if type_identity.is_empty() => {
                return Err(ModuleError::InvalidErasedStructuralField {
                    structural_type,
                    field: field.id,
                });
            }
            StructuralFieldType::Erased { .. }
                if !field.relevance.is_erased() && !is_provider_attachment =>
            {
                return Err(ModuleError::InvalidErasedStructuralField {
                    structural_type,
                    field: field.id,
                });
            }
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::Structural(_)
                if field.relevance.is_erased() =>
            {
                return Err(ModuleError::InvalidErasedStructuralField {
                    structural_type,
                    field: field.id,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_structural_cases(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    cases: &[terminal_psi::StructuralCaseDeclaration],
) -> Result<(), ModuleError> {
    if cases.is_empty() {
        return Err(ModuleError::EmptyStructuralSum(structural_type));
    }
    let mut case_ids = BTreeSet::new();
    let mut case_names = BTreeSet::new();
    for case in cases {
        if !case_ids.insert(case.id)
            || case.identity.is_empty()
            || !case_names.insert(case.identity.as_str())
        {
            return Err(ModuleError::InvalidStructuralCaseIdentity {
                structural_type,
                case: case.id,
            });
        }
        validate_structural_fields(module, structural_type, &case.fields, false)?;
    }
    Ok(())
}

pub(super) fn validate_structural_foundation(module: &TerminalModule) -> Result<(), ModuleError> {
    let types = structural_types::register_structural_types(module)?;
    let domains = domains::register_structural_domains(module, &types)?;
    let services = services::register_services(module)?;
    boundary_machines::validate_boundary_machines(module, &types, &domains, &services)?;
    provider_candidates::validate_provider_candidates(module, &types)?;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    for machine in &module.machines {
        machine_foundations::validate_machine_foundation(
            module, machine, &machines, &types, &domains, &services,
        )?;
    }
    Ok(())
}

fn validate_content_projection_scalar(
    value: &ContentProjectionScalar,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    depth: usize,
) -> bool {
    if depth > 256 {
        return false;
    }
    match value {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            if path.is_empty() || path.iter().any(String::is_empty) {
                return false;
            }
            let mut current = carrier;
            for (index, segment) in path.iter().enumerate() {
                let Some(declaration) = types.get(&current) else {
                    return false;
                };
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return false;
                };
                let Some(field) = fields.iter().find(|field| field.identity == *segment) else {
                    return false;
                };
                let last = index + 1 == path.len();
                match (&field.field_type, last) {
                    (StructuralFieldType::Structural(next), false) => current = *next,
                    (StructuralFieldType::Scalar(_), true) => {}
                    _ => return false,
                }
            }
            true
        }
        ContentProjectionScalar::Natural(value) => {
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0'))
        }
        ContentProjectionScalar::Successor(inner) => {
            validate_content_projection_scalar(inner, carrier, types, depth + 1)
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            validate_content_projection_scalar(left, carrier, types, depth + 1)
                && validate_content_projection_scalar(right, carrier, types, depth + 1)
        }
    }
}

fn validate_content_projection_expression(
    expression: &ContentProjectionExpression,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => members.iter().all(|(start, end)| {
            validate_content_projection_scalar(start, carrier, types, 0)
                && validate_content_projection_scalar(end, carrier, types, 0)
        }),
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            validate_content_projection_scalar(magnitude, carrier, types, 0)
        }
    }
}

fn validate_structural_content_projection(
    semantic_domain: semantic_vocabulary::DomainSemanticId,
    carrier: StructuralTypeId,
    projection: &terminal_psi::StructuralContentProjection,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let shape_matches_algebra = matches!(
        (&projection.expression, projection.algebra.kind),
        (
            ContentProjectionExpression::IntervalSet(_),
            semantic_vocabulary::ContentAlgebraKind::IntervalSet
        ) | (
            ContentProjectionExpression::CountedQuantity(_),
            semantic_vocabulary::ContentAlgebraKind::CountedQuantity
        )
    );
    projection.identity.domain.get() == semantic_domain.get()
        && projection.identity.projection_report_fingerprint != 0
        && !projection.algebra.parameter.is_empty()
        && shape_matches_algebra
        && validate_content_projection_expression(&projection.expression, carrier, types)
        && language_semantics::content::terminal_projection_report_fingerprint(
            &projection.algebra,
            &projection.expression,
        ) == projection.identity.projection_report_fingerprint
}

fn validate_program_local_root_introductions(
    boundary: &BoundaryMachineDeclaration,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> Result<(), ModuleError> {
    fn invalid(boundary: BoundaryMachineId, argument_index: u32) -> ModuleError {
        ModuleError::InvalidProgramLocalRootIntroduction {
            boundary,
            argument_index,
        }
    }
    let mut seen = BTreeSet::new();
    for schema in &boundary.program_local_root_introductions {
        if !seen.insert((schema.argument_index, schema.qualification)) {
            return Err(ModuleError::DuplicateProgramLocalRootIntroduction {
                boundary: boundary.id,
                argument_index: schema.argument_index,
                domain: schema.qualification,
            });
        }
        let Some(parameter) = boundary
            .structural_parameters
            .get(schema.argument_index as usize)
        else {
            return Err(invalid(boundary.id, schema.argument_index));
        };
        let Some(domain) = domains.get(&schema.qualification) else {
            return Err(invalid(boundary.id, schema.argument_index));
        };
        let requirement = terminal_psi::StructuralDomainRequirement {
            argument_index: schema.argument_index,
            domain: schema.qualification,
        };
        let Some(owner_projection) = domain.content_projection.as_ref() else {
            return Err(invalid(boundary.id, schema.argument_index));
        };
        let shape_matches_algebra = matches!(
            (&schema.capacity, schema.algebra.kind),
            (
                ContentProjectionExpression::IntervalSet(_),
                semantic_vocabulary::ContentAlgebraKind::IntervalSet
            ) | (
                ContentProjectionExpression::CountedQuantity(_),
                semantic_vocabulary::ContentAlgebraKind::CountedQuantity
            )
        );
        let capacity_valid =
            validate_content_projection_expression(&schema.capacity, schema.carrier, types);
        if schema.compatibility_report_identity == 0
            || schema.source_parameter_position != parameter.position
            || schema.carrier != parameter.structural_type
            || schema.carrier != domain.carrier
            || !parameter.qualifications.contains(&schema.qualification)
            || !boundary.requires.contains(&requirement)
            || schema.projection.domain.get() != domain.semantic_domain.get()
            || schema.projection.projection_report_fingerprint == 0
            || schema.algebra.parameter.is_empty()
            || !shape_matches_algebra
            || !capacity_valid
            || schema.projection != owner_projection.identity
            || schema.algebra != owner_projection.algebra
            || schema.capacity != owner_projection.expression
            || language_semantics::content::terminal_projection_report_fingerprint(
                &schema.algebra,
                &schema.capacity,
            ) != schema.projection.projection_report_fingerprint
            || program_local_root_introduction_compatibility_report_identity(
                &boundary.identity,
                &domain.identity,
                &types
                    .get(&schema.carrier)
                    .expect("schema carrier was validated before identity replay")
                    .identity,
                schema,
            ) != schema.compatibility_report_identity
        {
            return Err(invalid(boundary.id, schema.argument_index));
        }
    }
    if boundary
        .program_local_root_introductions
        .windows(2)
        .any(|pair| {
            (pair[0].argument_index, pair[0].qualification)
                >= (pair[1].argument_index, pair[1].qualification)
        })
    {
        return Err(ModuleError::NonCanonicalProgramLocalRootIntroductions(
            boundary.id,
        ));
    }
    Ok(())
}

fn validate_provider_attachment_specialization(
    module: &TerminalModule,
    machine: &TerminalMachine,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    let provider_roots = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((place.id, attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = machine
        .attachment
        .and_then(|attachment| types.get(&attachment))
        .and_then(|attachment| match &attachment.shape {
            StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(field.field_type, StructuralFieldType::Erased { .. })
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_fields.is_empty() && provider_roots.is_empty() {
        return Ok(());
    }
    let invalid = || ModuleError::InvalidProviderAttachmentSpecialization(machine.id);
    let [provider_field] = provider_fields.as_slice() else {
        return Err(invalid());
    };
    let Some(attachment) = machine.attachment else {
        return Err(invalid());
    };
    let self_parameters = machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let invalid_self = match self_parameters.as_slice() {
        [] => false,
        [parameter] => parameter.position != 0 || parameter.structural_type != attachment,
        _ => true,
    };
    if provider_roots.windows(2).any(|pair| pair[0].3 >= pair[1].3) || invalid_self {
        return Err(invalid());
    }
    let mut specialized_boundaries = BTreeSet::new();
    for (_, root_attachment, field, boundary) in &provider_roots {
        let Some(boundary_declaration) = module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == *boundary)
        else {
            return Err(invalid());
        };
        if *root_attachment != attachment
            || *field != provider_field.id
            || boundary_declaration.attachment.is_some()
            || !specialized_boundaries.insert(*boundary)
        {
            return Err(invalid());
        }
    }

    let mut called_boundaries = BTreeSet::new();
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        match &operation.kind {
            OperationKind::BoundaryCall {
                boundary,
                structural_arguments,
                ..
            } => {
                // A call to a boundary-declaration machine — its declaration
                // retains an attachment — settles through that machine's own
                // boundary seam and consumes no provider field. Only a call to
                // a signature boundary (attachment absent) is an obligation the
                // specialization must cover.
                if module
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == *boundary)
                    .is_some_and(|declaration| declaration.attachment.is_none())
                {
                    called_boundaries.insert(*boundary);
                }
                if structural_arguments.iter().any(|argument| {
                    provider_roots
                        .iter()
                        .any(|(place, ..)| *place == argument.place)
                }) {
                    return Err(invalid());
                }
            }
            OperationKind::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments.iter().any(|argument| {
                provider_roots
                    .iter()
                    .any(|(place, ..)| *place == argument.place)
            }) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
    }
    if called_boundaries != specialized_boundaries {
        return Err(invalid());
    }
    Ok(())
}

fn provider_signature_parameter(
    parameter: &StructuralParameterDeclaration,
) -> terminal_psi::ProviderSignatureParameter {
    terminal_psi::ProviderSignatureParameter {
        position: parameter.position,
        is_self: parameter.is_self,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
    }
}

fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), ModuleError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
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
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::Reference { referent, .. } => {
                visit(*referent, types, active, complete)?;
            }
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
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
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
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
) -> Result<(), ModuleError> {
    fn visit(
        id: ServiceId,
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
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
    MachineDeclared(MachineId),
    Boundary(BoundaryMachineId),
    BoundaryFixed(BoundaryMachineId),
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
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
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
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
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
        validate_projected_qualification_roster(
            parameter.place,
            parameter.structural_type,
            &parameter.projected_qualifications,
            types,
            domains,
        )?;
    }
    Ok(())
}

pub(super) fn validate_service_ceiling(
    ceiling: &[ServiceId],
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
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

pub(super) fn resolve_structural_path_in_types(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    types.get(&structural_type)?;
    for segment in path {
        let declaration = types.get(&structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        *types
                            .iter()
                            .find(|(_, declaration)| declaration.shape == shape)
                            .map(|(id, _)| id)?
                    }
                }
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
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        module
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.shape == shape)
                            .map(|declaration| declaration.id)?
                    }
                }
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

/// A borrowed join may also project through literal fixed-array elements: a
/// `FixedIndex` is as statically exact as a record field, and
/// `resolve_structural_path` still proves each ordinal inside its declared
/// extent and the leaf identical to the parameter's declared type. `Referent`
/// stays out -- a borrow that crosses another borrow's boundary is a
/// different custody contract, not a projection.
pub(super) fn is_nonempty_exact_projection_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        })
}
