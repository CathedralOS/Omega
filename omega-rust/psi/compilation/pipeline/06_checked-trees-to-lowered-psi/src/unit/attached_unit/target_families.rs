//! Rejoin admitted family handles to the one emitted callable namespace.
//! Boundary contracts and checked bodies retain distinct identities and supply.

use super::{
    BoundaryParameters, CheckedTrees, LoweringError, UnitPlans, unique_unit_boundary, unsupported,
};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::{
    TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
};

/// Concrete runtime families share one eligibility decision across discovery,
/// emission and the product completeness check. Open declarations are checked
/// templates, not callable bodies with a closed storage/ABI shape. Their closed
/// specialization cohorts retain every member here; a missing concrete plan
/// must still fail. Terminal specialization identities are not template openings.
pub(crate) fn runtime_families(
    checked: &CheckedTrees,
) -> Result<Vec<&typed_trees::machine::TargetMachineFamily>, LoweringError> {
    let mut families = Vec::new();
    for family in &checked.typed.target_machine_families {
        if family.purpose != typed_trees::machine::MachinePurpose::Runtime {
            continue;
        }
        let head =
            checked
                .typed
                .machine_by_symbol(family.canonical)
                .ok_or(LoweringError::Unsupported(
                    "target-family head lost its typed declaration",
                ))?;
        let open = head.has_specialization_parameters();
        for member in &family.members {
            let member = checked.typed.machine_by_symbol(member.machine).ok_or(
                LoweringError::Unsupported("target-family member lost its typed declaration"),
            )?;
            if member.has_specialization_parameters() != open {
                return unsupported("target-family cohort has mixed open and closed declarations");
            }
        }
        if !open {
            families.push(family);
        }
    }
    Ok(families)
}

pub(super) fn callable(
    source: SymbolHandle,
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
) -> Result<TargetMachineCallable, LoweringError> {
    let mut owners = machines
        .iter()
        .filter(|(symbol, _)| *symbol == source)
        .map(|(_, identity)| TargetMachineCallable::Machine(*identity))
        .chain(
            boundaries
                .iter()
                .filter(|boundary| boundary.source == source)
                .map(|boundary| TargetMachineCallable::Boundary(boundary.id)),
        );
    let owner = owners.next().ok_or(LoweringError::Unsupported(
        "target-family member has no emitted callable",
    ))?;
    if owners.next().is_some() {
        return unsupported("target-family member has multiple emitted callables");
    }
    Ok(owner)
}

pub(super) fn lower(
    checked: &CheckedTrees,
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
) -> Result<Vec<TargetMachineFamily>, LoweringError> {
    let mut families = Vec::with_capacity(checked.typed.target_machine_families.len());
    for family in runtime_families(checked)? {
        let canonical = callable(family.canonical, machines, boundaries)?;
        // Carry the checked published head row, never a selected member's
        // private contract or a digest reconstructed from display identity.
        // Legacy canonical-as-member families retain that exact callable's
        // checked commitment; missing provenance is not an empty contract.
        let head_contract = match canonical {
            TargetMachineCallable::Boundary(_) => {
                unique_unit_boundary(UnitPlans::published(&checked.facts.flow), family.canonical)
                    .map_err(crate::lowering_error::unit_machine_plan_error(
                        checked,
                        checked.symbols.name(family.canonical),
                        family.canonical,
                    ))?
                    .contract_commitment
            }
            TargetMachineCallable::Machine(_) => {
                checked
                    .facts
                    .contract_plans
                    .for_machine(family.canonical)
                    .ok_or(LoweringError::Unsupported(
                        "target-family executable head has no checked contract commitment",
                    ))?
                    .commitment
            }
        };
        if head_contract.is_zero() {
            return unsupported("target-family head has an empty checked contract commitment");
        }
        let mut members = Vec::with_capacity(family.members.len());
        for member in &family.members {
            let machine = checked.typed.machine_by_symbol(member.machine).ok_or(
                LoweringError::Unsupported("target-family member lost its typed declaration"),
            )?;
            let supply = match machine.supply_mode {
                language_semantics::MachineSupplyMode::CheckedBody => {
                    TargetMachineSupply::CheckedBody
                }
                language_semantics::MachineSupplyMode::Requirement => {
                    TargetMachineSupply::Requirement
                }
                language_semantics::MachineSupplyMode::TopLevelRequirement => {
                    TargetMachineSupply::TopLevelRequirement
                }
                language_semantics::MachineSupplyMode::Boundary => TargetMachineSupply::Boundary,
                language_semantics::MachineSupplyMode::AdmissionClaim => {
                    TargetMachineSupply::AdmissionClaim
                }
                language_semantics::MachineSupplyMode::ExternalRealization { .. } => {
                    TargetMachineSupply::ExternalRealization
                }
            };
            members.push(TargetMachineFamilyMember {
                target: member.target.as_str().to_owned(),
                callable: callable(member.machine, machines, boundaries)?,
                supply,
            });
        }
        families.push(TargetMachineFamily {
            canonical,
            head_contract: head_contract.as_bytes(),
            members,
        });
    }
    families.sort_by_key(|family| family.canonical);
    Ok(families)
}
