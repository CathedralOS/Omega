//! Place the callbacks named by an installation's own registrar application.
//! These plans identify code and private destinations; they do not publish a
//! table, establish backing loans or authorize physical interrupt arrival.

use backend_plan::NativeCallbackPlacement;
use calling_conventions::{BoundaryCallingApplication, NativePlace};
use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
use terminal_psi::{TerminalInstalledInterruptRegistration, TerminalModule};

use super::placement::{calling_application, inbound_application, place_callback};

pub fn settle_installed_callback_placements(
    module: &TerminalModule,
    target_profile: target::TargetProfile,
    targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
    calling: &[BoundaryCallingApplication],
) -> Result<Vec<NativeCallbackPlacement>, String> {
    let mut placements = Vec::new();
    for registration in &module.installed_interrupt_registrations {
        let registrar_boundary = crate::optimization::selected_calling_boundary(
            module,
            targets,
            registration.registrar_boundary,
        )?;
        let registrar = calling_application(
            calling,
            &module.calling_abi_identity(registrar_boundary)?,
            Some(&registration.registrar_application),
            None,
        )?;
        if registrar.target != target_profile.native_target() {
            return Err("installed registrar calling plan belongs to another target".into());
        }
        let mut timer_destinations = 0usize;
        for callback in &registration.callbacks {
            if callback.registrar != registration.operation || !callback.entry_admitted {
                return Err(
                    "installed callback lost its checked occurrence or entry contract".into(),
                );
            }
            let (inbound_boundary, inbound) =
                inbound_application(module, targets, calling, callback)?;
            if inbound.target != registrar.target {
                return Err(
                    "installed callback entry and registrar plans select different targets".into(),
                );
            }
            let placement = place_callback(
                module,
                callback,
                inbound_boundary,
                inbound,
                registrar_boundary,
                registrar,
            )?;
            let materialization = placement
                .private_materialization
                .as_ref()
                .ok_or("installed callback has no complete private materialization")?;
            if let NativePlace::Field { .. } = materialization.destination {
                let layouts = registrar
                    .signature
                    .callback_layouts
                    .span(registrar.signature.callback_layout_rows)
                    .ok_or("installed registrar lost its callback layout catalog")?;
                let mut matching = layouts
                    .iter()
                    .filter(|layout| layout.destination == materialization.destination);
                let (Some(layout), None) = (matching.next(), matching.next()) else {
                    return Err("installed callback has no unique destination layout".into());
                };
                let slot = calling_conventions::callback_destination_slot(
                    module,
                    target_profile,
                    registrar,
                    layout,
                )?;
                if slot.application_commitment == registration.slot_commitment {
                    require_timer_slot(registration, slot)?;
                    timer_destinations += 1;
                }
            }
            placements.push(placement);
        }
        if timer_destinations != 1 {
            return Err(format!(
                "installation timer slot resolves to {timer_destinations} callback destinations; exactly one is required"
            ));
        }
    }
    Ok(placements)
}

fn require_timer_slot(
    registration: &TerminalInstalledInterruptRegistration,
    slot: &PrivateCallbackSlotSelection,
) -> Result<(), String> {
    if slot.application_encoding.as_ref() != registration.slot_encoding.as_slice() {
        return Err("installed timer destination changed its exact slot application".into());
    }
    Ok(())
}
