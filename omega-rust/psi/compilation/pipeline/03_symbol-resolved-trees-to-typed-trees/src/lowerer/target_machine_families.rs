//! Publish the resolved family membership of bodies that survived typing.
//! Neither target selection nor source-name reconstruction belongs here.

use crate::lowerer::name::lower_name;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;
use typed_trees::machine::{TargetMachineFamily, TargetMachineFamilyMember};

pub(super) fn append(
    source: &SymbolResolvedTrees,
    typed: &mut TypedTrees,
    machine_frontier: usize,
) -> Result<(), Diagnostic> {
    let mut declarations = source
        .machines
        .iter()
        .skip(machine_frontier)
        .filter(|machine| machine.target_family.is_valid())
        .collect::<Vec<_>>();
    declarations.sort_by_key(|machine| machine.target_family.arena_index());
    let mut families = Vec::new();
    for declarations in
        declarations.chunk_by(|left, right| left.target_family == right.target_family)
    {
        let canonical = declarations[0].target_family;
        let Some(machine) = typed.machine_by_symbol(canonical) else {
            // Provider defaults may disappear during pre-check evaluation.
            continue;
        };
        if typed
            .target_machine_families
            .iter()
            .any(|family| family.canonical == canonical)
        {
            return Err(Diagnostic::error(
                "seeded typing cannot append a body to a retained target-machine family",
            ));
        }
        let mut members = declarations
            .iter()
            .filter_map(|declaration| {
                typed.machine_by_symbol(declaration.symbol)?;
                Some(TargetMachineFamilyMember {
                    target: lower_name(declaration.authored_target.as_ref()?),
                    machine: declaration.symbol,
                })
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.target.as_str().cmp(right.target.as_str()));
        families.push(TargetMachineFamily {
            name: machine.name.clone(),
            canonical,
            members,
        });
    }
    // Preserve a retained base byte-for-byte; new cohorts are ordered within
    // their own append, never globally sorted across the retained prefix.
    families.sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
    typed.target_machine_families.extend(families);
    Ok(())
}
