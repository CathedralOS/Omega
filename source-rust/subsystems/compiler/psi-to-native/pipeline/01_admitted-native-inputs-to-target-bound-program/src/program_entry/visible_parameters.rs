//! Rejoin the nominated continuation's checked formals to the target's visible arrival contract.
//! Capturing a parameter describes its required grant. Only the physical handoff can supply it.

use calling_conventions::terminal_layout::NativeGeometry;
use program_entry_contracts::{
    ProgramEntryExtentValueLayout, ProgramEntryVisibleParameterSignature,
    ProgramStorageEntryRootRole, SelectedProgramEntrySignature,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_psi::{
    BoundaryParameterKind, CheckedMachineDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(super) fn from_terminal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    checked: &CheckedMachineDeclaration,
    slot: target::ProgramEntrySlotDeclaration,
) -> Result<Vec<ProgramEntryVisibleParameterSignature>, String> {
    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .ok_or("the selected entry has no entry block")?;
    if !machine.parameters.is_empty()
        || !entry.parameters.is_empty()
        || !entry.structural_parameters.is_empty()
        || !entry.erased_scalar_formals.is_empty()
        || !entry.erased_proof_formals.is_empty()
    {
        return Err("the selected entry exposes additional arguments outside its slot".into());
    }
    if slot.visible_parameters == target::ProgramEntryVisibleParameters::None {
        if !machine.structural_parameters.is_empty()
            || !machine.parameter_order.is_empty()
            || !checked.parameter_types.is_empty()
        {
            return Err(
                "the selected hosted entry exposes arguments its slot does not provide".into(),
            );
        }
        return Ok(Vec::new());
    }
    let contract = module.program_entry_contract(slot.owner.target_name())?;
    if contract.entry != machine.id {
        return Err(
            "the selected storage entry differs from the installed target nomination".into(),
        );
    }
    let schema = module.service_schema(contract.schema_declaration, contract.schema_application)?;
    let surface = &schema.surface;
    let method = super::published::selected_method(surface, slot.semantic_arrival_requirement)?;
    let ids = &module.identities;
    if service_surface::declared_type_signature(
        checked
            .parameter_types
            .iter()
            .map(|ty| ids.text(*ty).unwrap_or_default()),
        ids.text(checked.result_type).unwrap_or_default(),
    )? != method.declared_type_signature
    {
        return Err(
            "storage entry declared types differ from the installed arrival requirement".into(),
        );
    }
    let [image, storage] = machine.structural_parameters.as_slice() else {
        return Err("storage entry must consume two separate granted Extent parameters".into());
    };
    let [first, second] = machine.entry_claims.as_slice() else {
        return Err("storage entry must own a claim for each granted extent".into());
    };
    if machine.parameter_order != [BoundaryParameterKind::Structural; 2]
        || checked.parameter_types.len() != 2
        || method.parameter_count != 2
        || method.has_result
        || method.entry_claims.len() != 2
        || image.place == storage.place
        || first.claim == second.claim
        || first.input == second.input
        || ![first, second]
            .iter()
            .all(|c| c.path.is_empty() && (c.input == image.place || c.input == storage.place))
    {
        return Err("storage entry has extra or substituted parameters or claims".into());
    }
    let requirement = ids
        .index_of(surface.identity_text(method.requirement_identity))
        .ok_or("storage entry lost the semantic requirement accepting the grants")?;
    let granted = terminal_semantics::program_storage_entry::ProgramStorageGrant::from_module(
        module,
        requirement,
    )?;
    let geometry = NativeGeometry::new(&module.types.structural_types)
        .map_err(|error| format!("storage entry has invalid type geometry: {error:?}"))?;
    let roles = [
        ProgramStorageEntryRootRole::Image,
        ProgramStorageEntryRootRole::InitialStorage,
    ];
    let mut signature = Vec::with_capacity(roles.len());
    for (ordinal, (role, parameter)) in roles.into_iter().zip([image, storage]).enumerate() {
        let claim = method
            .entry_claims
            .iter()
            .find(|claim| claim.parameter_index == ordinal)
            .ok_or("storage entry has no declared grant for its parameter")?;
        let carrier = module
            .structural_type(parameter.structural_type)
            .ok_or("storage entry lost its structural parameter declaration")?;
        let carrier_identity = surface.identity_text(claim.carrier_identity);
        if !granted.accepts(ordinal, parameter)
            || checked.parameter_types[ordinal] != granted.declared_type
            || ids.text(carrier.identity) != Some(carrier_identity)
            || claim.domain != terminal_semantics::program_storage_entry::SURFACE_GRANT
            || claim.authority_flow != provider_plans::ServiceEntryAuthorityFlow::Accepts
        {
            return Err(format!(
                "storage entry parameter {ordinal} must carry the installed whole Extent grant"
            ));
        }
        let layout = extent_layout(module, &geometry, carrier)?;
        signature.push(SelectedProgramEntrySignature::visible_parameter(
            role,
            ordinal,
            checked.parameter_types[ordinal],
            layout.shape(),
            layout,
            false,
            false,
        ));
    }
    Ok(signature)
}

fn extent_layout(
    module: &TerminalModule,
    geometry: &NativeGeometry<'_>,
    carrier: &StructuralTypeDeclaration,
) -> Result<ProgramEntryExtentValueLayout, String> {
    let StructuralTypeShape::Record { fields, .. } = &carrier.shape else {
        return Err("storage entry requires the checked Extent record".into());
    };
    let [base, length] = fields.as_slice() else {
        return Err("storage entry requires the Extent base and length fields".into());
    };
    for (field, name, integer) in [
        (base, "base", IntegerType::address(64).unwrap()),
        (
            length,
            "length",
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ),
    ] {
        if module.identities.text(field.identity) != Some(name)
            || field.relevance.is_erased()
            || field.field_type != StructuralFieldType::Scalar(ScalarType::Integer(integer))
        {
            return Err(
                "the checked Extent layout lost its address base or unsigned length".into(),
            );
        }
    }
    let geometry_error = |error| format!("storage entry Extent has invalid geometry: {error:?}");
    let base = geometry
        .field_placement(base, carrier.id, 0)
        .map_err(geometry_error)?;
    let after_base = base
        .byte_offset
        .checked_add(base.shape.byte_size)
        .ok_or("Extent base field position overflows")?;
    let length = geometry
        .field_placement(length, carrier.id, after_base)
        .map_err(geometry_error)?;
    ProgramEntryExtentValueLayout::from_checked_record(
        base.byte_offset
            .try_into()
            .map_err(|_| "Extent base offset exceeds entry ABI")?,
        base.shape,
        length
            .byte_offset
            .try_into()
            .map_err(|_| "Extent length offset exceeds entry ABI")?,
        length.shape,
        geometry.shape(carrier.id).map_err(geometry_error)?,
    )
}
