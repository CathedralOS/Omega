//! Rejoin the nominated continuation's checked formals to the target's visible arrival contract.
//! Capturing a parameter describes its required grant. Only the physical handoff can supply it.

use calling_conventions::terminal_layout::NativeGeometry;
use program_entry_contracts::{
    ProgramEntryExtentValueLayout, ProgramEntryVisibleParameterSignature,
    SelectedProgramEntrySignature,
};
use terminal_psi::{
    BoundaryParameterKind, CheckedMachineDeclaration, StructuralTypeDeclaration, TerminalMachine,
    TerminalModule,
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
    if slot.visible_parameters.is_empty() {
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
    let arguments = slot.visible_parameters;
    let parameters = &machine.structural_parameters;
    if parameters.len() != arguments.len()
        || machine.parameter_order.len() != arguments.len()
        || machine.parameter_order.iter().any(|kind| *kind != BoundaryParameterKind::Structural)
        || checked.parameter_types.len() != arguments.len()
        || method.has_result
        || machine.entry_claims.len() != arguments.len()
        || method.entry_claims.len() != method.parameter_count
    {
        return Err("storage entry has extra or missing parameters or claims".into());
    }
    let mut places = std::collections::BTreeSet::new();
    let mut claim_ids = std::collections::BTreeSet::new();
    for claim in &machine.entry_claims {
        if !claim.path.is_empty() || !places.insert(claim.input) || !claim_ids.insert(claim.claim) {
            return Err("storage entry requires a distinct whole claim for each argument".into());
        }
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
    let mut signature = Vec::with_capacity(arguments.len());
    for (ordinal, (argument, parameter)) in arguments.iter().zip(parameters).enumerate() {
        if !places.remove(&parameter.place) {
            return Err("storage entry parameter lacks its own whole claim".into());
        }
        let claim = method
            .entry_claims
            .iter()
            .find(|claim| claim.parameter_index == argument.requirement_parameter)
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
            argument.role,
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
    let (base, length) = terminal_semantics::program_storage_entry::extent_fields(module, carrier)?;
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
