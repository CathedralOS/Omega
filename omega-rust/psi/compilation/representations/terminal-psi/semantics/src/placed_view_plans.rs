//! Rejoin retained placement geometry to its exact logical and callable catalogs.
//! Success describes a valid demand; it grants neither backing nor access.
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineResult, StructuralAccess, StructuralTypeShape, TerminalModule,
    TerminalPlacedAccessorKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedPlanError {
    DuplicateCarrier,
    DuplicateAccessor,
    CarrierMissing,
    SchemaMissing,
    CarrierNotRecord,
    SchemaNotRecord,
    IdentityMissing,
    ViewIdentityMismatch,
    InputPlanMismatch,
    InputMissing {
        machine: semantic_vocabulary::MachineId,
        runtime_position: usize,
    },
    InputDuplicate {
        machine: semantic_vocabulary::MachineId,
        runtime_position: u32,
    },
    FieldSlotMismatch,
    ViewFieldMismatch,
    SchemaFieldMismatch,
    AccessorMissing,
    DuplicateBoundary,
    BoundaryMissing,
    BoundaryCarrierMismatch,
    OperationNotAdmitted,
    OperationSignatureMismatch,
    CarrierStorageMismatch,
}

/// Rejoin complete normalized geometry and scalar declarations without
/// manufacturing machine-input coverage or an active storage/loan grant.
pub fn validate_placed_view_catalog(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
    plans: &[terminal_psi::TerminalPlacedViewPlan],
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
) -> Result<(), PlacedPlanError> {
    let mut carriers = BTreeSet::new();
    let mut seen_boundaries = BTreeSet::new();
    let mut accessors = BTreeSet::new();
    for declaration in declarations {
        if matches!(
            declaration.shape,
            StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_),
                ..
            }
        ) && !plans.iter().any(|plan| {
            plan.carrier == declaration.id
                || plan
                    .fields
                    .iter()
                    .any(|field| field.accessor == declaration.id)
        }) {
            return Err(PlacedPlanError::CarrierStorageMismatch);
        }
    }
    for plan in plans {
        if !carriers.insert(plan.carrier) {
            return Err(PlacedPlanError::DuplicateCarrier);
        }
        let view = declarations
            .iter()
            .find(|row| row.id == plan.carrier)
            .ok_or(PlacedPlanError::CarrierMissing)?;
        let schema = declarations
            .iter()
            .find(|row| row.id == plan.schema)
            .ok_or(PlacedPlanError::SchemaMissing)?;
        let StructuralTypeShape::Record {
            fields: view_fields,
            storage: terminal_psi::StructuralRecordStorage::PlacedView(geometry),
        } = &view.shape
        else {
            return Err(PlacedPlanError::CarrierNotRecord);
        };
        if Some(geometry.byte_size) != plan.placement.layout().size
            || geometry.byte_alignment != plan.placement.layout().align
            || geometry.byte_alignment == 0
            || !geometry.byte_alignment.is_power_of_two()
        {
            return Err(PlacedPlanError::CarrierStorageMismatch);
        }
        let StructuralTypeShape::Record {
            fields: schema_fields,
            ..
        } = &schema.shape
        else {
            return Err(PlacedPlanError::SchemaNotRecord);
        };
        let text = |identity| {
            identities
                .text(identity)
                .filter(|text| !text.is_empty())
                .ok_or(PlacedPlanError::IdentityMissing)
        };
        let expected = terminal_psi::canonical_placed_view_identity(
            text(plan.policy_identity)?,
            text(plan.schema_identity)?,
        );
        if text(plan.view_identity)? != expected {
            return Err(PlacedPlanError::ViewIdentityMismatch);
        }
        text(plan.policy_plan_machine_identity)?;
        let mut slots = BTreeSet::new();
        for field in &plan.fields {
            if !accessors.insert(field.accessor) {
                return Err(PlacedPlanError::DuplicateAccessor);
            }
            if !slots.insert(field.access_slot) {
                return Err(PlacedPlanError::FieldSlotMismatch);
            }
            let access = plan
                .placement
                .access()
                .plan()
                .entries()
                .get(field.access_slot as usize)
                .filter(|entry| entry.key().slot() == field.access_slot)
                .ok_or(PlacedPlanError::FieldSlotMismatch)?;
            let view_field = view_fields
                .iter()
                .find(|row| {
                    row.identity == field.view_field
                        && row.field_type.as_structural() == Some(field.accessor)
                })
                .ok_or(PlacedPlanError::ViewFieldMismatch)?;
            let schema_field = schema_fields
                .iter()
                .find(|row| row.identity == field.schema_field)
                .ok_or(PlacedPlanError::SchemaFieldMismatch)?;
            if view_field.identity != schema_field.identity {
                return Err(PlacedPlanError::SchemaFieldMismatch);
            }
            // These are canonical normalized member identities, not source
            // lookup names: numbered members are invariant under renaming.
            let layout_entry = plan
                .placement
                .layout()
                .entries
                .iter()
                .find(|entry| entry.field == access.field())
                .ok_or(PlacedPlanError::FieldSlotMismatch)?;
            let canonical_member = layout_entry.member_identity.map_or_else(
                || layout_entry.field.clone(),
                |identity| format!("#{identity}"),
            );
            if text(field.schema_field)? != canonical_member {
                return Err(PlacedPlanError::SchemaFieldMismatch);
            }
            let accessor = declarations
                .iter()
                .find(|row| row.id == field.accessor)
                .ok_or(PlacedPlanError::AccessorMissing)?;
            let StructuralTypeShape::Record {
                fields: accessor_fields,
                storage: terminal_psi::StructuralRecordStorage::PlacedAccessor(accessor_geometry),
            } = &accessor.shape
            else {
                return Err(PlacedPlanError::AccessorMissing);
            };
            let descriptor = plan.placement.access().field_descriptor(access.key());
            let expected_size =
                descriptor.map_or(0, |descriptor| descriptor.effect_footprint().length_bytes());
            let expected_storage =
                descriptor.map(
                    |descriptor| semantic_vocabulary::AuthoredFieldStorage::Direct {
                        byte_offset: descriptor.container_byte_offset(),
                    },
                );
            if !accessor_fields.is_empty()
                || accessor_geometry.byte_size != expected_size
                || accessor_geometry.byte_alignment != 1
                || view_field.storage != expected_storage
            {
                return Err(PlacedPlanError::CarrierStorageMismatch);
            }
            for operation in &field.operations {
                if !seen_boundaries.insert(operation.boundary) {
                    return Err(PlacedPlanError::DuplicateBoundary);
                }
                let boundary = boundaries
                    .iter()
                    .find(|row| row.id == operation.boundary)
                    .ok_or(PlacedPlanError::BoundaryMissing)?;
                let name = match operation.kind {
                    TerminalPlacedAccessorKind::Read => "read",
                    TerminalPlacedAccessorKind::Take => "take",
                    TerminalPlacedAccessorKind::Write => "write",
                };
                if !access.access().operation_names().contains(&name) {
                    return Err(PlacedPlanError::OperationNotAdmitted);
                }
                let Some(parameter) = boundary.structural_parameters.first() else {
                    return Err(PlacedPlanError::BoundaryCarrierMismatch);
                };
                let expected_access = if operation.kind == TerminalPlacedAccessorKind::Read {
                    StructuralAccess::SharedBorrow
                } else {
                    StructuralAccess::MutableBorrow
                };
                if parameter.structural_type != field.accessor
                    || parameter.access != expected_access
                {
                    return Err(PlacedPlanError::BoundaryCarrierMismatch);
                }
                let signature_matches = if let Some(scalar) = schema_field.field_type.scalar_type()
                {
                    match operation.kind {
                        TerminalPlacedAccessorKind::Read | TerminalPlacedAccessorKind::Take => {
                            boundary.structural_parameters.len() == 1
                                && boundary.scalar_parameters.is_empty()
                                && boundary.result == BoundaryMachineResult::Scalar(scalar)
                        }
                        TerminalPlacedAccessorKind::Write => {
                            boundary.structural_parameters.len() == 1
                                && boundary.scalar_parameters == [scalar]
                                && boundary.result == BoundaryMachineResult::Unit
                        }
                    }
                } else if let Some(structural) = schema_field.field_type.as_structural() {
                    match operation.kind {
                        TerminalPlacedAccessorKind::Read | TerminalPlacedAccessorKind::Take => {
                            boundary.structural_parameters.len() == 1
                                && boundary.scalar_parameters.is_empty()
                                && matches!(&boundary.result,BoundaryMachineResult::Structural(result) if result.structural_type==structural)
                        }
                        TerminalPlacedAccessorKind::Write => {
                            boundary.structural_parameters.len() == 2
                                && boundary.structural_parameters[1].structural_type == structural
                                && boundary.scalar_parameters.is_empty()
                                && boundary.result == BoundaryMachineResult::Unit
                        }
                    }
                } else {
                    false
                };
                if !signature_matches {
                    return Err(PlacedPlanError::OperationSignatureMismatch);
                }
            }
        }
        if slots.len() != plan.placement.access().plan().entries().len()
            || view_fields.len() != plan.fields.len()
        {
            return Err(PlacedPlanError::FieldSlotMismatch);
        }
    }
    Ok(())
}

pub fn validate_placed_view_plans(module: &TerminalModule) -> Result<(), PlacedPlanError> {
    validate_placed_view_catalog(
        &module.structural_types,
        &module.identities,
        &module.placed_view_plans,
        &module.boundary_machines,
    )?;
    let carriers = module
        .placed_view_plans
        .iter()
        .map(|plan| plan.carrier)
        .collect::<BTreeSet<_>>();
    let mut inputs = BTreeSet::new();
    for input in &module.placed_view_inputs {
        if !inputs.insert((input.machine, input.runtime_position)) {
            return Err(PlacedPlanError::InputDuplicate {
                machine: input.machine,
                runtime_position: input.runtime_position,
            });
        }
        let parameter = crate::placed_view_input_parameter(module, input)
            .map_err(|_| PlacedPlanError::InputPlanMismatch)?;
        let plan = module
            .placed_view_plans
            .iter()
            .find(|plan| plan.carrier == parameter.structural_type)
            .ok_or(PlacedPlanError::InputPlanMismatch)?;
        if (
            input.view_identity,
            input.policy_identity,
            input.policy_plan_machine_identity,
            input.schema_identity,
            input.placement_commitment,
            input.placement_report_fingerprint,
        ) != (
            plan.view_identity,
            plan.policy_identity,
            plan.policy_plan_machine_identity,
            plan.schema_identity,
            plan.placement.content_interpretation().commitment(),
            plan.placement.identity().compatibility_fingerprint(),
        ) {
            return Err(PlacedPlanError::InputPlanMismatch);
        }
    }
    // A missing row must not turn a protected borrowed formal into ordinary
    // pointer ingress. Check the inverse relation here so every semantic
    // consumer receives the same coverage as canonical codec admission.
    for machine in &module.machines {
        for (runtime_position, parameter) in machine.structural_parameters.iter().enumerate() {
            if parameter.access != StructuralAccess::Owned
                && carriers.contains(&parameter.structural_type)
                && !u32::try_from(runtime_position)
                    .ok()
                    .is_some_and(|position| inputs.contains(&(machine.id, position)))
            {
                return Err(PlacedPlanError::InputMissing {
                    machine: machine.id,
                    runtime_position,
                });
            }
        }
    }
    Ok(())
}

/// Ordinary inline value transport cannot create or own a placement loan.
/// References stop this walk: their referents require separate live custody.
pub fn contains_placed_carrier(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
) -> bool {
    contains_placed_carrier_in_catalog(&module.structural_types, &module.placed_view_plans, root)
}

/// Same ownership judgment over the source-free catalog retained by native IRs.
pub fn contains_placed_carrier_in_catalog(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    plans: &[terminal_psi::TerminalPlacedViewPlan],
    root: semantic_vocabulary::StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(identity) = pending.pop() {
        if !visited.insert(identity) {
            continue;
        }
        if plans.iter().any(|plan| {
            plan.carrier == identity || plan.fields.iter().any(|field| field.accessor == identity)
        }) {
            return true;
        }
        let Some(declaration) = declarations.iter().find(|row| row.id == identity) else {
            return true;
        };
        let mut fields = Vec::new();
        match &declaration.shape {
            StructuralTypeShape::Record {
                storage:
                    terminal_psi::StructuralRecordStorage::PlacedView(_)
                    | terminal_psi::StructuralRecordStorage::PlacedAccessor(_),
                ..
            } => return true,
            StructuralTypeShape::Record { fields: rows, .. } => fields.extend(rows),
            StructuralTypeShape::Mixed {
                fields: rows,
                cases,
            } => {
                fields.extend(rows);
                for case in cases {
                    fields.extend(&case.fields);
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields.extend(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
                pending.push(*element)
            }
            _ => {}
        }
        for field in fields
            .into_iter()
            .filter(|field| !field.relevance.is_erased())
        {
            if let Some(child) = field.field_type.as_structural() {
                pending.push(child);
            }
        }
    }
    false
}
