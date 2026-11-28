//! A saved transfer may only keep the domains its source still holds.
//!
//! A move or leaf copy names the domains its result carries. The declared
//! contract of the source place says what the place held when it was
//! introduced, but a call or store before the transfer can take a domain
//! away: a boundary call with external reach clears the grants of a
//! caller-owned record. Ingress therefore replays the transfer's machine up
//! to the transfer and requires the result's domains to follow from what the
//! source holds there. A domain the replayed source lacks may only appear on
//! the result when no prerequisite it is missing has establishment routes,
//! because only those routes may establish such a prerequisite.

use super::payload_history::selected_payload_contract_in;
use super::*;

/// Refuses a move or leaf copy whose result claims domains its source no
/// longer holds just before the transfer. Other operations pass unchanged.
pub fn validate_transfer_source_memberships(
    analyses: &crate::ModuleAnalyses<'_>,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), &'static str> {
    let module = analyses.module();
    let Some(result) = operation.result.structural() else {
        return Ok(());
    };
    let (source, path, presented) = match &operation.kind {
        OperationKind::MoveStructuralValue { source } => {
            (source.place, source.path.as_slice(), false)
        }
        // A copy without domains carries nothing the source must still hold.
        OperationKind::StructuralLeafCopy { source, path }
            if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() =>
        {
            (*source, path.as_slice(), true)
        }
        _ => return Ok(()),
    };
    let position = machine
        .blocks
        .iter()
        .find_map(|block| {
            let ordinal = block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation.id)?;
            u32::try_from(ordinal)
                .ok()
                .map(|ordinal| (block.id, ordinal))
        })
        .ok_or("value transfer position is absent")?;
    let types = &module.types.structural_types;
    let domains = &module.types.structural_domains;
    let rows = analyses
        .descriptor_closure(DescriptorSlotKind::Views, [machine.id])
        .map_err(|_| "value transfer source has no descriptor closure")?;
    let mut analysis = analyses.analysis(DescriptorSlotKind::Views, &rows);
    let mut current = selected_payload_contract_in(
        types,
        domains,
        &mut analysis,
        rows.view().find(machine.id),
        machine.id,
        position,
        source,
        path,
        &|place| terminal_payload_contract(machine, place),
    )?;
    if presented {
        current = current
            .presented(types, domains, result.structural_type)
            .ok_or("leaf copy memberships do not follow from every current payload source")?;
    }
    if current.carrier != result.structural_type {
        return Err("value transfer source currently holds another carrier");
    }
    let additions = result
        .qualifications
        .iter()
        .map(|domain| (&[][..], domain))
        .chain(
            result
                .projected_qualifications
                .iter()
                .map(|row| (row.path.as_slice(), &row.domain)),
        );
    for (path, domain) in additions {
        let declaration = domains
            .iter()
            .find(|row| row.id == *domain)
            .ok_or("value transfer domain is absent")?;
        let (supplied, _) = terminal_psi::StructuralPathQualification::select(
            &current.qualifications,
            &current.projected_qualifications,
            path,
        );
        let missing = declaration
            .missing_membership_requirements(domains, &supplied)
            .ok_or("value transfer has unknown prerequisites")?;
        if missing.iter().any(|row| row.establishment_routes.is_some()) {
            return Err("value transfer cannot establish unsupplied routed prerequisites");
        }
    }
    Ok(())
}
