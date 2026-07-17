//! PRV3 (the admission vertical): authored `provides` blocks DERIVE
//! ProviderPlan values -- the bridge from today's rows to the typed
//! carrier -- and each plan is admitted through the chapter-10 trust path:
//! own-package dev-active (standing warning) until the final build grants
//! it by name, with the lockfile receipt hashing the plan's NORMALIZED
//! IDENTITY (identity_fingerprint), so a plan that changes under a grant
//! drifts. Selection (binding a plan to a slot) is the separately held
//! slot-owner capability and lands with PRV4's target packages.

use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema,
};
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
                HostProviderMappingKind::Syscall { number } => {
                    ProviderBinding::Syscall {
                        number: u32::try_from(*number).unwrap_or_default(),
                    }
                }
                HostProviderMappingKind::DllImport { module, symbol } => {
                    ProviderBinding::Import {
                        library: module.as_str().to_owned(),
                        symbol: symbol.as_str().to_owned(),
                    }
                }
                HostProviderMappingKind::VtableSlot { index } => {
                    ProviderBinding::VtableSlot { index: *index }
                }
                HostProviderMappingKind::VtableField { field } => {
                    ProviderBinding::VtableField {
                        table: provider.vtable_struct.as_str().to_owned(),
                        field: field.as_str().to_owned(),
                    }
                }
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
            target: provider.target.as_str().to_owned(),
            schema,
            rows,
            effect_set: omega_effects::EffectSet::empty(),
            origin_package: String::new(),
        });
    }
    plans
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
        return Err(format!("row `{}` is not a host-operations binding", row.method));
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
    let data = omega_calling_conventions::PlatformCallData::parse_call_shape(
        row.call_shape.as_deref(),
    )?;
    Ok((keys, data))
}


#[cfg(test)]
mod tests {
    use super::*;

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
                if lowering.platform.as_ref() != "*"
                    && lowering.platform.as_ref() != "Console"
                {
                    continue;
                }
                let row = plan
                    .rows
                    .iter()
                    .find(|row| row.method == lowering.state.as_ref())
                    .unwrap_or_else(|| panic!("{format:?}: no row for {}", lowering.state));
                let (keys, data) =
                    plan_row_to_lowering(row).expect("derived rows convert back");
                let table_keys: Vec<omega_calling_conventions::HostOperationKey> = abi_plan
                    .host_operations
                    .span_or_empty(lowering.operations)
                    .iter()
                    .map(|reference| reference.key)
                    .collect();
                assert_eq!(keys, table_keys, "{format:?}: {} operations", lowering.state);
                assert_eq!(data, lowering.data, "{format:?}: {} call data", lowering.state);
                matched += 1;
            }
            assert!(matched >= 4, "{format:?}: expected the Console surface, matched {matched}");
        }
    }
}
