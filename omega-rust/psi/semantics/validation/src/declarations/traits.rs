//! Traits: each trait's requirements, the conformance bounds that traits and
//! generic machines declare, the conformances that machines and named
//! conformance items claim, external `via` realizations, and local dynamic
//! trait values.
//!
//! There is no single entrance; `program_validation::validate` calls these
//! validators. Before its per-machine loop, in this order:
//! `validate_trait_requirements` (`requirements`),
//! `validate_trait_conformance_bounds` for each trait, `validate_conformances`
//! for named data conformance items (`data_conformance`), and
//! `collect_dynamic_conformance_selections` (`dynamic`). For each machine, in
//! this order: `validate_generic_conformance_bounds`,
//! `validate_machine_trait_conformances`, `validate_external_via_expression`
//! for each conformance, and `validate_external_leaf_native_shapes` for an
//! external leaf with one `via` binding. The bound, machine-conformance and
//! external checks come from `conformance`.
//!
//! Other readers. Call validation in `machine_calls::calls` uses the generic
//! bound queries from `conformance`, `arguments_for_declaring_trait` (also
//! read by `proof_contracts::quotients`) and `dynamic_requirement_call_error`;
//! `value_custody::recasts` reads the dynamic selections and
//! `dynamic_trait_symbol`. The typed-to-checked stage and the checked
//! compilation build continuation call `resolve_dynamic_call_targets`; the
//! typed-to-checked stage also reads the dynamic selections and descriptor
//! storages, `compose_forwarded_trait_arguments` and
//! `generic_bound_operator_requirement`. Package review calls
//! `revalidate_top_level_requirement_realization`.
//!
//! `trait_definition_by_symbol` is the trait lookup that the `conformance`,
//! `data_conformance` and `requirements` children share.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::trait_definition::TraitDefinition;

mod conformance;
mod data_conformance;
mod dynamic;
mod requirements;

pub use conformance::compose_forwarded_trait_arguments;
pub use conformance::generic_bound_operator_requirement;
pub use conformance::revalidate_top_level_requirement_realization;
pub(crate) use conformance::{
    GenericBoundRequirement, generic_bound_argument_matches, generic_bound_requirement_call,
    named_conformance_argument_matches, validate_external_leaf_native_shapes,
    validate_external_via_expression, validate_generic_conformance_bounds,
    validate_machine_trait_conformances, validate_trait_conformance_bounds,
};
pub(crate) use data_conformance::{arguments_for_declaring_trait, validate_conformances};
pub use dynamic::{
    DynamicConformanceSelection, DynamicDescriptorStorage, collect_dynamic_conformance_selections,
    collect_dynamic_descriptor_storages, resolve_dynamic_call_targets,
};
pub(crate) use dynamic::{dynamic_requirement_call_error, dynamic_trait_symbol};
pub(crate) use requirements::validate_trait_requirements;

/// The trait `symbol` declares, or `None` for an invalid symbol or a symbol
/// that names no trait. Every conformance, requirement, and data-conformance
/// validator below resolves its trait through this one lookup.
fn trait_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}
