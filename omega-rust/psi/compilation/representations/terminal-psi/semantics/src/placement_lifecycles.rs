//! Foundation checks for whole-value placement demand at foreign ingress.
//!
//! These joins check the source-authenticated catalog's coordinates and shared
//! geometry. Source proof/admission authenticates the projection itself.
//! They do not select a provider, establish resource supply, or prove that
//! initialization/extraction has executed. Those obligations belong to the
//! exact installed call and its execution-owned backing.

use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineResult, StructuralAccess, StructuralTypeShape,
    TerminalModule, TerminalPlacementLifecycleExecution, TerminalPlacementLifecycleOperation,
};

mod result_contract;
pub use result_contract::{
    PlacementResultContract, placement_result_contract, placement_view_reference_slot,
};

pub fn validate_placement_lifecycle_plans(module: &TerminalModule) -> Result<(), &'static str> {
    // Check the mandatory direction too: omitting a row never demotes a
    // source-authenticated primitive to an ordinary opaque host boundary.
    for boundary in &module.boundary_machines {
        if let Some(operation) = boundary
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.placement_operation())
            && !module.placement_lifecycle_plans.iter().any(|plan| {
                plan.boundary == boundary.id
                    && plan.operation == operation
                    && matches!(
                        plan.execution,
                        TerminalPlacementLifecycleExecution::CompilerPrimitive { .. }
                    )
            })
        {
            return Err("compiler placement primitive lost its mandatory lifecycle obligation");
        }
    }
    let mut previous = None;
    for (position, plan) in module.placement_lifecycle_plans.iter().enumerate() {
        if previous.is_some_and(|boundary| boundary >= plan.boundary) {
            return Err("placement lifecycle boundaries are not unique and ordered");
        }
        previous = Some(plan.boundary);
        let boundary = module
            .boundary_machine(plan.boundary)
            .ok_or("placement lifecycle boundary is absent")?;
        if boundary.role != BoundaryDeclarationRole::Callable {
            return Err("placement lifecycle names non-callable contract evidence");
        }
        let schema = terminal_psi::find_structural_type(&module.structural_types, plan.schema)
            .ok_or("placement lifecycle schema is absent")?;
        if !matches!(schema.shape, StructuralTypeShape::Record { .. }) {
            return Err("placement lifecycle schema is not a record");
        }
        for identity in [
            plan.policy_identity,
            plan.policy_plan_machine_identity,
            plan.schema_identity,
        ] {
            if module.identities.text(identity).is_none_or(str::is_empty) {
                return Err("placement lifecycle declaration identity is absent");
            }
        }
        if !boundary.scalar_parameters.is_empty() {
            return Err("placement lifecycle requires structural inputs");
        }
        let profile_type = match &plan.execution {
            TerminalPlacementLifecycleExecution::SelectedProvider => {
                if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Trait {
                    return Err("provider lifecycle does not name a trait requirement");
                }
                None
            }
            TerminalPlacementLifecycleExecution::CompilerPrimitive {
                profile_type,
                profile_requirement,
            } => {
                if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Machine {
                    return Err("compiler placement primitive does not name an exact machine");
                }
                if boundary
                    .declaration
                    .as_ref()
                    .and_then(|declaration| declaration.execution.placement_operation())
                    != Some(plan.operation)
                {
                    return Err("placement primitive obligation differs from its declaration");
                }
                match profile_type {
                    Some(profile_type) => {
                        let requirement = profile_requirement.as_ref().ok_or(
                            "placement initialization lost its profile supply requirement",
                        )?;
                        let supply = module.boundary_machine(requirement.boundary).ok_or(
                            "placement initialization lost its profile supply requirement",
                        )?;
                        let [storage] = supply.structural_parameters.as_slice() else {
                            return Err("profile supply lost its borrowed extent input");
                        };
                        if supply.role != BoundaryDeclarationRole::Callable
                            || supply.kind != semantic_vocabulary::BoundaryRequirementKind::Trait
                            || !requirement.owner.same_identity_as(
                                &module.identities,
                                &supply.owner,
                                &module.identities,
                            )
                            || module
                                .identities
                                .text(requirement.callable)
                                .filter(|identity| !identity.is_empty())
                                .is_none_or(|identity| {
                                    module.identities.text(supply.identity) != Some(identity)
                                })
                            || !supply.scalar_parameters.is_empty()
                            || storage.structural_type != plan.storage.carrier
                            || storage.access != StructuralAccess::SharedBorrow
                            || !storage.qualifications.contains(&plan.storage.granted)
                            || !matches!(&supply.result, BoundaryMachineResult::Structural(result)
                                if result.structural_type == *profile_type)
                        {
                            return Err(
                                "profile supply differs from its exact core declaration or extent/result contract",
                            );
                        }
                        if terminal_psi::find_structural_type(
                            &module.structural_types,
                            *profile_type,
                        )
                        .is_none_or(|declaration| {
                            !matches!(declaration.shape, StructuralTypeShape::Record { .. })
                        }) {
                            return Err(
                                "placement primitive profile is not a retained record type",
                            );
                        }
                    }
                    _ => return Err("placement primitive operation and profile input disagree"),
                }
                *profile_type
            }
        };
        match plan.operation {
            TerminalPlacementLifecycleOperation::View => {
                let [storage, profile] = boundary.structural_parameters.as_slice() else {
                    return Err("borrowed placement lost its storage and profile inputs");
                };
                if storage.access != StructuralAccess::SharedBorrow
                    || profile.access != StructuralAccess::SharedBorrow
                    || Some(profile.structural_type) != profile_type
                {
                    return Err(
                        "borrowed placement must retain shared storage and original profile loans",
                    );
                }
            }
            TerminalPlacementLifecycleOperation::Initialize => {
                let parameters = boundary.structural_parameters.as_slice();
                let expected = if profile_type.is_some() { 3 } else { 2 };
                if parameters.len() != expected {
                    return Err("placement initialization lost its storage/value inputs");
                }
                let value = &parameters[1];
                if value.structural_type != plan.schema
                    || parameters[..2]
                        .iter()
                        .any(|parameter| parameter.access != StructuralAccess::Owned)
                {
                    return Err("placement initialization value differs from its schema");
                }
                if let Some(profile_type) = profile_type
                    && (parameters[2].structural_type != profile_type
                        || parameters[2].access != StructuralAccess::SharedBorrow)
                {
                    return Err(
                        "placement initialization profile differs from its exact input type",
                    );
                }
            }
            TerminalPlacementLifecycleOperation::MoveOut => {
                let parameters = &boundary.structural_parameters;
                if parameters.len() != if profile_type.is_some() { 2 } else { 1 } {
                    return Err("placement extraction lost its storage input");
                }
                if boundary.structural_parameters[0].access != StructuralAccess::Owned {
                    return Err("placement extraction must own its resident storage");
                }
                if let Some(profile_type) = profile_type
                    && (parameters[1].structural_type != profile_type
                        || parameters[1].access != StructuralAccess::SharedBorrow)
                {
                    return Err("placement extraction must borrow its original profile");
                }
            }
        }
        validate_storage(module, plan)?;
        if plan.operation == TerminalPlacementLifecycleOperation::View {
            validate_placement_view_content(module, plan)?;
        }
        placement_result_contract(module, plan)?;
        validate_image_fields(module, plan)?;
        for view in module.placed_view_plans.iter().filter(|view| {
            view.policy_identity == plan.policy_identity
                && view.schema_identity == plan.schema_identity
        }) {
            if view.schema != plan.schema
                || view.policy_plan_machine_identity != plan.policy_plan_machine_identity
                || view.placement != plan.placement
            {
                return Err("placement lifecycle and field view disagree on normalized policy");
            }
            for field in &view.fields {
                let image_field = plan
                    .image_fields
                    .get(field.access_slot as usize)
                    .and_then(|identity| match &schema.shape {
                        StructuralTypeShape::Record { fields, .. } => {
                            fields.iter().find(|field| field.id == *identity)
                        }
                        _ => None,
                    })
                    .ok_or("placement view lost its whole-value field correspondence")?;
                if image_field.identity != field.schema_field {
                    return Err("placement view and whole-value image select different fields");
                }
            }
        }
        for earlier in module.placement_lifecycle_plans[..position]
            .iter()
            .filter(|earlier| {
                earlier.policy_identity == plan.policy_identity
                    && earlier.schema_identity == plan.schema_identity
            })
        {
            if earlier.schema != plan.schema
                || earlier.image_fields != plan.image_fields
                || earlier.storage.carrier != plan.storage.carrier
                || earlier.storage.base != plan.storage.base
                || earlier.storage.length != plan.storage.length
                || earlier.storage.granted != plan.storage.granted
                || (earlier.operation != TerminalPlacementLifecycleOperation::View
                    && plan.operation != TerminalPlacementLifecycleOperation::View
                    && earlier.storage.owned_content()? != plan.storage.owned_content()?)
                || earlier.policy_plan_machine_identity != plan.policy_plan_machine_identity
                || earlier.placement != plan.placement
            {
                return Err("placement lifecycle operations disagree on normalized policy");
            }
        }
    }
    Ok(())
}

/// Nonresident View supplies no validation or custody evidence. Its represented
/// leaves therefore need total decoding without reference, tag or refinement
/// fabrication. External bytes may change between observations, so a one-time
/// inspection could not justify stronger read predicates either.
pub fn validate_placement_view_content(
    module: &TerminalModule,
    plan: &terminal_psi::TerminalPlacementLifecyclePlan,
) -> Result<(), &'static str> {
    fn scalar(value: semantic_vocabulary::ScalarType) -> Result<(), &'static str> {
        match value {
            semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address() => Ok(()),
            _ => Err("nonresident placement View requires total scalar decoding evidence"),
        }
    }
    fn visit(
        module: &TerminalModule,
        identity: semantic_vocabulary::StructuralTypeId,
        active: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    ) -> Result<(), &'static str> {
        if active.contains(&identity) {
            return Err("nonresident placement View cannot decode cyclic inline content");
        }
        let declaration = module
            .structural_type(identity)
            .ok_or("nonresident placement View lost its exact content declaration")?;
        if declaration.owns_linear_claim
            || declaration.quotient_carrier.is_some()
            || declaration.scalar_restriction.is_some()
        {
            return Err("nonresident placement View requires content validity or custody evidence");
        }
        active.push(identity);
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(value) => scalar(*value)?,
            StructuralTypeShape::FixedArray { element, .. } => visit(module, *element, active)?,
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Scalar(value) => scalar(value)?,
                        terminal_psi::StructuralFieldType::IeeeFloat(_) => {}
                        terminal_psi::StructuralFieldType::Structural(child) => {
                            visit(module, child, active)?
                        }
                        _ => {
                            return Err(
                                "nonresident placement View requires total field decoding evidence",
                            );
                        }
                    }
                }
            }
            _ => {
                return Err(
                    "nonresident placement View requires explicit content validity or custody evidence",
                );
            }
        }
        active.pop();
        Ok(())
    }
    visit(module, plan.schema, &mut Vec::new())
}

fn validate_image_fields(
    module: &TerminalModule,
    plan: &terminal_psi::TerminalPlacementLifecyclePlan,
) -> Result<(), &'static str> {
    let Some(StructuralTypeShape::Record { fields, .. }) = module
        .structural_type(plan.schema)
        .map(|declaration| &declaration.shape)
    else {
        return Err("placement image schema is absent");
    };
    if plan.image_fields.len() != plan.placement.access().plan().entries().len() {
        return Err("placement image fields differ from the complete normalized slot roster");
    }
    for (position, identity) in plan.image_fields.iter().enumerate() {
        let field = fields
            .iter()
            .find(|field| field.id == *identity)
            .ok_or("placement image names an absent schema field")?;
        if field.relevance.is_erased() || plan.image_fields[..position].contains(identity) {
            return Err("placement image repeats a field or represents erased custody");
        }
    }
    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let size = if let Some(scalar) = field.field_type.scalar_type() {
            crate::reference_storage_byte_width(scalar).map(u64::from)
        } else {
            field.field_type.as_structural().and_then(|child| {
                crate::reference_storage_size(module.structural_types.iter(), child)
            })
        }
        .ok_or("placement image field has no closed physical representation")?;
        if (size != 0) != plan.image_fields.contains(&field.id) {
            return Err(
                "placement image omits represented content or represents a zero-byte field",
            );
        }
    }
    Ok(())
}

fn validate_storage(
    module: &TerminalModule,
    plan: &terminal_psi::TerminalPlacementLifecyclePlan,
) -> Result<(), &'static str> {
    let storage = &plan.storage;
    crate::resident_references::validate_resident_owner_geometry(
        module,
        storage.carrier,
        storage.base,
        storage.length,
    )?;
    let mut domains = vec![storage.granted];
    let content = match plan.operation {
        TerminalPlacementLifecycleOperation::View => {
            if storage.vacant.is_some() || storage.resident.is_some() {
                return Err("borrowed placement cannot declare owned content qualification roles");
            }
            None
        }
        operation => {
            let (vacant, resident) = storage.owned_content()?;
            domains.extend([vacant, resident]);
            Some(
                if operation == TerminalPlacementLifecycleOperation::Initialize {
                    vacant
                } else {
                    resident
                },
            )
        }
    };
    if domains
        .iter()
        .enumerate()
        .any(|(position, identity)| domains[..position].contains(identity))
        || domains.iter().any(|identity| {
            !module
                .structural_domains
                .iter()
                .any(|domain| domain.id == *identity && domain.carrier == storage.carrier)
        })
    {
        return Err("placement extent qualification roles are absent or aliased");
    }
    let boundary = module
        .boundary_machine(plan.boundary)
        .ok_or("placement lifecycle boundary is absent")?;
    let parameter = boundary
        .structural_parameters
        .first()
        .ok_or("placement extent input is absent")?;
    let input_multiplicity = match plan.operation {
        TerminalPlacementLifecycleOperation::View => {
            terminal_psi::StructuralMultiplicity::Unrestricted
        }
        TerminalPlacementLifecycleOperation::Initialize
        | TerminalPlacementLifecycleOperation::MoveOut => {
            terminal_psi::StructuralMultiplicity::Linear
        }
    };
    if parameter.structural_type != storage.carrier
        || parameter.multiplicity != input_multiplicity
        || !parameter.qualifications.contains(&storage.granted)
        || content.is_some_and(|content| !parameter.qualifications.contains(&content))
    {
        return Err("placement extent input does not retain its exact grant and content");
    }
    Ok(())
}
