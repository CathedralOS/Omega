//! Prepare target families before resolution, then select a product body later.
//!
//! Product source uses the first recognized target in name order as each
//! family's canonical body. Other bodies retain their markers and resolve as
//! siblings. Authored and generated product source therefore prepare the same
//! frontend regardless of the requested native product. Build-scope source is
//! different: its helpers must run against the admitted build execution target.
//!
//! Provider-default declarations retain their markers until their calls can be
//! admitted as provider-selection inputs. Unknown authored target names remain
//! inert; duplicate family members reject across authored and generated units.
//!
//! `select_product_target` still swaps typed bodies before checking. Moving
//! that selection behind Terminal Psi remains separate work; canonical source
//! preparation alone does not establish the final pipeline boundary.

use crate::SettledTargetMachineDeclarations;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, ItemHandle};
use target::{NativeTarget, TargetProfile};
use typed_trees::TypedTrees;
use typed_trees::typed_trees::inspection::callees::ExplicitStateCallees;

/// Exact target-scoped declarations retained across source filtering and
/// typed-tree construction.
///
/// Typed machines intentionally lose their target marker after filtering, so
/// the selected provider-default declarations must be retained before that
/// mutation. This carrier owns their deterministic full-name roster and
/// rebinds each dependency purpose against its corresponding typed machines.
/// Each retained name carries its declaring source so two checked instances
/// of one path rebind to their own typed machine rather than colliding.
#[derive(Debug, Clone, Default)]
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
    namespace: TargetNamespace,
    full_name: String,
    target: String,
    /// The checked instance (dependency scope) of the declaring source.
    scope: source::DependencyScope,
    source: source::SourceId,
    selected: bool,
}

/// Namespace identity is needed only before resolution supplies exact handles.
/// A reconciled package identity supersedes its physical source root.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct TargetNamespace {
    package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    package_root: std::path::PathBuf,
    module: Vec<String>,
}

/// Sources executed by the build, paired with their admitted execution target.
/// Product-only source preparation has no such scope and needs no target.
pub struct BuildTargetScope<'a> {
    pub execution: TargetProfile,
    pub sources: &'a HashSet<source::SourceId>,
}

impl TargetMachineOrigin {
    fn same_family(&self, other: &Self) -> bool {
        self.scope == other.scope
            && self.namespace == other.namespace
            && self.full_name == other.full_name
    }
}

fn sort_origins(origins: &mut [TargetMachineOrigin]) {
    origins.sort_by(compare_origins);
}

fn compare_origins(left: &TargetMachineOrigin, right: &TargetMachineOrigin) -> std::cmp::Ordering {
    left.scope
        .cmp(&right.scope)
        .then_with(|| left.namespace.cmp(&right.namespace))
        .then_with(|| left.full_name.cmp(&right.full_name))
        .then_with(|| left.target.cmp(&right.target))
        .then_with(|| left.source.0.cmp(&right.source.0))
}

impl SelectedTargetMachineDeclarations {
    fn new(
        mut provider_default_machine_names: Vec<(String, source::SourceId)>,
        mut selected_machine_origins: Vec<(String, String, source::SourceId)>,
        all_machine_origins: Vec<TargetMachineOrigin>,
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
        sources: Option<&source::SourceMap>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Generated source is product source: like the base, it selects each
        // family's canonical body, and the realized target's body replaces it
        // before checking. No build execution target enters this preparation.
        let (extension_origins, selected_declarations) =
            target_machine_origins(syntax, sources, None)?;
        let mut complete_origins = std::mem::take(&mut self.all_machine_origins);
        complete_origins.extend(extension_origins.iter().cloned());
        sort_origins(&mut complete_origins);
        validate_target_machine_origins(&complete_origins)?;

        let extension = select_target_machines(syntax, extension_origins, selected_declarations);
        // Generated units were evaluated before they reached this filter.
        extension.release_provider_default_declarations(syntax);
        self.provider_default_machine_names
            .extend(extension.provider_default_machine_names);
        self.selected_machine_origins
            .extend(extension.selected_machine_origins);
        self.all_machine_origins = complete_origins;
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
    pub fn admit_provider_default_calls(
        &self,
        context: crate::ProviderPlanningContext<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = context.typed;
        for (machine_name, source) in &self.provider_default_machine_names {
            if typed
                .symbols
                .source_files()
                .find(|file| file.source_id == *source)
                .is_some_and(|file| file.dependency_scope != context.scope)
            {
                continue;
            }
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
            crate::harvest_provider_selections(context, machine)?;
        }
        Ok(())
    }

    /// Replace each product-scope family's canonical body with `target`'s own
    /// body before checking: calls that resolved to the canonical body move to
    /// the target's sibling, which loses its sibling marker and so brings its
    /// own conformances into selection, while the canonical body becomes that
    /// target's sibling. A family with no body for `target` stays inert;
    /// only a surviving call demands an implementation. The number of foreign
    /// implementations does not change that obligation.
    pub fn select_product_target(
        &mut self,
        typed: &mut TypedTrees,
        target: TargetProfile,
    ) -> Result<(), Vec<Diagnostic>> {
        let machine_index = |symbol| {
            typed
                .machines()
                .iter()
                .position(|machine| machine.symbol == symbol)
        };
        let mut diagnostics = Vec::new();
        let mut replacements = Vec::new();
        let mut swaps = Vec::new();
        // Typing already owns family membership. Never rebuild a name-keyed
        // map here: equal names in different namespaces remain independent.
        for family in &typed.target_machine_families {
            let name = family.name.as_str();
            let Some(canonical) = family
                .members
                .iter()
                .find(|member| member.machine == family.canonical)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "target family `{name}` has no canonical member"
                ))]);
            };
            // A native shape is not deployment identity: Windows and UEFI
            // share x86-64/COFF but must never select each other's bodies.
            if TargetProfile::from_canonical_target_name(canonical.target.as_str())
                .is_ok_and(|resolved| resolved == target)
            {
                continue;
            }
            let Some(canonical_index) = machine_index(canonical.machine) else {
                return Err(vec![Diagnostic::error(format!(
                    "canonical body of target family `{name}` did not survive typing"
                ))]);
            };
            let canonical_origin = self.origin_index(typed, canonical)?;
            let wanted = family.members.iter().find(|member| {
                TargetProfile::from_canonical_target_name(member.target.as_str())
                    .is_ok_and(|resolved| resolved == target)
            });
            let Some(wanted) = wanted else {
                // Demote the canonical body before examining calls. Its mere
                // presence in a shared source set does not demand realization.
                swaps.push((canonical_index, None, canonical_origin, None));
                continue;
            };
            let Some(wanted_index) = machine_index(wanted.machine) else {
                diagnostics.push(Diagnostic::error(format!(
                    "target `{}` body of machine `{name}` did not survive typing",
                    wanted.target,
                )));
                continue;
            };
            let wanted_origin = self.origin_index(typed, wanted)?;
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
                canonical_origin,
                Some(wanted_origin),
            ));
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for (canonical_index, wanted_index, canonical_origin, wanted_origin) in swaps {
            let canonical_target = typed_trees::name::Identifier::generated(
                self.all_machine_origins[canonical_origin].target.clone(),
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
            self.reselect(canonical_origin, wanted_origin);
        }
        // Resolution already binds every target-authored body to its own
        // target's callees. Only portable callers still name family entries.
        // Rewriting the whole arena would change foreign bodies before they
        // are checked, judging them against another target's contracts.
        for machine_position in 0..typed.machines().len() {
            let machine = &typed.machines()[machine_position];
            if machine.target.is_some()
                || typed.target_machine_families.iter().any(|family| {
                    family
                        .members
                        .iter()
                        .any(|member| member.machine == machine.symbol)
                })
            {
                continue;
            }
            let machine = machine.clone();
            typed.retarget_machine_calls(&machine, &replacements);
        }
        reject_inert_sibling_callers(typed, target)
    }

    /// Join the exact resolved member back to source-admission custody.
    fn origin_index(
        &self,
        typed: &TypedTrees,
        member: &typed_trees::machine::TargetMachineFamilyMember,
    ) -> Result<usize, Vec<Diagnostic>> {
        let machine = typed.machine_by_symbol(member.machine);
        let source = typed.symbols.symbol_provenance_source_span(member.machine);
        let origin = machine.zip(source).and_then(|(machine, source)| {
            self.all_machine_origins.iter().position(|origin| {
                origin.scope == source::DependencyScope::Product
                    && origin.source == source.source_id
                    && origin.full_name == machine.name.as_str()
                    && origin.target == member.target.as_str()
            })
        });
        origin.ok_or_else(|| {
            vec![Diagnostic::error(
                "typed target-family member has no retained source declaration",
            )]
        })
    }

    /// Update only the two exact source rows selected by the typed family.
    fn reselect(&mut self, canonical_index: usize, wanted_index: Option<usize>) {
        self.all_machine_origins[canonical_index].selected = false;
        if let Some(wanted_index) = wanted_index {
            self.all_machine_origins[wanted_index].selected = true;
        }
        let canonical = &self.all_machine_origins[canonical_index];
        let wanted = wanted_index.map(|index| &self.all_machine_origins[index]);
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
        &self,
        context: crate::ProviderPlanningContext<'_>,
    ) -> Result<SettledTargetMachineDeclarations, Vec<Diagnostic>> {
        let typed = context.typed;
        // Source-instance identity survives target-body selection. A shared
        // roster must not feed the Build's defaults to Product or vice versa.
        // Missing source records remain a rejoin error below, not an omission.
        let outside_context = |source: source::SourceId| {
            typed
                .symbols
                .source_files()
                .find(|file| file.source_id == source)
                .is_some_and(|file| file.dependency_scope != context.scope)
        };
        let mut defaults = Vec::new();
        let mut origins = Vec::new();
        let mut diagnostics = Vec::new();
        for (machine_name, source) in &self.provider_default_machine_names {
            if outside_context(*source) {
                continue;
            }
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
            match crate::harvest_provider_selections(context, machine) {
                Ok(mut machine_defaults) => {
                    defaults.append(&mut machine_defaults);
                }
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        for (machine_name, target, source) in &self.selected_machine_origins {
            if outside_context(*source) {
                continue;
            }
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
            origins.push(crate::SelectedTargetMachineOrigin {
                machine: machine.symbol,
                target: target.clone(),
            });
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

/// Prepare canonical product families and the build's executable declarations.
/// Runtime product targets belong to later selection, not source preparation.
pub fn prepare_target_machines(
    syntax: &mut SyntaxTrees,
    sources: Option<&source::SourceMap>,
    build: Option<BuildTargetScope<'_>>,
) -> Result<SelectedTargetMachineDeclarations, Vec<Diagnostic>> {
    let (origins, selected_declarations) = target_machine_origins(syntax, sources, build.as_ref())?;
    validate_target_machine_origins(&origins)?;
    Ok(select_target_machines(
        syntax,
        origins,
        selected_declarations,
    ))
}

/// Construct one source roster, then choose the canonical member within each
/// exact namespace. No target-name map or second family grouping is needed.
fn target_machine_origins(
    syntax: &SyntaxTrees,
    sources: Option<&source::SourceMap>,
    build: Option<&BuildTargetScope<'_>>,
) -> Result<(Vec<TargetMachineOrigin>, Vec<ItemHandle>), Vec<Diagnostic>> {
    let modules = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Module(module) = item else {
                return None;
            };
            let path = syntax.items.identifier_path_members(module.path);
            Some((
                path.first()?.source_span().source_id,
                path.iter()
                    .map(|name| name.as_str().to_owned())
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    let mut origins = Vec::new();
    for &handle in syntax.root_item_handles() {
        let Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        let Some(target) = &machine.target else {
            continue;
        };
        let source = machine.name.source_span().source_id;
        let file = sources
            .map(|sources| {
                sources.get(source).ok_or_else(|| {
                    vec![
                        Diagnostic::error(
                            "target declaration has no source in the supplied source map",
                        )
                        .with_source_span(machine.name.source_span()),
                    ]
                })
            })
            .transpose()?;
        let package_identity = file.and_then(|file| file.package_identity);
        let namespace = TargetNamespace {
            package_identity,
            package_root: file
                .filter(|_| package_identity.is_none())
                .map(|file| file.package_root.clone())
                .unwrap_or_default(),
            module: modules
                .iter()
                .find(|(owner, _)| *owner == source)
                .map(|(_, module)| module.clone())
                .unwrap_or_default(),
        };
        origins.push((
            handle,
            TargetMachineOrigin {
                namespace,
                full_name: machine.name.as_str().to_owned(),
                target: target.as_str().to_owned(),
                scope: if build.is_some_and(|build| build.sources.contains(&source)) {
                    source::DependencyScope::Build
                } else {
                    source::DependencyScope::Product
                },
                source,
                selected: false,
            },
        ));
    }
    origins.sort_by(|left, right| compare_origins(&left.1, &right.1));
    for family in origins.chunk_by_mut(|left, right| left.1.same_family(&right.1)) {
        if family[0].1.scope == source::DependencyScope::Build {
            for (_, origin) in family {
                origin.selected = TargetProfile::from_canonical_target_name(&origin.target)
                    .is_ok_and(|target| build.is_some_and(|build| build.execution == target));
            }
            continue;
        }
        let canonical = family.iter().position(|(_, origin)| {
            NativeTarget::from_omega_target_name(Some(&origin.target)).is_ok()
        });
        if let Some(canonical) = canonical {
            // Mark duplicate rows too, so validation reports them instead of
            // making source order silently choose an implementation.
            let target = family[canonical].1.target.clone();
            for (_, origin) in family {
                origin.selected = origin.target == target;
            }
        }
    }
    // Carry exact syntax handles to the immediate consumer rather than joining
    // the roster back to declarations by name. They do not survive this pass.
    let selected_declarations = origins
        .iter()
        .filter_map(|(handle, origin)| origin.selected.then_some(*handle))
        .collect();
    Ok((
        origins.into_iter().map(|(_, origin)| origin).collect(),
        selected_declarations,
    ))
}

/// Admission consumes the already sorted source roster, including generated rows.
fn validate_target_machine_origins(origins: &[TargetMachineOrigin]) -> Result<(), Vec<Diagnostic>> {
    for family in origins.chunk_by(|left, right| left.same_family(right)) {
        let full_name = &family[0].full_name;
        let selected_count = family.iter().filter(|origin| origin.selected).count();
        if selected_count > 1 {
            return Err(vec![Diagnostic::error(format!(
                "machine `{full_name}` is implemented twice for the selected target -- \
                 a target supplies exactly one implementation of a contract machine",
            ))]);
        }
        for members in family.chunk_by(|left, right| left.target == right.target) {
            if members.len() > 1 {
                let target = &members[0].target;
                return Err(vec![Diagnostic::error(format!(
                    "machine `{full_name}` is implemented twice for `{target}` -- \
                     a target supplies exactly one implementation of a contract machine",
                ))]);
            }
        }
        if selected_count == 0 && family.len() >= 2 {
            let providers = family
                .iter()
                .map(|origin| origin.target.as_str())
                .collect::<Vec<_>>();
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
    all_machine_origins: Vec<TargetMachineOrigin>,
    selected_declarations: Vec<ItemHandle>,
) -> SelectedTargetMachineDeclarations {
    // PRV4c: a target package may contribute ordinary provider defaults with
    // a target-scoped, package-owned `Owner::provider_defaults` machine. Keep
    // the selected declarations' full names before erasing the target marker;
    // typed machines intentionally carry no deployment marker after this pass.
    let mut provider_default_machines = Vec::new();
    let mut selected_machine_origins = Vec::new();
    for handle in selected_declarations {
        let Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        let Some(target) = machine.target.as_ref() else {
            continue;
        };
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

/// A surviving caller of a body this target does not realize.
///
/// A family implemented only by other targets keeps its declaration as an
/// inert sibling with its callers, so the call still resolves. Nothing then
/// realizes the callee: checking accepts the program and the closure refuses
/// at `Legs::bump::linux_x86_64 ... selects no terminal machine for it`,
/// past `omega --check`. A caller that is itself scoped to the sibling's
/// target goes inert with it and is not reported.
fn reject_inert_sibling_callers(
    typed: &TypedTrees,
    target: TargetProfile,
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
        if TargetProfile::from_canonical_target_name(machine_target)
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
    let realized = target.target_name();
    let mut diagnostics = Vec::new();
    let mut callees = ExplicitStateCallees::default();
    for machine in typed.machines() {
        let caller_target = machine.target.as_ref().map(|target| target.as_str());
        for state in typed.machine_states(machine) {
            for called_symbol in callees.collect(typed, state) {
                let Some((_, callee, callee_target)) =
                    inert.iter().find(|(symbol, _, _)| symbol == called_symbol)
                else {
                    continue;
                };
                // The caller is exempt only when it carries the sibling's own
                // unrealized target and so went inert alongside it. Two
                // non-obvious callers land here: a dependency package's own
                // paradigm callers (tests/omega/packages/paradigm-internals),
                // and an inert machine's named-state transitions, which index
                // as calls to that machine's states.
                if caller_target == Some(*callee_target) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` calls `{callee}`, which is declared only for \
                     target `{callee_target}`. This build realizes `{realized}`, so that body \
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

#[cfg(test)]
mod tests {
    use super::{BuildTargetScope, SelectedTargetMachineDeclarations, prepare_target_machines};

    fn syntax(source_id: usize, source: &str) -> syntax_trees::SyntaxTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize target-machine fixture");
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(source_id), &tokens)
            .expect("parse target-machine fixture")
    }

    #[test]
    fn build_target_changes_only_build_scope_not_product_or_generated_families() {
        let build_sources = std::collections::HashSet::from([source::SourceId(1)]);
        for execution in [
            target::TargetProfile::LinuxX64,
            target::TargetProfile::WindowsX64,
        ] {
            let mut authored = syntax(
                0,
                "windows_x86_64 machine Product::value() -> u64 { 2 }\nlinux_x86_64 machine Product::value() -> u64 { 1 }\n",
            );
            authored.extend_from(&syntax(
                1,
                "windows_x86_64 machine Build::value() -> u64 { 4 }\nlinux_x86_64 machine Build::value() -> u64 { 3 }\n",
            ));
            let retained = prepare_target_machines(
                &mut authored,
                None,
                Some(BuildTargetScope {
                    execution,
                    sources: &build_sources,
                }),
            )
            .expect("each build target has a body");
            let mut generated = syntax(
                2,
                "windows_x86_64 machine Generated::value() -> u64 { 6 }\nlinux_x86_64 machine Generated::value() -> u64 { 5 }\n",
            );
            let retained = retained
                .filter_generated_extension(&mut generated, None)
                .expect("generated product families have no execution target");
            for origin in &retained.all_machine_origins {
                let expected = if origin.scope == source::DependencyScope::Build {
                    target::TargetProfile::from_canonical_target_name(&origin.target)
                        .expect("known authored target")
                        == execution
                } else {
                    origin.target == "linux_x86_64"
                };
                assert_eq!(origin.selected, expected, "{origin:?}");
            }
            assert_eq!(retained.all_machine_origins.len(), 6);
            assert_eq!(retained.selected_machine_origins.len(), 3);
        }
    }

    #[test]
    fn generated_extension_rejects_duplicate_against_authored_product() {
        let mut authored = syntax(0, "linux_x86_64 machine Product::value() -> u64 { 1 }\n");
        let retained = prepare_target_machines(&mut authored, None, None).expect("single member");
        let mut generated = syntax(1, "linux_x86_64 machine Product::value() -> u64 { 2 }\n");
        let diagnostics = retained
            .filter_generated_extension(&mut generated, None)
            .expect_err("generated source cannot replace an authored member");
        assert!(diagnostics[0].message.contains("implemented twice"));
    }

    #[test]
    fn build_profiles_with_equal_native_shapes_remain_distinct() {
        let mut program = syntax(
            0,
            "uefi_x86_64 machine helper() {}\nwindows_x86_64 machine helper() {}",
        );
        let sources = std::collections::HashSet::from([source::SourceId(0)]);
        let declarations = prepare_target_machines(
            &mut program,
            None,
            Some(BuildTargetScope {
                execution: target::TargetProfile::WindowsX64,
                sources: &sources,
            }),
        )
        .expect("equal native shapes do not merge distinct Build profiles");
        assert_eq!(declarations.selected_machine_origins.len(), 1);
        assert_eq!(declarations.selected_machine_origins[0].1, "windows_x86_64");
    }

    #[test]
    fn empty_target_declarations_settle_to_canonical_empty_defaults() {
        let settled = SelectedTargetMachineDeclarations::new(Vec::new(), Vec::new(), Vec::new())
            .settle_provider_defaults(crate::ProviderPlanningContext::new(
                &typed_trees::TypedTrees::default(),
                source::DependencyScope::Product,
            ))
            .expect("empty target declaration custody has no typed dependency");

        assert!(settled.provider_defaults.is_empty());
        assert!(settled.origins.is_empty());
    }

    #[test]
    fn generated_family_admission_uses_the_same_namespace_as_authored_source() {
        for (module, should_accept) in [("other", true), ("original", false)] {
            let mut sources = source::SourceMap::default();
            let base_text = "module original; linux_x86_64 machine helper() -> u64 { 1 }";
            let base_id = sources.add("base.omg".into(), base_text.into()).source_id;
            let mut base = syntax(base_id.0, base_text);
            let retained =
                prepare_target_machines(&mut base, Some(&sources), None).expect("authored family");
            let generated_text =
                format!("module {module}; linux_x86_64 machine helper() -> u64 {{ 2 }}");
            let generated_id = sources
                .add("generated.omg".into(), generated_text.clone())
                .source_id;
            let mut generated = syntax(generated_id.0, &generated_text);
            let result = retained.filter_generated_extension(&mut generated, Some(&sources));
            assert_eq!(result.is_ok(), should_accept);
            if let Err(errors) = result {
                assert!(errors[0].message.contains("implemented twice"));
            }
        }
    }

    #[test]
    fn reconciled_package_identity_not_source_root_owns_the_family() {
        let mut sources = source::SourceMap::default();
        let mut program = syntax_trees::SyntaxTrees::default();
        let identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32])
            .expect("package identity");
        let text = "linux_x86_64 machine helper() -> u64 { 1 }";
        for root in ["first", "second"] {
            let source_id = sources
                .add_with_metadata(
                    std::path::PathBuf::from(root).join("helper.omg"),
                    text.into(),
                    root.into(),
                    Some(identity),
                    source::SourceOrigin::User,
                )
                .source_id;
            program.extend_from(&syntax(source_id.0, text));
        }
        let errors = prepare_target_machines(&mut program, Some(&sources), None)
            .expect_err("one package identity cannot supply the same member twice");
        assert!(errors[0].message.contains("implemented twice"));
    }

    #[test]
    fn supplied_source_map_must_account_for_target_declarations() {
        let mut program = syntax(3, "linux_x86_64 machine helper() {}");
        let errors =
            prepare_target_machines(&mut program, Some(&source::SourceMap::default()), None)
                .expect_err("missing provenance is not a source-free fixture");
        assert!(errors[0].message.contains("no source"));
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
        let Err(diagnostics) =
            declarations.settle_provider_defaults(crate::ProviderPlanningContext::new(
                &typed_trees::TypedTrees::default(),
                source::DependencyScope::Product,
            ))
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

    #[test]
    fn generated_extension_retains_selected_origin_and_provider_default_custody() {
        let mut base = syntax(
            0,
            "linux_x86_64 machine Base::value() -> u64 { 1 }\nwindows_x86_64 machine Base::value() -> u64 { 2 }\n",
        );
        let retained = prepare_target_machines(&mut base, None, None)
            .expect("base target cohort selects exactly");
        let mut extension = syntax(
            1,
            "linux_x86_64 machine Generated::value() -> u64 { 3 }\nwindows_x86_64 machine Generated::value() -> u64 { 4 }\nlinux_x86_64 machine Generated::provider_defaults() { }\nwindows_x86_64 machine Generated::provider_defaults() { }\n",
        );

        let retained = retained
            .filter_generated_extension(&mut extension, None)
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
                let syntax_trees::item::Item::Machine(machine) = item else {
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

    /// Typing produces the family roster before the per-target swap:
    /// one row keys every authored body by
    /// target, the canonical member is the body whose target head the filter
    /// stripped, and the swap then consumes the roster to re-mark roles for
    /// the realized target. A family whose names resolve to no real target
    /// keeps no row.
    #[test]
    fn family_roster_keys_every_body_and_the_swap_consumes_it() {
        assert_family_roster_and_swap("");
    }

    #[test]
    fn module_owned_family_shares_its_signature_across_target_bodies() {
        assert_family_roster_and_swap("module arithmetic;\n");
    }

    #[test]
    fn same_named_target_families_remain_independent_across_modules() {
        assert_independent_target_families([
            ("first.omg", "module first;"),
            ("second.omg", "module second;"),
        ]);
    }

    #[test]
    fn equal_native_shapes_do_not_select_the_same_target_body() {
        let text = "data Helper { seed: u64; }\nuefi_x86_64 machine Helper::value(&self) -> u64 { 63 }\nwindows_x86_64 machine Helper::value(&self) -> u64 { 127 }";
        let mut sources = source::SourceMap::default();
        let source_id = sources.add("helper.omg".into(), text.into()).source_id;
        let mut program = syntax(source_id.0, text);
        let mut declarations = prepare_target_machines(&mut program, Some(&sources), None)
            .expect("distinct target bodies");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: program,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve attached target bodies");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type attached target bodies");
        let windows = typed.target_machine_families[0]
            .members
            .iter()
            .find(|member| member.target.as_str() == "windows_x86_64")
            .expect("Windows member")
            .machine;
        declarations
            .select_product_target(&mut typed, target::TargetProfile::WindowsX64)
            .expect("select Windows");
        assert!(
            typed
                .machine_by_symbol(windows)
                .expect("Windows body")
                .target
                .is_none(),
            "native shape equality must not retain the UEFI canonical body"
        );
    }

    #[test]
    fn missing_target_body_is_required_by_calls_not_family_size() {
        for foreign_profiles in [&["linux_x86_64"][..], &["linux_x86_64", "uefi_x86_64"][..]] {
            for called in [false, true] {
                let mut text = String::from("data Helper { seed: u64; }\n");
                for profile in foreign_profiles {
                    text.push_str(&format!(
                        "{profile} machine Helper::value(&self) -> u64 {{ 63 }}\n"
                    ));
                }
                let result = if called { "helper.value()" } else { "7u64" };
                text.push_str(&format!(
                    "machine portable(helper: &Helper) -> u64 {{ {result} }}"
                ));
                let mut sources = source::SourceMap::default();
                let source_id = sources.add("helper.omg".into(), text.clone()).source_id;
                let mut program = syntax(source_id.0, &text);
                let mut declarations = prepare_target_machines(&mut program, Some(&sources), None)
                    .expect("foreign families belong to the shared source set");
                let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                    syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                        syntax: program,
                        sources: Some(std::sync::Arc::new(sources)),
                        top_level_bindings: Vec::new(),
                    },
                )
                .expect("resolve every target body");
                let mut typed =
                    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                        .expect("type every target body");
                let selection = declarations
                    .select_product_target(&mut typed, target::TargetProfile::WindowsX64);
                if called {
                    let errors = selection.expect_err("a surviving call needs a Windows body");
                    assert!(errors.iter().any(|error| {
                        error.message.contains("call has no implementation")
                            && error.message.contains("portable")
                    }));
                } else {
                    selection.expect("unused foreign families do not demand a Windows body");
                    assert!(typed.target_machine_families.iter().all(|family| {
                        family.members.iter().all(|member| {
                            typed
                                .machine_by_symbol(member.machine)
                                .expect("retained foreign member")
                                .target
                                .is_some()
                        })
                    }));
                }
            }
        }
    }

    #[test]
    fn realization_retargets_portable_calls_without_rewriting_target_bodies() {
        let text = "data Helper { seed: u64; }
            linux_x86_64 machine Helper::value(&self) -> u64 { 1 }
            windows_x86_64 machine Helper::value(&self) -> u64 { 2 }
            linux_x86_64 machine Helper::run(&self) -> u64 {
                let value: u64 = self.finish(self.value());
                transition { _ -> value }
                state finish(&self, value: u64) -> u64 { value }
            }
            windows_x86_64 machine Helper::run(&self) -> u64 {
                let value: u64 = self.finish(self.value());
                transition { _ -> value }
                state finish(&self, value: u64) -> u64 { value }
            }
            machine portable(helper: &Helper) -> u64 { helper.run() }";
        let mut sources = source::SourceMap::default();
        let source_id = sources.add("helper.omg".into(), text.into()).source_id;
        let mut program = syntax(source_id.0, text);
        let mut declarations =
            prepare_target_machines(&mut program, Some(&sources), None).expect("target families");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: program,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve each target's own calls");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type each target's own calls");
        let mut inspection = super::ExplicitStateCallees::default();
        let target_calls = typed
            .target_machine_families
            .iter()
            .flat_map(|family| &family.members)
            .flat_map(|member| {
                typed.machine_states(typed.machine_by_symbol(member.machine).unwrap())
            })
            .map(|state| (state.clone(), inspection.collect(&typed, state).to_vec()))
            .collect::<Vec<_>>();
        let selected = typed
            .target_machine_families
            .iter()
            .find(|family| family.name.as_str() == "Helper::run")
            .unwrap()
            .members
            .iter()
            .find(|member| member.target.as_str() == "windows_x86_64")
            .unwrap()
            .machine;
        let selected_entry =
            typed.machine_states(typed.machine_by_symbol(selected).unwrap())[0].symbol;
        declarations
            .select_product_target(&mut typed, target::TargetProfile::WindowsX64)
            .expect("Windows realization");
        for (state, before) in target_calls {
            assert_eq!(
                inspection.collect(&typed, &state),
                before,
                "realization changed calls inside authored target state {}",
                state.name
            );
        }
        let portable = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "portable")
            .unwrap();
        assert_eq!(
            inspection.collect(&typed, &typed.machine_states(portable)[0]),
            &[selected_entry]
        );
    }

    #[test]
    fn missing_profile_body_is_not_supplied_by_equal_native_shape() {
        for profile in ["uefi_x86_64", "local_unchecked", "cross_platform_cli"] {
            let text = format!(
                "data Helper {{ seed: u64; }}\n{profile} machine Helper::value(&self) -> u64 {{ 63 }}\nmachine use_helper(helper: &Helper) -> u64 {{ helper.value() }}"
            );
            let mut sources = source::SourceMap::default();
            let source_id = sources.add("helper.omg".into(), text.clone()).source_id;
            let mut program = syntax(source_id.0, &text);
            let mut declarations = prepare_target_machines(&mut program, Some(&sources), None)
                .expect("retain one foreign family");
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: program,
                    sources: Some(std::sync::Arc::new(sources)),
                    top_level_bindings: Vec::new(),
                },
            )
            .expect("resolve foreign family call");
            let mut typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("type foreign family call");
            let errors = declarations
                .select_product_target(&mut typed, target::TargetProfile::WindowsX64)
                .expect_err("another profile does not supply a Windows implementation");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("call has no implementation"))
            );
            assert!(
                errors
                    .iter()
                    .all(|error| !error.message.contains("or `uefi_x86_64`"))
            );
        }
    }

    #[test]
    fn same_named_target_families_remain_independent_across_packages() {
        assert_independent_target_families([("first/helper.omg", ""), ("second/helper.omg", "")]);
    }

    fn assert_independent_target_families(units: [(&str, &str); 2]) {
        let mut sources = source::SourceMap::default();
        let mut program = syntax_trees::SyntaxTrees::default();
        for (path, module) in units {
            let text = format!(
                "{module}\nlinux_x86_64 machine helper() -> u64 {{ 1 }}\nwindows_x86_64 machine helper() -> u64 {{ 2 }}"
            );
            let source_id = sources.add(path.into(), text.clone()).source_id;
            program.extend_from(&syntax(source_id.0, &text));
        }
        let mut declarations = prepare_target_machines(&mut program, Some(&sources), None)
            .expect("equal names in distinct namespaces are separate families");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: program,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("independent families resolve");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("independent families type");
        assert_eq!(typed.target_machine_families.len(), 2);
        let selected = typed
            .target_machine_families
            .iter()
            .map(|family| {
                family
                    .members
                    .iter()
                    .find(|member| member.target.as_str() == "windows_x86_64")
                    .expect("each family has a Windows member")
                    .machine
            })
            .collect::<Vec<_>>();
        declarations
            .select_product_target(&mut typed, target::TargetProfile::WindowsX64)
            .expect("realization consumes each exact family");
        for symbol in selected {
            assert!(
                typed
                    .machine_by_symbol(symbol)
                    .expect("retained member")
                    .target
                    .is_none()
            );
        }
        let settled = declarations
            .settle_provider_defaults(crate::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ))
            .expect("selected origins retain both namespaces");
        assert_eq!(settled.origins.len(), 2);
        assert_ne!(settled.origins[0].machine, settled.origins[1].machine);
    }

    fn assert_family_roster_and_swap(module: &str) {
        // Resolution retains product scope from the source map.
        let text = format!(
            "{module}demo_target machine inert_helper() -> u64 {{ 3u64 }}\nlinux_x86_64 machine family_value() -> u64 {{ 1u64 }}\nwindows_x86_64 machine family_value() -> u64 {{ 2u64 }}\n"
        );
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add(
                std::path::PathBuf::from("tests/family.omg"),
                text.to_owned(),
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .expect("tokenize target-machine fixture");
        let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("parse target-machine fixture");
        let mut declarations =
            prepare_target_machines(&mut syntax, Some(&sources), None).expect("cohort selects");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("siblings resolve");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("siblings type");

        let [family] = typed.target_machine_families.as_slice() else {
            panic!("the recognized family is the only roster row");
        };
        assert_eq!(family.name.as_str(), "family_value");
        assert_eq!(
            family
                .members
                .iter()
                .map(|member| member.target.as_str())
                .collect::<Vec<_>>(),
            vec!["linux_x86_64", "windows_x86_64"]
        );
        let canonical = family.canonical;
        let windows_member = family.members[1].machine;
        fn machine_of(
            typed: &typed_trees::TypedTrees,
            symbol: symbols::SymbolHandle,
        ) -> &typed_trees::machine::Machine {
            typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .expect("a roster member names a typed machine")
        }
        assert!(
            machine_of(&typed, canonical).target.is_none(),
            "the canonical body sits at the family path"
        );
        assert_eq!(family.members[0].machine, canonical);

        declarations
            .select_product_target(&mut typed, target::TargetProfile::WindowsX64)
            .expect("the swap consumes the roster");

        assert_eq!(
            machine_of(&typed, canonical)
                .target
                .as_ref()
                .map(|target| target.as_str()),
            Some("linux_x86_64"),
            "the demoted canonical body takes its authored target marker"
        );
        assert!(
            machine_of(&typed, windows_member).target.is_none(),
            "the realized target's body moves to the family path"
        );
        // The roster itself is the durable record: the swap's renames do not
        // disturb which symbol keys which authored target.
        let family = &typed.target_machine_families[0];
        assert_eq!(family.canonical, canonical);
        assert_eq!(family.members[1].machine, windows_member);
    }

    #[test]
    fn generated_units_reject_a_duplicate_selected_target_row() {
        let mut base = syntax(0, "const BASE: u64 = 1;\n");
        let retained =
            prepare_target_machines(&mut base, None, None).expect("base has no target rows");
        let mut extension = syntax(1, "linux_x86_64 machine Duplicate::value() -> u64 { 2 }\n");
        let second = syntax(2, "linux_x86_64 machine Duplicate::value() -> u64 { 3 }\n");
        extension.extend_from(&second);

        let diagnostics = retained
            .filter_generated_extension(&mut extension, None)
            .expect_err("generated units must not split a duplicate selected row");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("implemented twice"));
        assert!(diagnostics[0].message.contains("Duplicate::value"));
    }
}
