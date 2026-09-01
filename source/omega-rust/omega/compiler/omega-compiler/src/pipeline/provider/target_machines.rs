//! Target-scoped machine selection (fs portable-contract settle 2026-07-18):
//! `<target> machine Path(..) {..}` declares a PER-TARGET implementation of a
//! portable contract signature, gated by the target filter. This stage runs BEFORE symbol resolution
//! in both engines' pipelines (the differential contract: the interpreter
//! sees the SAME selected program natives are built from):
//!
//! - the SELECTED target's machine has its marker cleared -- from resolution
//!   onward it is an ordinary machine, and no downstream stage grows a
//!   per-target concept;
//! - non-selected machines keep their marker and stay INERT (resolution skips
//!   them), so four targets' same-name implementations never collide.
//!
//! Loud edges (the settle's zero-or-two rule):
//! - two selected-target machines with one name = implemented twice;
//! - a name implemented ONLY by non-selected targets = the selected target is
//!   missing its implementation -- an unconditional error naming who does
//!   provide one (the contract is the cross-target name set; waiting for an
//!   unresolved call site would bury the real cause).
//!
//! An unknown target name on a machine is silently never-selected, matching
//! the target-scoped declaration semantic (a hypothetical target is inert
//! everywhere, which is also what makes fail-canaries host-portable).

use omega_target::NativeTarget;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::Item;
use psi_typed_trees::TypedTrees;
use std::collections::BTreeMap;

/// Exact target-scoped declarations retained across source filtering and
/// typed-tree construction.
///
/// Typed machines intentionally lose their target marker after filtering, so
/// the selected provider-default declarations must be retained before that
/// mutation. This carrier owns their deterministic full-name roster and
/// consumes it exactly once when rebinding the corresponding typed machines.
pub(crate) struct SelectedTargetMachineDeclarations {
    provider_default_machine_names: Vec<String>,
    selected_machine_origins: Vec<(String, String)>,
}

pub(crate) struct SettledTargetMachineDeclarations {
    pub(crate) provider_defaults: Vec<omega_provider_planning::ProviderSelection>,
    pub(crate) origins: Vec<omega_provider_planning::plans::SelectedTargetMachineOrigin>,
}

impl SelectedTargetMachineDeclarations {
    fn new(
        mut provider_default_machine_names: Vec<String>,
        mut selected_machine_origins: Vec<(String, String)>,
    ) -> Self {
        provider_default_machine_names.sort();
        selected_machine_origins.sort();
        Self {
            provider_default_machine_names,
            selected_machine_origins,
        }
    }

    /// Resolve the retained target-owned provider-default producers and
    /// preserve each producer's exact authored row order and identity.
    pub(crate) fn settle_provider_defaults(
        self,
        typed: &TypedTrees,
    ) -> Result<SettledTargetMachineDeclarations, Vec<Diagnostic>> {
        let mut defaults = Vec::new();
        let mut origins = Vec::new();
        let mut diagnostics = Vec::new();
        for machine_name in self.provider_default_machine_names {
            let Some(machine) = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == machine_name)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target provider-default machine `{machine_name}` did not survive lowering"
                )));
                continue;
            };
            match super::build_config::harvest_provider_selections(typed, machine) {
                Ok(mut machine_defaults) => defaults.append(&mut machine_defaults),
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        for (machine_name, target) in self.selected_machine_origins {
            let matches = typed
                .machines()
                .iter()
                .filter(|machine| machine.name.as_str() == machine_name)
                .collect::<Vec<_>>();
            let [machine] = matches.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target machine `{machine_name}` for `{target}` resolves to {} typed declarations",
                    matches.len(),
                )));
                continue;
            };
            origins.push(
                omega_provider_planning::plans::SelectedTargetMachineOrigin {
                    machine: machine.symbol,
                    target,
                },
            );
        }
        if diagnostics.is_empty() {
            Ok(SettledTargetMachineDeclarations {
                provider_defaults: defaults,
                origins,
            })
        } else {
            Err(diagnostics)
        }
    }
}

pub(crate) fn filter_target_machines(
    syntax: &mut SyntaxTrees,
    target_name: Option<&str>,
) -> Result<SelectedTargetMachineDeclarations, Vec<Diagnostic>> {
    let selected =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;

    // full machine name -> (selected handles, non-selected target names).
    // BTreeMap keeps diagnostics deterministic across runs.
    let mut rows: BTreeMap<String, (Vec<psi_syntax_trees::item::ItemHandle>, Vec<String>)> =
        BTreeMap::new();
    for handle in syntax.root_item_handles().to_vec() {
        let Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        let Some(target) = &machine.target else {
            continue;
        };
        // The parser's machine name is already the complete spelled path
        // (`Owner::provider_defaults` for an attached declaration). Rebuilding
        // it from `attached_data` would produce
        // `Owner::Owner::provider_defaults`, which still groups target rows but
        // cannot be resolved against the later typed machine.
        let full_name = machine.name.as_str().to_owned();
        let row_selected = NativeTarget::from_omega_target_name(Some(target.as_str()))
            .is_ok_and(|resolved| resolved == selected);
        let entry = rows.entry(full_name).or_default();
        if row_selected {
            entry.0.push(handle);
        } else {
            entry.1.push(target.as_str().to_owned());
        }
    }

    for (full_name, (selected_handles, other_targets)) in &rows {
        if selected_handles.len() > 1 {
            return Err(vec![Diagnostic::error(format!(
                "machine `{full_name}` is implemented twice for the selected target -- \
                 a target supplies exactly one implementation of a contract machine",
            ))]);
        }
        if selected_handles.is_empty() {
            let mut providers = other_targets.clone();
            providers.sort();
            providers.dedup();
            // A name implemented by ONE foreign target is that target's
            // paradigm INTERNAL (the windows dir-walk's find-enumeration
            // helpers exist on no posix target), not a portable-contract
            // surface -- filter it silently with its callers. The loud edge
            // is for CONTRACT names: two or more targets implementing a name
            // is the evidence a selected target is missing its row.
            if providers.len() < 2 {
                continue;
            }
            return Err(vec![Diagnostic::error(format!(
                "machine `{full_name}` has no implementation for the selected target -- \
                 target-scoped implementations exist for: {} (add this target's \
                 `<target> machine {full_name}(..)` in that target package)",
                providers.join(", "),
            ))]);
        }
    }

    // PRV4c: a target package may contribute ordinary provider defaults with
    // a target-scoped, package-owned `Owner::provider_defaults` machine. Keep
    // the selected declarations' full names before erasing the target marker;
    // typed machines intentionally carry no deployment marker after this pass.
    let mut provider_default_machines = Vec::new();
    let mut selected_machine_origins = Vec::new();
    for (full_name, (selected_handles, _)) in &rows {
        if !selected_handles.is_empty() && full_name.ends_with("::provider_defaults") {
            provider_default_machines.push(full_name.clone());
        }
        for handle in selected_handles {
            let Item::Machine(machine) = syntax.root_item(*handle) else {
                continue;
            };
            let Some(target) = machine.target.as_ref() else {
                continue;
            };
            selected_machine_origins.push((full_name.clone(), target.as_str().to_owned()));
        }
    }
    // Clear the selected machines' markers LAST, after the loud edges passed:
    // from here on they are ordinary machines.
    for (_, (selected_handles, _)) in rows {
        for handle in selected_handles {
            let Item::Machine(machine) = syntax.root_item(handle) else {
                continue;
            };
            let mut machine = machine.clone();
            machine.target = None;
            syntax.items.replace_item(handle, Item::Machine(machine));
        }
    }

    Ok(SelectedTargetMachineDeclarations::new(
        provider_default_machines,
        selected_machine_origins,
    ))
}

#[cfg(test)]
mod tests {
    use super::SelectedTargetMachineDeclarations;

    #[test]
    fn empty_target_declarations_settle_to_canonical_empty_defaults() {
        let settled = SelectedTargetMachineDeclarations::new(Vec::new(), Vec::new())
            .settle_provider_defaults(&psi_typed_trees::TypedTrees::default())
            .expect("empty target declaration custody has no typed dependency");

        assert!(settled.provider_defaults.is_empty());
        assert!(settled.origins.is_empty());
    }

    #[test]
    fn missing_typed_provider_default_machines_report_sorted_full_names() {
        let declarations = SelectedTargetMachineDeclarations::new(
            vec![
                "Zed::provider_defaults".into(),
                "Alpha::provider_defaults".into(),
            ],
            Vec::new(),
        );
        let Err(diagnostics) =
            declarations.settle_provider_defaults(&psi_typed_trees::TypedTrees::default())
        else {
            panic!("retained target declarations must rebind exactly after typing")
        };

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[0].to_string(),
            "error: selected target provider-default machine `Alpha::provider_defaults` did not survive lowering"
        );
        assert_eq!(
            diagnostics[1].to_string(),
            "error: selected target provider-default machine `Zed::provider_defaults` did not survive lowering"
        );
    }
}
