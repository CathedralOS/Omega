//! Close checked callback applications onto emitted operations before publication.

use checked_trees::CheckedTrees;
use checked_trees::NominalMachineUseSite;
use lowered_psi::LoweredPsi;
use symbols::SymbolHandle;
use terminal_psi::TerminalCallbackApplication;

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    lowered: &mut LoweredPsi,
) -> Result<(), &'static str> {
    // Index the requested applications, not every emitted source call. Multiple
    // static-machine parameters at one call share its registrar operation.
    let mut requested = checked
        .facts
        .nominal_machine_uses
        .uses
        .iter()
        .filter(|usage| usage.callback_entry.is_some())
        .map(|usage| {
            (
                source_key(usage.site, usage.registration_operation),
                usage,
                None,
            )
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        lowered.semantic_module.callback_applications.clear();
        return Ok(());
    }
    requested.sort_unstable_by_key(|(key, _, _)| *key);
    for occurrence in &lowered.source_call_occurrences {
        let (Some(site), Some(operation)) =
            (occurrence.source_site, occurrence.terminal_site.operation())
        else {
            continue;
        };
        // Emission retains the checked call's complete closed target, while
        // nominal registration belongs to its authored declaration. Project
        // only this lookup address through the canonical instance owner; the
        // exact generational source site still identifies the application.
        // In particular, equal tuples can share a closed instance whose saved
        // application handle came from a different call. Neither that handle
        // nor an equal ABI can replace this source-site join.
        let registration = checked
            .closed_requirement_application(occurrence.source_target)
            .map_or(occurrence.source_target, |application| application.template);
        let key = source_key(site, registration);
        let first = requested.partition_point(|(candidate, _, _)| *candidate < key);
        for (_, _, registrar) in requested[first..]
            .iter_mut()
            .take_while(|(candidate, _, _)| *candidate == key)
        {
            if registrar.replace(operation).is_some() {
                return Err("callback application names multiple emitted registrar operations");
            }
        }
    }
    let mut applications = Vec::with_capacity(requested.len());
    for (_, usage, registrar) in requested {
        // This module need not contain every checked machine's callbacks.
        let Some(registrar) = registrar else { continue };
        applications.push(TerminalCallbackApplication {
            application_index: usage.application_index,
            registrar,
            static_machine_ordinal: usage.static_machine_ordinal,
            requirement: usage.canonical_requirement_overload.clone(),
            published_contract: usage
                .published_requirement_envelope
                .contract_commitment
                .as_bytes(),
            body: super::declarations::identity(
                identities,
                checked,
                sources,
                usage.selected_machine,
            )?,
        });
    }
    applications.sort_unstable_by_key(|application| application.application_index);
    lowered.semantic_module.callback_applications = applications;
    Ok(())
}

fn source_key(site: NominalMachineUseSite, target: SymbolHandle) -> (u8, u32, u32, u32, u32) {
    let (kind, arena_index, generation) = match site {
        NominalMachineUseSite::Statement(handle) => (0, handle.arena_index(), handle.generation()),
        NominalMachineUseSite::Expression(handle) => (1, handle.arena_index(), handle.generation()),
    };
    (
        kind,
        arena_index,
        generation,
        target.arena_index(),
        target.generation(),
    )
}
