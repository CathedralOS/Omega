//! Rejoin a callback destination to its complete retained slot application.
//!
//! Calling plans identify the nominal field path. The selected native layout
//! supplies its exact conformance preimage; matching address geometry alone is
//! insufficient to choose an acknowledgement-producing installation entry.

use crate::{BoundaryCallbackLayout, BoundaryCallingApplication, NativePlace};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use terminal_psi::TerminalModule;

/// Follow the registrar formal's retained backing and nominal field path.
/// Scope's table operand is a separate occurrence: native installation must
/// still join that actual storage to this registrar destination.
pub fn callback_destination_slot<'module>(
    module: &'module TerminalModule,
    target_profile: target::TargetProfile,
    registrar: &BoundaryCallingApplication,
    layout: &BoundaryCallbackLayout,
) -> Result<&'module PrivateCallbackSlotSelection, String> {
    let signature = module
        .calling_abi_signature(registrar.abi.id())
        .ok_or("installed registrar lost its exact declaration ABI")?;
    if signature.identity(&module.identities)? != registrar.abi {
        return Err("installed registrar changed its complete declaration ABI".into());
    }
    let storage = signature.storage.as_ref().map_err(Clone::clone)?;
    let formal = storage
        .telescope
        .parameters
        .get(layout.formal_ordinal as usize)
        .ok_or("installed callback layout has no registrar formal")?;
    if !matches!(layout.destination, NativePlace::Field { parameter, .. } if parameter == formal.identity)
        || layout.native_ordinal != formal.native_ordinal
    {
        return Err("installed callback layout changed its exact registrar formal".into());
    }
    let mut record_id = storage
        .parameters
        .get(layout.formal_ordinal as usize)
        .and_then(|parameter| parameter.callback_backing)
        .ok_or("installed callback destination has no retained formal backing")?;
    let mut expected_identity = layout.root.identity.as_str();
    for field in layout.fields()? {
        let record = module
            .native_layouts
            .record(record_id)
            .ok_or("installed callback field lost its native record")?;
        if module.identities.text(record.data_identity) != Some(expected_identity) {
            return Err("installed callback path changed its owning record application".into());
        }
        let mut matching = module
            .native_layouts
            .fields(record)?
            .iter()
            .filter(|candidate| {
                module.identities.text(candidate.declaration_identity)
                    == Some(field.identity.as_str())
            });
        let (Some(selected), None) = (matching.next(), matching.next()) else {
            return Err("installed callback path has no unique declared field".into());
        };
        record_id = module
            .structural_type(record_id)
            .and_then(|record| record.shape.record_fields())
            .and_then(|fields| fields.iter().find(|field| field.id == selected.field))
            .and_then(|field| field.field_type.as_structural())
            .ok_or("installed callback path does not retain an inline child record")?;
        expected_identity = &field.child.identity;
    }
    let record = module
        .native_layouts
        .record(record_id)
        .ok_or("installed callback slot lost its containing native record")?;
    if module.identities.text(record.data_identity) != Some(expected_identity) {
        return Err("installed callback slot changed its containing record application".into());
    }
    let realization = module
        .native_layouts
        .for_target(record, target_profile.target_name())?
        .ok_or("installed callback slot has no selected authored layout")?;
    let mut demands = realization
        .report
        .private_callback_demands
        .iter()
        .filter(|demand| demand.slot_identity == layout.terminal.identity);
    let (Some(demand), None) = (demands.next(), demands.next()) else {
        return Err("installed callback destination has no unique selected slot demand".into());
    };
    let slot = &demand.slot_application;
    slot.validate_identity_encoding()?;
    if slot.application_commitment != layout.terminal.conformance_commitment
        || slot.requirement_identity != layout.terminal.requirement_identity
        || demand.callback_requirement_identity != layout.terminal.requirement_identity
        || slot.layout_identity != layout.terminal.policy
        || demand.layout_subject_identity != layout.terminal.policy
        || demand.placements.as_slice() != layout.terminal.placements.as_slice()
        || module.identities.text(record.semantic_identity) != Some(slot.semantic_identity.as_str())
    {
        return Err("installed callback layout differs from its selected slot demand".into());
    }
    Ok(slot)
}
