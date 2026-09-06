//! Crash predicates are claims proved from entry and reconstructed CFG facts.

use super::*;

pub(in crate::validation) fn validate_site_guard_truth(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    // This private bridge runs only after complete structural validation. It
    // never consumes producer evidence or treats the claimed guards as facts.
    let sites = crate::verification::reconstruct_validated_crash_site_facts(module)?;
    for machine in &module.machines {
        if !machine.blocks.iter().any(|block| {
            matches!(&block.terminator, Terminator::Crash { site_guard, .. } if !site_guard.is_empty())
        }) {
            continue;
        }
        let context = machine_value_context(module, machine)?;
        for block in &machine.blocks {
            let Terminator::Crash {
                edge, site_guard, ..
            } = &block.terminator
            else {
                continue;
            };
            for (predicate, guard) in site_guard.iter().enumerate() {
                let mut paths = sites
                    .iter()
                    .filter(|site| {
                        site.machine == machine.id && site.block == block.id && site.edge == *edge
                    })
                    .peekable();
                let present = paths.peek().is_some();
                if !present
                    || !paths.all(|site| {
                        entry_requirements::establishes(
                        &context,
                        guard.proposition(),
                        &machine.contract.requires,
                        &site.semantic_axioms,
                    )
                    // An infeasible CFG path cannot reach this terminator.
                    // Its contradiction must itself have a kernel-checked
                    // certificate; failed proof search never removes a path.
                    || entry_requirements::establishes(
                        &context,
                        &Proposition::Falsehood,
                        &machine.contract.requires,
                        &site.semantic_axioms,
                    )
                    })
                {
                    return Err(ModuleError::CrashSiteGuardUnproved {
                        block: block.id,
                        edge: *edge,
                        predicate,
                    });
                }
            }
        }
    }
    Ok(())
}
