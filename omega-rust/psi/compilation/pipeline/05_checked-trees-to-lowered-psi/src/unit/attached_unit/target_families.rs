//! Rejoin admitted family handles to the one emitted callable namespace.
//! Boundary contracts and checked bodies retain distinct identities and supply.

use super::{BoundaryParameters, CheckedTrees, LoweringError, unsupported};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::{
    TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
};

pub(super) fn lower(
    checked: &CheckedTrees,
    machines: &[(SymbolHandle, MachineId)],
    boundaries: &[BoundaryParameters],
) -> Result<Vec<TargetMachineFamily>, LoweringError> {
    let callable = |source: SymbolHandle| {
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
    };
    let mut families = Vec::with_capacity(checked.typed.target_machine_families.len());
    for family in &checked.typed.target_machine_families {
        if family.purpose != typed_trees::machine::MachinePurpose::Runtime {
            continue;
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
                callable: callable(member.machine)?,
                supply,
            });
        }
        families.push(TargetMachineFamily {
            canonical: callable(family.canonical)?,
            members,
        });
    }
    families.sort_by_key(|family| family.canonical);
    Ok(families)
}
