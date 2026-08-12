use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentConservation,
    ContentProjectionIdentity, ContentStructuralPlace, ContentTerm, ContractId, EdgeId, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    PropositionId, ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind,
    StructuralTypeId, ValueId, content_conservation_fingerprint,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimSettlement, ClaimTransfer, ContentPartitionComposition,
    CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, OperationKind,
    PropositionBinderArgumentKind, PropositionBinderKind, PropositionEvidence, StructuralArgument,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

use crate::verification::{substitute_proposition_places, substitute_proposition_values};

#[derive(Debug, Clone, Copy)]
pub struct ValidatedTerminalModule<'module> {
    module: &'module TerminalModule,
}

impl<'module> ValidatedTerminalModule<'module> {
    pub const fn module(self) -> &'module TerminalModule {
        self.module
    }

    pub fn machine(self, id: MachineId) -> Option<&'module TerminalMachine> {
        self.module.machines.iter().find(|machine| machine.id == id)
    }

    pub fn value_context(
        self,
        machine: &TerminalMachine,
    ) -> Result<PropositionContext, ModuleError> {
        PropositionContext::from_value_types_and_places(
            machine_value_types(machine),
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

pub fn validate_module(
    module: &TerminalModule,
) -> Result<ValidatedTerminalModule<'_>, ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Execution)?;
    Ok(ValidatedTerminalModule { module })
}

/// Validate the representation-wide invariants needed by canonical codecs.
///
/// This remains a distinct entry point so canonical representation checks do
/// not themselves confer an execution-grade `ValidatedTerminalModule`.
pub fn validate_module_representation(module: &TerminalModule) -> Result<(), ModuleError> {
    validate_module_with_policy(module, ValidationPolicy::Representation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationPolicy {
    Execution,
    Representation,
}

fn validate_module_with_policy(
    module: &TerminalModule,
    policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if module.machines.is_empty() {
        return Err(ModuleError::EmptyModule);
    }
    validate_proposition_vocabulary(module)?;
    validate_structural_foundation(module)?;

    let mut registry = IdRegistry::default();
    for machine in &module.machines {
        insert_unique(
            &mut registry.machines,
            machine.id,
            ModuleError::DuplicateMachine,
        )?;
        insert_unique(
            &mut registry.contracts,
            machine.contract.id,
            ModuleError::DuplicateContract,
        )?;
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    for machine in &module.machines {
        validate_machine(module, machine, &machines, &mut registry, policy)?;
    }
    validate_call_graph(module)?;
    if !registry.machines.contains(&module.entry) {
        return Err(ModuleError::UnknownEntryMachine(module.entry));
    }

    Ok(())
}

fn validate_call_graph(module: &TerminalModule) -> Result<(), ModuleError> {
    let calls = module
        .machines
        .iter()
        .map(|machine| {
            let callees = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, .. } | OperationKind::CallUnit { callee, .. } => {
                        Some(*callee)
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            (machine.id, callees)
        })
        .collect::<BTreeMap<_, _>>();

    let mut indegree = calls
        .keys()
        .copied()
        .map(|machine| (machine, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for callees in calls.values() {
        for callee in callees {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target is registered");
            *count += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(machine, count)| (*count == 0).then_some(*machine))
        .collect::<BTreeSet<_>>();
    let mut completed = 0_usize;
    while let Some(machine) = ready.pop_first() {
        completed += 1;
        for callee in &calls[&machine] {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*callee);
            }
        }
    }
    if completed != calls.len() {
        let machine = indegree
            .into_iter()
            .find_map(|(machine, count)| (count != 0).then_some(machine))
            .expect("incomplete topological order has a cyclic remainder");
        return Err(ModuleError::RecursiveCallSliceNotYetSupported(machine));
    }
    Ok(())
}

fn validate_proposition_vocabulary(module: &TerminalModule) -> Result<(), ModuleError> {
    let mut declarations = BTreeMap::new();
    let mut declaration_names = BTreeSet::new();
    for (index, declaration) in module.proposition_declarations.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if declaration.id != expected {
            return Err(ModuleError::NonDensePropositionDeclaration {
                expected,
                actual: declaration.id,
            });
        }
        if declarations.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicatePropositionDeclaration(declaration.id));
        }
        if declaration.name.is_empty() {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
        if !declaration_names.insert(declaration.name.as_str()) {
            return Err(ModuleError::DuplicatePropositionName(
                declaration.name.clone(),
            ));
        }
        let mut binder_names = BTreeSet::new();
        for binder in &declaration.binders {
            if binder.name.is_empty() || !binder_names.insert(binder.name.as_str()) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
            if matches!(
                &binder.kind,
                PropositionBinderKind::Const { type_identity } if type_identity.is_empty()
            ) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
        }
        if declaration.parameter_types.iter().any(String::is_empty)
            || matches!(
                &declaration.evidence,
                PropositionEvidence::Witness { evidence_type } if evidence_type.is_empty()
            )
        {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
    }

    let mut applications = BTreeSet::new();
    for (index, application) in module.proposition_applications.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if application.id != expected {
            return Err(ModuleError::NonDensePropositionApplication {
                expected,
                actual: application.id,
            });
        }
        if !applications.insert(application.id) {
            return Err(ModuleError::DuplicatePropositionApplication(application.id));
        }
        let Some(declaration) = declarations.get(&application.declaration) else {
            return Err(ModuleError::UnknownPropositionDeclaration(
                application.declaration,
            ));
        };
        if application.binder_arguments.len() != declaration.binders.len()
            || application.arguments.len() != declaration.parameter_types.len()
        {
            return Err(ModuleError::PropositionApplicationArityMismatch(
                application.id,
            ));
        }
        for (argument, binder) in application
            .binder_arguments
            .iter()
            .zip(&declaration.binders)
        {
            let kind_matches = matches!(
                (&argument.kind, &binder.kind),
                (
                    PropositionBinderArgumentKind::Type,
                    PropositionBinderKind::Type
                ) | (
                    PropositionBinderArgumentKind::Const,
                    PropositionBinderKind::Const { .. }
                ) | (
                    PropositionBinderArgumentKind::Machine,
                    PropositionBinderKind::Machine
                )
            );
            if !kind_matches || argument.identity.is_empty() {
                return Err(ModuleError::PropositionApplicationBinderMismatch(
                    application.id,
                ));
            }
        }
        if application.arguments.iter().any(String::is_empty) {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
    }
    Ok(())
}

fn validate_structural_foundation(module: &TerminalModule) -> Result<(), ModuleError> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &module.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralType(declaration.id));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
        }
        let StructuralTypeShape::Record { fields } = &declaration.shape;
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
    }
    for declaration in &module.structural_types {
        let StructuralTypeShape::Record { fields } = &declaration.shape;
        for field in fields {
            if let StructuralFieldType::Structural(target) = &field.field_type
                && !types.contains_key(target)
            {
                return Err(ModuleError::UnknownStructuralType(*target));
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

    for machine in &module.machines {
        validate_attachment(machine.id, machine.attachment, &types)?;
        validate_structural_signature(
            &machine.structural_parameters,
            machine.attachment,
            &types,
            &domains,
            StructuralSignatureOwner::Machine(machine.id),
        )?;
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
        validate_machine_entry_claims(machine)?;
    }
    Ok(())
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
        let StructuralTypeShape::Record { fields } = &declaration.shape;
        for field in fields {
            if let StructuralFieldType::Structural(target) = &field.field_type {
                visit(*target, types, active, complete)?;
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

fn validate_machine_entry_claims(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let mut claims = BTreeSet::new();
    let mut inputs = BTreeSet::new();
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
        if !inputs.insert(claim.input) {
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
    }
    Ok(())
}

#[derive(Default)]
struct IdRegistry {
    machines: BTreeSet<MachineId>,
    blocks: BTreeSet<BlockId>,
    contracts: BTreeSet<ContractId>,
    operations: BTreeSet<OperationId>,
    edges: BTreeSet<EdgeId>,
    obligations: BTreeSet<ObligationId>,
    values: BTreeSet<ValueId>,
    places: BTreeSet<PlaceId>,
    content_projection_algebras: BTreeMap<ContentProjectionIdentity, ContentAlgebra>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralRootKey {
    Parameter(u32),
    Result,
}

fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
    _policy: ValidationPolicy,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }

    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    let mut structural_roots = BTreeSet::new();
    let mut structural_place_kinds = BTreeMap::new();
    for place in &machine.structural_places {
        insert_unique(&mut registry.places, place.id, ModuleError::DuplicatePlace)?;
        if matches!(machine.result, TerminalMachineResult::Unit)
            && place.kind == psi_core::StructuralPlaceKind::Result
        {
            return Err(ModuleError::UnitMachineHasResultStructuralPlace {
                machine: machine.id,
                place: place.id,
            });
        }
        let root = match place.kind {
            psi_core::StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            psi_core::StructuralPlaceKind::Result => StructuralRootKey::Result,
        };
        if !structural_roots.insert(root) {
            return Err(ModuleError::DuplicateStructuralPlaceRoot {
                machine: machine.id,
                kind: place.kind,
            });
        }
        structural_place_kinds.insert(place.id, place.kind);
    }
    for declaration in machine.parameters.iter().chain(machine.result.scalar_ref()) {
        insert_value(
            &mut value_types,
            &mut registry.values,
            declaration.id,
            declaration.scalar_type,
        )?;
    }
    for block in &machine.blocks {
        insert_unique(&mut registry.blocks, block.id, ModuleError::DuplicateBlock)?;
        if blocks.insert(block.id, block).is_some() {
            return Err(ModuleError::DuplicateBlock(block.id));
        }
        for parameter in &block.parameters {
            insert_value(
                &mut value_types,
                &mut registry.values,
                parameter.id,
                parameter.scalar_type,
            )?;
        }
        for operation in &block.operations {
            insert_unique(
                &mut registry.operations,
                operation.id,
                ModuleError::DuplicateOperation,
            )?;
            if matches!(
                operation.kind,
                OperationKind::CallUnit { .. }
                    | OperationKind::BoundaryCallUnit { .. }
                    | OperationKind::PortWrite { .. }
            ) {
                if !matches!(operation.result, psi_terminal::OperationResult::Unit) {
                    return Err(ModuleError::UnitOperationHasScalarResult(operation.id));
                }
                validate_unit_operation_static(module, machine, machines, operation)?;
                if let OperationKind::CallUnit {
                    requirement_obligations,
                    ..
                } = &operation.kind
                {
                    for obligation in requirement_obligations {
                        insert_unique(
                            &mut registry.obligations,
                            *obligation,
                            ModuleError::DuplicateObligation,
                        )?;
                    }
                }
                continue;
            }
            let Some(result) = operation.result.scalar() else {
                return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
            };
            insert_value(
                &mut value_types,
                &mut registry.values,
                result.id,
                result.scalar_type,
            )?;
            match operation.kind.clone() {
                OperationKind::CallUnit { .. }
                | OperationKind::BoundaryCallUnit { .. }
                | OperationKind::PortWrite { .. } => {
                    unreachable!("structural/effect operations were validated above")
                }
                OperationKind::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                } => {
                    let callee =
                        machines
                            .get(&callee)
                            .copied()
                            .ok_or(ModuleError::UnknownCallTarget {
                                operation: operation.id,
                                callee,
                            })?;
                    validate_service_reach(
                        operation.id,
                        &machine.published_service_ceiling,
                        &callee.published_service_ceiling,
                    )?;
                    if crash_continuations
                        .windows(2)
                        .any(|pair| pair[0].cause >= pair[1].cause)
                    {
                        return Err(ModuleError::NonCanonicalCallCrashContinuations(
                            operation.id,
                        ));
                    }
                    let substitutions = callee
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.id,
                                ScalarTerm::value(*argument, parameter.scalar_type),
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    let expected_crash_continuations =
                        substitute_crash_routes(&callee.contract.crash_routes, &substitutions);
                    if crash_continuations != expected_crash_continuations {
                        return Err(ModuleError::CallCrashContinuationsMismatch {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    }
                    for continuation in &crash_continuations {
                        let covered = machine.contract.crash_routes.iter().any(|published| {
                            published.cause == continuation.cause
                                && (published.alternatives == [CrashRouteGuard::Truth]
                                    || continuation
                                        .alternatives
                                        .iter()
                                        .all(|route| published.alternatives.contains(route)))
                        });
                        if !covered {
                            return Err(ModuleError::CallCrashContinuationUncovered {
                                operation: operation.id,
                                cause: continuation.cause,
                            });
                        }
                    }
                    if !callee.structural_places.is_empty()
                        || !callee.content_entry_claims.is_empty()
                        || !callee.content_identity_reshuffles.is_empty()
                        || !callee.content_partition_compositions.is_empty()
                        || callee
                            .contract
                            .requires
                            .iter()
                            .chain(
                                callee
                                    .contract
                                    .ensures
                                    .iter()
                                    .map(|clause| &clause.proposition),
                            )
                            .any(proposition_contains_content)
                    {
                        return Err(ModuleError::CallTargetHasStructuralContract {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    }
                    let Some(callee_result) = callee.result.scalar() else {
                        return Err(ModuleError::CallTargetReturnsUnit {
                            operation: operation.id,
                            callee: callee.id,
                        });
                    };
                    if operation.result.expect_scalar().scalar_type != callee_result.scalar_type {
                        return Err(ModuleError::CallResultTypeMismatch {
                            operation: operation.id,
                            expected: callee_result.scalar_type,
                            actual: operation.result.expect_scalar().scalar_type,
                        });
                    }
                    if requirement_obligations.len() != callee.contract.requires.len() {
                        return Err(ModuleError::CallRequirementArityMismatch {
                            operation: operation.id,
                            expected: callee.contract.requires.len(),
                            actual: requirement_obligations.len(),
                        });
                    }
                    for obligation in requirement_obligations {
                        insert_unique(
                            &mut registry.obligations,
                            obligation,
                            ModuleError::DuplicateObligation,
                        )?;
                    }
                }
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        return Err(ModuleError::IntegerConstantRequiresIntegerResult(
                            operation.id,
                        ));
                    };
                    if !integer_type.admits(value) {
                        return Err(ModuleError::IntegerConstantOutsideResultType(operation.id));
                    }
                }
                OperationKind::BooleanConstant { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::BooleanNot { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanNotRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::BooleanEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::BooleanEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerEqualRequiresBooleanResult(operation.id));
                    }
                }
                OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. } => {
                    if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                        return Err(ModuleError::IntegerOrderingRequiresBooleanResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerBitwiseNot { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::IntegerWiden { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerWidenRequiresIntegerResult(operation.id));
                    }
                }
                OperationKind::IntegerExactCast { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::IntegerExactCastRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerShiftLeft { .. }
                | OperationKind::WrappingIntegerShiftRight { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::ExactIntegerShiftRight { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerShiftLeft { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerAdd { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerSubtract { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerMultiply { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::ExactIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::ExactIntegerRemainderRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerRemainderRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::SaturatingIntegerDivide { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerDivideRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::SaturatingIntegerRemainder { obligation, .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(
                            ModuleError::SaturatingIntegerRemainderRequiresIntegerResult(
                                operation.id,
                            ),
                        );
                    }
                    insert_unique(
                        &mut registry.obligations,
                        obligation,
                        ModuleError::DuplicateObligation,
                    )?;
                }
                OperationKind::WrappingIntegerAdd { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerAdd { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerSubtract { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerSubtract { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::WrappingIntegerMultiply { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
                OperationKind::SaturatingIntegerMultiply { .. } => {
                    if !matches!(
                        operation.result.expect_scalar().scalar_type,
                        ScalarType::Integer(_)
                    ) {
                        return Err(ModuleError::SaturatingIntegerMultiplyRequiresIntegerResult(
                            operation.id,
                        ));
                    }
                }
            }
        }
        for edge in block.terminator.edges() {
            insert_unique(&mut registry.edges, edge, ModuleError::DuplicateEdge)?;
        }
    }

    let Some(entry) = blocks.get(&machine.entry) else {
        return Err(ModuleError::UnknownEntryBlock {
            machine: machine.id,
            block: machine.entry,
        });
    };
    if !entry.parameters.is_empty() {
        return Err(ModuleError::EntryBlockCannotHaveParameters(machine.entry));
    }

    let context = PropositionContext::from_value_types_and_places(
        value_types.iter().map(|(id, ty)| (*id, *ty)),
        machine
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    validate_content_entry_claims(machine, registry, &structural_place_kinds, &context)?;
    validate_content_identity_reshuffles(machine, registry, &structural_place_kinds, &context)?;
    validate_content_partition_compositions(machine, registry, &structural_place_kinds, &context)?;
    let requires_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    validate_crash_frontiers(machine, &context, &requires_values)?;
    let mut ensures_values = requires_values.clone();
    if let Some(result) = machine.result.scalar() {
        ensures_values.insert(result.id);
    }
    for proposition in &machine.contract.requires {
        validate_contract_clause_kind(
            proposition,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            proposition,
            &requires_values,
            machine.contract.id,
            ContractClauseKind::Requires,
        )?;
    }
    for clause in &machine.contract.ensures {
        insert_unique(
            &mut registry.obligations,
            clause.obligation,
            ModuleError::DuplicateObligation,
        )?;
        validate_contract_clause_kind(
            &clause.proposition,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
        context
            .validate(&clause.proposition)
            .map_err(ModuleError::MalformedProposition)?;
        validate_contract_scope(
            &clause.proposition,
            &ensures_values,
            machine.contract.id,
            ContractClauseKind::Ensures,
        )?;
    }
    if machine
        .contract
        .ensures
        .windows(2)
        .any(|pair| pair[0].obligation >= pair[1].obligation)
    {
        return Err(ModuleError::NonCanonicalContractEnsures(
            machine.contract.id,
        ));
    }

    validate_control_flow(machine, machines, &blocks, &value_types)?;
    validate_structural_frontier(module, machine, machines, &blocks)
}

fn validate_unit_operation_static(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &psi_terminal::Operation,
) -> Result<(), ModuleError> {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee: *callee,
                })?;
            if callee.result != TerminalMachineResult::Unit || !callee.parameters.is_empty() {
                return Err(ModuleError::UnitCallTargetHasScalarSignature {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            validate_structural_arguments(
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
            )?;
            validate_unit_call_contract_places(callee, operation.id)?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            validate_unit_call_claim_transfers(
                machine,
                callee,
                structural_arguments,
                claim_transfers,
                operation.id,
            )?;
            validate_unit_call_crash_continuations(
                machine,
                callee,
                structural_arguments,
                crash_continuations,
                operation.id,
            )?;
        }
        OperationKind::BoundaryCallUnit {
            boundary,
            structural_arguments,
            claim_settlements,
            requirement_obligations,
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .ok_or(ModuleError::UnknownBoundaryCallTarget {
                    operation: operation.id,
                    boundary: *boundary,
                })?;
            if !requirement_obligations.is_empty() {
                return Err(ModuleError::BoundaryStructuralRequirementsMintObligations(
                    operation.id,
                ));
            }
            validate_structural_arguments(
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                operation.id,
            )?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &boundary.published_service_ceiling,
            )?;
            validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
            validate_boundary_settlements(
                machine,
                structural_arguments,
                claim_settlements,
                operation.id,
            )?;
        }
        OperationKind::PortWrite { service, .. } => {
            if !module
                .services
                .iter()
                .any(|candidate| candidate.id == *service)
            {
                return Err(ModuleError::UnknownOperationService {
                    operation: operation.id,
                    service: *service,
                });
            }
            if !machine.published_service_ceiling.contains(service) {
                return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
                    operation: operation.id,
                    service: *service,
                });
            }
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}

fn validate_unit_call_contract_places(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<(), ModuleError> {
    let parameters = callee
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    let propositions = callee
        .contract
        .requires
        .iter()
        .chain(
            callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )
        .chain(
            callee
                .contract
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                }),
        );
    for proposition in propositions {
        if let Some(place) = proposition_content_roots(proposition)
            .into_iter()
            .find(|place| !parameters.contains(place))
        {
            return Err(ModuleError::UnitCallContractPlaceHasNoArgument {
                operation,
                callee: callee.id,
                place,
            });
        }
    }
    Ok(())
}

fn validate_structural_arguments(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
) -> Result<(), ModuleError> {
    if arguments.len() != expected.len() {
        return Err(ModuleError::StructuralArgumentArityMismatch {
            operation,
            expected: expected.len(),
            actual: arguments.len(),
        });
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        let Some(actual) = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return Err(ModuleError::UnknownStructuralArgument {
                operation,
                argument_index: index as u32,
                place: argument.place,
            });
        };
        if actual.structural_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual.structural_type,
            });
        }
        if actual.multiplicity != expected.multiplicity {
            return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.multiplicity,
                actual: actual.multiplicity,
            });
        }
        for qualification in &expected.qualifications {
            if !actual.qualifications.contains(qualification) {
                return Err(ModuleError::StructuralArgumentMissingQualification {
                    operation,
                    argument_index: index as u32,
                    domain: *qualification,
                });
            }
        }
    }
    Ok(())
}

fn validate_service_reach(
    operation: OperationId,
    caller: &[ServiceId],
    reached: &[ServiceId],
) -> Result<(), ModuleError> {
    if let Some(service) = reached.iter().find(|service| !caller.contains(service)) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation,
            service: *service,
        });
    }
    Ok(())
}

fn validate_unit_call_claim_transfers(
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for (argument_index, (argument, parameter)) in arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
    {
        let caller_has_claim = caller
            .entry_claims
            .iter()
            .any(|claim| claim.input == argument.place);
        let callee_has_claim = callee
            .entry_claims
            .iter()
            .any(|claim| claim.input == parameter.place);
        if caller_has_claim != callee_has_claim {
            return Err(ModuleError::UnitCallClaimPresenceMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == argument.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == parameter.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return Err(ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    for (claim, input) in &callee_claims {
        if !callee
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *input)
        {
            return Err(ModuleError::UnitCallClaimHasNoStructuralArgument {
                operation,
                claim: *claim,
            });
        }
    }
    if transfers.len() != callee_claims.len() {
        return Err(ModuleError::UnitCallClaimTransferCountMismatch {
            operation,
            expected: callee_claims.len(),
            actual: transfers.len(),
        });
    }
    let mut caller_claims = BTreeSet::new();
    for transfer in transfers {
        if !caller_claims.insert(transfer.claim) {
            return Err(ModuleError::DuplicateUnitCallClaimTransfer(operation));
        }
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: transfer.argument_index,
            });
        };
        let Some(claim_input) = claim_input(caller, transfer.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        if claim_input != argument.place {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: transfer.claim,
                argument_index: transfer.argument_index,
            });
        }
    }
    for input in callee_claims.into_values() {
        let argument_index = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .expect("callee entry claims were validated against its signature")
            as u32;
        if !transfers
            .iter()
            .any(|transfer| transfer.argument_index == argument_index)
        {
            return Err(ModuleError::MissingUnitCallClaimTransfer {
                operation,
                argument_index,
            });
        }
    }
    if transfers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalUnitCallClaimTransfers(operation));
    }
    Ok(())
}

fn claim_input(machine: &TerminalMachine, claim: ClaimId) -> Option<PlaceId> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| (candidate.claim == claim).then_some(candidate.input))
        .or_else(|| {
            machine
                .content_entry_claims
                .iter()
                .find_map(|candidate| (candidate.claim == claim).then_some(candidate.input.root))
        })
}

fn validate_unit_call_crash_continuations(
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.place, argument.place))
        .collect::<BTreeMap<_, _>>();
    let expected = substitute_crash_route_places(&callee.contract.crash_routes, &substitutions);
    if continuations != expected {
        return Err(ModuleError::CallCrashContinuationsMismatch {
            operation,
            callee: callee.id,
        });
    }
    for continuation in continuations {
        let covered = caller.contract.crash_routes.iter().any(|published| {
            published.cause == continuation.cause
                && (published.alternatives == [CrashRouteGuard::Truth]
                    || continuation
                        .alternatives
                        .iter()
                        .all(|route| published.alternatives.contains(route)))
        });
        if !covered {
            return Err(ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: continuation.cause,
            });
        }
    }
    Ok(())
}

fn substitute_crash_route_places(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, PlaceId>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => {
                        CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                            substitute_proposition_places(predicate.proposition(), substitutions),
                        ))
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            }
        })
        .collect()
}

fn validate_boundary_requirements(
    caller: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[StructuralArgument],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for requirement in &boundary.requires {
        let argument = &arguments[requirement.argument_index as usize];
        let actual = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .expect("structural arguments were validated before requirements");
        if !actual.qualifications.contains(&requirement.domain) {
            return Err(ModuleError::BoundaryArgumentMissingQualification {
                operation,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

fn validate_boundary_settlements(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    settlements: &[ClaimSettlement],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claims
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place).then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for settlement in settlements {
        if !actual.insert((settlement.argument_index, settlement.claim))
            || !claims.insert(settlement.claim)
        {
            return Err(ModuleError::DuplicateBoundaryClaimSettlement(operation));
        }
        let Some(argument) = arguments.get(settlement.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: settlement.argument_index,
            });
        };
        let Some(claim_input) = claim_input(caller, settlement.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: settlement.claim,
            });
        };
        if claim_input != argument.place {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: settlement.claim,
                argument_index: settlement.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryClaimSettlementMismatch(operation));
    }
    if settlements.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryClaimSettlements(operation));
    }
    Ok(())
}

fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}

fn validate_crash_frontiers(
    machine: &TerminalMachine,
    context: &PropositionContext,
    contract_values: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if machine
        .contract
        .crash_routes
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err(ModuleError::NonCanonicalCrashRoutes(machine.id));
    }
    for bucket in &machine.contract.crash_routes {
        if bucket.alternatives.is_empty() {
            return Err(ModuleError::EmptyCrashRouteBucket {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        if bucket
            .alternatives
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                && bucket.alternatives != [CrashRouteGuard::Truth])
        {
            return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                machine: machine.id,
                cause: bucket.cause,
            });
        }
        for guard in &bucket.alternatives {
            let CrashRouteGuard::Predicate(predicate) = guard else {
                continue;
            };
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashRouteAlternatives {
                    machine: machine.id,
                    cause: bucket.cause,
                });
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
            validate_contract_scope(
                predicate.proposition(),
                contract_values,
                machine.contract.id,
                ContractClauseKind::Crash,
            )?;
        }
    }
    for block in &machine.blocks {
        let Terminator::Crash {
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = &block.terminator
        else {
            continue;
        };
        if site_guard.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
        }
        for predicate in site_guard {
            if matches!(
                predicate.proposition(),
                Proposition::Truth | Proposition::Falsehood
            ) {
                return Err(ModuleError::NonCanonicalCrashSiteGuard(block.id));
            }
            context
                .validate(predicate.proposition())
                .map_err(ModuleError::MalformedProposition)?;
        }
        let covered = machine
            .contract
            .crash_routes
            .iter()
            .filter(|bucket| bucket.cause == *cause)
            .any(|bucket| {
                bucket.alternatives.iter().any(|route| match route {
                    CrashRouteGuard::Truth => true,
                    CrashRouteGuard::Predicate(predicate) => site_guard.contains(predicate),
                })
            });
        if !covered {
            return Err(ModuleError::CrashRouteUncovered {
                block: block.id,
                cause: *cause,
            });
        }
        if frontier_lower_bound
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalCrashFrontier(block.id));
        }
    }
    Ok(())
}

fn validate_content_entry_claims(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    for (index, binding) in machine.content_entry_claims.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .expect("an in-memory claim count fits u64")
                .checked_add(1)
                .expect("an in-memory claim count cannot exhaust u64"),
        )
        .expect("dense claim identities begin at one");
        if binding.claim != expected {
            return Err(ModuleError::NonDenseContentEntryClaim {
                expected,
                actual: binding.claim,
            });
        }
        if binding.projections.is_empty() {
            return Err(ModuleError::ContentEntryClaimHasNoProjections(
                binding.claim,
            ));
        }
        if binding
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentEntryProjectionOrder(
                binding.claim,
            ));
        }
        if binding.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&binding.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentEntryClaimRequiresEntryParameter(
                binding.claim,
            ));
        }
        if let Some(structural_claim) = machine
            .entry_claims
            .iter()
            .find(|claim| claim.claim == binding.claim)
            && structural_claim.input != binding.input.root
        {
            return Err(ModuleError::ContentEntryClaimStructuralBindingMismatch(
                binding.claim,
            ));
        }
        if inputs.contains(&binding.input) {
            return Err(ModuleError::DuplicateContentEntryClaimInput(
                binding.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &binding.input))
        {
            return Err(ModuleError::OverlappingContentEntryClaimInput {
                first: previous.clone(),
                second: binding.input.clone(),
            });
        }
        inputs.insert(binding.input.clone());
        for content in &binding.projections {
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            let term = ContentTerm::Projection {
                projection: content.projection,
                subject: binding.input.clone(),
            };
            context
                .validate(&Proposition::ContentConservation(ContentConservation::new(
                    content.algebra.clone(),
                    term.clone(),
                    term,
                )))
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

fn validate_content_identity_reshuffles(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    if machine
        .content_identity_reshuffles
        .windows(2)
        .any(|pair| pair[0].claim >= pair[1].claim)
    {
        return Err(ModuleError::NonCanonicalContentIdentityReshuffles(
            machine.id,
        ));
    }
    let mut claims = BTreeSet::<ClaimId>::new();
    let mut inputs = BTreeSet::<ContentStructuralPlace>::new();
    let mut outputs = BTreeSet::<ContentStructuralPlace>::new();
    for reshuffle in &machine.content_identity_reshuffles {
        insert_unique(&mut claims, reshuffle.claim, ModuleError::DuplicateClaim)?;
        if reshuffle.projections.is_empty() {
            return Err(ModuleError::ContentIdentityReshuffleHasNoProjections(
                reshuffle.claim,
            ));
        }
        let Some(binding) = machine
            .content_entry_claims
            .iter()
            .find(|binding| binding.claim == reshuffle.claim)
        else {
            return Err(ModuleError::ContentIdentityClaimHasNoEntryBinding(
                reshuffle.claim,
            ));
        };
        if binding.input != reshuffle.input || binding.projections != reshuffle.projections {
            return Err(ModuleError::ContentIdentityEntryBindingMismatch(
                reshuffle.claim,
            ));
        }
        if reshuffle
            .projections
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentIdentityProjectionOrder(
                reshuffle.claim,
            ));
        }
        if reshuffle.input.version != psi_core::ContentPlaceVersion::Entry
            || !matches!(
                structural_place_kinds.get(&reshuffle.input.root),
                Some(StructuralPlaceKind::Parameter { .. })
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresEntryParameter(
                reshuffle.claim,
            ));
        }
        if reshuffle.output.version != psi_core::ContentPlaceVersion::Current
            || !matches!(
                structural_place_kinds.get(&reshuffle.output.root),
                Some(StructuralPlaceKind::Result)
            )
        {
            return Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
                reshuffle.claim,
            ));
        }
        if inputs.contains(&reshuffle.input) {
            return Err(ModuleError::DuplicateContentIdentityInput(
                reshuffle.input.clone(),
            ));
        }
        if let Some(previous) = inputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.input))
        {
            return Err(ModuleError::OverlappingContentIdentityInput {
                first: previous.clone(),
                second: reshuffle.input.clone(),
            });
        }
        inputs.insert(reshuffle.input.clone());
        if outputs.contains(&reshuffle.output) {
            return Err(ModuleError::DuplicateContentIdentityOutput(
                reshuffle.output.clone(),
            ));
        }
        if let Some(previous) = outputs
            .iter()
            .find(|previous| content_places_overlap(previous, &reshuffle.output))
        {
            return Err(ModuleError::OverlappingContentIdentityOutput {
                first: previous.clone(),
                second: reshuffle.output.clone(),
            });
        }
        outputs.insert(reshuffle.output.clone());
        for (content, proposition) in reshuffle
            .projections
            .iter()
            .zip(reshuffle.inferred_propositions())
        {
            if let Some(previous) = registry
                .content_projection_algebras
                .insert(content.projection, content.algebra.clone())
                && previous != content.algebra
            {
                return Err(ModuleError::ContentProjectionAlgebraMismatch(
                    content.projection,
                ));
            }
            context
                .validate(&proposition)
                .map_err(ModuleError::MalformedProposition)?;
        }
    }
    Ok(())
}

fn validate_content_partition_compositions(
    machine: &TerminalMachine,
    registry: &mut IdRegistry,
    structural_place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    context: &PropositionContext,
) -> Result<(), ModuleError> {
    let mut rows = BTreeSet::<&ContentPartitionComposition>::new();
    for composition in &machine.content_partition_compositions {
        if !rows.insert(composition) {
            return Err(ModuleError::DuplicateContentPartitionComposition);
        }
        if composition.input_claims.is_empty() {
            return Err(ModuleError::ContentPartitionCompositionHasNoInputClaims);
        }
        if composition
            .input_claims
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionInputClaims);
        }
        if composition
            .substitutions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        if composition.source.algebra() != composition.derived.algebra() {
            return Err(ModuleError::ContentPartitionAlgebraMismatch);
        }
        if !content_term_contains_partition(composition.source.left())
            && !content_term_contains_partition(composition.source.right())
        {
            return Err(ModuleError::ContentPartitionSourceHasNoSeparation);
        }

        let source_kinds = validate_partition_source_places(composition)?;
        let source_context = PropositionContext::from_value_types_and_places(
            [],
            composition
                .source_structural_places
                .iter()
                .map(|place| (place.id, place.kind)),
        )
        .map_err(ModuleError::MalformedProposition)?;
        source_context
            .validate(&Proposition::ContentConservation(
                composition.source.clone(),
            ))
            .map_err(ModuleError::MalformedProposition)?;
        let reconstructed_fingerprint =
            content_conservation_fingerprint(&composition.source, &source_kinds);
        if reconstructed_fingerprint != Some(composition.source_fingerprint) {
            return Err(ModuleError::ContentPartitionSourceFingerprintMismatch {
                recorded: composition.source_fingerprint,
                reconstructed: reconstructed_fingerprint,
            });
        }
        context
            .validate(&composition.inferred_proposition())
            .map_err(ModuleError::MalformedProposition)?;
        register_partition_projections(registry, &composition.source)?;
        register_partition_projections(registry, &composition.derived)?;

        let substitutions = composition
            .substitutions
            .iter()
            .map(|substitution| (substitution.source.clone(), substitution.target.clone()))
            .collect::<BTreeMap<_, _>>();
        if substitutions.len() != composition.substitutions.len() {
            return Err(ModuleError::NonCanonicalContentPartitionSubstitutions);
        }
        let target_count = composition
            .substitutions
            .iter()
            .map(|substitution| &substitution.target)
            .collect::<BTreeSet<_>>()
            .len();
        if target_count != composition.substitutions.len() {
            return Err(ModuleError::DuplicateContentPartitionSubstitutionTarget);
        }
        let source_subjects = content_conservation_subjects(&composition.source);
        if source_subjects
            != substitutions
                .keys()
                .cloned()
                .collect::<BTreeSet<ContentStructuralPlace>>()
        {
            return Err(ModuleError::ContentPartitionSubstitutionCoverageMismatch);
        }
        for substitution in &composition.substitutions {
            validate_partition_substitution_shape(
                substitution,
                &source_kinds,
                structural_place_kinds,
            )?;
        }
        let replayed = replay_partition_conservation(&composition.source, &substitutions)?;
        if replayed != composition.derived {
            return Err(ModuleError::ContentPartitionReplayMismatch);
        }

        let listed_claims = composition
            .input_claims
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used_claims = BTreeSet::new();
        for (projection, subject) in content_conservation_projections(&composition.derived) {
            if subject.version != psi_core::ContentPlaceVersion::Entry {
                continue;
            }
            let matching = machine
                .content_entry_claims
                .iter()
                .filter(|binding| {
                    binding.input == subject
                        && binding.projections.iter().any(|content| {
                            content.projection == projection
                                && content.algebra == *composition.derived.algebra()
                        })
                })
                .map(|binding| binding.claim)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return Err(ModuleError::ContentPartitionInputProjectionNotClaimBound(
                    subject,
                ));
            };
            if !listed_claims.contains(claim) {
                return Err(ModuleError::ContentPartitionInputClaimNotListed(*claim));
            }
            used_claims.insert(*claim);
        }
        if used_claims != listed_claims {
            return Err(ModuleError::ContentPartitionInputClaimUnused);
        }
    }
    Ok(())
}

fn validate_partition_source_places(
    composition: &ContentPartitionComposition,
) -> Result<BTreeMap<PlaceId, StructuralPlaceKind>, ModuleError> {
    let mut ids = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for place in &composition.source_structural_places {
        if ids.insert(place.id, place.kind).is_some() {
            return Err(ModuleError::DuplicateContentPartitionSourcePlace(place.id));
        }
        let root = match place.kind {
            StructuralPlaceKind::Parameter { position, .. } => {
                StructuralRootKey::Parameter(position)
            }
            StructuralPlaceKind::Result => StructuralRootKey::Result,
        };
        if !roots.insert(root) {
            return Err(ModuleError::DuplicateContentPartitionSourceRoot(place.kind));
        }
    }
    Ok(ids)
}

fn validate_partition_substitution_shape(
    substitution: &psi_terminal::ContentPlaceSubstitution,
    source_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
    target_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Result<(), ModuleError> {
    match (
        substitution.source.version,
        source_kinds.get(&substitution.source.root),
        substitution.target.version,
        target_kinds.get(&substitution.target.root),
    ) {
        (
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
            psi_core::ContentPlaceVersion::Entry,
            Some(StructuralPlaceKind::Parameter { .. }),
        )
        | (
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
            psi_core::ContentPlaceVersion::Current,
            Some(StructuralPlaceKind::Result),
        ) => Ok(()),
        _ => Err(ModuleError::InvalidContentPartitionSubstitutionShape),
    }
}

fn replay_partition_conservation(
    source: &ContentConservation,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentConservation, ModuleError> {
    Ok(ContentConservation::new(
        source.algebra().clone(),
        replay_partition_term(source.left(), substitutions)?,
        replay_partition_term(source.right(), substitutions)?,
    ))
}

fn replay_partition_term(
    term: &ContentTerm,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentTerm, ModuleError> {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => Ok(ContentTerm::Projection {
            projection: *projection,
            subject: substitutions
                .get(subject)
                .cloned()
                .ok_or(ModuleError::ContentPartitionSubstitutionCoverageMismatch)?,
        }),
        ContentTerm::Separate(terms) => ContentTerm::separate(
            terms
                .iter()
                .map(|term| replay_partition_term(term, substitutions))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(ModuleError::MalformedProposition),
    }
}

fn content_term_contains_partition(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { .. } => false,
        ContentTerm::Separate(_) => true,
    }
}

fn content_conservation_subjects(
    conservation: &ContentConservation,
) -> BTreeSet<ContentStructuralPlace> {
    content_conservation_projections(conservation)
        .into_iter()
        .map(|(_, subject)| subject)
        .collect()
}

fn content_conservation_projections(
    conservation: &ContentConservation,
) -> Vec<(ContentProjectionIdentity, ContentStructuralPlace)> {
    fn collect(
        term: &ContentTerm,
        projections: &mut Vec<(ContentProjectionIdentity, ContentStructuralPlace)>,
    ) {
        match term {
            ContentTerm::Projection {
                projection,
                subject,
            } => projections.push((*projection, subject.clone())),
            ContentTerm::Separate(terms) => {
                for term in terms {
                    collect(term, projections);
                }
            }
        }
    }
    let mut projections = Vec::new();
    collect(conservation.left(), &mut projections);
    collect(conservation.right(), &mut projections);
    projections
}

fn register_partition_projections(
    registry: &mut IdRegistry,
    conservation: &ContentConservation,
) -> Result<(), ModuleError> {
    for (projection, _) in content_conservation_projections(conservation) {
        if let Some(previous) = registry
            .content_projection_algebras
            .insert(projection, conservation.algebra().clone())
            && previous != *conservation.algebra()
        {
            return Err(ModuleError::ContentProjectionAlgebraMismatch(projection));
        }
    }
    Ok(())
}

fn content_places_overlap(left: &ContentStructuralPlace, right: &ContentStructuralPlace) -> bool {
    if left.version != right.version || left.root != right.root {
        return false;
    }
    let shared = left.segments.len().min(right.segments.len());
    left.segments[..shared] == right.segments[..shared]
}

fn validate_contract_clause_kind(
    proposition: &Proposition,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_contract_clause_kind(conjunct, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_clause_kind(premise, contract, clause)?;
            validate_contract_clause_kind(conclusion, contract, clause)
        }
        Proposition::ContentConservation(_) if clause == ContractClauseKind::Requires => {
            Err(ModuleError::ContentConservationRequiresEnsures { contract })
        }
        _ => Ok(()),
    }
}

fn validate_contract_scope(
    proposition: &Proposition,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)
        }
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_contract_scope(conjunct, allowed, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_scope(premise, allowed, contract, clause)?;
            validate_contract_scope(conclusion, allowed, contract, clause)
        }
        Proposition::ContentConservation(_) => Ok(()),
    }
}

fn validate_term_scope(
    term: &ScalarTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::Value { id, .. } => {
            if !allowed.contains(id) {
                return Err(ModuleError::ContractValueOutsideScope {
                    contract,
                    clause,
                    value: *id,
                });
            }
        }
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
        | ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            validate_term_scope(value, allowed, contract, clause)?;
            validate_term_scope(count, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => {
            validate_term_scope(operand, allowed, contract, clause)?;
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralOwnershipFrontier {
    // Claims carry proof-visible custody identity. Owned places independently
    // enforce by-value affine/linear use even when no linear claim row exists.
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
}

fn validate_structural_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
) -> Result<(), ModuleError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &machine.entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("entry claims were validated against structural parameters");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                multiplicity: Some(parameter.multiplicity),
            },
        );
    }
    for claim in &machine.content_entry_claims {
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: None,
            multiplicity: None,
        });
    }
    let entry = StructuralOwnershipFrontier {
        claims,
        owned_places: machine
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
    };

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            *predecessors
                .get_mut(target)
                .expect("control validation established every target") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = predecessors
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = predecessors
                .get_mut(target)
                .expect("control validation established every target");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }

    let mut incoming = BTreeMap::<BlockId, Vec<StructuralOwnershipFrontier>>::new();
    incoming.insert(machine.entry, vec![entry]);
    for block_id in order {
        let frontiers = incoming
            .remove(&block_id)
            .expect("control validation established reachability");
        let frontier = frontiers
            .first()
            .expect("a reachable block has an incoming frontier")
            .clone();
        if frontiers
            .iter()
            .any(|candidate| candidate.claims != frontier.claims)
        {
            return Err(ModuleError::ClaimFrontierJoinMismatch(block_id));
        }
        if frontiers
            .iter()
            .any(|candidate| candidate.owned_places != frontier.owned_places)
        {
            return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block_id));
        }
        let block = blocks
            .get(&block_id)
            .expect("topological order contains known blocks");
        let mut frontier = frontier;
        for operation in &block.operations {
            let claims = match &operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                } => claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>(),
                OperationKind::BoundaryCallUnit {
                    claim_settlements, ..
                } => claim_settlements
                    .iter()
                    .map(|settlement| settlement.claim)
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            for claim in claims {
                if frontier.claims.remove(&claim).is_none() {
                    return Err(ModuleError::ClaimNotLiveAtOperation {
                        operation: operation.id,
                        claim,
                    });
                }
            }
            let consumed_places = match &operation.kind {
                OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    ..
                } => structural_arguments
                    .iter()
                    .zip(&machines[callee].structural_parameters)
                    .filter_map(|(argument, parameter)| {
                        (parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                            .then_some(argument.place)
                    })
                    .collect::<Vec<_>>(),
                OperationKind::BoundaryCallUnit {
                    boundary,
                    structural_arguments,
                    ..
                } => {
                    let boundary = module
                        .boundary_machines
                        .iter()
                        .find(|candidate| candidate.id == *boundary)
                        .expect("static validation established the boundary target");
                    structural_arguments
                        .iter()
                        .zip(&boundary.structural_parameters)
                        .filter_map(|(argument, parameter)| {
                            (parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                                .then_some(argument.place)
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            for place in consumed_places {
                if frontier.owned_places.remove(&place).is_none() {
                    return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                        operation: operation.id,
                        place,
                    });
                }
            }
        }
        match &block.terminator {
            Terminator::Jump { target, .. } => {
                incoming.entry(*target).or_default().push(frontier);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                incoming
                    .entry(when_true.target)
                    .or_default()
                    .push(frontier.clone());
                incoming
                    .entry(when_false.target)
                    .or_default()
                    .push(frontier);
            }
            Terminator::ReturnUnit { .. } => {
                if let Some((claim, _)) = frontier
                    .claims
                    .iter()
                    .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
                {
                    return Err(ModuleError::LiveLinearClaimAtUnitReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
            }
            Terminator::Return { .. } => {
                if let Some((claim, _)) = frontier
                    .claims
                    .iter()
                    .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
                {
                    return Err(ModuleError::LiveLinearClaimAtScalarReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
            }
            Terminator::Crash {
                frontier_lower_bound,
                ..
            } => {
                let expected = frontier.claims.keys().copied().collect::<Vec<_>>();
                if frontier_lower_bound != &expected {
                    return Err(ModuleError::CrashFrontierMismatch { block: block.id });
                }
            }
        }
    }
    Ok(())
}

fn validate_control_flow(
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let globally_defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut definition_blocks = BTreeMap::new();
    for block in blocks.values() {
        for parameter in &block.parameters {
            definition_blocks.insert(parameter.id, block.id);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                definition_blocks.insert(result.id, block.id);
            }
        }
    }

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, Vec::<BlockId>::new()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            if !blocks.contains_key(target) {
                return Err(ModuleError::UnknownTargetBlock(*target));
            }
            predecessors
                .get_mut(target)
                .expect("known target has a predecessor row")
                .push(block.id);
        }
        successors.insert(block.id, targets);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(
                successors
                    .get(&block)
                    .expect("every block has successors")
                    .iter()
                    .copied(),
            );
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different set lengths guarantee an unreachable block");
        return Err(ModuleError::UnreachableBlock(block));
    }

    let mut indegree = predecessors
        .iter()
        .map(|(block, incoming)| (*block, incoming.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in successors.get(&block).expect("every block has successors") {
            let count = indegree
                .get_mut(target)
                .expect("known target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if order.len() != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(ModuleError::ControlCycle(block));
    }

    let mut dominators = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    for block in &order {
        let incoming = predecessors
            .get(block)
            .expect("every block has predecessors");
        let mut set = if *block == machine.entry {
            BTreeSet::new()
        } else {
            let mut incoming = incoming.iter();
            let first = incoming
                .next()
                .expect("reachable non-entry block has a predecessor");
            let mut intersection = dominators
                .get(first)
                .expect("topological predecessor has dominators")
                .clone();
            for predecessor in incoming {
                intersection = intersection
                    .intersection(
                        dominators
                            .get(predecessor)
                            .expect("topological predecessor has dominators"),
                    )
                    .copied()
                    .collect();
            }
            intersection
        };
        set.insert(*block);
        dominators.insert(*block, set);
    }

    for block_id in order {
        let block = blocks
            .get(&block_id)
            .copied()
            .expect("topological order contains known blocks");
        let block_dominators = dominators
            .get(&block_id)
            .expect("every ordered block has dominators");
        let mut defined = globally_defined.clone();
        defined.extend(block.parameters.iter().map(|parameter| parameter.id));
        defined.extend(definition_blocks.iter().filter_map(|(value, definition)| {
            (*definition != block_id && block_dominators.contains(definition)).then_some(*value)
        }));
        for operation in &block.operations {
            validate_operation_operands(operation, machines, value_types, &defined)?;
            if let Some(result) = operation.result.scalar() {
                defined.insert(result.id);
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
            } => validate_successor_bindings(
                *edge,
                *target,
                arguments,
                blocks,
                value_types,
                &defined,
            )?,
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                require_defined(*condition, value_types, &defined)?;
                let actual = value_types[condition];
                if actual != ScalarType::Boolean {
                    return Err(ModuleError::ConditionalConditionTypeMismatch {
                        block: block.id,
                        condition: *condition,
                        actual,
                    });
                }
                for successor in [when_true, when_false] {
                    validate_successor_bindings(
                        successor.edge,
                        successor.target,
                        &successor.arguments,
                        blocks,
                        value_types,
                        &defined,
                    )?;
                }
            }
            Terminator::Return { value, .. } => {
                let Some(result) = machine.result.scalar() else {
                    return Err(ModuleError::ScalarReturnFromUnitMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                require_defined(*value, value_types, &defined)?;
                let value_type = value_types[value];
                if value_type != result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: result.scalar_type,
                    });
                }
            }
            Terminator::ReturnUnit { .. } => {
                if matches!(machine.result, TerminalMachineResult::Scalar(_)) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::Crash { site_guard, .. } => {
                for predicate in site_guard {
                    validate_contract_scope(
                        predicate.proposition(),
                        &defined,
                        machine.contract.id,
                        ContractClauseKind::Crash,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_operation_operands(
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if let OperationKind::Call {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("call target was validated during operation registration");
        if arguments.len() != callee.parameters.len() {
            return Err(ModuleError::CallArgumentArityMismatch {
                operation: operation.id,
                expected: callee.parameters.len(),
                actual: arguments.len(),
            });
        }
        for (argument, parameter) in arguments.iter().zip(&callee.parameters) {
            require_defined(*argument, value_types, defined)?;
            let actual = value_types[argument];
            if actual != parameter.scalar_type {
                return Err(ModuleError::CallArgumentTypeMismatch {
                    operation: operation.id,
                    argument: *argument,
                    expected: parameter.scalar_type,
                    actual,
                });
            }
        }
        return Ok(());
    }
    if let OperationKind::IntegerExactCast { operand, .. } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = operation.result.expect_scalar().scalar_type;
        let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
            return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        };
        if !source.can_exact_cast_to(target) || source.can_widen_to(target) || source == target {
            return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerWiden { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = operation.result.expect_scalar().scalar_type;
        let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
            return Err(ModuleError::IntegerWidenOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        };
        if !source.can_widen_to(target) {
            return Err(ModuleError::IntegerWidenOperandTypeMismatch {
                operation: operation.id,
                source: actual,
                target: expected,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseNot { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual = value_types[&operand];
        if !matches!(expected, ScalarType::Integer(_)) || actual != expected {
            return Err(ModuleError::IntegerBitwiseNotOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::BooleanNot { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != ScalarType::Boolean {
            return Err(ModuleError::BooleanNotOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::BooleanEqual { left, right } = operation.kind.clone() {
        for operand in [left, right] {
            require_defined(operand, value_types, defined)?;
            let actual = value_types[&operand];
            if actual != ScalarType::Boolean {
                return Err(ModuleError::BooleanEqualOperandTypeMismatch {
                    operation: operation.id,
                    operand,
                    actual,
                });
            }
        }
        return Ok(());
    }
    if let OperationKind::IntegerEqual { left, right } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
            return Err(ModuleError::IntegerEqualOperandTypeMismatch {
                operation: operation.id,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerLessThan { left, right }
    | OperationKind::IntegerLessOrEqual { left, right } = operation.kind.clone()
    {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
            return Err(ModuleError::IntegerOrderingOperandTypeMismatch {
                operation: operation.id,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseAnd { left, right }
    | OperationKind::IntegerBitwiseOr { left, right }
    | OperationKind::IntegerBitwiseXor { left, right } = operation.kind.clone()
    {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let left_type = value_types[&left];
        let right_type = value_types[&right];
        if !matches!(expected, ScalarType::Integer(_))
            || left_type != expected
            || right_type != expected
        {
            return Err(ModuleError::IntegerBitwiseOperandTypeMismatch {
                operation: operation.id,
                expected,
                left: left_type,
                right: right_type,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerShiftLeft { value, count }
    | OperationKind::WrappingIntegerShiftRight { value, count } = operation.kind.clone()
    {
        require_defined(value, value_types, defined)?;
        require_defined(count, value_types, defined)?;
        let expected_value = operation.result.expect_scalar().scalar_type;
        let actual_value = value_types[&value];
        let actual_count = value_types[&count];
        if !matches!(expected_value, ScalarType::Integer(_))
            || actual_value != expected_value
            || !matches!(actual_count, ScalarType::Integer(_))
        {
            return Err(ModuleError::WrappingIntegerShiftOperandTypeMismatch {
                operation: operation.id,
                expected_value,
                actual_value,
                actual_count,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerShiftLeft { value, count, .. }
    | OperationKind::ExactIntegerShiftRight { value, count, .. } = operation.kind.clone()
    {
        require_defined(value, value_types, defined)?;
        require_defined(count, value_types, defined)?;
        let expected_value = operation.result.expect_scalar().scalar_type;
        let actual_value = value_types[&value];
        let actual_count = value_types[&count];
        if !matches!(expected_value, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_value != expected_value
            || !matches!(actual_count, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
        {
            return Err(ModuleError::ExactIntegerShiftOperandTypeMismatch {
                operation: operation.id,
                expected_value,
                actual_value,
                actual_count,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerAdd { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerAddOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerSubtract { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerSubtractOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerMultiply { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerMultiplyOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::ExactIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::WrappingIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::WrappingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::WrappingIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::SaturatingIntegerDivideOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_left != expected
            || actual_right != expected
        {
            return Err(ModuleError::SaturatingIntegerRemainderOperandTypeMismatch {
                operation: operation.id,
                expected,
                actual_left,
                actual_right,
            });
        }
        return Ok(());
    }
    let Some((left, right, arithmetic)) = (match operation.kind.clone() {
        OperationKind::WrappingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingAdd))
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingAdd))
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingSubtract))
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingSubtract))
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingMultiply))
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingMultiply))
        }
        OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerExactCast { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. } => None,
        OperationKind::ExactIntegerDivide { .. } => None,
        OperationKind::ExactIntegerRemainder { .. } => None,
        OperationKind::WrappingIntegerDivide { .. } => None,
        OperationKind::WrappingIntegerRemainder { .. } => None,
        OperationKind::SaturatingIntegerDivide { .. } => None,
        OperationKind::SaturatingIntegerRemainder { .. } => None,
        OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::BoundaryCallUnit { .. }
        | OperationKind::PortWrite { .. } => None,
    }) else {
        return Ok(());
    };
    let ScalarType::Integer(integer_type) = operation.result.expect_scalar().scalar_type else {
        unreachable!("operation shape validation requires an integer result")
    };
    for operand in [left, right] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = ScalarType::Integer(integer_type);
        if actual != expected {
            return Err(match arithmetic {
                ArithmeticOperandKind::SaturatingAdd => {
                    ModuleError::SaturatingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingAdd => {
                    ModuleError::WrappingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingSubtract => {
                    ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingSubtract => {
                    ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingMultiply => {
                    ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingMultiply => {
                    ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
            });
        }
    }
    Ok(())
}

fn proposition_contains_content(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::ContentConservation(_) => true,
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_contains_content),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_contains_content(premise) || proposition_contains_content(conclusion),
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::Equal(_, _)
        | Proposition::LessThan(_, _)
        | Proposition::LessOrEqual(_, _) => false,
    }
}

fn proposition_content_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn collect_term(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                roots.insert(subject.root);
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    collect_term(term, roots);
                }
            }
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::ContentConservation(conservation) => {
                collect_term(conservation.left(), roots);
                collect_term(conservation.right(), roots);
            }
            Proposition::Conjunction(conjuncts) => {
                for conjunct in conjuncts {
                    collect(conjunct, roots);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, roots);
                collect(conclusion, roots);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::Equal(_, _)
            | Proposition::LessThan(_, _)
            | Proposition::LessOrEqual(_, _) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
}

fn require_defined(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if !defined.contains(&value) {
        return Err(ModuleError::ValueUsedBeforeDefinition(value));
    }
    if !value_types.contains_key(&value) {
        return Err(ModuleError::UnknownValue(value));
    }
    Ok(())
}

fn validate_successor_bindings(
    edge: EdgeId,
    target: BlockId,
    arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let target_block = blocks
        .get(&target)
        .copied()
        .ok_or(ModuleError::UnknownTargetBlock(target))?;
    if target_block.parameters.len() != arguments.len() {
        return Err(ModuleError::JumpArityMismatch {
            edge,
            expected: target_block.parameters.len(),
            actual: arguments.len(),
        });
    }
    for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
        require_defined(*argument, value_types, defined)?;
        let argument_type = value_types[argument];
        if argument_type != parameter.scalar_type {
            return Err(ModuleError::JumpTypeMismatch {
                edge,
                argument: argument_type,
                parameter: parameter.scalar_type,
            });
        }
    }
    Ok(())
}

enum ArithmeticOperandKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

pub(crate) fn machine_value_types(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
    machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
}

fn insert_value(
    values: &mut BTreeMap<ValueId, ScalarType>,
    module_values: &mut BTreeSet<ValueId>,
    id: ValueId,
    scalar_type: ScalarType,
) -> Result<(), ModuleError> {
    if values.insert(id, scalar_type).is_some() || !module_values.insert(id) {
        return Err(ModuleError::DuplicateValue(id));
    }
    Ok(())
}

fn insert_unique<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    value: T,
    error: impl FnOnce(T) -> ModuleError,
) -> Result<(), ModuleError> {
    if !set.insert(value) {
        return Err(error(value));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    EmptyModule,
    DuplicatePropositionDeclaration(PropositionId),
    DuplicatePropositionApplication(PropositionId),
    NonDensePropositionDeclaration {
        expected: PropositionId,
        actual: PropositionId,
    },
    NonDensePropositionApplication {
        expected: PropositionId,
        actual: PropositionId,
    },
    DuplicatePropositionName(String),
    UnknownPropositionDeclaration(PropositionId),
    InvalidPropositionBinder(PropositionId),
    PropositionApplicationArityMismatch(PropositionId),
    PropositionApplicationBinderMismatch(PropositionId),
    EmptyPropositionIdentity,
    DuplicateMachine(MachineId),
    DuplicateStructuralType(StructuralTypeId),
    InvalidStructuralTypeIdentity(StructuralTypeId),
    InvalidStructuralFieldIdentity {
        structural_type: StructuralTypeId,
        field: psi_core::StructuralFieldId,
    },
    InvalidErasedStructuralField {
        structural_type: StructuralTypeId,
        field: psi_core::StructuralFieldId,
    },
    UnknownStructuralType(StructuralTypeId),
    RecursiveStructuralType(StructuralTypeId),
    DuplicateStructuralDomain(StructuralDomainId),
    InvalidStructuralDomainIdentity(StructuralDomainId),
    UnknownStructuralDomain(StructuralDomainId),
    StructuralDomainCarrierMismatch {
        domain: StructuralDomainId,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    DuplicateService(ServiceId),
    InvalidServiceIdentity(ServiceId),
    InvalidServiceParent {
        service: ServiceId,
        parent: ServiceId,
    },
    NonCanonicalServiceParents(ServiceId),
    RecursiveServiceHierarchy(ServiceId),
    IncompleteServiceParentClosure {
        service: ServiceId,
        ancestor: ServiceId,
    },
    DuplicateBoundaryMachine(BoundaryMachineId),
    InvalidBoundaryMachineIdentity(BoundaryMachineId),
    UnknownMachineAttachment {
        machine: MachineId,
        attachment: StructuralTypeId,
    },
    UnknownBoundaryAttachment {
        boundary: BoundaryMachineId,
        attachment: StructuralTypeId,
    },
    NonDenseStructuralParameter {
        owner: StructuralSignatureOwner,
        expected: u32,
        actual: u32,
    },
    DuplicateStructuralParameterPlace(PlaceId),
    InvalidStructuralSelfParameter {
        owner: StructuralSignatureOwner,
    },
    DuplicateStructuralQualification {
        place: PlaceId,
        domain: StructuralDomainId,
    },
    NonCanonicalStructuralQualifications(PlaceId),
    DuplicatePublishedService {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    NonCanonicalPublishedServiceCeiling(ServiceCeilingOwner),
    UnknownPublishedService {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    IncompletePublishedServiceClosure {
        owner: ServiceCeilingOwner,
        service: ServiceId,
    },
    BoundaryRequirementArgumentOutOfRange {
        boundary: BoundaryMachineId,
        argument_index: u32,
    },
    DuplicateBoundaryRequirement {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    NonCanonicalBoundaryRequirements(BoundaryMachineId),
    StructuralParameterPlaceMismatch {
        machine: MachineId,
        place: PlaceId,
    },
    StructuralPlaceHasNoParameter {
        machine: MachineId,
        place: PlaceId,
    },
    DuplicateEntryClaimInput(PlaceId),
    EntryClaimRequiresStructuralParameter(ClaimId),
    EntryClaimRequiresOwnedParameter(ClaimId),
    LinearParameterHasNoEntryClaim {
        machine: MachineId,
        place: PlaceId,
    },
    DuplicateBlock(BlockId),
    DuplicateContract(ContractId),
    DuplicateOperation(OperationId),
    ScalarOperationHasUnitResult(OperationId),
    UnitOperationHasScalarResult(OperationId),
    UnitCallTargetHasScalarSignature {
        operation: OperationId,
        callee: MachineId,
    },
    UnitCallContractPlaceHasNoArgument {
        operation: OperationId,
        callee: MachineId,
        place: PlaceId,
    },
    UnknownBoundaryCallTarget {
        operation: OperationId,
        boundary: BoundaryMachineId,
    },
    BoundaryStructuralRequirementsMintObligations(OperationId),
    StructuralArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    UnknownStructuralArgument {
        operation: OperationId,
        argument_index: u32,
        place: PlaceId,
    },
    StructuralArgumentTypeMismatch {
        operation: OperationId,
        argument_index: u32,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    StructuralArgumentMultiplicityMismatch {
        operation: OperationId,
        argument_index: u32,
        expected: StructuralMultiplicity,
        actual: StructuralMultiplicity,
    },
    StructuralArgumentMissingQualification {
        operation: OperationId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    UnknownOperationService {
        operation: OperationId,
        service: ServiceId,
    },
    OperationServiceOutsidePublishedCeiling {
        operation: OperationId,
        service: ServiceId,
    },
    UnitCallClaimTransferCountMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    UnitCallClaimHasNoStructuralArgument {
        operation: OperationId,
        claim: ClaimId,
    },
    UnitCallClaimPresenceMismatch {
        operation: OperationId,
        argument_index: u32,
    },
    UnitCallContentClaimMismatch {
        operation: OperationId,
        argument_index: u32,
    },
    DuplicateUnitCallClaimTransfer(OperationId),
    NonCanonicalUnitCallClaimTransfers(OperationId),
    MissingUnitCallClaimTransfer {
        operation: OperationId,
        argument_index: u32,
    },
    ClaimActionArgumentOutOfRange {
        operation: OperationId,
        argument_index: u32,
    },
    UnknownClaimAtOperation {
        operation: OperationId,
        claim: ClaimId,
    },
    ClaimActionPlaceMismatch {
        operation: OperationId,
        claim: ClaimId,
        argument_index: u32,
    },
    BoundaryArgumentMissingQualification {
        operation: OperationId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    DuplicateBoundaryClaimSettlement(OperationId),
    NonCanonicalBoundaryClaimSettlements(OperationId),
    BoundaryClaimSettlementMismatch(OperationId),
    ClaimNotLiveAtOperation {
        operation: OperationId,
        claim: ClaimId,
    },
    OwnedStructuralPlaceNotLiveAtOperation {
        operation: OperationId,
        place: PlaceId,
    },
    ClaimFrontierJoinMismatch(BlockId),
    OwnedStructuralFrontierJoinMismatch(BlockId),
    LiveLinearClaimAtUnitReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    LiveLinearClaimAtScalarReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    DuplicateEdge(EdgeId),
    DuplicateObligation(ObligationId),
    NonCanonicalContractEnsures(ContractId),
    DuplicateValue(ValueId),
    DuplicatePlace(PlaceId),
    DuplicateClaim(ClaimId),
    NonDenseStructuralEntryClaim {
        machine: MachineId,
        expected: ClaimId,
        actual: ClaimId,
    },
    DuplicateStructuralPlaceRoot {
        machine: MachineId,
        kind: psi_core::StructuralPlaceKind,
    },
    UnitMachineHasResultStructuralPlace {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownEntryMachine(MachineId),
    MachineHasNoBlocks(MachineId),
    UnknownEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockCannotHaveParameters(BlockId),
    ContractValueOutsideScope {
        contract: ContractId,
        clause: ContractClauseKind,
        value: ValueId,
    },
    NonCanonicalCrashRoutes(MachineId),
    EmptyCrashRouteBucket {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashRouteAlternatives {
        machine: MachineId,
        cause: CrashCause,
    },
    NonCanonicalCrashSiteGuard(BlockId),
    CrashRouteUncovered {
        block: BlockId,
        cause: CrashCause,
    },
    NonCanonicalCrashFrontier(BlockId),
    CrashFrontierMismatch {
        block: BlockId,
    },
    NonDenseContentEntryClaim {
        expected: ClaimId,
        actual: ClaimId,
    },
    ContentEntryClaimHasNoProjections(ClaimId),
    NonCanonicalContentEntryProjectionOrder(ClaimId),
    ContentEntryClaimRequiresEntryParameter(ClaimId),
    ContentEntryClaimStructuralBindingMismatch(ClaimId),
    DuplicateContentEntryClaimInput(ContentStructuralPlace),
    OverlappingContentEntryClaimInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentIdentityReshuffleHasNoProjections(ClaimId),
    ContentIdentityClaimHasNoEntryBinding(ClaimId),
    ContentIdentityEntryBindingMismatch(ClaimId),
    NonCanonicalContentIdentityProjectionOrder(ClaimId),
    NonCanonicalContentIdentityReshuffles(MachineId),
    ContentIdentityReshuffleRequiresEntryParameter(ClaimId),
    ContentIdentityReshuffleRequiresCurrentResult(ClaimId),
    DuplicateContentIdentityInput(ContentStructuralPlace),
    DuplicateContentIdentityOutput(ContentStructuralPlace),
    OverlappingContentIdentityInput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    OverlappingContentIdentityOutput {
        first: ContentStructuralPlace,
        second: ContentStructuralPlace,
    },
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    DuplicateContentPartitionComposition,
    ContentPartitionCompositionHasNoInputClaims,
    NonCanonicalContentPartitionInputClaims,
    NonCanonicalContentPartitionSubstitutions,
    DuplicateContentPartitionSubstitutionTarget,
    ContentPartitionAlgebraMismatch,
    ContentPartitionSourceHasNoSeparation,
    ContentPartitionSourceFingerprintMismatch {
        recorded: u64,
        reconstructed: Option<u64>,
    },
    DuplicateContentPartitionSourcePlace(PlaceId),
    DuplicateContentPartitionSourceRoot(StructuralPlaceKind),
    InvalidContentPartitionSubstitutionShape,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    ContentPartitionInputProjectionNotClaimBound(ContentStructuralPlace),
    ContentPartitionInputClaimNotListed(ClaimId),
    ContentPartitionInputClaimUnused,
    ContentConservationRequiresEnsures {
        contract: ContractId,
    },
    UnknownTargetBlock(BlockId),
    UnknownValue(ValueId),
    ValueUsedBeforeDefinition(ValueId),
    UnknownCallTarget {
        operation: OperationId,
        callee: MachineId,
    },
    NonCanonicalCallCrashContinuations(OperationId),
    CallCrashContinuationsMismatch {
        operation: OperationId,
        callee: MachineId,
    },
    CallCrashContinuationUncovered {
        operation: OperationId,
        cause: CrashCause,
    },
    CallTargetHasStructuralContract {
        operation: OperationId,
        callee: MachineId,
    },
    CallTargetReturnsUnit {
        operation: OperationId,
        callee: MachineId,
    },
    CallResultTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallArgumentArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    CallArgumentTypeMismatch {
        operation: OperationId,
        argument: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    CallRequirementArityMismatch {
        operation: OperationId,
        expected: usize,
        actual: usize,
    },
    RecursiveCallSliceNotYetSupported(MachineId),
    IntegerConstantRequiresIntegerResult(OperationId),
    IntegerConstantOutsideResultType(OperationId),
    BooleanConstantRequiresBooleanResult(OperationId),
    BooleanNotRequiresBooleanResult(OperationId),
    BooleanNotOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    BooleanEqualRequiresBooleanResult(OperationId),
    BooleanEqualOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        actual: ScalarType,
    },
    IntegerEqualRequiresBooleanResult(OperationId),
    IntegerEqualOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerOrderingRequiresBooleanResult(OperationId),
    IntegerOrderingOperandTypeMismatch {
        operation: OperationId,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerBitwiseRequiresIntegerResult(OperationId),
    IntegerWidenRequiresIntegerResult(OperationId),
    IntegerWidenOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerExactCastRequiresIntegerResult(OperationId),
    IntegerExactCastOperandTypeMismatch {
        operation: OperationId,
        source: ScalarType,
        target: ScalarType,
    },
    IntegerBitwiseNotOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual: ScalarType,
    },
    IntegerBitwiseOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerShiftRequiresIntegerResult(OperationId),
    WrappingIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerShiftRequiresIntegerResult(OperationId),
    ExactIntegerShiftOperandTypeMismatch {
        operation: OperationId,
        expected_value: ScalarType,
        actual_value: ScalarType,
        actual_count: ScalarType,
    },
    ExactIntegerAddRequiresIntegerResult(OperationId),
    ExactIntegerAddOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerSubtractRequiresIntegerResult(OperationId),
    ExactIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerMultiplyRequiresIntegerResult(OperationId),
    ExactIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerDivideRequiresIntegerResult(OperationId),
    ExactIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    ExactIntegerRemainderRequiresIntegerResult(OperationId),
    ExactIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerDivideRequiresIntegerResult(OperationId),
    WrappingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerRemainderRequiresIntegerResult(OperationId),
    WrappingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerDivideRequiresIntegerResult(OperationId),
    SaturatingIntegerDivideOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    SaturatingIntegerRemainderRequiresIntegerResult(OperationId),
    SaturatingIntegerRemainderOperandTypeMismatch {
        operation: OperationId,
        expected: ScalarType,
        actual_left: ScalarType,
        actual_right: ScalarType,
    },
    WrappingIntegerAddRequiresIntegerResult(OperationId),
    SaturatingIntegerAddRequiresIntegerResult(OperationId),
    WrappingIntegerSubtractRequiresIntegerResult(OperationId),
    SaturatingIntegerSubtractRequiresIntegerResult(OperationId),
    WrappingIntegerMultiplyRequiresIntegerResult(OperationId),
    SaturatingIntegerMultiplyRequiresIntegerResult(OperationId),
    WrappingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerAddOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerSubtractOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    WrappingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    SaturatingIntegerMultiplyOperandTypeMismatch {
        operation: OperationId,
        operand: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    JumpArityMismatch {
        edge: EdgeId,
        expected: usize,
        actual: usize,
    },
    JumpTypeMismatch {
        edge: EdgeId,
        argument: ScalarType,
        parameter: ScalarType,
    },
    ConditionalConditionTypeMismatch {
        block: BlockId,
        condition: ValueId,
        actual: ScalarType,
    },
    ReturnTypeMismatch {
        machine: MachineId,
        value: ScalarType,
        result: ScalarType,
    },
    ScalarReturnFromUnitMachine {
        machine: MachineId,
        block: BlockId,
    },
    UnitReturnFromScalarMachine {
        machine: MachineId,
        block: BlockId,
    },
    ControlCycle(BlockId),
    UnreachableBlock(BlockId),
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModuleError {}
