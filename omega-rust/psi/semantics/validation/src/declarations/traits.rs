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
