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
