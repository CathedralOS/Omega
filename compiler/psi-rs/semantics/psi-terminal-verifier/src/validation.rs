use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentConservation, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PropositionContext, PropositionError, PropositionId,
    ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
    ValueId, content_conservation_fingerprint,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ClaimTransfer, CompletionReceipt, ContentPartitionComposition,
    CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, EntryClaim, OperationKind,
    OperationResult, PropositionBinderArgumentKind, PropositionBinderKind, PropositionEvidence,
    StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator,
};

use crate::verification::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

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
                .map(|place| (place.id, place.kind))
                .chain(
                    nominal_cleanup_contract_receiver(self.module, machine.id).map(|receiver| {
                        (
                            receiver,
                            StructuralPlaceKind::Parameter {
                                position: 0,
                                is_self: true,
                            },
                        )
                    }),
                ),
        )
        .map_err(ModuleError::MalformedProposition)
    }
}

fn nominal_cleanup_contract_receiver(
    module: &TerminalModule,
    cleanup_machine: MachineId,
) -> Option<PlaceId> {
    module
        .machines
        .iter()
        .flat_map(|candidate| &candidate.blocks)
        .flat_map(|block| nominal_cleanups(&block.terminator))
        .find_map(|cleanup| {
            (cleanup.cleanup_machine == cleanup_machine)
                .then_some(cleanup.cleanup_receiver)
                .flatten()
        })
}

fn nominal_cleanups(
    terminator: &Terminator,
) -> Box<dyn Iterator<Item = &psi_terminal::NominalAffineCleanup> + '_> {
    match terminator {
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => Box::new(cleanups.iter()),
        Terminator::Return {
            cleanup_actions, ..
        } => Box::new(cleanup_actions.iter().filter_map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup),
            TerminalAffineCleanupAction::DiscardRoot(_)
            | TerminalAffineCleanupAction::DiscardResidual(_) => None,
        })),
        _ => Box::new(std::iter::empty()),
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

fn resolve_structural_path(
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

fn is_nonempty_field_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
}

fn partial_affine_residuals(
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
    TrivialAffineLocal(u32),
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

    let contract_receiver = nominal_cleanup_contract_receiver(module, machine.id);
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
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } => StructuralRootKey::TrivialAffineLocal(declaration_ordinal),
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
                    | OperationKind::EstablishTrivialAffineLocal { .. }
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
                | OperationKind::PortWrite { .. }
                | OperationKind::EstablishTrivialAffineLocal { .. } => {
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
        for cleanup in nominal_cleanups(&block.terminator) {
            for obligation in &cleanup.requirement_obligations {
                insert_unique(
                    &mut registry.obligations,
                    *obligation,
                    ModuleError::DuplicateObligation,
                )?;
            }
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
            .map(|place| (place.id, place.kind))
            .chain(contract_receiver.map(|receiver| {
                (
                    receiver,
                    StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: true,
                    },
                )
            })),
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
    validate_crash_frontiers(module, machine, &context, &requires_values)?;
    validate_partial_affine_cleanup_shape(module, machine, machines)?;
    validate_nominal_affine_cleanup_shape(module, machine, machines)?;
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
            if structural_arguments.iter().any(|argument| {
                !argument.path.is_empty()
                    && !matches!(
                        argument.path.as_slice(),
                        [StructuralPathSegment::FixedIndex(_)]
                    )
                    && !is_nonempty_field_path(&argument.path)
            }) {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: structural_arguments
                        .iter()
                        .position(|argument| {
                            !argument.path.is_empty()
                                && !matches!(
                                    argument.path.as_slice(),
                                    [StructuralPathSegment::FixedIndex(_)]
                                )
                                && !is_nonempty_field_path(&argument.path)
                        })
                        .unwrap_or_default() as u32,
                });
            }
            let projected = structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty());
            if projected
                && (machine.result != TerminalMachineResult::Unit
                    || !machine.parameters.is_empty()
                    || machine.structural_parameters.len() != 1
                    || structural_arguments.len() != 1
                    || callee.structural_parameters.len() != 1)
            {
                return Err(ModuleError::ProjectedUnitCallOutsideBoundedSlice {
                    operation: operation.id,
                });
            }
            validate_structural_arguments(
                module,
                machine,
                structural_arguments,
                &callee.structural_parameters,
                operation.id,
                true,
            )?;
            if let Some((argument_index, _)) = structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .enumerate()
                .find(|(_, (argument, expected))| {
                    !argument.path.is_empty()
                        && (!expected.qualifications.is_empty()
                            || machine
                                .structural_parameters
                                .iter()
                                .find(|actual| actual.place == argument.place)
                                .is_some_and(|actual| !actual.qualifications.is_empty()))
                })
            {
                return Err(ModuleError::InvalidStructuralArgumentPath {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
            validate_unit_call_contract_places(callee, operation.id)?;
            if projected {
                let projected_parameter = callee.structural_parameters[0].place;
                if unit_call_contract_propositions(callee).any(|proposition| {
                    proposition_content_roots(proposition).contains(&projected_parameter)
                }) {
                    return Err(
                        ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                            operation: operation.id,
                            callee: callee.id,
                            place: projected_parameter,
                        },
                    );
                }
            }
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
                module,
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
            completion_receipts,
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
                module,
                machine,
                structural_arguments,
                &boundary.structural_parameters,
                operation.id,
                true,
            )?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &boundary.published_service_ceiling,
            )?;
            validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
            validate_boundary_completion_receipts(
                machine,
                structural_arguments,
                completion_receipts,
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
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let Some(place) = machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } = place.kind
            else {
                return Err(ModuleError::UnknownTrivialAffineLocal {
                    operation: operation.id,
                    place: *destination,
                });
            };
            let Some(declaration) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
            else {
                return Err(ModuleError::UnknownStructuralType(structural_type));
            };
            if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(ModuleError::TrivialAffineLocalRequiresEmptyRecord {
                    operation: operation.id,
                    place: *destination,
                });
            }
        }
        _ => unreachable!("caller selects only structural/effect operations"),
    }
    Ok(())
}

/// Validate the complete bounded representation for a nonempty run of
/// pairwise-disjoint field transfers, followed by disposal of every maximal
/// residual sibling subtree in recursive reverse declaration order. This
/// partition is checked independently of producer facts before the ownership
/// walk relies on the path-sensitive terminator.
fn validate_partial_affine_cleanup_shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let field_calls = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block.operations.iter().filter_map(move |operation| {
                let OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    ..
                } = &operation.kind
                else {
                    return None;
                };
                structural_arguments
                    .iter()
                    .any(|argument| is_nonempty_field_path(&argument.path))
                    .then_some((
                        block,
                        operation,
                        *callee,
                        structural_arguments,
                        claim_transfers,
                    ))
            })
        })
        .collect::<Vec<_>>();
    let partial_returns = machine
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, Terminator::ReturnUnitPartialAffine { .. }))
        .collect::<Vec<_>>();
    if field_calls.is_empty() && partial_returns.is_empty() {
        return Ok(());
    }
    let invalid = |block: BlockId| ModuleError::InvalidPartialAffineCleanup {
        machine: machine.id,
        block,
    };
    let Some((block, ..)) = field_calls.first() else {
        return Err(invalid(
            partial_returns
                .first()
                .map_or(machine.entry, |block| block.id),
        ));
    };
    let [partial_block] = partial_returns.as_slice() else {
        return Err(invalid(block.id));
    };
    if partial_block.id != block.id
        || field_calls
            .iter()
            .any(|(candidate, ..)| candidate.id != block.id)
        || !matches!(machine.result, TerminalMachineResult::Unit)
        || block.operations.len() != field_calls.len()
        || machine.structural_parameters.len() != 1
        || machine.structural_places.len() != 1
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
    {
        return Err(invalid(block.id));
    }
    let [root] = machine.structural_parameters.as_slice() else {
        unreachable!()
    };
    if root.multiplicity != StructuralMultiplicity::Affine
        || !root.qualifications.is_empty()
        || !machine.structural_places.iter().any(|place| {
            place.id == root.place
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: root.position,
                        is_self: root.is_self,
                    }
        })
    {
        return Err(invalid(block.id));
    }
    let mut moved_paths = BTreeSet::new();
    for (_, _, callee_id, arguments, claim_transfers) in &field_calls {
        let [argument] = arguments.as_slice() else {
            return Err(invalid(block.id));
        };
        if argument.place != root.place || !moved_paths.insert(argument.path.clone()) {
            return Err(invalid(block.id));
        }
        let Some(moved_type) =
            resolve_structural_path(module, root.structural_type, &argument.path)
        else {
            return Err(invalid(block.id));
        };
        let Some(callee) = machines.get(callee_id).copied() else {
            return Err(invalid(block.id));
        };
        let [callee_parameter] = callee.structural_parameters.as_slice() else {
            return Err(invalid(block.id));
        };
        if !claim_transfers.is_empty()
            || callee.result != TerminalMachineResult::Unit
            || !callee.parameters.is_empty()
            || callee_parameter.structural_type != moved_type
            || callee_parameter.multiplicity != StructuralMultiplicity::Affine
            || !callee_parameter.qualifications.is_empty()
        {
            return Err(invalid(block.id));
        }
    }
    let Some(expected_residuals) =
        partial_affine_residuals(module, root.structural_type, &moved_paths)
    else {
        return Err(invalid(block.id));
    };
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        unreachable!()
    };
    if expected_residuals.is_empty()
        || !trivial_affine_discards.is_empty()
        || residual_affine_discards.len() != expected_residuals.len()
        || residual_affine_discards.iter().zip(expected_residuals).any(
            |(residual, (path, structural_type))| {
                residual.place != root.place
                    || residual.path != path
                    || residual.structural_type != structural_type
            },
        )
    {
        return Err(invalid(block.id));
    }
    Ok(())
}

fn validate_nominal_affine_cleanup_shape(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let nominal_returns = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => Some((block, cleanups)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if nominal_returns.is_empty() {
        return Ok(());
    }
    let invalid = |block: BlockId| ModuleError::InvalidNominalAffineCleanup {
        machine: machine.id,
        block,
    };
    let [(block, cleanups)] = nominal_returns.as_slice() else {
        return Err(invalid(machine.entry));
    };
    if machine.result != TerminalMachineResult::Unit
        || module.entry != machine.id
        || machine.blocks.len() != 1
        || block.id != machine.entry
        || !block.parameters.is_empty()
        || !block.operations.is_empty()
        || machine.parameters.len() != 0
        || cleanups.is_empty()
        || machine.structural_parameters.len() != cleanups.len()
        || machine.structural_places.len() != cleanups.len()
        || !machine.entry_claims.is_empty()
        || !machine.published_service_ceiling.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
        || !machine.contract.crash_routes.is_empty()
        || !machine.contract.ensures.is_empty()
    {
        return Err(invalid(block.id));
    }
    let expected_parameters = machine
        .structural_parameters
        .iter()
        .rev()
        .collect::<Vec<_>>();
    let mut target_ids = BTreeSet::new();
    let mut helper_ids = BTreeSet::new();
    for (cleanup, parameter) in cleanups.iter().zip(expected_parameters) {
        if parameter.place != cleanup.place
            || parameter.structural_type != cleanup.structural_type
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || !parameter.qualifications.is_empty()
        {
            return Err(invalid(block.id));
        }
        let Some(source_type) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == cleanup.structural_type)
        else {
            return Err(invalid(block.id));
        };
        if !bounded_nominal_cleanup_receiver_shape(&source_type.shape) {
            return Err(invalid(block.id));
        }
        let Some(target) = machines.get(&cleanup.cleanup_machine).copied() else {
            return Err(invalid(block.id));
        };
        target_ids.insert(target.id);
        let [target_block] = target.blocks.as_slice() else {
            return Err(invalid(block.id));
        };
        if target.id == machine.id
            || cleanup.requirement_obligations.len() != target.contract.requires.len()
            || target.attachment != Some(cleanup.structural_type)
            || target.result != TerminalMachineResult::Unit
            || !target.parameters.is_empty()
            || !target.structural_parameters.is_empty()
            || !target.structural_places.is_empty()
            || !target.entry_claims.is_empty()
            || !target.published_service_ceiling.is_empty()
            || !target.content_entry_claims.is_empty()
            || !target.content_identity_reshuffles.is_empty()
            || !target.content_partition_compositions.is_empty()
            || target.entry != target_block.id
            || !target_block.parameters.is_empty()
            || !matches!(target_block.terminator, Terminator::ReturnUnit { ref trivial_affine_discards, .. } if trivial_affine_discards.is_empty())
            || !target.contract.crash_routes.is_empty()
            || !target.contract.ensures.is_empty()
            || !valid_nominal_cleanup_requirements(module, target, cleanup)
        {
            return Err(invalid(block.id));
        }
        let mut target_helper_ids = BTreeSet::new();
        for operation in &target_block.operations {
            let OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = &operation.kind
            else {
                return Err(invalid(block.id));
            };
            if operation.result != OperationResult::Unit
                || *callee == machine.id
                || *callee == target.id
                || cleanups
                    .iter()
                    .any(|candidate| candidate.cleanup_machine == *callee)
                || !target_helper_ids.insert(*callee)
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
            {
                return Err(invalid(block.id));
            }
            helper_ids.insert(*callee);
            let Some(helper) = machines.get(callee).copied() else {
                return Err(invalid(block.id));
            };
            let Some(helper_attachment) = helper.attachment else {
                return Err(invalid(block.id));
            };
            let helper_attachment_is_empty = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == helper_attachment)
                .is_some_and(|declaration| {
                    matches!(
                        &declaration.shape,
                        StructuralTypeShape::Record { fields } if fields.is_empty()
                    )
                });
            let [helper_block] = helper.blocks.as_slice() else {
                return Err(invalid(block.id));
            };
            if !helper_attachment_is_empty
                || helper.result != TerminalMachineResult::Unit
                || !helper.parameters.is_empty()
                || !helper.structural_parameters.is_empty()
                || !helper.structural_places.is_empty()
                || !helper.entry_claims.is_empty()
                || !helper.published_service_ceiling.is_empty()
                || !helper.content_entry_claims.is_empty()
                || !helper.content_identity_reshuffles.is_empty()
                || !helper.content_partition_compositions.is_empty()
                || helper.entry != helper_block.id
                || !helper_block.parameters.is_empty()
                || !helper_block.operations.is_empty()
                || !matches!(
                    helper_block.terminator,
                    Terminator::ReturnUnit {
                        ref trivial_affine_discards,
                        ..
                    } if trivial_affine_discards.is_empty()
                )
                || !helper.contract.crash_routes.is_empty()
                || !helper.contract.requires.is_empty()
                || !helper.contract.ensures.is_empty()
            {
                return Err(invalid(block.id));
            }
        }
    }
    if module.machines.len() != 1 + target_ids.len() + helper_ids.len() {
        return Err(invalid(block.id));
    }
    Ok(())
}

fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::Structural(_) | StructuralFieldType::Erased { .. } => false,
            }
    })
}

fn valid_nominal_cleanup_requirements(
    module: &TerminalModule,
    target: &TerminalMachine,
    cleanup: &psi_terminal::NominalAffineCleanup,
) -> bool {
    if target.contract.requires.is_empty() {
        return cleanup.cleanup_receiver.is_none() && cleanup.requirement_obligations.is_empty();
    }

    let Some(receiver) = cleanup.cleanup_receiver else {
        return false;
    };
    if cleanup.requirement_obligations.len() != target.contract.requires.len()
        || module
            .machines
            .iter()
            .flat_map(|machine| &machine.structural_places)
            .any(|place| place.id == receiver)
        || module.machines.iter().any(|machine| {
            machine.blocks.iter().any(|block| {
                nominal_cleanups(&block.terminator).any(|candidate| {
                    candidate.cleanup_machine != target.id
                        && candidate.cleanup_receiver == Some(receiver)
                })
            })
        })
    {
        return false;
    }

    let Some(fields) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => Some(fields),
            StructuralTypeShape::FixedArray { .. } => None,
        })
    else {
        return false;
    };
    let mut previous_key = None;
    for requirement in &target.contract.requires {
        let Proposition::Equal(
            ScalarTerm::Boolean(expected),
            ScalarTerm::BooleanField { root, path },
        ) = requirement
        else {
            return false;
        };
        let [CanonicalStructuralPathSegment::Field(field)] = path.as_slice() else {
            return false;
        };
        let key = (*expected, field.get().to_le_bytes());
        if *root != receiver
            || previous_key.is_some_and(|previous| previous >= key)
            || !fields.iter().any(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && candidate.field_type == StructuralFieldType::Scalar(ScalarType::Boolean)
            })
        {
            return false;
        }
        previous_key = Some(key);
    }
    true
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
    for proposition in unit_call_contract_propositions(callee) {
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

fn unit_call_contract_propositions(callee: &TerminalMachine) -> impl Iterator<Item = &Proposition> {
    callee
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
        )
}

fn validate_structural_arguments(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    allow_projected: bool,
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
        if !allow_projected && !argument.path.is_empty() {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        }
        let Some(actual_type) =
            resolve_structural_path(module, actual.structural_type, &argument.path)
        else {
            return Err(ModuleError::InvalidStructuralArgumentPath {
                operation,
                argument_index: index as u32,
            });
        };
        if actual_type != expected.structural_type {
            return Err(ModuleError::StructuralArgumentTypeMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.structural_type,
                actual: actual_type,
            });
        }
        let actual_multiplicity = if argument.path.is_empty() {
            actual.multiplicity
        } else if expected.multiplicity == StructuralMultiplicity::Affine
            && is_nonempty_field_path(&argument.path)
            && actual.multiplicity == StructuralMultiplicity::Affine
        {
            StructuralMultiplicity::Affine
        } else {
            StructuralMultiplicity::Linear
        };
        if actual_multiplicity != expected.multiplicity {
            return Err(ModuleError::StructuralArgumentMultiplicityMismatch {
                operation,
                argument_index: index as u32,
                expected: expected.multiplicity,
                actual: actual_multiplicity,
            });
        }
        for qualification in &expected.qualifications {
            if !argument.path.is_empty() || !actual.qualifications.contains(qualification) {
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
        if !argument.path.is_empty() {
            let callee_claims = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            let claim_free_direct_affine = is_nonempty_field_path(&argument.path)
                && parameter.multiplicity == StructuralMultiplicity::Affine
                && callee_claims.is_empty()
                && caller
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != argument.place);
            if !claim_free_direct_affine
                && !matches!(callee_claims.as_slice(), [claim] if claim.path.is_empty())
            {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
            if caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(ModuleError::UnitCallContentClaimMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
        }
        let mut caller_claim_paths = caller
            .entry_claims
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_claim_paths = callee
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_claim_paths.sort();
        callee_claim_paths.sort();
        if caller_claim_paths != callee_claim_paths {
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
        let Some((claim_input, claim_path)) = claim_input(caller, transfer.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_path_matches = claim_path.starts_with(&argument.path)
            && callee.entry_claims.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_matches = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_path_matches && !content_matches) {
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

fn claim_input(
    machine: &TerminalMachine,
    claim: ClaimId,
) -> Option<(PlaceId, &[StructuralPathSegment])> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            machine.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim)
                    .then_some((candidate.input.root, &[] as &[StructuralPathSegment]))
            })
        })
}

fn validate_unit_call_crash_continuations(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let boolean_roots = callee
        .contract
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
        .flat_map(proposition_boolean_field_roots)
        .collect::<BTreeSet<_>>();
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let prefix = structural_argument_canonical_prefix(module, caller, argument);
            if prefix.is_none() && boolean_roots.contains(&parameter.place) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation,
                        callee: callee.id,
                        place: parameter.place,
                    },
                );
            }
            Ok((
                parameter.place,
                (argument.place, prefix.unwrap_or_default()),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
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
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                        CrashPredicateTerm::new(substitute_proposition_structural_places(
                            predicate.proposition(),
                            substitutions,
                        )),
                    ),
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

pub(crate) fn structural_argument_canonical_prefix(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut structural_type = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)?
        .structural_type;
    let mut prefix = Vec::with_capacity(argument.path.len());
    for segment in &argument.path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let field = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match &declaration.shape {
                        StructuralTypeShape::Record { fields } => fields.iter().find(|field| {
                            field.identity == *identity && !field.relevance.is_erased()
                        }),
                        StructuralTypeShape::FixedArray { .. } => None,
                    })?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = next;
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(prefix)
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

fn validate_boundary_completion_receipts(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
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
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim)
        {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        let Some(argument) = arguments.get(receipt.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        };
        let Some((claim_input, claim_path)) = claim_input(caller, receipt.claim) else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: receipt.claim,
            });
        };
        if claim_input != argument.place
            || (!argument.path.is_empty() && claim_path != argument.path.as_slice())
        {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: receipt.claim,
                argument_index: receipt.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryCompletionReceiptMismatch(operation));
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
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
    module: &TerminalModule,
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
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
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
            validate_boolean_field_terms(
                module,
                machine,
                predicate.proposition(),
                &machine.contract.requires,
            )?;
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

fn validate_boolean_field_terms(
    module: &TerminalModule,
    machine: &TerminalMachine,
    proposition: &Proposition,
    runtime_requirements: &[Proposition],
) -> Result<(), ModuleError> {
    fn validate_term(
        module: &TerminalModule,
        machine: &TerminalMachine,
        term: &ScalarTerm,
        runtime_requirements: &[Proposition],
    ) -> Result<(), ModuleError> {
        fn safe_exact_divisor(
            integer_type: IntegerType,
            dividend: &ScalarTerm,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0 && *value != -1,
                _ => {}
            }
            let one = match integer_type.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
                IntegerSign::Signed => IntegerValue::Signed(1),
            };
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            if ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).is_ok_and(
                |negative_two| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_two))
                },
            ) {
                return true;
            }
            let negative_one = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
                .expect("every signed fixed integer admits negative one");
            if !requirements.contains(&Proposition::LessOrEqual(divisor.clone(), negative_one)) {
                return false;
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed fixed integer has a signed minimum")
            };
            ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            )
            .is_ok_and(|minimum_plus_one| {
                requirements.contains(&Proposition::LessOrEqual(
                    minimum_plus_one,
                    dividend.clone(),
                ))
            })
        }

        fn safe_policy_divisor(
            integer_type: IntegerType,
            divisor: &ScalarTerm,
            requirements: &[Proposition],
        ) -> bool {
            match divisor {
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(value),
                } => return *scalar_type == integer_type && *value != 0,
                ScalarTerm::Integer {
                    scalar_type,
                    value: IntegerValue::Signed(value),
                } => return *scalar_type == integer_type && *value != 0,
                _ => {}
            }
            let one = match integer_type.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
                IntegerSign::Signed => IntegerValue::Signed(1),
            };
            if ScalarTerm::integer(integer_type, one).is_ok_and(|one| {
                requirements.contains(&Proposition::LessOrEqual(one, divisor.clone()))
            }) {
                return true;
            }
            if integer_type.sign() != IntegerSign::Signed {
                return false;
            }
            [IntegerValue::Signed(-1), IntegerValue::Signed(-2)]
                .into_iter()
                .filter_map(|bound| ScalarTerm::integer(integer_type, bound).ok())
                .any(|bound| {
                    requirements.contains(&Proposition::LessOrEqual(divisor.clone(), bound))
                })
        }

        match term {
            ScalarTerm::BooleanField { root, path } => {
                let mut structural_type = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *root)
                    .map(|parameter| parameter.structural_type);
                let mut valid = !path.is_empty();
                for (index, segment) in path.iter().enumerate() {
                    let Some(current_type) = structural_type else {
                        valid = false;
                        break;
                    };
                    let is_last = index + 1 == path.len();
                    match segment {
                        CanonicalStructuralPathSegment::Field(field_id) => {
                            let field = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match &declaration.shape {
                                    StructuralTypeShape::Record { fields } => {
                                        fields.iter().find(|candidate| candidate.id == *field_id)
                                    }
                                    StructuralTypeShape::FixedArray { .. } => None,
                                });
                            let Some(field) = field.filter(|field| !field.relevance.is_erased())
                            else {
                                valid = false;
                                break;
                            };
                            match (&field.field_type, is_last) {
                                (StructuralFieldType::Structural(next), false) => {
                                    structural_type = Some(*next);
                                }
                                (StructuralFieldType::Scalar(ScalarType::Boolean), true) => {
                                    structural_type = None;
                                }
                                _ => {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        CanonicalStructuralPathSegment::FixedIndex(fixed_index) => {
                            let element = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match declaration.shape {
                                    StructuralTypeShape::FixedArray { element, length }
                                        if *fixed_index < length =>
                                    {
                                        Some(element)
                                    }
                                    _ => None,
                                });
                            let Some(element) = element.filter(|_| !is_last) else {
                                valid = false;
                                break;
                            };
                            structural_type = Some(element);
                        }
                    }
                }
                if !valid {
                    return Err(ModuleError::InvalidBooleanFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                    });
                }
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } => {
                let mut structural_type = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *root)
                    .map(|parameter| parameter.structural_type);
                let mut valid = !path.is_empty();
                for (index, segment) in path.iter().enumerate() {
                    let Some(current_type) = structural_type else {
                        valid = false;
                        break;
                    };
                    let is_last = index + 1 == path.len();
                    match segment {
                        CanonicalStructuralPathSegment::Field(field_id) => {
                            let field = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match &declaration.shape {
                                    StructuralTypeShape::Record { fields } => {
                                        fields.iter().find(|candidate| candidate.id == *field_id)
                                    }
                                    StructuralTypeShape::FixedArray { .. } => None,
                                });
                            let Some(field) = field.filter(|field| !field.relevance.is_erased())
                            else {
                                valid = false;
                                break;
                            };
                            match (&field.field_type, is_last) {
                                (StructuralFieldType::Structural(next), false) => {
                                    structural_type = Some(*next);
                                }
                                (
                                    StructuralFieldType::Scalar(ScalarType::Integer(actual)),
                                    true,
                                ) if actual == scalar_type => {
                                    structural_type = None;
                                }
                                _ => {
                                    valid = false;
                                    break;
                                }
                            }
                        }
                        CanonicalStructuralPathSegment::FixedIndex(fixed_index) => {
                            let element = module
                                .structural_types
                                .iter()
                                .find(|declaration| declaration.id == current_type)
                                .and_then(|declaration| match declaration.shape {
                                    StructuralTypeShape::FixedArray { element, length }
                                        if *fixed_index < length =>
                                    {
                                        Some(element)
                                    }
                                    _ => None,
                                });
                            let Some(element) = element.filter(|_| !is_last) else {
                                valid = false;
                                break;
                            };
                            structural_type = Some(element);
                        }
                    }
                }
                if !valid {
                    return Err(ModuleError::InvalidIntegerFieldTerm {
                        machine: machine.id,
                        root: *root,
                        path: path.clone(),
                        scalar_type: *scalar_type,
                    });
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                validate_term(module, machine, operand, runtime_requirements)?;
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                if !safe_policy_divisor(*scalar_type, right, runtime_requirements) {
                    return Err(ModuleError::UnsafeStructuralCrashPolicyDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                if !safe_exact_divisor(*scalar_type, left, right, runtime_requirements) {
                    return Err(ModuleError::UnsafeStructuralCrashExactDivisor {
                        machine: machine.id,
                        scalar_type: *scalar_type,
                    });
                }
                validate_term(module, machine, left, runtime_requirements)?;
                validate_term(module, machine, right, runtime_requirements)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                validate_term(module, machine, value, runtime_requirements)?;
                validate_term(module, machine, count, runtime_requirements)?;
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
        Ok(())
    }

    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term(module, machine, left, runtime_requirements)?;
            validate_term(module, machine, right, runtime_requirements)?;
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_boolean_field_terms(module, machine, proposition, runtime_requirements)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_boolean_field_terms(module, machine, premise, runtime_requirements)?;
            validate_boolean_field_terms(module, machine, conclusion, runtime_requirements)?;
        }
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => {}
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
            && (structural_claim.input != binding.input.root
                || binding.input.segments
                    != structural_claim
                        .path
                        .iter()
                        .map(|segment| match segment {
                            StructuralPathSegment::Field(identity) => {
                                psi_core::ContentPlaceSegment::Field(identity.clone())
                            }
                            StructuralPathSegment::FixedIndex(index) => {
                                psi_core::ContentPlaceSegment::FixedIndex(*index)
                            }
                        })
                        .collect::<Vec<_>>())
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
    let Some(result) = machine.result.structural() else {
        if machine.content_identity_reshuffles.is_empty() {
            return Ok(());
        }
        return Err(ModuleError::ContentIdentityReshuffleRequiresStructuralResult(machine.id));
    };
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
            || reshuffle.output.root != result.place
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
            StructuralPlaceKind::TrivialAffineLocal { .. } => {
                return Err(ModuleError::ContentPartitionSourceLocalUnsupported(
                    place.id,
                ));
            }
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
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_contract_clause_kind(proposition, contract, clause)?;
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
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_contract_scope(proposition, allowed, contract, clause)?;
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
        ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralOwnershipFrontier {
    // Claims carry proof-visible custody identity. Owned places independently
    // enforce by-value affine/linear use even when no linear claim row exists.
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    /// Exact field paths already transferred from an otherwise-live affine
    /// root. The root remains present until its complementary residual action
    /// proves complete exhaustion at `ReturnUnitPartialAffine`.
    moved_field_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
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
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &machine.content_entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
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
        moved_field_paths: BTreeMap::new(),
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
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
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
            || frontiers
                .iter()
                .any(|candidate| candidate.moved_field_paths != frontier.moved_field_paths)
        {
            return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block_id));
        }
        let block = blocks
            .get(&block_id)
            .expect("topological order contains known blocks");
        let mut frontier = frontier;
        for operation in &block.operations {
            if let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind {
                if frontier
                    .owned_places
                    .insert(destination, StructuralMultiplicity::Affine)
                    .is_some()
                {
                    return Err(ModuleError::TrivialAffineLocalAlreadyLive {
                        operation: operation.id,
                        place: destination,
                    });
                }
            }
            let claims = match &operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                } => claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>(),
                OperationKind::BoundaryCallUnit {
                    completion_receipts,
                    ..
                } => completion_receipts
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
                        (argument.path.is_empty()
                            && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
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
                            (argument.path.is_empty()
                                && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
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
            let projected_arguments = match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                }
                | OperationKind::BoundaryCallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            for argument in projected_arguments
                .iter()
                .filter(|argument| !argument.path.is_empty())
            {
                if is_nonempty_field_path(&argument.path)
                    && matches!(block.terminator, Terminator::ReturnUnitPartialAffine { .. })
                {
                    let moved = frontier
                        .moved_field_paths
                        .entry(argument.place)
                        .or_default();
                    if !frontier.owned_places.contains_key(&argument.place)
                        || moved.iter().any(|existing| {
                            existing.starts_with(&argument.path)
                                || argument.path.starts_with(existing)
                        })
                        || !moved.insert(argument.path.clone())
                    {
                        return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                            operation: operation.id,
                            place: argument.place,
                        });
                    }
                    continue;
                }
                if !frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(argument.place))
                    && frontier.owned_places.remove(&argument.place).is_none()
                {
                    return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                        operation: operation.id,
                        place: argument.place,
                    });
                }
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                trivial_affine_discards,
                ..
            } => {
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut frontier,
                    *edge,
                    trivial_affine_discards,
                )?;
                incoming.entry(*target).or_default().push(frontier);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let mut true_frontier = frontier.clone();
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut true_frontier,
                    when_true.edge,
                    &when_true.trivial_affine_discards,
                )?;
                incoming
                    .entry(when_true.target)
                    .or_default()
                    .push(true_frontier);
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut frontier,
                    when_false.edge,
                    &when_false.trivial_affine_discards,
                )?;
                incoming
                    .entry(when_false.target)
                    .or_default()
                    .push(frontier);
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
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
            Terminator::ReturnUnitPartialAffine {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                let Some(first_residual) = residual_affine_discards.first() else {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                let root_place = first_residual.place;
                if residual_affine_discards
                    .iter()
                    .any(|residual| residual.place != root_place)
                {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let Some(moved) = frontier.moved_field_paths.remove(&root_place) else {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                let expected_residuals = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == root_place)
                    .and_then(|parameter| {
                        partial_affine_residuals(module, parameter.structural_type, &moved)
                    });
                if moved.is_empty()
                    || expected_residuals.as_ref().is_none_or(|expected| {
                        residual_affine_discards.len() != expected.len()
                            || residual_affine_discards.iter().zip(expected).any(
                                |(residual, (path, structural_type))| {
                                    residual.path != *path
                                        || residual.structural_type != *structural_type
                                },
                            )
                    })
                    || frontier.owned_places.remove(&root_place)
                        != Some(StructuralMultiplicity::Affine)
                {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if !frontier.moved_field_paths.is_empty() {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
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
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    if frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(cleanup.place))
                        || frontier.owned_places.remove(&cleanup.place)
                            != Some(StructuralMultiplicity::Affine)
                    {
                        return Err(ModuleError::InvalidNominalAffineCleanup {
                            machine: machine.id,
                            block: block.id,
                        });
                    }
                }
                if !frontier.moved_field_paths.is_empty()
                    || !frontier.claims.is_empty()
                    || !frontier.owned_places.is_empty()
                {
                    return Err(ModuleError::InvalidNominalAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
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
                validate_scalar_cleanup_actions(
                    module,
                    machine,
                    machines,
                    block.id,
                    &frontier,
                    cleanup_actions,
                )?;
            }
            Terminator::ReturnStructural {
                source,
                returned_claims,
                trivial_affine_discards,
                ..
            } => {
                let result = machine
                    .result
                    .structural()
                    .expect("control validation requires a structural result");
                let source_parameter = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *source)
                    .expect("control validation requires a structural source parameter");
                if frontier.owned_places.remove(source).is_none() {
                    return Err(ModuleError::StructuralReturnSourceNotLive {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                if source_parameter.structural_type != result.structural_type
                    || source_parameter.multiplicity != result.multiplicity
                    || source_parameter.qualifications != result.qualifications
                {
                    return Err(ModuleError::StructuralReturnSignatureMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if returned_claims.is_empty()
                    || returned_claims.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(ModuleError::NonCanonicalStructuralReturnClaims {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let expected_claims = frontier
                    .claims
                    .iter()
                    .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
                    .collect::<Vec<_>>();
                if *returned_claims != expected_claims {
                    return Err(ModuleError::StructuralReturnClaimSetMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                for claim in returned_claims {
                    frontier.claims.remove(claim);
                }
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::StructuralReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if let Some(claim) = frontier.claims.keys().next() {
                    return Err(ModuleError::LiveClaimAtStructuralReturn {
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

fn validate_scalar_cleanup_actions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    block: BlockId,
    frontier: &StructuralOwnershipFrontier,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), ModuleError> {
    let mismatch = || ModuleError::ScalarReturnAffineDiscardsMismatch {
        machine: machine.id,
        block,
    };
    let mut frontier = frontier.clone();
    let mut actions = actions.iter();

    let mut locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    for (_, place) in locals {
        if actions.next() != Some(&TerminalAffineCleanupAction::DiscardRoot(place)) {
            return Err(mismatch());
        }
        frontier.owned_places.remove(&place);
    }

    for parameter in machine.structural_parameters.iter().rev() {
        if !frontier.owned_places.contains_key(&parameter.place) {
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Affine
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(mismatch());
        }
        if let Some(moved) = frontier.moved_field_paths.remove(&parameter.place) {
            let Some(residuals) =
                partial_affine_residuals(module, parameter.structural_type, &moved)
            else {
                return Err(mismatch());
            };
            if moved.is_empty() || residuals.is_empty() {
                return Err(mismatch());
            }
            for (path, structural_type) in residuals {
                let expected = TerminalAffineCleanupAction::DiscardResidual(
                    psi_terminal::StructuralAffineDiscard {
                        place: parameter.place,
                        path,
                        structural_type,
                    },
                );
                if actions.next() != Some(&expected) {
                    return Err(mismatch());
                }
            }
        } else {
            let Some(action) = actions.next() else {
                return Err(mismatch());
            };
            match action {
                TerminalAffineCleanupAction::DiscardRoot(place) if *place == parameter.place => {}
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if cleanup.place == parameter.place
                        && cleanup.structural_type == parameter.structural_type
                        && valid_scalar_nominal_cleanup(module, machine, machines, cleanup) => {}
                _ => return Err(mismatch()),
            }
        }
        frontier.owned_places.remove(&parameter.place);
    }

    if actions.next().is_some()
        || !frontier.owned_places.is_empty()
        || !frontier.moved_field_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

fn valid_scalar_nominal_cleanup(
    module: &TerminalModule,
    caller: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    cleanup: &psi_terminal::NominalAffineCleanup,
) -> bool {
    let Some(source) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
    else {
        return false;
    };
    let Some(target) = machines.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.id
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && target.result == TerminalMachineResult::Unit
        && target.parameters.is_empty()
        && target.structural_parameters.is_empty()
        && target.entry_claims.is_empty()
        && target.content_entry_claims.is_empty()
        && target.contract.ensures.is_empty()
        && target.contract.crash_routes.is_empty()
        && cleanup.requirement_obligations.len() == target.contract.requires.len()
        && valid_nominal_cleanup_requirements(module, target, cleanup)
}

fn expected_trivial_affine_discards(
    machine: &TerminalMachine,
    frontier: &StructuralOwnershipFrontier,
) -> Vec<PlaceId> {
    let mut output = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    output.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    let mut output = output
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    output.extend(
        machine
            .structural_parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == StructuralMultiplicity::Affine
                    && frontier.owned_places.contains_key(&parameter.place)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            })
            .collect::<Vec<_>>(),
    );
    output
}

fn apply_edge_trivial_affine_discards(
    machine: &TerminalMachine,
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    discards: &[PlaceId],
) -> Result<(), ModuleError> {
    let eligible = expected_trivial_affine_discards(machine, frontier);
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return Err(ModuleError::EdgeAffineDiscardsInvalid { edge });
    }
    for place in discards {
        frontier.owned_places.remove(place);
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
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
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
                ..
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
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. } => {
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::ReturnStructural { source, .. } => {
                if machine.result.structural().is_none() {
                    return Err(ModuleError::StructuralReturnFromNonStructuralMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if !machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == *source)
                {
                    return Err(ModuleError::StructuralReturnRequiresParameterSource {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
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
        | OperationKind::PortWrite { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. } => None,
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
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            propositions.iter().any(proposition_contains_content)
        }
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

fn proposition_boolean_field_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn collect_term(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                roots.insert(*root);
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => collect_term(operand, roots),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
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
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                collect_term(left, roots);
                collect_term(right, roots);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                collect_term(value, roots);
                collect_term(count, roots);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                collect_term(left, roots);
                collect_term(right, roots);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
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
            | Proposition::ContentConservation(_) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
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
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
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
    InvalidPartialAffineCleanup {
        machine: MachineId,
        block: BlockId,
    },
    InvalidNominalAffineCleanup {
        machine: MachineId,
        block: BlockId,
    },
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
    InvalidStructuralArrayLength(StructuralTypeId),
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
    UnknownTrivialAffineLocal {
        operation: OperationId,
        place: PlaceId,
    },
    TrivialAffineLocalRequiresEmptyRecord {
        operation: OperationId,
        place: PlaceId,
    },
    TrivialAffineLocalDeclarationRequiresEmptyRecord {
        machine: MachineId,
        place: PlaceId,
    },
    TrivialAffineLocalEstablishmentMismatch(MachineId),
    NonCanonicalTrivialAffineLocals(MachineId),
    TrivialAffineLocalAlreadyLive {
        operation: OperationId,
        place: PlaceId,
    },
    StructuralResultMustBeOwned(MachineId),
    StructuralResultPlaceMismatch {
        machine: MachineId,
        place: PlaceId,
    },
    DuplicateEntryClaimInput(PlaceId),
    InvalidEntryClaimFieldPath(ClaimId),
    OverlappingEntryClaimInput {
        first: ClaimId,
        second: ClaimId,
    },
    NonCanonicalEntryClaimOrder(MachineId),
    EntryClaimRequiresStructuralParameter(ClaimId),
    EntryClaimRequiresOwnedParameter(ClaimId),
    LinearParameterHasNoEntryClaim {
        machine: MachineId,
        place: PlaceId,
    },
    IncompleteFixedArrayEntryClaims {
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
    ProjectedUnitCallOutsideBoundedSlice {
        operation: OperationId,
    },
    ProjectedUnitCallContractUsesStructuralParameter {
        operation: OperationId,
        callee: MachineId,
        place: PlaceId,
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
    InvalidStructuralArgumentPath {
        operation: OperationId,
        argument_index: u32,
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
    DuplicateBoundaryCompletionReceipt(OperationId),
    NonCanonicalBoundaryCompletionReceipts(OperationId),
    BoundaryCompletionReceiptMismatch(OperationId),
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
    UnitReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    ScalarReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    EdgeAffineDiscardsInvalid {
        edge: EdgeId,
    },
    LiveLinearClaimAtScalarReturn {
        machine: MachineId,
        block: BlockId,
        claim: ClaimId,
    },
    StructuralReturnFromNonStructuralMachine {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnRequiresParameterSource {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralReturnSourceNotLive {
        machine: MachineId,
        block: BlockId,
        place: PlaceId,
    },
    StructuralReturnSignatureMismatch {
        machine: MachineId,
        block: BlockId,
    },
    NonCanonicalStructuralReturnClaims {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnClaimSetMismatch {
        machine: MachineId,
        block: BlockId,
    },
    StructuralReturnAffineDiscardsMismatch {
        machine: MachineId,
        block: BlockId,
    },
    LiveClaimAtStructuralReturn {
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
    ContentPartitionSourceLocalUnsupported(PlaceId),
    UnitMachineHasResultStructuralPlace {
        machine: MachineId,
        place: PlaceId,
    },
    ScalarMachineHasResultStructuralPlace {
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
    InvalidBooleanFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    InvalidIntegerFieldTerm {
        machine: MachineId,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: psi_core::IntegerType,
    },
    UnsafeStructuralCrashExactDivisor {
        machine: MachineId,
        scalar_type: psi_core::IntegerType,
    },
    UnsafeStructuralCrashPolicyDivisor {
        machine: MachineId,
        scalar_type: psi_core::IntegerType,
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
    ContentIdentityReshuffleRequiresStructuralResult(MachineId),
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
