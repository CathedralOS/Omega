//! Crash predicates are claims proved from entry and reconstructed CFG facts.

use super::super::machine_value_context;
use super::{ModuleError, Proposition, TerminalModule, Terminator, entry_requirements};
pub(in crate::validation) fn validate_site_guard_truth(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    // This private bridge runs only after complete structural validation. It
    // never consumes producer evidence or treats the claimed guards as facts.
    let sites = crate::verification::reconstruct_validated_crash_site_facts(module)?;
    // The producer stage runs the bounded searches; consumption below only
    // re-decides each supplied node through the kernel check.
    let certificates = entry_requirements::certify_crash_sites(
        module,
        sites.iter().map(|site| {
            (
                site.machine,
                site.block,
                site.edge,
                site.semantic_axioms.as_slice(),
            )
        }),
    )?;
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
                    .zip(&certificates)
                    .filter(|(site, _)| {
                        site.machine == machine.id && site.block == block.id && site.edge == *edge
                    })
                    .peekable();
                let present = paths.peek().is_some();
                if !present
                    || !paths.all(|(site, site_certificates)| {
                        site_certificates.guard(predicate).iter().any(|certificate| {
                            entry_requirements::check_supplied_certificate(
                                &context,
                                guard.proposition(),
                                &machine.contract.requires,
                                &site.semantic_axioms,
                                certificate,
                            )
                        })
                        // An infeasible CFG path cannot reach this terminator.
                        // Its contradiction must itself have a kernel-checked
                        // certificate; missing proof supply never removes a path.
                        || site_certificates.infeasible().iter().any(|certificate| {
                            entry_requirements::check_supplied_certificate(
                                &context,
                                &Proposition::Falsehood,
                                &machine.contract.requires,
                                &site.semantic_axioms,
                                certificate,
                            )
                        })
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
