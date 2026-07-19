//! PRV3 (the admission vertical): authored `provides` blocks DERIVE
//! ProviderPlan values -- the bridge from today's rows to the typed
//! carrier -- and each plan is admitted through the chapter-10 trust path:
//! own-package dev-active (standing warning) until the final build grants
//! it by name, with the lockfile receipt hashing the plan's NORMALIZED
//! IDENTITY (identity_fingerprint), so a plan that changes under a grant
//! drifts. Implicit selection consumes only a unique covering candidate;
//! explicit binding under slot-owner authority lands with PRV4c's build API.

use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use omega_typed_trees::TypedTrees;

/// Derive one plan per authored `<target> provides <Trait> { .. }` block.
/// The schema reifies from the TYPED boundary trait when it exists (the
/// honest surface); a provides block over an unknown trait derives an
/// empty-schema plan whose validation names every row as stray -- loud,
/// never silent.
pub(super) fn derive_provider_plans(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    typed: &TypedTrees,
) -> Vec<ProviderPlan> {
    let mut plans: Vec<ProviderPlan> = Vec::new();
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::HostProvider(provider) = item else {
            continue;
        };
        let trait_leaf = syntax_trees
            .items
            .identifier_path_members(provider.boundary_trait)
            .last()
            .map(|member| member.as_str().to_owned())
            .unwrap_or_default();
        let schema = typed
            .traits()
            .iter()
            .find(|definition| {
                definition.name.as_str() == trait_leaf
                    || definition
                        .name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .is_some_and(|leaf| leaf == trait_leaf)
            })
            .and_then(|definition| ServiceSchema::from_typed(typed, definition))
            .unwrap_or_else(|| ServiceSchema {
                trait_name: trait_leaf.clone(),
                methods: Vec::new(),
            });
        let mut rows = Vec::new();
        for mapping in syntax_trees.items.host_provider_mappings(provider.mappings) {
            use omega_syntax_trees::item::HostProviderMappingKind;
            let binding = match &mapping.binding {
                HostProviderMappingKind::Syscall { number } => ProviderBinding::Syscall {
                    number: u32::try_from(*number).unwrap_or_default(),
                },
                HostProviderMappingKind::DllImport { module, symbol } => ProviderBinding::Import {
                    library: module.as_str().to_owned(),
                    symbol: symbol.as_str().to_owned(),
                },
                HostProviderMappingKind::VtableSlot { index } => {
                    ProviderBinding::VtableSlot { index: *index }
                }
                HostProviderMappingKind::VtableField { field } => ProviderBinding::VtableField {
                    table: provider.vtable_struct.as_str().to_owned(),
                    field: field.as_str().to_owned(),
                },
                HostProviderMappingKind::TableFunction { field } => {
                    ProviderBinding::TableFunction {
                        table: provider.vtable_struct.as_str().to_owned(),
                        field: field.as_str().to_owned(),
                    }
                }
                HostProviderMappingKind::Value { value } => {
                    ProviderBinding::Value { value: *value }
                }
            };
            rows.push(ProviderPlanRow {
                method: mapping.machine.as_str().to_owned(),
                binding,
                call_shape: None,
            });
        }
        plans.push(ProviderPlan {
            name: format!("{}::{}", provider.target.as_str(), trait_leaf),
            provider_type: String::new(),
            target: provider.target.as_str().to_owned(),
            schema,
            rows,
            effect_set: omega_effects::EffectSet::empty(),
            origin_package: String::new(),
        });
    }
    plans
}

/// PRV4 order step (2): derive plans from explicit SATISFIES edges -- one
/// plan per (provider type, boundary trait, target), assembled only from
/// that provider's conformance closure. External leaves and checked adapters
/// attached to the same provider type join one plan; legacy free machines
/// retain one anonymous compatibility candidate until PRV4f. Coverage never
/// combines unrelated provider types. Coverage/signatures come from the typed schema
/// (signature refinement is enforced by the conformance checker on each
/// edge); the effect surface is the union of the SATISFIED requirements'
/// declared effects -- the requirement supplies the ceiling, never the
/// leaf. Selection v1: a slot whose (trait, target) has exactly one FULLY
/// COVERING derived plan selects it implicitly; ambiguity or partial
/// coverage is loud at the consumer (the trust report shows coverage).
pub(crate) fn derive_satisfies_plans(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    let mut plans: Vec<ProviderPlan> = Vec::new();
    for item in syntax_trees.root_items() {
        let omega_syntax_trees::item::Item::Machine(machine) = item else {
            continue;
        };
        if machine.boundary {
            continue;
        }
        for clause in syntax_trees.items.satisfies_clauses(machine.satisfies) {
            // A bodyless leaf carries `via`; a CHECKED ADAPTER is an
            // ordinary machine with a body and a requirement-named
            // satisfies edge (no via). Both contribute rows; whole-trait
            // conformances (no requirement) are the trait system's
            // ordinary business and derive nothing here.
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let binding_kind = match (&clause.via, machine.bodyless) {
                (Some(binding), true) => Some(binding.clone()),
                (None, false) => {
                    // A CHECKED ADAPTER derives a plan row only over a
                    // BOUNDARY trait (a service schema). A plain trait's
                    // conformance -- including its effect ceiling -- is the
                    // existing trait machinery's business (the decision-20
                    // admission fixtures pin it) and derives nothing here.
                    let is_boundary_trait = typed.traits().iter().any(|definition| {
                        definition.is_boundary
                            && (definition.name.as_str() == clause.trait_name.as_str()
                                || definition
                                    .name
                                    .as_str()
                                    .rsplit("::")
                                    .next()
                                    .is_some_and(|leaf| leaf == clause.trait_name.as_str()))
                    });
                    if !is_boundary_trait {
                        continue;
                    }
                    None
                }
                _ => continue, // refused elsewhere (via rungs)
            };
            let binding = binding_kind.as_ref();
            let _ = &binding;
            // The selected target-machine marker is cleared before lowering
            // so the machine behaves ordinarily. Recover the deployment
            // dimension from compile selection for plan identity/selection;
            // otherwise a target-scoped leaf silently becomes a universal
            // provider after it is selected.
            let target = machine.target.as_ref().map_or_else(
                || selected_target.unwrap_or_default().to_owned(),
                |target| target.as_str().to_owned(),
            );
            let trait_leaf = clause.trait_name.as_str().to_owned();
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &trait_leaf, &provider_type);
            let position = plans
                .iter()
                .position(|plan| plan.name == plan_name)
                .unwrap_or_else(|| {
                    let schema = typed
                        .traits()
                        .iter()
                        .find(|definition| {
                            definition.name.as_str() == trait_leaf
                                || definition
                                    .name
                                    .as_str()
                                    .rsplit("::")
                                    .next()
                                    .is_some_and(|leaf| leaf == trait_leaf)
                        })
                        .and_then(|definition| ServiceSchema::from_typed(typed, definition))
                        .unwrap_or_else(|| ServiceSchema {
                            trait_name: trait_leaf.clone(),
                            methods: Vec::new(),
                        });
                    plans.push(ProviderPlan {
                        name: plan_name.clone(),
                        provider_type: provider_type.clone(),
                        target: target.clone(),
                        schema,
                        rows: Vec::new(),
                        effect_set: omega_effects::EffectSet::empty(),
                        origin_package: String::new(),
                    });
                    plans.len() - 1
                });
            let plan = &mut plans[position];
            use omega_syntax_trees::item::HostProviderMappingKind;
            let row_binding = match binding {
                None => ProviderBinding::CheckedAdapter {
                    machine: machine.name.as_str().to_owned(),
                },
                Some(binding) => match binding {
                    HostProviderMappingKind::Syscall { number } => ProviderBinding::Syscall {
                        number: u32::try_from(*number).unwrap_or_default(),
                    },
                    HostProviderMappingKind::DllImport { module, symbol } => {
                        ProviderBinding::Import {
                            library: module.clone(),
                            symbol: symbol.clone(),
                        }
                    }
                    HostProviderMappingKind::VtableSlot { index } => {
                        ProviderBinding::VtableSlot { index: *index }
                    }
                    HostProviderMappingKind::VtableField { field } => {
                        ProviderBinding::VtableField {
                            table: String::new(),
                            field: field.as_str().to_owned(),
                        }
                    }
                    HostProviderMappingKind::TableFunction { field } => {
                        ProviderBinding::TableFunction {
                            table: String::new(),
                            field: field.as_str().to_owned(),
                        }
                    }
                    HostProviderMappingKind::Value { value } => {
                        ProviderBinding::Value { value: *value }
                    }
                },
            };
            plan.rows.push(ProviderPlanRow {
                method: requirement.as_str().to_owned(),
                binding: row_binding,
                call_shape: None,
            });
            // The effect CEILING: the satisfied requirement's declared
            // effects, from the schema.
            let mut ceiling = plan.effect_set;
            if let Some(method) = plan
                .schema
                .methods
                .iter()
                .find(|method| method.name == requirement.as_str())
            {
                for effect in &method.effects {
                    ceiling.insert_name(effect);
                }
            }
            plan.effect_set = ceiling;
        }
    }
    plans
}

/// The stable name shared by derivation, reports, selection, and backend row
/// extraction. The anonymous form preserves the free-machine migration bridge;
/// a real provider type is deliberately visible in artifact identity.
pub(crate) fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

/// PRV4 adapter ADMISSION (the refinement half): a checked adapter's
/// TRANSITIVE effects must fit inside the satisfied requirement's declared
/// ceiling -- the requirement is the public contract; a hidden effect in
/// the body refuses loudly (the decision-20 provider-admission rule, now
/// enforced at plan derivation).
pub(crate) fn validate_adapter_refinement(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Vec<omega_core::diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let effect_plan = omega_effects::infer_effects(typed);
    for plan in plans {
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                continue;
            };
            let Some(adapter) = typed
                .machines()
                .iter()
                .find(|candidate| candidate.name.as_str() == machine.as_str())
            else {
                continue;
            };
            let transitive = effect_plan
                .machines()
                .iter()
                .find(|entry| entry.symbol == adapter.symbol)
                .map(|entry| entry.transitive)
                .unwrap_or_else(omega_effects::EffectSet::empty);
            let ceiling: Vec<&str> = plan
                .schema
                .methods
                .iter()
                .find(|method| method.name == row.method)
                .map(|method| method.effects.iter().map(String::as_str).collect())
                .unwrap_or_default();
            let hidden: Vec<&str> = transitive
                .names()
                .filter(|name| !ceiling.contains(name))
                .collect();
            if !hidden.is_empty() {
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body reaches effect(s) \
                     [{}] outside the requirement's declared ceiling [{}] -- the \
                     satisfied requirement is the public contract; widen it or drop \
                     the effect",
                    machine,
                    plan.schema.trait_name,
                    row.method,
                    hidden.join(", "),
                    ceiling.join(", "),
                )));
            }
        }
    }
    diagnostics
}

/// PRV4 step (2) selection v1: a SLOT -- a (boundary trait, target) pair --
/// selects its provider implicitly when exactly one FULLY COVERING plan
/// exists; two covering plans are AMBIGUOUS and refuse loudly, naming both
/// (the build.omg per-slot override spelling rides the target-package
/// surface and will resolve such ties explicitly). Partially covering
/// plans never select and never collide -- the trust report's coverage
/// column is their surface.
pub(crate) fn validate_slot_selection(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
) -> Vec<omega_core::diagnostics::Diagnostic> {
    // Target inertness (the fail-canary host-portability convention): a
    // plan scoped to a NON-selected target is inert and never collides --
    // only plans that RESOLVE to the selected target participate.
    let applies = |target: &str| -> bool {
        if target.is_empty() {
            return true; // portable: every target
        }
        omega_target::NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == selected_target)
    };
    let mut diagnostics = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        if plan.schema.methods.is_empty() || !plan.covers_schema() || !applies(&plan.target) {
            continue;
        }
        for other in plans.iter().skip(index + 1) {
            if other.schema.trait_name == plan.schema.trait_name
                && applies(&other.target)
                && other.covers_schema()
                && !other.schema.methods.is_empty()
            {
                diagnostics.push(omega_core::diagnostics::Diagnostic::error(format!(
                    "slot `{}` (target `{}`) has two covering provider plans: `{}` \
                     [{:016x}] and `{}` [{:016x}] -- selection is implicit only when \
                     unique; retire one or scope them to different targets",
                    plan.schema.trait_name,
                    if plan.target.is_empty() {
                        "portable"
                    } else {
                        &plan.target
                    },
                    plan.name,
                    plan.identity_fingerprint(),
                    other.name,
                    other.identity_fingerprint(),
                )));
            }
        }
    }
    diagnostics
}

/// Return the uniquely selected covering plans for the current target.
/// Partial plans remain reportable candidates but contribute no backend rows,
/// preventing unrelated providers from being assembled accidentally.
pub(crate) fn implicitly_selected_plan_names(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
) -> Vec<String> {
    let applies = |target: &str| -> bool {
        target.is_empty()
            || omega_target::NativeTarget::from_omega_target_name(Some(target))
                .is_ok_and(|resolved| resolved == selected_target)
    };
    let mut selected = Vec::new();
    for plan in plans {
        if plan.schema.methods.is_empty() || !plan.covers_schema() || !applies(&plan.target) {
            continue;
        }
        let covering_count = plans
            .iter()
            .filter(|candidate| {
                candidate.schema.trait_name == plan.schema.trait_name
                    && !candidate.schema.methods.is_empty()
                    && candidate.covers_schema()
                    && applies(&candidate.target)
            })
            .count();
        if covering_count == 1 {
            selected.push(plan.name.clone());
        }
    }
    selected
}

/// P4a: the CONSOLE methods the platform block declares -- the vertical's
/// scope fence.
pub(crate) const CONSOLE_METHODS: &[&str] = &[
    "write_line",
    "write",
    "read_line",
    "read_byte",
    "write_byte",
    "exit_process",
];

/// P4a (the lossless-representation oracle): derive the built-in Console
/// plan FROM a populated host-ABI plan's platform lowerings. The round
/// trip back to (operations, PlatformCallData) rows must be exact --
/// proven before the populate tables can retire into authored plans.
pub(crate) fn builtin_console_plan(
    target_name: &str,
    abi_plan: &omega_calling_conventions::HostAbiPlan,
) -> omega_effects::provider_plan::ProviderPlan {
    let mut rows = Vec::new();
    for (_, lowering) in abi_plan.platform_call_lowerings.iter() {
        if !CONSOLE_METHODS.contains(&lowering.state.as_ref()) {
            continue;
        }
        // The lowering's platform field discriminates same-named states:
        // the Console surface rides the wildcard/Console platforms; the
        // filesystem raw seam's `write`/`read` ride "FilesystemHost".
        if lowering.platform.as_ref() != "*" && lowering.platform.as_ref() != "Console" {
            continue;
        }
        let operations = abi_plan
            .host_operations
            .span_or_empty(lowering.operations)
            .iter()
            .map(|reference| {
                format!(
                    "{}::{}",
                    reference.key.capability.name(),
                    reference.key.operation.name()
                )
            })
            .collect();
        rows.push(ProviderPlanRow {
            method: lowering.state.as_ref().to_owned(),
            binding: ProviderBinding::HostOperations { operations },
            call_shape: lowering.data.render_call_shape(),
        });
    }
    ProviderPlan {
        name: format!("{target_name}::Console"),
        provider_type: String::new(),
        target: target_name.to_owned(),
        schema: ServiceSchema {
            trait_name: "Console".to_owned(),
            methods: Vec::new(),
        },
        rows,
        effect_set: omega_effects::EffectSet::empty(),
        origin_package: "omega::language::std".to_owned(),
    }
}

/// The inverse: a plan row back to the lowering pair. Errors name the
/// defect (the merge seam's refusal surface).
pub(crate) fn plan_row_to_lowering(
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<
    (
        Vec<omega_calling_conventions::HostOperationKey>,
        omega_calling_conventions::PlatformCallData,
    ),
    String,
> {
    let ProviderBinding::HostOperations { operations } = &row.binding else {
        return Err(format!(
            "row `{}` is not a host-operations binding",
            row.method
        ));
    };
    let mut keys = Vec::with_capacity(operations.len());
    for operation in operations {
        let Some((capability, name)) = operation.split_once("::") else {
            return Err(format!("malformed host operation `{operation}`"));
        };
        keys.push(omega_calling_conventions::HostOperationKey::from_names(
            capability, name,
        ));
    }
    let data =
        omega_calling_conventions::PlatformCallData::parse_call_shape(row.call_shape.as_deref())?;
    Ok((keys, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection_plan(name: &str, methods: &[&str], rows: &[&str]) -> ProviderPlan {
        ProviderPlan {
            name: name.to_owned(),
            provider_type: name.to_owned(),
            target: String::new(),
            schema: ServiceSchema {
                trait_name: "Pair".to_owned(),
                methods: methods
                    .iter()
                    .map(|method| omega_effects::provider_plan::ServiceMethod {
                        name: (*method).to_owned(),
                        parameter_count: 0,
                        has_result: false,
                        effects: Vec::new(),
                    })
                    .collect(),
            },
            rows: rows
                .iter()
                .map(|method| ProviderPlanRow {
                    method: (*method).to_owned(),
                    binding: ProviderBinding::VtableSlot { index: 0 },
                    call_shape: None,
                })
                .collect(),
            effect_set: omega_effects::EffectSet::empty(),
            origin_package: String::new(),
        }
    }

    #[test]
    fn implicit_selection_never_combines_partial_candidates() {
        let plans = vec![
            selection_plan("FirstProvider", &["first", "second"], &["first"]),
            selection_plan("SecondProvider", &["first", "second"], &["second"]),
        ];
        assert!(
            implicitly_selected_plan_names(&plans, omega_target::NativeTarget::host()).is_empty(),
            "two partial candidates are not one provider"
        );
    }

    #[test]
    fn implicit_selection_returns_the_unique_covering_candidate() {
        let plans = vec![
            selection_plan(
                "CompleteProvider",
                &["first", "second"],
                &["first", "second"],
            ),
            selection_plan("PartialProvider", &["first", "second"], &["first"]),
        ];
        assert_eq!(
            implicitly_selected_plan_names(&plans, omega_target::NativeTarget::host()),
            vec!["CompleteProvider".to_owned()]
        );
    }

    #[test]
    fn console_plan_round_trips_the_populate_tables() {
        // P4a ORACLE: for every host target, the Console rows of the
        // populated ABI plan survive the plan representation LOSSLESSLY --
        // derive the plan from the table, convert every row back, and the
        // (operations, PlatformCallData) pairs are exact. This is the
        // precondition for retiring insert_platform_lowering into authored
        // plans (P4a-2).
        for format in [
            omega_target::ObjectFormat::Coff,
            omega_target::ObjectFormat::Elf,
            omega_target::ObjectFormat::MachO,
        ] {
            let target = omega_target::NativeTarget {
                object_format: format,
                ..omega_target::NativeTarget::host()
            };
            let abi_plan = omega_calling_conventions::build_host_abi_plan(target);
            let plan = builtin_console_plan("probe", &abi_plan);
            assert!(
                !plan.rows.is_empty(),
                "{format:?}: the Console surface must derive rows"
            );
            let mut matched = 0usize;
            for (_, lowering) in abi_plan.platform_call_lowerings.iter() {
                if !CONSOLE_METHODS.contains(&lowering.state.as_ref()) {
                    continue;
                }
                if lowering.platform.as_ref() != "*" && lowering.platform.as_ref() != "Console" {
                    continue;
                }
                let row = plan
                    .rows
                    .iter()
                    .find(|row| row.method == lowering.state.as_ref())
                    .unwrap_or_else(|| panic!("{format:?}: no row for {}", lowering.state));
                let (keys, data) = plan_row_to_lowering(row).expect("derived rows convert back");
                let table_keys: Vec<omega_calling_conventions::HostOperationKey> = abi_plan
                    .host_operations
                    .span_or_empty(lowering.operations)
                    .iter()
                    .map(|reference| reference.key)
                    .collect();
                assert_eq!(
                    keys, table_keys,
                    "{format:?}: {} operations",
                    lowering.state
                );
                assert_eq!(
                    data, lowering.data,
                    "{format:?}: {} call data",
                    lowering.state
                );
                matched += 1;
            }
            assert!(
                matched >= 4,
                "{format:?}: expected the Console surface, matched {matched}"
            );
        }
    }
}
