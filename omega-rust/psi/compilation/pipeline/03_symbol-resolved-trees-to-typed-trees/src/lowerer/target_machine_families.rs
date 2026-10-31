//! Admit and publish the resolved family membership of bodies that survived typing.
//! Neither target selection nor source-name reconstruction belongs here.

use crate::lowerer::name::lower_name;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;
use typed_trees::machine::{TargetMachineFamily, TargetMachineFamilyMember};

mod telescopes;

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
    // Every comparison reads this same completed typing frontier. Reuse its
    // program-fixed contract facts, then discard them before publishing the
    // appended families; a later seeded append starts a fresh invocation.
    let checking = validation::CheckingContext::new(typed);
    let mut families = Vec::new();
    for declarations in
        declarations.chunk_by(|left, right| left.target_family == right.target_family)
    {
        let canonical = declarations[0].target_family;
        let Some(machine) = typed.machine_by_symbol(canonical) else {
            return Err(Diagnostic::error(
                "target-machine family interface did not survive typing",
            ));
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
        let mut members = Vec::new();
        for declaration in declarations {
            let Some(target) = declaration.authored_target.as_ref() else {
                continue;
            };
            if typed.machine_by_symbol(declaration.symbol).is_none() {
                return Err(Diagnostic::error(
                    "target-machine family member did not survive typing",
                ));
            }
            members.push(TargetMachineFamilyMember {
                target: lower_name(target),
                machine: declaration.symbol,
            });
        }
        if members.is_empty() {
            return Err(Diagnostic::error(
                "target-machine family has no retained members",
            ));
        }
        members.sort_by(|left, right| left.target.as_str().cmp(right.target.as_str()));
        let canonical_identity = typed.machine_callable_identity(machine);
        let canonical_result = typed.machine_entry_result_type_identity(machine);
        for member in &members {
            let member_machine = typed.machine_by_symbol(member.machine).ok_or_else(|| {
                Diagnostic::error("target-machine family member did not survive typing")
            })?;
            let member_identity = typed.machine_callable_identity(member_machine);
            telescopes::validate(&checking, machine, member_machine)?;
            if !canonical_identity.is_valid()
                || !member_identity.is_valid()
                || member_identity != canonical_identity
                || typed.machine_entry_result_type_identity(member_machine) != canonical_result
                || member_machine.purpose != machine.purpose
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` has an incompatible signature for `{}` -- \
                     every target's body of one machine shares its signature",
                    machine.name, member.target,
                )));
            }
        }
        families.push(TargetMachineFamily {
            name: machine.name.clone(),
            canonical,
            purpose: machine.purpose,
            members,
        });
    }
    // Preserve a retained base byte-for-byte; new cohorts are ordered within
    // their own append, never globally sorted across the retained prefix.
    families.sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
    drop(checking);
    typed.target_machine_families.extend(families);
    Ok(())
}
