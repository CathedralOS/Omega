//! Target-scoped machine selection (fs portable-contract settle 2026-07-18):
//! `<target> machine Path(..) {..}` declares a PER-TARGET implementation of a
//! portable contract signature, gated by the target filter. This stage runs BEFORE symbol resolution
//! in both engines' pipelines (the differential contract: the interpreter
//! sees the SAME selected program natives are built from):
//!
//! - the SELECTED target's machine has its marker cleared -- from resolution
//!   onward it is an ordinary machine;
//! - every other target's machine keeps its marker and resolution lowers it
//!   as a sibling (symbol `<path>::<target>`): resolved, typed, and checked so a
//!   compile on one host reports every target's errors, never selected by
//!   name, conformance, or attachment lookups, and pruned once checking has
//!   committed. This pre-resolution selection is transitional; realization-
//!   time selection belongs to Omega (board item
//!   PROVIDER-SELECTION-AFTER-TERMINAL).
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
//!
//! A selected `Owner::provider_defaults` declaration is the exception: it is
//! provider-settlement input whose `select_provider` calls only the typed
//! admission below can grant, so it keeps its marker through build-time
//! evaluation and enters the program when the coordinator releases it.
//!
//! Selection is per dependency scope: product-scope sources select against
//! the product target, and build-scope sources (the build entry and the
//! root-local helpers it imports) select against the admitted build execution
//! profile, so a build helper never carries the product target's row onto the
//! host that executes it.

use diagnostics::Diagnostic;
use std::collections::{BTreeMap, HashSet};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use target::NativeTarget;
use tokens_to_syntax_trees::syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::syntax_trees::item::Item;

/// Exact target-scoped declarations retained across source filtering and
/// typed-tree construction.
///
/// Typed machines intentionally lose their target marker after filtering, so
/// the selected provider-default declarations must be retained before that
/// mutation. This carrier owns their deterministic full-name roster and
/// consumes it exactly once when rebinding the corresponding typed machines.
/// Each retained name carries its declaring source so two checked instances
/// of one path rebind to their own typed machine rather than colliding.
#[derive(Debug)]
pub struct SelectedTargetMachineDeclarations {
    provider_default_machine_names: Vec<(String, source::SourceId)>,
    selected_machine_origins: Vec<(String, String, source::SourceId)>,
    all_machine_origins: Vec<TargetMachineOrigin>,
}

/// One target-scoped declaration: its full name, authored target, declaring
/// source instance, and whether that target is the selected one for the
/// declaration's dependency scope. Two checked instances of one file produce
/// separate rows that never share a validation group.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetMachineOrigin {
    full_name: String,
    target: String,
    /// The checked instance (dependency scope) of the declaring source.
    scope: source::DependencyScope,
    source: source::SourceId,
    selected: bool,
}

/// The selected target per dependency scope.
struct ScopeTargets<'a> {
    product: NativeTarget,
    execution: NativeTarget,
    build_scope_sources: &'a HashSet<source::SourceId>,
}

impl ScopeTargets<'_> {
    /// Which dependency scope a machine declaration belongs to.
    fn scope_of(
        &self,
        machine: &tokens_to_syntax_trees::syntax_trees::item::Machine,
    ) -> source::DependencyScope {
        if self
            .build_scope_sources
            .contains(&machine.name.source_span().source_id)
        {
            source::DependencyScope::Build
        } else {
            source::DependencyScope::Product
        }
    }

    fn selects(
        &self,
        machine: &tokens_to_syntax_trees::syntax_trees::item::Machine,
        target: &str,
    ) -> bool {
        let scope_target = match self.scope_of(machine) {
            source::DependencyScope::Build => self.execution,
            source::DependencyScope::Product => self.product,
        };
        NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == scope_target)
    }
}

pub struct SettledTargetMachineDeclarations {
    pub provider_defaults: Vec<crate::provider_planning::ProviderSelection>,
    pub origins: Vec<crate::provider_planning::SelectedTargetMachineOrigin>,
}

impl SelectedTargetMachineDeclarations {
    fn new(
        mut provider_default_machine_names: Vec<(String, source::SourceId)>,
        mut selected_machine_origins: Vec<(String, String, source::SourceId)>,
        mut all_machine_origins: Vec<TargetMachineOrigin>,
    ) -> Self {
        provider_default_machine_names.sort_by(|(name, source), (other, other_source)| {
            name.cmp(other).then(source.0.cmp(&other_source.0))
        });
        selected_machine_origins.sort_by(
            |(name, target, source), (other_name, other_target, other_source)| {
                name.cmp(other_name)
                    .then(target.cmp(other_target))
                    .then(source.0.cmp(&other_source.0))
            },
        );
        all_machine_origins.sort_by(|left, right| {
            left.scope
                .cmp(&right.scope)
                .then_with(|| left.full_name.cmp(&right.full_name))
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.source.0.cmp(&right.source.0))
        });
        Self {
            provider_default_machine_names,
            selected_machine_origins,
            all_machine_origins,
        }
    }

    /// Filter one generated-source extension against the exact complete
    /// target-declaration roster retained before the authored frontend was
    /// admitted. The extension is selected in place, while this carrier is
    /// consumed into the combined base-plus-extension custody.
    pub fn filter_generated_extension(
        mut self,
        syntax: &mut SyntaxTrees,
        target_name: Option<&str>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Generated source is product source: it selects against the
        // product target only.
        let selected = NativeTarget::from_omega_target_name(target_name)
            .map_err(|diagnostic| vec![diagnostic])?;
        let no_build_scope = HashSet::new();
        let scopes = ScopeTargets {
            product: selected,
            execution: selected,
            build_scope_sources: &no_build_scope,
        };
        let extension_origins = target_machine_origins(syntax, &scopes);
        let mut complete_origins = self.all_machine_origins.clone();
        complete_origins.extend(extension_origins.iter().cloned());
        validate_target_machine_origins(&complete_origins)?;

        let extension = select_target_machines(syntax, &scopes, extension_origins);
        // Generated units were evaluated before they reached this filter.
        extension.release_provider_default_declarations(syntax);
        self.provider_default_machine_names
            .extend(extension.provider_default_machine_names);
        self.selected_machine_origins
            .extend(extension.selected_machine_origins);
        self.all_machine_origins
            .extend(extension.all_machine_origins);
        self.provider_default_machine_names
            .sort_by(|(name, source), (other, other_source)| {
                name.cmp(other).then(source.0.cmp(&other_source.0))
            });
        self.selected_machine_origins.sort_by(
            |(name, target, source), (other_name, other_target, other_source)| {
                name.cmp(other_name)
                    .then(target.cmp(other_target))
                    .then(source.0.cmp(&other_source.0))
            },
        );
        self.all_machine_origins.sort_by(|left, right| {
            left.scope
                .cmp(&right.scope)
                .then_with(|| left.full_name.cmp(&right.full_name))
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.source.0.cmp(&right.source.0))
        });
        Ok(self)
    }

    /// Clear the marker of every selected provider-default declaration so it
    /// resolves as an ordinary machine. Selection leaves them inert because
    /// build-time evaluation checks its probes without the declaration-call
    /// admission they need; release them before symbol resolution.
    pub fn release_provider_default_declarations(&self, syntax: &mut SyntaxTrees) {
        for handle in syntax.root_item_handles().to_vec() {
            let Item::Machine(machine) = syntax.root_item(handle) else {
                continue;
            };
            let Some(target) = machine.target.as_ref() else {
                continue;
            };
            let name = machine.name.as_str();
            let source = machine.name.source_span().source_id;
            let selected = self.selected_machine_origins.iter().any(
                |(selected, selected_target, selected_source)| {
                    selected == name
                        && selected_target == target.as_str()
                        && *selected_source == source
                },
            );
            let provider_default = self
                .provider_default_machine_names
                .iter()
                .any(|(default, default_source)| default == name && *default_source == source);
            if !(selected && provider_default) {
                continue;
            }
            let mut machine = machine.clone();
            machine.target = None;
            syntax.items.replace_item(handle, Item::Machine(machine));
        }
    }

    /// Admit declaration-call custody before preliminary package checking.
    /// Only this retained selected-target roster may grant the declaration
    /// intrinsic; it grants no authority to execute a Build mutation.
    /// Admit the selected target's `provider_defaults` bodies: typing has
    /// already finalized their `select_provider` calls as the build
    /// provider-selection intrinsic, so this harvest only validates what the
    /// realized target's declaration selects.
    pub fn admit_provider_default_calls(&self, typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        for (machine_name, source) in &self.provider_default_machine_names {
            let matching = typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.target.is_none()
                        && machine.name.as_str() == machine_name
                        && typed
                            .symbols
                            .symbol_provenance_source_span(machine.symbol)
                            .is_some_and(|span| span.source_id == *source)
                })
                .collect::<Vec<_>>();
            let [machine] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected target provider-default machine `{machine_name}` resolves to {} typed declarations",
                    matching.len(),
                ))]);
            };
            crate::build_evaluation::harvest_provider_selections(typed, machine)?;
        }
        Ok(())
    }

    pub fn select_product_target(
        &mut self,
        typed: &mut TypedTrees,
        target: NativeTarget,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut families: BTreeMap<&str, Vec<&TargetMachineOrigin>> = BTreeMap::new();
        for origin in &self.all_machine_origins {
            if origin.scope == source::DependencyScope::Product {
                families
                    .entry(origin.full_name.as_str())
                    .or_default()
                    .push(origin);
            }
        }
        let machine_index = |typed: &TypedTrees, name: &str, sibling: Option<&str>, source| {
            typed.machines().iter().position(|machine| {
                machine.name.as_str() == name
                    && machine.target.as_ref().map(|target| target.as_str()) == sibling
                    && typed
                        .symbols
                        .symbol_provenance_source_span(machine.symbol)
                        .is_some_and(|span| span.source_id == source)
            })
        };
        let mut diagnostics = Vec::new();
        let mut replacements = Vec::new();
        let mut swaps = Vec::new();
        for (name, origins) in &families {
            let Some(canonical) = origins.iter().find(|origin| origin.selected) else {
                continue;
            };
            if NativeTarget::from_omega_target_name(Some(canonical.target.as_str()))
                .is_ok_and(|resolved| resolved == target)
            {
                continue;
            }
            let wanted = origins.iter().find(|origin| {
                NativeTarget::from_omega_target_name(Some(origin.target.as_str()))
                    .is_ok_and(|resolved| resolved == target)
            });
            let Some(canonical_index) = machine_index(typed, name, None, canonical.source) else {
                // Provider defaults and other declarations that never reach
                // typing keep no typed body to swap.
                continue;
            };
            let Some(wanted) = wanted else {
                let mut providers = origins
                    .iter()
                    .map(|origin| origin.target.as_str())
                    .collect::<Vec<_>>();
                providers.sort();
                providers.dedup();
                if providers.len() >= 2 {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{name}` has no implementation for the selected target -- \
                         target-scoped implementations exist for: {} (add this target's \
                         `<target> machine {name}(..)` in that target package)",
                        providers.join(", "),
                    )));
                }
                swaps.push((canonical_index, None, (*canonical).clone(), None));
                continue;
            };
            let Some(wanted_index) =
                machine_index(typed, name, Some(wanted.target.as_str()), wanted.source)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}` body of machine `{name}` did not survive typing",
                    wanted.target,
                )));
                continue;
            };
            let canonical_machine = &typed.machines()[canonical_index];
            let wanted_machine = &typed.machines()[wanted_index];
            let canonical_identity = typed
                .normalized_machine_overload_identity(canonical_machine)
                .map(|identity| identity.identity().to_owned());
            let wanted_identity = typed
                .normalized_machine_overload_identity(wanted_machine)
                .map(|identity| identity.identity().to_owned());
            if canonical_identity != wanted_identity {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{name}` has a different signature for `{}` than for `{}` -- \
                     every target's body of one machine shares its signature",
                    wanted.target, canonical.target,
                )));
                continue;
            }
            let canonical_states = typed.machine_states(canonical_machine);
            let wanted_states = typed.machine_states(wanted_machine);
            if let (Some(from), Some(to)) = (canonical_states.first(), wanted_states.first()) {
                replacements.push((from.symbol, to.symbol));
            }
            for from in canonical_states.iter().skip(1) {
                if let Some(to) = wanted_states.iter().find(|state| state.name == from.name) {
                    replacements.push((from.symbol, to.symbol));
                }
            }
            swaps.push((
                canonical_index,
                Some(wanted_index),
                (*canonical).clone(),
                Some((*wanted).clone()),
            ));
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for (canonical_index, wanted_index, canonical, wanted) in swaps {
            let canonical_target =
                symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(
                    canonical.target.clone(),
                );
            let machines = typed.machines_mut();
            machines[canonical_index].target = Some(canonical_target);
            let canonical_symbol = machines[canonical_index].symbol;
            let demoted_spelling = machines[canonical_index].symbol_spelling().into_owned();
            let promoted = wanted_index.map(|wanted_index| {
                machines[wanted_index].target = None;
                (
                    machines[wanted_index].symbol,
                    machines[wanted_index].symbol_spelling().into_owned(),
                )
            });
            // Validation joins each machine to its symbol by spelling, so the
            // symbol table follows the markers.
            typed.symbols.rename(canonical_symbol, &demoted_spelling);
            if let Some((symbol, spelling)) = promoted {
                typed.symbols.rename(symbol, &spelling);
            }
            self.reselect(&canonical, wanted.as_ref());
        }
        typed.statement_table.retarget_call_states(&replacements);
        typed.expression_table.retarget_call_states(&replacements);
        reject_inert_sibling_callers(typed, target)
    }

    /// Move the retained selection of one family from its canonical body to
    /// the realized target's body, or drop it when that target has none.
    fn reselect(&mut self, canonical: &TargetMachineOrigin, wanted: Option<&TargetMachineOrigin>) {
        for origin in &mut self.all_machine_origins {
            if origin.scope != source::DependencyScope::Product
                || origin.full_name != canonical.full_name
            {
                continue;
            }
            origin.selected = wanted.is_some_and(|wanted| {
                origin.target == wanted.target && origin.source == wanted.source
            });
        }
        self.selected_machine_origins
            .retain(|(name, target, source)| {
                !(name == &canonical.full_name
                    && target == &canonical.target
                    && *source == canonical.source)
            });
        let was_provider_default = self
            .provider_default_machine_names
            .iter()
            .any(|(name, source)| name == &canonical.full_name && *source == canonical.source);
        self.provider_default_machine_names
            .retain(|(name, source)| {
                !(name == &canonical.full_name && *source == canonical.source)
            });
        if let Some(wanted) = wanted {
            self.selected_machine_origins.push((
                wanted.full_name.clone(),
                wanted.target.clone(),
                wanted.source,
            ));
            if was_provider_default {
                self.provider_default_machine_names
                    .push((wanted.full_name.clone(), wanted.source));
            }
        }
    }

    pub fn settle_provider_defaults(
        self,
        typed: &mut TypedTrees,
    ) -> Result<SettledTargetMachineDeclarations, Vec<Diagnostic>> {
        let mut defaults = Vec::new();
        let mut origins = Vec::new();
        let mut diagnostics = Vec::new();
        for (machine_name, source) in &self.provider_default_machine_names {
            let Some(machine) = typed.machines().iter().find(|machine| {
                machine.target.is_none()
                    && machine.name.as_str() == machine_name
                    && typed
                        .symbols
                        .symbol_provenance_source_span(machine.symbol)
                        .is_some_and(|span| span.source_id == *source)
            }) else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target provider-default machine `{machine_name}` did not survive lowering"
                )));
                continue;
            };
            match crate::build_evaluation::harvest_provider_selections(typed, machine) {
                Ok(mut machine_defaults) => {
                    defaults.append(&mut machine_defaults);
                }
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        for (machine_name, target, source) in &self.selected_machine_origins {
            let matches = typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.target.is_none()
                        && machine.name.as_str() == machine_name
                        && typed
                            .symbols
                            .symbol_provenance_source_span(machine.symbol)
                            .is_some_and(|span| span.source_id == *source)
                })
                .collect::<Vec<_>>();
            let [machine] = matches.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target machine `{machine_name}` for `{target}` resolves to {} typed declarations",
                    matches.len(),
                )));
                continue;
            };
            origins.push(crate::provider_planning::SelectedTargetMachineOrigin {
                machine: machine.symbol,
                target: target.clone(),
            });
        }
        if diagnostics.is_empty() {
            self.admit_provider_default_calls(typed)?;
            Ok(SettledTargetMachineDeclarations {
                provider_defaults: defaults,
                origins,
            })
        } else {
            Err(diagnostics)
        }
    }
}

/// Only the retained selected-target producer roster admits these declarations.
/// The intrinsic records an admitted provider declaration, not an executed
/// root Build mutation; evaluation must still prove the activation's receiver.
pub fn filter_target_machines(
    syntax: &mut SyntaxTrees,
    target_name: Option<&str>,
) -> Result<SelectedTargetMachineDeclarations, Vec<Diagnostic>> {
    filter_target_machines_by_scope(syntax, target_name, target_name, &HashSet::new())
}

/// Select target-scoped declarations per dependency scope: sources in
/// `build_scope_sources` select against `execution_profile_name` (the
/// admitted build execution profile), every other source against
/// `product_target_name`. `None` for either names the compiler host.
pub fn filter_target_machines_by_scope(
    syntax: &mut SyntaxTrees,
    product_target_name: Option<&str>,
    execution_profile_name: Option<&str>,
    build_scope_sources: &HashSet<source::SourceId>,
) -> Result<SelectedTargetMachineDeclarations, Vec<Diagnostic>> {
    let product = NativeTarget::from_omega_target_name(product_target_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    let execution = NativeTarget::from_omega_target_name(execution_profile_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    let scopes = ScopeTargets {
        product,
        execution,
        build_scope_sources,
    };
    let origins = target_machine_origins(syntax, &scopes);
    validate_target_machine_origins(&origins)?;
    Ok(select_target_machines(syntax, &scopes, origins))
}

fn target_machine_origins(
    syntax: &SyntaxTrees,
    scopes: &ScopeTargets<'_>,
) -> Vec<TargetMachineOrigin> {
    let mut origins = Vec::new();
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
        origins.push(TargetMachineOrigin {
            selected: scopes.selects(machine, target.as_str()),
            scope: scopes.scope_of(machine),
            source: machine.name.source_span().source_id,
            full_name,
            target: target.as_str().to_owned(),
        });
    }
    origins
}

fn validate_target_machine_origins(origins: &[TargetMachineOrigin]) -> Result<(), Vec<Diagnostic>> {
    validate_selected_target_machine_origins(origins)?;
    // A sibling target supplies exactly one body as well: two would lower to
    // one `<path>::<target>` name and collide.
    let mut per_target: BTreeMap<(source::DependencyScope, &str, &str), usize> = BTreeMap::new();
    for origin in origins.iter().filter(|origin| !origin.selected) {
        *per_target
            .entry((
                origin.scope,
                origin.full_name.as_str(),
                origin.target.as_str(),
            ))
            .or_default() += 1;
    }
    for ((_, full_name, target), count) in per_target {
        if count > 1 {
            return Err(vec![Diagnostic::error(format!(
                "machine `{full_name}` is implemented twice for `{target}` -- \
                 a target supplies exactly one implementation of a contract machine",
            ))]);
        }
    }
    Ok(())
}

fn validate_selected_target_machine_origins(
    origins: &[TargetMachineOrigin],
) -> Result<(), Vec<Diagnostic>> {
    // (dependency scope, full machine name) -> (selected count, non-selected
    // target names). The zero-or-two rule applies within one checked
    // instance: two instances of a dual-purpose file legitimately select the
    // same-named row once each. BTreeMap keeps diagnostics deterministic
    // across runs and generated units.
    let mut rows: BTreeMap<(source::DependencyScope, &str), (usize, Vec<&str>)> = BTreeMap::new();
    for origin in origins {
        let entry = rows
            .entry((origin.scope, origin.full_name.as_str()))
            .or_default();
        if origin.selected {
            entry.0 += 1;
        } else {
            entry.1.push(origin.target.as_str());
        }
    }
    for ((_, full_name), (selected_count, other_targets)) in rows {
        if selected_count > 1 {
            return Err(vec![Diagnostic::error(format!(
                "machine `{full_name}` is implemented twice for the selected target -- \
                 a target supplies exactly one implementation of a contract machine",
            ))]);
        }
        if selected_count == 0 {
            let mut providers = other_targets;
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
    Ok(())
}

fn select_target_machines(
    syntax: &mut SyntaxTrees,
    scopes: &ScopeTargets<'_>,
    all_machine_origins: Vec<TargetMachineOrigin>,
) -> SelectedTargetMachineDeclarations {
    // PRV4c: a target package may contribute ordinary provider defaults with
    // a target-scoped, package-owned `Owner::provider_defaults` machine. Keep
    // the selected declarations' full names before erasing the target marker;
    // typed machines intentionally carry no deployment marker after this pass.
    let mut provider_default_machines = Vec::new();
    let mut selected_machine_origins = Vec::new();
    for handle in syntax.root_item_handles().to_vec() {
        let Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        let Some(target) = machine.target.as_ref() else {
            continue;
        };
        if !scopes.selects(machine, target.as_str()) {
            continue;
        }
        let full_name = machine.name.as_str().to_owned();
        let declaring_source = machine.name.source_span().source_id;
        let provider_default = full_name.ends_with("::provider_defaults");
        if provider_default {
            provider_default_machines.push((full_name.clone(), declaring_source));
        }
        selected_machine_origins.push((full_name, target.as_str().to_owned(), declaring_source));
        if provider_default {
            continue;
        }

        // Typed machines intentionally carry no target marker after this
        // selection point.
        let mut machine = machine.clone();
        machine.target = None;
        syntax.items.replace_item(handle, Item::Machine(machine));
    }

    SelectedTargetMachineDeclarations::new(
        provider_default_machines,
        selected_machine_origins,
        all_machine_origins,
    )
}

fn reject_inert_sibling_callers(
    typed: &TypedTrees,
    target: NativeTarget,
) -> Result<(), Vec<Diagnostic>> {
    // Symbol handles are arena handles, not ordered keys; the sibling set is
    // small and scanned linearly like the rest of this pass.
    // A call names the callee's entry state, not its machine, so the index
    // carries each inert machine's states beside its own symbol.
    let mut inert: Vec<(symbols::SymbolHandle, &str, &str)> = Vec::new();
    for machine in typed.machines() {
        let Some(machine_target) = machine.target.as_ref().map(|target| target.as_str()) else {
            continue;
        };
        // `NativeTarget` carries no canonical name and several spellings can
        // share one profile, so the comparison is structural -- the same
        // conversion the rest of this pass uses.
        if NativeTarget::from_omega_target_name(Some(machine_target))
            .is_ok_and(|declared| declared == target)
        {
            continue;
        }
        inert.push((machine.symbol, machine.name.as_str(), machine_target));
        for state in typed.machine_states(machine) {
            inert.push((state.symbol, machine.name.as_str(), machine_target));
        }
    }
    if inert.is_empty() {
        return Ok(());
    }
    // `NativeTarget` is a profile, not a name, and more than one spelling can
    // share it; name every spelling this build realizes rather than guess one.
    // The conversion is the fallible one: `alpha_bootstrap` has no native
    // target at all and panics if asked for one directly.
    let realized = target::TargetProfile::ALL
        .into_iter()
        .filter(|profile| {
            // `cross_platform_cli` and `local_unchecked` resolve to whatever
            // the host is rather than naming a target of their own, so they
            // would list the host profile twice more under other names.
            !matches!(
                profile,
                target::TargetProfile::CrossPlatformCli | target::TargetProfile::LocalUnchecked
            ) && NativeTarget::from_omega_target_name(Some(profile.target_name()))
                .is_ok_and(|profile| profile == target)
        })
        .map(|profile| format!("`{}`", profile.target_name()))
        .collect::<Vec<_>>()
        .join(" or ");
    let mut diagnostics = Vec::new();
    let mut called = Vec::new();
    let mut visited = Vec::new();
    for machine in typed.machines() {
        let caller_target = machine.target.as_ref().map(|target| target.as_str());
        for state in typed.machine_states(machine) {
            called.clear();
            visited.clear();
            for statement in typed.statement_table.statements(state.statement_nodes) {
                collect_statement_callees(typed, statement, &mut visited, &mut called);
            }
            for called_symbol in &called {
                let Some((_, callee, callee_target)) =
                    inert.iter().find(|(symbol, _, _)| symbol == called_symbol)
                else {
                    continue;
                };
                if caller_target == Some(*callee_target) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` calls `{callee}`, which is declared only for \
                     target `{callee_target}`. This build realizes {realized}, so that body \
                     is an inert sibling and the call has no implementation; scope the \
                     caller to `{callee_target}` as well, or declare `{callee}` for the \
                     target being built.",
                    machine.name.as_str(),
                    state.name.as_str(),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Record one callee once. A state that names the same inert sibling twice
/// has one defect, and the diagnostic carries no span to tell the two sites
/// apart, so a second copy would be unreadable noise.
fn note_callee(called: &mut Vec<symbols::SymbolHandle>, symbol: symbols::SymbolHandle) {
    if symbol.is_valid() && !called.contains(&symbol) {
        called.push(symbol);
    }
}

/// Every callee one statement can name. The statement-position call is only
/// one spelling: a value call in an initializer, an argument, a guard, or a
/// transition argument reaches the same callee, and a walk that matched only
/// `StatementNode::Call` left those call sites to vanish with their filtered
/// callee and bind the ZII zero.
fn collect_statement_callees(
    typed: &TypedTrees,
    statement: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode,
    visited: &mut Vec<
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    >,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
    match statement {
        StatementNode::Call(call) => {
            note_callee(called, call.target_symbol);
            for argument in typed.expression_table.expression_handles(call.arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_callees(typed, assignment.target, visited, called);
            collect_expression_callees(typed, assignment.value, visited, called);
        }
        StatementNode::Expression(expression) => {
            collect_expression_callees(typed, *expression, visited, called);
        }
        StatementNode::LocalData(local) => {
            collect_expression_callees(typed, local.initial_value, visited, called);
        }
        StatementNode::AssemblyFact(fact) => {
            collect_expression_callees(typed, fact.expression, visited, called);
        }
        StatementNode::Transition(transition) => {
            if let symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
                collect_expression_callees(typed, *guard, visited, called);
            }
            for target in [transition.target, transition.continuation] {
                collect_transition_target_callees(typed, target, visited, called);
            }
        }
        StatementNode::RootBinding(_) => {}
    }
}

/// A named transition target is itself a call to a state, so its own symbol
/// is a callee. Its argument span is walked for the same reason the other
/// spans are, though typed trees present it empty for the tail-call
/// spelling, so no fixture here exercises that arm.
fn collect_transition_target_callees(
    typed: &TypedTrees,
    target: symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetHandle,
    visited: &mut Vec<
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    >,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode;
    if !typed.statement_table.transition_target_is_valid(target) {
        return;
    }
    match typed.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            note_callee(called, path.symbol);
            for argument in typed.expression_table.expression_handles(*arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        TransitionTargetNode::Value(expression) => {
            collect_expression_callees(typed, *expression, visited, called);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

/// Every callee inside one expression graph. `visited` is the graph's own
/// shared-subexpression guard: the table is a DAG, not a tree.
fn collect_expression_callees(
    typed: &TypedTrees,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    >,
    called: &mut Vec<symbols::SymbolHandle>,
) {
    use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
        ExpressionNode, MatchPattern,
    };
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    match typed.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            note_callee(called, call.target_symbol);
            collect_expression_callees(typed, call.receiver, visited, called);
            for argument in typed.expression_table.expression_handles(call.arguments) {
                collect_expression_callees(typed, *argument, visited, called);
            }
        }
        ExpressionNode::Match(dispatch) => {
            collect_expression_callees(typed, dispatch.subject, visited, called);
            for arm in typed.expression_table.match_arms(dispatch.arms) {
                if let MatchPattern::Value(value) = arm.pattern {
                    collect_expression_callees(typed, value, visited, called);
                }
                collect_expression_callees(typed, arm.value, visited, called);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in typed.expression_table.expression_handles(*values) {
                collect_expression_callees(typed, *value, visited, called);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_callees(typed, atomic.value, visited, called);
            collect_expression_callees(typed, atomic.result, visited, called);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_callees(typed, binary.left, visited, called);
            collect_expression_callees(typed, binary.right, visited, called);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_callees(typed, cast.value, visited, called);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_callees(typed, indexed.collection, visited, called);
            collect_expression_callees(typed, indexed.index, visited, called);
        }
        ExpressionNode::Member(member) => {
            collect_expression_callees(typed, member.receiver, visited, called);
        }
        ExpressionNode::Borrow(borrow) => {
            collect_expression_callees(typed, borrow.target, visited, called);
        }
        ExpressionNode::Range(range) => {
            collect_expression_callees(typed, range.start, visited, called);
            collect_expression_callees(typed, range.end, visited, called);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in typed.expression_table.struct_fields(literal.fields) {
                collect_expression_callees(typed, field.value, visited, called);
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_callees(typed, unary.operand, visited, called);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SelectedTargetMachineDeclarations, filter_target_machines, filter_target_machines_by_scope,
    };
    use std::collections::HashSet;

    fn syntax(source_id: usize, source: &str) -> tokens_to_syntax_trees::syntax_trees::SyntaxTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize target-machine fixture");
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(source_id), &tokens)
            .expect("parse target-machine fixture")
    }

    #[test]
    fn build_scope_sources_select_against_the_execution_profile_not_the_product_target() {
        const HELPER: &str =
            "macos_arm64 machine Tool::probe() {}\nwindows_x86_64 machine Tool::probe() {}\n";
        let build_scope = HashSet::from([source::SourceId(7)]);

        // A macOS-hosted build of a Linux product: the helper's macOS row is
        // selected because the helper is build scope.
        let mut helper = syntax(7, HELPER);
        let retained = filter_target_machines_by_scope(
            &mut helper,
            Some("linux_x86_64"),
            Some("macos_arm64"),
            &build_scope,
        )
        .expect("build-scope helper selects against the execution profile");
        assert_eq!(
            retained.selected_machine_origins,
            vec![(
                "Tool::probe".into(),
                "macos_arm64".into(),
                source::SourceId(7)
            )]
        );
        let selected_markers = helper
            .root_items()
            .filter_map(|item| match item {
                tokens_to_syntax_trees::syntax_trees::item::Item::Machine(machine) => Some(
                    machine
                        .target
                        .as_ref()
                        .map(|target| target.as_str().to_owned()),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(selected_markers, vec![None, Some("windows_x86_64".into())]);

        // The same declarations as product scope keep the product rule: a
        // contract name implemented only by foreign targets is the loud edge.
        let mut product = syntax(7, HELPER);
        let diagnostics = filter_target_machines_by_scope(
            &mut product,
            Some("linux_x86_64"),
            Some("macos_arm64"),
            &HashSet::new(),
        )
        .expect_err("product-scope declarations still select against the product target");
        assert!(
            diagnostics[0]
                .to_string()
                .contains("machine `Tool::probe` has no implementation for the selected target"),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn empty_target_declarations_settle_to_canonical_empty_defaults() {
        let settled = SelectedTargetMachineDeclarations::new(Vec::new(), Vec::new(), Vec::new())
            .settle_provider_defaults(
                &mut symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default(),
            )
            .expect("empty target declaration custody has no typed dependency");

        assert!(settled.provider_defaults.is_empty());
        assert!(settled.origins.is_empty());
    }

    #[test]
    fn only_selected_target_default_calls_receive_declaration_intrinsics() {
        let text = "boundary trait Reader { machine read(); } data Provider {} linux_x86_64 machine Provider::provider_defaults(defaults: &mut Provider) { defaults.select_provider<Reader, Provider>(); } machine Provider::ordinary(defaults: &mut Provider) { defaults.select_provider<Reader, Provider>(); }";
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add(std::path::PathBuf::from("provider.omg"), text.to_owned())
            .source_id;
        let mut syntax = syntax(source_id.0, text);
        let selected = filter_target_machines(&mut syntax, Some("linux_x86_64"))
            .expect("select exact target producer");
        let marker = |syntax: &tokens_to_syntax_trees::syntax_trees::SyntaxTrees| {
            syntax.root_items().find_map(|item| match item {
                tokens_to_syntax_trees::syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "Provider::provider_defaults" =>
                {
                    Some(
                        machine
                            .target
                            .as_ref()
                            .map(|target| target.as_str().to_owned()),
                    )
                }
                _ => None,
            })
        };
        // Build-time evaluation runs between selection and release: the
        // selected declaration is still inert there.
        assert_eq!(marker(&syntax), Some(Some("linux_x86_64".into())));
        selected.release_provider_default_declarations(&mut syntax);
        assert_eq!(marker(&syntax), Some(None));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve defaults");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type defaults");
        let settled = selected
            .settle_provider_defaults(&mut typed)
            .expect("settle defaults");
        assert_eq!(settled.provider_defaults.len(), 1);
        let call_targets = typed
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.kind() == symbol_resolved_trees_to_typed_trees::typed_trees::AuthoredDeclarationSelectionKind::Call
            })
            .map(|selection| selection.target())
            .collect::<Vec<_>>();
        assert_eq!(
            call_targets
                .iter()
                .filter(|target| **target
                    == symbol_resolved_trees_to_typed_trees::typed_trees::AuthoredDeclarationSelectionTarget::Intrinsic(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection
                    ))
                .count(),
            1
        );
        assert_eq!(
            call_targets
                .iter()
                .filter(|target| **target
                    == symbol_resolved_trees_to_typed_trees::typed_trees::AuthoredDeclarationSelectionTarget::LateBound(
                        symbol_resolved_trees_to_typed_trees::typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedCall
                    ))
                .count(),
            1
        );
    }

    #[test]
    fn missing_typed_provider_default_machines_report_sorted_full_names() {
        let declarations = SelectedTargetMachineDeclarations::new(
            vec![
                ("Zed::provider_defaults".into(), source::SourceId(0)),
                ("Alpha::provider_defaults".into(), source::SourceId(0)),
            ],
            Vec::new(),
            Vec::new(),
        );
        let Err(diagnostics) = declarations.settle_provider_defaults(
            &mut symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default(),
        ) else {
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

    #[test]
    fn generated_extension_retains_selected_origin_and_provider_default_custody() {
        let mut base = syntax(
            0,
            "linux_x86_64 machine Base::value() -> u64 { 1 }\nwindows_x86_64 machine Base::value() -> u64 { 2 }\n",
        );
        let retained = filter_target_machines(&mut base, Some("linux_x86_64"))
            .expect("base target cohort selects exactly");
        let mut extension = syntax(
            1,
            "linux_x86_64 machine Generated::value() -> u64 { 3 }\nwindows_x86_64 machine Generated::value() -> u64 { 4 }\nlinux_x86_64 machine Generated::provider_defaults() { }\nwindows_x86_64 machine Generated::provider_defaults() { }\n",
        );

        let retained = retained
            .filter_generated_extension(&mut extension, Some("linux_x86_64"))
            .expect("generated target cohort selects against retained base");

        assert_eq!(
            retained.provider_default_machine_names,
            vec![("Generated::provider_defaults".into(), source::SourceId(1))]
        );
        assert_eq!(
            retained.selected_machine_origins,
            vec![
                (
                    "Base::value".into(),
                    "linux_x86_64".into(),
                    source::SourceId(0)
                ),
                (
                    "Generated::provider_defaults".into(),
                    "linux_x86_64".into(),
                    source::SourceId(1)
                ),
                (
                    "Generated::value".into(),
                    "linux_x86_64".into(),
                    source::SourceId(1)
                ),
            ]
        );
        let generated_targets = extension
            .root_items()
            .filter_map(|item| {
                let tokens_to_syntax_trees::syntax_trees::item::Item::Machine(machine) = item
                else {
                    return None;
                };
                Some((
                    machine.name.as_str().to_owned(),
                    machine
                        .target
                        .as_ref()
                        .map(|target| target.as_str().to_owned()),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            generated_targets,
            vec![
                ("Generated::value".into(), None),
                ("Generated::value".into(), Some("windows_x86_64".into())),
                ("Generated::provider_defaults".into(), None),
                (
                    "Generated::provider_defaults".into(),
                    Some("windows_x86_64".into()),
                ),
            ]
        );
    }

    #[test]
    fn generated_extension_rejects_selected_duplicate_across_base_stratum() {
        let mut base = syntax(0, "linux_x86_64 machine Duplicate::value() -> u64 { 1 }\n");
        let retained = filter_target_machines(&mut base, Some("linux_x86_64"))
            .expect("base target row selects exactly");
        let mut extension = syntax(1, "linux_x86_64 machine Duplicate::value() -> u64 { 2 }\n");

        let diagnostics = retained
            .filter_generated_extension(&mut extension, Some("linux_x86_64"))
            .expect_err("base and generated selected rows must form one global cohort");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("implemented twice"));
        assert!(diagnostics[0].message.contains("Duplicate::value"));
    }

    #[test]
    fn generated_extension_completes_missing_target_validation_across_base_stratum() {
        let mut base = syntax(0, "windows_x86_64 machine Missing::value() -> u64 { 1 }\n");
        let retained = filter_target_machines(&mut base, Some("linux_x86_64"))
            .expect("one foreign-only base row remains an inert target-local helper");
        let mut extension = syntax(1, "macos_arm64 machine Missing::value() -> u64 { 2 }\n");

        let diagnostics = retained
            .filter_generated_extension(&mut extension, Some("linux_x86_64"))
            .expect_err("base and generated rows must form one portable target cohort");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("no implementation"));
        assert!(diagnostics[0].message.contains("macos_arm64"));
        assert!(diagnostics[0].message.contains("windows_x86_64"));
    }

    #[test]
    fn generated_units_reject_a_duplicate_selected_target_row() {
        let mut base = syntax(0, "const BASE: u64 = 1;\n");
        let retained = filter_target_machines(&mut base, Some("linux_x86_64"))
            .expect("base has no target rows");
        let mut extension = syntax(1, "linux_x86_64 machine Duplicate::value() -> u64 { 2 }\n");
        let second = syntax(2, "linux_x86_64 machine Duplicate::value() -> u64 { 3 }\n");
        extension.extend_from(&second);

        let diagnostics = retained
            .filter_generated_extension(&mut extension, Some("linux_x86_64"))
            .expect_err("generated units must not split a duplicate selected row");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("implemented twice"));
        assert!(diagnostics[0].message.contains("Duplicate::value"));
    }

    #[test]
    fn generated_units_reject_a_portable_cohort_missing_the_selected_target() {
        let mut base = syntax(0, "const BASE: u64 = 1;\n");
        let retained = filter_target_machines(&mut base, Some("linux_x86_64"))
            .expect("base has no target rows");
        let mut extension = syntax(1, "windows_x86_64 machine Missing::value() -> u64 { 2 }\n");
        let second = syntax(2, "macos_arm64 machine Missing::value() -> u64 { 3 }\n");
        extension.extend_from(&second);

        let diagnostics = retained
            .filter_generated_extension(&mut extension, Some("linux_x86_64"))
            .expect_err("generated units must expose a complete target cohort");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("no implementation"));
        assert!(diagnostics[0].message.contains("macos_arm64"));
        assert!(diagnostics[0].message.contains("windows_x86_64"));
    }
}
